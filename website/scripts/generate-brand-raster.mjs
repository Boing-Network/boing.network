/**
 * Rasterize the Boing "Arc Slab" mark for favicons, Apple touch / PWA icons and the Open Graph card.
 * - public/logo.svg: full mark (48px and larger rasters)
 * - public/favicon.svg: simplified 32-grid mark (16/32px rasters and favicon.ico layers)
 * - public/assets/boing-network-wordmark.svg: mark + "Boing Network" in Comfortaa Bold, outlined (OG card)
 * Run via npm run generate:brand-raster (also runs automatically before astro build). Output is committed.
 */
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import sharp from 'sharp'

const __dirname = dirname(fileURLToPath(import.meta.url))
const root = join(__dirname, '..')
const publicDir = join(root, 'public')
const assetsDir = join(publicDir, 'assets')
const logoSvgPath = join(publicDir, 'logo.svg')
const smallSvgPath = join(publicDir, 'favicon.svg')
const wordmarkPath = join(assetsDir, 'boing-network-wordmark.svg')

const BRAND_RGB = { r: 5, g: 10, b: 26 } // navy 950 (see BRAND.md)
const PNG = { compressionLevel: 9 }

function pngsToIco(pngBuffers) {
  const count = pngBuffers.length
  const headerSize = 6 + 16 * count
  let offset = headerSize
  const buf = Buffer.alloc(headerSize + pngBuffers.reduce((n, b) => n + b.length, 0))
  buf.writeUInt16LE(0, 0)
  buf.writeUInt16LE(1, 2)
  buf.writeUInt16LE(count, 4)
  let entryPos = 6
  for (const png of pngBuffers) {
    const meta = Buffer.from(png.subarray(16, 24))
    const width = meta.readUInt32BE(0)
    const height = meta.readUInt32BE(4)
    buf.writeUInt8(width >= 256 ? 0 : width, entryPos)
    buf.writeUInt8(height >= 256 ? 0 : height, entryPos + 1)
    buf.writeUInt8(0, entryPos + 2)
    buf.writeUInt8(0, entryPos + 3)
    buf.writeUInt16LE(1, entryPos + 4)
    buf.writeUInt16LE(32, entryPos + 6)
    buf.writeUInt32LE(png.length, entryPos + 8)
    buf.writeUInt32LE(offset, entryPos + 12)
    entryPos += 16
    png.copy(buf, offset)
    offset += png.length
  }
  return buf
}

/** Rasterize at 4x the target size, then downsample (crisp, stable output). */
const raster = (svg, size, viewBox) =>
  sharp(svg, { density: Math.round(((size * 4) / viewBox) * 72) }).resize(size, size, { kernel: 'lanczos3' })

async function writePng(svg, size, viewBox, outPath) {
  await raster(svg, size, viewBox).png(PNG).toFile(outPath)
}

/** Mark centred on the solid brand background (Apple touch / maskable PWA icons). */
async function writeSolidIcon(svg, size, outPath, pad = 0.04) {
  const inner = Math.round(size * (1 - 2 * pad))
  const mark = await raster(svg, inner, 64).png().toBuffer()
  await sharp({ create: { width: size, height: size, channels: 3, background: BRAND_RGB } })
    .composite([{ input: mark, gravity: 'centre' }])
    .png(PNG)
    .toFile(outPath)
}

async function writeFaviconIco(smallSvg, svg, outPath) {
  const pngs = [
    await raster(smallSvg, 16, 32).png(PNG).toBuffer(),
    await raster(smallSvg, 32, 32).png(PNG).toBuffer(),
    await raster(svg, 48, 64).png(PNG).toBuffer(),
  ]
  await mkdir(dirname(outPath), { recursive: true })
  await writeFile(outPath, pngsToIco(pngs))
}

function ogTaglineSvg() {
  return Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="800" height="120" viewBox="0 0 800 120">
  <text x="0" y="40" fill="#94a3b8" font-family="Comfortaa,Segoe UI,system-ui,sans-serif" font-size="30">Quality, enforced by the protocol.</text>
  <text x="0" y="92" fill="#64748b" font-family="Comfortaa,Segoe UI,system-ui,sans-serif" font-size="24">boing.network · Public testnet, mainnet not live</text>
</svg>`,
    'utf8',
  )
}

/** 1200x630: outlined wordmark (mark + Comfortaa Bold paths) with the tagline under the text. */
async function writeOgImage(wordmarkSvg, outPath) {
  const W = 1200
  const H = 630
  const wmWidth = 900
  const wm = await sharp(wordmarkSvg, { density: 900 }).resize({ width: wmWidth }).png().toBuffer()
  const { height: wmHeight } = await sharp(wm).metadata()
  const tag = await sharp(ogTaglineSvg()).png().toBuffer()
  const left = Math.round((W - wmWidth) / 2)
  const top = Math.round(H / 2 - wmHeight / 2 - 50)
  // the text starts at x=76 of the wordmark's 290-unit viewBox
  const textLeft = left + Math.round(wmWidth * (76 / 290))
  await sharp({ create: { width: W, height: H, channels: 3, background: BRAND_RGB } })
    .composite([
      { input: wm, left, top },
      { input: tag, left: textLeft, top: top + wmHeight + 8 },
    ])
    .png(PNG)
    .toFile(outPath)
}

async function main() {
  const svg = await readFile(logoSvgPath)
  const smallSvg = await readFile(smallSvgPath)
  const wordmark = await readFile(wordmarkPath)
  await mkdir(assetsDir, { recursive: true })

  await writePng(svg, 512, 64, join(assetsDir, 'icon-only-transparent.png'))
  await writePng(svg, 192, 64, join(assetsDir, 'logo-192.png'))
  await writePng(smallSvg, 32, 32, join(assetsDir, 'favicon-32x32.png'))
  await writePng(smallSvg, 16, 32, join(assetsDir, 'favicon-16x16.png'))

  await writeSolidIcon(svg, 180, join(publicDir, 'apple-touch-icon.png'))
  await writeSolidIcon(svg, 512, join(assetsDir, 'logo-512-maskable.png'), 0.14)

  await writeOgImage(wordmark, join(publicDir, 'og.png'))
  await writeFaviconIco(smallSvg, svg, join(publicDir, 'favicon.ico'))

  console.log('Brand raster assets written: icon-only-transparent.png, favicon-*.png, logo-192.png, logo-512-maskable.png, apple-touch-icon.png, og.png, favicon.ico')
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
