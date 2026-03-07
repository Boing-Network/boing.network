/**
 * Shared password hashing for portal auth. Uses Node crypto (nodejs_compat).
 * Salt + scrypt hash; verify with timingSafeEqual.
 */
import { randomBytes, scryptSync, timingSafeEqual } from 'node:crypto';

const SALT_BYTES = 16;
const HASH_BYTES = 64;
const SCRYPT_OPTIONS = { N: 16384, r: 8, p: 1 };

/**
 * Hash a password for storage. Returns { saltHex, hashHex }.
 */
export function hashPassword(password) {
  if (!password || typeof password !== 'string' || password.length < 8) {
    throw new Error('Password must be at least 8 characters');
  }
  const salt = randomBytes(SALT_BYTES);
  const hash = scryptSync(password, salt, HASH_BYTES, SCRYPT_OPTIONS);
  return {
    saltHex: salt.toString('hex'),
    hashHex: hash.toString('hex'),
  };
}

/**
 * Verify password against stored salt and hash. Returns true if match.
 */
export function verifyPassword(password, saltHex, hashHex) {
  if (!password || typeof password !== 'string' || !saltHex || !hashHex) {
    return false;
  }
  try {
    const salt = Buffer.from(saltHex, 'hex');
    const storedHash = Buffer.from(hashHex, 'hex');
    if (salt.length !== SALT_BYTES || storedHash.length !== HASH_BYTES) return false;
    const computed = scryptSync(password, salt, HASH_BYTES, SCRYPT_OPTIONS);
    return timingSafeEqual(storedHash, computed);
  } catch {
    return false;
  }
}
