/**
 * Promo-video branding rasters from official Arc Slab / wordmark SVGs.
 * Output: public/assets/promo/
 * Text on graphics is limited to "Boing Network" — no mainnet/TGE claims.
 */
import { mkdir, readFile, writeFile, copyFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import sharp from 'sharp'

const __dirname = dirname(fileURLToPath(import.meta.url))
const root = join(__dirname, '..')
const publicDir = join(root, 'public')
const assetsDir = join(publicDir, 'assets')
const promoDir = join(assetsDir, 'promo')

const logoSvgPath = join(publicDir, 'logo.svg')
const wordmarkPath = join(assetsDir, 'boing-network-wordmark.svg')

const NAVY = { r: 10, g: 17, b: 40 } // #0A1128
const NAVY_DEEP = { r: 5, g: 10, b: 26 } // #050A1A
const ACCENT = { r: 59, g: 140, b: 255 } // #3B8CFF
const PNG = { compressionLevel: 9 }

async function rasterWordmark(svgBuf, width) {
  return sharp(svgBuf, { density: Math.round((width / 290) * 72 * 4) })
    .resize({ width, kernel: 'lanczos3' })
    .png(PNG)
    .toBuffer()
}

async function rasterMark(svgBuf, size) {
  return sharp(svgBuf, { density: Math.round(((size * 4) / 64) * 72) })
    .resize(size, size, { kernel: 'lanczos3' })
    .png(PNG)
    .toBuffer()
}

/** Recolor filled wordmark paths for light backgrounds (navy type). */
function lightWordmarkSvg(src) {
  return src
    .replace(
      /<linearGradient id="bx-word"[^>]*>[\s\S]*?<\/linearGradient>/,
      `<linearGradient id="bx-word" x1="76" y1="0" x2="290" y2="0" gradientUnits="userSpaceOnUse"><stop stop-color="#0A1128"/><stop offset="1" stop-color="#18244A"/></linearGradient>`,
    )
}

async function writeTransparentWordmark(wordmarkSvg, outPath, width = 1600) {
  const buf = await rasterWordmark(wordmarkSvg, width)
  await writeFile(outPath, buf)
}

async function writeTransparentIcon(logoSvg, outPath, size = 1024) {
  const buf = await rasterMark(logoSvg, size)
  await writeFile(outPath, buf)
}

function gradientBgSvg(w, h) {
  return Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">
  <defs>
    <radialGradient id="glow" cx="50%" cy="42%" r="55%">
      <stop offset="0%" stop-color="#18244A"/>
      <stop offset="55%" stop-color="#0A1128"/>
      <stop offset="100%" stop-color="#050A1A"/>
    </radialGradient>
    <linearGradient id="accentLine" x1="0" y1="0" x2="1" y2="0">
      <stop offset="0%" stop-color="#3B8CFF" stop-opacity="0"/>
      <stop offset="50%" stop-color="#3B8CFF" stop-opacity="0.85"/>
      <stop offset="100%" stop-color="#8DB8FF" stop-opacity="0"/>
    </linearGradient>
  </defs>
  <rect width="${w}" height="${h}" fill="url(#glow)"/>
  <rect x="${Math.round(w * 0.28)}" y="${Math.round(h * 0.62)}" width="${Math.round(w * 0.44)}" height="2" fill="url(#accentLine)" rx="1"/>
</svg>`,
    'utf8',
  )
}

/** Text-only slice of the official wordmark (viewBox starts at letterforms). */
function textOnlyWordmarkSvg(src) {
  return src
    .replace('viewBox="0 0 290 64"', 'viewBox="74 12 210 40"')
    .replace(/<rect[\s\S]*?<\/rect>\s*<path d="M13 45[\s\S]*?<\/path>\s*<circle[\s\S]*?\/>\s*<circle[\s\S]*?\/>\s*/, '')
}

async function rasterTextWordmark(svgBuf, width) {
  // cropped viewBox ≈ 210 units wide
  return sharp(svgBuf, { density: Math.round((width / 210) * 72 * 4) })
    .resize({ width, kernel: 'lanczos3' })
    .png(PNG)
    .toBuffer()
}

async function writeTitleCard(wordmarkSvg, logoSvg, textWmSvg, outPath, w, h, vertical = false) {
  const bg = await sharp(gradientBgSvg(w, h)).png().toBuffer()

  if (vertical) {
    // Large mark above text-only wordmark (avoid duplicating the slab mark)
    const markSize = Math.round(w * 0.32)
    const textWidth = Math.round(w * 0.72)
    const mark = await rasterMark(logoSvg, markSize)
    const text = await rasterTextWordmark(textWmSvg, textWidth)
    const { height: textH } = await sharp(text).metadata()
    const stackH = markSize + 56 + textH
    const top = Math.round((h - stackH) / 2)
    const markLeft = Math.round((w - markSize) / 2)
    const textLeft = Math.round((w - textWidth) / 2)
    await sharp(bg)
      .composite([
        { input: mark, left: markLeft, top },
        { input: text, left: textLeft, top: top + markSize + 56 },
      ])
      .png(PNG)
      .toFile(outPath)
    return
  }

  const wmWidth = Math.round(w * 0.62)
  const wm = await rasterWordmark(wordmarkSvg, wmWidth)
  const { height: wmH } = await sharp(wm).metadata()
  const left = Math.round((w - wmWidth) / 2)
  const top = Math.round((h - wmH) / 2) - Math.round(h * 0.02)
  await sharp(bg)
    .composite([{ input: wm, left, top }])
    .png(PNG)
    .toFile(outPath)
}

/**
 * Lower-third bar: translucent navy strip + mark + wordmark, wide transparent canvas.
 * Designed for overlay on 1920×1080 footage (bar sits in lower region).
 */
async function writeLowerThird(wordmarkSvg, logoSvg, outPath) {
  const W = 1920
  const H = 220
  const barH = 120
  const barY = Math.round((H - barH) / 2)
  const padX = 48
  const markSize = 72

  const barSvg = Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
  <defs>
    <linearGradient id="barFade" x1="0" y1="0" x2="1" y2="0">
      <stop offset="0%" stop-color="#0A1128" stop-opacity="0.92"/>
      <stop offset="72%" stop-color="#0A1128" stop-opacity="0.88"/>
      <stop offset="100%" stop-color="#0A1128" stop-opacity="0"/>
    </linearGradient>
  </defs>
  <rect x="0" y="${barY}" width="${W}" height="${barH}" fill="url(#barFade)"/>
  <rect x="0" y="${barY}" width="4" height="${barH}" fill="#3B8CFF"/>
</svg>`,
    'utf8',
  )

  const bar = await sharp(barSvg).png().toBuffer()
  const mark = await rasterMark(logoSvg, markSize)
  const wm = await rasterWordmark(wordmarkSvg, 720)
  const { height: wmH } = await sharp(wm).metadata()

  const markTop = Math.round((H - markSize) / 2)
  const wmTop = Math.round((H - wmH) / 2)
  const markLeft = padX + 12
  const wmLeft = markLeft + markSize + 28

  await sharp({
    create: { width: W, height: H, channels: 4, background: { r: 0, g: 0, b: 0, alpha: 0 } },
  })
    .composite([
      { input: bar, left: 0, top: 0 },
      { input: mark, left: markLeft, top: markTop },
      { input: wm, left: wmLeft, top: wmTop },
    ])
    .png(PNG)
    .toFile(outPath)
}

async function main() {
  const logoSvg = await readFile(logoSvgPath)
  const wordmarkSrc = await readFile(wordmarkPath, 'utf8')
  const wordmarkSvg = Buffer.from(wordmarkSrc, 'utf8')
  const lightWmSvg = Buffer.from(lightWordmarkSvg(wordmarkSrc), 'utf8')
  const textWmSvg = Buffer.from(textOnlyWordmarkSvg(wordmarkSrc), 'utf8')

  await mkdir(promoDir, { recursive: true })

  const outputs = {
    'wordmark-transparent.png': () => writeTransparentWordmark(wordmarkSvg, join(promoDir, 'wordmark-transparent.png'), 1600),
    'wordmark-dark.png': () => writeTransparentWordmark(wordmarkSvg, join(promoDir, 'wordmark-dark.png'), 1600),
    'wordmark-light.png': () => writeTransparentWordmark(lightWmSvg, join(promoDir, 'wordmark-light.png'), 1600),
    'icon-transparent.png': () => writeTransparentIcon(logoSvg, join(promoDir, 'icon-transparent.png'), 1024),
    'title-card-1920x1080.png': () =>
      writeTitleCard(wordmarkSvg, logoSvg, textWmSvg, join(promoDir, 'title-card-1920x1080.png'), 1920, 1080, false),
    'title-card-1080x1920.png': () =>
      writeTitleCard(wordmarkSvg, logoSvg, textWmSvg, join(promoDir, 'title-card-1080x1920.png'), 1080, 1920, true),
    'lower-third.png': () => writeLowerThird(wordmarkSvg, logoSvg, join(promoDir, 'lower-third.png')),
  }

  // Also keep a compact wide wordmark for lower-thirds / end cards
  outputs['wordmark-wide-1200.png'] = () =>
    writeTransparentWordmark(wordmarkSvg, join(promoDir, 'wordmark-wide-1200.png'), 1200)

  for (const [name, fn] of Object.entries(outputs)) {
    await fn()
    console.log('wrote', name)
  }

  // Manifest for editors
  const manifest = {
    brand: {
      navy: '#0A1128',
      navyDeep: '#050A1A',
      accent: '#3B8CFF',
      soft: '#8DB8FF',
    },
    sources: [
      'public/logo.svg',
      'public/assets/boing-network-wordmark.svg',
    ],
    files: Object.keys(outputs),
    notes: 'On-graphic text is "Boing Network" only. No mainnet/TGE claims.',
  }
  await writeFile(join(promoDir, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n')
  console.log('Promo branding assets written to', promoDir)
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
