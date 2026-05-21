import { describe, expect, it } from 'vitest';
import {
  extractHttpOrIpfsUrl,
  ipfsUriToGatewayUrl,
  metadataHashWordToFetchUrls,
  resolveImageUrlFromMetadataJson,
  resolveImageUrlFromSources,
} from '../src/metadataMedia.js';
import {
  referenceNftMetadataStorageKey,
  referenceNftTokenIdWordFromU64,
} from '../src/referenceNft.js';

describe('metadataMedia', () => {
  it('rewrites ipfs URIs to gateway URLs', () => {
    expect(ipfsUriToGatewayUrl('ipfs://bafybeigdyrzt')).toBe('https://ipfs.io/ipfs/bafybeigdyrzt');
  });

  it('extracts https URLs from text', () => {
    expect(extractHttpOrIpfsUrl('Logo at https://cdn.example.com/logo.png today')).toBe(
      'https://cdn.example.com/logo.png',
    );
  });

  it('parses inline JSON image fields', () => {
    const json = '{"name":"Demo","image":"ipfs://QmDemo"}';
    expect(resolveImageUrlFromSources(json)).toBe('https://ipfs.io/ipfs/QmDemo');
    expect(resolveImageUrlFromMetadataJson(JSON.parse(json))).toBe('https://ipfs.io/ipfs/QmDemo');
  });

  it('builds ipfs fetch candidates from metadata hash words', () => {
    const urls = metadataHashWordToFetchUrls('0x' + 'ab'.repeat(32));
    expect(urls.some((u) => u.includes('ipfs.io/ipfs/'))).toBe(true);
  });
});

describe('referenceNft storage keys', () => {
  it('derives metadata storage key for u64 token id', () => {
    const tokenId = referenceNftTokenIdWordFromU64(1);
    expect(tokenId.endsWith('01')).toBe(true);
    expect(referenceNftMetadataStorageKey(tokenId)).toMatch(/^0x[0-9a-f]{64}$/i);
  });
});
