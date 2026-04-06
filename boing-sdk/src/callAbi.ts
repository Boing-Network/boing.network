/**
 * Minimal **call layout** helpers: encode Boing `ContractCall.calldata` as a reference-style
 * **selector low byte + 32-byte words** without a foreign ABI (no keccak4, no Solidity ABI).
 *
 * Pair with {@link encodeBoingCall} / {@link boingWord*} from `calldata.ts` for full control.
 * See `docs/BOING-REFERENCE-TOKEN.md` and `docs/BOING-REFERENCE-NFT.md` for protocol-defined layouts.
 */

import {
  type BoingCalldataWord,
  boingWordAccount,
  boingWordFixed,
  boingWordU128,
  encodeBoingCall,
} from './calldata.js';
import { SELECTOR_MINT_FIRST, SELECTOR_TRANSFER } from './referenceToken.js';
import {
  SELECTOR_OWNER_OF,
  SELECTOR_SET_METADATA_HASH,
  SELECTOR_TRANSFER_NFT,
} from './referenceNft.js';
import {
  SELECTOR_NATIVE_AMM_ADD_LIQUIDITY,
  SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY,
  SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY_TO,
  SELECTOR_NATIVE_AMM_SWAP,
} from './nativeAmm.js';
import {
  SELECTOR_NATIVE_AMM_LP_VAULT_CONFIGURE,
} from './nativeAmmLpVault.js';
import {
  SELECTOR_LP_SHARE_MINT,
  SELECTOR_LP_SHARE_SET_MINTER_ONCE,
  SELECTOR_LP_SHARE_TRANSFER,
} from './nativeLpShareToken.js';

/** Supported 32-byte word kinds for schema-driven encoding. */
export type BoingAbiParamKind = 'u128' | 'account' | 'bytes32' | 'bool';

/** Structured argument before encoding to a word. */
export type BoingAbiArg =
  | { kind: 'u128'; value: bigint }
  | { kind: 'account'; hex32: string }
  | { kind: 'bytes32'; hexOrBytes: string | Uint8Array }
  | { kind: 'bool'; value: boolean };

export function abiArgU128(value: bigint | number | string): BoingAbiArg {
  const v = typeof value === 'bigint' ? value : BigInt(value);
  return { kind: 'u128', value: v };
}

export function abiArgAccount(hex32: string): BoingAbiArg {
  return { kind: 'account', hex32 };
}

export function abiArgBytes32(hexOrBytes: string | Uint8Array): BoingAbiArg {
  return { kind: 'bytes32', hexOrBytes };
}

export function abiArgBool(value: boolean): BoingAbiArg {
  return { kind: 'bool', value };
}

/** Map one structured arg to a 32-byte calldata word. */
export function encodeBoingAbiArgToWord(arg: BoingAbiArg): BoingCalldataWord {
  switch (arg.kind) {
    case 'u128':
      return boingWordU128(arg.value);
    case 'account':
      return boingWordAccount(arg.hex32);
    case 'bytes32':
      return boingWordFixed(arg.hexOrBytes);
    case 'bool':
      return boingWordU128(arg.value ? 1n : 0n);
    default:
      throw new Error('unhandled BoingAbiArg');
  }
}

/**
 * Encode `selectorLowByte` + words from structured args (same result shape as {@link encodeBoingCall}).
 */
export function encodeBoingCallFromAbiArgs(
  selectorLowByte: number,
  args: readonly BoingAbiArg[]
): Uint8Array {
  const words = args.map(encodeBoingAbiArgToWord);
  return encodeBoingCall(selectorLowByte, words);
}

function coerceValue(kind: BoingAbiParamKind, value: unknown): BoingAbiArg {
  switch (kind) {
    case 'u128':
      return abiArgU128(value as bigint | number | string);
    case 'account':
      if (typeof value !== 'string') throw new TypeError('account arg must be hex string');
      return abiArgAccount(value);
    case 'bytes32':
      if (typeof value !== 'string' && !(value instanceof Uint8Array)) {
        throw new TypeError('bytes32 arg must be hex string or Uint8Array');
      }
      return abiArgBytes32(value as string | Uint8Array);
    case 'bool':
      if (typeof value !== 'boolean') throw new TypeError('bool arg must be boolean');
      return abiArgBool(value);
    default:
      throw new Error('unhandled BoingAbiParamKind');
  }
}

/**
 * Encode from a simple param-kind list + runtime values (order must match).
 */
export function encodeBoingCallTyped(
  selectorLowByte: number,
  paramKinds: readonly BoingAbiParamKind[],
  values: readonly unknown[]
): Uint8Array {
  if (paramKinds.length !== values.length) {
    throw new Error(
      `encodeBoingCallTyped: expected ${paramKinds.length} values, got ${values.length}`
    );
  }
  const args = paramKinds.map((k, i) => coerceValue(k, values[i]!));
  return encodeBoingCallFromAbiArgs(selectorLowByte, args);
}

/** Descriptor for {@link encodeBoingCallFromDescriptor}. */
export interface BoingCallDescriptor {
  readonly selector: number;
  readonly params: readonly BoingAbiParamKind[];
}

/**
 * Built-in layouts matching **reference** token / NFT docs (selectors + word order).
 * Use {@link encodeBoingCallTyped} or {@link encodeBoingCallFromAbiArgs} for custom contracts.
 */
export const BoingReferenceCallDescriptors = {
  token: {
    transfer: {
      selector: SELECTOR_TRANSFER,
      params: ['account', 'u128'] as const satisfies readonly BoingAbiParamKind[],
    },
    mint_first: {
      selector: SELECTOR_MINT_FIRST,
      params: ['account', 'u128'] as const satisfies readonly BoingAbiParamKind[],
    },
  },
  nft: {
    owner_of: {
      selector: SELECTOR_OWNER_OF,
      params: ['bytes32'] as const satisfies readonly BoingAbiParamKind[],
    },
    transfer_nft: {
      selector: SELECTOR_TRANSFER_NFT,
      params: ['account', 'bytes32'] as const satisfies readonly BoingAbiParamKind[],
    },
    set_metadata_hash: {
      selector: SELECTOR_SET_METADATA_HASH,
      params: ['bytes32', 'bytes32'] as const satisfies readonly BoingAbiParamKind[],
    },
  },
  /** Native constant-product pool (`docs/NATIVE-AMM-CALLDATA.md`) — three u128 words after selector. */
  nativeAmm: {
    swap: {
      selector: SELECTOR_NATIVE_AMM_SWAP,
      params: ['u128', 'u128', 'u128'] as const satisfies readonly BoingAbiParamKind[],
    },
    add_liquidity: {
      selector: SELECTOR_NATIVE_AMM_ADD_LIQUIDITY,
      params: ['u128', 'u128', 'u128'] as const satisfies readonly BoingAbiParamKind[],
    },
    remove_liquidity: {
      selector: SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY,
      params: ['u128', 'u128', 'u128'] as const satisfies readonly BoingAbiParamKind[],
    },
    remove_liquidity_to: {
      selector: SELECTOR_NATIVE_AMM_REMOVE_LIQUIDITY_TO,
      params: ['u128', 'u128', 'u128', 'account', 'account'] as const satisfies readonly BoingAbiParamKind[],
    },
  },
  /**
   * Native LP share token (`docs/NATIVE-LP-SHARE-TOKEN.md`) — same word layout as reference fungible
   * for `transfer` / `mint`; `set_minter_once` is one account word.
   */
  lpShare: {
    transfer: {
      selector: SELECTOR_LP_SHARE_TRANSFER,
      params: ['account', 'u128'] as const satisfies readonly BoingAbiParamKind[],
    },
    mint: {
      selector: SELECTOR_LP_SHARE_MINT,
      params: ['account', 'u128'] as const satisfies readonly BoingAbiParamKind[],
    },
    set_minter_once: {
      selector: SELECTOR_LP_SHARE_SET_MINTER_ONCE,
      params: ['account'] as const satisfies readonly BoingAbiParamKind[],
    },
  },
  /** LP vault `configure` only (`docs/NATIVE-AMM-LP-VAULT.md`). Use `encodeNativeAmmLpVaultDepositAddCalldata` for `deposit_add`. */
  nativeAmmLpVault: {
    configure: {
      selector: SELECTOR_NATIVE_AMM_LP_VAULT_CONFIGURE,
      params: ['account', 'account'] as const satisfies readonly BoingAbiParamKind[],
    },
  },
} as const;

export function encodeBoingCallFromDescriptor(
  descriptor: BoingCallDescriptor,
  values: readonly unknown[]
): Uint8Array {
  return encodeBoingCallTyped(descriptor.selector, descriptor.params, values);
}
