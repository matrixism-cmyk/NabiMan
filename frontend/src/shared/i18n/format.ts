import type { Lang } from './context';

/**
 * Dates and numbers in the language picked in NabiMan, not the browser's:
 * an English screen says "3:04 PM", not "오후 3:04", whatever the browser is
 * set to. The I18nProvider keeps the language here as it changes, so helpers
 * outside components format the same way.
 */
export const LOCALES: Record<Lang, string> = { ko: 'ko-KR', en: 'en-US', ja: 'ja-JP' };

let current: Lang = 'ko';

/** Called by the I18nProvider whenever the language is set. */
export function setFormatLang(lang: Lang) { current = lang; }

/** The BCP 47 locale for a language (the current one by default). */
export function localeOf(lang: Lang = current): string { return LOCALES[lang] ?? LOCALES.en; }

/** A Date, an ISO string, or milliseconds since the epoch (a unix second times 1000). */
export type DateInput = Date | string | number;

function toDate(value: DateInput): Date | null {
  const date = value instanceof Date ? value : new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

function format(value: DateInput, kind: 'all' | 'date' | 'time', opts?: Intl.DateTimeFormatOptions, lang?: Lang): string {
  const date = toDate(value);
  if (!date) return String(value);
  const locale = localeOf(lang);
  if (kind === 'date') return date.toLocaleDateString(locale, opts);
  if (kind === 'time') return date.toLocaleTimeString(locale, opts);
  return date.toLocaleString(locale, opts);
}

/** Date and time; an unreadable value comes back as it was given. */
export function formatDateTime(value: DateInput, opts?: Intl.DateTimeFormatOptions, lang?: Lang): string {
  return format(value, 'all', opts, lang);
}

/** The date alone. */
export function formatDate(value: DateInput, opts?: Intl.DateTimeFormatOptions, lang?: Lang): string {
  return format(value, 'date', opts, lang);
}

/** The time of day alone. */
export function formatTime(value: DateInput, opts?: Intl.DateTimeFormatOptions, lang?: Lang): string {
  return format(value, 'time', opts, lang);
}

/** A number with the language's digit grouping. */
export function formatNumber(value: number, opts?: Intl.NumberFormatOptions, lang?: Lang): string {
  return value.toLocaleString(localeOf(lang), opts);
}
