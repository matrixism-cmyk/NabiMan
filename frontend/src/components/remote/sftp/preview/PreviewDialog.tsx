import React, { useEffect, useMemo, useState } from 'react';
import { useT } from '../../../../i18n';
import Modal from '../../../common/Modal';
import { downloadFile, formatSize, SftpEntry, SftpSource } from '../sftpApi';
import ImagePreview from './ImagePreview';
import { previewOf } from './previewKinds';
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
  | { state: 'tooBig' }
  | { state: 'failed'; message: string };

const join = (dir: string, name: string) => (dir.endsWith('/') ? `${dir}${name}` : `${dir}/${name}`);

/**
 * A file shown in place, in a modal over the panel. ← and → (or the arrows on
 * screen) step through the other previewable files in the folder. Each kind
 * of file has its renderer; today, images.
 */
export default function PreviewDialog({ source, dir, files, start, onDownload, onClose }: Props) {
  const { t } = useT();
  const [at, setAt] = useState(() => Math.max(0, files.findIndex((f) => f.name === start)));
  const [load, setLoad] = useState<Load>({ state: 'loading', loaded: 0, total: 0 });
  const [fit, setFit] = useState(true);
  const [size, setSize] = useState<{ width: number; height: number } | null>(null);
  const entry = files[at];
  const spec = useMemo(() => (entry ? previewOf(entry.name) : null), [entry]);

  useEffect(() => {
    if (!entry || !spec) return undefined;
    setSize(null);
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
  }, [entry, spec, source, dir, t]);

  if (!entry || !spec) return null;
  const step = (by: number) => setAt((i) => (i + by + files.length) % files.length);
  const many = files.length > 1;

  const key = (e: React.KeyboardEvent) => {
    // Space and Enter on a button press that button.
    if (e.target instanceof HTMLButtonElement && (e.key === ' ' || e.key === 'Enter')) return false;
    if (e.key === 'ArrowRight' && many) { step(1); return true; }
    if (e.key === 'ArrowLeft' && many) { step(-1); return true; }
    if (e.key === 'Home' && many) { setAt(0); return true; }
    if (e.key === 'End' && many) { setAt(files.length - 1); return true; }
    if (e.key.toLowerCase() === 'f' || e.key === ' ') { setFit((f) => !f); return true; }
    return false;
  };

  const percent = load.state === 'loading' && load.total ? Math.round((load.loaded / load.total) * 100) : 0;

  return (
    <Modal className="sftp-preview" onClose={onClose} onKey={key}
      title={<span className="preview-title" title={entry.name}>{entry.name}</span>}>
      <div className="preview-bar">
        <span className="preview-facts">
          {formatSize(entry.size)}
          {size && ` · ${size.width} × ${size.height}`}
          {many && ` · ${at + 1} / ${files.length}`}
        </span>
        <span className="preview-actions">
          {spec.kind === 'image' && load.state === 'ready' && (
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => setFit((f) => !f)}>
              {fit ? t('preview.actualSize') : t('preview.fit')}
            </button>
          )}
          <button type="button" className="btn btn-secondary btn-sm" onClick={() => onDownload(entry)}>↓ {t('sftp.download')}</button>
          <button type="button" className="btn btn-secondary btn-sm" onClick={onClose} aria-label={t('common.close')}>✕</button>
        </span>
      </div>
      <div className="preview-stage">
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
        {many && <button type="button" className="preview-nav is-next" onClick={() => step(1)} aria-label={t('preview.next')}>›</button>}
      </div>
    </Modal>
  );
}
