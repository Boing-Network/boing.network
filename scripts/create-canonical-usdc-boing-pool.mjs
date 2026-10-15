#!/usr/bin/env node
/**
 * Create the canonical public-testnet USDC / BOING-token pool: a v9 native CP pool that actually
 * holds reference-token v2 balances (pulled with `transfer_from`), seed it 1:1, and list it in the
 * canonical DEX factory so `boing_listDexPools` returns it.
 *
 * Steps (each skipped when already done, so the script is safe to re-run):
 *   1. deploy pool v9 + set_tokens(USDC, BOING)       (skip: BOING_POOL_HEX given, or the pair is already listed)
 *   2. approve both tokens + add_liquidity(amount, amount)  (skip: reserves already > 0)
 *   3. factory register_pair(USDC, BOING, pool)        (skip: pool already in boing_listDexPools)
 *
 *   node scripts/create-canonical-usdc-boing-pool.mjs                      # dry run: plan + checks, nothing signed/sent
 *   BOING_SECRET_HEX=0x... node scripts/create-canonical-usdc-boing-pool.mjs --send
 *   BOING_POOL_HEX=0x... BOING_SECRET_HEX=0x... node scripts/...mjs --send   # resume with an already-deployed pool
 *
 * Env: BOING_RPC_URL (default https://testnet-rpc.boing.network/), BOING_USDC_TOKEN_HEX / BOING_BOING_TOKEN_HEX
 * (default: the live testnet tokens below), BOING_POOL_SEED_AMOUNT (default 100000 whole units of each token).
 * The signer must hold both tokens. Never commit or print the secret.
 */
import {
  addBoingPoolLiquidity,
  buildNativeDexFactoryAccessList,
  bytesToHex,
  createBoingPool,
  createClient,
  encodeNativeDexRegisterPairCalldata,
  getBoingPoolState,
  getBoingTokenBalance,
  hexToBytes,
  quoteBoingPoolSwap,
  senderHexFromSecretKey,
  submitContractCallWithSimulationRetry,
  validateHex32,
} from '../boing-sdk/dist/index.js';

const VERSION = 9;
const DEFAULT_USDC = '0x6676f971ade3982406efa3e2cdb0477923f8f2a15624c4b6ad3a432817a75e93'; // "Testnet USD Coin" (USDC)
const DEFAULT_BOING = '0x6fce9d3e3f13df9fc17e3fd947148a4e6ce6966538b7dd9bf2a891d3b9699c6b'; // "Boing Token" (BOING)

const send = process.argv.includes('--send');
const rpc = process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network/';
const client = createClient(rpc);
const usdc = validateHex32(process.env.BOING_USDC_TOKEN_HEX?.trim() || DEFAULT_USDC);
const boing = validateHex32(process.env.BOING_BOING_TOKEN_HEX?.trim() || DEFAULT_BOING);
const amount = BigInt(process.env.BOING_POOL_SEED_AMOUNT ?? '100000');
const lc = (h) => (h ?? '').toLowerCase();
const json = (o) => JSON.stringify(o, (_, v) => (typeof v === 'bigint' ? v.toString() : v), 2);

const secretHex = process.env.BOING_SECRET_HEX?.trim();
if (send && !secretHex) {
  console.error('--send needs BOING_SECRET_HEX (funded testnet key holding both tokens; never commit it).');
  process.exit(1);
}
const secret = secretHex ? hexToBytes(secretHex) : null;
const sender = secret ? await senderHexFromSecretKey(secret) : null;

const info = await client.getNetworkInfo();
const factory = info.end_user?.canonical_native_dex_factory;
if (!factory) throw new Error('node does not advertise end_user.canonical_native_dex_factory');

async function listedPools() {
  const r = await client.request('boing_listDexPools', [{ light: true }]);
  return r?.pools ?? [];
}
function pairMatch(p) {
  const a = lc(p.tokenAHex ?? p.token_a), b = lc(p.tokenBHex ?? p.token_b);
  return (a === lc(usdc) && b === lc(boing)) || (a === lc(boing) && b === lc(usdc));
}
async function waitTx(txId, label) {
  for (let i = 0; i < 60; i++) {
    const r = await client.getTransactionReceipt(txId).catch(() => null);
    if (r) {
      if (!r.success) throw new Error(`${label} ${txId} failed on-chain: ${r.error}`);
      return r;
    }
    await new Promise((s) => setTimeout(s, 2000));
  }
  throw new Error(`${label} ${txId} not included after 120 s`);
}
const log = [];
const step = (name, data) => { log.push({ step: name, ...data }); console.error(`[pool] ${name}`); };

// ---- locate an existing pool (idempotency) ----
let pools = await listedPools();
let pool = process.env.BOING_POOL_HEX?.trim() ? validateHex32(process.env.BOING_POOL_HEX.trim()) : null;
const listedPair = pools.find(pairMatch);
if (!pool && listedPair) pool = validateHex32(listedPair.poolHex);

const plan = { rpc, chain_id: info.chain_id, mode: send ? 'send' : 'dry-run', factory, version: VERSION, usdc, boing, seed_amount_each: amount, signer: sender, pool };
if (sender) {
  plan.signer_balances = {
    native: (await client.getAccount(sender)).balance,
    usdc: await getBoingTokenBalance(client, usdc, sender),
    boing: await getBoingTokenBalance(client, boing, sender),
  };
  if (plan.signer_balances.usdc < amount || plan.signer_balances.boing < amount) {
    console.log(json({ ...plan, ok: false, error: 'signer holds less than the seed amount of USDC or BOING' }));
    process.exit(1);
  }
}

// ---- 1. pool ----
if (!pool) {
  if (!send) {
    step('would deploy pool v9 + set_tokens', {});
  } else {
    const out = await createBoingPool(client, secret, { version: VERSION, tokenAHex: usdc, tokenBHex: boing });
    pool = out.poolHex;
    for (const t of out.txs) await waitTx(t.txId, 'create');
    step('deployed pool + set_tokens', { pool, tx_ids: out.txs.map((t) => t.txId) });
  }
} else {
  const st = await getBoingPoolState(client, pool);
  if (lc(st.tokenAHex) !== lc(usdc) || lc(st.tokenBHex) !== lc(boing)) {
    throw new Error(`pool ${pool} has tokens ${st.tokenAHex}/${st.tokenBHex}, not USDC/BOING`);
  }
  step('pool exists with USDC/BOING tokens', { pool });
}

// ---- 2. liquidity ----
if (pool) {
  const st = await getBoingPoolState(client, pool);
  if (st.reserveA > 0n || st.reserveB > 0n) {
    step('liquidity already present', { reserveA: st.reserveA, reserveB: st.reserveB });
  } else if (!send) {
    step('would approve + add_liquidity', { amountA: amount, amountB: amount });
  } else {
    const out = await addBoingPoolLiquidity(client, secret, { poolHex: pool, amountA: amount, amountB: amount, version: VERSION, tokenAHex: usdc, tokenBHex: boing });
    for (const t of out.approvals) await waitTx(t.txId, 'approve');
    await waitTx(out.txId, 'add_liquidity');
    step('added liquidity', { approve_tx_ids: out.approvals.map((t) => t.txId), add_tx_id: out.txId, expected_lp: out.expectedLp });
  }
} else {
  step('would approve + add_liquidity', { amountA: amount, amountB: amount });
}

// ---- 3. register ----
pools = await listedPools();
if (pool && pools.some((p) => lc(p.poolHex) === lc(pool))) {
  step('already registered in factory', {});
} else if (pool) {
  const calldata = encodeNativeDexRegisterPairCalldata(usdc, boing, pool);
  const sim = await client.simulateContractCall(factory, bytesToHex(calldata), { senderHex: sender });
  if (!send) {
    step('would register_pair', { simulation: { success: sim.success, gas_used: sim.gas_used, error: sim.error ?? null } });
  } else {
    const out = await submitContractCallWithSimulationRetry({ client, secretKey32: secret, senderHex: sender, contractHex: factory, calldata, accessList: buildNativeDexFactoryAccessList(sender, factory) });
    const txId = out.tx_hash ?? out.txId ?? out.tx_id;
    if (txId && /^0x[0-9a-f]{64}$/i.test(txId)) await waitTx(txId, 'register_pair');
    else await new Promise((s) => setTimeout(s, 8000));
    step('registered pair in factory', { tx: txId });
  }
} else {
  step('would register_pair (after deploy)', {});
}

// ---- verify ----
const verify = {};
if (pool) {
  const st = await getBoingPoolState(client, pool).catch((e) => ({ error: e.message }));
  verify.pool_state = st;
  verify.listed = (await listedPools()).some((p) => lc(p.poolHex) === lc(pool));
  if (st.reserveA > 0n) verify.quote_1_usdc_to_boing = await quoteBoingPoolSwap(client, { poolHex: pool, direction: 'a-to-b', amountIn: 1n, version: VERSION }).catch((e) => e.message);
}
console.log(json({ ...plan, pool, ok: true, steps: log, verify }));