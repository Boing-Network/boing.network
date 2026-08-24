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
  'Executive-Summary-Pitch-Deck.md',
  'TESTNET.md',
  'VIBEMINER-INTEGRATION.md',
  'RUNBOOK.md',
  'SIX-PILLARS.md',
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
const sixPillarsCss = path.join(scriptDir, 'pdf-six-pillars.css');

function requestedDocs() {
  const args = process.argv.slice(2).map((a) => a.replace(/\.md$/i, ''));
  if (!args.length) return DOCS_TO_PDF;
  return DOCS_TO_PDF.filter((name) => args.includes(name.replace(/\.md$/i, '')));
}

async function main() {
  if (!fs.existsSync(docsDir)) {
    console.error('Docs dir not found:', docsDir);
    process.exit(1);
  }
  fs.mkdirSync(outDir, { recursive: true });

  const mdToPdf = require('md-to-pdf').mdToPdf || require('md-to-pdf').default;
  const docs = requestedDocs();
  if (!docs.length) {
    console.error('No matching docs in DOCS_TO_PDF for:', process.argv.slice(2).join(' '));
    process.exit(1);
  }

  for (const name of docs) {
    const src = path.join(docsDir, name);
    if (!fs.existsSync(src)) {
      console.warn('Skip (not found):', name);
      continue;
    }
    const base = name.replace(/\.md$/i, '');
    const dest = path.join(outDir, base + '.pdf');
    const isSixPillars = base === 'SIX-PILLARS';

    try {
      await mdToPdf(
        { path: src },
        {
          dest,
          basedir: docsDir,
          ...(isSixPillars
            ? {
                stylesheet: [sixPillarsCss],
                pdf_options: {
                  format: 'Letter',
                  printBackground: true,
                  margin: { top: '22mm', right: '18mm', bottom: '20mm', left: '18mm' },
                  displayHeaderFooter: true,
                  headerTemplate:
                    '<div style="font-size:9px;width:100%;padding:0 18mm;color:#5b6b7c;font-family:Segoe UI,Helvetica,sans-serif;">Boing Network · Six Pillars</div>',
                  footerTemplate:
                    '<div style="font-size:9px;width:100%;padding:0 18mm;color:#5b6b7c;font-family:Segoe UI,Helvetica,sans-serif;display:flex;justify-content:space-between;"><span>boing.network</span><span>Page <span class="pageNumber"></span> of <span class="totalPages"></span></span></div>',
                },
              }
            : {}),
        }
      );
      console.log('Generated:', base + '.pdf');

      // Keep the explorer copy of written docs in sync when that repo is a sibling checkout.
      if (isSixPillars) {
        const observerPdfDir = path.join(repoRoot, '..', 'boing.observer', 'public', 'pdfs');
        if (fs.existsSync(path.join(repoRoot, '..', 'boing.observer'))) {
          fs.mkdirSync(observerPdfDir, { recursive: true });
          fs.copyFileSync(dest, path.join(observerPdfDir, base + '.pdf'));
          console.log('Copied to observer:', path.join(observerPdfDir, base + '.pdf'));
        }
      }
    } catch (err) {
      console.error('Failed', name, err?.message || err);
    }
  }
  console.log('PDF generation done.');
  process.exit(0);
}

main();
