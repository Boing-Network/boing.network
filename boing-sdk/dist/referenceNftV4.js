/**
 * Reference **NFT collection template v4** — calldata, storage keys, receipt-log decoding, storage reads and
 * deploy helpers. Mirrors `crates/boing-execution/src/reference_nft_v4.rs`; spec: `docs/BOING-REFERENCE-NFT.md`
 * § Template v4.
 *
 * v4 keeps the v1–v3 selectors (`owner_of` 0x03, `transfer_nft` 0x04, `set_metadata_hash` 0x05, `mint_batch`
 * 0x06) and their calldata layouts, so the encoders in `referenceNft.ts` work unchanged. What differs:
 *
 * - Every failed check faults, so a receipt with `success: true` means the change happened.
 * - Admin is set by the `0xFD` init frame (deploy with {@link buildReferenceNftV4DeployMetaTx}); there is no
 *   lazy admin and no lazy mint through `transfer_nft`.
 * - Every state change emits a log (see the `REF_NFT_V4_TOPIC_*` constants and {@link decodeReferenceNftV4Log}).
 * - Approvals (`approve`, `set_approval_for_all`), `transfer_admin`, metadata freezing and collection reads.
 * - Metadata is admin-only after mint.
 */
import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';
import { calldataAccountIdWord, calldataFixedWord32, calldataSelectorLastByte } from './calldata.js';
import { buildContractDeployMetaTx } from './canonicalDeployArtifacts.js';
import { DEFAULT_REFERENCE_NFT_V4_DEPLOY_BYTECODE_HEX, DEFAULT_REFERENCE_NFT_V4_RUNTIME_BYTECODE_HEX, } from './defaultReferenceNftV4BytecodeHex.js';
import { referenceNftMetadataStorageKey, referenceNftOwnerStorageKey } from './referenceNft.js';
export const SELECTOR_NFT_APPROVE = 0x07;
export const SELECTOR_NFT_SET_APPROVAL_FOR_ALL = 0x08;
export const SELECTOR_NFT_GET_APPROVED = 0x09;
export const SELECTOR_NFT_IS_APPROVED_FOR_ALL = 0x0a;
export const SELECTOR_NFT_ADMIN = 0x0b;
export const SELECTOR_NFT_TRANSFER_ADMIN = 0x0c;
export const SELECTOR_NFT_FREEZE_METADATA = 0x0d;
export const SELECTOR_NFT_FREEZE_ALL_METADATA = 0x0e;
export const SELECTOR_NFT_METADATA_HASH = 0x0f;
export const SELECTOR_NFT_TOTAL_MINTED = 0x10;
export const SELECTOR_NFT_IS_METADATA_FROZEN = 0x11;
/** Max tokens per v4 `mint_batch` (same as v3). */
export const MAX_REFERENCE_NFT_V4_MINT_BATCH = 500;
/** Token ids carried by one `MINT_BATCH` log (500 ids → 16 logs). */
export const REF_NFT_V4_IDS_PER_MINT_LOG = 32;
export const REFERENCE_NFT_V4_ARTIFACT_ID = 'boing.reference_nft_collection.v4';
export const REFERENCE_NFT_V4_TEMPLATE_VERSION = '4';
function tag32(ascii) {
    const out = new Uint8Array(32);
    for (let i = 0; i < ascii.length; i++)
        out[i] = ascii.charCodeAt(i);
    return bytesToHex(out);
}
/** Per-token approval slot tag: key = `token_id ^ tag`. */
export const REF_NFT_V4_APPROVED_XOR_HEX = tag32('BOING_REFNFT_APPROVED_V4');
/** Per-token metadata freeze slot tag (value 1 = frozen). */
export const REF_NFT_V4_FROZEN_XOR_HEX = tag32('BOING_REFNFT_FROZEN_V4');
/** Operator slot tag: key = `(owner[0..16] || operator[0..16]) ^ tag` (value 1 = approved for all). */
export const REF_NFT_V4_OPERATOR_XOR_HEX = tag32('BOING_REFNFT_OPERATOR_V4');
/** `LOG3 [topic, from, to]`, data `token_id`. */
export const REF_NFT_V4_TOPIC_TRANSFER_HEX = tag32('BOING_REFNFT_TRANSFER_V4');
/** `LOG2 [topic, to]`, data = up to 32 minted `token_id` words. */
export const REF_NFT_V4_TOPIC_MINT_BATCH_HEX = tag32('BOING_REFNFT_MINT_BATCH_V4');
/** `LOG3 [topic, owner, spender]`, data `token_id` (spender 0 = cleared). */
export const REF_NFT_V4_TOPIC_APPROVAL_HEX = tag32('BOING_REFNFT_APPROVAL_V4');
/** `LOG3 [topic, owner, operator]`, data 0/1 word. */
export const REF_NFT_V4_TOPIC_APPROVAL_FOR_ALL_HEX = tag32('BOING_REFNFT_APPROVAL_ALL_V4');
/** `LOG2 [topic, token_id]`, data `metadata_hash`. */
export const REF_NFT_V4_TOPIC_METADATA_HEX = tag32('BOING_REFNFT_METADATA_V4');
/** `LOG2 [topic, token_id]`, no data (`token_id = 0` means the whole collection). */
export const REF_NFT_V4_TOPIC_FREEZE_HEX = tag32('BOING_REFNFT_FREEZE_V4');
/** `LOG3 [topic, old_admin, new_admin]`, no data. */
export const REF_NFT_V4_TOPIC_ADMIN_HEX = tag32('BOING_REFNFT_ADMIN_V4');
function singletonKey(low) {
    const k = new Uint8Array(32);
    k[31] = low;
    return bytesToHex(k);
}
/** Admin slot (same singleton as v1–v3; written by the init frame). */
export const REF_NFT_V4_ADMIN_KEY_HEX = singletonKey(0xe0);
/** Collection-wide freeze flag slot. */
export const REF_NFT_V4_FROZEN_ALL_KEY_HEX = singletonKey(0xe1);
/** Total-minted counter slot (u64 word). */
export const REF_NFT_V4_TOTAL_MINTED_KEY_HEX = singletonKey(0xe2);
function xor32(a, maskHex) {
    const m = hexToBytes(maskHex);
    const out = new Uint8Array(32);
    for (let i = 0; i < 32; i++)
        out[i] = a[i] ^ m[i];
    return bytesToHex(out);
}
const word = (hex32) => hexToBytes(validateHex32(hex32));
/** Approval slot for `tokenId`. */
export function referenceNftV4ApprovedStorageKey(tokenIdHex32) {
    return xor32(word(tokenIdHex32), REF_NFT_V4_APPROVED_XOR_HEX);
}
/** Per-token metadata freeze slot for `tokenId`. */
export function referenceNftV4FrozenStorageKey(tokenIdHex32) {
    return xor32(word(tokenIdHex32), REF_NFT_V4_FROZEN_XOR_HEX);
}
/** Operator slot for `(owner, operator)`. */
export function referenceNftV4OperatorStorageKey(ownerHex32, operatorHex32) {
    const raw = new Uint8Array(32);
    raw.set(word(ownerHex32).subarray(0, 16), 0);
    raw.set(word(operatorHex32).subarray(0, 16), 16);
    return xor32(raw, REF_NFT_V4_OPERATOR_XOR_HEX);
}
function concat(parts) {
    const out = new Uint8Array(parts.reduce((s, p) => s + p.length, 0));
    let o = 0;
    for (const p of parts) {
        out.set(p, o);
        o += p.length;
    }
    return out;
}
const sel = (s) => calldataSelectorLastByte(s);
const flagWord = (b) => {
    const w = new Uint8Array(32);
    w[31] = b ? 1 : 0;
    return w;
};
/** 96-byte `approve(spender, tokenId)`; spender = zero account clears the approval. */
export function encodeReferenceNftApproveCalldata(spenderHex32, tokenIdHex32) {
    return concat([sel(SELECTOR_NFT_APPROVE), calldataAccountIdWord(spenderHex32), calldataFixedWord32(tokenIdHex32)]);
}
/** 96-byte `set_approval_for_all(operator, approved)`. */
export function encodeReferenceNftSetApprovalForAllCalldata(operatorHex32, approved) {
    return concat([sel(SELECTOR_NFT_SET_APPROVAL_FOR_ALL), calldataAccountIdWord(operatorHex32), flagWord(approved)]);
}
/** 64-byte `get_approved(tokenId)`. */
export function encodeReferenceNftGetApprovedCalldata(tokenIdHex32) {
    return concat([sel(SELECTOR_NFT_GET_APPROVED), calldataFixedWord32(tokenIdHex32)]);
}
/** 96-byte `is_approved_for_all(owner, operator)`. */
export function encodeReferenceNftIsApprovedForAllCalldata(ownerHex32, operatorHex32) {
    return concat([
        sel(SELECTOR_NFT_IS_APPROVED_FOR_ALL),
        calldataAccountIdWord(ownerHex32),
        calldataAccountIdWord(operatorHex32),
    ]);
}
/** 32-byte `admin()`. */
export function encodeReferenceNftAdminCalldata() {
    return sel(SELECTOR_NFT_ADMIN);
}
/** 64-byte `transfer_admin(newAdmin)` (admin only; newAdmin must be non-zero). */
export function encodeReferenceNftTransferAdminCalldata(newAdminHex32) {
    return concat([sel(SELECTOR_NFT_TRANSFER_ADMIN), calldataAccountIdWord(newAdminHex32)]);
}
/** 64-byte `freeze_metadata(tokenId)` (admin only). */
export function encodeReferenceNftFreezeMetadataCalldata(tokenIdHex32) {
    return concat([sel(SELECTOR_NFT_FREEZE_METADATA), calldataFixedWord32(tokenIdHex32)]);
}
/** 32-byte `freeze_all_metadata()` (admin only; irreversible). */
export function encodeReferenceNftFreezeAllMetadataCalldata() {
    return sel(SELECTOR_NFT_FREEZE_ALL_METADATA);
}
/** 64-byte `metadata_hash(tokenId)`. */
export function encodeReferenceNftMetadataHashCalldata(tokenIdHex32) {
    return concat([sel(SELECTOR_NFT_METADATA_HASH), calldataFixedWord32(tokenIdHex32)]);
}
/** 32-byte `total_minted()`. */
export function encodeReferenceNftTotalMintedCalldata() {
    return sel(SELECTOR_NFT_TOTAL_MINTED);
}
/** 64-byte `is_metadata_frozen(tokenId)`. */
export function encodeReferenceNftIsMetadataFrozenCalldata(tokenIdHex32) {
    return concat([sel(SELECTOR_NFT_IS_METADATA_FROZEN), calldataFixedWord32(tokenIdHex32)]);
}
const ZERO32 = '0x' + '00'.repeat(32);
function toBytes(data) {
    if (data instanceof Uint8Array)
        return data;
    const t = String(data ?? '').trim();
    if (t === '' || t === '0x' || t === '0X')
        return new Uint8Array(0);
    try {
        return hexToBytes(/^0x/i.test(t) ? t : `0x${t}`);
    }
    catch {
        return null;
    }
}
function topicHex(t) {
    try {
        return validateHex32(t).toLowerCase();
    }
    catch {
        return null;
    }
}
/**
 * Decode one `{ topics, data }` receipt log emitted by the v4 template. Returns null for other logs.
 *
 * Logs carry no emitter address on Boing today, so callers must only apply these to transactions whose
 * top-level target is a known v4 collection ({@link isReferenceNftV4Bytecode}). v4 makes no nested calls,
 * so every log in such a receipt came from that collection.
 */
export function decodeReferenceNftV4Log(log) {
    const topics = log.topics.map(topicHex);
    if (topics.length < 2 || topics.some((t) => t == null))
        return null;
    const [t0, t1, t2] = topics;
    const data = toBytes(log.data);
    if (!data)
        return null;
    const dataWord = data.length === 32 ? bytesToHex(data).toLowerCase() : null;
    switch (t0) {
        case REF_NFT_V4_TOPIC_TRANSFER_HEX:
            return topics.length === 3 && dataWord ? { kind: 'transfer', from: t1, to: t2, tokenId: dataWord } : null;
        case REF_NFT_V4_TOPIC_MINT_BATCH_HEX: {
            if (topics.length !== 2 || data.length === 0 || data.length % 32 !== 0)
                return null;
            if (data.length / 32 > REF_NFT_V4_IDS_PER_MINT_LOG)
                return null;
            const tokenIds = [];
            for (let o = 0; o < data.length; o += 32)
                tokenIds.push(bytesToHex(data.subarray(o, o + 32)).toLowerCase());
            return { kind: 'mint_batch', to: t1, tokenIds };
        }
        case REF_NFT_V4_TOPIC_APPROVAL_HEX:
            return topics.length === 3 && dataWord ? { kind: 'approval', owner: t1, spender: t2, tokenId: dataWord } : null;
        case REF_NFT_V4_TOPIC_APPROVAL_FOR_ALL_HEX: {
            if (topics.length !== 3 || !dataWord)
                return null;
            if (dataWord !== ZERO32 && dataWord !== '0x' + '00'.repeat(31) + '01')
                return null;
            return { kind: 'approval_for_all', owner: t1, operator: t2, approved: dataWord !== ZERO32 };
        }
        case REF_NFT_V4_TOPIC_METADATA_HEX:
            return topics.length === 2 && dataWord ? { kind: 'metadata', tokenId: t1, metadataHash: dataWord } : null;
        case REF_NFT_V4_TOPIC_FREEZE_HEX:
            return topics.length === 2 && data.length === 0 ? { kind: 'freeze', tokenId: t1, all: t1 === ZERO32 } : null;
        case REF_NFT_V4_TOPIC_ADMIN_HEX:
            return topics.length === 3 && data.length === 0 ? { kind: 'admin', oldAdmin: t1, newAdmin: t2 } : null;
        default:
            return null;
    }
}
/** Ownership moves (mints have `from` = zero account) from a v4 receipt's logs, in order. */
export function referenceNftV4OwnershipChanges(logs) {
    const out = [];
    for (const l of logs) {
        const d = decodeReferenceNftV4Log(l);
        if (d?.kind === 'transfer')
            out.push({ tokenId: d.tokenId, from: d.from, to: d.to });
        else if (d?.kind === 'mint_batch')
            for (const id of d.tokenIds)
                out.push({ tokenId: id, from: ZERO32, to: d.to });
    }
    return out;
}
function storageWord(v) {
    let raw = v;
    if (raw && typeof raw === 'object' && 'value' in raw)
        raw = raw.value;
    return typeof raw === 'string' && /^0x[0-9a-f]{64}$/i.test(raw) ? raw.toLowerCase() : ZERO32;
}
/** Read one token's owner, approval, metadata hash and freeze state through `boing_getContractStorage`. */
export async function fetchReferenceNftV4Token(client, contractHex32, tokenIdHex32) {
    const tokenId = validateHex32(tokenIdHex32).toLowerCase();
    const [owner, approved, meta, frozen, frozenAll] = await Promise.all([
        client.getContractStorage(contractHex32, referenceNftOwnerStorageKey(tokenId)),
        client.getContractStorage(contractHex32, referenceNftV4ApprovedStorageKey(tokenId)),
        client.getContractStorage(contractHex32, referenceNftMetadataStorageKey(tokenId)),
        client.getContractStorage(contractHex32, referenceNftV4FrozenStorageKey(tokenId)),
        client.getContractStorage(contractHex32, REF_NFT_V4_FROZEN_ALL_KEY_HEX),
    ]);
    return {
        tokenId,
        owner: storageWord(owner),
        approved: storageWord(approved),
        metadataHash: storageWord(meta),
        metadataFrozen: storageWord(frozen) !== ZERO32 || storageWord(frozenAll) !== ZERO32,
    };
}
/** Read the collection admin, total minted and the collection-wide freeze flag. */
export async function fetchReferenceNftV4CollectionInfo(client, contractHex32) {
    const [admin, total, frozenAll] = await Promise.all([
        client.getContractStorage(contractHex32, REF_NFT_V4_ADMIN_KEY_HEX),
        client.getContractStorage(contractHex32, REF_NFT_V4_TOTAL_MINTED_KEY_HEX),
        client.getContractStorage(contractHex32, REF_NFT_V4_FROZEN_ALL_KEY_HEX),
    ]);
    return {
        admin: storageWord(admin),
        totalMinted: BigInt(storageWord(total)),
        allMetadataFrozen: storageWord(frozenAll) !== ZERO32,
    };
}
/** True when `operator` is approved for all of `owner`'s tokens. */
export async function fetchReferenceNftV4IsApprovedForAll(client, contractHex32, ownerHex32, operatorHex32) {
    const w = await client.getContractStorage(contractHex32, referenceNftV4OperatorStorageKey(ownerHex32, operatorHex32));
    return storageWord(w) !== ZERO32;
}
// --- Identity + deploy ---------------------------------------------------------------------------
const norm = (h) => h.trim().toLowerCase().replace(/^0x/, '');
/** True when `codeHex` is the pinned v4 runtime (or deploy payload). */
export function isReferenceNftV4Bytecode(codeHex) {
    const c = norm(codeHex);
    return c === norm(DEFAULT_REFERENCE_NFT_V4_RUNTIME_BYTECODE_HEX) || c === norm(DEFAULT_REFERENCE_NFT_V4_DEPLOY_BYTECODE_HEX);
}
/** Express `contract_deploy_meta` tx for the pinned v4 collection (`0xFD` init; admin = deployer; purpose `nft`). */
export function buildReferenceNftV4DeployMetaTx(input) {
    return buildContractDeployMetaTx({
        bytecodeHex: DEFAULT_REFERENCE_NFT_V4_DEPLOY_BYTECODE_HEX,
        assetName: input.name,
        assetSymbol: input.symbol,
        purposeCategory: 'nft',
        descriptionHashHex: input.descriptionHashHex,
    });
}
export { DEFAULT_REFERENCE_NFT_V4_DEPLOY_BYTECODE_HEX, DEFAULT_REFERENCE_NFT_V4_RUNTIME_BYTECODE_HEX };
