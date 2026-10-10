/**
 * Off-chain **cache helpers** for linked NFT collection ↔ fungible token pairs.
 *
 * **Source of truth** is the on-chain registry (`linkedNftTokenRegistry.ts` /
 * `boing_execution::linked_nft_token_registry`). Schema **`boing.linked_nft_token.v1`** and
 * companion JSON keys are optional indexer/UI caches only — not authoritative.
 *
 * See `docs/BOING-LINKED-NFT-TOKEN.md`.
 */
import { blake3 } from '@noble/hashes/blake3';
import { bytesToHex as nobleBytesToHex } from '@noble/hashes/utils';
import { buildReferenceFungibleDeployMetaTx, buildReferenceFungibleSecuredDeployMetaTx, buildReferenceNftCollectionDeployMetaTx, resolveReferenceFungibleSecuredTemplateBytecodeHex, resolveReferenceFungibleTemplateBytecodeHex, resolveReferenceNftCollectionTemplateBytecodeHex, } from './canonicalDeployArtifacts.js';
import { predictCreate2ContractAddress } from './create2.js';
import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';
/** Canonical schema id committed into `description_hash` / off-chain JSON. */
export const LINKED_NFT_TOKEN_SCHEMA = 'boing.linked_nft_token.v1';
/** Domain label for derived CREATE2 salt (collection salt → token salt). */
export const LINKED_NFT_TOKEN_SALT_DOMAIN = 'boing.nft_token_pair.v1';
/** Full schema object key in off-chain NFT / project JSON. */
export const LINKED_NFT_TOKEN_OFFCHAIN_KEY = 'linked_nft_token';
/** Convenience peer list on NFT / project metadata (AccountId hex strings). */
export const COMPANION_TOKENS_OFFCHAIN_KEY = 'companion_tokens';
/** Convenience peer list on fungible / project metadata (AccountId hex strings). */
export const COMPANION_COLLECTIONS_OFFCHAIN_KEY = 'companion_collections';
function normalizeAccountIdHex(hex) {
    if (hex === undefined || hex === null)
        return '';
    const t = String(hex).trim();
    if (!t)
        return '';
    // Accept `0x` / `0X` / bare hex; validateHex32's ensureHex is case-sensitive on the prefix.
    const raw = t.replace(/^0x/i, '');
    return validateHex32(`0x${raw}`).toLowerCase();
}
function normalizePeerList(peers) {
    if (!peers?.length)
        return [];
    const set = new Set();
    for (const p of peers) {
        const n = normalizeAccountIdHex(p);
        if (n)
            set.add(n);
    }
    return [...set].sort();
}
/**
 * Stable object for hashing / logging (explicit key order).
 * Peers are lowercased, deduped, and sorted.
 */
export function normalizeLinkedNftToken(input) {
    if (input.role !== 'nft_collection' && input.role !== 'fungible_token') {
        throw new Error('normalizeLinkedNftToken: role must be nft_collection or fungible_token');
    }
    const revision = typeof input.revision === 'number' && Number.isFinite(input.revision)
        ? Math.max(0, Math.floor(input.revision))
        : 0;
    return {
        schema: LINKED_NFT_TOKEN_SCHEMA,
        role: input.role,
        self: normalizeAccountIdHex(input.self),
        peers: normalizePeerList(input.peers),
        attester: normalizeAccountIdHex(input.attester),
        revision,
        note: typeof input.note === 'string' ? input.note.trim() : '',
    };
}
/** UTF-8 JSON of the normalized document (canonical key order). */
export function encodeLinkedNftTokenJson(input) {
    return JSON.stringify(normalizeLinkedNftToken(input));
}
/**
 * Decode a `boing.linked_nft_token.v1` JSON string or object.
 * @throws if schema/role invalid or peer ids malformed.
 */
export function decodeLinkedNftTokenJson(json) {
    let obj;
    if (typeof json === 'string') {
        try {
            obj = JSON.parse(json);
        }
        catch (e) {
            throw new Error(`decodeLinkedNftTokenJson: invalid JSON (${e instanceof Error ? e.message : String(e)})`);
        }
    }
    else {
        obj = json;
    }
    if (!obj || typeof obj !== 'object' || Array.isArray(obj)) {
        throw new Error('decodeLinkedNftTokenJson: expected object');
    }
    const o = obj;
    if (o.schema !== LINKED_NFT_TOKEN_SCHEMA) {
        throw new Error(`decodeLinkedNftTokenJson: expected schema ${LINKED_NFT_TOKEN_SCHEMA}, got ${String(o.schema)}`);
    }
    if (o.role !== 'nft_collection' && o.role !== 'fungible_token') {
        throw new Error('decodeLinkedNftTokenJson: invalid role');
    }
    const peersRaw = Array.isArray(o.peers) ? o.peers.map((p) => String(p)) : [];
    return normalizeLinkedNftToken({
        role: o.role,
        self: typeof o.self === 'string' ? o.self : '',
        peers: peersRaw,
        attester: typeof o.attester === 'string' ? o.attester : '',
        revision: typeof o.revision === 'number' ? o.revision : 0,
        note: typeof o.note === 'string' ? o.note : '',
    });
}
/**
 * Blake3-256 digest of UTF-8 canonical JSON, as **`0x` + 64 hex** (fits `description_hash`).
 */
export function descriptionHashHexFromLinkedNftToken(input) {
    const json = encodeLinkedNftTokenJson(input);
    const digest = blake3(new TextEncoder().encode(json));
    const h = nobleBytesToHex(digest);
    if (h.length !== 64) {
        throw new Error('descriptionHashHexFromLinkedNftToken: unexpected digest length');
    }
    return `0x${h}`;
}
/** Attach one peer AccountId; increments `revision` by default. */
export function attachLinkedNftTokenPeer(doc, peerHex, opts) {
    const base = normalizeLinkedNftToken(doc);
    const peer = normalizeAccountIdHex(peerHex);
    if (!peer)
        throw new Error('attachLinkedNftTokenPeer: peer required');
    const peers = normalizePeerList([...base.peers, peer]);
    const bump = opts?.bumpRevision !== false;
    return normalizeLinkedNftToken({
        ...base,
        peers,
        revision: bump ? base.revision + 1 : base.revision,
    });
}
/** Replace the full peer list (many-to-many update). */
export function updateLinkedNftTokenPeers(doc, peers, opts) {
    const base = normalizeLinkedNftToken(doc);
    const bump = opts?.bumpRevision !== false;
    return normalizeLinkedNftToken({
        ...base,
        peers,
        revision: bump ? base.revision + 1 : base.revision,
    });
}
/** Remove one peer; no-op if absent (still bumps revision when `bumpRevision`). */
export function unlinkLinkedNftTokenPeer(doc, peerHex, opts) {
    const base = normalizeLinkedNftToken(doc);
    const peer = normalizeAccountIdHex(peerHex);
    if (!peer)
        throw new Error('unlinkLinkedNftTokenPeer: peer required');
    const peers = base.peers.filter((p) => p !== peer);
    const bump = opts?.bumpRevision !== false;
    return normalizeLinkedNftToken({
        ...base,
        peers,
        revision: bump ? base.revision + 1 : base.revision,
    });
}
/**
 * Build / merge off-chain JSON keys for display clients.
 * Writes full schema under {@link LINKED_NFT_TOKEN_OFFCHAIN_KEY} and convenience peer arrays.
 */
export function applyLinkedNftTokenOffchainKeys(metadata, doc) {
    const norm = normalizeLinkedNftToken(doc);
    const out = { ...metadata, [LINKED_NFT_TOKEN_OFFCHAIN_KEY]: norm };
    if (norm.role === 'nft_collection') {
        out[COMPANION_TOKENS_OFFCHAIN_KEY] = [...norm.peers];
    }
    else {
        out[COMPANION_COLLECTIONS_OFFCHAIN_KEY] = [...norm.peers];
    }
    return out;
}
/**
 * Read link document from off-chain metadata (full key preferred; falls back to convenience arrays).
 */
export function readLinkedNftTokenFromOffchainMetadata(metadata, roleHint) {
    const full = metadata[LINKED_NFT_TOKEN_OFFCHAIN_KEY];
    if (full && typeof full === 'object' && !Array.isArray(full)) {
        try {
            return decodeLinkedNftTokenJson(full);
        }
        catch {
            /* fall through */
        }
    }
    const tokens = metadata[COMPANION_TOKENS_OFFCHAIN_KEY];
    const collections = metadata[COMPANION_COLLECTIONS_OFFCHAIN_KEY];
    if (Array.isArray(tokens) && (roleHint === 'nft_collection' || roleHint === undefined)) {
        return normalizeLinkedNftToken({
            role: 'nft_collection',
            peers: tokens.map((p) => String(p)),
        });
    }
    if (Array.isArray(collections) && (roleHint === 'fungible_token' || roleHint === undefined)) {
        return normalizeLinkedNftToken({
            role: 'fungible_token',
            peers: collections.map((p) => String(p)),
        });
    }
    return null;
}
/** 32 random bytes as `0x` + 64 hex (collection CREATE2 salt). */
export function randomLinkedNftTokenCollectionSaltHex() {
    const bytes = new Uint8Array(32);
    if (typeof globalThis.crypto?.getRandomValues === 'function') {
        globalThis.crypto.getRandomValues(bytes);
    }
    else {
        for (let i = 0; i < 32; i++)
            bytes[i] = Math.floor(Math.random() * 256);
    }
    return validateHex32(bytesToHex(bytes));
}
/**
 * Token CREATE2 salt: `BLAKE3("boing.nft_token_pair.v1" ‖ collectionSalt32)`.
 */
export function deriveLinkedNftTokenFungibleSaltHex(collectionSaltHex) {
    const salt = hexToBytes(validateHex32(collectionSaltHex));
    const domain = new TextEncoder().encode(LINKED_NFT_TOKEN_SALT_DOMAIN);
    const preimage = new Uint8Array(domain.length + salt.length);
    preimage.set(domain, 0);
    preimage.set(salt, domain.length);
    const digest = blake3(preimage);
    return validateHex32(bytesToHex(digest));
}
export function linkedNftTokenPairSalts(collectionSaltHex) {
    const collection = (collectionSaltHex?.trim()
        ? validateHex32(collectionSaltHex)
        : randomLinkedNftTokenCollectionSaltHex());
    return {
        collectionSaltHex: collection,
        tokenSaltHex: deriveLinkedNftTokenFungibleSaltHex(collection),
    };
}
/**
 * Soft-gate for display-only links: prefer same deployer / attester.
 * Does **not** prove authenticity on-chain — document spoof risk to UI consumers.
 */
export function softGateLinkedNftTokenPeers(input) {
    const subject = normalizeAccountIdHex(input.subjectDeployerHex);
    const peer = normalizeAccountIdHex(input.peerDeployerHex);
    const attester = normalizeAccountIdHex(input.attesterHex);
    const requireMatch = Boolean(input.requireMatch);
    if (!subject && !peer && !attester) {
        return {
            ok: !requireMatch,
            spoofRisk: true,
            matched: false,
            reason: 'No deployer/attester provided; display-only link is spoofable without same-deployer soft-gate.',
        };
    }
    const refs = [subject, peer, attester].filter(Boolean);
    const allSame = refs.length > 0 && refs.every((r) => r === refs[0]);
    if (allSame && refs.length >= 2) {
        return {
            ok: true,
            spoofRisk: false,
            matched: true,
            reason: 'Deployer/attester AccountIds match (soft-gate preferred).',
        };
    }
    if (subject && peer && subject === peer) {
        return {
            ok: true,
            spoofRisk: false,
            matched: true,
            reason: 'Subject and peer deployers match.',
        };
    }
    if (attester && ((subject && attester === subject) || (peer && attester === peer))) {
        // Partial: attester matches one side only
        if (subject && peer && subject !== peer) {
            const msg = 'Attester matches one side but subject/peer deployers differ — spoof risk remains.';
            return { ok: !requireMatch, spoofRisk: true, matched: false, reason: msg };
        }
    }
    const msg = 'Deployer/attester mismatch or incomplete evidence; treat link as display-only with spoof risk.';
    return { ok: !requireMatch, spoofRisk: true, matched: false, reason: msg };
}
function bytecodeBytesForPredict(hex) {
    return hexToBytes(hex.startsWith('0x') || hex.startsWith('0X') ? hex : `0x${hex}`);
}
/**
 * Joint deploy helper: predict both CREATE2 addresses, emit **two** `contract_deploy_meta` txs
 * (NFT + fungible) with mutual `description_hash` commits and documented salts.
 *
 * Submit order is up to the dApp (either first); both hashes already name both addresses.
 * Handle partial success in UI (one of two mempool/QA deploys may land alone).
 */
export function buildLinkedNftTokenPairDeploys(input) {
    const deployer = validateHex32(input.deployerHex).toLowerCase();
    const salts = linkedNftTokenPairSalts(input.collectionSaltHex);
    const collectionBytecode = input.collectionBytecodeHexOverride?.trim()
        ? (input.collectionBytecodeHexOverride.startsWith('0x')
            ? input.collectionBytecodeHexOverride
            : `0x${input.collectionBytecodeHexOverride}`)
        : resolveReferenceNftCollectionTemplateBytecodeHex({
            extraEnvKeys: input.collectionExtraEnvKeys,
        });
    const tokenBytecode = input.tokenBytecodeHexOverride?.trim()
        ? (input.tokenBytecodeHexOverride.startsWith('0x')
            ? input.tokenBytecodeHexOverride
            : `0x${input.tokenBytecodeHexOverride}`)
        : input.securedFungible
            ? resolveReferenceFungibleSecuredTemplateBytecodeHex({
                extraEnvKeys: input.tokenExtraEnvKeys,
            })
            : resolveReferenceFungibleTemplateBytecodeHex({
                extraEnvKeys: input.tokenExtraEnvKeys,
            });
    // When secured + nativeTokenSecurity, deploy builder may rewrite bytecode; predict with the same path.
    let tokenBytecodeForPredict = tokenBytecode;
    let tokenDeployBase;
    if (input.securedFungible) {
        const securedIn = {
            assetName: input.tokenName,
            assetSymbol: input.tokenSymbol,
            purposeCategory: input.tokenPurposeCategory,
            nativeTokenSecurity: input.nativeTokenSecurity,
            chainContext: input.chainContext,
            mintFirstTotalSupplyWei: input.mintFirstTotalSupplyWei,
            bytecodeHexOverride: input.tokenBytecodeHexOverride,
            extraEnvKeys: input.tokenExtraEnvKeys,
        };
        tokenDeployBase = buildReferenceFungibleSecuredDeployMetaTx(securedIn);
        tokenBytecodeForPredict = tokenDeployBase.bytecode;
    }
    else {
        const fungibleIn = {
            assetName: input.tokenName,
            assetSymbol: input.tokenSymbol,
            purposeCategory: input.tokenPurposeCategory,
            nativeTokenSecurity: input.nativeTokenSecurity,
            bytecodeHexOverride: input.tokenBytecodeHexOverride,
            extraEnvKeys: input.tokenExtraEnvKeys,
        };
        tokenDeployBase = buildReferenceFungibleDeployMetaTx(fungibleIn);
        tokenBytecodeForPredict = tokenDeployBase.bytecode;
    }
    const nftIn = {
        collectionName: input.collectionName,
        collectionSymbol: input.collectionSymbol,
        purposeCategory: input.collectionPurposeCategory,
        bytecodeHexOverride: input.collectionBytecodeHexOverride,
        extraEnvKeys: input.collectionExtraEnvKeys,
    };
    const collectionDeployBase = buildReferenceNftCollectionDeployMetaTx(nftIn);
    const predictedCollectionAddress = validateHex32(predictCreate2ContractAddress(deployer, hexToBytes(salts.collectionSaltHex), bytecodeBytesForPredict(collectionDeployBase.bytecode))).toLowerCase();
    const predictedTokenAddress = validateHex32(predictCreate2ContractAddress(deployer, hexToBytes(salts.tokenSaltHex), bytecodeBytesForPredict(tokenBytecodeForPredict))).toLowerCase();
    const collectionLink = normalizeLinkedNftToken({
        role: 'nft_collection',
        self: predictedCollectionAddress,
        peers: [predictedTokenAddress],
        attester: deployer,
        revision: 0,
        note: input.note,
    });
    const tokenLink = normalizeLinkedNftToken({
        role: 'fungible_token',
        self: predictedTokenAddress,
        peers: [predictedCollectionAddress],
        attester: deployer,
        revision: 0,
        note: input.note,
    });
    const collectionDeployTx = {
        ...collectionDeployBase,
        description_hash: descriptionHashHexFromLinkedNftToken(collectionLink),
        create2_salt: salts.collectionSaltHex,
    };
    const tokenDeployTx = {
        ...tokenDeployBase,
        // Linked schema wins over security-only hash when both would apply.
        description_hash: descriptionHashHexFromLinkedNftToken(tokenLink),
        create2_salt: salts.tokenSaltHex,
    };
    return {
        collectionSaltHex: salts.collectionSaltHex,
        tokenSaltHex: salts.tokenSaltHex,
        predictedCollectionAddress,
        predictedTokenAddress,
        collectionLink,
        tokenLink,
        collectionDeployTx,
        tokenDeployTx,
    };
}
