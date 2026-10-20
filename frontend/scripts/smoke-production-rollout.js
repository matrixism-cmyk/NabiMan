const fs = require('fs');
const path = require('path');
const { chromium } = require('playwright');
const { autoConfirm, answerConfirm } = require('./confirm-modal');

(async () => {
  const fixture = JSON.parse(fs.readFileSync(0, 'utf8'));
  const browser = await chromium.launch({ headless: true, executablePath: fixture.chromiumPath, args: ['--no-sandbox'] });
  const errors = [];
  let probeSession;
  const http = await browser.newContext();
  const headers = { Authorization: 'Bearer ' + fixture.token };
  const known = new Set(fixture.sessionIds);
  try {
    const anonymous = await browser.newContext();
    const page = await anonymous.newPage({ viewport: { width: 1440, height: 1000 } });
    await page.addInitScript(() => {
      sessionStorage.clear(); localStorage.setItem('nabiman_lang', 'ko');
      localStorage.setItem('nabiman_term_windows', JSON.stringify([{ id: 'old-login-window', title: 'local',
        target: { kind: 'local' }, x: 20, y: 20, w: 640, h: 400, z: 1, minimized: true }]));
    });
    const terminalRequests = [];
    page.on('pageerror', error => errors.push(error.name));
    page.on('request', request => { if (new URL(request.url()).pathname.startsWith('/api/terminal')) terminalRequests.push(true); });
    const response = await page.goto(fixture.base + '/', { waitUntil: 'networkidle' });
    if (response.status() !== 200) throw new Error('Public login page unavailable');
    await page.getByRole('button', { name: '회원가입 신청', exact: true }).waitFor();
    await page.screenshot({ path: path.join(fixture.evidence, 'login.png'), fullPage: true });
    await page.getByRole('button', { name: '회원가입 신청', exact: true }).click();
    await page.getByLabel('가입 아이디', { exact: true }).waitFor();
    await page.reload({ waitUntil: 'networkidle' });
    const input = page.getByLabel('가입 아이디', { exact: true }); await input.waitFor();
    const alignment = await input.evaluate(element => getComputedStyle(element).textAlign);
    if (!['left', 'start'].includes(alignment)) throw new Error('Registration field alignment regressed');
    if (await page.locator('.term-taskbar, .term-window-layer').count() || terminalRequests.length) throw new Error('Logged-out page created terminal UI or requests');
    await page.screenshot({ path: path.join(fixture.evidence, 'signup.png'), fullPage: true });
    await anonymous.close();

    const admin = await browser.newContext();
    const member = await admin.newPage({ viewport: { width: 1440, height: 1000 } });
    member.on('pageerror', error => errors.push(error.name));
    await member.addInitScript(token => { sessionStorage.setItem('nabiman_token', token); localStorage.setItem('nabiman_lang', 'ko'); }, fixture.token);
    await member.goto(fixture.base + '/#administration/usermgmt');
    await member.getByRole('heading', { name: '최고관리자 · 회원 승인 및 소속 관리', exact: true }).waitFor();
    await member.screenshot({ path: path.join(fixture.evidence, 'members.png'), fullPage: true });
    await member.goto(fixture.base + '/#administration/scopeSettings');
    await member.getByRole('heading', { name: 'tenant·서버 운영 설정', exact: true }).waitFor();
    await member.getByLabel('관리할 소속').waitFor();
    await member.getByLabel('소속 회원의 2단계 인증 필수', { exact: true }).waitFor();
    await member.getByLabel('변경 사유', { exact: true }).waitFor();
    await member.getByRole('heading', { name: '최근 정책 변경 이력', exact: true }).waitFor();
    const [scopeKind, scopeId] = (await member.getByLabel('관리할 소속').inputValue()).split(':');
    const history = await http.request.get(`${fixture.base}/api/scope-settings/${scopeKind}/${scopeId}/history`, { headers });
    if (history.status() !== 200 || history.headers()['cache-control'] !== 'no-store' || !Array.isArray((await history.json()).data)) throw new Error('Policy history read failed');
    await member.screenshot({ path: path.join(fixture.evidence, 'scope-settings.png'), fullPage: true });
    if (fixture.expectedBinarySha256) {
      await member.goto(fixture.base + '/#monitoring/diagnostics');
      const runtime = member.getByRole('region', { name: '실행 중인 서버 버전', exact: true });
      await runtime.getByText(fixture.expectedBinarySha256, { exact: true }).waitFor();
      await runtime.getByRole('button').click();
      await runtime.getByText(fixture.expectedBinarySha256, { exact: true }).waitFor();
      await member.screenshot({ path: path.join(fixture.evidence, 'runtime.png'), fullPage: true });
      const collection = member.waitForResponse(r => r.url().endsWith('/api/diagnostics/support-report') && r.request().method() === 'POST');
      await member.getByRole('button', { name: '진단 보고서 생성', exact: true }).click();
      const response = await collection;
      if (response.status() !== 200 || response.headers()['cache-control'] !== 'no-store') throw new Error('Support collection failed');
      const preview = await member.getByLabel('다운로드할 진단 자료 미리보기', { exact: true }).textContent();
      const report = JSON.parse(preview);
      if (report.schema_version !== 1 || report.runtime.executable_sha256 !== fixture.expectedBinarySha256 || preview.includes(fixture.token)) throw new Error('Support report identity or privacy failed');
      const download = member.getByRole('button', { name: '검토한 JSON 다운로드', exact: true });
      if (!await download.isDisabled()) throw new Error('Support report review gate missing');
      await member.getByLabel('내용을 검토했으며 이 자료를 다운로드하겠습니다', { exact: true }).check();
      const downloading = member.waitForEvent('download'); await download.click();
      const artifact = await downloading;
      if (fs.readFileSync(await artifact.path(), 'utf8') !== preview) throw new Error('Support download differs from preview');
      await artifact.saveAs(path.join(fixture.evidence, 'support-report.json'));

    }
    await member.goto(fixture.base + '/#security/audit');
    await member.getByRole('heading', { name: '기록 저장 상태와 원본 검사', exact: true }).waitFor();
    const auditHealth = await http.request.get(fixture.base + '/api/audit/health', { headers });
    if (auditHealth.status() !== 200 || !(await auditHealth.json()).data.writable) throw new Error('Production audit writer unavailable');
    const inspectingAudit = member.waitForResponse(r => r.url().endsWith('/api/audit/verify'));
    await member.getByRole('button', { name: '보관된 감사 파일 검사', exact: true }).click();
    const auditResponse = await inspectingAudit;
    const auditReport = (await auditResponse.json()).data;
    if (auditResponse.status() !== 200 || !auditReport.segments.length || auditReport.complete_history_proven !== false || auditReport.independent_proof !== false) throw new Error('Audit inspection coverage not explicit');
    await member.screenshot({ path: path.join(fixture.evidence, 'audit-inspection.png'), fullPage: true });
    await member.goto(fixture.base + '/#management/backup');
    await member.getByRole('heading', { name: '암호화 백업 · 검증 복원', exact: true }).waitFor();
    await member.getByText('관리 데이터 복구 · 운영자 도구', { exact: true }).click();
    await member.getByRole('link', { name: '오프라인 백업·복구 절차 열기', exact: true }).waitFor();
    const recoveryGuide = await http.request.get(fixture.base + '/management-recovery.html');
    if (recoveryGuide.status() !== 200 || !(await recoveryGuide.text()).includes('nabiman-restored')) throw new Error('Recovery procedure unavailable');
    if (!await member.getByRole('button', { name: '암호화 백업 생성', exact: true }).isDisabled() || !await member.getByRole('button', { name: '검증 후 새 디렉터리에 복원', exact: true }).isDisabled()) throw new Error('Encrypted backup form gates missing');
    await member.screenshot({ path: path.join(fixture.evidence, 'encrypted-backup.png'), fullPage: true });
    await member.getByRole('button', { name: '스케줄', exact: true }).click();
    await member.getByText('예약은 UTC 기준입니다. 숫자·목록·범위·*/간격을 지원합니다. 중단 중 놓친 예약은 자동 재실행하지 않습니다.', { exact: true }).waitFor();
    const backupSchedules = await http.request.get(fixture.base + '/api/backup/schedules', { headers });
    if (backupSchedules.status() !== 200 || backupSchedules.headers()['cache-control'] !== 'no-store' || !Array.isArray((await backupSchedules.json()).data)) throw new Error('Backup schedule read failed');
    await member.screenshot({ path: path.join(fixture.evidence, 'backup-schedules.png'), fullPage: true });
    if (typeof fixture.mfaEnabled === 'boolean') {
      await member.goto(fixture.base + '/#administration/mfa');
      await member.getByRole('heading', { name: '2단계 인증', exact: true }).waitFor();
      await member.getByText(fixture.mfaEnabled ? '2단계 인증 사용 중' : '2단계 인증을 사용하지 않고 있습니다', { exact: true }).waitFor();
      await member.screenshot({ path: path.join(fixture.evidence, 'mfa-settings.png'), fullPage: true });
    }
    if (fixture.probeLogin) {
      if ((await http.request.get(fixture.base + '/api/users/me', { headers: { Authorization: 'Bearer ' + fixture.probeLogin.refresh_token } })).status() !== 401) throw new Error('Refresh token accepted as API access credential');
      await member.goto(fixture.base + '/#administration/loginSessions');
      await member.getByRole('heading', { name: '로그인 세션', exact: true }).waitFor();
      await member.getByText('현재 로그인', { exact: true }).waitFor();
      member.on('dialog', dialog => dialog.accept()); await autoConfirm(member);
      const row = member.locator('[data-session-id="' + fixture.probeLogin.sid + '"]');
      await row.getByRole('button', { name: '로그인 종료', exact: true }).click();
      await member.getByText('로그인 세션을 종료했습니다.', { exact: true }).waitFor();
      if ((await http.request.get(fixture.base + '/api/users/me', { headers: { Authorization: 'Bearer ' + fixture.probeLogin.token } })).status() !== 401) throw new Error('Own probe login remained active after UI revocation');
      if ((await http.request.post(fixture.base + '/api/auth/refresh', { data: { refresh_token: fixture.probeLogin.refresh_token } })).status() !== 401) throw new Error('Own probe refresh remained active');
      await member.screenshot({ path: path.join(fixture.evidence, 'login-sessions.png'), fullPage: true });
    }
    const sessions = await (await http.request.get(fixture.base + '/api/terminal/sessions', { headers })).json();
    if (!sessions.success || sessions.data.length !== known.size || sessions.data.some(session => !known.has(session.id) || session.state !== 'alive')) throw new Error('Legacy terminal migration failed through HTTPS');

    member.on('websocket', socket => socket.on('framereceived', frame => {
      if (typeof frame.payload === 'string' && frame.payload.startsWith('\x02SESSION:')) {
        const id = frame.payload.slice(9);
        if (known.has(id)) throw new Error('Probe unexpectedly adopted an existing terminal');
        probeSession = id;
      }
    }));
    await member.goto(fixture.base + '/#remote/remote');
    await member.locator('.rw-item').filter({ hasText: '로컬 셸' }).click();
    // A target opens on its session list; the terminal is one tab over.
    await member.locator('.rw-tabs button').filter({ hasText: '터미널' }).click();
    const connect = member.getByRole('button', { name: '접속', exact: true });
    if (await connect.count()) await connect.click();
    await member.locator('.xterm-helper-textarea').waitFor();
    for (let attempt = 0; attempt < 150 && !probeSession; attempt++) await member.waitForTimeout(100);
    if (!probeSession) throw new Error('Terminal session handshake did not finish');
    await member.waitForFunction(() => Boolean(document.querySelector('.xterm-rows')?.innerText.trim()));
    const marker = 'NABIMAN_DEPLOYMENT_PROBE_' + Date.now();
    await member.locator('.xterm-helper-textarea').focus();
    await member.keyboard.insertText("printf '" + marker + "\\n'"); await member.keyboard.press('Enter');
    await member.waitForFunction(value => document.querySelector('.xterm-rows')?.innerText.split('\n').some(line => line.trim() === value), marker);
    if (!probeSession || known.has(probeSession)) throw new Error('Missing isolated deployment terminal ID');
    await admin.close();
    if (errors.length) throw new Error('Browser runtime errors occurred');
    process.stdout.write(JSON.stringify({ mfa_settings_read_only: typeof fixture.mfaEnabled === 'boolean', login_session_revocation: Boolean(fixture.probeLogin), runtime_identity: Boolean(fixture.expectedBinarySha256), support_report_preview_download: Boolean(fixture.expectedBinarySha256), https_login: true, signup: true, input_alignment: alignment,
      logged_out_terminal_isolation: true, member_management: true, scope_settings: true, mfa_policy_control_read_only: true, policy_history_read_only: true, backup_schedules_read_only: true, encrypted_backup_controls_read_only: true, management_recovery_guide: true, audit_writer_and_inspection: true,
      preserved_sessions: known.size, actual_terminal_input_output: true, browser_errors: errors.length }));
  } finally {
    if (probeSession && !known.has(probeSession)) {
      const result = await (await http.request.delete(fixture.base + '/api/terminal/sessions/' + encodeURIComponent(probeSession), { headers })).json();
      if (!result.success) throw new Error('Own deployment probe terminal cleanup failed');
    }
    await http.close(); await browser.close();
  }
})().catch(error => { console.error('Production rollout verification failed: ' + error.name + ': ' + error.message.split('\n')[0]); process.exit(1); });
