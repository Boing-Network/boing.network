/**
 * Regenerates thematic promo icons, mascot size/frame variants, and network plates.
 * Usage (from website/): node public/assets/promo/generate-thematic-pack.mjs
 * Colors: navy #0A1128, cyan #3B8CFF, soft #8DB8FF. No mainnet/TGE copy.
 */
import sharp from "sharp";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROMO = __dirname;
const ASSETS = path.resolve(__dirname, "..");
const STORE = String.raw`C:\Users\chiku\AppData\Local\Cursor\AgentStores\cursor_agent_stores\bc-f63b2f44-f811-4c2e-a32b-54485f08277c\files\media\promo-branding`;

const NAVY = "#0A1128";
const CYAN = "#3B8CFF";
const SOFT = "#8DB8FF";
const DEEP = "#050A1A";

function ensureDir(d) {
  fs.mkdirSync(d, { recursive: true });
}

async function writeBoth(relPath, buffer) {
  const desktop = path.join(PROMO, relPath);
  ensureDir(path.dirname(desktop));
  await fs.promises.writeFile(desktop, buffer);
  const storePath = path.join(STORE, relPath);
  ensureDir(path.dirname(storePath));
  await fs.promises.writeFile(storePath, buffer);
  return desktop;
}

async function writeBothText(relPath, text) {
  return writeBoth(relPath, Buffer.from(text, "utf8"));
}

function iconSvg(inner, view = 128) {
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${view} ${view}" width="${view}" height="${view}" fill="none" role="img">
  <rect width="${view}" height="${view}" rx="28" fill="${NAVY}"/>
  ${inner}
</svg>`;
}

const ICONS = {
  authenticity: iconSvg(`
    <circle cx="64" cy="64" r="34" stroke="${CYAN}" stroke-width="4"/>
    <path d="M46 65 L58 77 L84 49" stroke="${CYAN}" stroke-width="6" stroke-linecap="round" stroke-linejoin="round"/>
    <circle cx="64" cy="64" r="44" stroke="${SOFT}" stroke-width="2" opacity="0.45"/>
  `),
  decentralization: iconSvg(`
    <circle cx="64" cy="28" r="10" fill="${CYAN}"/>
    <circle cx="28" cy="88" r="10" fill="${CYAN}"/>
    <circle cx="100" cy="88" r="10" fill="${CYAN}"/>
    <circle cx="64" cy="64" r="12" fill="${SOFT}"/>
    <path d="M64 38 L64 52 M54 70 L36 82 M74 70 L92 82" stroke="${CYAN}" stroke-width="3.5" stroke-linecap="round"/>
    <path d="M36 88 L92 88" stroke="${SOFT}" stroke-width="2" opacity="0.5"/>
  `),
  scalability: iconSvg(`
    <path d="M30 86 L52 52 L68 68 L98 34" stroke="${CYAN}" stroke-width="6" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M82 34 H98 V50" stroke="${CYAN}" stroke-width="6" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M28 42 H48 M28 54 H40" stroke="${SOFT}" stroke-width="4" stroke-linecap="round" opacity="0.7"/>
  `),
  security: iconSvg(`
    <path d="M64 24 L96 38 V62 C96 84 78 100 64 106 C50 100 32 84 32 62 V38 Z" fill="${CYAN}" opacity="0.18" stroke="${CYAN}" stroke-width="4" stroke-linejoin="round"/>
    <path d="M52 64 L60 72 L78 52" stroke="${SOFT}" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>
  `),
  transparency: iconSvg(`
    <ellipse cx="64" cy="64" rx="40" ry="24" stroke="${CYAN}" stroke-width="4"/>
    <circle cx="64" cy="64" r="14" fill="${CYAN}"/>
    <circle cx="64" cy="64" r="6" fill="${NAVY}"/>
    <path d="M30 40 C42 28 86 28 98 40" stroke="${SOFT}" stroke-width="3" opacity="0.55" stroke-linecap="round"/>
  `),
  "quality-gate": iconSvg(`
    <rect x="30" y="28" width="68" height="72" rx="10" stroke="${CYAN}" stroke-width="4"/>
    <path d="M30 52 H98" stroke="${SOFT}" stroke-width="3" opacity="0.6"/>
    <circle cx="64" cy="72" r="14" stroke="${CYAN}" stroke-width="4"/>
    <path d="M58 72 L62 76 L72 64" stroke="${SOFT}" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round"/>
  `),
  developers: iconSvg(`
    <path d="M44 44 L28 64 L44 84" stroke="${CYAN}" stroke-width="6" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M84 44 L100 64 L84 84" stroke="${CYAN}" stroke-width="6" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M70 38 L58 90" stroke="${SOFT}" stroke-width="5" stroke-linecap="round"/>
  `),
  community: iconSvg(`
    <circle cx="64" cy="40" r="12" fill="${CYAN}"/>
    <circle cx="38" cy="52" r="9" fill="${SOFT}" opacity="0.85"/>
    <circle cx="90" cy="52" r="9" fill="${SOFT}" opacity="0.85"/>
    <path d="M40 92 C40 76 52 68 64 68 C76 68 88 76 88 92" stroke="${CYAN}" stroke-width="5" stroke-linecap="round"/>
    <path d="M24 96 C24 84 32 78 38 78" stroke="${SOFT}" stroke-width="4" stroke-linecap="round" opacity="0.8"/>
    <path d="M104 96 C104 84 96 78 90 78" stroke="${SOFT}" stroke-width="4" stroke-linecap="round" opacity="0.8"/>
  `),
  operators: iconSvg(`
    <rect x="36" y="30" width="56" height="68" rx="8" stroke="${CYAN}" stroke-width="4"/>
    <circle cx="64" cy="52" r="10" fill="${CYAN}"/>
    <circle cx="64" cy="52" r="4" fill="${NAVY}"/>
    <path d="M48 76 H80 M48 88 H68" stroke="${SOFT}" stroke-width="4" stroke-linecap="round"/>
  `),
  wallet: iconSvg(`
    <rect x="26" y="40" width="76" height="52" rx="10" stroke="${CYAN}" stroke-width="4"/>
    <path d="M26 56 H102" stroke="${SOFT}" stroke-width="3" opacity="0.55"/>
    <rect x="74" y="62" width="28" height="18" rx="6" fill="${CYAN}"/>
    <circle cx="88" cy="71" r="3.5" fill="${NAVY}"/>
  `),
  dex: iconSvg(`
    <path d="M40 48 H78" stroke="${CYAN}" stroke-width="5" stroke-linecap="round"/>
    <path d="M68 36 L80 48 L68 60" stroke="${CYAN}" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M88 80 H50" stroke="${SOFT}" stroke-width="5" stroke-linecap="round"/>
    <path d="M60 68 L48 80 L60 92" stroke="${SOFT}" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>
  `),
  observer: iconSvg(`
    <circle cx="64" cy="64" r="36" stroke="${CYAN}" stroke-width="3" opacity="0.45"/>
    <circle cx="64" cy="64" r="22" stroke="${SOFT}" stroke-width="3" opacity="0.7"/>
    <circle cx="64" cy="64" r="8" fill="${CYAN}"/>
    <path d="M64 28 V36 M64 92 V100 M28 64 H36 M92 64 H100" stroke="${CYAN}" stroke-width="3" stroke-linecap="round"/>
    <ellipse cx="88" cy="40" rx="10" ry="7" fill="${SOFT}" opacity="0.55"/>
  `),
  testnet: iconSvg(`
    <path d="M48 28 H80 V44 C80 52 74 58 64 64 C54 58 48 52 48 44 Z" stroke="${CYAN}" stroke-width="4" stroke-linejoin="round"/>
    <path d="M64 64 V78" stroke="${CYAN}" stroke-width="4" stroke-linecap="round"/>
    <path d="M64 78 C52 78 46 88 52 96 C58 102 70 102 76 96 C82 88 76 78 64 78 Z" fill="${CYAN}" opacity="0.25" stroke="${SOFT}" stroke-width="3"/>
  `),
};

const HEX_MOTIF = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1920 1080" width="1920" height="1080" fill="none">
  <defs>
    <radialGradient id="bg" cx="50%" cy="45%" r="70%">
      <stop offset="0%" stop-color="#0F1A38"/>
      <stop offset="100%" stop-color="${DEEP}"/>
    </radialGradient>
  </defs>
  <rect width="1920" height="1080" fill="url(#bg)"/>
  ${Array.from({ length: 9 }, (_, row) =>
    Array.from({ length: 14 }, (_, col) => {
      const x = 80 + col * 140 + (row % 2 ? 70 : 0);
      const y = 70 + row * 120;
      const op = 0.08 + ((col + row) % 5) * 0.04;
      return `<polygon points="${x},${y - 36} ${x + 32},${y - 18} ${x + 32},${y + 18} ${x},${y + 36} ${x - 32},${y + 18} ${x - 32},${y - 18}" stroke="${CYAN}" stroke-width="2" opacity="${op.toFixed(2)}"/>`;
    }).join("\n")
  ).join("\n")}
  ${[[480, 360], [960, 540], [1440, 320], [700, 780], [1200, 700]]
    .map(
      ([x, y], i) =>
        `<circle cx="${x}" cy="${y}" r="${6 + (i % 3) * 4}" fill="${i % 2 ? CYAN : SOFT}" opacity="${0.35 + i * 0.08}"/>`
    )
    .join("\n")}
</svg>`;

const CHAIN_MOTIF = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1920 1080" width="1920" height="1080" fill="none">
  <defs>
    <linearGradient id="plate" x1="0" y1="0" x2="1920" y2="1080" gradientUnits="userSpaceOnUse">
      <stop stop-color="${DEEP}"/><stop offset="1" stop-color="${NAVY}"/>
    </linearGradient>
  </defs>
  <rect width="1920" height="1080" fill="url(#plate)"/>
  ${[0, 1, 2, 3, 4]
    .map((i) => {
      const x = 280 + i * 280;
      const y = 420 + (i % 2) * 40;
      return `
      <rect x="${x}" y="${y}" width="180" height="90" rx="45" transform="rotate(-28 ${x + 90} ${y + 45})" stroke="${CYAN}" stroke-width="14" opacity="0.55"/>
      <rect x="${x + 110}" y="${y + 70}" width="180" height="90" rx="45" transform="rotate(28 ${x + 200} ${y + 115})" stroke="${SOFT}" stroke-width="14" opacity="0.4"/>`;
    })
    .join("\n")}
  <circle cx="960" cy="540" r="8" fill="${CYAN}"/>
</svg>`;

const PARTICLE_MOTIF = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1920 1080" width="1920" height="1080" fill="none">
  <rect width="1920" height="1080" fill="${DEEP}"/>
  ${Array.from({ length: 80 }, (_, i) => {
    const x = (i * 137 + 41) % 1920;
    const y = (i * 89 + 17) % 1080;
    const r = 2 + (i % 5);
    const op = 0.15 + (i % 7) * 0.08;
    return `<circle cx="${x}" cy="${y}" r="${r}" fill="${i % 3 ? CYAN : SOFT}" opacity="${op.toFixed(2)}"/>`;
  }).join("\n")}
  ${[[400, 300, 900, 700], [1200, 200, 600, 800], [200, 800, 1600, 400]]
    .map(
      ([x1, y1, x2, y2]) =>
        `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="${CYAN}" stroke-width="1.5" opacity="0.2"/>`
    )
    .join("\n")}
</svg>`;

async function rasterSvg(svg, width, height) {
  return sharp(Buffer.from(svg)).resize(width, height).png().toBuffer();
}

async function exportIcons() {
  for (const [name, svg] of Object.entries(ICONS)) {
    await writeBothText(`icons/${name}.svg`, svg);
    for (const size of [512, 1024]) {
      const png = await sharp(Buffer.from(svg))
        .resize(size, size)
        .png()
        .toBuffer();
      await writeBoth(`icons/${name}-${size}.png`, png);
    }
    console.log("icon", name);
  }
}

async function circleMask(size) {
  const r = size / 2;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}"><circle cx="${r}" cy="${r}" r="${r}" fill="#fff"/></svg>`;
  return Buffer.from(svg);
}

async function navyPlate(size, radius = Math.round(size * 0.12)) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}">
    <defs>
      <radialGradient id="p" cx="35%" cy="30%" r="75%">
        <stop offset="0%" stop-color="#152044"/>
        <stop offset="100%" stop-color="${NAVY}"/>
      </radialGradient>
    </defs>
    <rect width="${size}" height="${size}" rx="${radius}" fill="url(#p)"/>
    <rect x="${size * 0.03}" y="${size * 0.03}" width="${size * 0.94}" height="${size * 0.94}" rx="${radius * 0.9}" fill="none" stroke="${CYAN}" stroke-width="${Math.max(2, size * 0.012)}" opacity="0.55"/>
  </svg>`;
  return sharp(Buffer.from(svg)).png().toBuffer();
}

async function exportMascots() {
  const variants = [
    ["default", "mascot-default.png"],
    ["excited", "mascot-excited.png"],
    ["thinking", "mascot-thinking.png"],
    ["winking", "mascot-winking.png"],
  ];
  const sizes = [512, 1024, 2048];

  for (const [slug, file] of variants) {
    const src = path.join(ASSETS, file);
    const input = sharp(src).ensureAlpha();
    const meta = await sharp(src).metadata();

    for (const size of sizes) {
      // Clean transparent square
      const clean = await sharp(src)
        .ensureAlpha()
        .resize(size, size, {
          fit: "contain",
          background: { r: 0, g: 0, b: 0, alpha: 0 },
          kernel: meta.width < size ? sharp.kernel.lanczos3 : sharp.kernel.lanczos3,
        })
        .png()
        .toBuffer();
      await writeBoth(`mascot/${slug}-${size}.png`, clean);

      // Circle crop
      const circBase = await sharp(clean)
        .resize(size, size)
        .composite([{ input: await circleMask(size), blend: "dest-in" }])
        .png()
        .toBuffer();
      await writeBoth(`mascot/${slug}-circle-${size}.png`, circBase);

      // Soft navy plate with mascot inset
      const plate = await navyPlate(size);
      const inset = Math.round(size * 0.12);
      const mascotOnPlate = await sharp(src)
        .ensureAlpha()
        .resize(size - inset * 2, size - inset * 2, {
          fit: "contain",
          background: { r: 0, g: 0, b: 0, alpha: 0 },
        })
        .png()
        .toBuffer();
      const framed = await sharp(plate)
        .composite([{ input: mascotOnPlate, gravity: "center" }])
        .png()
        .toBuffer();
      await writeBoth(`mascot/${slug}-plate-${size}.png`, framed);
    }
    console.log("mascot", slug, meta.width + "x" + meta.height);
  }
}

async function exportNetwork() {
  const hexSvg = HEX_MOTIF;
  const chainSvg = CHAIN_MOTIF;
  const particleSvg = PARTICLE_MOTIF;

  await writeBothText("network/hex-motif-1920x1080.svg", hexSvg);
  await writeBoth("network/hex-motif-1920x1080.png", await rasterSvg(hexSvg, 1920, 1080));

  await writeBothText("network/chain-link-1920x1080.svg", chainSvg);
  await writeBoth("network/chain-link-1920x1080.png", await rasterSvg(chainSvg, 1920, 1080));

  await writeBothText("network/particle-field-1920x1080.svg", particleSvg);
  await writeBoth(
    "network/particle-field-1920x1080.png",
    await rasterSvg(particleSvg, 1920, 1080)
  );

  // Transparent motif strips for overlays (1080 square crops)
  const hexOverlay = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024" fill="none">
  ${Array.from({ length: 8 }, (_, row) =>
    Array.from({ length: 8 }, (_, col) => {
      const x = 80 + col * 120 + (row % 2 ? 60 : 0);
      const y = 80 + row * 110;
      return `<polygon points="${x},${y - 28} ${x + 24},${y - 14} ${x + 24},${y + 14} ${x},${y + 28} ${x - 24},${y + 14} ${x - 24},${y - 14}" stroke="${CYAN}" stroke-width="3" opacity="0.35"/>`;
    }).join("")
  ).join("")}
</svg>`;
  await writeBothText("network/hex-overlay-1024.svg", hexOverlay);
  await writeBoth(
    "network/hex-overlay-1024.png",
    await sharp(Buffer.from(hexOverlay)).png().toBuffer()
  );

  console.log("network motifs done");
}

async function exportSlates() {
  const wordmarkPath = path.join(PROMO, "boing-wordmark-dark-2048.png");
  const mascotPath = path.join(ASSETS, "mascot-excited.png");
  const fallbackMascot = path.join(ASSETS, "mascot-default.png");
  const mascotSrc = fs.existsSync(mascotPath) ? mascotPath : fallbackMascot;

  // End slate 1920x1080 — navy plate, mascot left/center, wordmark
  const endBg = await sharp({
    create: {
      width: 1920,
      height: 1080,
      channels: 4,
      background: { r: 5, g: 10, b: 26, alpha: 1 },
    },
  })
    .png()
    .toBuffer();

  // Soft radial glow via SVG overlay
  const glow = Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080">
    <defs><radialGradient id="g" cx="42%" cy="48%" r="45%">
      <stop offset="0%" stop-color="#3B8CFF" stop-opacity="0.22"/>
      <stop offset="100%" stop-color="#050A1A" stop-opacity="0"/>
    </radialGradient></defs>
    <rect width="1920" height="1080" fill="url(#g)"/>
  </svg>`);

  const mascotEnd = await sharp(mascotSrc)
    .ensureAlpha()
    .resize(620, 620, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .png()
    .toBuffer();

  let wordmark = null;
  if (fs.existsSync(wordmarkPath)) {
    wordmark = await sharp(wordmarkPath)
      .resize({ width: 900, withoutEnlargement: true })
      .png()
      .toBuffer();
  } else {
    const wmSvg = fs.readFileSync(path.join(PROMO, "boing-wordmark-dark.svg"));
    wordmark = await sharp(wmSvg).resize({ width: 900 }).png().toBuffer();
  }

  const endSlate = await sharp(endBg)
    .composite([
      { input: await sharp(glow).png().toBuffer(), top: 0, left: 0 },
      { input: mascotEnd, left: 280, top: 230 },
      { input: wordmark, left: 920, top: 470 },
      {
        input: Buffer.from(
          `<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="8"><rect width="1920" height="8" fill="${CYAN}"/></svg>`
        ),
        top: 1072,
        left: 0,
      },
    ])
    .png()
    .toBuffer();
  await writeBoth("network/end-slate-mascot-1920x1080.png", endSlate);

  // Vertical mascot hero plate 1080x1920
  const vertBg = await sharp({
    create: {
      width: 1080,
      height: 1920,
      channels: 4,
      background: { r: 5, g: 10, b: 26, alpha: 1 },
    },
  })
    .png()
    .toBuffer();

  const vertGlow = Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="1080" height="1920">
    <defs><radialGradient id="g" cx="50%" cy="42%" r="50%">
      <stop offset="0%" stop-color="#3B8CFF" stop-opacity="0.28"/>
      <stop offset="100%" stop-color="#050A1A" stop-opacity="0"/>
    </radialGradient></defs>
    <rect width="1080" height="1920" fill="url(#g)"/>
  </svg>`);

  const mascotHero = await sharp(mascotSrc)
    .ensureAlpha()
    .resize(820, 820, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .png()
    .toBuffer();

  const wordmarkVert = await sharp(wordmark).resize({ width: 720 }).png().toBuffer();

  const hero = await sharp(vertBg)
    .composite([
      { input: await sharp(vertGlow).png().toBuffer(), top: 0, left: 0 },
      { input: mascotHero, left: 130, top: 420 },
      { input: wordmarkVert, left: 180, top: 1320 },
      {
        input: Buffer.from(
          `<svg xmlns="http://www.w3.org/2000/svg" width="1080" height="8"><rect width="1080" height="8" fill="${CYAN}"/></svg>`
        ),
        top: 1912,
        left: 0,
      },
    ])
    .png()
    .toBuffer();
  await writeBoth("network/mascot-hero-1080x1920.png", hero);

  // Also default-mascot end slate variant for softer close
  const mascotDefault = await sharp(path.join(ASSETS, "mascot-default.png"))
    .ensureAlpha()
    .resize(620, 620, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .png()
    .toBuffer();
  const endDefault = await sharp(endBg)
    .composite([
      { input: await sharp(glow).png().toBuffer(), top: 0, left: 0 },
      { input: mascotDefault, left: 280, top: 230 },
      { input: wordmark, left: 920, top: 470 },
    ])
    .png()
    .toBuffer();
  await writeBoth("network/end-slate-default-1920x1080.png", endDefault);

  console.log("slates done");
}

async function main() {
  ensureDir(path.join(PROMO, "icons"));
  ensureDir(path.join(PROMO, "mascot"));
  ensureDir(path.join(PROMO, "network"));
  await exportIcons();
  await exportMascots();
  await exportNetwork();
  await exportSlates();
  console.log("ALL DONE");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
