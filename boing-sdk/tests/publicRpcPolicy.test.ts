/**
 * Drift guard for the public RPC edge (docs/PUBLIC-RPC-WAF-GUIDANCE.md): every method the node serves must be
 * classified in ops/cloudflare/public-rpc/policy.json (allowed class or explicit blocked list), and the
 * generated Cloudflare ruleset / Worker table / wrangler bindings must match policy.json.
 */
import fs from 'node:fs';
import { describe, expect, it } from 'vitest';
// @ts-expect-error plain ESM helper without types
import * as lib from '../../scripts/lib/public-rpc-policy.mjs';
import {
  checkRates,
  classify,
  denial,
  inspectBody,
  parseMode,
  planRequest,
  type RateLimitBinding,
} from '../../workers/public-rpc-gateway/src/edgePolicy';
import { CLASS_LIMITS, MAX_BATCH_SIZE } from '../../workers/public-rpc-gateway/src/methodPolicy.generated';

const policy = lib.loadPolicy();
const { all: catalog } = lib.catalogMethods() as { all: string[] };
const norm = (s: string) => s.replace(/\r\n/g, '\n');

describe('public RPC policy vs node method catalog', () => {
  it('classifies every catalog method exactly once and has no stale entries', () => {
    expect(lib.validatePolicy(policy, catalog)).toEqual([]);
    expect(catalog.length).toBeGreaterThanOrEqual(66);
  });

  it('fails when a catalog method is neither allowed nor explicitly blocked', () => {
    const problems = lib.validatePolicy(policy, [...catalog, 'boing_brandNewMethod']);
    expect(problems.join('\n')).toContain('boing_brandNewMethod is in the node catalog but in neither');
  });

  it('fails on stale and duplicate entries', () => {
    const dup = structuredClone(policy);
    dup.classes.blocked.methods.push('boing_getBalance');
    expect(lib.validatePolicy(dup, catalog).join('\n')).toContain('boing_getBalance is in both');
    expect(lib.validatePolicy(policy, catalog.filter((m) => m !== 'boing_getBalance')).join('\n')).toContain('stale entry');
  });

  it('blocks every operator method and never blocks transaction submission or reads', () => {
    const table = lib.methodTable(policy) as Record<string, string>;
    for (const m of catalog.filter((x) => /operator/i.test(x))) expect(table[m]).toBe('blocked');
    expect(table.boing_submitTransaction).toBe('submit');
    for (const m of ['boing_getBalance', 'boing_getBlockByHeight', 'boing_health', 'boing_getNetworkInfo', 'boing_getTransactionReceipt']) {
      expect(table[m]).toBe('read');
    }
  });

  it('keeps limits ordered: submit tighter than reads, then write, then faucet', () => {
    const l = policy.worker.limits;
    expect(l.submit.limit).toBeLessThan(l.read.limit);
    expect(l.write.limit).toBeLessThanOrEqual(l.submit.limit);
    expect(l.faucet.limit).toBeLessThanOrEqual(l.write.limit);
    expect(l.heavy_read.limit).toBeLessThan(l.read.limit);
    for (const c of ['read', 'heavy_read', 'submit', 'write', 'faucet']) expect([10, 60]).toContain(l[c].period);
  });
});

describe('generated artefacts are up to date', () => {
  for (const [file, expected] of Object.entries(lib.expectedArtifacts(policy) as Record<string, string>)) {
    it(`${file.split(/[\\/]/).slice(-3).join('/')} matches policy.json`, () => {
      expect(norm(fs.readFileSync(file, 'utf8'))).toBe(expected);
    });
  }

  it('ruleset only touches our own refs and scopes block rules to the public host', () => {
    const phases = lib.buildRulesets(policy) as Record<string, Array<{ ref: string; expression: string; action: string }>>;
    expect(Object.keys(phases).sort()).toEqual(['http_ratelimit', 'http_request_firewall_custom']);
    for (const rules of Object.values(phases)) for (const r of rules) expect(r.ref.startsWith(lib.RULE_REF_PREFIX)).toBe(true);
    for (const r of phases.http_request_firewall_custom) expect(r.expression).toContain(`http.host eq "${policy.host}"`);
  });
});

describe('zone rate limits are plan-aware (Cloudflare plan limits)', () => {
  type Rule = { ref: string; expression: string; action: string; ratelimit: { period: number; mitigation_timeout: number; requests_per_period: number; characteristics: string[] } };
  const rl = (plan: string) => (lib.buildRulesets(policy, plan) as { http_ratelimit: Rule[] }).http_ratelimit;

  it('defaults to the free plan and every profile fits its own rule budget', () => {
    expect(policy.zone_rules.plan).toBe('free');
    for (const plan of lib.PLANS as string[]) {
      expect(rl(plan).length).toBeLessThanOrEqual(policy.zone_rules.plans[plan].max_rate_limit_rules);
      expect(() => lib.assertFitsPlan(policy, plan)).not.toThrow();
    }
  });

  it('free: exactly one rule, 10 s period and mitigation, block action, path-only expression (no host or method)', () => {
    const rules = rl('free');
    expect(rules).toHaveLength(1);
    const [r] = rules;
    expect(r.ref).toBe('boing_public_rpc_rl_guard');
    expect(r.action).toBe('block');
    expect(r.ratelimit.period).toBe(10);
    expect(r.ratelimit.mitigation_timeout).toBe(10);
    expect(r.ratelimit.requests_per_period).toBe(100);
    expect(r.ratelimit.characteristics).toContain('ip.src');
    expect(r.expression).not.toMatch(/http\.host|http\.request\.method/);
    expect(r.expression).toContain('http.request.uri.path');
  });

  it('pro and above keep the burst plus sustained pair; pro cannot use the method field', () => {
    for (const plan of ['pro', 'business', 'enterprise']) {
      expect(rl(plan).map((r) => r.ref)).toEqual(['boing_public_rpc_rl_burst', 'boing_public_rpc_rl_sustained']);
      expect(rl(plan)[0].expression).toContain(`http.host eq "${policy.host}"`);
    }
    expect(rl('pro')[0].expression).not.toContain('http.request.method');
    expect(rl('business')[0].expression).toContain('http.request.method eq "POST"');
  });

  it('assertFitsPlan rejects exceeding the rule budget before any API call, counting existing rules', () => {
    expect(() => lib.assertFitsPlan(policy, 'free', 1)).toThrow(/allows 1 rate-limit rule/);
    expect(() => lib.assertFitsPlan(policy, 'pro', 1)).toThrow(/allows 2/);
    expect(lib.assertFitsPlan(policy, 'business', 3)).toMatchObject({ total: 5, max: 5 });
    expect(() => lib.planProfile(policy, 'platinum')).toThrow(/unknown plan/);
  });

  it('validatePolicy rejects a free profile that cannot work on Free', () => {
    const bad = structuredClone(policy);
    bad.zone_rules.plans.free.rate_limit_rules.extra = { requests_per_period: 5, period: 10, mitigation_timeout: 10 };
    expect(lib.validatePolicy(bad, catalog).join('\n')).toContain('allows 1');
    const bad2 = structuredClone(policy);
    bad2.zone_rules.plans.free.match = 'host_method';
    expect(lib.validatePolicy(bad2, catalog).join('\n')).toContain('cannot use host or method');
    const bad3 = structuredClone(policy);
    bad3.zone_rules.plans.free.rate_limit_rules.guard.period = 60;
    expect(lib.validatePolicy(bad3, catalog).join('\n')).toContain('Free allows only period 10');
  });
});

describe('gateway edge policy', () => {
  it('parses modes, defaulting to log', () => {
    expect(parseMode(undefined)).toBe('log');
    expect(parseMode('ENFORCE')).toBe('enforce');
    expect(parseMode('off')).toBe('off');
    expect(parseMode('nonsense')).toBe('log');
  });

  it('allows reads and counts them per class', () => {
    const body = JSON.stringify([
      { jsonrpc: '2.0', id: 1, method: 'boing_getBalance', params: [] },
      { jsonrpc: '2.0', id: 2, method: 'boing_getLogs', params: [] },
      { jsonrpc: '2.0', id: 3, method: 'boing_submitTransaction', params: [] },
    ]);
    const plan = planRequest(inspectBody(body)!);
    expect(plan.violations).toEqual([]);
    expect(plan.counts).toEqual({ read: 1, heavy_read: 1, submit: 1 });
  });

  it('rejects operator and unknown methods with -32601, keeping the request id for single calls', () => {
    const plan = planRequest(inspectBody('{"jsonrpc":"2.0","id":7,"method":"boing_operatorApplyFeePolicy"}')!);
    expect(plan.violations[0]).toMatchObject({ reason: 'method_not_public', id: 7 });
    const d = denial(plan.violations[0], false);
    expect(d.status).toBe(403);
    expect((d.body as { id: number; error: { code: number } }).id).toBe(7);
    expect((d.body as { error: { code: number } }).error.code).toBe(-32601);
    expect(planRequest(inspectBody('{"id":1,"method":"eth_sendRawTransaction"}')!).violations[0].reason).toBe('unknown_method');
    expect(classify('boing_getBalance')).toBe('read');
    expect(classify('toString')).toBe('unknown');
  });

  it('rejects oversized batches and ignores non-JSON-RPC bodies', () => {
    const calls = Array.from({ length: MAX_BATCH_SIZE + 1 }, (_, i) => ({ id: i, method: 'boing_health' }));
    const plan = planRequest(inspectBody(JSON.stringify(calls))!);
    expect(plan.violations[0].reason).toBe('batch_too_large');
    expect(denial(plan.violations[0], true).status).toBe(400);
    expect(inspectBody('not json')).toBeNull();
    expect(inspectBody('42')).toBeNull();
  });

  it('charges each call in a batch to its class and reports the first class over its limit', async () => {
    const seen: string[] = [];
    const bucket = (allow: number): RateLimitBinding => {
      let n = 0;
      return {
        async limit({ key }) {
          seen.push(key);
          n += 1;
          return { success: n <= allow };
        },
      };
    };
    const env = { [CLASS_LIMITS.read.binding]: bucket(2), [CLASS_LIMITS.submit.binding]: bucket(10) };
    expect(await checkRates(env, '203.0.113.9', { read: 2, submit: 1 })).toBeNull();
    expect(seen).toEqual(['read:203.0.113.9', 'read:203.0.113.9', 'submit:203.0.113.9']);
    expect(await checkRates(env, '203.0.113.9', { read: 1 })).toBe('read');
    expect(await checkRates({}, 'x', { read: 5, faucet: 1 })).toBeNull(); // bindings absent -> no limit
  });

  it('rate-limit denials use -32016 with Retry-After', () => {
    const d = denial({ reason: 'rate_limited', class: 'submit', id: 3 }, false);
    expect(d.status).toBe(429);
    expect(d.retryAfter).toBe(CLASS_LIMITS.submit.period);
    expect((d.body as { error: { code: number } }).error.code).toBe(-32016);
  });
});