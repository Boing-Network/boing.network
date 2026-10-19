/**
 * On-chain **linked NFT ↔ fungible token** registry (enforced).
 * Matches `boing_execution::linked_nft_token_registry`.
 * See `docs/BOING-LINKED-NFT-TOKEN.md`.
 *
 * **Public testnet** registry AccountId: {@link CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX}
 * (v2, CREATE2 salt `BOING_NFT_TOKEN_LINK_REG_V2`, selectors `0xE0`–`0xE8`).
 *
 * **v2 (current)** — only an asset's creator can claim it (`boing_execution::linked_nft_token_registry_v2`):
 * - the asset's own init code calls `claim_self(claimer)` ({@link SELECTOR_LINKED_NFT_TOKEN_CLAIM_SELF}) inside
 *   the deploy tx, so the creator is the claimer from the first block (see
 *   {@link buildLinkedNftTokenClaimSelfInitSnippetHex});
 * - `claim_asset(asset)` only works for the deployer recorded by the registry's own deploy (assets that
 *   existed when v2 went live), or as a no-op for the current claimer;
 * - failed writes fail the transaction (v1 silently did nothing).
 *
 * v1 ({@link CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_V1_HEX}) let the first caller claim any
 * AccountId. Its claims and links were copied into v2 when v2 was deployed.
 */
import { mergeAccessListWithSimulation } from './accessList.js';
import { buildContractDeployMetaTx, ensure0xHex, } from './canonicalDeployArtifacts.js';
import { DEFAULT_LINKED_NFT_TOKEN_REGISTRY_BYTECODE_HEX } from './defaultLinkedNftTokenRegistryBytecodeHex.js';
import { DEFAULT_LINKED_NFT_TOKEN_REGISTRY_V2_RUNTIME_BYTECODE_HEX } from './defaultLinkedNftTokenRegistryV2BytecodeHex.js';
import { bytesToHex, ensureHex, hexToBytes, validateHex32 } from './hex.js';
import { decodeBoingStorageWordU128 } from './nativeAmmPool.js';
/**
 * Live linked NFT↔token registry on **public Boing testnet** (`https://testnet-rpc.boing.network/`):
 * **v2** (creator-only claims), CREATE2 salt `BOING_NFT_TOKEN_LINK_REG_V2`, seeded with the v1 claims,
 * links and a deployer record for every contract that existed at the copy height. Mainnet: not deployed.
 */
export const CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX = validateHex32('0xf704485ad29f56bb6516ac822fcf9ca8fcb6c4266e22c9ed7f9c6908668655dd');
/**
 * Previous (v1) testnet registry: first caller of `claim_asset` became the claimer. Read-only history;
 * do not send new claims or links there.
 */
export const CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_V1_HEX = validateHex32('0xebf9f0190f415852f90d0e60343126201248ab96273fdbf8acc5fe5fa03c3dd8');
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
/** v2: `claim_self(claimer)` — **64** bytes; `CALLER` is the asset (call it from the asset's init code). */
export const SELECTOR_LINKED_NFT_TOKEN_CLAIM_SELF = 0xe7;
/** v2: `get_asset_deployer(asset)` — **64** bytes; seeded deployer record (zero if none). */
export const SELECTOR_LINKED_NFT_TOKEN_GET_ASSET_DEPLOYER = 0xe8;
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
/** v2: `Log2` topic0 after a claim (topic1 = asset, data = claimer). */
export const LINKED_NFT_TOKEN_TOPIC_CLAIM_V2_HEX = (() => {
    const u8 = new Uint8Array(32);
    u8.set(new TextEncoder().encode('BOING_NFT_TOKEN_LINK_CLM2'));
    return bytesToHex(u8);
})();
export const LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2_HEX = (() => {
    const u8 = new Uint8Array(32);
    u8.set(new TextEncoder().encode('BOING_NFT_TOKEN_LINK_REG_V2'));
    return bytesToHex(u8);
})();
function asciiWord(text) {
    const u8 = new Uint8Array(32);
    u8.set(new TextEncoder().encode(text));
    return u8;
}
function xorWords(a, b) {
    const o = new Uint8Array(32);
    for (let i = 0; i < 32; i++)
        o[i] = a[i] ^ b[i];
    return o;
}
/** Claimer slot of `asset` in the registry storage (v1 and v2): `asset ^ "BOING_LNK_CLAIMER_V1"`. */
export function linkedNftTokenClaimerStorageKeyHex(assetHex32) {
    return bytesToHex(xorWords(hexToBytes(validateHex32(assetHex32)), asciiWord('BOING_LNK_CLAIMER_V1')));
}
/** v2 deployer-record slot of `asset`: `asset ^ "BOING_LNK_DEPLOYER_V2"`. */
export function linkedNftTokenDeployerStorageKeyHex(assetHex32) {
    return bytesToHex(xorWords(hexToBytes(validateHex32(assetHex32)), asciiWord('BOING_LNK_DEPLOYER_V2')));
}
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
export function encodeLinkedNftTokenClaimSelfCalldata(claimerHex32) {
    return concatWords([
        selectorWord(SELECTOR_LINKED_NFT_TOKEN_CLAIM_SELF),
        hexToBytes(validateHex32(claimerHex32)),
    ]);
}
export function encodeLinkedNftTokenClaimSelfCalldataHex(claimerHex32) {
    return bytesToHex(encodeLinkedNftTokenClaimSelfCalldata(claimerHex32));
}
export function encodeLinkedNftTokenGetAssetDeployerCalldata(assetHex32) {
    return concatWords([
        selectorWord(SELECTOR_LINKED_NFT_TOKEN_GET_ASSET_DEPLOYER),
        hexToBytes(validateHex32(assetHex32)),
    ]);
}
export function encodeLinkedNftTokenGetAssetDeployerCalldataHex(assetHex32) {
    return bytesToHex(encodeLinkedNftTokenGetAssetDeployerCalldata(assetHex32));
}
/**
 * Init-code snippet (hex, no `0x` marker byte) for an **asset** deploy that runs `0xFD` init code:
 * `CALL registry.claim_self(CALLER)`, so the deploy tx sender becomes the asset's claimer in the same
 * transaction that creates it. Same bytes as `linked_nft_token_registry_v2_claim_self_init_snippet` in
 * `boing_execution`. Uses memory `[0, 64)` and leaves the CALL flag on the stack, so place it before the
 * init writes the runtime to memory. If `registry` has no code the call does nothing.
 */
export function buildLinkedNftTokenClaimSelfInitSnippetHex(registryHex32) {
    const PUSH32 = 0x7f;
    const MSTORE = 0x52;
    const CALLER = 0x33;
    const CALL = 0xf1;
    const parts = [];
    const push = (w) => {
        parts.push(PUSH32, ...w);
    };
    const num = (n) => {
        const w = new Uint8Array(32);
        new DataView(w.buffer).setBigUint64(24, BigInt(n), false);
        return w;
    };
    push(selectorWord(SELECTOR_LINKED_NFT_TOKEN_CLAIM_SELF));
    push(num(0));
    parts.push(MSTORE, CALLER);
    push(num(32));
    parts.push(MSTORE);
    push(hexToBytes(validateHex32(registryHex32)));
    push(num(0));
    push(num(64));
    push(num(0));
    push(num(0));
    parts.push(CALL);
    return bytesToHex(Uint8Array.from(parts));
}
/** v2 runtime bytecode (a plain deploy of it is an empty v2 registry for a new network). */
export function resolveLinkedNftTokenRegistryV2BytecodeHex(opts) {
    if (opts?.explicitHex?.trim())
        return ensure0xHex(opts.explicitHex);
    return ensure0xHex(DEFAULT_LINKED_NFT_TOKEN_REGISTRY_V2_RUNTIME_BYTECODE_HEX);
}
/** Deploy an empty v2 registry (salt `BOING_NFT_TOKEN_LINK_REG_V2`). */
export function buildLinkedNftTokenRegistryV2DeployMetaTx(input) {
    return buildLinkedNftTokenRegistryDeployMetaTx({
        ...input,
        assetName: input?.assetName?.trim() || 'Linked NFT Token Registry v2',
        assetSymbol: input?.assetSymbol?.trim() || 'LNTR2',
        create2SaltHex: input?.create2SaltHex ?? LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2_HEX,
        bytecodeHexOverride: input?.bytecodeHexOverride?.trim()
            ? input.bytecodeHexOverride
            : resolveLinkedNftTokenRegistryV2BytecodeHex(),
    });
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
 *
 * With `includeClaims: false` only `register_link` is returned: use it when both assets claimed
 * themselves at deploy (v2 `claim_self` in their init code). On v2 a `claim_asset` by the current
 * claimer is a no-op, so the default three-tx flow still works for such assets.
 */
export function buildLinkedNftTokenRegisterFlowTxs(input) {
    const { senderHex32, registryHex32, collectionHex32, tokenHex32 } = input;
    if (input.includeClaims === false) {
        return [
            buildLinkedNftTokenRegistryContractCallTx(senderHex32, registryHex32, encodeLinkedNftTokenRegisterLinkCalldataHex(collectionHex32, tokenHex32)),
        ];
    }
    return [
        buildLinkedNftTokenRegistryContractCallTx(senderHex32, registryHex32, encodeLinkedNftTokenClaimAssetCalldataHex(collectionHex32)),
        buildLinkedNftTokenRegistryContractCallTx(senderHex32, registryHex32, encodeLinkedNftTokenClaimAssetCalldataHex(tokenHex32)),
        buildLinkedNftTokenRegistryContractCallTx(senderHex32, registryHex32, encodeLinkedNftTokenRegisterLinkCalldataHex(collectionHex32, tokenHex32)),
    ];
}
