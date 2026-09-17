import { defineConfig, loadEnv } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';

// CRA-style REACT_APP_* variables keep working, so no application code had to
// change when the build tool did.
const { publicVars } = loadEnv({ prefixes: ['REACT_APP_'] });

export default defineConfig({
  plugins: [pluginReact()],
  html: {
    template: './public/index.html',
  },
  source: {
    define: {
      ...publicVars,
      // CRA injected this for assets under public/; NabiMan serves them from
      // the site root, so an empty prefix is the correct replacement.
      'process.env.PUBLIC_URL': JSON.stringify(''),
    },
  },
  output: {
    // The Rust server and the deploy scripts read frontend/build, and the
    // static/js · static/css layout matches what CRA produced.
    distPath: { root: 'build' },
  },
  server: {
    port: 3000,
  },
});
