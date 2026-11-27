type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// VM consoles as windows on the page, beside the terminal windows: the two
// ways to detach a console, the notice in the workspace, and the bar's words.
const t: T = {
  'terminal.windows': { ko: '창', en: 'Windows', ja: 'ウィンドウ' },
  'terminal.consoleWindow': { ko: 'VM 콘솔', en: 'VM console', ja: 'VM コンソール' },
  'terminal.closeConsoleHint': {
    ko: '창 닫기 (연결은 30분 동안 유지됩니다. 바로 끊으려면 창 안의 "연결 끊기"를 누르세요)',
    en: 'Close the window (the connection is kept for 30 minutes; press Disconnect in the window to end it now)',
    ja: 'ウィンドウを閉じる（接続は30分間維持されます。すぐ切るにはウィンドウ内の「切断」を押してください）',
  },
  'remote.console.detach': { ko: '분리', en: 'Detach', ja: '分離' },
  'remote.console.detachHint': {
    ko: '이 페이지 안의 창으로 엽니다. 아래 줄에 나오고, 정렬·계단식에 함께 들어갑니다.',
    en: 'Opens it as a window on this page: it shows on the bar below and is tiled and cascaded with the others.',
    ja: 'このページ内のウィンドウで開きます。下のバーに表示され、整列・重ねて表示の対象になります。',
  },
  'remote.console.popOut': { ko: '새창으로 분리', en: 'Detach to a browser window', ja: 'ブラウザの別ウィンドウに分離' },
  'remote.console.popOutHint': {
    ko: '브라우저 창을 따로 엽니다. 다른 모니터로 옮길 수 있습니다. 이 탭의 연결은 끊고 새 창이 다시 연결합니다.',
    en: 'Opens a browser window of its own, which can go to another monitor. This tab disconnects and the new window connects again.',
    ja: 'ブラウザの別ウィンドウを開きます。別のモニターに移せます。このタブの接続は切り、新しいウィンドウがつなぎ直します。',
  },
  'remote.console.windowed': { ko: '이 콘솔은 창으로 열려 있습니다', en: 'This console is open in a window', ja: 'このコンソールはウィンドウで開いています' },
  'remote.console.goToWindow': { ko: '창으로 가기', en: 'Go to the window', ja: 'ウィンドウへ移動' },
  'remote.console.bringHere': { ko: '여기로 가져오기', en: 'Bring it here', ja: 'ここに表示する' },
  'start.consolePopOut': { ko: '{label} 새창으로 분리', en: 'Detach {label} to a browser window', ja: '{label} をブラウザの別ウィンドウに分離' },
};

export default t;
