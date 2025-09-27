export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {
      // Production browser support
      overrideBrowserslist: [
        '>1%',
        'last 4 versions',
        'Firefox ESR',
        'not dead'
      ],
      grid: 'autoplace' // Enable CSS Grid prefixing
    },
    // Add cssnano for production minification
    ...(process.env.NODE_ENV === 'production' && {
      cssnano: {
        preset: ['default', {
          discardComments: {
            removeAll: true,
          },
          normalizeWhitespace: true,
          colormin: true,
          convertValues: true,
          reduceIdents: true,
          calc: true,
          orderedValues: true,
          minifySelectors: true,
          mergeRules: true,
          minifyFontValues: true,
          normalizeUrl: true,
          minifyParams: true,
          minifyGradients: true
        }]
      }
    })
  },
};