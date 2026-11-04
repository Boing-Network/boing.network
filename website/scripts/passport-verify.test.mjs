import test from 'node:test';
import assert from 'node:assert/strict';
import { verifyAccount, isAutoQuest } from '../functions/api/portal/_lib/passportVerify.js';
import { ALLOWED_QUEST_IDS } from '../functions/api/portal/_lib/quests.js';

const ME = '0x' + 'ab'.repeat(32);
const FAUCET = '0x1e1a11f9eb8612aee66bdf0286dbb29ca662cadf7d76e384ac19449d62433615';
const POOL = '0x' + 'cd'.repeat(32);

function mockFetch(txs, { validators = [], pools = [POOL] } = {}) {
  return async (_url, init) => {
    const { method } = JSON.parse(init.body);
    const result =
      method === 'boing_getAccountTransactions' ? { transactions: txs, next_cursor: null }
      : method === 'boing_listDexPools' ? { pools: pools.map((poolHex) => ({ poolHex })), nextCursor: null }
      : method === 'boing_getValidators' ? { validators }
      : null;
    return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result }));
  };
}

test('all nine quests are allowed; five are auto', () => {
  assert.equal(ALLOWED_QUEST_IDS.length, 9);
  assert.deepEqual(ALLOWED_QUEST_IDS.filter(isAutoQuest).sort(), ['faucet', 'finance_swap', 'first_deploy', 'first_tx', 'validator_connect']);
});

test('empty history verifies nothing', async () => {
  const r = await verifyAccount(ME, null, { fetchImpl: mockFetch([]) });
  assert.ok(Object.values(r.results).every((x) => !x.verified));
  assert.equal(r.history_complete, true);
});

test('activity verifies matching quests', async () => {
  const txs = [
    { tx_id: 't1', sender: FAUCET, success: true, payload: { kind: 'transfer', to: ME } },
    { tx_id: 't2', sender: ME, success: true, payload: { kind: 'contract_deploy' } },
    { tx_id: 't3', sender: ME, success: true, payload: { kind: 'batch', steps: [{ contract: POOL }] } },
    { tx_id: 't4', sender: ME, success: false, payload: { kind: 'validator_register' } },
  ];
  const r = await verifyAccount(ME, null, { fetchImpl: mockFetch(txs) });
  assert.equal(r.results.faucet.evidence, 't1');
  assert.equal(r.results.first_tx.evidence, 't2');
  assert.equal(r.results.first_deploy.evidence, 't2');
  assert.equal(r.results.finance_swap.evidence, 't3');
  assert.equal(r.results.validator_connect.verified, false, 'failed tx must not count');
});

test('validator set membership verifies validator_connect', async () => {
  const r = await verifyAccount(ME, ['validator_connect'], { fetchImpl: mockFetch([], { validators: [{ id: ME, status: 'active' }] }) });
  assert.equal(r.results.validator_connect.verified, true);
});
