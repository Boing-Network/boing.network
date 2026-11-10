/**
 * Reference **multi-token** (ERC-1155-style shared item types) — calldata, storage keys, log decoding and
 * balance reads. Mirrors `crates/boing-execution/src/reference_multi_token.rs`; spec:
 * `docs/BOING-REFERENCE-MULTI-TOKEN.md`.
 *
 * One contract holds many item types; every holder of item `7` holds the same item. Item ids are u64,
 * amounts are u128. No hashing opcode is needed: slot keys are XOR-tagged words.
 */
import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';
import { calldataAccountIdWord, calldataFixedWord32, calldataSelectorLastByte } from './calldata.js';
import { buildContractDeployMetaTx } from './canonicalDeployArtifacts.js';
import { DEFAULT_REFERENCE_MULTI_TOKEN_DEPLOY_BYTECODE_HEX, DEFAULT_REFERENCE_MULTI_TOKEN_RUNTIME_BYTECODE_HEX, } from './defaultReferenceMultiTokenBytecodeHex.js';
export const SELECTOR_MT_BALANCE_OF = 0x20;
export const SELECTOR_MT_TRANSFER = 0x21;
export const SELECTOR_MT_MINT = 0x22;
export const SELECTOR_MT_SET_ITEM_METADATA = 0x23;
export const SELECTOR_MT_MINT_BATCH = 0x24;
export const SELECTOR_MT_TRANSFER_BATCH = 0x25;
export const SELECTOR_MT_TOTAL_SUPPLY = 0x26;
export const SELECTOR_MT_ITEM_METADATA = 0x27;
export const SELECTOR_MT_ADMIN = 0x28;
export const SELECTOR_MT_TRANSFER_ADMIN = 0x29;
/** Max entries per `mint_batch` / `transfer_batch` (one log per entry; VM cap 24 logs per tx). */
export const MAX_REFERENCE_MULTI_TOKEN_BATCH = 16;
/** Logical artifact id + pinned template version. */
export const REFERENCE_MULTI_TOKEN_ARTIFACT_ID = 'boing.reference_multi_token.v1';
export const REFERENCE_MULTI_TOKEN_TEMPLATE_VERSION = '1';
function tag32(ascii) {
    const out = new Uint8Array(32);
    for (let i = 0; i < ascii.length; i++)
        out[i] = ascii.charCodeAt(i);
    return bytesToHex(out);
}
/** Balance slot XOR tag (`BOING_REFMT_BAL_V1`). */
export const REF_MT_BALANCE_XOR_HEX = tag32('BOING_REFMT_BAL_V1');
/** Total-supply slot XOR tag (`BOING_REFMT_SUPPLY_V1`). */
export const REF_MT_SUPPLY_XOR_HEX = tag32('BOING_REFMT_SUPPLY_V1');
/** Item metadata slot XOR tag (`BOING_REFMT_META_V1`). */
export const REF_MT_METADATA_XOR_HEX = tag32('BOING_REFMT_META_V1');
/** `LOG3` topic0 for balance moves: `[topic, from, to]`, data `item_word || amount_word` (mint: from = 0). */
export const REF_MT_TOPIC_TRANSFER_HEX = tag32('BOING_REFMT_TRANSFER_V1');
/** `LOG2` topic0 for `set_item_metadata`: `[topic, item_word]`, data = hash. */
export const REF_MT_TOPIC_METADATA_HEX = tag32('BOING_REFMT_METADATA_V1');
/** `LOG3` topic0 for `transfer_admin`: `[topic, old_admin, new_admin]`. */
export const REF_MT_TOPIC_ADMIN_HEX = tag32('BOING_REFMT_ADMIN_V1');
/** Admin slot (set to the deployer by the init frame). */
export const REF_MT_ADMIN_KEY_HEX = bytesToHex((() => {
    const k = new Uint8Array(32);
    k[31] = 0xe0;
    return k;
})());
const U64_MAX = (1n << 64n) - 1n;
const U128_MAX = (1n << 128n) - 1n;
function toU64(v, what) {
    const n = BigInt(v);
    if (n < 0n || n > U64_MAX)
        throw new RangeError(`${what} must be a u64`);
    return n;
}
function toU128(v, what) {
    const n = BigInt(v);
    if (n < 0n || n > U128_MAX)
        throw new RangeError(`${what} must be a u128`);
    return n;
}
function beWord(n) {
    const out = new Uint8Array(32);
    let x = n;
    for (let i = 31; i >= 0 && x > 0n; i--) {
        out[i] = Number(x & 0xffn);
        x >>= 8n;
    }
    return out;
}
function wordToBigInt(w) {
    let n = 0n;
    for (const b of w)
        n = (n << 8n) | BigInt(b);
    return n;
}
function xor32(a, maskHex) {
    const m = hexToBytes(maskHex);
    const out = new Uint8Array(32);
    for (let i = 0; i < 32; i++)
        out[i] = a[i] ^ m[i];
    return bytesToHex(out);
}
/** Item id word (big-endian u64 in the low 8 bytes). */
export function multiTokenItemIdWord(itemId) {
    return bytesToHex(beWord(toU64(itemId, 'itemId')));
}
/** Amount word (big-endian u128 in the low 16 bytes). */
export function multiTokenAmountWord(amount) {
    return bytesToHex(beWord(toU128(amount, 'amount')));
}
/** `SLOAD` key for `balance[itemId][account]`: `(account[0..24] || itemId_be8) ^ REF_MT_BALANCE_XOR`. */
export function multiTokenBalanceStorageKey(accountHex32, itemId) {
    const acct = hexToBytes(validateHex32(accountHex32));
    const raw = new Uint8Array(32);
    raw.set(acct.subarray(0, 24), 0);
    raw.set(beWord(toU64(itemId, 'itemId')).subarray(24), 24);
    return xor32(raw, REF_MT_BALANCE_XOR_HEX);
}
/** `SLOAD` key for `total_supply[itemId]`. */
export function multiTokenSupplyStorageKey(itemId) {
    return xor32(beWord(toU64(itemId, 'itemId')), REF_MT_SUPPLY_XOR_HEX);
}
/** `SLOAD` key for `item_metadata[itemId]`. */
export function multiTokenMetadataStorageKey(itemId) {
    return xor32(beWord(toU64(itemId, 'itemId')), REF_MT_METADATA_XOR_HEX);
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
const itemW = (id) => beWord(toU64(id, 'itemId'));
const amtW = (a) => beWord(toU128(a, 'amount'));
/** 96-byte `balance_of(account, itemId)`. */
export function encodeMultiTokenBalanceOfCalldata(accountHex32, itemId) {
    return concat([sel(SELECTOR_MT_BALANCE_OF), calldataAccountIdWord(accountHex32), itemW(itemId)]);
}
/** 128-byte `transfer(to, itemId, amount)` (caller's own balance). */
export function encodeMultiTokenTransferCalldata(toHex32, itemId, amount) {
    return concat([sel(SELECTOR_MT_TRANSFER), calldataAccountIdWord(toHex32), itemW(itemId), amtW(amount)]);
}
/** 128-byte `mint(to, itemId, amount)` (admin only). */
export function encodeMultiTokenMintCalldata(toHex32, itemId, amount) {
    return concat([sel(SELECTOR_MT_MINT), calldataAccountIdWord(toHex32), itemW(itemId), amtW(amount)]);
}
/** 96-byte `set_item_metadata(itemId, hash)` (admin only). */
export function encodeMultiTokenSetItemMetadataCalldata(itemId, hashHex32) {
    return concat([sel(SELECTOR_MT_SET_ITEM_METADATA), itemW(itemId), calldataFixedWord32(hashHex32)]);
}
function encodeBatch(selector, toHex32, entries) {
    const n = entries.length;
    if (n < 1 || n > MAX_REFERENCE_MULTI_TOKEN_BATCH) {
        throw new RangeError(`batch n must be 1..=${MAX_REFERENCE_MULTI_TOKEN_BATCH}, got ${n}`);
    }
    return concat([
        sel(selector),
        calldataAccountIdWord(toHex32),
        beWord(BigInt(n)),
        ...entries.map((e) => itemW(e.itemId)),
        ...entries.map((e) => amtW(e.amount)),
    ]);
}
/** `mint_batch(to, n, itemIds[n], amounts[n])` — `96 + 64n` bytes (admin only). */
export function encodeMultiTokenMintBatchCalldata(toHex32, entries) {
    return encodeBatch(SELECTOR_MT_MINT_BATCH, toHex32, entries);
}
/** `transfer_batch(to, n, itemIds[n], amounts[n])` — `96 + 64n` bytes (caller's balances). */
export function encodeMultiTokenTransferBatchCalldata(toHex32, entries) {
    return encodeBatch(SELECTOR_MT_TRANSFER_BATCH, toHex32, entries);
}
/** 64-byte `total_supply(itemId)`. */
export function encodeMultiTokenTotalSupplyCalldata(itemId) {
    return concat([sel(SELECTOR_MT_TOTAL_SUPPLY), itemW(itemId)]);
}
/** 64-byte `item_metadata(itemId)`. */
export function encodeMultiTokenItemMetadataCalldata(itemId) {
    return concat([sel(SELECTOR_MT_ITEM_METADATA), itemW(itemId)]);
}
/** 64-byte `transfer_admin(newAdmin)` (admin only). */
export function encodeMultiTokenTransferAdminCalldata(newAdminHex32) {
    return concat([sel(SELECTOR_MT_TRANSFER_ADMIN), calldataAccountIdWord(newAdminHex32)]);
}
function toBytes(calldata) {
    if (calldata instanceof Uint8Array)
        return calldata;
    const t = String(calldata).trim();
    if (!t)
        return null;
    try {
        return hexToBytes(/^0x/i.test(t) ? t : `0x${t}`);
    }
    catch {
        return null;
    }
}
function u64At(b, off) {
    const w = b.subarray(off, off + 32);
    if (w.length !== 32)
        return null;
    for (let i = 0; i < 24; i++)
        if (w[i] !== 0)
            return null;
    return wordToBigInt(w);
}
function u128At(b, off) {
    const w = b.subarray(off, off + 32);
    if (w.length !== 32)
        return null;
    for (let i = 0; i < 16; i++)
        if (w[i] !== 0)
            return null;
    return wordToBigInt(w);
}
const wordHexAt = (b, off) => bytesToHex(b.subarray(off, off + 32));
/**
 * Best-effort decode of reference multi-token calldata (wallet approval copy, explorers). Returns null when the
 * selector or layout does not match. Values that the contract would reject (u64/u128 overflow) also return null.
 */
export function decodeMultiTokenCalldata(calldata) {
    const b = toBytes(calldata);
    if (!b || b.length < 32)
        return null;
    for (let i = 0; i < 31; i++)
        if (b[i] !== 0)
            return null;
    const s = b[31];
    switch (s) {
        case SELECTOR_MT_BALANCE_OF: {
            const itemId = b.length >= 96 ? u64At(b, 64) : null;
            return itemId == null ? null : { selector: s, kind: 'balance_of', account: wordHexAt(b, 32), itemId };
        }
        case SELECTOR_MT_TRANSFER:
        case SELECTOR_MT_MINT: {
            if (b.length < 128)
                return null;
            const itemId = u64At(b, 64);
            const amount = u128At(b, 96);
            if (itemId == null || amount == null)
                return null;
            return s === SELECTOR_MT_MINT
                ? { selector: s, kind: 'mint', to: wordHexAt(b, 32), itemId, amount }
                : { selector: s, kind: 'transfer', to: wordHexAt(b, 32), itemId, amount };
        }
        case SELECTOR_MT_SET_ITEM_METADATA: {
            const itemId = b.length >= 96 ? u64At(b, 32) : null;
            return itemId == null ? null : { selector: s, kind: 'set_item_metadata', itemId, metadataHash: wordHexAt(b, 64) };
        }
        case SELECTOR_MT_MINT_BATCH:
        case SELECTOR_MT_TRANSFER_BATCH: {
            if (b.length < 96)
                return null;
            const n = u64At(b, 64);
            if (n == null || n < 1n || n > BigInt(MAX_REFERENCE_MULTI_TOKEN_BATCH))
                return null;
            const count = Number(n);
            const end = 96 + 64 * count;
            if (b.length < end)
                return null;
            for (let i = end; i < b.length; i++)
                if (b[i] !== 0)
                    return null;
            const entries = [];
            for (let i = 0; i < count; i++) {
                const itemId = u64At(b, 96 + 32 * i);
                const amount = u128At(b, 96 + 32 * (count + i));
                if (itemId == null || amount == null)
                    return null;
                entries.push({ itemId, amount });
            }
            return {
                selector: s,
                kind: s === SELECTOR_MT_MINT_BATCH ? 'mint_batch' : 'transfer_batch',
                to: wordHexAt(b, 32),
                entries,
            };
        }
        case SELECTOR_MT_TOTAL_SUPPLY:
        case SELECTOR_MT_ITEM_METADATA: {
            const itemId = b.length >= 64 ? u64At(b, 32) : null;
            if (itemId == null)
                return null;
            return s === SELECTOR_MT_TOTAL_SUPPLY
                ? { selector: s, kind: 'total_supply', itemId }
                : { selector: s, kind: 'item_metadata', itemId };
        }
        case SELECTOR_MT_ADMIN:
            return { selector: s, kind: 'admin' };
        case SELECTOR_MT_TRANSFER_ADMIN:
            return b.length >= 64 ? { selector: s, kind: 'transfer_admin', newAdmin: wordHexAt(b, 32) } : null;
        default:
            return null;
    }
}
const ZERO32 = '0x' + '00'.repeat(32);
/** Decode a `{ topics, data }` receipt log emitted by the template; null for other logs. */
export function decodeMultiTokenTransferLog(log) {
    if (log.topics.length !== 3)
        return null;
    if (log.topics[0].toLowerCase() !== REF_MT_TOPIC_TRANSFER_HEX.toLowerCase())
        return null;
    const data = toBytes(log.data);
    if (!data || data.length !== 64)
        return null;
    const itemId = u64At(data, 0);
    const amount = u128At(data, 32);
    if (itemId == null || amount == null)
        return null;
    const from = validateHex32(log.topics[1]).toLowerCase();
    return { from, to: validateHex32(log.topics[2]).toLowerCase(), itemId, amount, isMint: from === ZERO32 };
}
/** Parse a `boing_getContractStorage` result (`{ value }` or bare hex) as a u128 amount. */
export function multiTokenAmountFromStorageWord(word) {
    let raw = word;
    if (raw && typeof raw === 'object' && 'value' in raw)
        raw = raw.value;
    if (typeof raw !== 'string')
        return 0n;
    const b = toBytes(raw);
    if (!b || b.length !== 32)
        return 0n;
    return wordToBigInt(b);
}
/** Read `balance[itemId][account]` via `boing_getContractStorage` (no tx, no simulation). */
export async function fetchMultiTokenBalance(client, contractHex32, accountHex32, itemId) {
    return multiTokenAmountFromStorageWord(await client.getContractStorage(contractHex32, multiTokenBalanceStorageKey(accountHex32, itemId)));
}
/** Read balances for several item ids in parallel. */
export async function fetchMultiTokenBalances(client, contractHex32, accountHex32, itemIds) {
    return Promise.all(itemIds.map(async (id) => ({
        itemId: BigInt(id),
        balance: await fetchMultiTokenBalance(client, contractHex32, accountHex32, id),
    })));
}
/** Read `total_supply[itemId]` and the type-level metadata hash. */
export async function fetchMultiTokenItemInfo(client, contractHex32, itemId) {
    const [s, m] = await Promise.all([
        client.getContractStorage(contractHex32, multiTokenSupplyStorageKey(itemId)),
        client.getContractStorage(contractHex32, multiTokenMetadataStorageKey(itemId)),
    ]);
    let meta = m;
    if (meta && typeof meta === 'object' && 'value' in meta)
        meta = meta.value;
    const metadataHash = typeof meta === 'string' && /^0x[0-9a-f]{64}$/i.test(meta) ? meta.toLowerCase() : ZERO32;
    return { itemId: BigInt(itemId), totalSupply: multiTokenAmountFromStorageWord(s), metadataHash };
}
const norm = (h) => h.trim().toLowerCase().replace(/^0x/, '');
/** True when `codeHex` is the pinned multi-token runtime (or deploy payload) — wallet / explorer identity. */
export function isReferenceMultiTokenBytecode(codeHex) {
    const c = norm(codeHex);
    return (c === norm(DEFAULT_REFERENCE_MULTI_TOKEN_RUNTIME_BYTECODE_HEX) ||
        c === norm(DEFAULT_REFERENCE_MULTI_TOKEN_DEPLOY_BYTECODE_HEX));
}
/** Express `contract_deploy_meta` tx for the pinned multi-token (`0xFD` init; admin = deployer; purpose `nft`). */
export function buildReferenceMultiTokenDeployMetaTx(input) {
    return buildContractDeployMetaTx({
        bytecodeHex: DEFAULT_REFERENCE_MULTI_TOKEN_DEPLOY_BYTECODE_HEX,
        assetName: input.name,
        assetSymbol: input.symbol,
        purposeCategory: 'nft',
        descriptionHashHex: input.descriptionHashHex,
    });
}
export { DEFAULT_REFERENCE_MULTI_TOKEN_DEPLOY_BYTECODE_HEX, DEFAULT_REFERENCE_MULTI_TOKEN_RUNTIME_BYTECODE_HEX };
