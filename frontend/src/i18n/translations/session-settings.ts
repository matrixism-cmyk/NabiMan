type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// A session's own settings over the account's, and a share visitor's own view.
const t: T = {
  'terminal.sessionTab': { ko: '이 세션', en: 'This session', ja: 'このセッション' },
  'terminal.defaultsTab': { ko: '전체 기본값', en: 'Defaults for all', ja: '全体の既定値' },
  'terminal.sessionHint': { ko: '이 세션에만 적용됩니다. "전체 설정 따름"을 끄면 이 세션만 다른 값을 씁니다. 원격 화면, 떠 있는 창, 다른 브라우저에서 이 세션을 열 때 모두 같은 값이 쓰입니다.', en: 'For this session alone. Untick "follow" to give it its own value; it shows the same in the pane, a floating window or another browser.', ja: 'このセッションだけに適用されます。「全体設定に従う」を外すと、このセッションだけ別の値を使います。' },
  'terminal.follow': { ko: '전체 설정 따름 ({value})', en: 'Follow the defaults ({value})', ja: '全体設定に従う（{value}）' },
  'terminal.sessionReset': { ko: '모두 전체 설정 따르기', en: 'Follow the defaults for all', ja: 'すべて全体設定に従う' },
  'terminal.sessionKeepHint': { ko: '이 세션을 서버에 계속 살려 둘지, 아무도 안 볼 때 얼마 뒤 닫을지는 세션마다 따로 정해집니다.', en: 'Whether this session stays alive on the server, or closes some time after no one watches, is the session’s own.', ja: 'このセッションをサーバーで維持するか、誰も見ていないとき何後に閉じるかはセッションごとに決まります。' },
  'terminal.on': { ko: '켬', en: 'on', ja: 'オン' },
  'terminal.off': { ko: '끔', en: 'off', ja: 'オフ' },
  'terminal.days': { ko: '{n}일', en: '{n} d', ja: '{n}日' },
  'terminal.hours': { ko: '{n}시간', en: '{n} h', ja: '{n}時間' },
  'terminal.sessionSettings': { ko: '이 세션 설정', en: 'This session’s settings', ja: 'このセッションの設定' },
  'share.viewSettings': { ko: '보기 설정', en: 'View settings', ja: '表示設定' },
  'share.viewSettingsHint': { ko: '이 브라우저에만 저장됩니다. 세션을 공유한 사람의 화면에는 영향이 없습니다.', en: 'Kept in this browser only; the person who shared the session sees no change.', ja: 'このブラウザーにだけ保存されます。共有した人の画面には影響しません。' },
  'share.viewSizeHint': { ko: '글자 크기는 공유한 사람의 화면 칸 수에 맞춰 자동으로 정해집니다. 직접 키우거나 줄이려면 터미널 위에서 Ctrl+마우스 휠을 쓰고, "화면 맞춤"으로 되돌립니다.', en: 'The text is sized to fit the shared screen’s grid. Ctrl+wheel over the terminal makes it larger or smaller; "Fit window" puts it back.', ja: '文字サイズは共有した人の画面の桁数に合わせて自動で決まります。ターミナル上で Ctrl+ホイールで拡大・縮小し、「画面に合わせる」で戻します。' },
  'share.viewReset': { ko: '처음 값으로', en: 'Back to defaults', ja: '初期値に戻す' },
};

export default t;
