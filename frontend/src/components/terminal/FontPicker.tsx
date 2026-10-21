import React from 'react';
import { useT } from '../../shared/i18n';
import { fontOf, TERMINAL_FONTS } from './terminalFonts';

/** What shows a coding font's character: look-alikes, brackets, box lines, Korean, alignment. */
const SAMPLE = ['0O oO 1lI| {}[]() => != ->', '│ 한글 정렬 확인 │ abc │', '└──────────────┴─────┘'];

/** The terminal font, with a sample in it before saving. */
export default function FontPicker({ value, size, onChange }: { value: string; size: number; onChange: (id: string) => void }) {
  const { t } = useT();
  return (
    <div className="form-group">
      <label htmlFor="term-font">{t('terminal.font')}</label>
      <select id="term-font" className="select-input" value={fontOf(value).id} onChange={(e) => onChange(e.target.value)}>
        {TERMINAL_FONTS.map((f) => (
          <option key={f.id} value={f.id}>{f.id === 'system' ? t('terminal.fontSystem') : f.label}</option>
        ))}
      </select>
      <pre className="term-font-sample" style={{ fontFamily: fontOf(value).family, fontSize: size }} aria-label={t('terminal.fontSample')}>
        {SAMPLE.join('\n')}
      </pre>
      <small className="text-secondary">{t('terminal.fontHint')}</small>
    </div>
  );
}
