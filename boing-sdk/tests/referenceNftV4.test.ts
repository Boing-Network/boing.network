import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  DEFAULT_REFERENCE_NFT_V4_DEPLOY_BYTECODE_HEX,
  DEFAULT_REFERENCE_NFT_V4_RUNTIME_BYTECODE_HEX,
  REF_NFT_V4_ADMIN_KEY_HEX,
  REF_NFT_V4_TOPIC_APPROVAL_FOR_ALL_HEX,
  REF_NFT_V4_TOPIC_FREEZE_HEX,
  REF_NFT_V4_TOPIC_MINT_BATCH_HEX,
  REF_NFT_V4_TOPIC_TRANSFER_HEX,
  REF_NFT_V4_TOTAL_MINTED_KEY_HEX,
  REFERENCE_NFT_V4_TEMPLATE_VERSION,
  SELECTOR_NFT_APPROVE,
  SELECTOR_NFT_SET_APPROVAL_FOR_ALL,
  buildReferenceNftV4DeployMetaTx,
  decodeReferenceNftV4Log,
  encodeReferenceNftAdminCalldata,
  encodeReferenceNftApproveCalldata,
  encodeReferenceNftSetApprovalForAllCalldata,
  encodeReferenceNftTransferAdminCalldata,
  fetchReferenceNftV4CollectionInfo,
  fetchReferenceNftV4IsApprovedForAll,
  fetchReferenceNftV4Token,
  isReferenceNftV4Bytecode,
  referenceNftV4OperatorStorageKey,
  referenceNftV4OwnershipChanges,
} from '../src/referenceNftV4.js';
import {
  referenceNftMetadataStorageKey,
  referenceNftOwnerStorageKey,
  referenceNftTokenIdWordFromU64,
} from '../src/referenceNft.js';
import { bytesToHex } from '../src/hex.js';

const ALICE = '0x' + 'a1'.repeat(32);
const BOB = '0x' + 'b2'.repeat(32);
const CAROL = '0x' + 'c3'.repeat(32);
const ZERO = '0x' + '00'.repeat(32);
const tid = (n: number) => referenceNftTokenIdWordFromU64(n);

describe('reference NFT v4', () => {
  it('matches the Rust golden key and topic', () => {
    expect(referenceNftV4OperatorStorageKey(ALICE, CAROL)).toBe(
      '0xe3eee8efe6fef3e4e7efe7f5feeef1e49182978c919c95f7c3c3c3c3c3c3c3c3',
    );
    expect(REF_NFT_V4_TOPIC_TRANSFER_HEX).toBe('0x424f494e475f5245464e46545f5452414e534645525f56340000000000000000');
    expect(REF_NFT_V4_ADMIN_KEY_HEX.endsWith('e0')).toBe(true);
    expect(REF_NFT_V4_TOTAL_MINTED_KEY_HEX.endsWith('e2')).toBe(true);
  });

  it('encodes calldata with the documented sizes', () => {
    const a = encodeReferenceNftApproveCalldata(BOB, tid(7));
    expect(a).toHaveLength(96);
    expect(a[31]).toBe(SELECTOR_NFT_APPROVE);
    expect(bytesToHex(a.subarray(64))).toBe(tid(7));
    const s = encodeReferenceNftSetApprovalForAllCalldata(CAROL, true);
    expect(s[31]).toBe(SELECTOR_NFT_SET_APPROVAL_FOR_ALL);
    expect(s[95]).toBe(1);
    expect(encodeReferenceNftAdminCalldata()).toHaveLength(32);
    expect(encodeReferenceNftTransferAdminCalldata(BOB)).toHaveLength(64);
  });

  it('decodes logs and folds ownership changes', () => {
    const transfer = { topics: [REF_NFT_V4_TOPIC_TRANSFER_HEX, ALICE, BOB], data: tid(3) };
    expect(decodeReferenceNftV4Log(transfer)).toEqual({ kind: 'transfer', from: ALICE, to: BOB, tokenId: tid(3) });
    const mint = { topics: [REF_NFT_V4_TOPIC_MINT_BATCH_HEX, ALICE], data: tid(1) + tid(2).slice(2) };
    expect(decodeReferenceNftV4Log(mint)).toEqual({ kind: 'mint_batch', to: ALICE, tokenIds: [tid(1), tid(2)] });
    expect(
      decodeReferenceNftV4Log({ topics: [REF_NFT_V4_TOPIC_APPROVAL_FOR_ALL_HEX, ALICE, CAROL], data: tid(1) }),
    ).toEqual({ kind: 'approval_for_all', owner: ALICE, operator: CAROL, approved: true });
    expect(decodeReferenceNftV4Log({ topics: [REF_NFT_V4_TOPIC_FREEZE_HEX, ZERO], data: '0x' })).toEqual({
      kind: 'freeze',
      tokenId: ZERO,
      all: true,
    });
    expect(decodeReferenceNftV4Log({ topics: [REF_NFT_V4_TOPIC_TRANSFER_HEX, ALICE], data: tid(1) })).toBeNull();
    expect(decodeReferenceNftV4Log({ topics: [ALICE, BOB], data: '0x' })).toBeNull();
    expect(referenceNftV4OwnershipChanges([mint, transfer])).toEqual([
      { tokenId: tid(1), from: ZERO, to: ALICE },
      { tokenId: tid(2), from: ZERO, to: ALICE },
      { tokenId: tid(3), from: ALICE, to: BOB },
    ]);
  });

  it('reads token and collection state through boing_getContractStorage', async () => {
    const one = '0x' + '00'.repeat(31) + '01';
    const store = new Map<string, string>([
      [referenceNftOwnerStorageKey(tid(5)), ALICE],
      [referenceNftMetadataStorageKey(tid(5)), '0x' + '42'.repeat(32)],
      [REF_NFT_V4_ADMIN_KEY_HEX, BOB],
      [REF_NFT_V4_TOTAL_MINTED_KEY_HEX, '0x' + '00'.repeat(31) + '28'],
      [referenceNftV4OperatorStorageKey(ALICE, CAROL), one],
    ]);
    const client = {
      async getContractStorage(_c: string, k: string) {
        return { value: store.get(k) ?? ZERO };
      },
    };
    expect(await fetchReferenceNftV4Token(client, BOB, tid(5))).toEqual({
      tokenId: tid(5),
      owner: ALICE,
      approved: ZERO,
      metadataHash: '0x' + '42'.repeat(32),
      metadataFrozen: false,
    });
    expect(await fetchReferenceNftV4CollectionInfo(client, BOB)).toEqual({
      admin: BOB,
      totalMinted: 40n,
      allMetadataFrozen: false,
    });
    expect(await fetchReferenceNftV4IsApprovedForAll(client, BOB, ALICE, CAROL)).toBe(true);
    expect(await fetchReferenceNftV4IsApprovedForAll(client, BOB, ALICE, BOB)).toBe(false);
  });

  it('pins bytecode to the docs artifacts and builds a deploy meta tx', () => {
    const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'docs', 'artifacts');
    for (const [name, hex] of [
      ['deploy', DEFAULT_REFERENCE_NFT_V4_DEPLOY_BYTECODE_HEX],
      ['runtime', DEFAULT_REFERENCE_NFT_V4_RUNTIME_BYTECODE_HEX],
    ] as const) {
      expect(readFileSync(join(root, `reference-nft-collection-v4-${name}.hex`), 'utf8').trim()).toBe(hex);
      const sha = createHash('sha256').update(Buffer.from(hex.slice(2), 'hex')).digest('hex');
      expect(readFileSync(join(root, `reference-nft-collection-v4-${name}.sha256`), 'utf8').trim()).toBe(sha);
    }
    expect(DEFAULT_REFERENCE_NFT_V4_DEPLOY_BYTECODE_HEX.startsWith('0xfd')).toBe(true);
    expect(isReferenceNftV4Bytecode(DEFAULT_REFERENCE_NFT_V4_RUNTIME_BYTECODE_HEX.toUpperCase())).toBe(true);
    expect(isReferenceNftV4Bytecode('0x00')).toBe(false);
    expect(REFERENCE_NFT_V4_TEMPLATE_VERSION).toBe('4');
    const tx = buildReferenceNftV4DeployMetaTx({ name: 'Pasture Pets', symbol: 'PET' });
    expect(tx.type).toBe('contract_deploy_meta');
    expect(tx.purpose_category).toBe('nft');
    expect(tx.bytecode).toBe(DEFAULT_REFERENCE_NFT_V4_DEPLOY_BYTECODE_HEX);
  });
});
