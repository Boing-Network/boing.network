/**
 * Generate PDFs from linked docs for the website.
 * Run from website: npm run build:pdfs
 * Requires: md-to-pdf (npm install md-to-pdf)
 * Output: website/public/pdfs/*.pdf
 *
 * Mermaid fenced blocks are rendered to SVG in Chromium before print.
 *
 * First run may take a few minutes while Puppeteer downloads Chromium.
 */

import path from 'path';
import fs from 'fs';
import os from 'os';
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
const docsCss = path.join(scriptDir, 'pdf-docs.css');

const MERMAID_CDN = 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.min.js';

function requestedDocs() {
  const args = process.argv.slice(2).map((a) => a.replace(/\.md$/i, ''));
  if (!args.length) return DOCS_TO_PDF;
  return DOCS_TO_PDF.filter((name) => args.includes(name.replace(/\.md$/i, '')));
}

function mermaidFencesToHtml(markdown) {
  return markdown.replace(/```mermaid[ \t]*\r?\n([\s\S]*?)```/g, (_, body) => {
    return `\n\n<pre class="mermaid">\n${body.trim()}\n</pre>\n\n`;
  });
}

function injectMermaidBoot(html) {
  const boot = `
<script src="${MERMAID_CDN}"></script>
<script>
  (async function () {
    try {
      if (window.mermaid) {
        window.mermaid.initialize({
          startOnLoad: false,
          theme: 'neutral',
          securityLevel: 'loose',
          fontFamily: 'Segoe UI, Helvetica, sans-serif',
        });
        document.querySelectorAll('code.language-mermaid').forEach((code) => {
          const pre = code.parentElement;
          if (pre && pre.tagName === 'PRE') {
            pre.classList.add('mermaid');
            pre.textContent = code.textContent;
          }
        });
        if (document.querySelector('.mermaid')) {
          await window.mermaid.run({ querySelector: '.mermaid' });
        }
      }
      document.documentElement.setAttribute('data-mermaid-ready', 'true');
    } catch (err) {
      console.error(err);
      document.documentElement.setAttribute('data-mermaid-ready', 'error');
    }
  })();
</script>
</body>`;
  if (html.includes('</body>')) {
    return html.replace('</body>', boot);
  }
  return html + boot;
}

async function main() {
  if (!fs.existsSync(docsDir)) {
    console.error('Docs dir not found:', docsDir);
    process.exit(1);
  }
  fs.mkdirSync(outDir, { recursive: true });

  const mdToPdf = require('md-to-pdf').mdToPdf || require('md-to-pdf').default;
  const puppeteer = require('puppeteer');
  const docs = requestedDocs();
  if (!docs.length) {
    console.error('No matching docs in DOCS_TO_PDF for:', process.argv.slice(2).join(' '));
    process.exit(1);
  }

  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'boing-pdfs-'));
  const browser = await puppeteer.launch({
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  for (const name of docs) {
    const src = path.join(docsDir, name);
    if (!fs.existsSync(src)) {
      console.warn('Skip (not found):', name);
      continue;
    }
    const base = name.replace(/\.md$/i, '');
    const dest = path.join(outDir, base + '.pdf');
    const isSixPillars = base === 'SIX-PILLARS';
    const processed = mermaidFencesToHtml(fs.readFileSync(src, 'utf8'));
    const tmpSrc = path.join(tmpDir, name);
    const tmpHtml = path.join(tmpDir, base + '.html');
    fs.writeFileSync(tmpSrc, processed, 'utf8');

    try {
      const htmlResult = await mdToPdf(
        { path: tmpSrc },
        {
          dest: tmpHtml,
          basedir: docsDir,
          as_html: true,
          stylesheet: [isSixPillars ? sixPillarsCss : docsCss],
          body_class: ['boing-pdf'],
        }
      );
      const html = injectMermaidBoot(htmlResult?.content || fs.readFileSync(tmpHtml, 'utf8'));
      const page = await browser.newPage();
      await page.setContent(html, { waitUntil: 'networkidle0', timeout: 120000 });
      await page.waitForFunction(
        () => document.documentElement.getAttribute('data-mermaid-ready') === 'true'
          || document.documentElement.getAttribute('data-mermaid-ready') === 'error',
        { timeout: 45000 }
      ).catch(() => {});
      await page.pdf({
        path: dest,
        format: 'Letter',
        printBackground: true,
        timeout: 120000,
        margin: { top: '22mm', right: '18mm', bottom: '20mm', left: '18mm' },
        displayHeaderFooter: true,
        headerTemplate: `<div style="font-size:9px;width:100%;padding:0 18mm;color:#5b6b7c;font-family:Segoe UI,Helvetica,sans-serif;">Boing Network · ${base.replace(/-/g, ' ')}</div>`,
        footerTemplate:
          '<div style="font-size:9px;width:100%;padding:0 18mm;color:#5b6b7c;font-family:Segoe UI,Helvetica,sans-serif;display:flex;justify-content:space-between;"><span>boing.network</span><span>Page <span class="pageNumber"></span> of <span class="totalPages"></span></span></div>',
      });
      await page.close();
      console.log('Generated:', base + '.pdf');

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

  await browser.close();
  try {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  } catch {
    /* ignore */
  }
  console.log('PDF generation done.');
  process.exit(0);
}

main();
