import { describe, expect, it } from 'vitest';
import {
  SELECTOR_MINT_BATCH,
  encodeReferenceMintBatchCalldata,
  encodeReferenceSetMetadataHashCalldata,
  encodeReferenceTransferNftCalldata,
  referenceNftTokenIdWordFromU64,
} from '@boing-network/boing-sdk';
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

  it('skips mint_batch rows when normalizeHex64 cannot parse owner/token', () => {
    const mint = encodeReferenceMintBatchCalldata(alice, [id(1)], [hash(1)]);
    const bad = contractCallTx(mint);
    // Corrupt embedded AccountId hex so normalizeHex64 returns null (audit #61 item 74).
    (bad.payload.ContractCall as { calldata: string }).calldata = '06' + 'zz'.repeat(64);
    const block = {
      transactions: [bad],
      receipts: [{ tx_id: '0x' + '01'.repeat(32), success: true }],
    };
    // Undecodable calldata → no events (decode fails) rather than throwing on `!`.
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

describe('extractNftEventsFromBlock auth replay (no-op STOP calls)', () => {
  const admin = '0x' + '33'.repeat(32);
  const mallory = '0x' + '44'.repeat(32);
  const ok = (n: number) => ({ tx_id: '0x' + n.toString(16).padStart(64, '0'), success: true });
  const call = (sender: string, calldata: Uint8Array) => ({ sender, ...contractCallTx(calldata) });
  const adminOf = () => admin;

  it('ignores a transfer of an owned token by someone other than the owner', () => {
    const steal = encodeReferenceTransferNftCalldata(mallory, id(1));
    const block = { transactions: [call(mallory, steal)], receipts: [ok(1)] };
    expect(extractNftEventsFromBlock(block, 50, () => alice, adminOf)).toHaveLength(0);
  });

  it('keeps a transfer sent by the current owner', () => {
    const xfer = encodeReferenceTransferNftCalldata(bob, id(1));
    const block = { transactions: [call(alice, xfer)], receipts: [ok(1)] };
    expect(extractNftEventsFromBlock(block, 50, () => alice, adminOf)).toMatchObject([
      { eventKind: 'transfer_nft', fromOwnerHex: alice, toOwnerHex: bob },
    ]);
  });

  it('accepts sender as a 32-number byte array', () => {
    const xfer = encodeReferenceTransferNftCalldata(bob, id(1));
    const block = { transactions: [call(Array(32).fill(0x11) as unknown as string, xfer)], receipts: [ok(1)] };
    expect(extractNftEventsFromBlock(block, 50, () => alice, adminOf)).toHaveLength(1);
  });

  it('lazy mint of an unowned token needs the admin', () => {
    const mint = encodeReferenceTransferNftCalldata(mallory, id(9));
    const byMallory = { transactions: [call(mallory, mint)], receipts: [ok(1)] };
    const byAdmin = { transactions: [call(admin, mint)], receipts: [ok(1)] };
    expect(extractNftEventsFromBlock(byMallory, 50, () => null, adminOf)).toHaveLength(0);
    expect(extractNftEventsFromBlock(byAdmin, 50, () => null, adminOf)).toHaveLength(1);
  });

  it('applies in-block ownership: owner check uses the overlay after a same-block mint', () => {
    const mint = encodeReferenceMintBatchCalldata(alice, [id(1)], [hash(1)]);
    const steal = encodeReferenceTransferNftCalldata(mallory, id(1));
    const block = { transactions: [call(admin, mint), call(mallory, steal)], receipts: [ok(1), ok(2)] };
    const events = extractNftEventsFromBlock(block, 50, () => null, adminOf);
    expect(events.map((e) => e.eventKind)).toEqual(['mint_batch']);
  });

  it('ignores mint_batch from a non-admin and contracts with no admin', () => {
    const mint = encodeReferenceMintBatchCalldata(mallory, [id(1)], [hash(1)]);
    const block = { transactions: [call(mallory, mint)], receipts: [ok(1)] };
    expect(extractNftEventsFromBlock(block, 50, () => null, adminOf)).toHaveLength(0);
    const byAdmin = { transactions: [call(admin, mint)], receipts: [ok(1)] };
    expect(extractNftEventsFromBlock(byAdmin, 50, () => null, () => null)).toHaveLength(0);
  });

  it('ignores metadata writes from someone who is neither admin nor owner', () => {
    const meta = encodeReferenceSetMetadataHashCalldata(id(1), hash(7));
    const byMallory = { transactions: [call(mallory, meta)], receipts: [ok(1)] };
    const byOwner = { transactions: [call(alice, meta)], receipts: [ok(1)] };
    expect(extractNftEventsFromBlock(byMallory, 50, () => alice, adminOf)).toHaveLength(0);
    expect(extractNftEventsFromBlock(byOwner, 50, () => alice, adminOf)).toHaveLength(1);
  });
});
