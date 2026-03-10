/**
 * Minimal self-host server for boing.network.
 * Serves static files from ../dist and a health check at /api/health.
 * For full portal API (sign-in, nonce, register), wire the handlers in
 * ../functions/api/ with a D1-compatible DB — see README.md.
 */

import { createServer } from 'node:http';
import { readFileSync, existsSync } from 'node:fs';
import { join, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const DIST = process.env.DIST_PATH || join(__dirname, '..', 'dist');
const PORT = Number(process.env.PORT) || 3000;

const MIMES = {
  '.html': 'text/html',
  '.css': 'text/css',
  '.js': 'application/javascript',
  '.json': 'application/json',
  '.ico': 'image/x-icon',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
  '.svg': 'image/svg+xml',
  '.woff2': 'font/woff2',
  '.woff': 'font/woff',
  '.ttf': 'font/ttf',
};

function serveFile(pathname) {
  if (!pathname || pathname === '/') pathname = '/index.html';
  if (!pathname.startsWith('/')) pathname = '/' + pathname;
  const file = join(DIST, pathname.replace(/^\//, ''));
  if (!file.startsWith(DIST) || !existsSync(file)) return null;
  try {
    return readFileSync(file);
  } catch {
    return null;
  }
}

function getMime(pathname) {
  return MIMES[extname(pathname)] || 'application/octet-stream';
}

const server = createServer((req, res) => {
  const url = new URL(req.url || '/', `http://localhost:${PORT}`);
  const pathname = url.pathname;

  if (pathname === '/api/health') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ ok: true, self_hosted: true }));
    return;
  }

  const body = serveFile(pathname);
  if (body) {
    res.writeHead(200, { 'Content-Type': getMime(pathname) });
    res.end(body);
    return;
  }

  res.writeHead(404, { 'Content-Type': 'text/plain' });
  res.end('Not found');
});

server.listen(PORT, () => {
  console.log(`Boing website (self-host) http://localhost:${PORT}`);
  console.log('Static only; for portal API see README.md');
});
