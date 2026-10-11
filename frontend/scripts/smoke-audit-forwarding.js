const fs = require('fs');
const { chromium } = require('playwright');
(async () => {
  const fixture = JSON.parse(fs.readFileSync(0, 'utf8'));
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  try {
    const page = await browser.newPage();
    await page.addInitScript(token => { sessionStorage.setItem('nabiman_token', token); localStorage.setItem('nabiman_lang', 'ko'); }, fixture.token);
    await page.goto(fixture.base + '/#security/audit');
    await page.getByLabel('수신처 이름', { exact: true }).fill('Browser receiver');
    await page.getByLabel('HTTPS 수신 주소', { exact: true }).fill(fixture.receiver);
    await page.getByLabel('Authorization 헤더 · 선택', { exact: true }).fill('Bearer browser-fixture');
    await page.getByRole('button', { name: '사용 중지 상태로 저장', exact: true }).click();
    await page.getByText('사용 중지 상태로 저장했습니다.', { exact: true }).waitFor();
    if (await page.getByLabel('Authorization 헤더 · 선택', { exact: true }).inputValue()) throw new Error('Credential input retained');
    const row = page.getByRole('row').filter({ hasText: 'Browser receiver' });
    await row.getByRole('button', { name: '시험 메시지 추가', exact: true }).click();
    await page.getByText('시험 메시지를 대기열에 저장했습니다. 수신처의 응답 상태를 확인하세요.', { exact: true }).waitFor();
    page.on('dialog', dialog => dialog.accept());
    await row.getByRole('button', { name: '수신처 삭제', exact: true }).click();
    await row.waitFor({ state: 'detached' });
    process.stdout.write('PASS: disabled create, credential clearing, queued-not-delivered probe feedback, reviewed deletion');
  } finally { await browser.close(); }
})().catch(error => { console.error(error.message); process.exit(1); });
