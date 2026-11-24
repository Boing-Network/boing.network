/**
 * Confirm that a submitted transaction actually did what it was meant to.
 *
 * On Boing a contract that ends with `STOP` commits, so a receipt can say `success: true` although the call
 * changed nothing (reference NFT templates v1–v3 end unauthorized transfers this way). A fault reverts and
 * reports `success: false`. Apps that act on receipts should therefore check both: the receipt succeeded
 * **and** the state they expect is now on chain. {@link assertCallEffect} does that in one call.
 */

import type { ExecutionReceipt } from './types.js';
import { referenceNftOwnerStorageKey } from './referenceNft.js';
import { validateHex32 } from './hex.js';

export type CallEffectFailure = 'timeout' | 'reverted' | 'no_effect';

export class CallEffectError extends Error {
  constructor(
    public readonly kind: CallEffectFailure,
    message: string,
    public readonly receipt: ExecutionReceipt | null,
  ) {
    super(message);
    this.name = 'CallEffectError';
    Object.setPrototypeOf(this, CallEffectError.prototype);
  }
}

type ReceiptReader = { getTransactionReceipt(txIdHex: string): Promise<ExecutionReceipt | null> };
type StorageReader = { getContractStorage(contractHex: string, keyHex: string): Promise<unknown> };

export type WaitForReceiptOptions = {
  /** Give up after this long (default 60 s). */
  timeoutMs?: number;
  /** Delay between polls (default 1 s). */
  intervalMs?: number;
  signal?: AbortSignal;
  /** Injected for tests. */
  sleep?: (ms: number) => Promise<void>;
  now?: () => number;
};

const defaultSleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/** Poll `boing_getTransactionReceipt` until the tx is included. Throws `CallEffectError('timeout')`. */
export async function waitForReceipt(
  client: ReceiptReader,
  txIdHex: string,
  opts: WaitForReceiptOptions = {},
): Promise<ExecutionReceipt> {
  const timeoutMs = opts.timeoutMs ?? 60_000;
  const intervalMs = opts.intervalMs ?? 1_000;
  const sleep = opts.sleep ?? defaultSleep;
  const now = opts.now ?? Date.now;
  const deadline = now() + timeoutMs;
  for (;;) {
    if (opts.signal?.aborted) throw new CallEffectError('timeout', 'aborted while waiting for receipt', null);
    const r = await client.getTransactionReceipt(txIdHex);
    if (r) return r;
    if (now() >= deadline) {
      throw new CallEffectError('timeout', `no receipt for ${txIdHex} after ${timeoutMs} ms`, null);
    }
    await sleep(intervalMs);
  }
}

/**
 * Wait for the receipt, require `success: true`, then run `check` against current state. Returns the receipt.
 * Throws `CallEffectError` with kind `timeout`, `reverted` (receipt says failure) or `no_effect` (receipt
 * says success but `check` returned false: the call was a silent no-op).
 */
export async function assertCallEffect(
  client: ReceiptReader,
  txIdHex: string,
  check: (receipt: ExecutionReceipt) => boolean | Promise<boolean>,
  opts: WaitForReceiptOptions = {},
): Promise<ExecutionReceipt> {
  const receipt = await waitForReceipt(client, txIdHex, opts);
  if (!receipt.success) {
    throw new CallEffectError('reverted', `tx ${txIdHex} reverted: ${receipt.error ?? 'no error message'}`, receipt);
  }
  if (!(await check(receipt))) {
    throw new CallEffectError('no_effect', `tx ${txIdHex} succeeded but the expected state change is not on chain`, receipt);
  }
  return receipt;
}

function storageWordHex(v: unknown): string | null {
  let raw: unknown = v;
  if (raw && typeof raw === 'object' && 'value' in raw) raw = (raw as { value: unknown }).value;
  return typeof raw === 'string' && /^0x[0-9a-f]{64}$/i.test(raw) ? raw.toLowerCase() : null;
}

/**
 * `check` for {@link assertCallEffect}: the reference NFT (any template version) `tokenId` is now owned by
 * `expectedOwner`. Use after `transfer_nft` or `mint_batch`.
 */
export function referenceNftOwnedBy(
  client: StorageReader,
  collectionHex32: string,
  tokenIdHex32: string,
  expectedOwnerHex32: string,
): () => Promise<boolean> {
  const want = validateHex32(expectedOwnerHex32).toLowerCase();
  return async () =>
    storageWordHex(await client.getContractStorage(collectionHex32, referenceNftOwnerStorageKey(tokenIdHex32))) === want;
}
