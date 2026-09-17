# NabiMan frontend

React SPA for the NabiMan server console, built with [Rsbuild](https://rsbuild.rs)
(Rspack). The production build lands in `build/`, which the Rust server serves
as static files — see the repository `Makefile`.

## Commands

| Command | What it does |
|---------|--------------|
| `npm start` | Dev server on http://localhost:3000 (add `--host` to expose it) |
| `npm run build` | Production build into `build/` |
| `npm run preview` | Serve the built output locally |
| `npm test` | Jest + Testing Library (`npm test -- --watch` to iterate) |
| `npm run typecheck` | `tsc --noEmit` over `src/` |
| `npm run lint` | ESLint over `src/` (hook rules, unused symbols, TS checks) |
| `npm run smoke` | Loads the built app in Chromium, signs in, opens the remote workspace |

## Configuration

- `rsbuild.config.ts` — build config. `REACT_APP_*` variables are read from
  `.env` files and injected as `process.env.REACT_APP_*`, and
  `process.env.PUBLIC_URL` resolves to the site root.
- `public/index.html` — the HTML template. `<%= assetPrefix %>` stands in for
  the asset root, and Rsbuild injects the script and stylesheet tags.
- `eslint.config.js` — lint rules, replacing the `react-app` preset that used
  to arrive with react-scripts.
- `scripts/check-bundle.js` — runs after every build and fails on anything that
  cannot survive in a browser (an unreplaced `process.env`, a stray
  `%PUBLIC_URL%`).
- `scripts/smoke-browser.js` — the browser check behind `npm run smoke`. Point
  it at a server with `SMOKE_URL` / `SMOKE_PASSWORD`; needs
  `npx playwright install chromium` once.
- `jest.config.js` — test runner: jsdom, `src/setupTests.ts`, CSS and static
  imports stubbed, mock implementations reset between tests.
