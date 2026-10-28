import { defineConfig } from 'astro/config';
import tailwindcss from 'tailwindcss';
import autoprefixer from 'autoprefixer';

// https://astro.build/config
// Tailwind v3 runs as a plain PostCSS plugin (the @astrojs/tailwind integration
// does not support Astro 6+). Base/components/utilities layers are imported once
// from src/styles/tailwind.css in layouts/Layout.astro, which every page uses.
export default defineConfig({
  output: 'static',
  // Astro's HTML compressor drops the space between a line-ending word and a following inline tag
  // ("then<strong>" rendered as "thenregister"). Keep whitespace as authored.
  compressHTML: false,
  site: 'https://boing.network',
  vite: {
    css: {
      postcss: {
        plugins: [tailwindcss(), autoprefixer()],
      },
    },
  },
});
