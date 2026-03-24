import common from './common';
import monitoring from './monitoring';
import management from './management';
import security from './security';

export type Lang = 'ko' | 'en' | 'ja';

export const LANG_LABELS: Record<Lang, string> = {
  ko: '한국어',
  en: 'English',
  ja: '日本語',
};

const translations: Record<string, Record<Lang, string>> = {
  ...common,
  ...monitoring,
  ...management,
  ...security,
};

export default translations;
