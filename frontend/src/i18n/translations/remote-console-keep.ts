type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// VM consoles that stay connected while out of sight, their speed choice, and
// the notice when another login took the Windows session.
const t: T = {
  'remote.console.kept': { ko: '연결 유지 중', en: 'Kept connected', ja: '接続維持中' },
  'remote.console.keptHint': {
    ko: '다른 화면을 보는 동안에도 연결을 유지하고 있습니다. {minutes}분 동안 다시 열지 않으면 끊습니다. 지금 끊으려면 오른쪽 클릭(길게 누르기) 메뉴에서 "연결 끊기"를 고르세요.',
    en: 'Still connected while you look at something else. It is disconnected after {minutes} minutes without being opened. To disconnect now, right-click (or long-press) and choose Disconnect.',
    ja: '他の画面を見ている間も接続を維持しています。{minutes}分間開かなければ切断します。今すぐ切断するには右クリック（長押し）メニューで「切断」を選んでください。',
  },
  'remote.console.hangUp': { ko: '연결 끊기', en: 'Disconnect', ja: '切断' },
  'remote.console.hangUpHint': {
    ko: '이 콘솔의 연결을 끊습니다. VM 안에서 하던 작업은 그대로 남습니다.',
    en: 'Disconnects this console. What is running inside the VM stays as it is.',
    ja: 'このコンソールの接続を切ります。VM 内の作業はそのまま残ります。',
  },
  'remote.console.err.displaced': {
    ko: '다른 곳에서 이 계정으로 접속해 연결이 끊겼습니다. 여기서 다시 쓰려면 "다시 연결"을 누르세요.',
    en: 'Disconnected because this account signed in from somewhere else. Press Reconnect to use it here again.',
    ja: '別の場所からこのアカウントで接続されたため、切断されました。ここで使うには「再接続」を押してください。',
  },
  'remote.console.speed.label': { ko: '화면 속도', en: 'Display speed', ja: '画面の速度' },
  'remote.console.speed.hint': {
    ko: '회선이 느리면 "빠르게", 화면을 또렷하게 보려면 "선명하게"를 고르세요. RDP 콘솔은 바꿀 때 잠깐 다시 연결합니다.',
    en: 'Choose Fast on a slow line, Sharp for the clearest picture. An RDP console reconnects briefly when this changes.',
    ja: '回線が遅いときは「高速」、くっきり表示したいときは「鮮明」を選んでください。RDP コンソールは変更時に一度つなぎ直します。',
  },
  'remote.console.speed.fast': { ko: '빠르게', en: 'Fast', ja: '高速' },
  'remote.console.speed.balanced': { ko: '균형', en: 'Balanced', ja: 'バランス' },
  'remote.console.speed.sharp': { ko: '선명하게', en: 'Sharp', ja: '鮮明' },
};

export default t;
