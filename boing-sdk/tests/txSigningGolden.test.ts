import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import * as ed25519 from '@noble/ed25519';
import {
  encodeAccessList,
  encodeSignedTransaction,
  encodeTransactionPayload,
  signableTransactionHashWithTip,
  type TransactionInput,
  type TransactionPayloadInput,
} from '../src/bincode.js';
import { bytesToHex, hexToBytes } from '../src/hex.js';

/**
 * Consumes the Rust-generated golden vectors in `docs/fixtures/tx-signing-golden-vectors.json`
 * (crates/boing-primitives/tests/tx_signing_golden.rs). Any signer (boing.express, SDK) that
 * reproduces every field here is wire-compatible with the node, including the tipped
 * `BOING_TX_PRIORITY` domain.
 */
interface Vector {
  name: string;
  chain_id: number;
  nonce: string;
  sender: string;
  payload: { kind: string; to?: string; amount?: string; contract?: string; calldata?: string };
  access_list: { read: string[]; write: string[] };
  priority_tip: string;
  domain: string;
  payload_bincode_hex: string;
  access_list_bincode_hex: string;
  preimage_hex: string;
  signing_digest_hex: string;
  signature_hex: string;
  signed_tx_bincode_hex: string;
}

const fixture = JSON.parse(
  readFileSync(
    fileURLToPath(new URL('../../docs/fixtures/tx-signing-golden-vectors.json', import.meta.url)),
    'utf8',
  ),
) as { signer_seed_hex: string; signer_public_key_hex: string; vectors: Vector[] };

function payloadOf(v: Vector): TransactionPayloadInput {
  if (v.payload.kind === 'transfer') {
    return {
      kind: 'transfer',
      to: hexToBytes(v.payload.to!),
      amount: BigInt(v.payload.amount!),
    };
  }
  if (v.payload.kind === 'contractCall') {
    return {
      kind: 'contractCall',
      contract: hexToBytes(v.payload.contract!),
      calldata: hexToBytes(v.payload.calldata!),
    };
  }
  throw new Error(`unsupported fixture payload kind ${v.payload.kind}`);
}

function txOf(v: Vector): TransactionInput {
  const tip = BigInt(v.priority_tip);
  return {
    nonce: BigInt(v.nonce),
    sender: hexToBytes(v.sender),
    payload: payloadOf(v),
    accessList: {
      read: v.access_list.read.map((h) => hexToBytes(h)),
      write: v.access_list.write.map((h) => hexToBytes(h)),
    },
    priorityTip: tip,
  };
}

const strip = (h: string) => (h.startsWith('0x') ? h.slice(2) : h).toLowerCase();

describe('tx signing golden vectors (Rust fixture)', () => {
  it('fixture is non-trivial and covers tipped and untipped digests', () => {
    expect(fixture.vectors.length).toBeGreaterThanOrEqual(5);
    expect(fixture.vectors.some((v) => v.priority_tip === '0')).toBe(true);
    expect(fixture.vectors.some((v) => v.domain === 'BOING_TX_PRIORITY')).toBe(true);
  });

  for (const v of fixture.vectors) {
    it(`${v.name}: payload, access list, digest, signature and signed bytes match Rust`, async () => {
      const tx = txOf(v);
      expect(bytesToHex(encodeTransactionPayload(tx.payload)).replace(/^0x/, '')).toBe(
        v.payload_bincode_hex,
      );
      expect(
        bytesToHex(encodeAccessList(tx.accessList.read, tx.accessList.write)).replace(/^0x/, ''),
      ).toBe(v.access_list_bincode_hex);

      const digest = signableTransactionHashWithTip(tx, v.chain_id, BigInt(v.priority_tip));
      expect(strip(bytesToHex(digest))).toBe(v.signing_digest_hex);

      const seed = hexToBytes(fixture.signer_seed_hex);
      expect(strip(bytesToHex(await ed25519.getPublicKeyAsync(seed)))).toBe(
        strip(fixture.signer_public_key_hex),
      );
      const sig = await ed25519.signAsync(digest, seed);
      expect(strip(bytesToHex(sig))).toBe(v.signature_hex);
      expect(strip(bytesToHex(encodeSignedTransaction(tx, sig)))).toBe(v.signed_tx_bincode_hex);
    });
  }

  it('the same transaction signs differently per chain id and per tip', () => {
    const [untipped, tipped, mainnet] = fixture.vectors;
    expect(untipped.signing_digest_hex).not.toBe(tipped.signing_digest_hex);
    expect(tipped.signing_digest_hex).not.toBe(mainnet.signing_digest_hex);
  });
});