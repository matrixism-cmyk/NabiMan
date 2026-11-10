type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// Memberships: a scope manager's members, the membership editor and access fields.
const t: T = {
  'membership.role.admin': { ko: '소속 관리자', en: 'Scope administrator', ja: '所属管理者' },
  'membership.role.operator': { ko: '운영자', en: 'Operator', ja: 'オペレーター' },
  'membership.role.viewer': { ko: '조회자', en: 'Viewer', ja: '閲覧者' },
  'membership.roleLabel': { ko: '소속 권한', en: 'Membership role', ja: '所属権限' },
  'membership.remove': { ko: '제거', en: 'Remove', ja: '削除' },
  'membership.add': { ko: '소속 추가', en: 'Add membership', ja: '所属を追加' },
  'membership.addServer': { ko: '쓸 수 있게 할 서버 추가', en: 'Add a server they may use', ja: '使えるようにするサーバーを追加' },
  'membership.addScope': { ko: '서버 또는 tenant 소속 추가', en: 'Add a server or tenant membership', ja: 'サーバーまたは tenant の所属を追加' },
  'membership.allowUse': { ko: '사용 허용', en: 'Allow use', ja: '使用を許可' },
  'membership.allowAccess': { ko: '소속 접근 허용', en: 'Allow membership access', ja: '所属へのアクセスを許可' },
  'membership.allowExec': { ko: '명령 실행 허용', en: 'Allow running commands', ja: 'コマンド実行を許可' },
  'membership.expiry': { ko: '접근 만료일 (현지 시간)', en: 'Access expires (local time)', ja: 'アクセス期限 (現地時刻)' },
  'membership.expiryOn': { ko: '만료 후 자동으로 접근 차단', en: 'Access is blocked automatically after expiry', ja: '期限後は自動的にアクセスを遮断' },
  'membership.expiryOff': { ko: '만료 제한 없음', en: 'No expiry', ja: '期限なし' },

  'scopeMembers.title': { ko: '소속 회원 관리', en: 'Scope members', ja: '所属メンバー管理' },
  'scopeMembers.lead': { ko: '내가 관리하는 소속의 승인된 회원에게 운영·조회 권한을 줍니다. 가입 승인은 최고관리자에게 요청하세요.', en: 'Give approved members of the scopes you manage operator or viewer rights. Ask a super administrator to approve sign-ups.', ja: '自分が管理する所属の承認済みメンバーに運用・閲覧権限を与えます。加入承認は最高管理者に依頼してください。' },
  'scopeMembers.policyBefore': { ko: '소속별 정책은 ', en: 'Per-membership policies are shown in ', ja: '所属ごとのポリシーは ' },
  'scopeMembers.policyAfter': { ko: '에서 봅니다.', en: '.', ja: ' で確認します。' },
  'scopeMembers.addTitle': { ko: '승인된 회원을 소속에 추가', en: 'Add an approved member to a scope', ja: '承認済みメンバーを所属に追加' },
  'scopeMembers.addId': { ko: '추가할 회원 아이디', en: 'Member ID to add', ja: '追加するメンバーの ID' },
  'scopeMembers.idPlaceholder': { ko: '회원 아이디', en: 'Member ID', ja: 'メンバー ID' },
  'scopeMembers.managedScope': { ko: '관리 소속', en: 'Managed scope', ja: '管理する所属' },
  'scopeMembers.pickScope': { ko: '소속 선택', en: 'Pick a scope', ja: '所属を選択' },
  'scopeMembers.addRole': { ko: '추가할 회원 권한', en: 'Role to give', ja: '追加するメンバーの権限' },
  'scopeMembers.accessTitle': { ko: '{name} · 소속 접근 설정', en: '{name} · membership access', ja: '{name} · 所属アクセス設定' },
  'scopeMembers.saveAccess': { ko: '접근 설정 저장', en: 'Save access', ja: 'アクセス設定を保存' },
  'scopeMembers.colScopes': { ko: '소속 및 권한', en: 'Memberships & roles', ja: '所属と権限' },
  'scopeMembers.allowed': { ko: '접근 허용', en: 'Access allowed', ja: 'アクセス許可' },
  'scopeMembers.blocked': { ko: '접근 차단', en: 'Access blocked', ja: 'アクセス遮断' },
  'scopeMembers.execBlocked': { ko: ' · 명령 실행 차단', en: ' · commands blocked', ja: ' · コマンド実行遮断' },
  'scopeMembers.expires': { ko: ' · 만료 {when}', en: ' · expires {when}', ja: ' · 期限 {when}' },
  'scopeMembers.roleOf': { ko: '{name} 소속 권한', en: '{name} membership role', ja: '{name} の所属権限' },
  'scopeMembers.access': { ko: '접근 설정', en: 'Access', ja: 'アクセス設定' },
  'scopeMembers.leave': { ko: '소속 해제', en: 'Remove membership', ja: '所属を解除' },
  'scopeMembers.empty': { ko: '표시할 회원이 없습니다.', en: 'No members to show.', ja: '表示するメンバーはいません。' },
};

export default t;
