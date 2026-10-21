import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useT } from '../../../../shared/i18n';
import { decode, Encoding, linesOf, looksBinary, splitHighlighted } from './textContent';

/** What the dialog shows about a text once it is read. */
export interface TextInfo {
  lines: number;
  encoding: string;
  /** The whole text, for copying. */
  text: string;
  highlighted: boolean;
}

interface Props {
  blob: Blob;
  language: string;
  wrap: boolean;
  encoding: Encoding;
  onInfo: (info: TextInfo | null) => void;
}

/** Up to this size each line is its own row, coloured; beyond it, one plain block. */
const ROWS_LIMIT = 300 * 1024;

/**
 * A text or source file with line numbers, coloured by its language when it
 * is small enough. The colouring library is loaded only when a text is shown.
 */
export default function TextPreview({ blob, language, wrap, encoding, onInfo }: Props) {
  const { t } = useT();
  const [bytes, setBytes] = useState<Uint8Array | null>(null);
  const [anyway, setAnyway] = useState(false);
  const [coloured, setColoured] = useState<string[] | null>(null);
  const boxRef = useRef<HTMLDivElement>(null);
  const infoRef = useRef(onInfo);
  infoRef.current = onInfo;

  useEffect(() => {
    let live = true;
    setBytes(null);
    setAnyway(false);
    blob.arrayBuffer().then((buf) => { if (live) setBytes(new Uint8Array(buf)); });
    return () => { live = false; };
  }, [blob]);

  const binary = bytes ? looksBinary(bytes) && !anyway : false;
  const decoded = useMemo(() => (bytes && !binary ? decode(bytes, encoding) : null), [bytes, binary, encoding]);
  const lines = useMemo(() => (decoded ? linesOf(decoded.text) : []), [decoded]);
  const asRows = Boolean(decoded) && (decoded?.text.length || 0) <= ROWS_LIMIT;

  useEffect(() => {
    setColoured(null);
    if (!decoded || !asRows || language === 'plaintext') return undefined;
    let live = true;
    import('./highlight').then(({ highlight }) => {
      if (live) setColoured(splitHighlighted(highlight(decoded.text, language)));
    }).catch(() => { /* plain text is still there */ });
    return () => { live = false; };
  }, [decoded, asRows, language]);

  useEffect(() => {
    infoRef.current(decoded
      ? { lines: lines.length, encoding: decoded.encoding, text: decoded.text, highlighted: Boolean(coloured) }
      : null);
  }, [decoded, lines, coloured]);

  // The text takes the keyboard, so arrows and Page Up/Down scroll it.
  useEffect(() => { if (decoded) boxRef.current?.focus({ preventScroll: true }); }, [decoded]);

  if (!bytes) return <p className="preview-note">{t('preview.loading')}</p>;
  if (binary) {
    return (
      <div className="preview-note">
        {t('preview.binary')}
        <button type="button" className="btn btn-secondary btn-sm" onClick={() => setAnyway(true)}>{t('preview.showAnyway')}</button>
      </div>
    );
  }
  if (!decoded) return null;

  const gutter = `${String(lines.length).length + 1}ch`;
  if (!asRows) {
    // A large file: two plain blocks are far lighter than a row per line.
    return (
      <div ref={boxRef} className="text-preview is-block" tabIndex={0} style={{ '--tp-gutter': gutter } as React.CSSProperties}>
        <pre className="tp-gutter" aria-hidden="true">{lines.map((_, i) => i + 1).join('\n')}</pre>
        <pre className="tp-text">{decoded.text}</pre>
      </div>
    );
  }
  return (
    <div ref={boxRef} className={`text-preview hljs ${wrap ? 'is-wrap' : ''}`} tabIndex={0}
      style={{ '--tp-gutter': gutter } as React.CSSProperties}>
      {lines.map((line, i) => (
        <div className="tp-row" key={i}>
          <span className="tp-no" aria-hidden="true">{i + 1}</span>
          {coloured
            ? <code className="tp-code" dangerouslySetInnerHTML={{ __html: coloured[i] || ' ' }} />
            : <code className="tp-code">{line || ' '}</code>}
        </div>
      ))}
    </div>
  );
}
