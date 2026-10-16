import { describe, expect, it } from 'vitest';
import {
  buildValidatorDeregisterTransaction,
  buildValidatorRegisterTransaction,
  buildValidatorUnjailTransaction,
  boingOpenRegistrationOpen,
  findBoingValidator,
  signValidatorRegistration,
  validatorRegistrationMessage,
  validatorRegistrationMessageV2,
  verifyValidatorRegistration,
} from '../src/boingOpenValidator.js';
import { encodeTransactionPayload, PayloadVariant } from '../src/bincode.js';
import { bytesToHex } from '../src/hex.js';

const seed = (b: number) => new Uint8Array(32).fill(b);
const OWNER = '0x' + '09'.repeat(32);
// Golden values shared with the Rust test in crates/boing-primitives/src/validator_registration.rs.
const NODE7 = '0xea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c';
const MSG = '0xf8bbe9cbc961743405a6154a8dd07192efa73a3e9368271ec4b11dd0b66562b5';

describe('open validator registration', () => {
  it('message matches the Rust golden vector', () => {
    expect(bytesToHex(validatorRegistrationMessage(OWNER, NODE7))).toBe(MSG);
  });

  it('v2 message matches the Rust golden vector and is chain-bound', async () => {
    expect(bytesToHex(validatorRegistrationMessageV2(OWNER, NODE7, 6913))).toBe(
      '0x2d9b960878d673d6e84ded770590a0242fdc66ac4f34ca9d158078c7bf4b59d9',
    );
    const { nodeIdHex, nodeSignature } = await signValidatorRegistration(OWNER, seed(7), { chainId: 6913n });
    expect(await verifyValidatorRegistration(OWNER, nodeIdHex, nodeSignature, 6913)).toBe(true);
    expect(await verifyValidatorRegistration(OWNER, nodeIdHex, nodeSignature, 6914)).toBe(false);
    expect(await verifyValidatorRegistration(OWNER, nodeIdHex, nodeSignature)).toBe(false);
  });

  it('signs and verifies a proof bound to the owner', async () => {
    const { nodeIdHex, nodeSignature } = await signValidatorRegistration(OWNER, seed(7));
    expect(nodeIdHex).toBe(NODE7);
    expect(nodeSignature.length).toBe(64);
    expect(await verifyValidatorRegistration(OWNER, nodeIdHex, nodeSignature)).toBe(true);
    expect(await verifyValidatorRegistration('0x' + '08'.repeat(32), nodeIdHex, nodeSignature)).toBe(false);
    expect(await verifyValidatorRegistration(OWNER, nodeIdHex, nodeSignature.slice(0, 63))).toBe(false);
  });

  it('encodes the payload variants like bincode', async () => {
    const { nodeIdHex, nodeSignature } = await signValidatorRegistration(OWNER, seed(7));
    const reg = buildValidatorRegisterTransaction({ nonce: 3n, ownerHex: OWNER, nodeIdHex, nodeSignature });
    const enc = encodeTransactionPayload(reg.payload);
    expect(PayloadVariant.ValidatorRegister).toBe(10);
    // u32 variant + 32 node id + u64 len + 64 sig
    expect(enc.length).toBe(4 + 32 + 8 + 64);
    expect(Array.from(enc.slice(0, 4))).toEqual([10, 0, 0, 0]);
    expect(Array.from(enc.slice(36, 44))).toEqual([64, 0, 0, 0, 0, 0, 0, 0]);
    const de = encodeTransactionPayload(
      buildValidatorDeregisterTransaction({ nonce: 0n, ownerHex: OWNER, nodeIdHex }).payload,
    );
    expect(de[0]).toBe(11);
    expect(de.length).toBe(36);
    const un = encodeTransactionPayload(buildValidatorUnjailTransaction({ nonce: 0n, ownerHex: OWNER, nodeIdHex }).payload);
    expect(un[0]).toBe(12);
    expect(reg.accessList.read.length).toBe(1);
  });

  it('report helpers', () => {
    const report = {
      mode: 'open' as const,
      height: 99,
      quorum: 1,
      validator_count: 1,
      open: {
        activation_height: 100,
        activated_at: null,
        bond: '100',
        deregister_delay: 100,
        epoch_len: 100,
        next_epoch_height: 100,
        max_open_validators: 20,
        liveness_window: 1000,
        miss_threshold_bps: 1000,
        min_liveness_samples: 10,
        jail_cooldown: 500,
        anchors: [NODE7],
      },
      validators: [
        {
          id: NODE7.toUpperCase().replace('0X', '0x'),
          kind: 'anchor' as const,
          status: 'active' as const,
          in_set: true,
          owner: null,
          bond: '0',
          registered_at: null,
          jailed_until: null,
          jail_count: 0,
          scheduled_proposals: 0,
          missed_proposals: 0,
          uptime_pct: 100,
        },
      ],
    };
    expect(boingOpenRegistrationOpen(report)).toBe(true);
    expect(boingOpenRegistrationOpen({ ...report, height: 50 })).toBe(false);
    expect(findBoingValidator(report, NODE7)?.kind).toBe('anchor');
  });
});
