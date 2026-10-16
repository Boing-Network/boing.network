/**
 * First-class reference fungible token helpers -- the TypeScript twin of the `boing token` CLI
 * (`crates/boing-cli/src/token.rs`). Transactions are byte-identical to the CLI for the same
 * signer, nonce and arguments (cross-checked in `tests/boingToken.test.ts` against CLI `--dry-run`
 * vectors): Boing VM template bytecode, bincode `SignedTransaction`, Ed25519, nonce-derived token
 * address, access list = Rust `Transaction::suggested_parallel_access_list()`.
 *
 * See `docs/BOING-REFERENCE-TOKEN.md`. The template allows exactly one `mint_first` (fixed supply);
 * the first caller becomes admin.
 */
import { transactionIdFromSignedTransactionHex } from './bincode.js';
import { predictNonceDerivedContractAddress } from './create2.js';
import { DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX } from './defaultReferenceFungibleTemplateBytecodeHex.js';
import { DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_V2_BYTECODE_HEX } from './defaultReferenceFungibleTemplateV2BytecodeHex.js';
import { bytesToHex, hexToAccountId, hexToBytes, validateHex32 } from './hex.js';
import { encodeReferenceApproveCalldata, encodeReferenceMintFirstCalldata, encodeReferenceTransferCalldata, } from './referenceToken.js';
import { fetchNextNonce, senderHexFromSecretKey, signTransactionInputForClient, } from './transactionBuilder.js';
import { submitSignedTransactionIdempotent } from './submitIdempotent.js';
/** QA purpose category for fungible deploys (same as CLI `TOKEN_PURPOSE`). */
export const BOING_TOKEN_PURPOSE = 'token';
/** `TransactionPayload::MAX_ASSET_NAME_LEN` (UTF-8 bytes). */
export const BOING_TOKEN_MAX_NAME_BYTES = 256;
/** `TransactionPayload::MAX_ASSET_SYMBOL_LEN` (UTF-8 bytes). */
export const BOING_TOKEN_MAX_SYMBOL_BYTES = 32;
/** `REF_FUNGIBLE_BALANCE_XOR`: ASCII `BOING_REFFUNGIBLE_BAL1` zero-padded to 32 bytes. */
export const REFERENCE_FUNGIBLE_BALANCE_XOR = (() => {
    const out = new Uint8Array(32);
    out.set(new TextEncoder().encode('BOING_REFFUNGIBLE_BAL1'));
    return out;
})();
export const BOING_TOKEN_DEFAULT_VERSION = 2;
/** `REF_FUNGIBLE_ALLOWANCE_XOR`: ASCII `BOING_REFFUNGIBLE_ALW2` zero-padded to 32 bytes. */
export const REFERENCE_FUNGIBLE_ALLOWANCE_XOR = (() => {
    const out = new Uint8Array(32);
    out.set(new TextEncoder().encode('BOING_REFFUNGIBLE_ALW2'));
    return out;
})();
/** v2 storage key of `allowance[owner][spender]`: `(owner[0..16] ‖ spender[0..16]) ^ ALLOWANCE_XOR`. */
export function boingTokenAllowanceStorageKeyHex(ownerHex, spenderHex) {
    const o = hexToAccountId(validateHex32(ownerHex));
    const s = hexToAccountId(validateHex32(spenderHex));
    const k = new Uint8Array(32);
    k.set(o.subarray(0, 16), 0);
    k.set(s.subarray(0, 16), 16);
    for (let i = 0; i < 32; i++)
        k[i] = k[i] ^ REFERENCE_FUNGIBLE_ALLOWANCE_XOR[i];
    return bytesToHex(k);
}
/** Reference fungible template bytecode hex (what `boing token bytecode --version vN` prints). */
export function boingTokenBytecodeHex(version = BOING_TOKEN_DEFAULT_VERSION) {
    const src = version === 1 ? DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX : DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_V2_BYTECODE_HEX;
    const h = src.trim().toLowerCase();
    return (h.startsWith('0x') ? h : `0x${h}`);
}
/** `boing_getContractStorage` key for `holder`'s balance (`holder ^ REF_FUNGIBLE_BALANCE_XOR`). */
export function boingTokenBalanceStorageKeyHex(holderHex) {
    const h = hexToAccountId(validateHex32(holderHex));
    const k = new Uint8Array(32);
    for (let i = 0; i < 32; i++)
        k[i] = h[i] ^ REFERENCE_FUNGIBLE_BALANCE_XOR[i];
    return bytesToHex(k);
}
/** Decode a 32-byte big-endian storage word to a balance (throws if it exceeds u128). */
export function decodeBoingTokenBalanceWord(wordHex) {
    const b = hexToBytes(wordHex.trim().startsWith('0x') ? wordHex.trim() : `0x${wordHex.trim()}`);
    if (b.length !== 32)
        throw new Error(`storage word must be 32 bytes, got ${b.length}`);
    if (b.subarray(0, 16).some((x) => x !== 0))
        throw new Error('balance exceeds u128');
    let v = 0n;
    for (let i = 16; i < 32; i++)
        v = (v << 8n) | BigInt(b[i]);
    return v;
}
function utf8Len(s) {
    return new TextEncoder().encode(s).length;
}
/** Same checks as CLI `validate_metadata`. */
export function validateBoingTokenMetadata(name, symbol) {
    if (name != null && utf8Len(name) > BOING_TOKEN_MAX_NAME_BYTES) {
        throw new Error(`name exceeds ${BOING_TOKEN_MAX_NAME_BYTES} bytes`);
    }
    if (symbol != null && utf8Len(symbol) > BOING_TOKEN_MAX_SYMBOL_BYTES) {
        throw new Error(`symbol exceeds ${BOING_TOKEN_MAX_SYMBOL_BYTES} bytes`);
    }
}
function checkU128(amount, what) {
    if (amount < 0n || amount >= 1n << 128n)
        throw new Error(`${what} must fit in u128`);
}
/** Unsigned `ContractDeployWithPurposeAndMetadata` (purpose `token`) for the reference template. */
export function buildBoingTokenDeployTransaction(input) {
    validateBoingTokenMetadata(input.name, input.symbol);
    const sender = hexToAccountId(validateHex32(input.senderHex));
    return {
        nonce: input.nonce,
        sender,
        payload: {
            kind: 'contractDeployWithPurposeAndMetadata',
            bytecode: hexToBytes(boingTokenBytecodeHex(input.version ?? BOING_TOKEN_DEFAULT_VERSION)),
            purposeCategory: BOING_TOKEN_PURPOSE,
            descriptionHash: null,
            assetName: input.name ?? null,
            assetSymbol: input.symbol ?? null,
            create2Salt: null,
        },
        // suggested_parallel_access_list for a nonce-derived deploy: [sender] / [sender]
        accessList: { read: [sender], write: [sender] },
    };
}
function buildCall(input, calldata) {
    const sender = hexToAccountId(validateHex32(input.senderHex));
    const contract = hexToAccountId(validateHex32(input.tokenHex));
    return {
        nonce: input.nonce,
        sender,
        payload: { kind: 'contractCall', contract, calldata },
        // suggested_parallel_access_list for ContractCall: [sender, contract] / [sender, contract]
        accessList: { read: [sender, contract], write: [sender, contract] },
    };
}
/** Unsigned one-time `mint_first(to, amount)` call. */
export function buildBoingTokenMintTransaction(input) {
    checkU128(input.amount, 'amount');
    return buildCall(input, encodeReferenceMintFirstCalldata(validateHex32(input.toHex), input.amount));
}
/** Unsigned `transfer(to, amount)` call. */
export function buildBoingTokenTransferTransaction(input) {
    checkU128(input.amount, 'amount');
    if (input.amount === 0n)
        throw new Error('amount must be > 0');
    if (validateHex32(input.toHex).toLowerCase() === validateHex32(input.senderHex).toLowerCase()) {
        throw new Error('refusing self-transfer: v1 reference tokens double the balance on self-transfer (known bug); it is a no-op on v2');
    }
    return buildCall(input, encodeReferenceTransferCalldata(validateHex32(input.toHex), input.amount));
}
/** v2: unsigned `approve(spender = toHex, amount)` call (overwrites the allowance). */
export function buildBoingTokenApproveTransaction(input) {
    checkU128(input.amount, 'amount');
    return buildCall(input, encodeReferenceApproveCalldata(validateHex32(input.toHex), input.amount));
}
async function signAndMaybeSubmit(client, tx, secretKey32, dryRun) {
    const signedHex = await signTransactionInputForClient(client, tx, secretKey32);
    const txId = transactionIdFromSignedTransactionHex(signedHex);
    if (!dryRun) {
        if (!client)
            throw new Error('a BoingClient is required to submit');
        await submitSignedTransactionIdempotent(client, signedHex);
    }
    return { txId, signedHex, submitted: !dryRun };
}
async function resolveNonce(client, senderHex, nonce) {
    if (nonce != null)
        return nonce;
    if (!client)
        throw new Error('pass a nonce or a BoingClient to fetch it');
    return fetchNextNonce(client, senderHex);
}
/** `boing token deploy`: deploy the reference template, optionally minting the fixed supply. */
export async function deployBoingToken(client, secretKey32, input = {}) {
    validateBoingTokenMetadata(input.name, input.symbol);
    const deployerHex = await senderHexFromSecretKey(secretKey32);
    const nonce = await resolveNonce(client, deployerHex, input.nonce);
    const tokenHex = predictNonceDerivedContractAddress(deployerHex, nonce);
    const dryRun = !!input.dryRun;
    const deployTx = await signAndMaybeSubmit(client, buildBoingTokenDeployTransaction({ nonce, senderHex: deployerHex, name: input.name, symbol: input.symbol, version: input.version }), secretKey32, dryRun);
    let mintTx = null;
    const supply = input.initialSupply ?? 0n;
    if (supply > 0n) {
        mintTx = await signAndMaybeSubmit(client, buildBoingTokenMintTransaction({
            nonce: nonce + 1n,
            senderHex: deployerHex,
            tokenHex,
            toHex: input.mintToHex ?? deployerHex,
            amount: supply,
        }), secretKey32, dryRun);
    }
    return { deployerHex, tokenHex, deployTx, mintTx };
}
/** `boing token mint`: one-time `mint_first` (admin only). `toHex` defaults to the signer. */
export async function mintBoingToken(client, secretKey32, input) {
    const senderHex = await senderHexFromSecretKey(secretKey32);
    const nonce = await resolveNonce(client, senderHex, input.nonce);
    return signAndMaybeSubmit(client, buildBoingTokenMintTransaction({ nonce, senderHex, tokenHex: input.tokenHex, toHex: input.toHex ?? senderHex, amount: input.amount }), secretKey32, !!input.dryRun);
}
/** `boing token transfer`. */
export async function transferBoingToken(client, secretKey32, input) {
    const senderHex = await senderHexFromSecretKey(secretKey32);
    const tx = buildBoingTokenTransferTransaction({ nonce: 0n, senderHex, tokenHex: input.tokenHex, toHex: input.toHex, amount: input.amount });
    tx.nonce = await resolveNonce(client, senderHex, input.nonce);
    return signAndMaybeSubmit(client, tx, secretKey32, !!input.dryRun);
}
/** `boing token balance`: read a holder's balance via `boing_getContractStorage`. */
export async function getBoingTokenBalance(client, tokenHex, holderHex) {
    const word = await client.getContractStorage(validateHex32(tokenHex), boingTokenBalanceStorageKeyHex(holderHex));
    if (typeof word?.value !== 'string')
        throw new Error('unexpected boing_getContractStorage result');
    return decodeBoingTokenBalanceWord(word.value);
}
/** `boing token approve` (v2): let `spenderHex` pull up to `amount` (overwrites). */
export async function approveBoingToken(client, secretKey32, input) {
    const senderHex = await senderHexFromSecretKey(secretKey32);
    const nonce = await resolveNonce(client, senderHex, input.nonce);
    return signAndMaybeSubmit(client, buildBoingTokenApproveTransaction({ nonce, senderHex, tokenHex: input.tokenHex, toHex: input.spenderHex, amount: input.amount }), secretKey32, !!input.dryRun);
}
/** `boing token allowance` (v2): read `allowance[owner][spender]` via `boing_getContractStorage`. */
export async function getBoingTokenAllowance(client, tokenHex, ownerHex, spenderHex) {
    const word = await client.getContractStorage(validateHex32(tokenHex), boingTokenAllowanceStorageKeyHex(ownerHex, spenderHex));
    if (typeof word?.value !== 'string')
        throw new Error('unexpected boing_getContractStorage result');
    return decodeBoingTokenBalanceWord(word.value);
}
