/**
 * Verify a Boing wallet message signature.
 *
 * Boing Express `boing_signMessage` (and its `personal_sign` alias) signs Ed25519(BLAKE3(utf8(message)))
 * — see boing.express src/crypto/keys.ts signMessage. The account id is the raw 32-byte Ed25519 public key.
 * Signatures are 0x + 128 hex. For backward compatibility we also accept Ed25519 over the raw UTF-8 bytes.
 */
import * as ed from '@noble/ed25519';
import { sha512 } from '@noble/hashes/sha2.js';
import { blake3 } from '@noble/hashes/blake3.js';

// Same verifier setup as auth/sign-in.js (node:crypto raw-key import is not portable across runtimes).
if (typeof ed.etc.sha512Sync !== 'function') {
  ed.etc.sha512Sync = (...messages) => sha512(ed.etc.concatBytes(...messages));
}

function ed25519Verify(publicKeyBytes, messageBytes, signatureBytes) {
  if (!publicKeyBytes || !signatureBytes || publicKeyBytes.length !== 32 || signatureBytes.length !== 64) return false;
  try {
    return ed.verify(new Uint8Array(signatureBytes), new Uint8Array(messageBytes), new Uint8Array(publicKeyBytes));
  } catch {
    return false;
  }
}

/**
 * @param {Uint8Array} publicKeyBytes 32-byte account id
 * @param {string} message exact string the wallet signed
 * @param {Uint8Array} signatureBytes 64 bytes
 * @returns {false | 'blake3' | 'raw'}
 */
export function verifyBoingMessageSignature(publicKeyBytes, message, signatureBytes) {
  const bytes = Buffer.from(String(message), 'utf8');
  if (ed25519Verify(publicKeyBytes, blake3(new Uint8Array(bytes)), signatureBytes)) return 'blake3';
  if (ed25519Verify(publicKeyBytes, bytes, signatureBytes)) return 'raw';
  return false;
}
