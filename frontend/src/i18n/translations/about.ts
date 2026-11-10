type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// The About screen: version, build, licence, notices and support.
const t: T = {
  'app.about': { ko: '정보', en: 'About', ja: '情報' },
  'about.title': { ko: '{name} 정보', en: 'About {name}', ja: '{name}について' },
  'about.version': { ko: '버전', en: 'Version', ja: 'バージョン' },
  'about.commit': { ko: '빌드 커밋', en: 'Build commit', ja: 'ビルドコミット' },
  'about.unknown': { ko: '확인할 수 없음', en: 'Not available', ja: '確認できません' },
  'about.openLicense': { ko: '라이선스 화면 열기', en: 'Open license screen', ja: 'ライセンス画面を開く' },
  'about.notices': { ko: '오픈소스 고지', en: 'Third-party notices', ja: 'サードパーティー通知' },
  'about.noticesText': {
    ko: 'NOTICE와 THIRD_PARTY_LICENSES.md는 릴리스 패키지에 함께 들어 있습니다.',
    en: 'NOTICE and THIRD_PARTY_LICENSES.md are included in the release package.',
    ja: 'NOTICE と THIRD_PARTY_LICENSES.md はリリースパッケージに同梱されています。',
  },
  'about.support': { ko: '지원 문의', en: 'Support', ja: 'サポート窓口' },
  'about.supportNone': { ko: '등록된 연락처가 없습니다', en: 'No contact configured', ja: '連絡先が登録されていません' },
  'about.copy': { ko: '진단 정보 복사', en: 'Copy diagnostics', ja: '診断情報をコピー' },
  'about.copyHint': {
    ko: '문의할 때 붙여 넣어 주세요. 버전·브라우저·설치된 도구만 담기고, 토큰이나 계정 정보는 들어가지 않습니다.',
    en: 'Paste this when you contact support. It holds the version, browser and installed tools — no tokens or account details.',
    ja: 'お問い合わせの際に貼り付けてください。バージョン・ブラウザー・導入済みツールのみで、トークンやアカウント情報は含みません。',
  },
  'about.copied': { ko: '진단 정보를 복사했습니다.', en: 'Diagnostics copied.', ja: '診断情報をコピーしました。' },
  'about.copyFailed': { ko: '복사하지 못했습니다. 브라우저의 클립보드 권한을 확인해 주세요.', en: 'Could not copy. Check the browser\'s clipboard permission.', ja: 'コピーできませんでした。ブラウザーのクリップボード権限を確認してください。' },
};

export default t;
