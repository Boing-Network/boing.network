/**
 * Public RPC edge policy: load ops/cloudflare/public-rpc/policy.json, cross-check it against the
 * node's method catalog, and render the Cloudflare ruleset, the Worker method table and the Worker
 * rate-limit bindings. Pure (no network); used by scripts/public-rpc-edge.mjs and the drift test.
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const REPO_ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
export const POLICY_PATH = path.join(REPO_ROOT, 'ops/cloudflare/public-rpc/policy.json');
export const RULESET_PATH = path.join(REPO_ROOT, 'ops/cloudflare/public-rpc/rulesets.generated.json');
export const WORKER_TABLE_PATH = path.join(REPO_ROOT, 'workers/public-rpc-gateway/src/methodPolicy.generated.ts');
export const WRANGLER_PATH = path.join(REPO_ROOT, 'workers/public-rpc-gateway/wrangler.toml');
export const RPC_RS_PATH = path.join(REPO_ROOT, 'crates/boing-node/src/rpc.rs');
export const DEVELOPER_API_PATH = path.join(REPO_ROOT, 'crates/boing-node/schemas/developer_api.json');

export const WRANGLER_BEGIN = '# BEGIN GENERATED public-rpc-policy (scripts/public-rpc-edge.mjs generate; do not edit)';
export const WRANGLER_END = '# END GENERATED public-rpc-policy';
export const RULE_REF_PREFIX = 'boing_public_rpc_';
export const PLANS = ['free', 'pro', 'business', 'enterprise'];
export const MATCH_MODES = ['path', 'host_path', 'host_method'];
export const ALLOWED_CLASSES = ['read', 'heavy_read', 'submit', 'write', 'faucet'];
const ALL_CLASSES = [...ALLOWED_CLASSES, 'blocked'];

export const loadPolicy = (file = POLICY_PATH) => JSON.parse(fs.readFileSync(file, 'utf8'));

/** Every method the node can serve: the Rust dispatcher list unioned with the published catalog. */
export function catalogMethods() {
  const rs = fs.readFileSync(RPC_RS_PATH, 'utf8');
  const start = rs.indexOf('BOING_RPC_SUPPORTED_METHODS');
  if (start < 0) throw new Error('BOING_RPC_SUPPORTED_METHODS not found in rpc.rs');
  const end = rs.indexOf('];', start);
  const fromRust = [...rs.slice(start, end).matchAll(/"(boing_[A-Za-z0-9_]+)"/g)].map((m) => m[1]);
  const api = JSON.parse(fs.readFileSync(DEVELOPER_API_PATH, 'utf8'));
  const fromCatalog = api.method_catalog.methods.map((m) => m.name);
  return { fromRust, fromCatalog, all: [...new Set([...fromRust, ...fromCatalog])].sort() };
}

/**
 * @returns {string[]} human-readable problems; empty when the policy covers the catalog exactly.
 */
export function validatePolicy(policy, methods = catalogMethods().all) {
  const problems = [];
  const seen = new Map();
  for (const cls of Object.keys(policy.classes)) {
    if (!ALL_CLASSES.includes(cls)) problems.push(`unknown class "${cls}"`);
    for (const m of policy.classes[cls].methods) {
      if (seen.has(m)) problems.push(`${m} is in both "${seen.get(m)}" and "${cls}"`);
      seen.set(m, cls);
    }
  }
  for (const m of methods) {
    if (!seen.has(m)) problems.push(`${m} is in the node catalog but in neither an allow class nor "blocked" (classify it in policy.json)`);
  }
  for (const m of seen.keys()) {
    if (!methods.includes(m)) problems.push(`${m} is in policy.json but not served by the node (stale entry)`);
  }
  for (const cls of ALLOWED_CLASSES) {
    const l = policy.worker.limits[cls];
    if (!l) problems.push(`worker.limits.${cls} missing`);
    else if (![10, 60].includes(l.period)) problems.push(`worker.limits.${cls}.period must be 10 or 60 (Workers Rate Limiting binding)`);
  }
  const zr = policy.zone_rules ?? {};
  if (!PLANS.includes(zr.plan)) problems.push(`zone_rules.plan must be one of ${PLANS.join(', ')}`);
  for (const plan of PLANS) {
    const prof = zr.plans?.[plan];
    if (!prof) {
      problems.push(`zone_rules.plans.${plan} missing`);
      continue;
    }
    if (!MATCH_MODES.includes(prof.match)) problems.push(`zone_rules.plans.${plan}.match must be one of ${MATCH_MODES.join(', ')}`);
    if (plan === 'free' && prof.match !== 'path') problems.push('zone_rules.plans.free.match must be "path": Free rate-limit expressions cannot use host or method');
    if (plan === 'pro' && prof.match === 'host_method') problems.push('zone_rules.plans.pro.match cannot use method (Pro has Host and Path only)');
    const rules = Object.entries(prof.rate_limit_rules ?? {});
    if (rules.length === 0) problems.push(`zone_rules.plans.${plan} has no rate_limit_rules`);
    if (rules.length > prof.max_rate_limit_rules) {
      problems.push(`zone_rules.plans.${plan} defines ${rules.length} rate-limit rules but the plan allows ${prof.max_rate_limit_rules}`);
    }
    for (const [name, r] of rules) {
      if (!(r.requests_per_period > 0 && r.period > 0 && r.mitigation_timeout > 0)) problems.push(`zone_rules.plans.${plan}.${name} needs positive requests_per_period, period, mitigation_timeout`);
      if (plan === 'free' && (r.period !== 10 || r.mitigation_timeout !== 10)) problems.push(`zone_rules.plans.free.${name}: Free allows only period 10 and mitigation_timeout 10`);
    }
  }
  const submit = policy.worker.limits.submit?.limit;
  const read = policy.worker.limits.read?.limit;
  if (submit !== undefined && read !== undefined && !(submit < read)) problems.push('submit limit must be tighter than the read limit');
  if (policy.classes.blocked.methods.includes('boing_submitTransaction')) problems.push('boing_submitTransaction must not be blocked');
  return problems;
}

const sorted = (o) => Object.fromEntries(Object.entries(o).sort(([a], [b]) => (a < b ? -1 : 1)));

export function methodTable(policy) {
  const table = {};
  for (const cls of ALL_CLASSES) for (const m of policy.classes[cls].methods) table[m] = cls;
  return sorted(table);
}

const RL_BODY = JSON.stringify({
  jsonrpc: '2.0',
  id: null,
  error: { code: -32016, message: 'Rate limit exceeded at the edge. Retry shortly.' },
});

/** The plan profile selected by name (defaults to policy.zone_rules.plan). Throws on an unknown plan. */
export function planProfile(policy, plan = policy.zone_rules.plan) {
  const prof = policy.zone_rules.plans?.[plan];
  if (!prof) throw new Error(`unknown plan "${plan}" (expected one of ${PLANS.join(', ')})`);
  return prof;
}

/**
 * Cloudflare Rulesets API payloads, one object per phase, for one plan profile.
 * The rate-limit rule count always fits the plan (validatePolicy + assertFitsPlan).
 */
export function buildRulesets(policy, plan = policy.zone_rules.plan) {
  const prof = planProfile(policy, plan);
  const host = `http.host eq "${policy.host}"`;
  const exempt = policy.zone_rules.exempt_expression?.trim();
  const base = (extra) => `(${host} and ${extra})${exempt ? ` and not (${exempt})` : ''}`;
  const action = prof.action ?? policy.rollout.zone_rule_action;
  // Rate-limit expressions can only use the fields the plan offers (Free: path; Pro: host + path; Business+: host + method).
  const rlExpression = () => {
    const tail = exempt ? ` and not (${exempt})` : '';
    if (prof.match === 'path') return `(http.request.uri.path eq "/")${tail}`;
    if (prof.match === 'host_path') return `(${host} and http.request.uri.path eq "/")${tail}`;
    return base('http.request.method eq "POST"');
  };
  const scope = { path: 'requests to /', host_path: 'requests to /', host_method: 'POST' }[prof.match];
  const rl = (suffix, desc, cfg) => ({
    ref: `${RULE_REF_PREFIX}${suffix}`,
    description: desc,
    expression: rlExpression(),
    action,
    ...(action === 'block'
      ? { action_parameters: { response: { status_code: 429, content_type: 'application/json', content: RL_BODY } } }
      : {}),
    ratelimit: {
      characteristics: ['cf.colo.id', 'ip.src'],
      period: cfg.period,
      requests_per_period: cfg.requests_per_period,
      mitigation_timeout: cfg.mitigation_timeout,
    },
    enabled: true,
  });
  return {
    http_ratelimit: Object.entries(prof.rate_limit_rules).map(([name, cfg]) =>
      rl(`rl_${name}`, `Public RPC: per-IP ${name} limit (${cfg.requests_per_period} ${scope} per ${cfg.period}s, ${plan} plan)`, cfg),
    ),
    http_request_firewall_custom: [
      {
        ref: `${RULE_REF_PREFIX}block_http_methods`,
        description: 'Public RPC: only POST, GET, HEAD and OPTIONS',
        expression: `(${host} and not http.request.method in {"POST" "GET" "HEAD" "OPTIONS"})`,
        action: 'block',
        enabled: true,
      },
      {
        ref: `${RULE_REF_PREFIX}block_operator_header`,
        description: 'Public RPC: operator RPC must use the origin, never the public hostname',
        expression: `(${host} and len(http.request.headers["x-boing-operator"]) gt 0)`,
        action: 'block',
        enabled: true,
      },
    ],
  };
}

/**
 * Throws when the rate-limit rules that would exist after the apply (ours plus any other rules already in the
 * phase) exceed the plan's rule budget. Called before any API write.
 */
export function assertFitsPlan(policy, plan, otherRateLimitRules = 0) {
  const prof = planProfile(policy, plan);
  const ours = buildRulesets(policy, plan).http_ratelimit.length;
  const total = ours + otherRateLimitRules;
  if (total > prof.max_rate_limit_rules) {
    throw new Error(
      `plan "${plan}" allows ${prof.max_rate_limit_rules} rate-limit rule(s) but this would create ${total} (${ours} ours + ${otherRateLimitRules} existing). Nothing was changed.`,
    );
  }
  return { ours, total, max: prof.max_rate_limit_rules };
}

export function renderRulesetFile(policy) {
  return `${JSON.stringify(
    {
      $comment: 'GENERATED by scripts/public-rpc-edge.mjs from ops/cloudflare/public-rpc/policy.json. Do not edit. Apply with: node scripts/public-rpc-edge.mjs apply (dry-run unless --apply).',
      host: policy.host,
      zone_name: policy.zone_name,
      plan: policy.zone_rules.plan,
      phases: buildRulesets(policy),
    },
    null,
    2,
  )}\n`;
}

export function renderWorkerTable(policy) {
  const table = methodTable(policy);
  const lines = Object.entries(table).map(([m, c]) => `  ${JSON.stringify(m)}: ${JSON.stringify(c)},`);
  const limits = ALLOWED_CLASSES.map((c) => {
    const l = policy.worker.limits[c];
    return `  ${c}: { limit: ${l.limit}, period: ${l.period}, binding: ${JSON.stringify(l.binding)} },`;
  });
  return `// GENERATED by scripts/public-rpc-edge.mjs from ops/cloudflare/public-rpc/policy.json. Do not edit.
// Regenerate: node scripts/public-rpc-edge.mjs generate   (verified by boing-sdk/tests/publicRpcPolicy.test.ts)

export type MethodClass = "read" | "heavy_read" | "submit" | "write" | "faucet" | "blocked";
export type RateClass = Exclude<MethodClass, "blocked">;

export const METHOD_CLASS: Readonly<Record<string, MethodClass>> = {
${lines.join('\n')}
};

export const CLASS_LIMITS: Readonly<Record<RateClass, { limit: number; period: number; binding: string }>> = {
${limits.join('\n')}
};

export const MAX_BATCH_SIZE = ${policy.worker.max_batch_size};
`;
}

export function renderWranglerBlock(policy) {
  const blocks = ALLOWED_CLASSES.map((c) => {
    const l = policy.worker.limits[c];
    return `[[ratelimits]]\nname = "${l.binding}"\nnamespace_id = "${l.namespace_id}"\nsimple = { limit = ${l.limit}, period = ${l.period} }\n`;
  });
  return `${WRANGLER_BEGIN}\n${blocks.join('\n')}${WRANGLER_END}`;
}

/** Replace (or append) the generated block in wrangler.toml text. */
export function applyWranglerBlock(toml, block) {
  const a = toml.indexOf(WRANGLER_BEGIN);
  const b = toml.indexOf(WRANGLER_END);
  if (a >= 0 && b > a) return toml.slice(0, a) + block + toml.slice(b + WRANGLER_END.length);
  return `${toml.replace(/\s*$/, '')}\n\n${block}\n`;
}

/** All generated artefacts as { path: expected content }. */
export function expectedArtifacts(policy = loadPolicy()) {
  const wrangler = fs.readFileSync(WRANGLER_PATH, 'utf8').replace(/\r\n/g, '\n');
  return {
    [RULESET_PATH]: renderRulesetFile(policy),
    [WORKER_TABLE_PATH]: renderWorkerTable(policy),
    [WRANGLER_PATH]: applyWranglerBlock(wrangler, renderWranglerBlock(policy)),
  };
}