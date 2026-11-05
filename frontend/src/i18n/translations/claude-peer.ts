type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// The Claude Code session address in a terminal's header, and work in progress in the list.
const t: T = {
  'claudePeer.busy': { ko: '작업 중', en: 'working', ja: '作業中' },
  'claudePeer.idle': { ko: '대기 중', en: 'waiting', ja: '待機中' },
  'claudePeer.hint': {
    ko: 'Claude 세션 주소: {address}\n작업 폴더: {cwd}\n상태: {state}\n누르면 주소를 복사합니다. 다른 Claude 세션의 SendMessage에 그대로 쓰면 됩니다.',
    en: 'Claude session address: {address}\nFolder: {cwd}\nState: {state}\nClick to copy the address, ready for another Claude session\'s SendMessage.',
    ja: 'Claude セッションのアドレス: {address}\n作業フォルダ: {cwd}\n状態: {state}\nクリックでアドレスをコピーします。他の Claude セッションの SendMessage にそのまま使えます。',
  },
  'claudePeer.copyLabel': { ko: 'Claude 세션 주소 {address} 복사', en: 'Copy Claude session address {address}', ja: 'Claude セッションのアドレス {address} をコピー' },
  'claudePeer.copied': { ko: '복사됨', en: 'copied', ja: 'コピー済み' },
  'claudePeer.working': { ko: 'Claude 작업 중', en: 'Claude working', ja: 'Claude 作業中' },
  'claudePeer.output': { ko: '출력 중', en: 'output', ja: '出力中' },
  'claudePeer.workingHint': { ko: '이 세션의 Claude Code가 지금 작업하고 있습니다.', en: 'Claude Code in this session is working now.', ja: 'このセッションの Claude Code が作業中です。' },
  'claudePeer.orchestrator': { ko: '오케스트레이터 → {count}', en: 'Orchestrating → {count}', ja: 'オーケストレーター → {count}' },
  'claudePeer.orchestratorHint': {
    ko: '이 세션의 Claude가 다른 세션들에 일을 맡기고 있습니다(최근 1시간 안에 주고받은 메시지 기준). 화살표가 맡긴 쪽에서 받은 쪽으로 이어집니다.',
    en: 'Claude here is handing work to other sessions (messages in the last hour). Arrows run from it to each.',
    ja: 'このセッションの Claude が他のセッションに作業を任せています(直近1時間のメッセージ基準)。矢印は任せた側から受けた側へ向かいます。',
  },
  'claudePeer.led': { ko: '← {name}', en: '← {name}', ja: '← {name}' },
  'claudePeer.ledHint': { ko: '{name}에게서 일을 받아 하고 있습니다.', en: 'Working on what {name} handed over.', ja: '{name} から任された作業をしています。' },
  'claudePeer.outputHint': { ko: '방금 화면 내용이 바뀌었습니다.', en: 'The screen changed just now.', ja: '画面が今変わりました。' },
};

export default t;
