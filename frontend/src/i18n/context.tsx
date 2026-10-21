import React from 'react';
import { I18nProvider as Frame, useT, LANG_LABELS } from '../shared/i18n';
import type { Lang } from '../shared/i18n';
import translations from './translations';

/** NabiMan's languages: the shared frame with all of NabiMan's words. */
export function I18nProvider({ children }: { children: React.ReactNode }) {
  return <Frame translations={translations}>{children}</Frame>;
}

export { useT, LANG_LABELS };
export type { Lang };
