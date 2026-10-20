import React, { useEffect, useState } from 'react';
import { useT } from '../../../../i18n';
import { formatSize, readRange } from '../sftpApi';
import { findEnd, readDirectory, readZip64End, TAIL, ZipEntry } from './zipIndex';

interface Props {
  url: string;
  size: number;
  onFacts: (facts: string) => void;
}

/** A directory over this is not read. */
const MOST_DIR = 32 * 1024 * 1024;
const SHOWN = 5000;

/**
 * What a ZIP holds — and an Office document, a JAR, an APK or an e-book,
 * which are ZIPs inside — read from its directory alone: two small ranges
 * from the end of the file, however large the archive.
 */
export default function ZipPreview({ url, size, onFacts }: Props) {
  const { t } = useT();
  const [entries, setEntries] = useState<ZipEntry[] | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    const abort = new AbortController();
    setEntries(null);
    setError('');
    (async () => {
      const tailStart = Math.max(0, size - TAIL);
      const tail = await readRange(url, tailStart, size - 1, abort.signal);
      const found = findEnd(tail, tailStart);
      if (!found) throw new Error(t('preview.zipBroken'));
      let end = found.end;
      if (found.zip64At !== null) {
        const record = await readRange(url, found.zip64At, found.zip64At + 55, abort.signal);
        end = readZip64End(record, end.endOffset) || end;
      }
      if (end.dirSize > MOST_DIR) throw new Error(t('preview.zipTooBig'));
      const dir = end.dirSize ? await readRange(url, end.dirOffset, end.dirOffset + end.dirSize - 1, abort.signal) : new Uint8Array();
      const list = readDirectory(dir);
      setEntries(list);
      const files = list.filter((e) => !e.folder);
      const total = files.reduce((n, e) => n + e.size, 0);
      onFacts(t('preview.zipFacts', { count: files.length.toLocaleString(), size: formatSize(total) }));
    })().catch((e: unknown) => {
      if (!abort.signal.aborted) setError(e instanceof Error ? e.message : t('preview.zipBroken'));
    });
    return () => abort.abort();
  }, [url, size, onFacts, t]);

  if (error) return <p className="preview-note" role="alert">{error}</p>;
  if (!entries) return <p className="preview-note">{t('preview.loading')}</p>;
  return (
    <div className="zip-preview" tabIndex={0}>
      <table>
        <thead>
          <tr><th>{t('preview.zipName')}</th><th>{t('preview.zipSize')}</th><th>{t('preview.zipPacked')}</th><th>{t('preview.zipModified')}</th></tr>
        </thead>
        <tbody>
          {entries.slice(0, SHOWN).map((e, i) => (
            <tr key={i} className={e.folder ? 'is-folder' : ''}>
              <td title={e.name}>{e.folder ? '📁 ' : ''}{e.name}{e.encrypted ? ' 🔒' : ''}</td>
              <td>{e.folder ? '' : formatSize(e.size)}</td>
              <td>{e.folder ? '' : formatSize(e.compressed)}</td>
              <td>{e.mtime ? new Date(e.mtime * 1000).toISOString().slice(0, 16).replace('T', ' ') : ''}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {entries.length > SHOWN && <p className="preview-note">{t('preview.zipMore', { count: (entries.length - SHOWN).toLocaleString() })}</p>}
    </div>
  );
}
