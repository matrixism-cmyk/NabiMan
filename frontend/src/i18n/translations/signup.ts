type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// Signing up, the checks before the app opens, and linking a Rancher account.
const t: T = {
  'signup.open': { ko: '회원가입 신청', en: 'Request an account', ja: '会員登録を申請' },
  'signup.title': { ko: '회원가입 신청', en: 'Request an account', ja: '会員登録の申請' },
  'signup.lead': { ko: '최고관리자가 가입을 승인하고 서버·tenant 소속을 지정하면 서비스를 이용할 수 있습니다.', en: 'You can use the service once a super administrator approves the request and assigns server or tenant memberships.', ja: '最高管理者が加入を承認し、サーバー・tenant の所属を指定するとサービスを利用できます。' },
  'signup.mismatch': { ko: '비밀번호 확인이 일치하지 않습니다.', en: 'The passwords do not match.', ja: '確認用パスワードが一致しません。' },
  'signup.failed': { ko: '가입 신청을 처리하지 못했습니다.', en: 'The request could not be processed.', ja: '加入申請を処理できませんでした。' },
  'signup.offline': { ko: '서버에 연결할 수 없습니다. 잠시 후 다시 시도하세요.', en: 'Cannot reach the server. Try again shortly.', ja: 'サーバーに接続できません。しばらくしてから再試行してください。' },
  'signup.cooldown': { ko: '요청이 많습니다. {seconds}초 후 다시 신청하세요.', en: 'Too many requests. Try again in {seconds} seconds.', ja: 'リクエストが多すぎます。{seconds}秒後に再申請してください。' },
  'signup.id': { ko: '아이디', en: 'ID', ja: 'ID' },
  'signup.idLabel': { ko: '가입 아이디', en: 'Sign-up ID', ja: '登録 ID' },
  'signup.password': { ko: '비밀번호 (8자 이상)', en: 'Password (8+ characters)', ja: 'パスワード (8文字以上)' },
  'signup.passwordLabel': { ko: '가입 비밀번호', en: 'Sign-up password', ja: '登録パスワード' },
  'signup.confirm': { ko: '비밀번호 확인', en: 'Confirm password', ja: 'パスワード確認' },
  'signup.submitting': { ko: '신청 중…', en: 'Sending…', ja: '申請中…' },
  'signup.submit': { ko: '가입 신청', en: 'Send request', ja: '加入を申請' },
  'signup.back': { ko: '로그인으로 돌아가기', en: 'Back to sign-in', ja: 'ログインに戻る' },
  'app.checkingAccess': { ko: '권한을 확인하는 중…', en: 'Checking your access…', ja: '権限を確認中…' },

  'identity.title': { ko: 'Rancher 계정 연결', en: 'Link Rancher account', ja: 'Rancher アカウント連携' },
  'identity.heading': { ko: '{name} · Rancher 계정 연결', en: '{name} · link Rancher account', ja: '{name} · Rancher アカウント連携' },
  'identity.lead': { ko: '실제 Rancher 계정 ID를 확인한 뒤 연결하세요. 이 연결만으로 프로젝트 접근 권한을 부여하거나 회수하지는 않습니다.', en: 'Check the real Rancher account ID before linking. Linking alone neither grants nor revokes project access.', ja: '実際の Rancher アカウント ID を確認してから連携してください。この連携だけでプロジェクトのアクセス権限を付与・回収することはありません。' },
  'identity.loadFailed': { ko: '연결 정보를 불러올 수 없습니다. Rancher 설정과 회원 승인 상태를 확인하세요.', en: 'The link could not be loaded. Check the Rancher settings and the member\'s approval.', ja: '連携情報を読み込めません。Rancher の設定とメンバーの承認状態を確認してください。' },
  'identity.loading': { ko: '연결 정보를 불러오는 중…', en: 'Loading the link…', ja: '連携情報を読み込み中…' },
  'identity.realm': { ko: '대상 Rancher: ', en: 'Rancher: ', ja: '対象 Rancher: ' },
  'identity.linked': { ko: '연결된 계정', en: 'Linked account', ja: '連携済みアカウント' },
  'identity.verified': { ko: '확인 시각 / 관리자', en: 'Verified at / by', ja: '確認日時 / 管理者' },
  'identity.reason': { ko: '연결 사유', en: 'Reason', ja: '連携理由' },
  'identity.linkFailed': { ko: '연결하지 못했습니다. 활성 Rancher 계정인지, 이미 연결된 계정인지 확인하고 다시 조회하세요.', en: 'Could not link. Check that the Rancher account is active and not linked already, then look it up again.', ja: '連携できませんでした。有効な Rancher アカウントか、すでに連携済みでないかを確認して再度照会してください。' },
  'identity.accountId': { ko: 'Rancher 계정 ID', en: 'Rancher account ID', ja: 'Rancher アカウント ID' },
  'identity.reasonInput': { ko: '계정 확인 및 연결 사유', en: 'How the account was verified, and why', ja: 'アカウント確認と連携の理由' },
  'identity.submit': { ko: '확인하고 연결', en: 'Verify and link', ja: '確認して連携' },
};

export default t;
