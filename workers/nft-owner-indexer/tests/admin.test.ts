import { beforeEach, describe, expect, it } from 'vitest';
import { adminCache, readCollectionAdmin } from '../src/admin.js';

const collection = '0x' + 'aa'.repeat(32);
const admin = '0x' + '11'.repeat(32);

describe('readCollectionAdmin', () => {
  beforeEach(() => adminCache.clear());

  it('reads the template admin slot and caches a non-zero admin', async () => {
    const keys: string[] = [];
    const read = async (_c: string, k: string) => {
      keys.push(k);
      return { value: admin };
    };
    expect(await readCollectionAdmin(read, collection)).toBe(admin);
    expect(await readCollectionAdmin(read, collection)).toBe(admin);
    expect(keys).toEqual(['0x' + '00'.repeat(31) + 'e0']);
  });

  it('returns null for an empty slot and does not cache it', async () => {
    let calls = 0;
    const read = async () => {
      calls++;
      return { value: '0x' + '00'.repeat(32) };
    };
    expect(await readCollectionAdmin(read, collection)).toBeNull();
    expect(await readCollectionAdmin(read, collection)).toBeNull();
    expect(calls).toBe(2);
  });
});
