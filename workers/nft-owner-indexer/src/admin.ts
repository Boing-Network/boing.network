import { isZeroHex64, normalizeHex64 } from './cors.js';

/** Reference NFT template admin slot (`ref_nft_collection_admin_key`: 31 zero bytes then 0xe0). */
const REF_NFT_ADMIN_KEY = `0x${'00'.repeat(31)}e0`;

/**
 * Admin per collection. The template has no admin transfer, so a non-zero admin never changes and is
 * cached for the isolate's lifetime. A zero slot is not cached (the first call may set it later).
 */
export const adminCache = new Map<string, string>();

export type StorageReader = (contractHex: string, keyHex: string) => Promise<unknown>;

export async function readCollectionAdmin(read: StorageReader, collectionHex: string): Promise<string | null> {
  const hit = adminCache.get(collectionHex);
  if (hit) return hit;
  const res = await read(collectionHex, REF_NFT_ADMIN_KEY);
  const raw = res && typeof res === 'object' ? (res as { value?: unknown }).value : res;
  const admin = normalizeHex64(raw);
  if (!admin || isZeroHex64(admin)) return null;
  adminCache.set(collectionHex, admin);
  return admin;
}

