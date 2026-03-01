# Boing Network — Public Assets

Place the following assets in this directory (`/public/assets/`) so they resolve at `/assets/...` on the site.

## Required for full design implementation

| Filename | Used for |
|----------|----------|
| **boing-aquatic-space-bg.png** | Legacy/optional. Same scene as .webp. |
| **boing-aquatic-space-bg.webp** | Fallback background when Canvas engine is unavailable (reduced-motion or engine error). Used by `BoingCanvasBackground.astro`. |
| **icon-only-transparent.png** | Boing icon/logo (transparent). Used in header nav and hero. Single asset for all contexts. |
| **mascot-default.png** | Corner mascot (fixed bottom-right). Default pose. |
| **mascot-excited.png** | Hero section mascot. Energetic pose. |
| **pillar-security.png** | Pillars section — Security. |
| **pillar-scalability.png** | Pillars section — Scalability. |
| **pillar-decentralization.png** | Pillars section — Decentralization. |
| **pillar-authenticity.png** | Pillars section — Authenticity. |
| **pillar-transparency.png** | Pillars section — Transparency. |
| **pillar-quality.png** | Pillars section — Quality Assurance. |

## Optional (for future use)

| Filename | Suggested use |
|----------|----------------|
| **mascot-thinking.png** | FAQ, help, or “considering” UI states. |
| **mascot-winking.png** | Success, confirmation, or playful CTAs. |

## Already in repo

- **hex-grid.svg** — Hexagonal grid for app pages (`.page-app`). No action needed.

## SEO / Metadata

- **og.png** — Open Graph image (1200×630 recommended) for social sharing. Located at `/public/og.png`. Update to match current branding when the logo changes.
