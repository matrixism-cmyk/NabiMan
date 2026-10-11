const fs = require('fs');
const { chromium } = require('playwright');
(async () => {
  const fixture = JSON.parse(fs.readFileSync(0, 'utf8'));
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  try {
    const page = await browser.newPage();
    await page.addInitScript(token => { sessionStorage.setItem('nabiman_token', token); localStorage.setItem('nabiman_lang', 'ko'); }, fixture.token);
    await page.goto(fixture.base + '/#security/audit');
    await page.getByRole('heading', { name: '기록 저장 상태와 원본 검사', exact: true }).waitFor();
    if (await page.getByRole('button', { name: '감사 로그 삭제', exact: true }).count()) throw new Error('Destructive clear remains exposed');
    await page.getByRole('button', { name: '보관된 감사 파일 검사', exact: true }).click();
    await page.getByText(/기록 무결성 확인 필요/).waitFor();
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'audit.jsonl.1', exact: true }) });
    await row.getByRole('button', { name: '내보내기 준비', exact: true }).click();
    await page.getByLabel('감사 내보내기 미리보기 · 처음 20건', { exact: true }).waitFor();
    const preview = await page.getByLabel('감사 내보내기 미리보기 · 처음 20건', { exact: true }).textContent();
    if (preview.includes('SECRET_CAP') || preview.includes('SECRET_QUERY')) throw new Error('Export exposes capability');
    const download = page.getByRole('button', { name: '검토한 감사 JSON 다운로드', exact: true });
    if (!await download.isDisabled()) throw new Error('Export review missing');
    await page.getByLabel('포함 범위와 민감 정보 주의사항을 확인했습니다', { exact: true }).check();
    const pending = page.waitForEvent('download'); await download.click();
    const file = await pending; const bundle = JSON.parse(fs.readFileSync(await file.path(), 'utf8'));
    if (!bundle.sanitized || !bundle.original_chain_not_reconstructible || bundle.segment !== 'audit.jsonl.1' || bundle.verification.unhashed_entries !== 1) throw new Error('Export evidence invalid');
    process.stdout.write('PASS: original-file findings and reviewed sanitized export');
  } finally { await browser.close(); }
})().catch(error => { console.error(error.message); process.exit(1); });
