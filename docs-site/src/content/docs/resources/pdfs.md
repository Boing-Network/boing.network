---
title: PDF downloads
description: Printable Boing Network PDFs on boing.network — kept available alongside this docs site.
---

The marketing site continues to host the **PDF pipeline** (`website/scripts/generate-pdfs.js` → `website/public/pdfs/`). This Starlight site does **not** replace or delete those assets. Prefer these downloads for decks, offline reading, and audits.

## Library

| PDF | Link |
|-----|------|
| Whitepaper | [BOING-NETWORK-WHITEPAPER.pdf](https://boing.network/pdfs/BOING-NETWORK-WHITEPAPER.pdf) |
| Essentials (plain) | [BOING-NETWORK-ESSENTIALS.pdf](https://boing.network/pdfs/BOING-NETWORK-ESSENTIALS.pdf) |
| Technical essentials | [BOING-NETWORK-TECHNICAL-ESSENTIALS.pdf](https://boing.network/pdfs/BOING-NETWORK-TECHNICAL-ESSENTIALS.pdf) |
| Six pillars | [SIX-PILLARS.pdf](https://boing.network/pdfs/SIX-PILLARS.pdf) |
| Features timeline | [FEATURES.pdf](https://boing.network/pdfs/FEATURES.pdf) |
| Tokenomics (plain) | [BOING-TOKENOMICS-AND-INCENTIVIZED-TESTNET.pdf](https://boing.network/pdfs/BOING-TOKENOMICS-AND-INCENTIVIZED-TESTNET.pdf) |
| Initial tokenomics | [BOING-INITIAL-TOKENOMICS.pdf](https://boing.network/pdfs/BOING-INITIAL-TOKENOMICS.pdf) |
| RPC API spec | [RPC-API-SPEC.pdf](https://boing.network/pdfs/RPC-API-SPEC.pdf) |
| Testnet | [TESTNET.pdf](https://boing.network/pdfs/TESTNET.pdf) |
| Runbook | [RUNBOOK.pdf](https://boing.network/pdfs/RUNBOOK.pdf) |
| Quality assurance | [QUALITY-ASSURANCE-NETWORK.pdf](https://boing.network/pdfs/QUALITY-ASSURANCE-NETWORK.pdf) |
| QA gate rules | [QA-GATE-RULES.pdf](https://boing.network/pdfs/QA-GATE-RULES.pdf) |
| Security standards | [SECURITY-STANDARDS.pdf](https://boing.network/pdfs/SECURITY-STANDARDS.pdf) |
| Executive summary deck | [Executive-Summary-Pitch-Deck.pdf](https://boing.network/pdfs/Executive-Summary-Pitch-Deck.pdf) |
| Design plan | [BOING-BLOCKCHAIN-DESIGN-PLAN.pdf](https://boing.network/pdfs/BOING-BLOCKCHAIN-DESIGN-PLAN.pdf) |
| Build roadmap | [BUILD-ROADMAP.pdf](https://boing.network/pdfs/BUILD-ROADMAP.pdf) |
| Development & enhancements | [DEVELOPMENT-AND-ENHANCEMENTS.pdf](https://boing.network/pdfs/DEVELOPMENT-AND-ENHANCEMENTS.pdf) |

Browse on the site: [boing.network/resources](https://boing.network/resources) and [boing.network/about](https://boing.network/about).

## Regenerating

From the monorepo:

```bash
cd website
npm run build:pdfs
```

Only regenerate when the **source Markdown** for a PDF intentionally changes. Do not delete `website/public/pdfs/` as part of docs-site work.
