import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/vite-plugin-svelte').Config} */
const config = {
  preprocess: vitePreprocess(),
  compilerOptions: {
    // Enable runtime checks in development, disable in production
    dev: process.env.NODE_ENV !== 'production',
    // Enable immutable for better performance
    immutable: true,
    // Enable accessors for better compatibility
    accessors: true,
    // Enable CSS optimization
    css: 'injected'
  },
  // Production optimizations
  onwarn: (warning, handler) => {
    // Suppress specific warnings in production
    if (process.env.NODE_ENV === 'production') {
      // Ignore a11y warnings in production (handle separately)
      if (warning.code.startsWith('a11y-')) return;
      // Ignore unused CSS selector warnings
      if (warning.code === 'css-unused-selector') return;
    }
    // Pass through all other warnings
    handler(warning);
  }
};

export default config;