type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// The "AI 안내" button on a session: the orchestration guide, for the AI working in it.
const t: T = {
  'aiGuide.button': { ko: 'AI 안내', en: 'AI guide', ja: 'AI 案内' },
  'aiGuide.short': { ko: 'AI', en: 'AI', ja: 'AI' },
  'aiGuide.buttonHint': {
    ko: 'AI 안내: 이 세션의 AI에게 이 서버의 협업 규칙을 건네줍니다',
    en: 'AI guide: hand this server\'s working rules to the AI in this session',
    ja: 'AI 案内: このセッションの AI にこのサーバーの協業ルールを渡します',
  },
  'aiGuide.copy': { ko: '안내문 복사', en: 'Copy the guide', ja: '案内文をコピー' },
  'aiGuide.paste': { ko: '이 세션에 붙여넣기', en: 'Paste into this session', ja: 'このセッションに貼り付け' },
  'aiGuide.preview': { ko: '미리 보기', en: 'Preview', ja: 'プレビュー' },
  'aiGuide.hint': {
    ko: '이 세션에서 새로 일을 시작한 AI에게 이 서버의 협업 규칙(누가 총괄하고, 언제 무엇을 보고하는지)을 건네줄 때 씁니다.',
    en: 'For handing a new AI in this session the rules of this server: who leads, and what to report when.',
    ja: 'このセッションで新しく作業を始めた AI に、このサーバーの協業ルール(誰が総括し、いつ何を報告するか)を渡すときに使います。',
  },
  'aiGuide.loading': { ko: '안내문을 불러오는 중…', en: 'Loading the guide…', ja: '案内文を読み込み中…' },
  'aiGuide.loadFailed': { ko: '안내문을 불러오지 못했어요.', en: 'Could not load the guide.', ja: '案内文を読み込めませんでした。' },
  'aiGuide.copied': { ko: '복사했어요', en: 'Copied', ja: 'コピーしました' },
  'aiGuide.copyFailed': {
    ko: '복사하지 못했어요. 미리 보기에서 직접 선택해 복사해 주세요.',
    en: 'Could not copy. Select the text in the preview and copy it yourself.',
    ja: 'コピーできませんでした。プレビューで選択してコピーしてください。',
  },
  'aiGuide.pasted': {
    ko: '붙여넣었어요. Enter를 눌러 보내세요.',
    en: 'Pasted. Press Enter to send it.',
    ja: '貼り付けました。Enter を押して送信してください。',
  },
  'aiGuide.notConnected': {
    ko: '세션이 연결되어 있지 않아 붙여넣지 못했어요.',
    en: 'The session is not connected, so nothing was pasted.',
    ja: 'セッションが接続されていないため、貼り付けられませんでした。',
  },
  'aiGuide.previewTitle': { ko: 'AI 안내문 미리 보기', en: 'AI guide preview', ja: 'AI 案内文のプレビュー' },
  'aiGuide.text': { ko: 'AI 안내문', en: 'AI guide text', ja: 'AI 案内文' },
  'aiGuide.updated': { ko: '안내문 수정: {date}', en: 'Guide updated: {date}', ja: '案内文の更新: {date}' },
};

export default t;
