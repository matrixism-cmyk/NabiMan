type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// The audit panel's "what changed" column (an entry's `change` field).
const t: T = {
  'audit.change': { ko: '변경 내용', en: 'Change', ja: '変更内容' },
  'audit.change.create': { ko: '생성', en: 'created', ja: '作成' },
  'audit.change.update': { ko: '수정', en: 'updated', ja: '変更' },
  'audit.change.delete': { ko: '삭제', en: 'deleted', ja: '削除' },
  'audit.change.close': { ko: '닫음', en: 'closed', ja: '終了' },
  'audit.change.secret': { ko: '변경됨 (비밀 값)', en: 'changed (secret)', ja: '変更済み (秘密値)' },
  'audit.change.none': { ko: '없음', en: 'none', ja: 'なし' },
};

export default t;
