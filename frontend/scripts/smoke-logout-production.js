const { chromium } = require('playwright');
const path = require('path');
(async () => {
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  const base = process.argv[2] || 'https://man.bebo.kr';
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 }, locale: 'ko-KR' });
    await page.addInitScript(() => {
      sessionStorage.clear();
      localStorage.setItem('nabiman_term_windows', JSON.stringify([{ id: 'tw_logged_out', title: 'local', subtitle: 'local',
        target: { kind: 'local' }, x: 20, y: 20, w: 640, h: 400, z: 11, minimized: true, maximized: false }]));
    });
    const sockets = [];
    const terminalRequests = [];
    const errors = [];
    page.on('websocket', ws => sockets.push(ws.url()));
    page.on('request', req => { if (req.url().includes('/api/terminal')) terminalRequests.push(req.url()); });
    page.on('pageerror', error => errors.push(error.message));
    const response = await page.goto(base + '/', { waitUntil: 'networkidle' });
    if (response.status() !== 200) throw new Error('Login page did not return 200');
    await page.getByRole('button', { name: '회원가입 신청', exact: true }).waitFor();
    if (await page.locator('.term-taskbar, .term-window-layer').count() || sockets.length || terminalRequests.length) throw new Error('Logged-out page loaded terminal UI or connections');
    await page.screenshot({ path: path.resolve(__dirname, '../../screenshots/production-login-20261005.png'), fullPage: true });
    await page.getByRole('button', { name: '회원가입 신청', exact: true }).click();
    await page.getByLabel('가입 아이디', { exact: true }).waitFor();
    if (new URL(page.url()).pathname !== '/signup') throw new Error('Signup button does not link to /signup');
    await page.reload({ waitUntil: 'networkidle' });
    await page.getByRole('heading', { name: '회원가입 신청', exact: true }).waitFor();
    if (await page.locator('.term-taskbar, .term-window-layer').count() || sockets.length || terminalRequests.length || errors.length) throw new Error('Signup regression: ' + JSON.stringify(errors));
    await page.screenshot({ path: path.resolve(__dirname, '../../screenshots/production-signup-20261005.png'), fullPage: true });
    console.log('PASS: production HTTPS login has signup button; /signup supports refresh; saved terminal layout produces no logged-out UI, WebSocket or terminal API request');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exit(1); });
