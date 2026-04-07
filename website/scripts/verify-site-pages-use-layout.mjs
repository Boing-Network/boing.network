/**
 * Ensures every Astro page under src/pages imports the main Layout (stone engraved BG + site chrome).
 * Run: node scripts/verify-site-pages-use-layout.mjs
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const pagesDir = path.join(__dirname, '..', 'src', 'pages');

const LAYOUT_IMPORT_RE =
  /import\s+Layout\s+from\s+['"][^'"]*\/layouts\/Layout\.astro['"]\s*;/;

function walkAstro(dir, out = []) {
  for (const name of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, name.name);
    if (name.isDirectory()) walkAstro(full, out);
    else if (name.name.endsWith('.astro')) out.push(full);
  }
  return out;
}

const pages = walkAstro(pagesDir);
const bad = [];

for (const file of pages) {
  const src = fs.readFileSync(file, 'utf8');
  if (!LAYOUT_IMPORT_RE.test(src)) {
    bad.push(path.relative(path.join(__dirname, '..'), file));
  }
}

if (bad.length) {
  console.error(
    'These pages must import Layout from layouts/Layout.astro (site-wide background + stone overlay):\n',
    bad.map((p) => `  - ${p}`).join('\n'),
  );
  process.exit(1);
}

console.log(`verify-site-pages-use-layout: OK (${pages.length} pages).`);
