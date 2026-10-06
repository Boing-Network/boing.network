import { describe, expect, it } from 'vitest';
import {
  SELECTOR_MINT_BATCH,
  encodeReferenceMintBatchCalldata,
  encodeReferenceTransferNftCalldata,
  referenceNftTokenIdWordFromU64,
} from 'boing-sdk';
import { extractNftEventsFromBlock } from '../src/extract.js';
import { compareCanonicalBlockHash } from '../src/reorg.js';

const collection = '0x' + 'aa'.repeat(32);
const alice = '0x' + '11'.repeat(32);
const bob = '0x' + '22'.repeat(32);

function id(n: number): string {
  return referenceNftTokenIdWordFromU64(n);
}

function hash(n: number): string {
  const low = n.toString(16).padStart(16, '0');
  return '0x' + 'aa' + '00'.repeat(23) + low;
}

function contractCallTx(calldata: Uint8Array) {
  return {
    payload: {
      ContractCall: {
        contract: collection,
        calldata: Buffer.from(calldata).toString('hex'),
      },
    },
  };
}

describe('extractNftEventsFromBlock', () => {
  it('indexes mint_batch then same-block transfer_nft', () => {
    const mint = encodeReferenceMintBatchCalldata(alice, [id(1), id(2)], [hash(1), hash(2)]);
    const xfer = encodeReferenceTransferNftCalldata(bob, id(1));
    const block = {
      transactions: [contractCallTx(mint), contractCallTx(xfer)],
      receipts: [
        { tx_id: '0x' + '01'.repeat(32), success: true },
        { tx_id: '0x' + '02'.repeat(32), success: true },
      ],
    };

    const events = extractNftEventsFromBlock(block, 42, () => null);
    expect(events.filter((e) => e.eventKind === 'mint_batch')).toHaveLength(2);
    const transfer = events.find((e) => e.eventKind === 'transfer_nft');
    expect(transfer).toMatchObject({
      toOwnerHex: bob,
      fromOwnerHex: alice,
      tokenIdHex: id(1),
      collectionHex: collection,
    });
    expect(events[0]!.eventKind).toBe('mint_batch');
    expect(SELECTOR_MINT_BATCH).toBe(0x06);
  });

  it('skips failed receipts', () => {
    const mint = encodeReferenceMintBatchCalldata(alice, [id(1)], [hash(1)]);
    const block = {
      transactions: [contractCallTx(mint)],
      receipts: [{ tx_id: '0x' + '01'.repeat(32), success: false }],
    };
    expect(extractNftEventsFromBlock(block, 1, () => null)).toEqual([]);
  });

  it('uses lookupCurrentOwner for transfers of previously indexed tokens', () => {
    const xfer = encodeReferenceTransferNftCalldata(bob, id(9));
    const block = {
      transactions: [contractCallTx(xfer)],
      receipts: [{ tx_id: '0x' + '03'.repeat(32), success: true }],
    };
    const events = extractNftEventsFromBlock(block, 7, (c, t) =>
      c === collection && t === id(9) ? alice : null
    );
    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({ fromOwnerHex: alice, toOwnerHex: bob });
  });
});

describe('compareCanonicalBlockHash', () => {
  it('matches case-insensitively with 0x', () => {
    expect(compareCanonicalBlockHash('0x' + 'Ab'.repeat(32), '0x' + 'ab'.repeat(32))).toBe('match');
    expect(compareCanonicalBlockHash('0x' + '11'.repeat(32), '0x' + '22'.repeat(32))).toBe(
      'mismatch'
    );
  });
});

describe('normalizeHex64', () => {
  it('accepts 32-byte number arrays from RPC Hash fields', async () => {
    const { normalizeHex64 } = await import('../src/cors.js');
    const genesis = [
      4, 125, 29, 31, 237, 79, 151, 64, 141, 66, 100, 110, 67, 80, 120, 95, 219, 213, 13, 248, 154,
      112, 69, 38, 227, 46, 99, 114, 164, 183, 32, 106,
    ];
    expect(normalizeHex64(genesis)).toBe(
      '0x047d1d1fed4f97408d42646e4350785fdbd50df89a704526e32e6372a4b7206a'
    );
    expect(normalizeHex64('047d1d1fed4f97408d42646e4350785fdbd50df89a704526e32e6372a4b7206a')).toBe(
      '0x047d1d1fed4f97408d42646e4350785fdbd50df89a704526e32e6372a4b7206a'
    );
  });
});
