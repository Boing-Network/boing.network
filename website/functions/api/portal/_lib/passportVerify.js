/**
 * Testnet Passport on-chain quest verification (stage 1).
 * Read-only JSON-RPC against the public testnet node; no writes, no keys.
 */
import { PASSPORT_RULES, QUESTS_BY_ID } from './quests.js';

export const DEFAULT_RPC_URL = 'https://testnet-rpc.boing.network/';
const lc = (s) => String(s || '').toLowerCase();

export async function rpcCall(rpcUrl, method, params, fetchImpl = fetch) {
  const res = await fetchImpl(rpcUrl, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  });
  if (!res.ok) throw new Error(`RPC HTTP ${res.status}`);
  const data = await res.json();
  if (data.error) throw new Error(`RPC ${method}: ${data.error.message || data.error.code}`);
  return data.result;
}

/** Collects up to max_history_pages × 100 committed txs touching the account (newest first). */
export async function fetchAccountTxs(rpcUrl, account, fetchImpl = fetch) {
  const maxPages = PASSPORT_RULES.verification.max_history_pages || 10;
  const out = [];
  let cursor = null;
  for (let i = 0; i < maxPages; i++) {
    const r = await rpcCall(rpcUrl, 'boing_getAccountTransactions', [account, { cursor, limit: 100 }], fetchImpl);
    out.push(...(r.transactions || []));
    cursor = r.next_cursor;
    if (!cursor) return { txs: out, complete: true };
  }
  return { txs: out, complete: false };
}

export async function fetchDexContracts(rpcUrl, fetchImpl = fetch) {
  const set = new Set(PASSPORT_RULES.verification.dex_contracts_hex.map(lc));
  let cursor;
  for (let i = 0; i < 20; i++) {
    const r = await rpcCall(rpcUrl, 'boing_listDexPools', [cursor ? { cursor, limit: 100 } : { limit: 100 }], fetchImpl);
    for (const p of r.pools || []) set.add(lc(p.poolHex));
    cursor = r.nextCursor;
    if (!cursor) break;
  }
  return set;
}

function calledContracts(tx) {
  const p = tx.payload || {};
  if (p.kind === 'contract_call' && p.contract) return [lc(p.contract)];
  if (p.kind === 'batch' && Array.isArray(p.steps)) return p.steps.filter((s) => s && s.contract).map((s) => lc(s.contract));
  return [];
}

/** Pure checks over a tx list (newest first). Returns { verified, evidence? }. */
export const CHECKS = {
  faucet(ctx) {
    const f = lc(PASSPORT_RULES.verification.faucet_account_hex);
    const tx = ctx.txs.find((t) => t.success && lc(t.sender) === f && t.payload?.kind === 'transfer' && lc(t.payload.to) === ctx.account);
    return tx ? { verified: true, evidence: tx.tx_id } : { verified: false };
  },
  first_tx(ctx) {
    const tx = ctx.txs.find((t) => t.success && lc(t.sender) === ctx.account);
    return tx ? { verified: true, evidence: tx.tx_id } : { verified: false };
  },
  first_deploy(ctx) {
    const tx = ctx.txs.find((t) => t.success && lc(t.sender) === ctx.account && String(t.payload?.kind || '').startsWith('contract_deploy'));
    return tx ? { verified: true, evidence: tx.tx_id } : { verified: false };
  },
  finance_swap(ctx) {
    const tx = ctx.txs.find((t) => t.success && lc(t.sender) === ctx.account && calledContracts(t).some((c) => ctx.dex.has(c)));
    return tx ? { verified: true, evidence: tx.tx_id } : { verified: false };
  },
  validator_connect(ctx) {
    const v = (ctx.validators || []).find((x) => lc(x.id) === ctx.account || lc(x.owner) === ctx.account);
    if (v) return { verified: true, evidence: `validator ${v.id} (${v.status})` };
    const tx = ctx.txs.find((t) => t.success && lc(t.sender) === ctx.account && t.payload?.kind === 'validator_register');
    return tx ? { verified: true, evidence: tx.tx_id } : { verified: false };
  },
};

export function isAutoQuest(questId) {
  const q = QUESTS_BY_ID[questId];
  return !!q && q.verificationType === 'on_chain' && typeof CHECKS[questId] === 'function';
}

/**
 * Verify one or all auto quests for an account. Read-only.
 * @returns {Promise<{ account, head_height?, history_complete, results: Record<string,{verified:boolean,evidence?:string}> }>}
 */
export async function verifyAccount(account, questIds, { rpcUrl = DEFAULT_RPC_URL, fetchImpl = fetch } = {}) {
  const acct = lc(account);
  const ids = (questIds || Object.keys(CHECKS)).filter(isAutoQuest);
  const { txs, complete } = await fetchAccountTxs(rpcUrl, acct, fetchImpl);
  const ctx = { account: acct, txs, dex: new Set(), validators: [] };
  if (ids.includes('finance_swap')) ctx.dex = await fetchDexContracts(rpcUrl, fetchImpl);
  if (ids.includes('validator_connect')) {
    const v = await rpcCall(rpcUrl, 'boing_getValidators', [], fetchImpl);
    ctx.validators = v.validators || [];
  }
  const results = {};
  for (const id of ids) results[id] = CHECKS[id](ctx);
  return { account: acct, history_complete: complete, results };
}
