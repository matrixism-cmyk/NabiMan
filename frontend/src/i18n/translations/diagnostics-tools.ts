type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// The optional programs on the server and what each enables (GET /api/diagnostics/runtime → tools).
const t: T = {
  'diagnostics.tools': { ko: '외부 프로그램', en: 'External programs', ja: '外部プログラム' },
  'diagnostics.toolsHint': { ko: '서버가 시작할 때 확인한 결과입니다. 없는 프로그램은 그 기능만 꺼집니다.', en: 'Checked when the server started. A missing program turns off only its features.', ja: 'サーバー起動時の確認結果です。ないプログラムはその機能だけがオフになります。' },
  'diagnostics.toolsNone': { ko: '아직 확인 결과가 없습니다.', en: 'Not checked yet.', ja: 'まだ確認結果がありません。' },
  'diagnostics.toolsCheckedAt': { ko: '확인 시각: {at}', en: 'Checked at {at}', ja: '確認時刻: {at}' },
  'diagnostics.toolsRecheck': { ko: '다시 확인', en: 'Check again', ja: '再確認' },
  'diagnostics.toolName': { ko: '프로그램', en: 'Program', ja: 'プログラム' },
  'diagnostics.toolStatus': { ko: '상태', en: 'Status', ja: '状態' },
  'diagnostics.toolFeatures': { ko: '쓰는 기능', en: 'Enables', ja: '使う機能' },
  'diagnostics.toolFound': { ko: '있음', en: 'Found', ja: 'あり' },
  'diagnostics.toolMissing': { ko: '없음 — 이 기능 꺼짐', en: 'Missing — these are off', ja: 'なし — この機能はオフ' },
  'diagnostics.toolUnused': { ko: '없음 (이 버전에서 쓰지 않음)', en: 'Missing (not used by this version)', ja: 'なし（このバージョンでは未使用）' },
  'diagnostics.feature.terminal': { ko: '터미널', en: 'Terminals', ja: 'ターミナル' },
  'diagnostics.feature.remote_ssh': { ko: '원격 서버 SSH', en: 'Remote servers over SSH', ja: 'リモートサーバー SSH' },
  'diagnostics.feature.sftp': { ko: '파일(SFTP)', en: 'Files (SFTP)', ja: 'ファイル（SFTP）' },
  'diagnostics.feature.ssh_password': { ko: '비밀번호로 로그인하는 서버', en: 'Servers that sign in with a password', ja: 'パスワードでログインするサーバー' },
  'diagnostics.feature.ssh_keys': { ko: 'SSH 키 만들기', en: 'Making SSH keys', ja: 'SSH キーの作成' },
  'diagnostics.feature.ssh_certificates': { ko: 'SSH 인증서 로그인', en: 'SSH certificate sign-in', ja: 'SSH 証明書ログイン' },
  'diagnostics.feature.serial_bridge': { ko: '브라우저 시리얼 장치', en: 'Browser serial devices', ja: 'ブラウザーのシリアル機器' },
  'diagnostics.feature.rdp_console': { ko: 'RDP 콘솔', en: 'RDP consoles', ja: 'RDP コンソール' },
  'diagnostics.feature.office_preview': { ko: 'Office 문서 미리보기', en: 'Office document previews', ja: 'Office 文書のプレビュー' },
  'diagnostics.feature.hangul_preview': { ko: '한글(HWP) 문서 미리보기', en: 'Hangul (HWP) previews', ja: 'ハングル（HWP）文書のプレビュー' },
  'diagnostics.feature.claude_voice': { ko: 'Claude 음성 대화', en: 'Claude voice chat', ja: 'Claude 音声会話' },
};

export default t;
