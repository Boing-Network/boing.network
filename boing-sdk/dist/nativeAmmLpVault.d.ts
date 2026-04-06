/**
 * Native AMM LP vault calldata + Express / JSON-RPC access lists. Matches `boing_execution::native_amm_lp_vault`.
 * See `docs/NATIVE-AMM-LP-VAULT.md`.
 */
import type { SimulateResult } from './types.js';
/** `configure(pool, share_token)` — **96** bytes. */
export declare const SELECTOR_NATIVE_AMM_LP_VAULT_CONFIGURE = 192;
/** `deposit_add(inner_add_liquidity_128, min_lp)` — **192** bytes. */
export declare const SELECTOR_NATIVE_AMM_LP_VAULT_DEPOSIT_ADD = 193;
export declare function encodeNativeAmmLpVaultConfigureCalldata(poolHex32: string, shareTokenHex32: string): Uint8Array;
export declare function encodeNativeAmmLpVaultConfigureCalldataHex(poolHex32: string, shareTokenHex32: string): string;
export declare function encodeNativeAmmLpVaultDepositAddCalldata(innerAddLiquidity128: Uint8Array, minLp: bigint): Uint8Array;
export declare function encodeNativeAmmLpVaultDepositAddCalldataHex(innerAddLiquidity128: Uint8Array, minLp: bigint): string;
/** `read` / `write`: signer + vault (parallel scheduling minimum for configure-only). */
export declare function buildNativeAmmLpVaultConfigureAccessList(senderHex32: string, vaultHex32: string): {
    read: string[];
    write: string[];
};
/**
 * `read` / `write`: signer + vault + pool + share token (`deposit_add` nested `Call`s).
 */
export declare function buildNativeAmmLpVaultDepositAddAccessList(senderHex32: string, vaultHex32: string, poolHex32: string, shareTokenHex32: string): {
    read: string[];
    write: string[];
};
export declare function mergeNativeAmmLpVaultConfigureAccessListWithSimulation(senderHex32: string, vaultHex32: string, sim: SimulateResult): {
    read: string[];
    write: string[];
};
export declare function mergeNativeAmmLpVaultDepositAddAccessListWithSimulation(senderHex32: string, vaultHex32: string, poolHex32: string, shareTokenHex32: string, sim: SimulateResult): {
    read: string[];
    write: string[];
};
export declare function buildNativeAmmLpVaultConfigureContractCallTx(senderHex32: string, vaultHex32: string, calldataHex: string): {
    type: 'contract_call';
    contract: string;
    calldata: string;
    access_list: {
        read: string[];
        write: string[];
    };
};
export declare function buildNativeAmmLpVaultDepositAddContractCallTx(senderHex32: string, vaultHex32: string, poolHex32: string, shareTokenHex32: string, calldataHex: string): {
    type: 'contract_call';
    contract: string;
    calldata: string;
    access_list: {
        read: string[];
        write: string[];
    };
};
//# sourceMappingURL=nativeAmmLpVault.d.ts.map