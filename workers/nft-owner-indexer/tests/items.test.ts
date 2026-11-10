import { describe, expect, it } from 'vitest';
import {
  REF_MT_TOPIC_TRANSFER_HEX,
  encodeMultiTokenMintBatchCalldata,
  encodeMultiTokenTransferCalldata,
} from '@boing-network/boing-sdk';
import { extractItemTouchesFromBlock, parseItemIdParam } from '../src/items.js';

const contract = '0x' + 'cc'.repeat(32);
const alice = '0x' + '11'.repeat(32);
const bob = '0x' + '22'.repeat(32);
const hex = (b: Uint8Array) => '0x' + Buffer.from(b).toString('hex');
const word = (n: number) => n.toString(16).padStart(64, '0');

function tx(sender: string, calldata: Uint8Array) {
  return { sender, payload: { ContractCall: { contract, calldata: hex(calldata) } } };
}

describe('extractItemTouchesFromBlock', () => {
  it('uses transfer logs when present (from and to)', () => {
    const block = {
      transactions: [tx(alice, encodeMultiTokenTransferCalldata(bob, 3, 2))],
      receipts: [
        {
          success: true,
          tx_id: '0x' + 'ab'.repeat(32),
          logs: [{ topics: [REF_MT_TOPIC_TRANSFER_HEX, alice, bob], data: '0x' + word(3) + word(2) }],
        },
      ],
    };
    const t = extractItemTouchesFromBlock(block, 10);
    expect(t.map((x) => [x.accountHex, x.itemId])).toEqual([
      [alice, '3'],
      [bob, '3'],
    ]);
    expect(t[0]!.txId).toBe('0x' + 'ab'.repeat(32));
  });

  it('falls back to calldata, skips mint zero-sender and failed txs', () => {
    const block = {
      transactions: [
        tx(alice, encodeMultiTokenMintBatchCalldata(bob, [{ itemId: 1, amount: 5 }, { itemId: 2, amount: 1 }])),
        tx(alice, encodeMultiTokenTransferCalldata(bob, 9, 1)),
      ],
      receipts: [{ success: true }, { success: false }],
    };
    const t = extractItemTouchesFromBlock(block, 11);
    expect(t.map((x) => [x.accountHex, x.itemId])).toEqual([
      [bob, '1'],
      [bob, '2'],
    ]);
  });

  it('parses item id params', () => {
    expect(parseItemIdParam('7')).toBe('7');
    expect(parseItemIdParam('18446744073709551616')).toBeNull();
    expect(parseItemIdParam('x')).toBeNull();
  });
});
