/**
 * Rasterize public/logo.svg for favicons, Apple touch icon, Open Graph, and SEO fallbacks.
 * Run via npm run generate:brand-raster (also runs automatically before astro build).
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

const BRAND_RGB = { r: 10, g: 30, b: 46 }

async function loadLogoSvg() {
  return readFile(logoSvgPath)
}

/** Square PNG with transparent background; logo inset by pad ratio. */
async function writeTransparentIcon(svg, size, outPath, pad = 0.11) {
  const inner = Math.round(size * (1 - 2 * pad))
  const resized = await sharp(svg)
    .resize(inner, inner, {
      fit: 'contain',
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    })
    .png()
    .toBuffer()

  await sharp({
    create: {
      width: size,
      height: size,
      channels: 4,
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    },
  })
    .composite([{ input: resized, gravity: 'centre' }])
    .png()
    .toFile(outPath)
}

/** Square icon on solid brand background (Apple touch, etc.). */
async function writeSolidIcon(svg, size, outPath, pad = 0.12) {
  const inner = Math.round(size * (1 - 2 * pad))
  const resized = await sharp(svg)
    .resize(inner, inner, {
      fit: 'contain',
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    })
    .png()
    .toBuffer()

  await sharp({
    create: {
      width: size,
      height: size,
      channels: 3,
      background: BRAND_RGB,
    },
  })
    .composite([{ input: resized, gravity: 'centre' }])
    .png()
    .toFile(outPath)
}

/** Pack PNG buffers into a multi-size ICO (Vista+ PNG-in-ICO). */
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

async function writeFaviconIco(svg, outPath) {
  const sizes = [16, 32, 48]
  const pngs = []
  for (const size of sizes) {
    const inner = Math.round(size * 0.88)
    const resized = await sharp(svg)
      .resize(inner, inner, {
        fit: 'contain',
        background: { r: 0, g: 0, b: 0, alpha: 0 },
      })
      .png()
      .toBuffer()
    const tiled = await sharp({
      create: {
        width: size,
        height: size,
        channels: 4,
        background: { r: BRAND_RGB.r, g: BRAND_RGB.g, b: BRAND_RGB.b, alpha: 1 },
      },
    })
      .composite([{ input: resized, gravity: 'centre' }])
      .png()
      .toBuffer()
    pngs.push(tiled)
  }
  await mkdir(dirname(outPath), { recursive: true })
  await writeFile(outPath, pngsToIco(pngs))
}

function ogTextOverlaySvg() {
  const esc = (s) =>
    s
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')

  return Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="680" height="400" viewBox="0 0 680 400">
  <text x="0" y="52" fill="#22d3ee" font-family="Segoe UI,system-ui,-apple-system,sans-serif" font-size="52" font-weight="700">${esc('Boing Network')}</text>
  <text x="0" y="118" fill="#94a3b8" font-family="Segoe UI,system-ui,-apple-system,sans-serif" font-size="26">${esc('The DeFi that always bounces back!')}</text>
  <text x="0" y="168" fill="#64748b" font-family="Segoe UI,system-ui,-apple-system,sans-serif" font-size="22">${esc('boing.network')}</text>
</svg>`,
    'utf8',
  )
}

async function writeOgImage(svg, outPath) {
  const W = 1200
  const H = 630

  const bg = await sharp({
    create: {
      width: W,
      height: H,
      channels: 3,
      background: BRAND_RGB,
    },
  })
    .png()
    .toBuffer()

  const logoSize = 360
  const logoBuf = await sharp(svg)
    .resize(logoSize, logoSize, {
      fit: 'contain',
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    })
    .png()
    .toBuffer()

  const textBuf = await sharp(ogTextOverlaySvg()).png().toBuffer()

  const logoLeft = Math.round((W * 0.38 - logoSize) / 2)
  const logoTop = Math.round((H - logoSize) / 2)

  await sharp(bg)
    .composite([
      { input: logoBuf, left: Math.max(48, logoLeft), top: logoTop },
      { input: textBuf, left: Math.round(W * 0.38) + 32, top: Math.round(H / 2 - 200) },
    ])
    .png()
    .toFile(outPath)
}

async function main() {
  const svg = await loadLogoSvg()

  await mkdir(assetsDir, { recursive: true })

  await writeTransparentIcon(svg, 512, join(assetsDir, 'icon-only-transparent.png'))
  await writeTransparentIcon(svg, 192, join(assetsDir, 'logo-192.png'))
  await writeTransparentIcon(svg, 32, join(assetsDir, 'favicon-32x32.png'), 0.06)
  await writeTransparentIcon(svg, 16, join(assetsDir, 'favicon-16x16.png'), 0.06)

  await writeSolidIcon(svg, 180, join(publicDir, 'apple-touch-icon.png'))
  await writeSolidIcon(svg, 512, join(assetsDir, 'logo-512-maskable.png'), 0.18)

  await writeOgImage(svg, join(publicDir, 'og.png'))
  await writeFaviconIco(svg, join(publicDir, 'favicon.ico'))

  console.log('Brand raster assets written: icon-only-transparent.png, favicon-*.png, logo-192.png, logo-512-maskable.png, apple-touch-icon.png, og.png, favicon.ico')
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
