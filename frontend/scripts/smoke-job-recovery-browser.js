const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
(async () => {
  const fixture = JSON.parse(fs.readFileSync(0, 'utf8'));
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    const streamUrls = [];
    const streamStatuses = [];
    const pageErrors = [];
    page.on('pageerror', error => pageErrors.push(error.message));
    page.on('response', response => { if (response.url().includes('/stream')) streamStatuses.push(response.status()); });
    page.on('request', request => {
      if (request.url().includes('/jobs/') && request.url().includes('/stream')) streamUrls.push(request.url());
    });
    await page.addInitScript(() => { if (location.protocol === 'http:' || location.protocol === 'https:') localStorage.setItem('nabiman_lang', 'ko'); });
    await page.goto(process.argv[2] + '/#mec/mecJobs');
    await page.locator('input[autocomplete="username"]').fill('admin');
    await page.locator('input[autocomplete="current-password"]').fill(fixture.password);
    await page.getByRole('button', { name: /^(로그인|Login|ログイン)$/ }).click();
    const row = page.getByRole('row').filter({ hasText: 'orphan-running' });
    await row.getByText('결과 확인 필요', { exact: true }).waitFor();
    await row.getByRole('button', { name: '기록', exact: true }).click();
    try { await page.locator('li').filter({ hasText: 'delete_namespace' }).waitFor({ timeout: 10000 }); } catch (error) {
      await page.screenshot({ path: path.resolve(__dirname, '../../screenshots/job-recovery-failure.png'), fullPage: true });
      throw new Error(JSON.stringify({ streamStatuses, pageErrors, history: await page.locator('h3').filter({ hasText: 'orphan-running' }).locator('..').locator('..').innerText() }));
    }
    if (await page.getByText('결과 확인 필요', { exact: true }).count() < 3) throw new Error('History did not restore unknown outcome');
    await page.getByRole('alert').filter({ hasText: '외부 자원 상태를 확인한 뒤 복구하세요.' }).waitFor();
    if (!streamUrls.length || streamUrls.some(url => url.includes('token='))) throw new Error('Job history must authenticate without token in URL');
    await row.getByRole('button', { name: '자원 확인', exact: true }).click();
    const inspection = page.getByRole('region', { name: '복구 전 자원 확인', exact: true });
    await inspection.getByText(/데모 데이터입니다/).waitFor();
    if (await inspection.getByText('조회 대상에 없음', { exact: true }).count() !== 3) throw new Error('Recovery inspection did not distinguish recorded absence');
    await inspection.getByText(/소유권 증명이 아닙니다/).waitFor();
    await page.screenshot({ path: path.resolve(__dirname, '../../screenshots/job-recovery.png'), fullPage: true });
    await page.goto(process.argv[2] + '/#mec/mecUsers');
    await page.getByRole('button', { name: '+ 사용자', exact: true }).click();
    await page.getByPlaceholder('ygram-dev').fill('browser-bootstrap');
    await page.getByRole('button', { name: '생성', exact: true }).click();
    const password = page.getByLabel('초기 비밀번호', { exact: true });
    await password.waitFor();
    if (await password.getAttribute('type') !== 'password' || (await password.inputValue()).length !== 24) throw new Error('Initial secret receipt missing or unmasked');
    await page.getByRole('button', { name: '비밀번호 보기', exact: true }).click();
    if (await password.getAttribute('type') !== 'text') throw new Error('Secret reveal failed');
    await page.getByRole('button', { name: '안전하게 보관했습니다', exact: true }).click();
    if (await password.count()) throw new Error('Acknowledged secret remained in the receipt');
    console.log('PASS browser: secure user creation displays a masked initial secret, reveals explicitly and clears on acknowledgement');
    console.log('PASS browser: interrupted job has verification state, persisted step history and header-only stream authentication');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exit(1); });
