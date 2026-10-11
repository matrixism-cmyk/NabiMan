const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
let stage = 'startup';
(async () => {
  const fixture = JSON.parse(fs.readFileSync(0, 'utf8'));
  const origin = new URL(process.argv[2]).origin;
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  const errors = [];
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1200 } });
    page.on('pageerror', () => errors.push('Page error'));
    page.on('request', req => { if (req.url().includes(fixture.admin_token) || req.url().includes('token=')) errors.push('Credential URL'); });
    await page.addInitScript(({ token, origin }) => {
      if (window.top !== window || window.location.origin !== origin) return;
      sessionStorage.setItem('nabiman_token', token); sessionStorage.setItem('nabiman_role', 'admin'); localStorage.setItem('nabiman_lang', 'ko');
    }, { token: fixture.admin_token, origin });
    stage = 'pending-inventory';
    await page.goto(origin + '/#mec/mecTenants');
    await page.getByRole('row').filter({ hasText: fixture.tenant_id }).click();
    const pendingResponse = page.waitForResponse(response => response.url().endsWith('/pending-managed-grants'));
    await page.getByRole('button', { name: '외부 프로젝트 회원', exact: true }).click();
    const pending = await pendingResponse;
    const rows = (await pending.json()).data;
    if (pending.status() !== 200 || pending.headers()['cache-control'] !== 'no-store' || rows.length !== 1 || rows[0].local_user_id !== fixture.local_id || !rows[0].operation.binding_id || rows[0].operation.state !== 'unknown_result') throw new Error('Pending identity missing');
    const summary = page.locator('summary').filter({ hasText: fixture.local_id });
    await summary.click();
    await page.getByText(/회수가 완료된 상태는 아닙니다/).waitFor();
    await page.getByText(/현재 유효 접근 권한을 증명하지 않습니다/).waitFor();
    await summary.scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.resolve(__dirname, '../../screenshots/pending-managed-candidate.png'), fullPage: true });
    const denied = await page.request.get(origin + `/api/mec/v1/tenants/${fixture.tenant_id}/pending-managed-grants`, { headers: { Authorization: `Bearer ${fixture.token}` } });
    if (denied.status() !== 403) throw new Error('Scoped manager can read root pending inventory');
    stage = 'policy-disable';
    const review = await page.request.post(origin + `/api/users/${fixture.local_id}/review`, { headers: { Authorization: `Bearer ${fixture.admin_token}` }, data: { status: 'active', memberships: [{ kind: 'tenant', scope_id: fixture.tenant_id, role: 'viewer', enabled: false }] } });
    if (review.status() !== 200 || !(await review.json()).success) throw new Error('Source disable failed');
    stage = 'automatic-confirmation-and-cleanup';
    let revoked = false;
    const deadline = Date.now() + 55000;
    while (Date.now() < deadline) {
      const response = await page.request.get(origin + `/api/mec/v1/tenants/${fixture.tenant_id}/managed-memberships`, { headers: { Authorization: `Bearer ${fixture.admin_token}` } });
      if (response.status() === 200 && (await response.json()).data.some(row => row.operation_id === rows[0].operation.id && row.ownership_state === 'revoked')) { revoked = true; break; }
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    if (!revoked) throw new Error('Known candidate cleanup not confirmed');
    const cleared = await page.request.get(origin + `/api/mec/v1/tenants/${fixture.tenant_id}/pending-managed-grants`, { headers: { Authorization: `Bearer ${fixture.admin_token}` } });
    if ((await cleared.json()).data.length !== 0) throw new Error('Completed candidate remains pending');
    const inventory = await page.request.get(origin + `/api/mec/v1/tenants/${fixture.tenant_id}/members`, { headers: { Authorization: `Bearer ${fixture.admin_token}` } });
    const bindings = (await inventory.json()).data;
    if (bindings.some(binding => binding.user_id === fixture.remote_id) || !bindings.some(binding => binding.role_template === 'project-owner')) throw new Error('Cleanup modified unrelated bindings');
    await page.reload();
    await page.getByRole('row').filter({ hasText: fixture.tenant_id }).click();
    await page.getByRole('button', { name: '외부 프로젝트 회원', exact: true }).click();
    await page.getByText('현재 결과 불명 관리 요청이 없습니다.', { exact: true }).waitFor();
    await page.getByText(/직접 연결 부재 확인됨/).waitFor();
    if (errors.length) throw new Error('Browser errors');
    process.stdout.write('PASS: pending managed candidate UI, highest-only no-store inventory, exact-ID automatic confirmation after policy loss, durable cleanup, no create replay and unrelated binding preservation.\n');
  } finally { await browser.close(); }
})().catch(() => { process.stderr.write('Candidate cleanup browser smoke failed at ' + stage + '\n'); process.exitCode = 1; });
