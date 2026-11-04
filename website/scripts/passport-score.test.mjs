import test from 'node:test';
import assert from 'node:assert/strict';
import { computeSignals, computeBadges, computeTiers, merkleRoot, snapshotLeaf, isoWeek, nextTierProgress } from '../functions/api/portal/_lib/passportScore.js';
import { PASSPORT_RULES } from '../functions/api/portal/_lib/quests.js';

const A = '0x' + 'a'.repeat(64);
const DEX = '0x' + 'd'.repeat(64);
const FAUCET = PASSPORT_RULES.verification.faucet_account_hex;
const day = (n) => 1_790_000_000 + n * 86400;
let h = 0;
const tx = (o) => ({ success: true, block_height: ++h, tx_index: 0, tx_id: 't' + h, sender: A, payload: { kind: 'transfer' }, ...o });

test('rules 1.1.0 has badges and tiers for every track', () => {
  assert.equal(PASSPORT_RULES.rules_version, '1.1.0');
  for (const t of PASSPORT_RULES.tracks) assert.ok(PASSPORT_RULES.tiers[t.id].gold.requires_verified_social);
  const ids = new Set(PASSPORT_RULES.badges.map((b) => b.id));
  for (const t of PASSPORT_RULES.tracks) for (const k of ['bronze', 'silver', 'gold']) for (const r of PASSPORT_RULES.tiers[t.id][k].requires) assert.ok(ids.has(r), r);
});

test('signals, badges (historic), tiers', () => {
  const txs = [
    tx({ sender: FAUCET, payload: { kind: 'transfer', to: A }, timestamp: day(0) }),
    tx({ timestamp: day(0) }),
    tx({ payload: { kind: 'contract_call', contract: DEX }, timestamp: day(1) }),
    tx({ payload: { kind: 'contract_deploy' }, timestamp: day(2) }),
    tx({ success: false, payload: { kind: 'contract_deploy' }, timestamp: day(3) }),
  ].reverse();
  const s = computeSignals(A, txs, new Set([DEX]), []);
  assert.deepEqual([s.counts.tx_sent, s.counts.faucet_drips, s.counts.dex_calls, s.counts.deploys, s.counts.active_days], [3, 1, 1, 1, 3]);
  const reg = new Date(day(1) * 1000 + 1000).toISOString();
  const badges = computeBadges({ signals: s, registeredAt: reg, nowIso: new Date(day(5) * 1000).toISOString() });
  const by = Object.fromEntries(badges.map((b) => [b.id, b]));
  assert.ok(by.first_drip.historic && by.dex_trader.historic);
  assert.equal(by.first_deploy.historic, false);
  const tiers = computeTiers(badges, ['user', 'developer', 'node_operator']);
  assert.equal(tiers.user.tier, 'silver');
  assert.equal(tiers.developer.tier, 'bronze');
  assert.equal(tiers.node_operator.tier, 'none');
  assert.deepEqual(nextTierProgress('developer', badges, 'bronze').missing_required, ['multi_deploy']);
});

test('gold needs verified social; uptime badges', () => {
  const s = computeSignals(A, [tx({ payload: { kind: 'validator_register' }, timestamp: day(0) })], new Set(), []);
  const badges = computeBadges({ signals: s, uptime: { avg_pct: 99, samples: 2000 }, verifiedQuests: { feedback: '2026-11-01T00:00:00Z' }, nowIso: '2026-11-04T00:00:00Z' });
  assert.ok(badges.some((b) => b.id === 'validator_veteran'));
  assert.equal(computeTiers(badges, ['node_operator']).node_operator.tier, 'silver');
  assert.equal(computeTiers(badges, ['node_operator'], { verifiedSocial: true }).node_operator.tier, 'gold');
});

test('merkle root is order independent and week id', async () => {
  const e = [{ account_id_hex: A, track: 'user', tier: 'bronze', score: 3 }, { account_id_hex: DEX, track: 'user', tier: 'silver', score: 9 }, { account_id_hex: FAUCET, track: 'developer', tier: 'bronze', score: 2 }];
  const l1 = await Promise.all(e.map(snapshotLeaf));
  const r1 = await merkleRoot(l1);
  assert.match(r1, /^0x[0-9a-f]{64}$/);
  assert.equal(await merkleRoot([...l1].reverse()), r1);
  assert.equal(await merkleRoot([]), null);
  assert.equal(isoWeek(new Date('2026-11-04T12:00:00Z')), '2026-W45');
});
