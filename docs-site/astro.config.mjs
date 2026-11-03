import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

/** @type {import('astro').AstroUserConfig} */
export default defineConfig({
  site: 'https://docs.boing.network',
  integrations: [
    starlight({
      title: 'Boing Network Docs',
      description:
        'Developer and operator documentation for Boing Network. Public testnet; mainnet not live. Quality, enforced by the protocol.',
      favicon: '/favicon.svg',
      logo: {
        src: './src/assets/logo.svg',
        alt: 'Boing Network',
        replacesTitle: false,
      },
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/Boing-Network/boing.network',
        },
        {
          icon: 'discord',
          label: 'Discord',
          href: 'https://discord.gg/K3w9nbUw4N',
        },
      ],
      editLink: {
        baseUrl: 'https://github.com/Boing-Network/boing.network/edit/main/docs-site/src/content/docs/',
      },
      customCss: ['./src/styles/custom.css'],
      head: [
        {
          tag: 'meta',
          attrs: { name: 'theme-color', content: '#050A1A' },
        },
      ],
      sidebar: [
        {
          label: 'Start here',
          items: [
            { label: 'Overview', slug: '' },
            { label: 'What is Boing', slug: 'start/what-is-boing' },
            { label: 'Get started', slug: 'start/get-started' },
          ],
        },
        {
          label: 'Protocol',
          items: [
            { label: 'Network & activations', slug: 'protocol/network' },
            { label: 'Fees & denomination', slug: 'protocol/fees' },
            { label: 'Quality Gate', slug: 'protocol/quality-gate' },
            { label: 'Media blobs', slug: 'protocol/media-blobs' },
            { label: 'Native DEX', slug: 'protocol/native-dex' },
          ],
        },
        {
          label: 'Developers',
          items: [
            { label: 'JSON-RPC', slug: 'developers/rpc' },
            { label: 'SDK', slug: 'developers/sdk' },
            { label: 'Wallet & dApps', slug: 'developers/wallet-dapps' },
          ],
        },
        {
          label: 'Apps',
          items: [
            { label: 'Express, Finance, Observer', slug: 'apps/products' },
          ],
        },
        {
          label: 'Testnet & ops',
          items: [
            { label: 'Join testnet', slug: 'testnet/join' },
            { label: 'Operators', slug: 'testnet/operators' },
          ],
        },
        {
          label: 'Resources',
          items: [
            { label: 'PDF downloads', slug: 'resources/pdfs' },
            { label: 'Specialist specs', slug: 'resources/specialist-specs' },
          ],
        },
      ],
      lastUpdated: true,
      pagination: true,
    }),
  ],
});
