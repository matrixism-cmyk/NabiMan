import React, { useEffect, useState } from 'react';
import { useT } from '../../../../shared/i18n';
import { readRange, SftpEntry } from '../sftpApi';
import { hexLines, sniff } from './fileSniff';
import TextPreview, { TextInfo } from './TextPreview';
import { looksBinary } from './textContent';

interface Props {
  url: string;
  entry: SftpEntry;
  onFacts: (facts: string) => void;
  onText: (info: TextInfo | null) => void;
  wrap: boolean;
  encoding: React.ComponentProps<typeof TextPreview>['encoding'];
}

/** How much of an unknown file is looked at; a text up to this shows whole. */
const HEAD = 64 * 1024;
const WHOLE_TEXT = 5 * 1024 * 1024;
/** Bytes shown as hex. */
const HEX = 4096;

/**
 * Any other file: its first bytes say what it is. Text — whatever its name —
 * is shown as text; anything else gets its kind, when known, and a hex view
 * of its start.
 */
export default function FilePreview({ url, entry, onFacts, onText, wrap, encoding }: Props) {
  const { t } = useT();
  const [head, setHead] = useState<Uint8Array | null>(null);
  const [text, setText] = useState<Blob | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    const abort = new AbortController();
    setHead(null); setText(null); setError('');
    (async () => {
      if (entry.size === 0) { setHead(new Uint8Array()); return; }
      const first = await readRange(url, 0, Math.min(entry.size, HEAD) - 1, abort.signal);
      if (!looksBinary(first)) {
        const whole = entry.size <= WHOLE_TEXT && entry.size > first.length
          ? await readRange(url, 0, entry.size - 1, abort.signal) : first;
        setText(new Blob([whole]));
        onFacts(entry.size > whole.length ? t('preview.textHead', { size: Math.round(HEAD / 1024) }) : t('preview.textUnnamed'));
        return;
      }
      setHead(first);
      onFacts(sniff(first) || t('preview.binaryUnknown'));
    })().catch((e: unknown) => { if (!abort.signal.aborted) setError(e instanceof Error ? e.message : t('common.error')); });
    return () => abort.abort();
  }, [url, entry.size, onFacts, t]);

  if (error) return <p className="preview-note" role="alert">{error}</p>;
  if (text) return <TextPreview blob={text} language="plaintext" wrap={wrap} encoding={encoding} onInfo={onText} />;
  if (!head) return <p className="preview-note">{t('preview.loading')}</p>;
  if (!head.length) return <p className="preview-note">{t('preview.empty')}</p>;
  return (
    <div className="hex-preview" tabIndex={0}>
      <pre>{hexLines(head.subarray(0, HEX)).join('\n')}</pre>
      {entry.size > HEX && <p className="preview-note">{t('preview.hexHead', { size: '4 KB' })}</p>}
    </div>
  );
}
