/**
 * Thin ergonomic wrapper around `submitContractCallWithSimulationRetry` for a fixed contract + signer.
 */
import type { BoingClient } from './client.js';
import { type SubmitContractCallWithSimulationOptions, type SubmitFlowResult } from './submitFlow.js';
import type { Ed25519SecretKey32 } from './transactionBuilder.js';
export interface NativeContractSubmitterConfig {
    client: BoingClient;
    secretKey32: Ed25519SecretKey32;
    senderHex: string;
    contractHex: string;
    /** Default access list for calls (signer + contract is typical). */
    accessList?: {
        read: string[];
        write: string[];
    };
    maxSimulationRetries?: number;
}
/**
 * Factory for repeated `ContractCall` submits to one contract with shared signer/client options.
 */
export declare function createNativeContractSubmitter(config: NativeContractSubmitterConfig): {
    /** Submit arbitrary calldata with simulate → access-list retry → submit. */
    submitCalldata(calldata: Uint8Array, overrides?: Pick<SubmitContractCallWithSimulationOptions, "accessList" | "maxSimulationRetries">): Promise<SubmitFlowResult>;
};
//# sourceMappingURL=nativeContractSubmit.d.ts.map