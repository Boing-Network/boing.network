import { type ContractDeployMetaTxObject } from './canonicalDeployArtifacts.js';
/** Public testnet registry (CREATE2 from the canonical DEX deployer). */
export declare const CANONICAL_BOING_TESTNET_INTERFACE_REGISTRY_HEX: string;
export declare const SELECTOR_IFACE_CLAIM_ASSET = 208;
export declare const SELECTOR_IFACE_SET_INTERFACE = 209;
export declare const SELECTOR_IFACE_GET_INTERFACE = 210;
export declare const SELECTOR_IFACE_GET_CLAIMER = 211;
export declare const SELECTOR_IFACE_TRANSFER_CLAIMER = 212;
export declare const INTERFACE_REGISTRY_MAX_URI_BYTES = 192;
export declare const IFACE_FIELD_HASH = 0;
export declare const IFACE_FIELD_URI_LEN = 1;
export declare const IFACE_FIELD_URI0 = 2;
export declare const IFACE_FIELD_PUBLISHER = 8;
export declare const INTERFACE_REGISTRY_CREATE2_SALT_V1: Uint8Array<ArrayBufferLike>;
export declare const INTERFACE_REGISTRY_CREATE2_SALT_V1_HEX: string;
export declare const IFACE_TOPIC_SET_HEX: string;
/** BLAKE3 of the exact interface JSON bytes (string is UTF-8 encoded). */
export declare function interfaceJsonHashHex(json: string | Uint8Array): `0x${string}`;
export declare function interfaceRegistryClaimerStorageKeyHex(contractHex32: string): string;
export declare function interfaceRegistryFieldStorageKeyHex(contractHex32: string, field: number): string;
export declare function encodeIfaceClaimAssetCalldataHex(contractHex32: string): string;
export declare function encodeIfaceSetInterfaceCalldataHex(contractHex32: string, interfaceHashHex32: string, uri: string): string;
export declare function encodeIfaceGetInterfaceCalldataHex(contractHex32: string): string;
export declare function encodeIfaceGetClaimerCalldataHex(contractHex32: string): string;
export declare function encodeIfaceTransferClaimerCalldataHex(contractHex32: string, newClaimerHex32: string): string;
export type InterfaceRecord = {
    claimer: string | null;
    publisher: string;
    interfaceHash: string;
    uri: string;
};
/** Decode 320-byte `get_interface` return data; `null` when nothing is published. */
export declare function decodeIfaceGetInterfaceReturn(returnDataHex: string): InterfaceRecord | null;
type StorageReader = (contractHex32: string, keyHex32: string) => Promise<{
    value: string;
}>;
/**
 * Read a record from storage (`client.getContractStorage.bind(client)` works as the reader). No simulation needed.
 * Returns `{ claimer, record }`; `record` is null when nothing is published.
 */
export declare function fetchInterfaceRecord(getContractStorage: StorageReader, contractHex32: string, registryHex32?: string): Promise<{
    claimer: string | null;
    record: InterfaceRecord | null;
}>;
export declare function predictInterfaceRegistryCreate2Address(deployerHex32: string): string;
export declare function buildInterfaceRegistryDeployMetaTx(input?: {
    purposeCategory?: string;
}): ContractDeployMetaTxObject;
/** `contract_call` tx object for the registry (access list = sender + registry). */
export declare function buildInterfaceRegistryContractCallTx(senderHex32: string, registryHex32: string, calldataHex: string): {
    type: "contract_call";
    contract: string;
    calldata: string;
    access_list: {
        read: string[];
        write: string[];
    };
};
/** Claim (unless `alreadyClaimed`) + set_interface, in order. */
export declare function buildPublishInterfaceFlowTxs(input: {
    senderHex32: string;
    contractHex32: string;
    interfaceHashHex32: string;
    uri: string;
    registryHex32?: string;
    alreadyClaimed?: boolean;
}): Array<ReturnType<typeof buildInterfaceRegistryContractCallTx>>;
export {};
//# sourceMappingURL=interfaceRegistry.d.ts.map