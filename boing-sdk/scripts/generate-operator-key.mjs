#!/usr/bin/env node
/**
 * Generate a fresh **32-byte Ed25519 signing seed** + derived Boing `AccountId` (public).
 *
 *   cd boing-sdk && node scripts/generate-operator-key.mjs
 *   npm run generate-operator-key --prefix boing-sdk
 *
 * **Never** commit `BOING_SECRET_HEX` or paste it into issues/chat. Store it like any private key.
 */
import { randomBytes } from 'node:crypto';
import * as ed25519 from '@noble/ed25519';

const secret = randomBytes(32);
const pub = await ed25519.getPublicKeyAsync(secret);
const hx = (u8) => '0x' + Buffer.from(u8).toString('hex');

console.error(
  "Warning: prints a raw signing seed to stdout — avoid CI logs and terminal scrollback retention (audit #61 item 58).",
);
console.log(
  JSON.stringify(
    {
      BOING_SECRET_HEX: hx(secret),
      BOING_EXPECT_SENDER_HEX: hx(pub),
      next_steps: [
        'Fund BOING_EXPECT_SENDER_HEX on your chain (faucet or genesis).',
        'cargo run -p boing-execution --example print_native_create2_manifest -- ' + hx(pub),
        'node scripts/sync-canonical-testnet-from-manifest.mjs <that-manifest.json>   # from repo root',
        'Deploy pool + DEX programs with BOING_SECRET_HEX set (tutorial npm scripts).',
      ],
    },
    null,
    2,
  ),
);
