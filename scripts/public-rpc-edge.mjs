#!/usr/bin/env node
/**
 * Public RPC edge config-as-code (docs/PUBLIC-RPC-WAF-GUIDANCE.md).
 *
 *   node scripts/public-rpc-edge.mjs generate          rewrite the generated artefacts from policy.json (default plan profile)
 *   node scripts/public-rpc-edge.mjs check             fail if policy.json is out of step with the node catalog
 *                                                      or an artefact is stale (also run by the vitest drift test)
 *   node scripts/public-rpc-edge.mjs apply             DRY RUN (default): show what would change in Cloudflare
 *   node scripts/public-rpc-edge.mjs apply --apply     really PUT the rules (needs CLOUDFLARE_API_TOKEN and a zone id)
 *   node scripts/public-rpc-edge.mjs apply --remove --apply   delete only the rules this tool owns (rollback)
 *
 * Options: --plan free|pro|business|enterprise (default: zone_rules.plan in policy.json, i.e. free)
 *          --zone-id <id> (or CLOUDFLARE_ZONE_ID), --api https://api.cloudflare.com/client/v4
 *
 * The plan decides how many zone rate-limit rules are sent (Free = 1). apply refuses, before any API call that
 * writes, when the rate-limit rule count would exceed the plan's budget.
 *
 * The apply step only ever touches rules whose `ref` starts with "boing_public_rpc_"; every other rule in
 * the zone's phase entrypoint is sent back unchanged. Without CLOUDFLARE_API_TOKEN the dry run is fully
 * offline. The token needs Zone > Zone WAF > Edit and Zone > Rate Limiting... (Rulesets) on the zone.
 */
import fs from 'node:fs';
import {
  RULE_REF_PREFIX,
  expectedArtifacts,
  loadPolicy,
  buildRulesets,
  assertFitsPlan,
  PLANS,
  validatePolicy,
  catalogMethods,
} from './lib/public-rpc-policy.mjs';

const [cmd = 'check', ...rest] = process.argv.slice(2);
const flag = (n) => rest.includes(`--${n}`);
const opt = (n, d) => {
  const i = rest.indexOf(`--${n}`);
  return i >= 0 ? rest[i + 1] : d;
};
const norm = (s) => s.replace(/\r\n/g, '\n');
const fail = (msg) => {
  console.error(msg);
  process.exit(1);
};

const policy = loadPolicy();
const problems = validatePolicy(policy);
if (problems.length) fail(`policy.json does not match the node catalog:\n - ${problems.join('\n - ')}`);

if (cmd === 'generate') {
  for (const [file, content] of Object.entries(expectedArtifacts(policy))) {
    fs.writeFileSync(file, content);
    console.log(`wrote ${file}`);
  }
  const { all } = catalogMethods();
  console.log(`${all.length} methods classified.`);
} else if (cmd === 'check') {
  const stale = Object.entries(expectedArtifacts(policy)).filter(([f, c]) => norm(fs.readFileSync(f, 'utf8')) !== c);
  if (stale.length) fail(`stale generated files (run: node scripts/public-rpc-edge.mjs generate):\n - ${stale.map(([f]) => f).join('\n - ')}`);
  console.log('public RPC edge policy: ok (catalog covered, artefacts up to date)');
} else if (cmd === 'apply') {
  await apply();
} else {
  fail(`unknown command "${cmd}" (generate | check | apply)`);
}

async function apply() {
  const doApply = flag('apply');
  const remove = flag('remove');
  const token = process.env.CLOUDFLARE_API_TOKEN;
  const zoneId = opt('zone-id', process.env.CLOUDFLARE_ZONE_ID);
  const api = (opt('api', 'https://api.cloudflare.com/client/v4') ?? '').replace(/\/+$/, '');
  const plan = opt('plan', policy.zone_rules.plan);
  if (!PLANS.includes(plan)) fail(`unknown --plan "${plan}" (expected one of ${PLANS.join(', ')})`);
  const desired = remove ? Object.fromEntries(Object.keys(buildRulesets(policy, plan)).map((p) => [p, []])) : buildRulesets(policy, plan);
  if (!remove) {
    try {
      assertFitsPlan(policy, plan, 0);
    } catch (e) {
      fail(e.message);
    }
  }

  if (doApply && (!token || !zoneId)) {
    fail('Refusing to apply: set CLOUDFLARE_API_TOKEN and CLOUDFLARE_ZONE_ID (or --zone-id). Nothing was changed.');
  }
  console.log(doApply ? (remove ? 'APPLY (remove owned rules)' : 'APPLY') : 'DRY RUN (nothing will be changed; pass --apply to apply)');
  console.log(`host ${policy.host}; plan profile: ${plan}; rate-limit rules: ${desired.http_ratelimit.length}\n`);

  for (const [phase, ours] of Object.entries(desired)) {
    let current = null;
    if (token && zoneId) {
      current = await cf('GET', `/zones/${zoneId}/rulesets/phases/${phase}/entrypoint`, undefined, true);
    }
    const existing = current?.rules ?? [];
    const keep = existing.filter((r) => !String(r.ref ?? '').startsWith(RULE_REF_PREFIX));
    const oldOurs = existing.filter((r) => String(r.ref ?? '').startsWith(RULE_REF_PREFIX));
    const merged = [...keep, ...ours];
    if (phase === 'http_ratelimit' && !remove) {
      try {
        assertFitsPlan(policy, plan, keep.length);
      } catch (e) {
        fail(e.message);
      }
    }
    console.log(`phase ${phase}`);
    console.log(`  other rules kept unchanged: ${current ? keep.length : '(unknown offline)'}`);
    for (const r of ours) {
      const was = oldOurs.find((o) => o.ref === r.ref);
      console.log(`  ${current ? (was ? 'update' : 'create') : 'would create/update'}  ${r.ref}  [${r.action}]  ${r.description}`);
    }
    for (const o of oldOurs.filter((o) => !ours.some((r) => r.ref === o.ref))) console.log(`  delete  ${o.ref}`);
    if (doApply) {
      const rules = merged.map(stripReadOnly);
      await cf('PUT', `/zones/${zoneId}/rulesets/phases/${phase}/entrypoint`, { rules });
      console.log('  applied.');
    }
  }
  if (!doApply) console.log('\nNothing applied. Review the generated expressions in ops/cloudflare/public-rpc/rulesets.generated.json first.');

  async function cf(method, p, body, allow404 = false) {
    const res = await fetch(`${api}${p}`, {
      method,
      headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
      body: body ? JSON.stringify(body) : undefined,
    });
    const j = await res.json().catch(() => ({}));
    if (res.status === 404 && allow404) return null;
    if (!res.ok || j.success === false) fail(`Cloudflare ${method} ${p} failed: HTTP ${res.status} ${JSON.stringify(j.errors ?? j).slice(0, 400)}`);
    return j.result;
  }
}

/** Drop only server-maintained fields. Other rules keep everything else (id, ref, logging, action_parameters) so they are re-sent unchanged. */
function stripReadOnly(rule) {
  const { version, last_updated, categories, ...r } = rule;
  void version; void last_updated; void categories;
  return r;
}