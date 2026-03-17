/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './src/**/*.{astro,html,js,jsx,md,mdx,svelte,ts,tsx,vue}',
    './index.html',
  ],
  theme: {
    extend: {
      colors: {
        // Primary Colors
        'ocean-blue': '#0A1E2E',
        'neon-cyan': '#00D9FF',
        'electric-blue': '#0099FF',
        'cosmic-purple': '#6B21A8',
        'soft-white': '#F0F4F8',

        // Secondary Colors
        'bio-green': '#00FF88',
        'warm-gold': '#FFB800',
        'coral-red': '#FF4444',

        // Transparent variants
        'cyan-transparent': 'rgba(0, 217, 255, 0.2)',
        'cyan-transparent-light': 'rgba(0, 217, 255, 0.1)',
        'white-transparent': 'rgba(240, 244, 248, 0.05)',
        'green-transparent': 'rgba(0, 255, 136, 0.15)',
        'gold-transparent': 'rgba(255, 184, 0, 0.15)',
        'red-transparent': 'rgba(255, 68, 68, 0.15)',
      },
      fontFamily: {
        display: ['Poppins', 'sans-serif'],
        body: ['Inter', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace'],
      },
      fontSize: {
        'display-xl': ['3.5rem', { lineHeight: '1.2', letterSpacing: '-0.02em' }],
        'display-lg': ['2.5rem', { lineHeight: '1.3', letterSpacing: '-0.01em' }],
        'display-md': ['1.75rem', { lineHeight: '1.4' }],
        'display-sm': ['1.25rem', { lineHeight: '1.5' }],
        'body-lg': ['1.125rem', { lineHeight: '1.6' }],
        'body-base': ['1rem', { lineHeight: '1.6' }],
        'body-sm': ['0.875rem', { lineHeight: '1.5', letterSpacing: '0.02em' }],
        label: ['0.75rem', { lineHeight: '1.5', letterSpacing: '0.05em' }],
      },
      spacing: {
        xs: '4px',
        sm: '8px',
        md: '16px',
        lg: '24px',
        xl: '32px',
        '2xl': '48px',
        '3xl': '64px',
      },
      borderRadius: {
        sm: '4px',
        md: '8px',
        lg: '12px',
        xl: '16px',
      },
      boxShadow: {
        sm: '0 2px 8px rgba(0, 0, 0, 0.1)',
        md: '0 8px 32px rgba(0, 0, 0, 0.3)',
        lg: '0 16px 48px rgba(0, 0, 0, 0.4)',
        'glow-cyan': '0 0 20px rgba(0, 217, 255, 0.6)',
        'glow-cyan-lg': '0 0 30px rgba(0, 217, 255, 0.8)',
        'glow-blue': '0 0 20px rgba(0, 153, 255, 0.6)',
        'glow-blue-lg': '0 0 30px rgba(0, 153, 255, 0.8)',
      },
      backgroundImage: {
        'gradient-ocean': 'linear-gradient(180deg, #0A1E2E 0%, #0D2E42 100%)',
        'gradient-neon': 'linear-gradient(135deg, #0099FF 0%, #00D9FF 50%, #6B21A8 100%)',
        'gradient-bio': 'linear-gradient(90deg, #00D9FF 0%, #00FF88 100%)',
      },
      animation: {
        'glow-pulse': 'glow-pulse 2s ease-in-out infinite',
        float: 'float 4s ease-in-out infinite',
        shimmer: 'shimmer 2s infinite',
      },
      keyframes: {
        'glow-pulse': {
          '0%, 100%': { boxShadow: '0 0 10px rgba(0, 217, 255, 0.6)' },
          '50%': { boxShadow: '0 0 20px rgba(0, 217, 255, 1)' },
        },
        float: {
          '0%, 100%': { transform: 'translateY(0px)', opacity: '0.3' },
          '50%': { transform: 'translateY(-20px)', opacity: '0.6' },
        },
        shimmer: {
          '0%': { backgroundPosition: '-1000px 0' },
          '100%': { backgroundPosition: '1000px 0' },
        },
      },
      transitionTimingFunction: {
        smooth: 'cubic-bezier(0.4, 0, 0.2, 1)',
        'bounce-ease': 'cubic-bezier(0.34, 1.56, 0.64, 1)',
      },
    },
  },
  plugins: [],
};
