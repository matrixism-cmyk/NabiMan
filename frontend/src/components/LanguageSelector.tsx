import React from 'react';
import { useT, LANG_LABELS, Lang } from '../i18n';

export default function LanguageSelector() {
  const { lang, setLang, t } = useT();
  const langs = Object.entries(LANG_LABELS) as [Lang, string][];

  return (
    <div className="lang-selector">
      <select
        value={lang}
        onChange={e => setLang(e.target.value as Lang)}
        className="lang-select"
        title={t('common.language')}
      >
        {langs.map(([code, label]) => (
          <option key={code} value={code}>{label}</option>
        ))}
      </select>
    </div>
  );
}
