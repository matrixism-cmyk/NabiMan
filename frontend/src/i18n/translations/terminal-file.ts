type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// A file name or path printed in a terminal, pressed and held: its menu, and the link to a file.
const t: T = {
  'termFile.hint': { ko: '길게 누르면 열기·복사 메뉴가 나옵니다', en: 'Press and hold to open or copy', ja: '長押しすると「開く・コピー」のメニューが出ます' },
  'termFile.open': { ko: '열기', en: 'Open', ja: '開く' },
  'termFile.copyPath': { ko: '경로 복사', en: 'Copy path', ja: 'パスをコピー' },
  'termFile.copyLink': { ko: '링크 복사', en: 'Copy link', ja: 'リンクをコピー' },
  'termFile.showInPanel': { ko: '파일 패널에서 열기', en: 'Open in the file panel', ja: 'ファイルパネルで開く' },
  'termFile.copied': { ko: '복사했습니다', en: 'Copied', ja: 'コピーしました' },
  'termFile.copyFailed': { ko: '복사하지 못했습니다. 브라우저가 클립보드를 막았습니다.', en: 'Could not copy: the browser blocked the clipboard.', ja: 'コピーできませんでした。ブラウザーがクリップボードを止めています。' },
  'sftp.linkMissing': { ko: '{name} 파일이 이 폴더에 없습니다. 옮겼거나 지웠을 수 있습니다.', en: '{name} is not in this folder. It may have been moved or deleted.', ja: '{name} はこのフォルダーにありません。移動または削除された可能性があります。' },
};

export default t;
