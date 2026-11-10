import { describe, expect, it } from 'vitest';
import {
  MAX_REFERENCE_MULTI_TOKEN_BATCH,
  REF_MT_TOPIC_TRANSFER_HEX,
  REFERENCE_MULTI_TOKEN_TEMPLATE_VERSION,
  SELECTOR_MT_MINT_BATCH,
  SELECTOR_MT_TRANSFER,
  buildReferenceMultiTokenDeployMetaTx,
  decodeMultiTokenCalldata,
  decodeMultiTokenTransferLog,
  encodeMultiTokenBalanceOfCalldata,
  encodeMultiTokenMintBatchCalldata,
  encodeMultiTokenMintCalldata,
  encodeMultiTokenSetItemMetadataCalldata,
  encodeMultiTokenTransferBatchCalldata,
  encodeMultiTokenTransferCalldata,
  fetchMultiTokenBalance,
  fetchMultiTokenItemInfo,
  isReferenceMultiTokenBytecode,
  multiTokenAmountWord,
  multiTokenBalanceStorageKey,
  multiTokenItemIdWord,
  multiTokenMetadataStorageKey,
  multiTokenSupplyStorageKey,
  DEFAULT_REFERENCE_MULTI_TOKEN_DEPLOY_BYTECODE_HEX,
  DEFAULT_REFERENCE_MULTI_TOKEN_RUNTIME_BYTECODE_HEX,
} from '../src/referenceMultiToken.js';
import { bytesToHex } from '../src/hex.js';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ALICE = '0x' + 'a1'.repeat(32);
const BOB = '0x' + 'b2'.repeat(32);

describe('reference multi-token', () => {
  it('matches the Rust golden storage keys', () => {
    expect(multiTokenBalanceStorageKey(ALICE, 7)).toBe(
      '0xe3eee8efe6fef3e4e7ecf5fee3e0edfef790a1a1a1a1a1a10000000000000007',
    );
    expect(multiTokenSupplyStorageKey(7)).toBe('0x424f494e475f5245464d545f535550504c595f56310000000000000000000007');
    expect(multiTokenMetadataStorageKey(7)).toBe('0x424f494e475f5245464d545f4d4554415f563100000000000000000000000007');
    expect(REF_MT_TOPIC_TRANSFER_HEX).toBe('0x424f494e475f5245464d545f5452414e534645525f5631000000000000000000');
  });

  it('encodes fixed-size calldata', () => {
    expect(encodeMultiTokenBalanceOfCalldata(ALICE, 1)).toHaveLength(96);
    const t = encodeMultiTokenTransferCalldata(BOB, 3, 25n);
    expect(t).toHaveLength(128);
    expect(t[31]).toBe(SELECTOR_MT_TRANSFER);
    expect(bytesToHex(t.subarray(64, 96))).toBe(multiTokenItemIdWord(3));
    expect(bytesToHex(t.subarray(96, 128))).toBe(multiTokenAmountWord(25));
    expect(encodeMultiTokenSetItemMetadataCalldata(3, '0x' + '42'.repeat(32))).toHaveLength(96);
    expect(() => encodeMultiTokenMintCalldata(BOB, -1, 1)).toThrow();
    expect(() => encodeMultiTokenMintCalldata(BOB, 1, 1n << 128n)).toThrow();
  });

  it('round-trips batch calldata through the decoder', () => {
    const entries = [
      { itemId: 1, amount: 10 },
      { itemId: 2, amount: 5n },
    ];
    const cd = encodeMultiTokenMintBatchCalldata(ALICE, entries);
    expect(cd).toHaveLength(96 + 64 * 2);
    const d = decodeMultiTokenCalldata(bytesToHex(cd));
    expect(d).toEqual({
      selector: SELECTOR_MT_MINT_BATCH,
      kind: 'mint_batch',
      to: ALICE,
      entries: [
        { itemId: 1n, amount: 10n },
        { itemId: 2n, amount: 5n },
      ],
    });
    const many = Array.from({ length: MAX_REFERENCE_MULTI_TOKEN_BATCH + 1 }, (_, i) => ({ itemId: i, amount: 1 }));
    expect(() => encodeMultiTokenTransferBatchCalldata(ALICE, many)).toThrow();
    const trailing = new Uint8Array([...cd, ...new Uint8Array(32).fill(0xff)]);
    expect(decodeMultiTokenCalldata(trailing)).toBeNull();
    expect(decodeMultiTokenCalldata(encodeMultiTokenTransferCalldata(BOB, 9, 4))).toMatchObject({
      kind: 'transfer',
      to: BOB,
      itemId: 9n,
      amount: 4n,
    });
    expect(decodeMultiTokenCalldata('0x' + '00'.repeat(31) + '04')).toBeNull();
  });

  it('decodes transfer logs', () => {
    const log = {
      topics: [REF_MT_TOPIC_TRANSFER_HEX, '0x' + '00'.repeat(32), ALICE],
      data: multiTokenItemIdWord(7) + multiTokenAmountWord(100).slice(2),
    };
    expect(decodeMultiTokenTransferLog(log)).toEqual({
      from: '0x' + '00'.repeat(32),
      to: ALICE,
      itemId: 7n,
      amount: 100n,
      isMint: true,
    });
    expect(decodeMultiTokenTransferLog({ topics: [ALICE], data: '0x' })).toBeNull();
  });

  it('reads balances and item info through boing_getContractStorage', async () => {
    const store = new Map<string, string>([
      [multiTokenBalanceStorageKey(ALICE, 7), multiTokenAmountWord(42)],
      [multiTokenSupplyStorageKey(7), multiTokenAmountWord(100)],
      [multiTokenMetadataStorageKey(7), '0x' + '42'.repeat(32)],
    ]);
    const client = {
      async getContractStorage(_c: string, k: string) {
        return { value: store.get(k) ?? '0x' + '00'.repeat(32) };
      },
    };
    expect(await fetchMultiTokenBalance(client, BOB, ALICE, 7)).toBe(42n);
    expect(await fetchMultiTokenBalance(client, BOB, BOB, 7)).toBe(0n);
    expect(await fetchMultiTokenItemInfo(client, BOB, 7)).toEqual({
      itemId: 7n,
      totalSupply: 100n,
      metadataHash: '0x' + '42'.repeat(32),
    });
  });

  it('pins bytecode to the docs artifacts and builds a deploy meta tx', () => {
    const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'docs', 'artifacts');
    expect(readFileSync(join(root, 'reference-multi-token-v1-deploy.hex'), 'utf8').trim()).toBe(
      DEFAULT_REFERENCE_MULTI_TOKEN_DEPLOY_BYTECODE_HEX,
    );
    expect(readFileSync(join(root, 'reference-multi-token-v1-runtime.hex'), 'utf8').trim()).toBe(
      DEFAULT_REFERENCE_MULTI_TOKEN_RUNTIME_BYTECODE_HEX,
    );
    expect(DEFAULT_REFERENCE_MULTI_TOKEN_DEPLOY_BYTECODE_HEX.startsWith('0xfd')).toBe(true);
    expect(isReferenceMultiTokenBytecode(DEFAULT_REFERENCE_MULTI_TOKEN_RUNTIME_BYTECODE_HEX.toUpperCase())).toBe(true);
    expect(isReferenceMultiTokenBytecode('0x00')).toBe(false);
    expect(REFERENCE_MULTI_TOKEN_TEMPLATE_VERSION).toBe('1');
    const tx = buildReferenceMultiTokenDeployMetaTx({ name: 'Pasture Resources', symbol: 'res' });
    expect(tx.type).toBe('contract_deploy_meta');
    expect(tx.purpose_category).toBe('nft');
    expect(tx.bytecode).toBe(DEFAULT_REFERENCE_MULTI_TOKEN_DEPLOY_BYTECODE_HEX);
  });
});
