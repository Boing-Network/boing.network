/**
 * On-chain **linked NFT ↔ fungible token** registry (enforced).
 * Matches `boing_execution::linked_nft_token_registry`.
 * See `docs/BOING-LINKED-NFT-TOKEN.md`.
 *
 * **Public testnet** registry AccountId: {@link CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX}
 * on `https://testnet-rpc.boing.network/` (CREATE2 salt `BOING_NFT_TOKEN_LINK_REG_V1`, selectors `0xE0`–`0xE6`).
 */
import { mergeAccessListWithSimulation } from './accessList.js';
import { buildContractDeployMetaTx, ensure0xHex, } from './canonicalDeployArtifacts.js';
import { DEFAULT_LINKED_NFT_TOKEN_REGISTRY_BYTECODE_HEX } from './defaultLinkedNftTokenRegistryBytecodeHex.js';
import { bytesToHex, ensureHex, hexToBytes, validateHex32 } from './hex.js';
import { decodeBoingStorageWordU128 } from './nativeAmmPool.js';
/**
 * Live linked NFT↔token registry on **public Boing testnet** (`https://testnet-rpc.boing.network/`).
 * Deployed with salt `BOING_NFT_TOKEN_LINK_REG_V1` after PR #42 (`924c0ba`). Mainnet: not deployed.
 */
export const CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX = validateHex32('0xebf9f0190f415852f90d0e60343126201248ab96273fdbf8acc5fe5fa03c3dd8');
/** `claim_asset(asset)` — **64** bytes. */
export const SELECTOR_LINKED_NFT_TOKEN_CLAIM_ASSET = 0xe0;
/** `register_link(collection, token)` — **96** bytes. */
export const SELECTOR_LINKED_NFT_TOKEN_REGISTER_LINK = 0xe1;
/** `unlink_at(index)` — **64** bytes. */
export const SELECTOR_LINKED_NFT_TOKEN_UNLINK_AT = 0xe2;
/** `links_count` — **32** bytes. */
export const SELECTOR_LINKED_NFT_TOKEN_LINKS_COUNT = 0xe3;
/** `get_link_at(index)` — **64** bytes; returns **64** bytes. */
export const SELECTOR_LINKED_NFT_TOKEN_GET_LINK_AT = 0xe4;
/** `get_asset_claimer(asset)` — **64** bytes. */
export const SELECTOR_LINKED_NFT_TOKEN_GET_ASSET_CLAIMER = 0xe5;
/** `transfer_asset_claimer(asset, new_claimer)` — **96** bytes. */
export const SELECTOR_LINKED_NFT_TOKEN_TRANSFER_ASSET_CLAIMER = 0xe6;
export const LINKED_NFT_TOKEN_REGISTRY_MAX_LINKS = 4096;
export const LINKED_NFT_TOKEN_TOPIC_REGISTER_HEX = (() => {
    const u8 = new Uint8Array(32);
    u8.set(new TextEncoder().encode('BOING_NFT_TOKEN_LINK_REG1'));
    return bytesToHex(u8);
})();
export const LINKED_NFT_TOKEN_TOPIC_UNLINK_HEX = (() => {
    const u8 = new Uint8Array(32);
    u8.set(new TextEncoder().encode('BOING_NFT_TOKEN_LINK_UNL1'));
    return bytesToHex(u8);
})();
export const LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V1_HEX = (() => {
    const u8 = new Uint8Array(32);
    u8.set(new TextEncoder().encode('BOING_NFT_TOKEN_LINK_REG_V1'));
    return bytesToHex(u8);
})();
/** Count storage key (`linked_nft_token_registry_count_key`). */
export const LINKED_NFT_TOKEN_REGISTRY_COUNT_KEY_HEX = `0x${'00'.repeat(16)}424f494e474c4e4b00000000ffffffff`;
const HEX_RE = /^[0-9a-fA-F]+$/;
function selectorWord(selector) {
    const w = new Uint8Array(32);
    w[31] = selector & 0xff;
    return w;
}
function u64Word(n) {
    if (!Number.isInteger(n) || n < 0 || n > Number.MAX_SAFE_INTEGER) {
        throw new RangeError('index must be a non-negative safe integer');
    }
    const w = new Uint8Array(32);
    new DataView(w.buffer).setBigUint64(24, BigInt(n), false);
    return w;
}
function concatWords(parts) {
    const n = parts.reduce((a, p) => a + p.length, 0);
    const out = new Uint8Array(n);
    let o = 0;
    for (const p of parts) {
        out.set(p, o);
        o += p.length;
    }
    return out;
}
function normalizeCalldataHex(calldataHex) {
    const h = ensureHex(calldataHex.trim());
    const raw = h.slice(2);
    if (raw.length % 2 !== 0)
        throw new Error('calldata must be even-length hex');
    if (!HEX_RE.test(raw))
        throw new Error('calldata: invalid hex');
    return `0x${raw.toLowerCase()}`;
}
export function encodeLinkedNftTokenClaimAssetCalldata(assetHex32) {
    return concatWords([
        selectorWord(SELECTOR_LINKED_NFT_TOKEN_CLAIM_ASSET),
        hexToBytes(validateHex32(assetHex32)),
    ]);
}
export function encodeLinkedNftTokenClaimAssetCalldataHex(assetHex32) {
    return bytesToHex(encodeLinkedNftTokenClaimAssetCalldata(assetHex32));
}
export function encodeLinkedNftTokenRegisterLinkCalldata(collectionHex32, tokenHex32) {
    return concatWords([
        selectorWord(SELECTOR_LINKED_NFT_TOKEN_REGISTER_LINK),
        hexToBytes(validateHex32(collectionHex32)),
        hexToBytes(validateHex32(tokenHex32)),
    ]);
}
export function encodeLinkedNftTokenRegisterLinkCalldataHex(collectionHex32, tokenHex32) {
    return bytesToHex(encodeLinkedNftTokenRegisterLinkCalldata(collectionHex32, tokenHex32));
}
export function encodeLinkedNftTokenUnlinkAtCalldata(index) {
    return concatWords([selectorWord(SELECTOR_LINKED_NFT_TOKEN_UNLINK_AT), u64Word(index)]);
}
export function encodeLinkedNftTokenUnlinkAtCalldataHex(index) {
    return bytesToHex(encodeLinkedNftTokenUnlinkAtCalldata(index));
}
export function encodeLinkedNftTokenLinksCountCalldata() {
    return selectorWord(SELECTOR_LINKED_NFT_TOKEN_LINKS_COUNT);
}
export function encodeLinkedNftTokenLinksCountCalldataHex() {
    return bytesToHex(encodeLinkedNftTokenLinksCountCalldata());
}
export function encodeLinkedNftTokenGetLinkAtCalldata(index) {
    return concatWords([selectorWord(SELECTOR_LINKED_NFT_TOKEN_GET_LINK_AT), u64Word(index)]);
}
export function encodeLinkedNftTokenGetLinkAtCalldataHex(index) {
    return bytesToHex(encodeLinkedNftTokenGetLinkAtCalldata(index));
}
export function encodeLinkedNftTokenGetAssetClaimerCalldata(assetHex32) {
    return concatWords([
        selectorWord(SELECTOR_LINKED_NFT_TOKEN_GET_ASSET_CLAIMER),
        hexToBytes(validateHex32(assetHex32)),
    ]);
}
export function encodeLinkedNftTokenGetAssetClaimerCalldataHex(assetHex32) {
    return bytesToHex(encodeLinkedNftTokenGetAssetClaimerCalldata(assetHex32));
}
export function encodeLinkedNftTokenTransferAssetClaimerCalldata(assetHex32, newClaimerHex32) {
    return concatWords([
        selectorWord(SELECTOR_LINKED_NFT_TOKEN_TRANSFER_ASSET_CLAIMER),
        hexToBytes(validateHex32(assetHex32)),
        hexToBytes(validateHex32(newClaimerHex32)),
    ]);
}
export function encodeLinkedNftTokenTransferAssetClaimerCalldataHex(assetHex32, newClaimerHex32) {
    return bytesToHex(encodeLinkedNftTokenTransferAssetClaimerCalldata(assetHex32, newClaimerHex32));
}
export function resolveLinkedNftTokenRegistryBytecodeHex(opts) {
    if (opts?.explicitHex?.trim())
        return ensure0xHex(opts.explicitHex);
    return ensure0xHex(DEFAULT_LINKED_NFT_TOKEN_REGISTRY_BYTECODE_HEX);
}
export function buildLinkedNftTokenRegistryDeployMetaTx(input) {
    const bytecodeHex = input?.bytecodeHexOverride?.trim()
        ? ensure0xHex(input.bytecodeHexOverride)
        : resolveLinkedNftTokenRegistryBytecodeHex();
    return buildContractDeployMetaTx({
        bytecodeHex,
        assetName: input?.assetName?.trim() || 'Linked NFT Token Registry',
        assetSymbol: input?.assetSymbol?.trim() || 'LNTR',
        purposeCategory: input?.purposeCategory ?? 'dapp',
        descriptionHashHex: input?.descriptionHashHex,
        create2SaltHex: input?.create2SaltHex ?? LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V1_HEX,
    });
}
export function buildLinkedNftTokenRegistryAccessList(senderHex32, registryHex32) {
    const s = validateHex32(senderHex32).toLowerCase();
    const r = validateHex32(registryHex32).toLowerCase();
    return { read: [s, r], write: [s, r] };
}
export function buildLinkedNftTokenRegistryContractCallTx(senderHex32, registryHex32, calldataHex) {
    return {
        type: 'contract_call',
        contract: validateHex32(registryHex32).toLowerCase(),
        calldata: normalizeCalldataHex(calldataHex),
        access_list: buildLinkedNftTokenRegistryAccessList(senderHex32, registryHex32),
    };
}
export function mergeLinkedNftTokenRegistryAccessListWithSimulation(senderHex32, registryHex32, sim) {
    const base = buildLinkedNftTokenRegistryAccessList(senderHex32, registryHex32);
    return mergeAccessListWithSimulation(base.read, base.write, sim);
}
export function decodeLinkedNftTokenLinksCountReturnData(returnDataHex) {
    const w = decodeBoingStorageWordU128(ensureHex(returnDataHex));
    return w & 0xffffffffffffffffn;
}
export function decodeLinkedNftTokenGetLinkAtReturnData(returnDataHex) {
    const raw = ensureHex(returnDataHex).slice(2).toLowerCase();
    if (!HEX_RE.test(raw))
        throw new Error('get_link_at return data: invalid hex');
    if (raw.length < 128) {
        throw new Error('get_link_at return data: expected 64 bytes (128 hex chars)');
    }
    const collectionHex = validateHex32(`0x${raw.slice(0, 64)}`);
    const tokenHex = validateHex32(`0x${raw.slice(64, 128)}`);
    const zero = '0x' + '0'.repeat(64);
    return {
        collectionHex,
        tokenHex,
        active: collectionHex !== zero && tokenHex !== zero,
    };
}
/**
 * Build Express `contract_call` txs to claim both assets and register one edge.
 * Caller must be (or become) claimer of both AccountIds — typically the joint deployer.
 */
export function buildLinkedNftTokenRegisterFlowTxs(input) {
    const { senderHex32, registryHex32, collectionHex32, tokenHex32 } = input;
    return [
        buildLinkedNftTokenRegistryContractCallTx(senderHex32, registryHex32, encodeLinkedNftTokenClaimAssetCalldataHex(collectionHex32)),
        buildLinkedNftTokenRegistryContractCallTx(senderHex32, registryHex32, encodeLinkedNftTokenClaimAssetCalldataHex(tokenHex32)),
        buildLinkedNftTokenRegistryContractCallTx(senderHex32, registryHex32, encodeLinkedNftTokenRegisterLinkCalldataHex(collectionHex32, tokenHex32)),
    ];
}
