/**
 * Generate PDFs from linked docs for the website.
 * Run from website: npm run build:pdfs
 * Requires: md-to-pdf (npm install md-to-pdf)
 * Output: website/public/pdfs/*.pdf
 *
 * First run may take a few minutes while Puppeteer downloads Chromium.
 */

import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);
const __dirname = path.dirname(fileURLToPath(import.meta.url));

const DOCS_TO_PDF = [
  'TESTNET.md',
  'VIBEMINER-INTEGRATION.md',
  'RUNBOOK.md',
  'BOING-NETWORK-ESSENTIALS.md',
  'QUALITY-ASSURANCE-NETWORK.md',
  'DEVELOPMENT-AND-ENHANCEMENTS.md',
  'BOING-BLOCKCHAIN-DESIGN-PLAN.md',
  'RPC-API-SPEC.md',
  'SECURITY-STANDARDS.md',
  'BUILD-ROADMAP.md',
];

const scriptDir = __dirname;
const websiteDir = path.join(scriptDir, '..');
const repoRoot = path.join(websiteDir, '..');
const docsDir = path.join(repoRoot, 'docs');
const outDir = path.join(websiteDir, 'public', 'pdfs');

async function main() {
  if (!fs.existsSync(docsDir)) {
    console.error('Docs dir not found:', docsDir);
    process.exit(1);
  }
  fs.mkdirSync(outDir, { recursive: true });

  const mdToPdf = require('md-to-pdf').mdToPdf || require('md-to-pdf').default;

  for (const name of DOCS_TO_PDF) {
    const src = path.join(docsDir, name);
    if (!fs.existsSync(src)) {
      console.warn('Skip (not found):', name);
      continue;
    }
    const base = name.replace(/\.md$/i, '');
    const dest = path.join(outDir, base + '.pdf');

    try {
      await mdToPdf(
        { path: src },
        { dest, basedir: docsDir }
      );
      console.log('Generated:', base + '.pdf');
    } catch (err) {
      console.error('Failed', name, err?.message || err);
    }
  }
  console.log('PDF generation done.');
}

main();
