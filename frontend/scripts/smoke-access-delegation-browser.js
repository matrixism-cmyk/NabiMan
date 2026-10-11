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
  async function pageFor(token, role) {
   const page = await browser.newPage({ viewport: { width: 1440, height: 1200 } });
   page.on('pageerror', () => errors.push('Page error'));
   page.on('request', req => { if (req.url().includes(token) || req.url().includes('token=')) errors.push('Credential URL'); });
   await page.addInitScript(({ token, role, origin }) => { if (window.top !== window || window.location.origin !== origin) return; sessionStorage.setItem('nabiman_token', token); sessionStorage.setItem('nabiman_role', role); localStorage.setItem('nabiman_lang', 'ko'); }, { token, role, origin });
   await page.goto(origin + '/#mec/mecTenants');
   if (role === 'admin') await page.getByRole('row').filter({ hasText: fixture.tenant_id }).click();
   return page;
  }
  const root = await pageFor(fixture.admin_token, 'admin');
  stage = 'highest-policy';
  await root.getByRole('button', { name: '외부 프로젝트 회원', exact: true }).click();
  const policyForm = root.locator('form').filter({ has: root.getByRole('button', { name: '위임 정책 저장', exact: true }) });
  await policyForm.getByLabel('위임 활성화', { exact: true }).check();
  await policyForm.getByRole('checkbox', { name: /^scope-manager/ }).check();
  await policyForm.getByLabel('위임 만료일 (지금부터 최대 30일)', { exact: true }).fill(new Date(Date.now() + 86400000).toISOString().slice(0, 16));
  await policyForm.getByLabel('위임 변경 사유', { exact: true }).fill('승인된 tenant 관리자에게 읽기 역할만 위임합니다');
  await policyForm.getByLabel('현재 tenant ID를 입력해 확인', { exact: true }).fill(fixture.tenant_id);
  await policyForm.getByLabel('위임 대상·역할 범위·만료일과 기존 권한 유지 조건을 검토했습니다.', { exact: true }).check();
  const policyResponse = root.waitForResponse(response => response.url().endsWith('/access-delegation') && response.request().method() === 'PUT');
  await policyForm.getByRole('button', { name: '위임 정책 저장', exact: true }).click();
  const policy = await policyResponse;
  const savedPolicy = (await policy.json()).data;
  if (policy.status() !== 200 || policy.headers()['cache-control'] !== 'no-store' || !savedPolicy.version || savedPolicy.enabled !== true || savedPolicy.allowed_roles.join(',') !== 'read-only') throw new Error('Root delegation invalid');
  await root.getByText('위임 정책과 변경 감사를 함께 저장했습니다.', { exact: true }).waitFor();
  await policyForm.screenshot({ path: path.resolve(__dirname, '../../screenshots/tenant-access-delegation.png') });
  stage = 'delegated-context';
  const manager = await pageFor(fixture.token, 'viewer');
  const contextResponse = manager.waitForResponse(response => response.url().endsWith('/delegated-access-context'));
  await manager.getByRole('row').filter({ hasText: fixture.tenant_id }).getByRole('button', { name: '외부 권한 관리', exact: true }).click();
  const context = await contextResponse;
  const data = (await context.json()).data;
  stage = `context-status-${context.status()}-version-${data?.version === savedPolicy.version}-candidates-${data?.candidates?.length}`;
  if (context.status() !== 200 || context.headers()['cache-control'] !== 'no-store' || data.version !== savedPolicy.version || data.candidates.length !== 1 || data.candidates[0].id !== fixture.local_id) throw new Error('Scoped candidate inventory invalid');
  const form = manager.locator('form').filter({ has: manager.getByRole('button', { name: '관리되는 외부 권한 부여', exact: true }) });
  stage = 'delegated-member-selection';
  await form.getByLabel('권한을 부여할 승인 회원', { exact: true }).selectOption(fixture.local_id);
  stage = 'delegated-identity-preview';
  await form.getByText(fixture.remote_id, { exact: true }).waitFor();
  if (await form.getByRole('option', { name: 'project-owner', exact: true }).count() || await form.getByRole('option', { name: 'project-member', exact: true }).count()) throw new Error('Role ceiling missing');
  stage = 'delegated-role-review';
  await form.getByLabel('권한 부여 사유', { exact: true }).fill('검토된 하위 회원에게 읽기 권한과 자동 회수 승인');
  await form.getByRole('checkbox').nth(0).check();
  await form.getByRole('checkbox').nth(1).check();
  await form.screenshot({ path: path.resolve(__dirname, '../../screenshots/delegated-managed-grant.png') });
  stage = 'delegated-grant';
  const grantResponse = manager.waitForResponse(response => response.url().endsWith('/managed-memberships') && response.request().method() === 'POST');
  await form.getByRole('button', { name: '관리되는 외부 권한 부여', exact: true }).click();
  const grant = await grantResponse;
  const receipt = (await grant.json()).data;
  if (grant.status() !== 201 || grant.headers()['cache-control'] !== 'no-store' || receipt?.local_user_id !== fixture.local_id || receipt?.automatic_revocation !== true || !receipt.binding_id) throw new Error('Delegated grant receipt invalid');
  await manager.getByText('외부 연결과 자동 회수 정책을 함께 저장했습니다.', { exact: true }).waitFor();
  for (const endpoint of [`/api/mec/v1/tenants/${fixture.tenant_id}/access-delegation`, `/api/mec/v1/tenants/${fixture.tenant_id}/membership-operations`, `/api/users/${fixture.local_id}/external-identities/rancher`]) {
   const denied = await manager.request.get(origin + endpoint, { headers: { Authorization: `Bearer ${fixture.token}` } });
   if (denied.status() !== 403) throw new Error('Scoped root-only access allowed');
  }
  stage = 'delegation-disable';
  await policyForm.getByLabel('위임 활성화', { exact: true }).uncheck();
  await policyForm.getByLabel('위임 변경 사유', { exact: true }).fill('새 부여 위임을 해제하고 기존 소속 정책은 유지합니다');
  await policyForm.getByLabel('현재 tenant ID를 입력해 확인', { exact: true }).fill(fixture.tenant_id);
  await policyForm.getByLabel('위임 대상·역할 범위·만료일과 기존 권한 유지 조건을 검토했습니다.', { exact: true }).check();
  const disabledResponse = root.waitForResponse(response => response.url().endsWith('/access-delegation') && response.request().method() === 'PUT');
  await policyForm.getByRole('button', { name: '위임 정책 저장', exact: true }).click();
  if ((await (await disabledResponse).json()).data.enabled !== false) throw new Error('Root disable failed');
  const deniedContext = await manager.request.get(origin + `/api/mec/v1/tenants/${fixture.tenant_id}/delegated-access-context`, { headers: { Authorization: `Bearer ${fixture.token}` } });
  if (deniedContext.status() !== 403) throw new Error('Disabled delegation still visible');
  const blockedGrant = await manager.request.post(origin + `/api/mec/v1/tenants/${fixture.tenant_id}/managed-memberships`, { headers: { Authorization: `Bearer ${fixture.token}` }, data: { expected_delegation_version: savedPolicy.version, expected_realm: data.realm, expected_remote_user_id: fixture.remote_id, local_user_id: fixture.local_id, role_template: 'read-only', reason: 'Old delegation request must not dispatch', automatic_revocation: true, acknowledge_project_role_scope: true } });
  if (blockedGrant.status() !== 403) throw new Error('Disabled delegation can dispatch');
  const retained = await root.request.get(origin + `/api/mec/v1/tenants/${fixture.tenant_id}/managed-memberships`, { headers: { Authorization: `Bearer ${fixture.admin_token}` } });
  if (!(await retained.json()).data.some(record => record.operation_id === receipt.operation_id && record.ownership_state === 'active')) throw new Error('Policy disable unexpectedly revoked an existing source grant');
  stage = 'member-policy-revocation';
  const source = await root.request.post(origin + `/api/users/${fixture.local_id}/review`, { headers: { Authorization: `Bearer ${fixture.admin_token}` }, data: { status: 'active', memberships: [{ kind: 'tenant', scope_id: fixture.tenant_id, role: 'viewer', enabled: false }] } });
  if (source.status() !== 200 || !(await source.json()).success) throw new Error('Source revocation failed');
  const deadline = Date.now() + 55000;
  let revoked = false;
  while (Date.now() < deadline) {
   const response = await root.request.get(origin + `/api/mec/v1/tenants/${fixture.tenant_id}/managed-memberships`, { headers: { Authorization: `Bearer ${fixture.admin_token}` } });
   if ((await response.json()).data.some(record => record.operation_id === receipt.operation_id && record.ownership_state === 'revoked')) { revoked = true; break; }
   await new Promise(resolve => setTimeout(resolve, 1000));
  }
  if (!revoked) throw new Error('Delegated grant source cleanup not confirmed');
  const inventory = await root.request.get(origin + `/api/mec/v1/tenants/${fixture.tenant_id}/members`, { headers: { Authorization: `Bearer ${fixture.admin_token}` } });
  const bindings = (await inventory.json()).data;
  if (bindings.some(binding => binding.user_id === fixture.remote_id) || !bindings.some(binding => binding.role_template === 'project-owner')) throw new Error('Unrelated binding modified');
  if (errors.length) throw new Error('Browser error or credential URL');
  process.stdout.write('PASS: highest-only expiring delegation UI, exact lower-member context, tenant manager grant with role ceiling and durable policy, disabled delegation blocks dispatch but preserves source grants, source revocation cleanup and unrelated binding preservation.\n');
 } finally { await browser.close(); }
})().catch(() => { process.stderr.write('Access delegation browser smoke failed at ' + stage + '\n'); process.exitCode = 1; });
