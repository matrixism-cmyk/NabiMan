const fs = require('fs');
const { chromium } = require('playwright');
(async () => {
  const fixture = JSON.parse(fs.readFileSync(0, 'utf8'));
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  try {
    const page = await browser.newPage();
    await page.addInitScript(({ token }) => {
      sessionStorage.setItem('nabiman_token', token);
      localStorage.setItem('nabiman_lang', 'en');
    }, fixture);
    // The host need not run systemd. Stub only the inventory reader, never control/inspection.
    await page.route('**/api/services', route => route.fulfill({ contentType: 'application/json', body: JSON.stringify({
      success: true, message: 'Fixture inventory', data: [{ name: fixture.unit.replace(/\.service$/, ''),
        description: 'Nonexistent helper fixture', load_state: 'not-found', active_state: 'inactive', sub_state: 'dead', enabled: true }],
    }) }));
    let posts = 0;
    page.on('request', request => { if (request.method() === 'POST' && request.url().endsWith('/api/services/action')) posts++; });
    await page.goto(fixture.base+'/#management/services');
    await page.waitForSelector('.app-header');
    if (!(await page.getByRole('heading', { name: 'Service Management', exact: true }).count())) {
      await page.locator('.cat-btn').filter({ hasText: 'Management' }).click();
      await page.getByRole('button', { name: 'Services', exact: true }).click();
    }
    const restart = page.getByRole('button', { name: 'Restart', exact: true });
    await restart.waitFor();
    await page.waitForFunction(() => [...document.querySelectorAll('button')].some(button => button.textContent === 'Restart' && !button.disabled));
    await restart.click();
    await page.getByText(/Control command failed; verify service state/).waitFor();
    const field = page.getByRole('textbox', { name: 'Operation record ID' });
    const confirmed = await field.inputValue();
    await page.getByRole('button', { name: 'Inspect recorded result' }).click();
    await page.getByText(/Control command failed; verify service state/).first().waitFor();
    if (posts !== 1) throw new Error('Inspection replayed the control command');
    await page.route('**/api/services/action', async route => { await route.fetch(); await route.abort('failed'); });
    await restart.click();
    await page.getByText(/No confirmed response. Inspect this operation ID/).waitFor();
    const lost = await field.inputValue();
    if (lost === confirmed) throw new Error('Fresh deliberate operation reused an old ID');
    await page.waitForTimeout(1200);
    if (posts !== 2) throw new Error('Lost response automatically replayed the mutation');
    await page.unroute('**/api/services/action');
    await page.unroute('**/api/services');
    await page.route('**/api/services', route => route.fulfill({ contentType: 'application/json', body: JSON.stringify({ success: false, message: 'Fixture inventory unavailable', data: null }) }));
    await page.reload();
    await page.getByRole('alert').getByText(/Fixture inventory unavailable/).waitFor();
    await page.getByRole('textbox', { name: 'Operation record ID' }).waitFor();
    await page.waitForFunction(id => [...document.querySelectorAll('input')].some(input => input.value === id), lost);
    await page.getByRole('button', { name: 'Inspect recorded result' }).click();
    await page.getByText(/Control command failed; verify service state/).first().waitFor();
    if (posts !== 2) throw new Error('Reload or result recovery replayed a mutation');
    process.stdout.write(JSON.stringify({ confirmed, lost, posts }));
  } finally { await browser.close(); }
})().catch(error => { console.error(error.message); process.exit(1); });
