import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

/** @type {import('astro').AstroUserConfig} */
export default defineConfig({
  site: 'https://docs.boing.network',
  integrations: [
    starlight({
      title: 'Boing Network Docs',
      description:
        'Guides for using, building on, and running nodes for the Boing Network public testnet. Mainnet is not live.',
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
        {
          tag: 'script',
          attrs: { src: '/js/semantic-links.js', defer: true },
        },
      ],
      sidebar: [
        {
          label: 'Start here',
          items: [
            { label: 'Overview', slug: '' },
            { label: 'Get started', slug: 'start/get-started' },
            { label: 'What is Boing', slug: 'start/what-is-boing' },
            { label: 'Join testnet', slug: 'testnet/join' },
          ],
        },
        {
          label: 'Build apps',
          items: [
            { label: 'JSON-RPC', slug: 'developers/rpc' },
            { label: 'SDK', slug: 'developers/sdk' },
            { label: 'Wallet & dApps', slug: 'developers/wallet-dapps' },
            { label: 'Express, Finance, Observer', slug: 'apps/products' },
          ],
        },
        {
          label: 'Run a node',
          items: [
            { label: 'Run a full node', slug: 'nodes/run-a-node' },
            { label: 'Become a validator', slug: 'nodes/validator' },
            { label: 'Bootnodes', slug: 'nodes/bootnodes' },
            { label: 'Troubleshooting', slug: 'nodes/troubleshooting' },
            { label: 'Host public RPC', slug: 'testnet/operators' },
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
          label: 'Reference',
          items: [
            { label: 'Specifications', slug: 'resources/specialist-specs' },
            { label: 'PDF downloads', slug: 'resources/pdfs' },
          ],
        },
      ],
      lastUpdated: true,
      pagination: true,
    }),
  ],
});
