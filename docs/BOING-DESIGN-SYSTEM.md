# Boing Design System — Site Variants

**Version:** 1.0 · **Date:** February 2026

This document is the single source of truth for the Boing design system across **boing.express**, **boing.finance**, and **boing.network**. It defines a shared visual language and three tailored variants so each site expresses its own personality while staying recognizably Boing.

---

## Table of Contents

1. [Overview](#1-overview)
2. [Shared Foundation](#2-shared-foundation)
3. [Variant 1 — boing.express: Aqua Personal](#3-variant-1--boingexpress-aqua-personal)
4. [Variant 2 — boing.finance: Deep Trade](#4-variant-2--boingfinance-deep-trade)
5. [Variant 3 — boing.network: Cosmic Foundation](#5-variant-3--boingnetwork-cosmic-foundation)
6. [Cross-Site Consistency Rules](#6-cross-site-consistency-rules)
7. [Mascot Usage](#7-mascot-usage)
8. [Accessibility (Motion & Contrast)](#8-accessibility-motion--contrast)
9. [Implementation Notes](#9-implementation-notes)
10. [Reference](#10-reference)

---

## 1. Overview

- **Shared base:** Dark backgrounds, Orbitron display type, Inter body type, glassmorphism cards. **boing.network** uses a charcoal stone slab with engraved vein SVG and soft neon accents (no canvas engine or aquatic photo background).
- **Per site:** Distinct accent palette, motion language, and background character.
- **This repo:** Implements the **boing.network** variant (“Cosmic Foundation”). Theme tokens live in `website/src/styles/`; reference prototype (full HTML) is in the design-themes package (see [Reference](#10-reference)).

---

## 2. Shared Foundation

All three variants use the same structural tokens and rules.

### 2.1 Shared Color Tokens

| Token | Value | Usage |
|-------|--------|--------|
| `--boing-black` | `#020408` – `#050c18` | Page background |
| `--boing-navy` | `#060f1e` – `#0a1628` | Section backgrounds |
| `--boing-navy-mid` | `#091828` – `#0d1f3c` | Card backgrounds |
| `--text-primary` | `#f0f9ff` / `#f8fafc` | Headings, primary text |
| `--text-secondary` | `#94a3b8` | Body copy, descriptions |
| `--text-muted` | `#475569` – `#64748b` | Labels, metadata |

### 2.2 Shared Typography

| Role | Font | Weight | Usage |
|------|------|--------|--------|
| Display / Brand | Orbitron | 700–900 | Hero titles, section titles, logo |
| Body | Inter | 300–700 | Body copy, UI text, labels |
| Monospace | JetBrains Mono | 400–600 | Code, prices, data (finance/network) |

### 2.3 Shared Spacing & Radius

| Token | Value |
|-------|--------|
| `--radius-sm` | 6–8px |
| `--radius-md` | 12–14px |
| `--radius-lg` | 18–20px |
| `--radius-xl` | 24–28px |
| `--radius-2xl` | 32px |
| `--radius-pill` | 999px |

### 2.4 Shared Background System

All sites use a layered background:

1. **Base gradient** — deep navy-black radial gradients with accent color tints
2. **Structural layer** — grid (finance), nebula (network), or caustic waves (express)
3. **Star field** — animated twinkling star dots
4. **Shooting stars** — animated streaks using the site’s accent gradient
5. **Atmospheric glow** — bottom ocean/aurora layer for depth

### 2.5 Shared Card System

- `backdrop-filter: blur(12–16px)`
- Semi-transparent background (`rgba(...)` with 0.65–0.80 alpha)
- 1px border using the site’s accent color at 12–20% opacity; 35–45% on hover
- Subtle box-shadow using accent glow

---

## 3. Variant 1 — boing.express: Aqua Personal

| Attribute | Value |
|----------|--------|
| **Personality** | Personal · Secure · Approachable · Trustworthy |
| **Primary accent** | `#00e8c8` (warm teal-cyan) |

- **Background:** Underwater caustic waves, warm teal-cyan shooting stars, calmer star field.
- **Use when:** Building the express wallet/explorer UI.

---

## 4. Variant 2 — boing.finance: Deep Trade

| Attribute | Value |
|----------|--------|
| **Personality** | Professional · Data-driven · Dynamic · Powerful |
| **Primary accent** | `#00e5ff` (electric cyan) + `#00ff88` (profit green) |

- **Background:** 60px grid, data-stream lines, cyan shooting stars.
- **Use when:** Building the finance/DeFi platform UI.

---

## 5. Variant 3 — boing.network: Cosmic Foundation

| Attribute | Value |
|----------|--------|
| **Personality** | Authoritative · Technical · Epic · Foundational |
| **Primary accent** | `#7c3aed` (deep violet) + `#06b6d4` (cosmic cyan) |

### 5.1 Network Accent Palette

| Token | Value | Role |
|-------|--------|------|
| `--network-primary` | `#7c3aed` | Primary brand accent |
| `--network-primary-light` | `#a78bfa` | Text, active states |
| `--network-cyan` | `#06b6d4` | Secondary accent |
| `--network-cyan-light` | `#22d3ee` | Highlights, hover |
| `--network-aurora` | `#0ea5e9` | Gradient blends |
| `--network-nebula` | `#c026d3` | Atmospheric glow |
| `--network-gold` | `#fbbf24` | Code highlights, special |
| `--status-live` | `#22c55e` | Live ecosystem items |
| `--status-building` | `#f59e0b` | In progress |
| `--status-planned` | `#64748b` | Planned |

### 5.2 Background Character (Network)

- Nebula clouds (large blurred radial gradients, slow drift)
- Dense star field (9+ star dots, varying size/opacity/duration)
- Shooting stars: violet → cyan gradient; slower, more dramatic
- Ocean floor: subtle cyan tint at bottom

### 5.3 Button Variants (Network)

| Variant | Style | Usage |
|---------|--------|--------|
| `btn-primary` | Violet → cyan gradient | Primary CTAs (e.g. Join Testnet) |
| `btn-secondary` | Transparent + violet border | Secondary actions |
| `btn-cyan` | Transparent + dashed cyan border | Tertiary/exploratory |
| `btn-ghost` | Semi-transparent white | Quaternary |

### 5.4 Roadmap & Ecosystem Status

- **Complete:** Solid violet fill + glow; badge violet
- **In progress:** Solid cyan + pulse; badge cyan
- **Planned:** Empty circle, slate border; badge slate
- **Ecosystem:** Live (green), Build (amber), Planned (slate)

---

## 6. Cross-Site Consistency Rules

1. **Logo & brand mark**
   - Orbitron Bold for site name.
   - Unique icon per site: ⚡ (express), ◈ (finance), ⬡ (network).
   - Icon uses the site’s primary accent.

2. **Section structure**
   - Pattern: section-eyebrow → section-title → section-subtitle.
   - Eyebrow: 0.72rem, 700, uppercase, 0.15em letter-spacing, accent color.
   - Title: Orbitron, `clamp(1.8rem, 3–3.5vw, 2.6–3rem)`, 700.
   - Subtitle: 1–1.05rem, `--text-secondary`, max-width 520–540px, centered.

3. **Cards**
   - Shared glassmorphism; `transition: all 0.3s ease` on hover.
   - No solid opaque card backgrounds.

4. **Six pillars**
   - All sites reference: Security, Scalability, Decentralization, Authenticity, Transparency, Quality Assurance.

5. **Footer**
   - Tagline: “Authentic. Decentralized. Optimal. Quality-Assured.”
   - Links to other Boing properties.
   - `border-top: 1px solid` with accent at 8–10% opacity.

6. **Responsive breakpoints**
   - `> 1024px`: Full desktop.
   - `≤ 1024px`: Reduced columns, hide sidebar panels.
   - `≤ 640px`: Single column, hide nav links.

---

## 7. Mascot Usage

- **Role:** Unifying brand element across all three sites.
- **Asset:** Friendly robot character (e.g. `mascot-default.png`); glowing yellow eyes in some assets.
- **Placement (boing.network):**
  - Fixed position, bottom-right (e.g. `bottom: 1.5rem; right: 1.5rem`).
  - Size: `clamp(80px, 22vw, 140px)`; smaller on mobile (e.g. 64px).
  - `aria-hidden="true"` when decorative; no pointer-events so it doesn’t block clicks.
- **Variant personality (design intent):**
  - **Express:** Helpful guide, safe and approachable.
  - **Finance:** Energetic, data-forward.
  - **Network:** Explorer/architect of the “cosmic” foundation.
- **Motion:** Subtle float and glow; must respect `prefers-reduced-motion` (see [§8](#8-accessibility-motion--contrast)).

---

## 8. Accessibility (Motion & Contrast)

### 8.1 Reduced Motion

- **Requirement:** All decorative or non-essential motion must respect `prefers-reduced-motion: reduce`.
- **Implementation:** Use `@media (prefers-reduced-motion: reduce)` to:
  - Disable or greatly reduce background animations (nebula drift, star twinkle, shooting stars).
  - Disable mascot float/glow (static pose is fine).
  - Disable 3D card tilt and link tilt; keep only essential hover states (e.g. border/opacity).
  - Set `animation: none` for decorative classes (e.g. `animate-float`, `animate-pulse-glow`, `fade-in-up`); keep final state visible (e.g. `opacity: 1`).
- **Current coverage (this repo):**
  - `website/src/styles/boing-theme.css`: Cards, links, float, pulse, fade-in-up.
  - `website/src/components/BoingMascot.astro`: Mascot float and glow.
  - `website/src/components/EnhancedAnimatedBackground.astro`: BG drift, SVG animations, shooting stars (hidden when reduced).

### 8.2 Contrast & Focus

- Text and interactive elements must meet WCAG 2.1 Level AA where applicable.
- Ensure focus indicators (outline/focus-visible) are visible against the dark theme.

---

## 9. Implementation Notes

### 9.1 File Structure (This Repo)

```
website/
  src/
    styles/
      boing-theme.css          # Base theme (shared + network)
      design-tokens-cosmic.css # Cosmic Foundation tokens (network) — loaded after base
      motion-config.css        # Motion variables
    components/
      BoingMascot.astro
      StoneEngravingOverlay.astro   # Site-wide stone slab + engraved vein + neon field (CSS/SVG)
  src/layouts/
    Layout.astro               # Imports boing-theme.css, design-tokens-cosmic.css; uses StoneEngravingOverlay
docs/
  BOING-DESIGN-SYSTEM.md       # This document
```

The live site applies the Cosmic Foundation palette by importing `design-tokens-cosmic.css` after `boing-theme.css` in `Layout.astro`. To revert to the pre-Cosmic teal/cyan look, remove the `design-tokens-cosmic.css` import.

**Page background:** The site shell uses **`StoneEngravingOverlay.astro`** on every route (cool charcoal stone, micro-grain, SVG “carved” vein network, soft colored neon accents). There is no Canvas/WebGL layer and no aquatic still-image fallback. The layout also adds a body class per route (e.g. `route-index`, `route-about`, `route-docs-getting-started`) so per-page overrides can be applied in CSS if needed. Optional `pageClass="page-landing"` / `page-app` on `Layout` add a light body tint or hex grid; the primary art remains the overlay.

### 9.2 CSS Architecture

- Design tokens in `:root` (in `boing-theme.css` and `design-tokens-cosmic.css`).
- Cosmic Foundation overrides applied so the live site uses violet + cyan for boing.network.
- No external CSS except Google Fonts (Orbitron, Inter, JetBrains Mono).

### 9.3 Extending the System

To add a new Boing property:

1. Choose a new accent color that doesn’t conflict with express/finance/network.
2. Define the variant’s personality (3–4 adjectives).
3. Choose background character (grid, nebula, caustics, etc.).
4. Copy shared foundation tokens; override only accent palette and variant-specific components.
5. Ensure the six pillars are referenced somewhere on the page.
6. Apply mascot and accessibility rules above.

---

## 10. Reference

- **Design system PDF:** *Boing_Design_System_—_Site_Variants.pdf* (external design-themes package).
- **Strategy & audit:** *design_strategy.md.pdf*, *audit_notes.md.pdf*.
- **Network theme prototype:** Full HTML/CSS reference for “Cosmic Foundation” in the design-themes package (`network/index.html`), used as the visual and token reference for this site.

---

## Appendix A: Implementation Compliance

| Area | Spec | Implementation |
|------|------|-----------------|
| Backgrounds | Deep Space Navy #0A0E1A | `--bg-primary` |
| Accents | Teal #00E5CC, Cyan #00B4FF | `--accent-teal`, `--accent-cyan` |
| Typography | Orbitron (display), Inter (body), JetBrains Mono (code) | `boing-theme.css` |
| Cards | Glassmorphism, blur(12px) | `.card`, `--bg-card` |
| Buttons | Gradient teal→cyan, hover glow | `.btn-primary`, `.btn-secondary` |
| Reduced motion | `prefers-reduced-motion` | boing-theme.css, BoingMascot, EnhancedAnimatedBackground |
| Contrast | WCAG AA | Token-based |

Hardcoded colors should use `var(--…)`; meta theme-color remains hex.

---

*Boing Design System · Authentic. Decentralized. Optimal. Quality-Assured.*
