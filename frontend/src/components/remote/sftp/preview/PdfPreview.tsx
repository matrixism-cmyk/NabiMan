import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useT } from '../../../../shared/i18n';
import PdfPage from './PdfPage';
import type { PDFDocumentProxy } from './pdf';

interface Props {
  blob: Blob;
  onInfo: (info: { pages: number } | null) => void;
}

type Pdf = typeof import('./pdf');
type Zoom = number | 'width';

const STEPS = [0.5, 0.67, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4];
/** Room around the pages when fitting them to the width. */
const MARGIN = 40;

/**
 * A PDF drawn by pdf.js inside the preview: pages one under another, fitted
 * to the width or zoomed (−, +, 0), with the page in view shown and any page
 * a number away. Pages are drawn as they come near, so a long document opens
 * as fast as a short one. A password-protected file asks for its password.
 */
export default function PdfPreview({ blob, onInfo }: Props) {
  const { t } = useT();
  const [lib, setLib] = useState<Pdf | null>(null);
  const [doc, setDoc] = useState<PDFDocumentProxy | null>(null);
  const [guess, setGuess] = useState({ width: 612, height: 792 });
  const [error, setError] = useState('');
  const [asking, setAsking] = useState<{ retry: boolean } | null>(null);
  const [password, setPassword] = useState('');
  const [zoom, setZoom] = useState<Zoom>('width');
  const [width, setWidth] = useState(0);
  const [page, setPage] = useState(1);
  const [pageText, setPageText] = useState('1');
  const scrollRef = useRef<HTMLDivElement>(null);
  const answer = useRef<((pw: string) => void) | null>(null);
  const keep = useRef<{ page: number; within: number } | null>(null);
  const infoRef = useRef(onInfo);
  infoRef.current = onInfo;

  useEffect(() => {
    let live = true;
    // Destroying the loading task closes the document and its worker.
    let task: { destroy: () => Promise<void> } | null = null;
    setDoc(null); setError(''); setAsking(null);
    Promise.all([import('./pdf'), blob.arrayBuffer()]).then(async ([pdf, buf]) => {
      if (!live) return;
      setLib(pdf);
      const loading = pdf.openPdf(new Uint8Array(buf), (retry) => new Promise<string>((resolve) => {
        answer.current = resolve;
        setAsking({ retry });
      }));
      task = loading;
      const loaded = await loading.promise;
      if (!live) return;
      const first = (await loaded.getPage(1)).getViewport({ scale: 1 });
      if (!live) return;
      setAsking(null);
      setGuess({ width: first.width, height: first.height });
      setDoc(loaded);
      infoRef.current({ pages: loaded.numPages });
    }).catch((e: unknown) => {
      if (live) setError(e instanceof Error && e.name === 'PasswordException' ? t('preview.pdfLocked') : t('preview.pdfBroken'));
    });
    return () => {
      live = false;
      infoRef.current(null);
      task?.destroy();
    };
  }, [blob, t]);

  // Fitting to the width follows the window.
  useEffect(() => {
    const box = scrollRef.current;
    if (!box) return undefined;
    const observer = new ResizeObserver(() => setWidth(box.clientWidth));
    observer.observe(box);
    setWidth(box.clientWidth);
    return () => observer.disconnect();
  }, [doc]);

  const scale = zoom === 'width' ? Math.max(0.25, (width - MARGIN) / guess.width) : zoom;

  /** The page whose middle is nearest the top third of the view. */
  const pageInView = useCallback(() => {
    const box = scrollRef.current;
    if (!box) return { page: 1, within: 0 };
    const line = box.scrollTop + box.clientHeight / 3;
    const pages = Array.from(box.querySelectorAll<HTMLElement>('.pdf-page'));
    const at = pages.find((p) => p.offsetTop + p.offsetHeight > line) || pages[pages.length - 1];
    if (!at) return { page: 1, within: 0 };
    return { page: Number(at.dataset.page), within: (box.scrollTop - at.offsetTop) / at.offsetHeight };
  }, []);

  const onScroll = () => {
    const now = pageInView().page;
    if (now !== page) { setPage(now); setPageText(String(now)); }
  };

  // A zoom keeps the same place on the same page in view.
  const changeZoom = (next: Zoom) => { keep.current = pageInView(); setZoom(next); };
  useLayoutEffect(() => {
    const box = scrollRef.current;
    const held = keep.current;
    keep.current = null;
    if (!box || !held) return;
    const el = box.querySelector<HTMLElement>(`.pdf-page[data-page="${held.page}"]`);
    if (el) box.scrollTop = el.offsetTop + held.within * el.offsetHeight;
  }, [scale]);

  const step = (by: 1 | -1) => {
    const now = zoom === 'width' ? scale : zoom;
    const next = by > 0 ? STEPS.find((s) => s > now + 0.001) : [...STEPS].reverse().find((s) => s < now - 0.001);
    if (next) changeZoom(next);
  };

  const goTo = (n: number) => {
    if (!doc) return;
    const target = Math.min(doc.numPages, Math.max(1, Math.round(n)));
    scrollRef.current?.querySelector(`.pdf-page[data-page="${target}"]`)?.scrollIntoView({ block: 'start' });
    setPage(target); setPageText(String(target));
  };

  const keyDown = (e: React.KeyboardEvent) => {
    if (e.target instanceof HTMLInputElement) return;
    const handled = e.key === '+' || e.key === '=' ? (step(1), true)
      : e.key === '-' ? (step(-1), true)
      : e.key === '0' ? (changeZoom('width'), true) : false;
    if (handled) { e.preventDefault(); e.stopPropagation(); }
  };

  useEffect(() => { if (doc) scrollRef.current?.focus({ preventScroll: true }); }, [doc]);

  if (asking) {
    return (
      <form className="preview-note pdf-password" onSubmit={(e) => { e.preventDefault(); answer.current?.(password); setPassword(''); }}>
        <span>{t(asking.retry ? 'preview.pdfWrongPassword' : 'preview.pdfNeedsPassword')}</span>
        <input type="password" className="filter-input" value={password} autoFocus aria-label={t('preview.pdfPassword')}
          onChange={(e) => setPassword(e.target.value)} />
        <button type="submit" className="btn btn-primary btn-sm">{t('preview.pdfOpen')}</button>
      </form>
    );
  }
  if (error) return <p className="preview-note" role="alert">{error}</p>;

  return (
    <div className="pdf-preview">
      <div className="pdf-tools">
        <button type="button" className="btn btn-secondary btn-sm" onClick={() => goTo(page - 1)} disabled={!doc || page <= 1}
          aria-label={t('preview.pdfPrev')}>▲</button>
        <input className="filter-input pdf-page-input" value={pageText} inputMode="numeric" aria-label={t('preview.pdfPage')}
          onChange={(e) => setPageText(e.target.value.replace(/\D/g, ''))}
          onKeyDown={(e) => { if (e.key === 'Enter') { e.stopPropagation(); goTo(Number(pageText) || 1); } }}
          onBlur={() => setPageText(String(page))} />
        <span className="pdf-page-count">/ {doc?.numPages ?? '…'}</span>
        <button type="button" className="btn btn-secondary btn-sm" onClick={() => goTo(page + 1)} disabled={!doc || page >= doc.numPages}
          aria-label={t('preview.pdfNext')}>▼</button>
        <span className="pdf-tools-gap" />
        <button type="button" className="btn btn-secondary btn-sm" onClick={() => step(-1)} aria-label={t('preview.zoomOut')}>−</button>
        <span className="pdf-zoom">{Math.round(scale * 100)}%</span>
        <button type="button" className="btn btn-secondary btn-sm" onClick={() => step(1)} aria-label={t('preview.zoomIn')}>+</button>
        <button type="button" className={`btn btn-secondary btn-sm ${zoom === 'width' ? 'is-on' : ''}`}
          onClick={() => changeZoom('width')}>{t('preview.fitWidth')}</button>
      </div>
      <div ref={scrollRef} className="pdf-scroll" tabIndex={0} onScroll={onScroll} onKeyDown={keyDown}>
        {!doc || !lib ? <p className="preview-note">{t('preview.loading')}</p> : (
          Array.from({ length: doc.numPages }, (_, i) => (
            <PdfPage key={i + 1} doc={doc} number={i + 1} scale={scale} guess={guess} root={scrollRef.current}
              textLayer={lib.TextLayer} />
          ))
        )}
      </div>
    </div>
  );
}
