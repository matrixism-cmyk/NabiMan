const { chromium } = require('playwright');
const path = require('path');
(async () => {
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  const base = process.argv[2];
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    await page.addInitScript(() => {
      if (!localStorage.getItem('logout-regression-seeded')) {
        localStorage.setItem('logout-regression-seeded', '1');
        localStorage.setItem('nabiman_term_windows', JSON.stringify([{ id: 'tw_logout_regression', title: 'local', subtitle: 'local',
          target: { kind: 'local' }, x: 20, y: 20, w: 640, h: 400, z: 11, minimized: true, maximized: false }]));
        localStorage.setItem('nabiman_term_sid_tw_logout_regression', 'pty_does_not_exist');
      }
    });
    const sockets = [];
    page.on('websocket', ws => sockets.push(ws.url()));
    await page.goto(base);
    if (await page.locator('.term-taskbar').count() || sockets.length) throw new Error('Logged-out page restored a terminal');
    await page.goto(base + '/signup');
    await page.getByRole('heading', { name: '회원가입 신청', exact: true }).waitFor();
    await page.getByLabel('가입 아이디', { exact: true }).fill('browser-member');
    await page.getByLabel('가입 비밀번호', { exact: true }).fill('browser-member-password-123');
    await page.getByLabel('비밀번호 확인', { exact: true }).fill('browser-member-password-123');
    await page.getByRole('button', { name: '가입 신청', exact: true }).click();
    await page.getByText('가입 신청이 접수되었습니다. 최고관리자 승인 후 로그인할 수 있습니다.').waitFor();
    await page.getByRole('button', { name: '로그인으로 돌아가기' }).click();
    await page.locator('input[autocomplete="username"]').fill('admin');
    await page.locator('input[autocomplete="current-password"]').fill('isolated-admin-password-123');
    await page.getByRole('button', { name: /^(로그인|Login|ログイン)$/ }).click();
    await page.getByRole('button', { name: /^(로그아웃|Logout|ログアウト)$/ }).waitFor();
    await page.goto(base + '/#security/usermgmt');
    await page.getByRole('heading', { name: '최고관리자 · 회원 승인 및 소속 관리' }).waitFor();
    const row = page.getByRole('row').filter({ hasText: 'browser-member' });
    await row.getByRole('button', { name: '소속 설정 / 승인', exact: true }).click();
    await page.getByLabel('소속 추가', { exact: true }).selectOption({ label: 'tenant · tenant-one' });
    await page.getByLabel('소속 권한', { exact: true }).selectOption('admin');
    await page.getByRole('button', { name: '승인하고 저장', exact: true }).click();
    await row.getByRole('cell', { name: '승인', exact: true }).waitFor();
    await page.screenshot({ path: path.resolve(__dirname, '../../screenshots/membership-superadmin.png'), fullPage: true });
    await page.getByRole('button', { name: /^(로그아웃|Logout|ログアウト)$/ }).click();
    await page.getByRole('button', { name: '회원가입 신청', exact: true }).waitFor();
    if (await page.locator('.term-taskbar').count()) throw new Error('Terminal taskbar survived logout');
    await page.locator('input[autocomplete="username"]').fill('browser-member');
    await page.locator('input[autocomplete="current-password"]').fill('browser-member-password-123');
    await page.getByRole('button', { name: /^(로그인|Login|ログイン)$/ }).click();
    await page.getByRole('heading', { name: '소속 회원 관리', exact: true }).waitFor();
    if (await page.locator('.term-taskbar').count()) throw new Error('Scoped member restored admin terminal windows');
    if (await page.getByRole('button', { name: '소속 설정 / 승인', exact: true }).count()) throw new Error('Delegated user can approve applicants');
    const choices = await page.getByLabel('관리 소속', { exact: true }).locator('option').allTextContents();
    if (choices.some(c => c.includes('tenant-two')) || !choices.some(c => c.includes('tenant-one'))) throw new Error('Incorrect delegated scope choices');
    await page.getByRole('button', { name: /^(소속 관리|Membership|所属管理)$/ }).click();
    await page.getByRole('button', { name: /^(tenant·서버 운영 설정|Tenant & Server Settings|tenant・サーバー運用設定)$/ }).click();
    await page.getByRole('heading', { name: 'tenant · tenant-one · 운영 설정', exact: true }).waitFor();
    await page.getByLabel('소속 회원에게 표시할 공지').fill('tenant-one members only');
    await page.getByLabel('회원 수 제한', { exact: true }).fill('3');
    await page.getByLabel('변경 사유', { exact: true }).fill('Tenant membership policy verification');
    await page.getByRole('button', { name: '운영 설정 저장', exact: true }).click();
    await page.getByRole('status').filter({ hasText: '소속 운영 설정을 저장했습니다' }).waitFor();
    await page.reload();
    await page.getByLabel('소속 회원에게 표시할 공지').waitFor();
    if (await page.getByLabel('소속 회원에게 표시할 공지').inputValue() !== 'tenant-one members only') throw new Error('Tenant policy did not persist');
    const scopeChoices = await page.getByLabel('관리할 소속', { exact: true }).locator('option').allTextContents();
    if (scopeChoices.some(c => c.includes('tenant-two'))) throw new Error('Other tenant leaked into settings');
    await page.getByRole('button', { name: '이 소속 회원 관리', exact: true }).click();
    await page.getByRole('heading', { name: '소속 회원 관리', exact: true }).waitFor();
    if (await page.getByLabel('소속 필터').inputValue() !== 'tenant:tenant-one') throw new Error('Tenant member list was not filtered');
    const memberRow = page.getByRole('row').filter({ hasText: 'member' }).filter({ hasNotText: 'browser-member' });
    await memberRow.getByRole('button', { name: '접근 설정', exact: true }).click();
    await page.getByLabel('소속 접근 허용').uncheck();
    await page.getByRole('button', { name: '접근 설정 저장', exact: true }).click();
    await memberRow.getByText('· 접근 차단', { exact: true }).waitFor();
    await page.screenshot({ path: path.resolve(__dirname, '../../screenshots/membership-tenant-admin.png'), fullPage: true });
    console.log('PASS: direct signup, super-admin tenant approval, logout terminal isolation, scoped admin navigation and member controls');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exit(1); });
