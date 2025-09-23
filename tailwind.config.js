/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    './index.html',
    './src/**/*.{js,ts,jsx,tsx}',
  ],
  theme: {
    extend: {
      fontFamily: {
        mono: [
          'ui-monospace',
          'SFMono-Regular',
          'Menlo',
          'Monaco',
          'Consolas',
          'Liberation Mono',
          'monospace'
        ],
      },
      maxWidth: {
        '40': '10rem',
      },
      colors: {
        terminal: {
          bg: '#0b1020',
          panel: '#0f172a',
          border: '#1f2937',
          text: '#e5e7eb',
          dim: '#9ca3af',
          accent: '#60a5fa',
        },
      },
      boxShadow: {
        'inner-glow': 'inset 0 0 0 1px rgba(148, 163, 184, 0.15), inset 0 20px 60px rgba(2, 6, 23, 0.6)',
      },
      backgroundImage: {
        'terminal-grid':
          'radial-gradient(circle at 1px 1px, rgba(148,163,184,0.12) 1px, transparent 0)',
      },
      backgroundSize: {
        'grid': '24px 24px',
      },
    },
  },
  plugins: [],
};
