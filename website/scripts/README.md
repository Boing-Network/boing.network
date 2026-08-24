# Website scripts

## Animation config (site-wide 3D and mascot)

```bash
python generate_animation_config.py
```

Run from `website/scripts/`. Uses only the Python standard library (`pathlib`, `json`).

Writes `src/styles/motion-config.css` (CSS variables for mascot float/glow, card tilt, link tilt) and `src/data/animations.json`. The theme and `BoingMascot` use these for consistent motion. Re-run after editing `CONFIG` in the script to tune values.

## PDFs (Node)

From `website/`:

```bash
npm run build:pdfs
```

See `generate-pdfs.js` for the list of Markdown sources. Output is `website/public/pdfs/*.pdf`. Written pages such as `/about` embed **SIX-PILLARS.pdf** (and essentials) instead of duplicating the markdown as HTML.

Regenerate only the pillars PDF (applies `pdf-six-pillars.css` and copies into a sibling `boing.observer` checkout):

```bash
node scripts/generate-pdfs.js SIX-PILLARS.md
```

