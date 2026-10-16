/**
 * On-chain **interface registry**: a contract's deployer publishes the BLAKE3 hash + URI of an interface JSON.
 * Matches `boing_execution::interface_registry` (selectors `0xD0`–`0xD4`, CREATE2 salt `BOING_INTERFACE_REGISTRY_V1`).
 * See `docs/INTERFACE-REGISTRY.md`.
 *
 * The VM cannot see who deployed a contract, so `claim_asset` is first-caller. Clients must check that the claimer
 * is the contract's deployer (deploy tx sender) before treating a record as "published by deployer".
 */
import { blake3 } from '@noble/hashes/blake3';
import { buildContractDeployMetaTx, ensure0xHex } from './canonicalDeployArtifacts.js';
import { predictCreate2ContractAddress } from './create2.js';
import { DEFAULT_INTERFACE_REGISTRY_BYTECODE_HEX } from './defaultInterfaceRegistryBytecodeHex.js';
import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';
/** Public testnet registry (CREATE2 from the canonical DEX deployer). */
export const CANONICAL_BOING_TESTNET_INTERFACE_REGISTRY_HEX = validateHex32('0xbd7373b6a3830768b2cea3768bedc737b43d1f981123c83e7f0d75ebb310fb6c');
export const SELECTOR_IFACE_CLAIM_ASSET = 0xd0;
export const SELECTOR_IFACE_SET_INTERFACE = 0xd1;
export const SELECTOR_IFACE_GET_INTERFACE = 0xd2;
export const SELECTOR_IFACE_GET_CLAIMER = 0xd3;
export const SELECTOR_IFACE_TRANSFER_CLAIMER = 0xd4;
export const INTERFACE_REGISTRY_MAX_URI_BYTES = 192;
export const IFACE_FIELD_HASH = 0;
export const IFACE_FIELD_URI_LEN = 1;
export const IFACE_FIELD_URI0 = 2;
export const IFACE_FIELD_PUBLISHER = 8;
function asciiWord(s) {
    const u8 = new Uint8Array(32);
    u8.set(new TextEncoder().encode(s));
    return u8;
}
export const INTERFACE_REGISTRY_CREATE2_SALT_V1 = asciiWord('BOING_INTERFACE_REGISTRY_V1');
export const INTERFACE_REGISTRY_CREATE2_SALT_V1_HEX = bytesToHex(INTERFACE_REGISTRY_CREATE2_SALT_V1);
export const IFACE_TOPIC_SET_HEX = bytesToHex(asciiWord('BOING_IFACE_SET_V1'));
const CLAIMER_XOR = asciiWord('BOING_IFACE_CLAIMER_V1');
const FIELD_XOR_BASE = asciiWord('BOING_IFACE_FIELD_V1');
function xor(a, b) {
    const o = new Uint8Array(32);
    for (let i = 0; i < 32; i++)
        o[i] = a[i] ^ b[i];
    return o;
}
function word(n) {
    const o = new Uint8Array(32);
    let v = BigInt(n);
    for (let i = 31; i >= 0 && v > 0n; i--) {
        o[i] = Number(v & 0xffn);
        v >>= 8n;
    }
    return o;
}
function sel(s) {
    return word(s);
}
function concat(parts) {
    const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
    let o = 0;
    for (const p of parts) {
        out.set(p, o);
        o += p.length;
    }
    return out;
}
const acct = (h) => hexToBytes(validateHex32(h));
/** BLAKE3 of the exact interface JSON bytes (string is UTF-8 encoded). */
export function interfaceJsonHashHex(json) {
    const bytes = typeof json === 'string' ? new TextEncoder().encode(json) : json;
    return bytesToHex(blake3(bytes));
}
export function interfaceRegistryClaimerStorageKeyHex(contractHex32) {
    return bytesToHex(xor(acct(contractHex32), CLAIMER_XOR));
}
export function interfaceRegistryFieldStorageKeyHex(contractHex32, field) {
    if (!Number.isInteger(field) || field < 0 || field > IFACE_FIELD_PUBLISHER)
        throw new Error('bad field');
    const m = FIELD_XOR_BASE.slice();
    m[31] = field;
    return bytesToHex(xor(acct(contractHex32), m));
}
export function encodeIfaceClaimAssetCalldataHex(contractHex32) {
    return bytesToHex(concat([sel(SELECTOR_IFACE_CLAIM_ASSET), acct(contractHex32)]));
}
export function encodeIfaceSetInterfaceCalldataHex(contractHex32, interfaceHashHex32, uri) {
    const u = new TextEncoder().encode(uri);
    if (u.length > INTERFACE_REGISTRY_MAX_URI_BYTES) {
        throw new Error(`interface URI is ${u.length} bytes; max ${INTERFACE_REGISTRY_MAX_URI_BYTES}`);
    }
    const uriWords = new Uint8Array(INTERFACE_REGISTRY_MAX_URI_BYTES);
    uriWords.set(u);
    return bytesToHex(concat([sel(SELECTOR_IFACE_SET_INTERFACE), acct(contractHex32), acct(interfaceHashHex32), word(u.length), uriWords]));
}
export function encodeIfaceGetInterfaceCalldataHex(contractHex32) {
    return bytesToHex(concat([sel(SELECTOR_IFACE_GET_INTERFACE), acct(contractHex32)]));
}
export function encodeIfaceGetClaimerCalldataHex(contractHex32) {
    return bytesToHex(concat([sel(SELECTOR_IFACE_GET_CLAIMER), acct(contractHex32)]));
}
export function encodeIfaceTransferClaimerCalldataHex(contractHex32, newClaimerHex32) {
    return bytesToHex(concat([sel(SELECTOR_IFACE_TRANSFER_CLAIMER), acct(contractHex32), acct(newClaimerHex32)]));
}
/** Decode 320-byte `get_interface` return data; `null` when nothing is published. */
export function decodeIfaceGetInterfaceReturn(returnDataHex) {
    const b = hexToBytes(returnDataHex);
    if (b.length < 320)
        return null;
    const w = (i) => b.slice(i * 32, i * 32 + 32);
    const zero = (u) => u.every((x) => x === 0);
    if (zero(w(2)))
        return null;
    let len = 0;
    for (const x of b.slice(3 * 32 + 24, 4 * 32))
        len = len * 256 + x;
    len = Math.min(len, INTERFACE_REGISTRY_MAX_URI_BYTES);
    return {
        claimer: zero(w(0)) ? null : bytesToHex(w(0)),
        publisher: bytesToHex(w(1)),
        interfaceHash: bytesToHex(w(2)),
        uri: new TextDecoder().decode(b.slice(4 * 32, 4 * 32 + len)),
    };
}
/**
 * Read a record from storage (`client.getContractStorage.bind(client)` works as the reader). No simulation needed.
 * Returns `{ claimer, record }`; `record` is null when nothing is published.
 */
export async function fetchInterfaceRecord(getContractStorage, contractHex32, registryHex32 = CANONICAL_BOING_TESTNET_INTERFACE_REGISTRY_HEX) {
    const read = async (key) => hexToBytes((await getContractStorage(registryHex32, key)).value);
    const isZero = (u) => u.every((x) => x === 0);
    const claimerW = await read(interfaceRegistryClaimerStorageKeyHex(contractHex32));
    const claimer = isZero(claimerW) ? null : bytesToHex(claimerW);
    const hash = await read(interfaceRegistryFieldStorageKeyHex(contractHex32, IFACE_FIELD_HASH));
    if (isZero(hash))
        return { claimer, record: null };
    const [pub, lenW] = await Promise.all([
        read(interfaceRegistryFieldStorageKeyHex(contractHex32, IFACE_FIELD_PUBLISHER)),
        read(interfaceRegistryFieldStorageKeyHex(contractHex32, IFACE_FIELD_URI_LEN)),
    ]);
    let len = 0;
    for (const x of lenW.slice(24))
        len = len * 256 + x;
    len = Math.min(len, INTERFACE_REGISTRY_MAX_URI_BYTES);
    const words = await Promise.all(Array.from({ length: Math.ceil(len / 32) }, (_, i) => read(interfaceRegistryFieldStorageKeyHex(contractHex32, IFACE_FIELD_URI0 + i))));
    const uri = new TextDecoder().decode(concat(words).slice(0, len));
    return { claimer, record: { claimer, publisher: bytesToHex(pub), interfaceHash: bytesToHex(hash), uri } };
}
export function predictInterfaceRegistryCreate2Address(deployerHex32) {
    return predictCreate2ContractAddress(deployerHex32, INTERFACE_REGISTRY_CREATE2_SALT_V1, hexToBytes(DEFAULT_INTERFACE_REGISTRY_BYTECODE_HEX));
}
export function buildInterfaceRegistryDeployMetaTx(input) {
    return buildContractDeployMetaTx({
        bytecodeHex: ensure0xHex(DEFAULT_INTERFACE_REGISTRY_BYTECODE_HEX),
        assetName: 'Interface Registry',
        assetSymbol: 'IFACE',
        purposeCategory: input?.purposeCategory ?? 'dapp',
        create2SaltHex: INTERFACE_REGISTRY_CREATE2_SALT_V1_HEX,
    });
}
/** `contract_call` tx object for the registry (access list = sender + registry). */
export function buildInterfaceRegistryContractCallTx(senderHex32, registryHex32, calldataHex) {
    const s = validateHex32(senderHex32).toLowerCase();
    const r = validateHex32(registryHex32).toLowerCase();
    return {
        type: 'contract_call',
        contract: r,
        calldata: calldataHex,
        access_list: { read: [s, r], write: [s, r] },
    };
}
/** Claim (unless `alreadyClaimed`) + set_interface, in order. */
export function buildPublishInterfaceFlowTxs(input) {
    const reg = input.registryHex32 ?? CANONICAL_BOING_TESTNET_INTERFACE_REGISTRY_HEX;
    const out = [];
    if (!input.alreadyClaimed) {
        out.push(buildInterfaceRegistryContractCallTx(input.senderHex32, reg, encodeIfaceClaimAssetCalldataHex(input.contractHex32)));
    }
    out.push(buildInterfaceRegistryContractCallTx(input.senderHex32, reg, encodeIfaceSetInterfaceCalldataHex(input.contractHex32, input.interfaceHashHex32, input.uri)));
    return out;
}
