/**
 * Thin ergonomic wrapper around `submitContractCallWithSimulationRetry` for a fixed contract + signer.
 */
import { submitContractCallWithSimulationRetry, } from './submitFlow.js';
/**
 * Factory for repeated `ContractCall` submits to one contract with shared signer/client options.
 */
export function createNativeContractSubmitter(config) {
    return {
        /** Submit arbitrary calldata with simulate → access-list retry → submit. */
        async submitCalldata(calldata, overrides) {
            return submitContractCallWithSimulationRetry({
                client: config.client,
                secretKey32: config.secretKey32,
                senderHex: config.senderHex,
                contractHex: config.contractHex,
                calldata,
                accessList: overrides?.accessList ?? config.accessList,
                maxSimulationRetries: overrides?.maxSimulationRetries ?? config.maxSimulationRetries,
            });
        },
    };
}
