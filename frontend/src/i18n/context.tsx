import React, { createContext, useContext, useState, useCallback } from 'react';
import translations, { Lang, LANG_LABELS } from './translations';

interface I18nContextType {
  lang: Lang;
  setLang: (lang: Lang) => void;
  t: (key: string, vars?: Record<string, string | number>) => string;
}

const I18nContext = createContext<I18nContextType>({
  lang: 'ko',
  setLang: () => {},
  t: (key) => key,
});

const STORAGE_KEY = 'nabiman_lang';

function getSavedLang(): Lang {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved === 'ko' || saved === 'en' || saved === 'ja') return saved;
  // Auto-detect from browser
  const browserLang = navigator.language.slice(0, 2);
  if (browserLang === 'ko') return 'ko';
  if (browserLang === 'ja') return 'ja';
  return 'en';
}

export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [lang, setLangState] = useState<Lang>(getSavedLang);

  const setLang = useCallback((newLang: Lang) => {
    setLangState(newLang);
    localStorage.setItem(STORAGE_KEY, newLang);
    document.documentElement.lang = newLang;
  }, []);

  const t = useCallback((key: string, vars?: Record<string, string | number>): string => {
    const entry = translations[key];
    if (!entry) return key;
    let text = entry[lang] || entry['en'] || key;
    if (vars) {
      for (const [k, v] of Object.entries(vars)) {
        text = text.replace(`{${k}}`, String(v));
      }
    }
    return text;
  }, [lang]);

  return (
    <I18nContext.Provider value={{ lang, setLang, t }}>
      {children}
    </I18nContext.Provider>
  );
}

export function useT() {
  return useContext(I18nContext);
}

export { LANG_LABELS };
export type { Lang };
