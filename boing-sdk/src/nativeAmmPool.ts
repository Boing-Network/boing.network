/**
 * Native constant-product pool — `contract_call` + access list for Boing Express / JSON-RPC.
 * Matches `Transaction::suggested_parallel_access_list` for `ContractCall` when only sender + pool touch state.
 */

import { mergeAccessListWithSimulation } from './accessList.js';
import { ensureHex, validateHex32 } from './hex.js';
import type { SimulateResult } from './types.js';

const HEX_RE = /^[0-9a-fA-F]+$/;

function normalizeCalldataHex(calldataHex: string): string {
  const h = ensureHex(calldataHex.trim());
  const raw = h.slice(2);
  if (raw.length % 2 !== 0) {
    throw new Error('calldata must be even-length hex');
  }
  if (!HEX_RE.test(raw)) {
    throw new Error('calldata: invalid hex');
  }
  return `0x${raw.toLowerCase()}`;
}

/** `read` and `write` both include signer + pool (parallel-scheduling minimum for pool-only bytecode). */
export function buildNativeConstantProductPoolAccessList(
  senderHex32: string,
  poolHex32: string
): { read: string[]; write: string[] } {
  const s = validateHex32(senderHex32).toLowerCase();
  const p = validateHex32(poolHex32).toLowerCase();
  return { read: [s, p], write: [s, p] };
}

/** Params for `boing_sendTransaction` / Express `contract_call` with explicit access list. */
export function buildNativeConstantProductContractCallTx(
  senderHex32: string,
  poolHex32: string,
  calldataHex: string
): {
  type: 'contract_call';
  contract: string;
  calldata: string;
  access_list: { read: string[]; write: string[] };
} {
  return {
    type: 'contract_call',
    contract: validateHex32(poolHex32).toLowerCase(),
    calldata: normalizeCalldataHex(calldataHex),
    access_list: buildNativeConstantProductPoolAccessList(senderHex32, poolHex32),
  };
}

/**
 * Widen `read`/`write` with `sim.suggested_access_list` (e.g. after `boing_simulateTransaction`).
 */
export function mergeNativePoolAccessListWithSimulation(
  senderHex32: string,
  poolHex32: string,
  sim: SimulateResult
): { read: string[]; write: string[] } {
  const base = buildNativeConstantProductPoolAccessList(senderHex32, poolHex32);
  return mergeAccessListWithSimulation(base.read, base.write, sim);
}
