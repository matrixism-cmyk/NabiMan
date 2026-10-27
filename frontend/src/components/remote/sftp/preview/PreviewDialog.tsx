import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useT } from '../../../../shared/i18n';
import Modal from '../../../../shared/ui/Modal';
import { downloadFile, formatSize, SftpEntry, SftpSource, streamLink } from '../sftpApi';
import ImagePreview from './ImagePreview';
import TextPreview, { TextInfo } from './TextPreview';
import PdfPreview from './PdfPreview';
import MediaPreview from './MediaPreview';
import ZipPreview from './ZipPreview';
import FilePreview from './FilePreview';
import { previewOf, STREAMED } from './previewKinds';
import { Encoding, ENCODINGS } from './textContent';
import './preview.css';

interface Props {
  source: SftpSource;
  /** The folder the files are in. */
  dir: string;
  /** The files that can be previewed, in the order the list shows them. */
  files: SftpEntry[];
  /** The one to show first. */
  start: string;
  onDownload: (entry: SftpEntry) => void;
  onClose: () => void;
}

type Load =
  | { state: 'loading'; loaded: number; total: number }
  | { state: 'ready'; blob: Blob }
  /** Read a range at a time through this link (media, archives, other files). */
  | { state: 'link'; url: string }
  | { state: 'tooBig' }
  | { state: 'failed'; message: string };

const join = (dir: string, name: string) => (dir.endsWith('/') ? `${dir}${name}` : `${dir}/${name}`);

const WRAP_KEY = 'nabiman_preview_wrap';
const readWrap = () => { try { return localStorage.getItem(WRAP_KEY) === '1'; } catch { return false; } };

/**
 * A file shown in place, in a modal over the panel. ← and → (or the arrows on
 * screen) step through the other previewable files in the folder. Each kind
 * of file has its renderer: images, text and source code, PDF, video and
 * audio, what a ZIP holds, and for anything else what it is and its start.
 */
export default function PreviewDialog({ source, dir, files, start, onDownload, onClose }: Props) {
  const { t } = useT();
  // The file shown is followed by name: the panel re-reads the folder every
  // few seconds, which brings new entries and can move this one in the list.
  const [name, setName] = useState(start);
  const [load, setLoad] = useState<Load>({ state: 'loading', loaded: 0, total: 0 });
  const [fit, setFit] = useState(true);
  const [size, setSize] = useState<{ width: number; height: number } | null>(null);
  const [wrap, setWrapState] = useState(readWrap);
  const [encoding, setEncoding] = useState<Encoding>('auto');
  const [text, setText] = useState<TextInfo | null>(null);
  const [copied, setCopied] = useState(false);
  const [pages, setPages] = useState<number | null>(null);
  const [facts, setFacts] = useState('');
  const found = files.findIndex((f) => f.name === name);
  const at = Math.max(0, found);
  // A file deleted while shown stays on screen as it was.
  const shown = useRef<SftpEntry | undefined>(undefined);
  if (found >= 0) shown.current = files[found];
  const entry = shown.current || files[0];
  const shownName = entry?.name;
  const spec = useMemo(() => (shownName ? previewOf(shownName) : null), [shownName]);
  // Fetched again only when the file itself changes, not each time the folder is re-read.
  const stamp = entry ? `${entry.name}\0${entry.size}\0${entry.mtime}` : '';

  useEffect(() => {
    if (!entry || !spec) return undefined;
    setSize(null);
    setText(null);
    setPages(null);
    setFacts('');
    setCopied(false);
    setEncoding('auto');
    if (STREAMED.has(spec.kind)) {
      let live = true;
      setLoad({ state: 'loading', loaded: 0, total: 0 });
      streamLink(source, join(dir, entry.name))
        .then((url) => { if (live) setLoad({ state: 'link', url }); })
        .catch((e: unknown) => { if (live) setLoad({ state: 'failed', message: e instanceof Error ? e.message : t('common.error') }); });
      return () => { live = false; };
    }
    if (entry.size > spec.maxBytes) { setLoad({ state: 'tooBig' }); return undefined; }
    setLoad({ state: 'loading', loaded: 0, total: entry.size });
    const transfer = downloadFile(source, join(dir, entry.name), (loaded, total) =>
      setLoad((now) => (now.state === 'loading' ? { state: 'loading', loaded, total: total || entry.size } : now)));
    let live = true;
    transfer.promise.then((r) => {
      if (!live || r.aborted) return;
      if (r.ok && r.blob) setLoad({ state: 'ready', blob: new Blob([r.blob], { type: spec.mime }) });
      else setLoad({ state: 'failed', message: r.message || t('common.error') });
    });
    return () => { live = false; transfer.abort(); };
  // eslint-disable-next-line react-hooks/exhaustive-deps -- `stamp` stands for `entry`
  }, [stamp, spec, source, dir, t]);

  if (!entry || !spec) return null;
  const step = (by: number) => setName(files[(at + by + files.length) % files.length].name);
  const many = files.length > 1;

  const key = (e: React.KeyboardEvent) => {
    // Space and Enter on a button press that button; a player takes its own keys.
    if (e.target instanceof HTMLButtonElement && (e.key === ' ' || e.key === 'Enter')) return false;
    if (e.target instanceof HTMLMediaElement && e.key !== 'Escape') return false;
    if (e.key === 'ArrowRight' && many) { step(1); return true; }
    if (e.key === 'ArrowLeft' && many) { step(-1); return true; }
    if (e.key === 'Home' && many) { setName(files[0].name); return true; }
    if (e.key === 'End' && many) { setName(files[files.length - 1].name); return true; }
    if (spec.kind === 'image' && (e.key.toLowerCase() === 'f' || e.key === ' ')) { setFit((f) => !f); return true; }
    if (spec.kind === 'text' && e.key.toLowerCase() === 'w' && !e.ctrlKey && !e.metaKey) { setWrap(!wrap); return true; }
    return false;
  };

  const setWrap = (next: boolean) => {
    setWrapState(next);
    try { localStorage.setItem(WRAP_KEY, next ? '1' : '0'); } catch { /* private mode */ }
  };

  const copy = () => {
    if (!text) return;
    navigator.clipboard?.writeText(text.text).then(() => setCopied(true), () => setCopied(false));
  };

  const percent = load.state === 'loading' && load.total ? Math.round((load.loaded / load.total) * 100) : 0;

  return (
    <Modal className="sftp-preview" onClose={onClose} onKey={key}
      title={<span className="preview-title" title={entry.name}>{entry.name}</span>}>
      <div className="preview-bar">
        <span className="preview-facts">
          {formatSize(entry.size)}
          {size && ` · ${size.width} × ${size.height}`}
          {text && ` · ${t('preview.lines', { count: text.lines.toLocaleString() })} · ${text.encoding.toUpperCase()}`}
          {text && spec.language && spec.language !== 'plaintext' && ` · ${spec.language}`}
          {pages !== null && ` · ${t('preview.pages', { count: pages.toLocaleString() })}`}
          {facts && ` · ${facts}`}
          {many && ` · ${at + 1} / ${files.length}`}
        </span>
        <span className="preview-actions">
          {spec.kind === 'image' && load.state === 'ready' && (
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => setFit((f) => !f)}>
              {fit ? t('preview.actualSize') : t('preview.fit')}
            </button>
          )}
          {(text || (spec.kind === 'text' && load.state === 'ready')) && (
            <>
              <button type="button" className={`btn btn-sm btn-secondary ${wrap ? 'is-on' : ''}`} aria-pressed={wrap}
                onClick={() => setWrap(!wrap)} title={t('preview.wrapHint')}>{t('preview.wrap')}</button>
              <select className="select-input preview-encoding" value={encoding} aria-label={t('preview.encoding')}
                onChange={(e) => setEncoding(e.target.value as Encoding)}>
                {ENCODINGS.map((enc) => <option key={enc} value={enc}>{enc === 'auto' ? t('preview.encodingAuto') : enc.toUpperCase()}</option>)}
              </select>
              <button type="button" className="btn btn-secondary btn-sm" onClick={copy} disabled={!text}>
                {copied ? t('preview.copied') : t('preview.copy')}
              </button>
            </>
          )}
          <button type="button" className="btn btn-secondary btn-sm" onClick={() => onDownload(entry)}>↓ {t('sftp.download')}</button>
          <button type="button" className="btn btn-secondary btn-sm" onClick={onClose} aria-label={t('common.close')}>✕</button>
        </span>
      </div>
      <div className={`preview-stage is-${spec.kind}`}>
        {many && <button type="button" className="preview-nav is-prev" onClick={() => step(-1)} aria-label={t('preview.previous')}>‹</button>}
        {load.state === 'loading' && (
          <div className="preview-note" role="status">
            {t('preview.loading')}{percent ? ` ${percent}%` : ''}
            <progress max={100} value={percent} />
          </div>
        )}
        {load.state === 'tooBig' && <p className="preview-note">{t('preview.tooBig', { size: formatSize(spec.maxBytes) })}</p>}
        {load.state === 'failed' && <p className="preview-note" role="alert">{load.message}</p>}
        {load.state === 'ready' && spec.kind === 'image' && (
          <ImagePreview blob={load.blob} name={entry.name} fit={fit} onToggleFit={() => setFit((f) => !f)} onSize={setSize} />
        )}
        {load.state === 'link' && (spec.kind === 'video' || spec.kind === 'audio') && (
          <MediaPreview url={load.url} kind={spec.kind} name={entry.name} onFacts={setFacts} />
        )}
        {load.state === 'link' && spec.kind === 'zip' && <ZipPreview url={load.url} size={entry.size} onFacts={setFacts} />}
        {load.state === 'link' && spec.kind === 'file' && (
          <FilePreview url={load.url} entry={entry} onFacts={setFacts} onText={setText} wrap={wrap} encoding={encoding} />
        )}
        {load.state === 'ready' && spec.kind === 'pdf' && (
          <PdfPreview blob={load.blob} onInfo={(info) => setPages(info ? info.pages : null)} />
        )}
        {load.state === 'ready' && spec.kind === 'text' && (
          <TextPreview blob={load.blob} language={spec.language || 'plaintext'} wrap={wrap} encoding={encoding} onInfo={setText} />
        )}
        {many && <button type="button" className="preview-nav is-next" onClick={() => step(1)} aria-label={t('preview.next')}>›</button>}
      </div>
    </Modal>
  );
}
