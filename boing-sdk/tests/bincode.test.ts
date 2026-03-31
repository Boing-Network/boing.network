import { describe, expect, it } from 'vitest';
import * as ed25519 from '@noble/ed25519';
import {
  encodeAccessList,
  encodeSignedTransaction,
  encodeTransactionPayload,
  signableTransactionHash,
} from '../src/bincode.js';
import { bytesToHex, hexToBytes } from '../src/hex.js';
import { signTransactionInput } from '../src/transactionBuilder.js';

/** From `cargo run -p boing-primitives --example dump_bincode` (deterministic ed25519 seed). */
const GOLDEN = {
  transferPayload:
    '00000000010101010101010101010101010101010101010101010101010101010101010164000000000000000000000000000000',
  contractCall:
    '0100000001010101010101010101010101010101010101010101010101010101010101010300000000000000010203',
  contractDeploy: '020000000200000000000000dead00',
  bond: '0500000001000000000000000000000000000000',
  unbond: '0600000002000000000000000000000000000000',
  deployWithPurpose:
    '030000000100000000000000ab040000000000000064656669012000000000000000cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc00',
  deployWithMeta:
    '040000000100000000000000ef04000000000000006d656d6500010500000000000000546f6b656e010300000000000000544b4e00',
  accessListEmpty: '00000000000000000000000000000000',
  signableHash: 'd12d13301b15a8bc8284e74e77a3f33d44c4103cdf88e6bb27742a4c475e05c4',
  signedTx:
    '070000000000000010afc921830e3e1027d35ef863ffd726278ca6f789ce71dcccd1af7984ee1de8000000000202020202020202020202020202020202020202020202020202020202020202090000000000000000000000000000000000000000000000000000000000000040000000000000008774b633e4c0bda025ce289aac37d52c50e225ce41184846f7283deb8c8c54d693284137c26c9b22b61130c84449ea7b7edc5b4fe599fa60351d81b44afc6f03',
};

const DETERMINISTIC_SEED = hexToBytes(
  '0x9d61b19deffd5a60ba844af492ec2cc44449c5697b326091a2b8c7304ee93770',
);

describe('bincode golden vectors (boing-primitives)', () => {
  it('encodes Transfer payload', () => {
    const p = encodeTransactionPayload({
      kind: 'transfer',
      to: new Uint8Array(32).fill(1),
      amount: 100n,
    });
    expect(Buffer.from(p).toString('hex')).toBe(GOLDEN.transferPayload);
  });

  it('encodes ContractCall payload', () => {
    const p = encodeTransactionPayload({
      kind: 'contractCall',
      contract: new Uint8Array(32).fill(1),
      calldata: new Uint8Array([1, 2, 3]),
    });
    expect(Buffer.from(p).toString('hex')).toBe(GOLDEN.contractCall);
  });

  it('encodes ContractDeploy payload', () => {
    const p = encodeTransactionPayload({
      kind: 'contractDeploy',
      bytecode: new Uint8Array([0xde, 0xad]),
      create2Salt: null,
    });
    expect(Buffer.from(p).toString('hex')).toBe(GOLDEN.contractDeploy);
  });

  it('encodes Bond / Unbond', () => {
    expect(
      Buffer.from(encodeTransactionPayload({ kind: 'bond', amount: 1n })).toString('hex'),
    ).toBe(GOLDEN.bond);
    expect(
      Buffer.from(encodeTransactionPayload({ kind: 'unbond', amount: 2n })).toString('hex'),
    ).toBe(GOLDEN.unbond);
  });

  it('encodes deploy-with-purpose and deploy-with-metadata', () => {
    expect(
      Buffer.from(
        encodeTransactionPayload({
          kind: 'contractDeployWithPurpose',
          bytecode: new Uint8Array([0xab]),
          purposeCategory: 'defi',
          descriptionHash: new Uint8Array(32).fill(0xcc),
          create2Salt: null,
        }),
      ).toString('hex'),
    ).toBe(GOLDEN.deployWithPurpose);

    expect(
      Buffer.from(
        encodeTransactionPayload({
          kind: 'contractDeployWithPurposeAndMetadata',
          bytecode: new Uint8Array([0xef]),
          purposeCategory: 'meme',
          descriptionHash: null,
          assetName: 'Token',
          assetSymbol: 'TKN',
          create2Salt: null,
        }),
      ).toString('hex'),
    ).toBe(GOLDEN.deployWithMeta);
  });

  it('encodes empty AccessList', () => {
    const a = encodeAccessList([], []);
    expect(Buffer.from(a).toString('hex')).toBe(GOLDEN.accessListEmpty);
  });

  it('matches signable hash + SignedTransaction for deterministic key', async () => {
    const sender = await ed25519.getPublicKeyAsync(DETERMINISTIC_SEED);
    expect(sender.length).toBe(32);

    const tx = {
      nonce: 7n,
      sender,
      payload: {
        kind: 'transfer' as const,
        to: new Uint8Array(32).fill(2),
        amount: 9n,
      },
      accessList: { read: [] as Uint8Array[], write: [] as Uint8Array[] },
    };

    expect(Buffer.from(signableTransactionHash(tx)).toString('hex')).toBe(GOLDEN.signableHash);

    const sig = await ed25519.signAsync(signableTransactionHash(tx), DETERMINISTIC_SEED);
    const raw = encodeSignedTransaction(tx, sig);
    expect(Buffer.from(raw).toString('hex')).toBe(GOLDEN.signedTx);
  });

  it('signTransactionInput matches golden hex', async () => {
    const sender = await ed25519.getPublicKeyAsync(DETERMINISTIC_SEED);
    const to = new Uint8Array(32).fill(2);
    const tx = {
      nonce: 7n,
      sender,
      payload: { kind: 'transfer' as const, to, amount: 9n },
      accessList: { read: [] as Uint8Array[], write: [] as Uint8Array[] },
    };
    const hex = await signTransactionInput(tx, DETERMINISTIC_SEED);
    expect(hex.toLowerCase()).toBe('0x' + GOLDEN.signedTx);
  });

  it('signTransactionInput rejects sender mismatch', async () => {
    const wrongSender = new Uint8Array(32).fill(0xab);
    const tx = {
      nonce: 0n,
      sender: wrongSender,
      payload: {
        kind: 'transfer' as const,
        to: new Uint8Array(32).fill(2),
        amount: 1n,
      },
      accessList: { read: [] as Uint8Array[], write: [] as Uint8Array[] },
    };
    await expect(signTransactionInput(tx, DETERMINISTIC_SEED)).rejects.toThrow(/sender must equal/);
  });
});
