/**
 * Regression: Boing Express signs Ed25519(BLAKE3(utf8(message))) (boing.express src/crypto/keys.ts).
 * The portal verifiers (ownership: passport Discord link/unlink, quests…; set-password) must accept that.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import * as ed from '@noble/ed25519';
import { sha512 } from '@noble/hashes/sha2.js';
import { blake3 } from '@noble/hashes/blake3.js';
import { verifyBoingMessageSignature } from '../functions/api/portal/_lib/boingSignature.js';
import { verifyAccountOwnership } from '../functions/api/portal/_lib/ownership.js';

if (typeof ed.etc.sha512Sync !== 'function') ed.etc.sha512Sync = (...m) => sha512(ed.etc.concatBytes(...m));
const hex = (b) => Array.from(b).map((x) => x.toString(16).padStart(2, '0')).join('');

/** Exact copy of Boing Express signMessage(messageToBytes(msg), sk). */
async function boingExpressSign(message, sk) {
  const sig = await ed.signAsync(blake3(new TextEncoder().encode(message)), sk);
  return '0x' + hex(sig);
}

async function keypair() {
  const sk = ed.utils.randomPrivateKey();
  const pk = await ed.getPublicKeyAsync(sk);
  return { sk, pk, account: '0x' + hex(pk) };
}

function nonceDb(nonce, origin) {
  const rows = { [nonce]: { nonce, origin, expires_at: new Date(Date.now() + 60000).toISOString(), used_at: null } };
  return { prepare(sql) { let args = []; const st = { bind(...a) { args = a; return st; },
    async first() { return rows[args[0]] || null; },
    async run() { const r = rows[args[1]]; if (r && !r.used_at) { r.used_at = args[0]; return { meta: { changes: 1 } }; } return { meta: { changes: 0 } }; } }; return st; } };
}

test('verifies a Boing Express (BLAKE3) signature; rejects tampered message / wrong key', async () => {
  const { sk, pk } = await keypair();
  const msg = 'Set portal password for Boing Portal\nOrigin: https://boing.network\nTimestamp: 2026-11-04T09:50:00.000Z\nNonce: abc';
  const sig = Buffer.from((await boingExpressSign(msg, sk)).slice(2), 'hex');
  assert.equal(verifyBoingMessageSignature(pk, msg, sig), 'blake3');
  assert.equal(verifyBoingMessageSignature(pk, msg + ' ', sig), false);
  const other = await keypair();
  assert.equal(verifyBoingMessageSignature(other.pk, msg, sig), false);
});

test('still accepts legacy raw-message Ed25519 signatures', async () => {
  const { sk, pk } = await keypair();
  const msg = 'hello';
  const sig = await ed.signAsync(new TextEncoder().encode(msg), sk);
  assert.equal(verifyBoingMessageSignature(pk, msg, sig), 'raw');
});

test('passport Discord link message signed by Boing Express passes verifyAccountOwnership', async () => {
  const { sk, account } = await keypair();
  const origin = 'https://boing.network', nonce = 'n0nce_123';
  // Same template as src/pages/testnet/passport/index.astro walletAuth()
  const message = 'Authorize Boing Portal\nAction: discord_link\nAccount: ' + account + '\nOrigin: ' + origin + '\nTimestamp: ' + new Date().toISOString() + '\nNonce: ' + nonce;
  const signature = await boingExpressSign(message, sk);
  const env = { DB: nonceDb(nonce, origin) };
  const ok = await verifyAccountOwnership(env, account, { message, signature }, 'discord_link');
  assert.equal(ok.ok, true, JSON.stringify(ok));
  const replay = await verifyAccountOwnership(env, account, { message, signature }, 'discord_link');
  assert.equal(replay.ok, false);
  const wrongAction = await verifyAccountOwnership({ DB: nonceDb(nonce, origin) }, account, { message, signature }, 'discord_unlink');
  assert.equal(wrongAction.error_code, 'action_mismatch');
});
