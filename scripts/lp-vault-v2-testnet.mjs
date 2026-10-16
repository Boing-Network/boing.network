#!/usr/bin/env node
/**
 * LP vault v2 on the public testnet (USDC/BOING v9 pool by default). Dry run unless --send.
 *
 *   node scripts/lp-vault-v2-testnet.mjs deploy  [--send]                 # deploy + configure(pool, tokenA, tokenB)
 *   node scripts/lp-vault-v2-testnet.mjs deposit --amount-a 50 [--send]   # exact approvals + balanced deposit
 *   node scripts/lp-vault-v2-testnet.mjs withdraw --shares 20 [--send]    # burn shares, pool pays the signer
 *   node scripts/lp-vault-v2-testnet.mjs state
 *
 * Env: BOING_SECRET_HEX (signer; never commit/print), BOING_RPC_URL, BOING_LP_VAULT_V2_HEX (deposit/withdraw/state),
 * BOING_POOL_HEX (default: canonical USDC/BOING v9 pool). Every tx is simulated before it is submitted.
 */
import {
  CANONICAL_BOING_TESTNET_USDC_BOING_POOL_HEX,
  buildBoingLpVaultV2ConfigureTransaction,
  buildBoingLpVaultV2DeployTransaction,
  buildBoingLpVaultV2DepositFlow,
  buildBoingLpVaultV2WithdrawTransaction,
  createClient,
  fetchNextNonce,
  getBoingLpVaultV2Shares,
  getBoingLpVaultV2State,
  getBoingPoolState,
  getBoingTokenBalance,
  hexToBytes,
  predictNonceDerivedContractAddress,
  quoteBoingLpVaultV2Deposit,
  quoteBoingLpVaultV2Withdraw,
  senderHexFromSecretKey,
  signTransactionInput,
  submitSignedTransactionIdempotent,
  transactionIdFromSignedTransactionHex,
  validateHex32,
} from '../boing-sdk/dist/index.js';

const [cmd = 'state'] = process.argv.slice(2).filter((a) => !a.startsWith('--') && !/^\d+$/.test(a));
const opt = (n) => (process.argv.includes(n) ? process.argv[process.argv.indexOf(n) + 1] : undefined);
const send = process.argv.includes('--send');
const client = createClient(process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network/');
const poolHex = validateHex32(process.env.BOING_POOL_HEX?.trim() || CANONICAL_BOING_TESTNET_USDC_BOING_POOL_HEX);
const json = (o) => JSON.stringify(o, (_, v) => (typeof v === 'bigint' ? v.toString() : v), 2);
const secretHex = process.env.BOING_SECRET_HEX?.trim();
if (cmd !== 'state' && !secretHex) throw new Error(`${cmd} needs BOING_SECRET_HEX`);
const secret = secretHex ? hexToBytes(secretHex) : null;
const sender = secret ? await senderHexFromSecretKey(secret) : null;
const pool = await getBoingPoolState(client, poolHex);
const cfgFor = (vaultHex) => ({ vaultHex, poolHex, tokenAHex: validateHex32(pool.tokenAHex), tokenBHex: validateHex32(pool.tokenBHex) });

async function waitTx(txId) {
  for (let i = 0; i < 60; i++) {
    const r = await client.getTransactionReceipt(txId).catch(() => null);
    if (r) {
      if (!r.success) throw new Error(`${txId} failed on-chain: ${r.error}`);
      return r;
    }
    await new Promise((s) => setTimeout(s, 2000));
  }
  throw new Error(`${txId} not included after 120 s`);
}
/** Sign; simulate when `simulate`; submit + wait when --send. */
async function run(label, tx, simulate = true) {
  const signed = await signTransactionInput(tx, secret);
  const txId = transactionIdFromSignedTransactionHex(signed);
  if (simulate) {
    const sim = await client.simulateTransaction(signed);
    if (!sim.success) throw new Error(`${label} simulation failed: ${sim.error}`);
  }
  if (!send) {
    console.log(`[dry-run] ${label} ${txId} (signed, simulated=${simulate}, NOT submitted)`);
    return { txId };
  }
  const r = await submitSignedTransactionIdempotent(client, signed);
  const rc = await waitTx(txId);
  console.log(`${label} ${txId} ${r} block ${rc.block_height ?? '?'}`);
  return { txId, block: rc.block_height };
}
const balances = async (vaultHex) => ({
  tokenA: await getBoingTokenBalance(client, pool.tokenAHex, sender),
  tokenB: await getBoingTokenBalance(client, pool.tokenBHex, sender),
  shares: vaultHex ? await getBoingLpVaultV2Shares(client, vaultHex, sender) : 0n,
});

if (cmd === 'deploy') {
  const nonce = await fetchNextNonce(client, sender);
  const vaultHex = predictNonceDerivedContractAddress(sender, nonce);
  console.log(json({ vaultHex, poolHex, tokenAHex: pool.tokenAHex, tokenBHex: pool.tokenBHex, deployNonce: nonce }));
  const d = await run('deploy', buildBoingLpVaultV2DeployTransaction({ nonce, senderHex: sender }));
  // configure can only be simulated once the vault exists.
  const c = await run('configure', buildBoingLpVaultV2ConfigureTransaction(nonce + 1n, sender, cfgFor(vaultHex)), send);
  if (send) console.log(json({ vaultHex, deployTx: d.txId, configureTx: c.txId, state: await getBoingLpVaultV2State(client, vaultHex) }));
} else {
  const vaultHex = validateHex32(opt('--vault') ?? process.env.BOING_LP_VAULT_V2_HEX ?? '');
  const st = await getBoingLpVaultV2State(client, vaultHex);
  if (!st.configured || st.poolHex.slice(-64) !== poolHex.slice(-64)) throw new Error(`vault not configured for ${poolHex}: ${json(st)}`);
  const cfg = cfgFor(vaultHex);
  if (cmd === 'state') {
    console.log(json({ vault: st, pool, signer: sender ? await balances(vaultHex) : null }));
  } else if (cmd === 'deposit') {
    const q = quoteBoingLpVaultV2Deposit(pool, BigInt(opt('--amount-a') ?? '50'), 100n);
    const before = await balances(vaultHex);
    const nonce = await fetchNextNonce(client, sender);
    const [apA, apB, dep] = buildBoingLpVaultV2DepositFlow(nonce, sender, cfg, q.amountA, q.amountB, q.minShares);
    console.log(json({ quote: q, before }));
    const r1 = await run('approve A', apA);
    // Later txs use future nonces / need earlier ones applied, so a dry run can only simulate the first.
    const r2 = await run('approve B', apB, send);
    const r3 = await run('deposit', dep, send);
    if (send) {
      const after = await balances(vaultHex);
      console.log(json({ after, vault: await getBoingLpVaultV2State(client, vaultHex), txs: [r1.txId, r2.txId, r3.txId] }));
      if (before.tokenA - after.tokenA !== q.amountA || before.tokenB - after.tokenB !== q.amountB) throw new Error('token deltas mismatch');
      if (after.shares - before.shares < q.minShares) throw new Error('fewer shares than minShares');
      for (const t of [pool.tokenAHex, pool.tokenBHex]) if ((await getBoingTokenBalance(client, t, vaultHex)) !== 0n) throw new Error('vault holds tokens');
      console.log('deposit ok');
    }
  } else if (cmd === 'withdraw') {
    const shares = BigInt(opt('--shares') ?? '0');
    const q = quoteBoingLpVaultV2Withdraw(pool, shares, 100n);
    const before = await balances(vaultHex);
    const nonce = await fetchNextNonce(client, sender);
    console.log(json({ quote: q, before }));
    const r = await run('withdraw', buildBoingLpVaultV2WithdrawTransaction(nonce, sender, cfg, shares, q.minA, q.minB));
    if (send) {
      const after = await balances(vaultHex);
      console.log(json({ after, vault: await getBoingLpVaultV2State(client, vaultHex), tx: r.txId }));
      if (before.shares - after.shares !== shares) throw new Error('share delta mismatch');
      if (after.tokenA - before.tokenA < q.minA || after.tokenB - before.tokenB < q.minB) throw new Error('received less than minimums');
      console.log('withdraw ok');
    }
  } else throw new Error(`unknown command ${cmd}`);
}