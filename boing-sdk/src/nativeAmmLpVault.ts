/**
 * Native AMM LP vault calldata + Express / JSON-RPC access lists. Matches `boing_execution::native_amm_lp_vault`.
 * See `docs/NATIVE-AMM-LP-VAULT.md`.
 */

import { mergeAccessListWithSimulation } from './accessList.js';
import { bytesToHex, ensureHex, hexToBytes, validateHex32 } from './hex.js';
import type { SimulateResult } from './types.js';

/** `configure(pool, share_token)` — **96** bytes. */
export const SELECTOR_NATIVE_AMM_LP_VAULT_CONFIGURE = 0xc0;
/** `deposit_add(inner_add_liquidity_128, min_lp)` — **192** bytes. */
export const SELECTOR_NATIVE_AMM_LP_VAULT_DEPOSIT_ADD = 0xc1;

function selectorWord(selector: number): Uint8Array {
  const w = new Uint8Array(32);
  w[31] = selector & 0xff;
  return w;
}

const CALldata_HEX_RE = /^[0-9a-fA-F]+$/;

function normalizeCalldataHex(calldataHex: string): string {
  const h = ensureHex(calldataHex.trim());
  const raw = h.slice(2);
  if (raw.length % 2 !== 0) {
    throw new Error('calldata must be even-length hex');
  }
  if (!CALldata_HEX_RE.test(raw)) {
    throw new Error('calldata: invalid hex');
  }
  return `0x${raw.toLowerCase()}`;
}

function sortedUniqueAccounts(hex32List: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const x of hex32List) {
    const h = validateHex32(x).toLowerCase();
    if (!seen.has(h)) {
      seen.add(h);
      out.push(h);
    }
  }
  out.sort();
  return out;
}

function amountWord(amount: bigint): Uint8Array {
  const w = new Uint8Array(32);
  if (amount < 0n || amount > (1n << 128n) - 1n) {
    throw new RangeError('amount must fit in u128');
  }
  const be = new Uint8Array(16);
  let x = amount;
  for (let i = 15; i >= 0; i--) {
    be[i] = Number(x & 0xffn);
    x >>= 8n;
  }
  w.set(be, 16);
  return w;
}

export function encodeNativeAmmLpVaultConfigureCalldata(poolHex32: string, shareTokenHex32: string): Uint8Array {
  const out = new Uint8Array(96);
  out.set(selectorWord(SELECTOR_NATIVE_AMM_LP_VAULT_CONFIGURE));
  out.set(hexToBytes(validateHex32(poolHex32)), 32);
  out.set(hexToBytes(validateHex32(shareTokenHex32)), 64);
  return out;
}

export function encodeNativeAmmLpVaultConfigureCalldataHex(poolHex32: string, shareTokenHex32: string): string {
  return bytesToHex(encodeNativeAmmLpVaultConfigureCalldata(poolHex32, shareTokenHex32));
}

export function encodeNativeAmmLpVaultDepositAddCalldata(innerAddLiquidity128: Uint8Array, minLp: bigint): Uint8Array {
  if (innerAddLiquidity128.length !== 128) {
    throw new Error('inner add_liquidity calldata must be 128 bytes');
  }
  const out = new Uint8Array(192);
  out.set(selectorWord(SELECTOR_NATIVE_AMM_LP_VAULT_DEPOSIT_ADD));
  out.set(innerAddLiquidity128, 32);
  out.set(amountWord(minLp), 160);
  return out;
}

export function encodeNativeAmmLpVaultDepositAddCalldataHex(
  innerAddLiquidity128: Uint8Array,
  minLp: bigint
): string {
  return bytesToHex(encodeNativeAmmLpVaultDepositAddCalldata(innerAddLiquidity128, minLp));
}

/** `read` / `write`: signer + vault (parallel scheduling minimum for configure-only). */
export function buildNativeAmmLpVaultConfigureAccessList(
  senderHex32: string,
  vaultHex32: string
): { read: string[]; write: string[] } {
  const accounts = sortedUniqueAccounts([senderHex32, vaultHex32]);
  return { read: accounts, write: [...accounts] };
}

/**
 * `read` / `write`: signer + vault + pool + share token (`deposit_add` nested `Call`s).
 */
export function buildNativeAmmLpVaultDepositAddAccessList(
  senderHex32: string,
  vaultHex32: string,
  poolHex32: string,
  shareTokenHex32: string
): { read: string[]; write: string[] } {
  const accounts = sortedUniqueAccounts([senderHex32, vaultHex32, poolHex32, shareTokenHex32]);
  return { read: accounts, write: [...accounts] };
}

export function mergeNativeAmmLpVaultConfigureAccessListWithSimulation(
  senderHex32: string,
  vaultHex32: string,
  sim: SimulateResult
): { read: string[]; write: string[] } {
  const base = buildNativeAmmLpVaultConfigureAccessList(senderHex32, vaultHex32);
  return mergeAccessListWithSimulation(base.read, base.write, sim);
}

export function mergeNativeAmmLpVaultDepositAddAccessListWithSimulation(
  senderHex32: string,
  vaultHex32: string,
  poolHex32: string,
  shareTokenHex32: string,
  sim: SimulateResult
): { read: string[]; write: string[] } {
  const base = buildNativeAmmLpVaultDepositAddAccessList(senderHex32, vaultHex32, poolHex32, shareTokenHex32);
  return mergeAccessListWithSimulation(base.read, base.write, sim);
}

export function buildNativeAmmLpVaultConfigureContractCallTx(
  senderHex32: string,
  vaultHex32: string,
  calldataHex: string
): {
  type: 'contract_call';
  contract: string;
  calldata: string;
  access_list: { read: string[]; write: string[] };
} {
  return {
    type: 'contract_call',
    contract: validateHex32(vaultHex32).toLowerCase(),
    calldata: normalizeCalldataHex(calldataHex),
    access_list: buildNativeAmmLpVaultConfigureAccessList(senderHex32, vaultHex32),
  };
}

export function buildNativeAmmLpVaultDepositAddContractCallTx(
  senderHex32: string,
  vaultHex32: string,
  poolHex32: string,
  shareTokenHex32: string,
  calldataHex: string
): {
  type: 'contract_call';
  contract: string;
  calldata: string;
  access_list: { read: string[]; write: string[] };
} {
  return {
    type: 'contract_call',
    contract: validateHex32(vaultHex32).toLowerCase(),
    calldata: normalizeCalldataHex(calldataHex),
    access_list: buildNativeAmmLpVaultDepositAddAccessList(senderHex32, vaultHex32, poolHex32, shareTokenHex32),
  };
}
