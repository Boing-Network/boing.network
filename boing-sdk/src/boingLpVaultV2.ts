/**
 * LP vault v2 for custody (v8/v9) pools: `deposit` pulls both tokens, adds liquidity and mints vault shares
 * (1:1 with the LP the pool returns); `withdraw` burns shares and the pool pays the user directly;
 * `transfer_shares` (self-transfer is a no-op). Mirrors `crates/boing-execution/src/native_lp_vault_v2.rs`
 * (see `tests/boingLpVaultV2.test.ts`, vectors from the CLI).
 */
import type { BoingClient } from './client.js';
import type { TransactionInput } from './bincode.js';
import { transactionIdFromSignedTransactionHex } from './bincode.js';
import { NATIVE_LP_VAULT_V2_BYTECODE_HEX } from './defaultNativeCpPoolBytecodeHex.js';
import { bytesToHex, hexToAccountId, hexToBytes, validateHex32 } from './hex.js';
import { calldataAccountIdWord, calldataSelectorLastByte, calldataU128BeWord, concatCalldata } from './calldata.js';
import { buildBoingTokenApproveTransaction, decodeBoingTokenBalanceWord } from './boingToken.js';
import { BOING_POOL_PURPOSE, buildBoingPoolCallTransaction, getBoingPoolState, quoteBoingPoolLpMinted, type BoingPoolSignedTx, type BoingPoolSendOptions } from './boingPool.js';
import { fetchNextNonce, senderHexFromSecretKey, signTransactionInput, type Ed25519SecretKey32 } from './transactionBuilder.js';
import { submitSignedTransactionIdempotent } from './submitIdempotent.js';

export const BOING_LP_VAULT_V2_BYTECODE_HEX = NATIVE_LP_VAULT_V2_BYTECODE_HEX;
export const BOING_LP_VAULT_V2_SELECTOR_CONFIGURE = 0xc0;
export const BOING_LP_VAULT_V2_SELECTOR_DEPOSIT = 0xc1;
export const BOING_LP_VAULT_V2_SELECTOR_WITHDRAW = 0xc2;
export const BOING_LP_VAULT_V2_SELECTOR_TRANSFER_SHARES = 0xc3;

const ascii32 = (s: string): Uint8Array => {
  const b = new Uint8Array(32);
  b.set(new TextEncoder().encode(s));
  return b;
};
/** Share slot = `holder ^ "BOING_LP_VAULT_V2_SHARES"` (zero-padded). */
export const BOING_LP_VAULT_V2_SHARE_XOR = ascii32('BOING_LP_VAULT_V2_SHARES');
const slot = (n: number) => `0x${n.toString(16).padStart(64, '0')}`;
export const BOING_LP_VAULT_V2_KEY_CONFIGURED_HEX = slot(0xe1);
export const BOING_LP_VAULT_V2_KEY_POOL_HEX = slot(0xe2);
export const BOING_LP_VAULT_V2_KEY_TOKEN_A_HEX = slot(0xe3);
export const BOING_LP_VAULT_V2_KEY_TOKEN_B_HEX = slot(0xe4);
export const BOING_LP_VAULT_V2_KEY_TOTAL_SHARES_HEX = slot(0xe5);

export function boingLpVaultV2ShareKeyHex(holderHex: string): string {
  const h = hexToBytes(validateHex32(holderHex));
  const k = new Uint8Array(32);
  for (let i = 0; i < 32; i++) k[i] = h[i]! ^ BOING_LP_VAULT_V2_SHARE_XOR[i]!;
  return bytesToHex(k);
}

export const encodeBoingLpVaultV2ConfigureCalldata = (poolHex: string, tokenAHex: string, tokenBHex: string) =>
  concatCalldata([calldataSelectorLastByte(BOING_LP_VAULT_V2_SELECTOR_CONFIGURE), calldataAccountIdWord(poolHex), calldataAccountIdWord(tokenAHex), calldataAccountIdWord(tokenBHex)]);
export const encodeBoingLpVaultV2DepositCalldata = (amountA: bigint, amountB: bigint, minShares: bigint, deadline = 0n) =>
  concatCalldata([calldataSelectorLastByte(BOING_LP_VAULT_V2_SELECTOR_DEPOSIT), calldataU128BeWord(amountA), calldataU128BeWord(amountB), calldataU128BeWord(minShares), calldataU128BeWord(deadline)]);
export const encodeBoingLpVaultV2WithdrawCalldata = (shares: bigint, minA: bigint, minB: bigint, deadline = 0n) =>
  concatCalldata([calldataSelectorLastByte(BOING_LP_VAULT_V2_SELECTOR_WITHDRAW), calldataU128BeWord(shares), calldataU128BeWord(minA), calldataU128BeWord(minB), calldataU128BeWord(deadline)]);
export const encodeBoingLpVaultV2TransferSharesCalldata = (toHex: string, amount: bigint) =>
  concatCalldata([calldataSelectorLastByte(BOING_LP_VAULT_V2_SELECTOR_TRANSFER_SHARES), calldataAccountIdWord(toHex), calldataU128BeWord(amount)]);

export function buildBoingLpVaultV2DeployTransaction(input: { nonce: bigint; senderHex: string }): TransactionInput {
  const sender = hexToAccountId(validateHex32(input.senderHex));
  return {
    nonce: input.nonce,
    sender,
    payload: {
      kind: 'contractDeployWithPurpose',
      bytecode: hexToBytes(NATIVE_LP_VAULT_V2_BYTECODE_HEX),
      purposeCategory: BOING_POOL_PURPOSE,
      descriptionHash: null,
      create2Salt: null,
    },
    accessList: { read: [sender], write: [sender] },
  };
}

export interface BoingLpVaultV2Config {
  vaultHex: string;
  poolHex: string;
  tokenAHex: string;
  tokenBHex: string;
}

const vaultCall = (nonce: bigint, senderHex: string, c: BoingLpVaultV2Config, calldata: Uint8Array) =>
  buildBoingPoolCallTransaction({ nonce, senderHex, poolHex: c.vaultHex, calldata, extraAccountsHex: [c.poolHex, c.tokenAHex, c.tokenBHex] });

export const buildBoingLpVaultV2ConfigureTransaction = (nonce: bigint, senderHex: string, c: BoingLpVaultV2Config) =>
  vaultCall(nonce, senderHex, c, encodeBoingLpVaultV2ConfigureCalldata(c.poolHex, c.tokenAHex, c.tokenBHex));
export const buildBoingLpVaultV2DepositTransaction = (nonce: bigint, senderHex: string, c: BoingLpVaultV2Config, a: bigint, b: bigint, minShares: bigint, deadline = 0n) =>
  vaultCall(nonce, senderHex, c, encodeBoingLpVaultV2DepositCalldata(a, b, minShares, deadline));
export const buildBoingLpVaultV2WithdrawTransaction = (nonce: bigint, senderHex: string, c: BoingLpVaultV2Config, shares: bigint, minA: bigint, minB: bigint, deadline = 0n) =>
  vaultCall(nonce, senderHex, c, encodeBoingLpVaultV2WithdrawCalldata(shares, minA, minB, deadline));
export const buildBoingLpVaultV2TransferSharesTransaction = (nonce: bigint, senderHex: string, c: BoingLpVaultV2Config, toHex: string, amount: bigint) =>
  buildBoingPoolCallTransaction({ nonce, senderHex, poolHex: c.vaultHex, calldata: encodeBoingLpVaultV2TransferSharesCalldata(toHex, amount), extraAccountsHex: [toHex] });

/** Exact-amount approvals (vault as spender) + deposit, in nonce order. */
export function buildBoingLpVaultV2DepositFlow(nonce: bigint, senderHex: string, c: BoingLpVaultV2Config, a: bigint, b: bigint, minShares: bigint, deadline = 0n): TransactionInput[] {
  return [
    buildBoingTokenApproveTransaction({ nonce, senderHex, tokenHex: c.tokenAHex, toHex: c.vaultHex, amount: a }),
    buildBoingTokenApproveTransaction({ nonce: nonce + 1n, senderHex, tokenHex: c.tokenBHex, toHex: c.vaultHex, amount: b }),
    buildBoingLpVaultV2DepositTransaction(nonce + 2n, senderHex, c, a, b, minShares, deadline),
  ];
}

/**
 * Balanced deposit for `amountA` at current reserves: `amountB = ceil(amountA * rB / rA)`, expected shares
 * (= pool LP minted) and `minShares` after `slippageBps`. Off-ratio excess is donated to the pool, so use this.
 */
export function quoteBoingLpVaultV2Deposit(
  s: { reserveA: bigint; reserveB: bigint; totalLp: bigint },
  amountA: bigint,
  slippageBps = 50n,
): { amountA: bigint; amountB: bigint; shares: bigint; minShares: bigint } {
  if (amountA <= 0n) throw new Error('amountA must be > 0');
  if (s.reserveA === 0n || s.reserveB === 0n || s.totalLp === 0n) throw new Error('pool has no liquidity; seed it directly');
  const amountB = (amountA * s.reserveB + s.reserveA - 1n) / s.reserveA;
  const shares = quoteBoingPoolLpMinted(s.reserveA, s.reserveB, s.totalLp, amountA, amountB);
  if (shares <= 0n) throw new Error('deposit too small to mint shares');
  let minShares = (shares * (10_000n - slippageBps)) / 10_000n;
  if (minShares < 1n) minShares = 1n;
  return { amountA, amountB, shares, minShares };
}

/** Withdraw outputs for `shares` (pool `burn * reserve / totalLp`) and slippage-bounded minimums. */
export function quoteBoingLpVaultV2Withdraw(
  s: { reserveA: bigint; reserveB: bigint; totalLp: bigint },
  shares: bigint,
  slippageBps = 50n,
): { outA: bigint; outB: bigint; minA: bigint; minB: bigint } {
  const outA = (shares * s.reserveA) / s.totalLp;
  const outB = (shares * s.reserveB) / s.totalLp;
  return { outA, outB, minA: (outA * (10_000n - slippageBps)) / 10_000n, minB: (outB * (10_000n - slippageBps)) / 10_000n };
}

async function word(client: BoingClient, c: string, k: string): Promise<string> {
  const w = await client.getContractStorage(validateHex32(c), k);
  if (typeof w?.value !== 'string') throw new Error('unexpected boing_getContractStorage result');
  return w.value;
}

export async function getBoingLpVaultV2Shares(client: BoingClient, vaultHex: string, holderHex: string): Promise<bigint> {
  return decodeBoingTokenBalanceWord(await word(client, vaultHex, boingLpVaultV2ShareKeyHex(holderHex)));
}

export async function getBoingLpVaultV2State(client: BoingClient, vaultHex: string): Promise<{ configured: boolean; poolHex: string; tokenAHex: string; tokenBHex: string; totalShares: bigint }> {
  const [c, p, a, b, t] = await Promise.all(
    [BOING_LP_VAULT_V2_KEY_CONFIGURED_HEX, BOING_LP_VAULT_V2_KEY_POOL_HEX, BOING_LP_VAULT_V2_KEY_TOKEN_A_HEX, BOING_LP_VAULT_V2_KEY_TOKEN_B_HEX, BOING_LP_VAULT_V2_KEY_TOTAL_SHARES_HEX].map((k) => word(client, vaultHex, k)),
  );
  return { configured: BigInt(c!) !== 0n, poolHex: p!.toLowerCase(), tokenAHex: a!.toLowerCase(), tokenBHex: b!.toLowerCase(), totalShares: decodeBoingTokenBalanceWord(t!) };
}

async function signAll(client: BoingClient | null, txs: TransactionInput[], key: Ed25519SecretKey32, dry: boolean): Promise<BoingPoolSignedTx[]> {
  const out: BoingPoolSignedTx[] = [];
  for (const tx of txs) {
    const signedHex = await signTransactionInput(tx, key);
    const txId = transactionIdFromSignedTransactionHex(signedHex);
    if (!dry) {
      if (!client) throw new Error('a BoingClient is required to submit');
      await submitSignedTransactionIdempotent(client, signedHex);
    }
    out.push({ txId, signedHex, submitted: !dry });
  }
  return out;
}

/** approve A, approve B (exact), deposit. Quotes a balanced `amountB` and `minShares` unless given. */
export async function depositBoingLpVaultV2(
  client: BoingClient | null,
  key: Ed25519SecretKey32,
  input: BoingLpVaultV2Config & { amountA: bigint; amountB?: bigint; minShares?: bigint; slippageBps?: bigint; deadlineHeight?: bigint } & BoingPoolSendOptions,
): Promise<{ txs: BoingPoolSignedTx[]; amountB: bigint; minShares: bigint }> {
  let { amountB, minShares } = input;
  if (amountB == null || minShares == null) {
    if (!client) throw new Error('offline deposit needs amountB and minShares');
    const q = quoteBoingLpVaultV2Deposit(await getBoingPoolState(client, input.poolHex), input.amountA, input.slippageBps ?? 50n);
    amountB ??= q.amountB;
    minShares ??= q.minShares;
  }
  const senderHex = await senderHexFromSecretKey(key);
  let nonce = input.nonce;
  if (nonce == null) {
    if (!client) throw new Error('pass a nonce or a BoingClient');
    nonce = await fetchNextNonce(client, senderHex);
  }
  const txs = buildBoingLpVaultV2DepositFlow(nonce, senderHex, input, input.amountA, amountB, minShares, input.deadlineHeight ?? 0n);
  return { txs: await signAll(client, txs, key, !!input.dryRun), amountB, minShares };
}

/** Burn `shares`; the pool pays both tokens to the signer. Quotes minimums unless given. */
export async function withdrawBoingLpVaultV2(
  client: BoingClient | null,
  key: Ed25519SecretKey32,
  input: BoingLpVaultV2Config & { shares: bigint; minA?: bigint; minB?: bigint; slippageBps?: bigint; deadlineHeight?: bigint } & BoingPoolSendOptions,
): Promise<{ tx: BoingPoolSignedTx; minA: bigint; minB: bigint }> {
  let { minA, minB } = input;
  if (minA == null || minB == null) {
    if (!client) throw new Error('offline withdraw needs minA and minB');
    const q = quoteBoingLpVaultV2Withdraw(await getBoingPoolState(client, input.poolHex), input.shares, input.slippageBps ?? 50n);
    minA ??= q.minA;
    minB ??= q.minB;
  }
  const senderHex = await senderHexFromSecretKey(key);
  let nonce = input.nonce;
  if (nonce == null) {
    if (!client) throw new Error('pass a nonce or a BoingClient');
    nonce = await fetchNextNonce(client, senderHex);
  }
  const tx = buildBoingLpVaultV2WithdrawTransaction(nonce, senderHex, input, input.shares, minA, minB, input.deadlineHeight ?? 0n);
  const [s] = await signAll(client, [tx], key, !!input.dryRun);
  return { tx: s!, minA, minB };
}