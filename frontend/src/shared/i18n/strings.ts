import type { Translations } from './context';

/**
 * The words the shared parts and Remote take from NabiMan: buttons, states
 * and the few labels a product built on NabiMan shows in its own shell.
 * Every product's translations start from these.
 */
const strings: Translations = {
  'common.loading': { ko: '로딩 중...', en: 'Loading...', ja: '読み込み中...' },
  'common.error': { ko: '오류', en: 'Error', ja: 'エラー' },
  'common.save': { ko: '저장', en: 'Save', ja: '保存' },
  'common.cancel': { ko: '취소', en: 'Cancel', ja: 'キャンセル' },
  'common.close': { ko: '닫기', en: 'Close', ja: '閉じる' },
  'common.delete': { ko: '삭제', en: 'Delete', ja: '削除' },
  'common.confirm': { ko: '확인', en: 'OK', ja: 'OK' },
  'common.actions': { ko: '작업', en: 'Actions', ja: '操作' },
  'common.name': { ko: '이름', en: 'Name', ja: '名前' },
  'common.working': { ko: '작업 중...', en: 'Working...', ja: '処理中...' },
  'common.run': { ko: '실행', en: 'Run', ja: '実行' },
  'common.running': { ko: '실행 중...', en: 'Running...', ja: '実行中...' },
  'common.online': { ko: '온라인', en: 'online', ja: 'オンライン' },
  'common.offline': { ko: '오프라인', en: 'offline', ja: 'オフライン' },
  'common.unknown': { ko: '알 수 없음', en: 'unknown', ja: '不明' },
  'common.connect': { ko: '연결', en: 'Connect', ja: '接続' },
  'common.settings': { ko: '환경설정', en: 'Settings', ja: '設定' },
  'common.language': { ko: '언어', en: 'Language', ja: '言語' },
  'confirm.more': { ko: '외 {count}개', en: 'and {count} more', ja: 'ほか {count} 件' },
  'app.logout': { ko: '로그아웃', en: 'Logout', ja: 'ログアウト' },
  'app.hideChrome': { ko: '상단 접기 (로고·메뉴 숨기기)', en: 'Fold away the top (logo and menu)', ja: '上部をたたむ（ロゴ・メニューを隠す）' },
  'app.showChrome': { ko: '로고·메뉴 다시 보이기', en: 'Show the logo and menu', ja: 'ロゴ・メニューを表示' },
  'accounts.password': { ko: '비밀번호', en: 'Password', ja: 'パスワード' },
  'tab.terminal': { ko: '터미널', en: 'Terminal', ja: 'ターミナル' },
};

export default strings;
