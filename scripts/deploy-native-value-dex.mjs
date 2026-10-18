#!/usr/bin/env node
/**
 * Native BOING DEX on the public testnet (native-value rules, height 1150+): deploy wBOING, two v10
 * custody pools (wBOING/USDC and wBOING/tBOING), seed them with modest liquidity and list both in the
 * canonical DEX factory.
 *
 * Steps (each skipped when its env override is given or the state shows it is done; safe to re-run):
 *   1. deploy wBOING (purpose token, "Wrapped BOING" / "wBOING")          skip: BOING_WBOING_HEX
 *   2. wrap the BOING needed for seeding (deposit with value)             skip: wBOING balance already covers it
 *   3. per pair: deploy pool v10 + set_tokens(wBOING, token)              skip: BOING_V10_USDC_POOL_HEX / BOING_V10_TBOING_POOL_HEX
 *   4. per pair: approve both + add_liquidity                              skip: reserves > 0
 *   5. per pair: factory register_pair(wBOING, token, pool)                skip: pool already in boing_listDexPools
 *
 *   node scripts/deploy-native-value-dex.mjs                         # dry run: plan, balances, QA + simulation, nothing sent
 *   BOING_SECRET_HEX=0x... node scripts/deploy-native-value-dex.mjs --send
 *
 * Env: BOING_RPC_URL (default https://testnet-rpc.boing.network/), BOING_SEED_BOING (whole BOING per pool, default 10),
 * BOING_SEED_USDC / BOING_SEED_TBOING (whole token units, default 1000 each; USDC and tBOING have 0 decimals).
 * The signer needs native BOING plus both tokens. Never commit or print the secret.
 */
import {
  addBoingPoolLiquidity,
  buildNativeDexFactoryAccessList,
  buildWrapBoingTransaction,
  bytesToHex,
  createBoingPool,
  createClient,
  encodeNativeDexRegisterPairCalldata,
  getBoingPoolState,
  getBoingTokenBalance,
  hexToAccountId,
  hexToBytes,
  nativeValueActiveAt,
  predictNonceDerivedContractAddress,
  senderHexFromSecretKey,
  signTransactionInputForClient,
  submitContractCallWithSimulationRetry,
  transactionIdFromSignedTransactionHex,
  validateHex32,
  WRAPPED_BOING_BYTECODE_HEX,
} from '../boing-sdk/dist/index.js';

const USDC = '0x6676f971ade3982406efa3e2cdb0477923f8f2a15624c4b6ad3a432817a75e93'; // Testnet USD Coin (0 decimals)
const TBOING = '0x6fce9d3e3f13df9fc17e3fd947148a4e6ce6966538b7dd9bf2a891d3b9699c6b'; // Boing Token (tBOING, 0 decimals)
const ONE = 10n ** 18n;

const send = process.argv.includes('--send');
const rpc = process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network/';
const client = createClient(rpc);
const seedBoing = BigInt(process.env.BOING_SEED_BOING ?? '10') * ONE;
const pairs = [
  { name: 'BOING/USDC', token: USDC, seed: BigInt(process.env.BOING_SEED_USDC ?? '1000'), env: 'BOING_V10_USDC_POOL_HEX' },
  { name: 'BOING/tBOING', token: TBOING, seed: BigInt(process.env.BOING_SEED_TBOING ?? '1000'), env: 'BOING_V10_TBOING_POOL_HEX' },
];
const lc = (h) => (h ?? '').toLowerCase();
const out = (o) => console.log(JSON.stringify(o, (_, v) => (typeof v === 'bigint' ? v.toString() : v), 2));

const secretHex = process.env.BOING_SECRET_HEX?.trim();
if (send && !secretHex) {
  console.error('--send needs BOING_SECRET_HEX (funded testnet key; never commit or print it).');
  process.exit(1);
}
const secret = secretHex ? hexToBytes(secretHex) : null;
const sender = secret ? await senderHexFromSecretKey(secret) : validateHex32(process.env.BOING_SENDER_HEX ?? '0x' + '00'.repeat(32));

const info = await client.getNetworkInfo();
const tip = await client.chainHeight();
const factory = info.end_user?.canonical_native_dex_factory;
if (!factory) throw new Error('node does not advertise end_user.canonical_native_dex_factory');
if (!nativeValueActiveAt(info.chain_id, tip + 1)) throw new Error(`native value is not active at height ${tip + 1}`);

async function nextNonce() {
  const a = await client.getAccount(sender);
  return BigInt(a.nonce);
}
async function waitReceipt(txId, what) {
  for (let i = 0; i < 90; i++) {
    const r = await client.getTransactionReceipt(txId);
    if (r) {
      if (!r.success) throw new Error(`${what} failed: ${r.error ?? 'unknown'} (${txId})`);
      return r;
    }
    await new Promise((res) => setTimeout(res, 2000));
  }
  throw new Error(`${what}: no receipt for ${txId}`);
}
async function signSubmit(tx, what) {
  const signed = await signTransactionInputForClient(client, tx, secret);
  const txId = transactionIdFromSignedTransactionHex(signed);
  await client.submitTransaction(signed);
  const r = await waitReceipt(txId, what);
  log.push({ step: what, tx: txId, block: r.block_height });
  return txId;
}
async function waitAll(txs, what) {
  for (const t of txs) if (t.submitted) {
    const r = await waitReceipt(t.txId, what);
    log.push({ step: what, tx: t.txId, block: r.block_height });
  }
}
const log = [];

const balance = BigInt((await client.getBalance(sender)).balance);
const tokenBal = {};
for (const p of pairs) tokenBal[p.name] = await getBoingTokenBalance(client, p.token, sender);
const listed = (await client.request('boing_listDexPools', [{ light: true }]))?.pools ?? [];

// 1. wBOING
let wboing = process.env.BOING_WBOING_HEX ? validateHex32(process.env.BOING_WBOING_HEX) : null;
const plan = { rpc, chain_id: info.chain_id, tip, factory, sender, native_balance: balance, token_balances: tokenBal, seed_boing_per_pool: seedBoing, pairs };
if (!wboing) {
  const n = await nextNonce();
  const predicted = predictNonceDerivedContractAddress(sender, n);
  const qa = await client.qaCheck(WRAPPED_BOING_BYTECODE_HEX, 'token', undefined, 'Wrapped BOING', 'wBOING');
  const tx = {
    nonce: n,
    sender: hexToAccountId(sender),
    payload: {
      kind: 'contractDeployWithPurposeAndMetadata',
      bytecode: hexToBytes(WRAPPED_BOING_BYTECODE_HEX),
      purposeCategory: 'token',
      descriptionHash: null,
      assetName: 'Wrapped BOING',
      assetSymbol: 'wBOING',
      create2Salt: null,
    },
    accessList: { read: [hexToAccountId(sender)], write: [hexToAccountId(sender)] },
  };
  plan.wboing = { action: 'deploy', predicted, nonce: n, qa };
  if (!send) {
    // With a key (still no --send): sign and simulate the deploy at the next height (QA included).
    if (secret) {
      const sim = await client.simulateTransaction(await signTransactionInputForClient(client, tx, secret));
      plan.wboing.simulation = { success: sim.success, gas_used: sim.gas_used, error: sim.error ?? null, network_fee: sim.network_fee ?? null };
    }
    out({ ...plan, mode: 'dry-run', next: 'BOING_SECRET_HEX=0x... node scripts/deploy-native-value-dex.mjs --send' });
    process.exit(0);
  }
  await signSubmit(tx, 'deploy wBOING');
  wboing = predicted;
} else plan.wboing = { action: 'existing', address: wboing };

// 2. wrap
const need = seedBoing * BigInt(pairs.length);
const have = await getBoingTokenBalance(client, wboing, sender);
if (!send) {
  for (const p of pairs) p.existing = process.env[p.env] ?? null;
  const sim = await client.simulateContractCall(wboing, '0x' + '00'.repeat(31) + '24', { senderHex: sender });
  out({ ...plan, mode: 'dry-run', wboing_balance: have, wrap_needed: need > have ? need - have : 0n, wboing_total_supply_sim: sim });
  process.exit(0);
}
if (have < need) {
  await signSubmit(buildWrapBoingTransaction({ nonce: await nextNonce(), senderHex: sender, wboingHex: wboing, amount: need - have }), 'wrap BOING');
}

// 3-5. pools
const result = { wboing, pools: {} };
for (const p of pairs) {
  let pool = process.env[p.env] ? validateHex32(process.env[p.env]) : null;
  if (!pool) {
    const c = await createBoingPool(client, secret, { version: 10, tokenAHex: wboing, tokenBHex: p.token });
    await waitAll(c.txs, `${p.name}: deploy v10 + set_tokens`);
    pool = c.poolHex;
  }
  const st = await getBoingPoolState(client, pool);
  if (st.reserveA === 0n && st.reserveB === 0n) {
    const a = await addBoingPoolLiquidity(client, secret, { poolHex: pool, amountA: seedBoing, amountB: p.seed, version: 10 });
    await waitAll([...a.approvals, a], `${p.name}: approve + add_liquidity`);
  }
  const already = listed.some((r) => lc(r.poolHex) === lc(pool));
  if (!already) {
    const r = await submitContractCallWithSimulationRetry({
      client,
      secretKey32: secret,
      senderHex: sender,
      contractHex: factory,
      calldata: encodeNativeDexRegisterPairCalldata(wboing, p.token, pool),
      accessList: buildNativeDexFactoryAccessList(sender, factory),
    });
    const id = r.tx_id ?? r.tx_hash;
    const rc = await waitReceipt(id, `${p.name}: register_pair`);
    log.push({ step: `${p.name}: register_pair`, tx: id, block: rc.block_height });
  }
  const s2 = await getBoingPoolState(client, pool);
  result.pools[p.name] = { pool, reserveA: s2.reserveA, reserveB: s2.reserveB, totalLp: s2.totalLp };
}
out({ ok: true, mode: 'sent', sender, ...result, txs: log });
