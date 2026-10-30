/** Normalize account/contract hex to lowercase 0x-prefixed form. */
export function normalizeHex(s) {
  if (!s || typeof s !== 'string') return '';
  const t = s.trim().toLowerCase();
  return t.startsWith('0x') ? t : '0x' + t;
}

export function hexToBytes(hexStr) {
  const h = String(hexStr || '').replace(/^0x/, '');
  if (!/^[0-9a-f]+$/.test(h) || h.length % 2 !== 0) return null;
  const buf = Buffer.alloc(h.length / 2);
  for (let i = 0; i < h.length; i += 2) {
    buf[i / 2] = parseInt(h.slice(i, i + 2), 16);
  }
  return buf;
}

export function isAccountHex(hex) {
  return typeof hex === 'string' && hex.length === 66 && /^0x[0-9a-f]{64}$/.test(hex);
}
