import type { Config } from 'tailwindcss';

export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        // App design tokens
        surface: {
          DEFAULT: '#0F0F0F',
          card: '#1A1A1A',
          elevated: '#242424',
        },
        accent: {
          DEFAULT: '#FF4444',
          hover: '#FF6666',
        },
        emotion: {
          empathy: '#4F8EF7',
          curiosity: '#F7A94F',
          loss: '#F74F4F',
        },
      },
      fontFamily: {
        sans: ['"Noto Sans JP"', 'ui-sans-serif', 'system-ui'],
      },
    },
  },
  plugins: [],
} satisfies Config;
