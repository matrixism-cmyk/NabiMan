import React, { createContext, useContext, useState, useCallback, useEffect } from 'react';
import { setFormatLang } from './format';

export type Lang = 'ko' | 'en' | 'ja';
/** Each key's words in every language. */
export type Translations = Record<string, Record<Lang, string>>;

export const LANG_LABELS: Record<Lang, string> = {
  ko: '한국어', // i18n-exempt: each language is named in its own script
  en: 'English',
  ja: '日本語',
};

interface I18nContextType {
  lang: Lang;
  setLang: (lang: Lang) => void;
  t: TFn;
}

/** The translate function, for helpers outside components that take it as an argument. */
export type TFn = (key: string, vars?: Record<string, string | number>) => string;

const I18nContext = createContext<I18nContextType>({
  lang: 'ko',
  setLang: () => {},
  t: (key) => key,
});

const STORAGE_KEY = 'nabiman_lang';

function getSavedLang(): Lang {
  let saved: string | null = null;
  try { saved = localStorage.getItem(STORAGE_KEY); } catch { /* storage blocked: detect from the browser */ }
  if (saved === 'ko' || saved === 'en' || saved === 'ja') return saved;
  // Auto-detect from browser
  const browserLang = navigator.language.slice(0, 2);
  if (browserLang === 'ko') return 'ko';
  if (browserLang === 'ja') return 'ja';
  return 'en';
}

/** `{name}` placeholders filled in, every occurrence of each. */
export function interpolate(text: string, vars?: Record<string, string | number>): string {
  if (!vars) return text;
  let out = text;
  for (const [k, v] of Object.entries(vars)) out = out.split(`{${k}}`).join(String(v));
  return out;
}

/**
 * Languages for a product built on NabiMan, with the words it brings. The
 * frame knows no words of its own; a missing key shows as itself.
 */
export function I18nProvider({ translations, children }: { translations: Translations; children: React.ReactNode }) {
  const [lang, setLangState] = useState<Lang>(getSavedLang);
  // Before the children render, so the dates they format follow this language.
  setFormatLang(lang);

  const setLang = useCallback((newLang: Lang) => {
    setLangState(newLang);
    try { localStorage.setItem(STORAGE_KEY, newLang); } catch { /* storage blocked: the choice lasts this visit */ }
  }, []);
  // index.html starts as ko; screen readers, fonts and hyphenation follow this.
  useEffect(() => { document.documentElement.lang = lang; }, [lang]);

  const t = useCallback((key: string, vars?: Record<string, string | number>): string => {
    const entry = translations[key];
    if (!entry) return key;
    return interpolate(entry[lang] || entry['en'] || key, vars);
  }, [lang, translations]);

  return (
    <I18nContext.Provider value={{ lang, setLang, t }}>
      {children}
    </I18nContext.Provider>
  );
}

export function useT() {
  return useContext(I18nContext);
}

