import { describe, expect, it } from 'vitest';
import {
  BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1_HEX,
  BUILDER_ATTESTATION_REGISTRY_COUNT_KEY_HEX,
  SELECTOR_BUILDER_ATTEST_ATTEST,
  SELECTOR_BUILDER_ATTEST_CLAIM_ASSET,
  SELECTOR_BUILDER_ATTEST_REVOKE_AT,
  decodeBuilderAttestCountReturnData,
  decodeBuilderAttestGetAtReturnData,
  encodeBuilderAttestAttestCalldata,
  encodeBuilderAttestClaimAssetCalldata,
  encodeBuilderAttestGetAtCalldata,
  encodeBuilderAttestRevokeAtCalldata,
} from '../src/builderAttestationRegistry.js';
import {
  BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1,
  builderAttestationRegistryCreate2SaltV1Hex,
} from '../src/create2.js';
import { bytesToHex } from '../src/hex.js';

const A = `0x${'11'.repeat(32)}`;
const B = `0x${'22'.repeat(32)}`;
const NOTE = `0x${'33'.repeat(32)}`;
const BUILDER = `0x${'aa'.repeat(32)}`;

describe('builderAttestationRegistry scaffold', () => {
  it('locks CREATE2 salt label', () => {
    expect(builderAttestationRegistryCreate2SaltV1Hex()).toBe(
      BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1_HEX,
    );
    expect(bytesToHex(BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1)).toBe(
      BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1_HEX,
    );
    const label = new TextDecoder().decode(
      BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1.slice(0, 27),
    );
    expect(label).toBe('BOING_BUILDER_ATTEST_REG_V1');
  });

  it('encodes claim / attest / revoke lengths and selectors', () => {
    const claim = encodeBuilderAttestClaimAssetCalldata(A);
    expect(claim.length).toBe(64);
    expect(claim[31]).toBe(SELECTOR_BUILDER_ATTEST_CLAIM_ASSET);

    const attest = encodeBuilderAttestAttestCalldata(A, B, NOTE);
    expect(attest.length).toBe(128);
    expect(attest[31]).toBe(SELECTOR_BUILDER_ATTEST_ATTEST);

    const revoke = encodeBuilderAttestRevokeAtCalldata(3);
    expect(revoke.length).toBe(64);
    expect(revoke[31]).toBe(SELECTOR_BUILDER_ATTEST_REVOKE_AT);
    expect(encodeBuilderAttestGetAtCalldata(0).length).toBe(64);
  });

  it('pins count storage key shape', () => {
    expect(BUILDER_ATTESTATION_REGISTRY_COUNT_KEY_HEX).toBe(
      `0x${'00'.repeat(16)}424f494e4741545400000000ffffffff`,
    );
  });

  it('decodes count and get_attestation_at return data', () => {
    const countWord = '0x' + '00'.repeat(31) + '03';
    expect(decodeBuilderAttestCountReturnData(countWord)).toBe(3n);

    const slot = A.slice(2) + B.slice(2) + BUILDER.slice(2) + NOTE.slice(2);
    const decoded = decodeBuilderAttestGetAtReturnData(`0x${slot}`);
    expect(decoded.collectionHex.toLowerCase()).toBe(A);
    expect(decoded.tokenHex.toLowerCase()).toBe(B);
    expect(decoded.builderHex.toLowerCase()).toBe(BUILDER);
    expect(decoded.noteHashHex.toLowerCase()).toBe(NOTE);
    expect(decoded.active).toBe(true);

    const tomb = '0x' + '00'.repeat(128);
    expect(decodeBuilderAttestGetAtReturnData(tomb).active).toBe(false);
  });
});
