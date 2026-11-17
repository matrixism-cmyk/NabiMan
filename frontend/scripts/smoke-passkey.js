// Passkey registration and sign-in in a real browser, with Chrome's virtual
// authenticator standing in for Face ID / Windows Hello.
//   SMOKE_URL=http://localhost:18097 SMOKE_PASSWORD=... npm run smoke:passkey
// The server needs NABIMAN_PUBLIC_ORIGIN set to the same origin (localhost, not 127.0.0.1).
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://localhost:18097';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const problems = [];
const check = (ok, what) => { console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`); if (!ok) problems.push(what); };

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
  page.on('pageerror', (e) => problems.push(`page error: ${e.message}`));
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('WebAuthn.enable');
  const { authenticatorId } = await cdp.send('WebAuthn.addVirtualAuthenticator', {
    options: { protocol: 'ctap2', transport: 'internal', hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true },
  });

  const signInWithPassword = async () => {
    await page.goto(BASE, { waitUntil: 'networkidle' });
    await page.fill('input[type="text"]', 'admin');
    await page.fill('input[type="password"]', PASSWORD);
    await page.click('button[type="submit"]');
    await page.waitForSelector('.cat-btn', { timeout: 20000 });
  };
  const signOut = async () => {
    await page.evaluate(async () => {
      const token = sessionStorage.getItem('nabiman_token');
      await fetch('/api/auth/logout', { method: 'POST', headers: token ? { Authorization: `Bearer ${token}` } : {} }).catch(() => undefined);
      localStorage.clear(); sessionStorage.clear();
    });
    await page.goto(BASE, { waitUntil: 'networkidle' });
  };

  await signInWithPassword();
  check(true, 'password sign-in');
  // The passkey panel sits under the second-factor panel.
  await page.click('.cat-btn:has-text("관리")');
  await page.locator('button:has-text("2단계 인증"), a:has-text("2단계 인증")').first().click();
  const panel = page.locator('section:has(h2:has-text("패스키"))').first();
  await panel.waitFor({ timeout: 15000 }).catch(() => undefined);
  check(await panel.count() > 0, 'passkey panel is reachable');
  if (await panel.count()) {
    await panel.locator('input[aria-label="패스키 이름"]').fill('시험용 기기');
    await panel.locator('input[type="password"]').fill(PASSWORD);
    await panel.locator('button[type="submit"]').click();
    await panel.locator('td:has-text("시험용 기기")').waitFor({ timeout: 15000 }).catch(() => undefined);
    check(await panel.locator('td:has-text("시험용 기기")').count() === 1, 'a passkey is registered and listed');
  }
  const stored = await cdp.send('WebAuthn.getCredentials', { authenticatorId });
  check(stored.credentials.length === 1 && stored.credentials[0].isResidentCredential, 'the authenticator holds one discoverable credential');
  await page.screenshot({ path: `${process.env.SMOKE_OUT || '.'}/passkey-panel.png` });

  await signOut();
  const button = page.locator('button:has-text("패스키로 로그인")');
  check(await button.count() === 1, 'the login screen offers passkey sign-in');
  await page.screenshot({ path: `${process.env.SMOKE_OUT || '.'}/passkey-login.png` });
  await button.click();
  const inside = await page.waitForSelector('.cat-btn', { timeout: 20000 }).then(() => true).catch(() => false);
  check(inside, 'passkey alone signs in (no username, no password)');

  // Without user verification the server must refuse.
  await signOut();
  await cdp.send('WebAuthn.setUserVerified', { authenticatorId, isUserVerified: false });
  await page.locator('button:has-text("패스키로 로그인")').click();
  await page.waitForTimeout(4000);
  check(await page.locator('.cat-btn').count() === 0, 'no sign-in when the person was not verified');
  await cdp.send('WebAuthn.setUserVerified', { authenticatorId, isUserVerified: true });

  // A passkey the server no longer knows.
  await signInWithPassword();
  const removed = await page.evaluate(async (password) => {
    const token = sessionStorage.getItem('nabiman_token');
    const headers = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` };
    const list = await (await fetch('/api/auth/passkeys', { headers })).json();
    const id = list.data.passkeys[0].id;
    return (await (await fetch('/api/auth/passkeys/delete', { method: 'POST', headers, body: JSON.stringify({ id, password }) })).json()).success;
  }, PASSWORD);
  check(removed, 'the passkey is deleted on the server');
  await signOut();
  await page.locator('button:has-text("패스키로 로그인")').click();
  await page.waitForTimeout(4000);
  check(await page.locator('.cat-btn').count() === 0, 'a deleted passkey no longer signs in');

  await browser.close();
  console.log(problems.length ? `PROBLEMS: ${problems.join(' | ')}` : 'PROBLEMS: none');
  process.exit(problems.length ? 1 : 0);
})().catch((e) => { console.error('FATAL:', e.message); process.exit(2); });
