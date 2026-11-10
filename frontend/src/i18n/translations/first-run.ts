type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// The first-run checklist an administrator sees on a new install.
const t: T = {
  'app.firstRun': { ko: '처음 시작하기', en: 'Getting started', ja: 'はじめに' },
  'firstRun.title': { ko: '처음 시작하기', en: 'Getting started', ja: 'はじめに' },
  'firstRun.intro': {
    ko: '새로 설치한 서버라면 아래 순서대로 준비해 보세요. ({done}/{total} 완료)',
    en: 'A new install? These steps get it ready. ({done}/{total} done)',
    ja: '新しく導入したサーバーなら、次の順に準備しましょう。({done}/{total} 完了)',
  },
  'firstRun.doneLabel': { ko: '완료', en: 'done', ja: '完了' },
  'firstRun.dismiss': { ko: '다시 보지 않기', en: "Don't show again", ja: '今後表示しない' },
  'firstRun.password': { ko: '관리자 비밀번호 바꾸기', en: 'Change the admin password', ja: '管理者パスワードを変更' },
  'firstRun.password.hint': { ko: '설치할 때 정한 비밀번호를 나만 아는 것으로 바꿉니다.', en: 'Replace the password set at install with one only you know.', ja: 'インストール時のパスワードを自分だけが知るものに変えます。' },
  'firstRun.server': { ko: '서버 추가하기', en: 'Add a server', ja: 'サーバーを追加' },
  'firstRun.server.hint': { ko: '원격 작업 공간에 관리할 서버를 등록합니다.', en: 'Register a server to manage in the remote workspace.', ja: 'リモートワークスペースに管理するサーバーを登録します。' },
  'firstRun.terminal': { ko: '터미널 열어 보기', en: 'Open a terminal', ja: 'ターミナルを開く' },
  'firstRun.terminal.hint': { ko: '이 서버의 셸에 바로 접속해 봅니다.', en: "Connect to this server's shell.", ja: 'このサーバーのシェルに接続してみます。' },
  'firstRun.backup': { ko: '백업 일정 정하기', en: 'Schedule backups', ja: 'バックアップの予定を設定' },
  'firstRun.backup.hint': { ko: '백업 화면의 일정 탭에서 정기 백업을 켭니다.', en: 'Turn on regular backups in the Schedules tab of Backup.', ja: 'バックアップ画面のスケジュールタブで定期バックアップを有効にします。' },
  'firstRun.users': { ko: '사용자 초대하기', en: 'Invite a user', ja: 'ユーザーを招待' },
  'firstRun.users.hint': { ko: '함께 쓸 사람의 계정을 만들고 권한을 정합니다.', en: 'Create accounts for the people you work with and set their roles.', ja: '一緒に使う人のアカウントを作り、権限を決めます。' },
  'firstRun.guide': { ko: '오케스트레이션 안내 읽기', en: 'Read the orchestration guide', ja: 'オーケストレーション案内を読む' },
  'firstRun.guide.hint': { ko: '여러 Claude 세션을 함께 운영하는 방법입니다.', en: 'How several Claude sessions work together here.', ja: '複数の Claude セッションを一緒に運用する方法です。' },
  'firstRun.guideUnavailable': { ko: '안내를 불러오지 못했습니다. 잠시 후 다시 열어 주세요.', en: 'The guide could not be loaded. Try again in a moment.', ja: '案内を読み込めませんでした。しばらくしてからもう一度開いてください。' },
};

export default t;
