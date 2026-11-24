import { describe, expect, it } from 'vitest';
import {
  REF_NFT_V4_TOPIC_METADATA_HEX,
  REF_NFT_V4_TOPIC_MINT_BATCH_HEX,
  REF_NFT_V4_TOPIC_TRANSFER_HEX,
  REF_NFT_V4_TOPIC_APPROVAL_HEX,
  DEFAULT_REFERENCE_NFT_V4_RUNTIME_BYTECODE_HEX,
  encodeReferenceMintBatchCalldata,
  encodeReferenceTransferNftCalldata,
  encodeReferenceSetMetadataHashCalldata,
  encodeReferenceNftApproveCalldata,
  referenceNftTokenIdWordFromU64,
} from '@boing-network/boing-sdk';
import { extractNftEventsFromBlock } from '../src/extract.js';
import { isV4Collection, v4Cache } from '../src/admin.js';

const collection = '0x' + 'aa'.repeat(32);
const admin = '0x' + '0a'.repeat(32);
const alice = '0x' + '11'.repeat(32);
const bob = '0x' + '22'.repeat(32);
const spender = '0x' + '33'.repeat(32);
const id = (n: number) => referenceNftTokenIdWordFromU64(n);
const hex = (b: Uint8Array) => '0x' + Buffer.from(b).toString('hex');
const h = (n: number) => '0x' + 'bb' + '00'.repeat(30) + n.toString(16).padStart(2, '0');

function call(sender: string, calldata: Uint8Array) {
  return { sender, payload: { ContractCall: { contract: collection, calldata: hex(calldata) } } };
}

const noOwner = () => null;
const noAdmin = () => null; // v4 path must not need the admin lookup
const v4 = () => true;

describe('template v4 log ingestion', () => {
  it('reads mints (with calldata hashes), spender transfers and metadata from logs', () => {
    const block = {
      transactions: [
        call(admin, encodeReferenceMintBatchCalldata(alice, [id(1), id(2)], [h(1), '0x' + '00'.repeat(32)])),
        call(spender, encodeReferenceTransferNftCalldata(bob, id(1))),
        call(admin, encodeReferenceSetMetadataHashCalldata(id(2), h(9))),
        call(alice, encodeReferenceNftApproveCalldata(spender, id(2))),
      ],
      receipts: [
        { tx_id: '0x' + '01'.repeat(32), success: true, logs: [{ topics: [REF_NFT_V4_TOPIC_MINT_BATCH_HEX, alice], data: id(1) + id(2).slice(2) }] },
        { tx_id: '0x' + '02'.repeat(32), success: true, logs: [{ topics: [REF_NFT_V4_TOPIC_TRANSFER_HEX, alice, bob], data: id(1) }] },
        { tx_id: '0x' + '03'.repeat(32), success: true, logs: [{ topics: [REF_NFT_V4_TOPIC_METADATA_HEX, id(2)], data: h(9) }] },
        { tx_id: '0x' + '04'.repeat(32), success: true, logs: [{ topics: [REF_NFT_V4_TOPIC_APPROVAL_HEX, alice, spender], data: id(2) }] },
      ],
    };
    const events = extractNftEventsFromBlock(block, 7, noOwner, noAdmin, v4);
    expect(events.map((e) => [e.eventKind, e.tokenIdHex, e.fromOwnerHex, e.toOwnerHex, e.metadataHashHex])).toEqual([
      ['mint_batch', id(1), '0x' + '00'.repeat(32), alice, h(1)],
      ['mint_batch', id(2), '0x' + '00'.repeat(32), alice, null],
      ['transfer_nft', id(1), alice, bob, null],
      ['set_metadata_hash', id(2), null, null, h(9)],
    ]);
    expect(events[1]!.eventIndex).toBe(1);
  });

  it('ignores failed receipts and calls whose logs are missing', () => {
    const block = {
      transactions: [
        call(bob, encodeReferenceTransferNftCalldata(bob, id(1))),
        call(bob, encodeReferenceTransferNftCalldata(bob, id(1))),
      ],
      receipts: [
        { success: false, logs: [] },
        { success: true, logs: [] },
      ],
    };
    expect(extractNftEventsFromBlock(block, 1, () => alice, noAdmin, v4)).toEqual([]);
  });

  it('accepts byte-array topics and data from the public RPC', () => {
    const bytes = (x: string) => Array.from(Buffer.from(x.slice(2), 'hex'));
    const block = {
      transactions: [call(alice, encodeReferenceTransferNftCalldata(bob, id(4)))],
      receipts: [{ success: true, logs: [{ topics: [REF_NFT_V4_TOPIC_TRANSFER_HEX, alice, bob].map(bytes), data: bytes(id(4)) }] }],
    };
    expect(extractNftEventsFromBlock(block, 1, noOwner, noAdmin, v4)).toMatchObject([
      { eventKind: 'transfer_nft', fromOwnerHex: alice, toOwnerHex: bob, tokenIdHex: id(4) },
    ]);
  });

  it('detects v4 collections by installed code and caches the answer', async () => {
    v4Cache.clear();
    let reads = 0;
    const readCode = async (c: string) => {
      reads++;
      return { bytecode: c === collection ? DEFAULT_REFERENCE_NFT_V4_RUNTIME_BYTECODE_HEX : '0x00' };
    };
    expect(await isV4Collection(readCode, collection)).toBe(true);
    expect(await isV4Collection(readCode, collection)).toBe(true);
    expect(await isV4Collection(readCode, alice)).toBe(false);
    expect(reads).toBe(2);
  });
});
