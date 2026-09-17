import { defineConfig, loadEnv } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';

// CRA-style REACT_APP_* variables keep working, so no application code had to
// change when the build tool did.
const { publicVars, rawPublicVars } = loadEnv({ prefixes: ['REACT_APP_'] });

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
      // loadEnv only defines variables that are actually set, while the app
      // reads REACT_APP_API_URL whether or not anyone configured it. CRA
      // replaced the whole object, so a leftover `process.env` reference is a
      // ReferenceError in the browser — replace the object as well.
      'process.env': JSON.stringify({ ...rawPublicVars, PUBLIC_URL: '' }),
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
