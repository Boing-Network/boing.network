/**
 * Minimal **call layout** helpers: encode Boing `ContractCall.calldata` as a reference-style
 * **selector low byte + 32-byte words** without a foreign ABI (no keccak4, no Solidity ABI).
 *
 * Pair with {@link encodeBoingCall} / {@link boingWord*} from `calldata.ts` for full control.
 * See `docs/BOING-REFERENCE-TOKEN.md` and `docs/BOING-REFERENCE-NFT.md` for protocol-defined layouts.
 */
import { type BoingCalldataWord } from './calldata.js';
/** Supported 32-byte word kinds for schema-driven encoding. */
export type BoingAbiParamKind = 'u128' | 'account' | 'bytes32' | 'bool';
/** Structured argument before encoding to a word. */
export type BoingAbiArg = {
    kind: 'u128';
    value: bigint;
} | {
    kind: 'account';
    hex32: string;
} | {
    kind: 'bytes32';
    hexOrBytes: string | Uint8Array;
} | {
    kind: 'bool';
    value: boolean;
};
export declare function abiArgU128(value: bigint | number | string): BoingAbiArg;
export declare function abiArgAccount(hex32: string): BoingAbiArg;
export declare function abiArgBytes32(hexOrBytes: string | Uint8Array): BoingAbiArg;
export declare function abiArgBool(value: boolean): BoingAbiArg;
/** Map one structured arg to a 32-byte calldata word. */
export declare function encodeBoingAbiArgToWord(arg: BoingAbiArg): BoingCalldataWord;
/**
 * Encode `selectorLowByte` + words from structured args (same result shape as {@link encodeBoingCall}).
 */
export declare function encodeBoingCallFromAbiArgs(selectorLowByte: number, args: readonly BoingAbiArg[]): Uint8Array;
/**
 * Encode from a simple param-kind list + runtime values (order must match).
 */
export declare function encodeBoingCallTyped(selectorLowByte: number, paramKinds: readonly BoingAbiParamKind[], values: readonly unknown[]): Uint8Array;
/** Descriptor for {@link encodeBoingCallFromDescriptor}. */
export interface BoingCallDescriptor {
    readonly selector: number;
    readonly params: readonly BoingAbiParamKind[];
}
/**
 * Built-in layouts matching **reference** token / NFT docs (selectors + word order).
 * Use {@link encodeBoingCallTyped} or {@link encodeBoingCallFromAbiArgs} for custom contracts.
 */
export declare const BoingReferenceCallDescriptors: {
    readonly token: {
        readonly transfer: {
            readonly selector: 1;
            readonly params: readonly ["account", "u128"];
        };
        readonly mint_first: {
            readonly selector: 2;
            readonly params: readonly ["account", "u128"];
        };
    };
    readonly nft: {
        readonly owner_of: {
            readonly selector: 3;
            readonly params: readonly ["bytes32"];
        };
        readonly transfer_nft: {
            readonly selector: 4;
            readonly params: readonly ["account", "bytes32"];
        };
        readonly set_metadata_hash: {
            readonly selector: 5;
            readonly params: readonly ["bytes32", "bytes32"];
        };
    };
    /** Native constant-product pool (`docs/NATIVE-AMM-CALLDATA.md`) — three u128 words after selector. */
    readonly nativeAmm: {
        readonly swap: {
            readonly selector: 16;
            readonly params: readonly ["u128", "u128", "u128"];
        };
        readonly add_liquidity: {
            readonly selector: 17;
            readonly params: readonly ["u128", "u128", "u128"];
        };
        readonly remove_liquidity: {
            readonly selector: 18;
            readonly params: readonly ["u128", "u128", "u128"];
        };
        readonly remove_liquidity_to: {
            readonly selector: 22;
            readonly params: readonly ["u128", "u128", "u128", "account", "account"];
        };
    };
    /**
     * Native LP share token (`docs/NATIVE-LP-SHARE-TOKEN.md`) — same word layout as reference fungible
     * for `transfer` / `mint`; `set_minter_once` is one account word.
     */
    readonly lpShare: {
        readonly transfer: {
            readonly selector: 1;
            readonly params: readonly ["account", "u128"];
        };
        readonly mint: {
            readonly selector: 6;
            readonly params: readonly ["account", "u128"];
        };
        readonly set_minter_once: {
            readonly selector: 7;
            readonly params: readonly ["account"];
        };
    };
    /** LP vault `configure` only (`docs/NATIVE-AMM-LP-VAULT.md`). Use `encodeNativeAmmLpVaultDepositAddCalldata` for `deposit_add`. */
    readonly nativeAmmLpVault: {
        readonly configure: {
            readonly selector: 192;
            readonly params: readonly ["account", "account"];
        };
    };
};
export declare function encodeBoingCallFromDescriptor(descriptor: BoingCallDescriptor, values: readonly unknown[]): Uint8Array;
//# sourceMappingURL=callAbi.d.ts.map