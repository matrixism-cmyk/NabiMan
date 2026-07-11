import { Lang } from './index';

// MEC operational panels: Jobs (+ JobProgress), Audit, Settings, Users.
// Reuses the shared mec.action.* / mec.state.* / mec.col.* keys defined in
// mec-live.ts so wording stays consistent across the module.
const mecOps: Record<string, Record<Lang, string>> = {
  // ── Jobs (MecJobsPanel + JobProgress) ──────────────────────────────────
  'mec.jobs.title': { ko: 'MEC Jobs', en: 'MEC Jobs', ja: 'MEC Jobs' },
  'mec.jobs.subtitle': {
    ko: '{total}건 · 진행 중 {running}건',
    en: '{total} total · {running} running',
    ja: '{total}件 · 実行中 {running}件',
  },
  'mec.jobs.liveProgress': {
    ko: '실시간 진행',
    en: 'Live progress',
    ja: 'リアルタイム進捗',
  },
  'mec.jobs.close': { ko: '닫기', en: 'Close', ja: '閉じる' },
  'mec.jobs.filterPlaceholder': {
    ko: '필터 (ID, 종류, 상태)',
    en: 'Filter (ID, kind, status)',
    ja: 'フィルター (ID, 種類, 状態)',
  },
  'mec.jobs.empty': {
    ko: 'Job 기록이 없습니다.',
    en: 'No jobs recorded.',
    ja: 'Job の記録がありません。',
  },
  'mec.jobs.colId': { ko: 'Job ID', en: 'Job ID', ja: 'Job ID' },
  'mec.jobs.colKind': { ko: '종류', en: 'Kind', ja: '種類' },
  'mec.jobs.colStatus': { ko: '상태', en: 'Status', ja: '状態' },
  'mec.jobs.colProgress': { ko: '진행률', en: 'Progress', ja: '進捗率' },
  'mec.jobs.colSteps': { ko: 'Steps', en: 'Steps', ja: 'Steps' },
  'mec.jobs.colStarted': { ko: '시작', en: 'Started', ja: '開始' },
  'mec.jobs.colCompleted': { ko: '완료', en: 'Completed', ja: '完了' },
  'mec.jobs.actionLive': { ko: '실시간', en: 'Live', ja: 'リアルタイム' },
  'mec.jobs.actionHistory': { ko: '기록', en: 'History', ja: '履歴' },

  // JobProgress
  'mec.jobs.statusLabel': { ko: '상태:', en: 'Status:', ja: '状態:' },
  'mec.jobs.stateRunning': { ko: 'running', en: 'running', ja: 'running' },
  'mec.jobs.stateConnecting': { ko: 'connecting', en: 'connecting', ja: 'connecting' },
  'mec.jobs.errorLabel': { ko: '에러:', en: 'Error:', ja: 'エラー:' },

  // ── Audit (MecAuditPanel) ──────────────────────────────────────────────
  'mec.audit.title': { ko: 'MEC Audit Log', en: 'MEC Audit Log', ja: 'MEC Audit Log' },
  'mec.audit.subtitle': {
    ko: '최근 {total}건 · 성공 {success} · 실패 {failed}',
    en: 'Last {total} · {success} succeeded · {failed} failed',
    ja: '直近 {total}件 · 成功 {success} · 失敗 {failed}',
  },
  'mec.audit.filterPlaceholder': {
    ko: '필터 (action, user, resource)',
    en: 'Filter (action, user, resource)',
    ja: 'フィルター (action, user, resource)',
  },
  'mec.audit.empty': {
    ko: '감사로그가 없습니다.',
    en: 'No audit logs.',
    ja: '監査ログがありません。',
  },
  'mec.audit.colUser': { ko: '사용자', en: 'User', ja: 'ユーザー' },
  'mec.audit.colAction': { ko: '작업', en: 'Action', ja: '操作' },
  'mec.audit.colResource': { ko: '리소스', en: 'Resource', ja: 'リソース' },
  'mec.audit.colResult': { ko: '결과', en: 'Result', ja: '結果' },
  'mec.audit.colDuration': { ko: '소요', en: 'Duration', ja: '所要時間' },
  'mec.audit.colOps': { ko: 'Ops', en: 'Ops', ja: 'Ops' },

  // ── Settings (MecSettingsPanel) ────────────────────────────────────────
  'mec.settings.title': { ko: 'MEC 연동 설정', en: 'MEC Integration Settings', ja: 'MEC連携設定' },
  'mec.settings.readOnlyBanner': {
    ko: 'Read-only 모드가 켜져 있습니다.',
    en: 'Read-only mode is enabled.',
    ja: 'Read-only モードが有効です。',
  },
  'mec.settings.readOnlyBody': {
    ko: '모든 변경 작업이 차단됩니다. 안전한 첫 연결을 위해 권장되는 상태이며, 조회가 모두 정상이면',
    en: 'All mutating operations are blocked. This is recommended for a safe first connection; once all reads look correct, set',
    ja: 'すべての変更操作がブロックされます。安全な初回接続のために推奨される状態で、参照がすべて正常であれば',
  },
  'mec.settings.readOnlyBodyAfter': {
    ko: '로 변경 후 재시작하세요.',
    en: 'and restart.',
    ja: 'に変更して再起動してください。',
  },
  'mec.settings.currentTitle': {
    ko: '현재 설정 ({mode} 모드{flags})',
    en: 'Current settings ({mode} mode{flags})',
    ja: '現在の設定 ({mode} モード{flags})',
  },
  'mec.settings.flagReadOnly': {
    ko: ', 🔒 read-only',
    en: ', 🔒 read-only',
    ja: ', 🔒 read-only',
  },
  'mec.settings.flagConfirmSkipped': {
    ko: ', confirm skipped',
    en: ', confirm skipped',
    ja: ', confirm skipped',
  },
  'mec.settings.kubeInCluster': {
    ko: '(in-cluster 추론)',
    en: '(in-cluster inferred)',
    ja: '(in-cluster 推論)',
  },
  'mec.settings.themeTitle': { ko: '색상 테마', en: 'Color Theme', ja: 'カラーテーマ' },
  'mec.settings.themeBody': {
    ko: '클릭으로 즉시 전환됩니다. 선택한 테마는 브라우저에 저장되어 다음 로그인에도 유지됩니다. (상단 헤더의 셀렉트 박스에서도 동일하게 변경 가능)',
    en: 'Switches instantly on click. The selected theme is saved in the browser and persists across logins. (You can also change it from the select box in the top header.)',
    ja: 'クリックで即座に切り替わります。選択したテーマはブラウザに保存され、次回ログイン時も維持されます。(上部ヘッダーのセレクトボックスからも同様に変更できます)',
  },
  'mec.settings.dropinTitle': {
    ko: 'systemd drop-in 템플릿',
    en: 'systemd drop-in template',
    ja: 'systemd drop-in テンプレート',
  },
  'mec.settings.copy': { ko: '복사', en: 'Copy', ja: 'コピー' },
  'mec.settings.copied': { ko: '복사됨!', en: 'Copied!', ja: 'コピーしました!' },
  'mec.settings.dropinBodyBefore': {
    ko: '아래 내용을',
    en: 'Save the content below to',
    ja: '以下の内容を',
  },
  'mec.settings.dropinBodyAfter': {
    ko: '에 저장한 뒤 적용 명령을 실행하세요. 비밀값(*** 부분)은 실제 값으로 교체해야 합니다.',
    en: 'then run the apply command. Secret values (the *** parts) must be replaced with real values.',
    ja: 'に保存してから適用コマンドを実行してください。シークレット値(*** の部分)は実際の値に置き換える必要があります。',
  },

  // ── Users (MecUsersPanel) ──────────────────────────────────────────────
  'mec.users.title': { ko: 'Rancher 사용자', en: 'Rancher Users', ja: 'Rancher ユーザー' },
  'mec.users.subtitle': {
    ko: '{total}명 · 활성 {enabled}',
    en: '{total} users · {enabled} active',
    ja: '{total}名 · アクティブ {enabled}',
  },
  'mec.users.cancel': { ko: '취소', en: 'Cancel', ja: 'キャンセル' },
  'mec.users.addUser': { ko: '+ 사용자', en: '+ User', ja: '+ ユーザー' },
  'mec.users.createTitle': { ko: '사용자 생성', en: 'Create User', ja: 'ユーザー作成' },
  'mec.users.initialPassword': { ko: '초기 비밀번호', en: 'Initial Password', ja: '初期パスワード' },
  'mec.users.auto': { ko: '🎲 자동', en: '🎲 Auto', ja: '🎲 自動' },
  'mec.users.creating': { ko: '생성 중...', en: 'Creating…', ja: '作成中…' },
  'mec.users.create': { ko: '생성', en: 'Create', ja: '作成' },
  'mec.users.filterPlaceholder': {
    ko: '필터 (ID, username)',
    en: 'Filter (ID, username)',
    ja: 'フィルター (ID, username)',
  },
  'mec.users.empty': {
    ko: '사용자가 없습니다.',
    en: 'No users.',
    ja: 'ユーザーがいません。',
  },
  'mec.users.close': { ko: '닫기', en: 'Close', ja: '閉じる' },
  'mec.users.colDisplayName': { ko: '표시명', en: 'Display Name', ja: '表示名' },
  'mec.users.colEnabled': { ko: '활성', en: 'Active', ja: 'アクティブ' },
  'mec.users.resetPassword': { ko: '비번 리셋', en: 'Reset Password', ja: 'パスワードリセット' },
  'mec.users.delete': { ko: '삭제', en: 'Delete', ja: '削除' },
  'mec.users.createdToast': {
    ko: '사용자 {username} 생성됨. 초기 비밀번호: {password} (한 번만 표시)',
    en: 'User {username} created. Initial password: {password} (shown once)',
    ja: 'ユーザー {username} を作成しました。初期パスワード: {password} (一度のみ表示)',
  },
  'mec.users.deletePrompt': {
    ko: "⚠️ 사용자 '{username}' ({id}) 을(를) 삭제합니다.\n확인을 위해 ID 를 다시 입력하세요:",
    en: "⚠️ Deleting user '{username}' ({id}).\nRe-enter the ID to confirm:",
    ja: "⚠️ ユーザー '{username}' ({id}) を削除します。\n確認のため ID を再入力してください:",
  },
  'mec.users.deleteMismatch': {
    ko: '입력 ID 가 일치하지 않아 삭제 취소.',
    en: 'Entered ID does not match; deletion cancelled.',
    ja: '入力した ID が一致しないため削除を中止しました。',
  },
  'mec.users.resetPrompt': {
    ko: '{username} 의 새 비밀번호 (비우면 자동 생성):',
    en: 'New password for {username} (leave blank to auto-generate):',
    ja: '{username} の新しいパスワード (空欄の場合は自動生成):',
  },
  'mec.users.resetToast': {
    ko: '{username} 비밀번호 리셋: {password} (한 번만 표시)',
    en: '{username} password reset: {password} (shown once)',
    ja: '{username} パスワードリセット: {password} (一度のみ表示)',
  },
};

export default mecOps;
