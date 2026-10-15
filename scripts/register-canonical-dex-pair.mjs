#!/usr/bin/env node
/**
 * Register the canonical native CP pool in the canonical DEX pair directory (factory `register_pair`),
 * so `boing_listDexPools` / `boing_listDexTokens` stop returning empty lists.
 *
 * Why the lists are empty: the node builds them from the factory's on-chain pair table
 * (`pairs_count` + triplets). The full-stack deploy created the factory and pool but never called
 * `register_pair`, so `pairs_count` is 0. Registration is a normal signed contract call (anyone can
 * register; the factory is a permissionless directory) - no node config change is needed.
 *
 * Default is a DRY RUN: reads live addresses from `boing_getNetworkInfo.end_user`, the pool's token ids
 * from storage, the current `pairs_count`, and simulates `register_pair`. Nothing is signed.
 *
 *   node scripts/register-canonical-dex-pair.mjs                 # dry run (no key needed)
 *   BOING_SECRET_HEX=0x... node scripts/register-canonical-dex-pair.mjs --send
 *
 * v1 (ledger-only) pools store no token ids; pass BOING_DEX_TOKEN_A_HEX / BOING_DEX_TOKEN_B_HEX for those.
 * Env: BOING_RPC_URL (default https://testnet-rpc.boing.network/), BOING_SECRET_HEX (only with --send).
 * Costs one contract-call fee (~1-2 BOING) from the signer.
 */
import {
  buildNativeDexFactoryAccessList,
  createClient,
  encodeNativeDexPairsCountCalldataHex,
  encodeNativeDexRegisterPairCalldata,
  bytesToHex,
  hexToBytes,
  NATIVE_CONSTANT_PRODUCT_TOKEN_A_KEY_HEX,
  NATIVE_CONSTANT_PRODUCT_TOKEN_B_KEY_HEX,
  senderHexFromSecretKey,
  submitContractCallWithSimulationRetry,
  validateHex32,
} from '../boing-sdk/dist/index.js';

const send = process.argv.includes('--send');
const rpc = process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network/';
const client = createClient(rpc);
const ZERO = '0x' + '00'.repeat(32);

const info = await client.getNetworkInfo();
const eu = info.end_user ?? {};
const factory = eu.canonical_native_dex_factory;
const pool = eu.canonical_native_cp_pool;
if (!factory || !pool) {
  console.error(JSON.stringify({ ok: false, error: 'node does not advertise canonical_native_dex_factory / canonical_native_cp_pool' }));
  process.exit(1);
}
const word = async (key) => (await client.getContractStorage(pool, key)).value;
const slotA = validateHex32(await word(NATIVE_CONSTANT_PRODUCT_TOKEN_A_KEY_HEX));
const slotB = validateHex32(await word(NATIVE_CONSTANT_PRODUCT_TOKEN_B_KEY_HEX));
// v1 pools keep ledger-only reserves and no token ids; then the directory row needs explicit ids.
const envA = process.env.BOING_DEX_TOKEN_A_HEX?.trim();
const envB = process.env.BOING_DEX_TOKEN_B_HEX?.trim();
const tokenA = slotA !== ZERO ? slotA : envA ? validateHex32(envA) : ZERO;
const tokenB = slotB !== ZERO ? slotB : envB ? validateHex32(envB) : ZERO;
const countSim = await client.simulateContractCall(factory, encodeNativeDexPairsCountCalldataHex());
const pairsCount = countSim.return_data && countSim.return_data !== '0x' ? BigInt(countSim.return_data) : 0n;
const listed = await client.request('boing_listDexPools', [{ light: true }]);
const already = (listed?.pools ?? []).some((p) => p.poolHex?.toLowerCase() === pool.toLowerCase());

const plan = {
  rpc,
  chain_id: info.chain_id,
  factory,
  pool,
  token_a: tokenA,
  token_b: tokenB,
  pairs_count_before: pairsCount.toString(),
  already_listed: already,
};
if (tokenA === ZERO || tokenB === ZERO) {
  console.log(JSON.stringify({ ...plan, ok: false, error: 'the canonical pool is a v1 (ledger-only) pool with no token ids in storage; choose the token ids it should be listed under and pass BOING_DEX_TOKEN_A_HEX / BOING_DEX_TOKEN_B_HEX' }, null, 2));
  process.exit(1);
}
if (already) {
  console.log(JSON.stringify({ ...plan, ok: true, note: 'pool already registered; nothing to do' }, null, 2));
  process.exit(0);
}
const calldata = encodeNativeDexRegisterPairCalldata(tokenA, tokenB, pool);
const secretHex = process.env.BOING_SECRET_HEX?.trim();
const secret = send && secretHex ? hexToBytes(secretHex) : null;
const senderHex = secret ? senderHexFromSecretKey(secret) : null;
const sim = await client.simulateContractCall(factory, bytesToHex(calldata), { senderHex });
plan.simulation = { success: sim.success, gas_used: sim.gas_used, error: sim.error ?? null };

if (!send) {
  console.log(JSON.stringify({ ...plan, ok: sim.success, mode: 'dry-run', next: 'BOING_SECRET_HEX=0x... node scripts/register-canonical-dex-pair.mjs --send' }, null, 2));
  process.exit(sim.success ? 0 : 1);
}
if (!secret) {
  console.error('--send needs BOING_SECRET_HEX (any funded testnet key; never commit it).');
  process.exit(1);
}
const out = await submitContractCallWithSimulationRetry({
  client,
  secretKey32: secret,
  senderHex,
  contractHex: factory,
  calldata,
  accessList: buildNativeDexFactoryAccessList(senderHex, factory),
});
console.log(JSON.stringify({ ...plan, ok: true, mode: 'sent', sender: senderHex, tx_hash: out.tx_hash }, null, 2));
