import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'path';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1431,
    strictPort: true,
    host: 'localhost',
    // Security headers for development server
    headers: {
      'X-Content-Type-Options': 'nosniff',
      'X-Frame-Options': 'DENY',
      'X-XSS-Protection': '1; mode=block',
      'Referrer-Policy': 'strict-origin-when-cross-origin'
    }
  },
  envPrefix: ['VITE_'],
  resolve: {
    alias: {
      $lib: resolve('./src/lib')
    }
  },
  build: {
    target: process.env.TAURI_PLATFORM == 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_DEBUG ? 'terser' : false, // Use terser for better production minification
    sourcemap: !!process.env.TAURI_DEBUG ? 'inline' : false, // No sourcemaps in production
    outDir: 'dist',
    // Production optimizations
    reportCompressedSize: false, // Disable gzip size reporting for faster builds
    chunkSizeWarningLimit: 1000, // Increase chunk size warning limit
    rollupOptions: {
      input: resolve('./index.html'),
      output: {
        // Enhanced manual chunk splitting for optimal caching and performance
        manualChunks: (id) => {
          if (id.includes('node_modules')) {
            // Framework chunks
            if (id.includes('svelte')) return 'svelte';
            if (id.includes('@tauri-apps')) return 'tauri';

            // UI library chunks
            if (id.includes('lucide')) return 'icons';
            if (id.includes('tailwind')) return 'styles';

            // Utility chunks
            if (id.includes('chart') || id.includes('d3')) return 'charts';
            if (id.includes('date-fns') || id.includes('dayjs')) return 'datetime';

            // Testing libraries (if accidentally included)
            if (id.includes('vitest') || id.includes('test')) return 'test';

            // All other vendor code
            return 'vendor';
          }

          // Application code splitting
          if (id.includes('src/lib/components/pages')) {
            // Split each page into its own chunk for lazy loading
            const pageName = id.split('/').pop()?.replace('.svelte', '');
            return `page-${pageName}`;
          }

          if (id.includes('src/lib/stores')) return 'stores';
          if (id.includes('src/lib/api')) return 'api';
          if (id.includes('src/lib/utils')) return 'utils';

          // Default return for any other files
          return undefined;
        },
        // Use content hash for cache busting
        assetFileNames: 'assets/[name]-[hash][extname]',
        chunkFileNames: 'chunks/[name]-[hash].js',
        entryFileNames: 'entry/[name]-[hash].js'
      }
    },
    // Terser options for production minification
    terserOptions: {
      compress: {
        drop_console: !process.env.TAURI_DEBUG, // Remove console.log in production
        drop_debugger: true,
        pure_funcs: ['console.log', 'console.info', 'console.debug', 'console.trace'],
        passes: 2
      },
      mangle: {
        safari10: true // Support Safari 10
      },
      format: {
        comments: false // Remove all comments
      }
    },
    cssCodeSplit: true, // Enable CSS code splitting
    assetsInlineLimit: 4096 // Inline assets smaller than 4kb
  },
  // Security and optimization
  esbuild: {
    drop: process.env.NODE_ENV === 'production' ? ['console', 'debugger'] : [],
    legalComments: 'none' // Remove legal comments
  },
  optimizeDeps: {
    include: ['svelte', '@tauri-apps/api'], // Pre-bundle critical dependencies
    exclude: ['@tauri-apps/cli'] // Exclude CLI tools from optimization
  }
});