import React, { useCallback, useEffect, useState } from 'react';
import { useT } from '../../shared/i18n';
import { DriveFile, fetchDriveFile, listDriveFiles } from './vncApi';
import { saveBlob } from './consoleDownload';

/** How often the open panel looks at the drive again. */
const REFRESH_MS = 3000;

const fmtSize = (n: number) =>
  (n < 1024 ? `${n} B` : n < 1048576 ? `${Math.round(n / 1024)} KB` : `${(n / 1048576).toFixed(1)} MB`);

/**
 * Browse and download the VM's redirected "NabiMan" drive. The drive is a host
 * directory guacd shares into Windows: drag-and-drop uploads land there, and a
 * file the user copies onto that drive in the VM shows up here to download.
 * Read server-side (NabiMan lists the directory), not back over RDPDR, and
 * looked at again every few seconds while open, so a copy shows up by itself.
 */
export default function RdpDrivePanel({ target, onClose }: { target: string; onClose: () => void }) {
  const { t } = useT();
  const [files, setFiles] = useState<DriveFile[] | null>(null);
  const [busy, setBusy] = useState('');

  const reload = useCallback(() => {
    listDriveFiles(target).then((r) => { if (r.success && r.data) setFiles(r.data); else setFiles((f) => f || []); })
      .catch(() => setFiles((f) => f || []));
  }, [target]);
  useEffect(() => {
    reload();
    const timer = window.setInterval(() => { if (!document.hidden) reload(); }, REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [reload]);

  const download = async (name: string) => {
    setBusy(name);
    try {
      const res = await fetchDriveFile(target, name);
      if (!res.ok) throw new Error('download failed');
      saveBlob(await res.blob(), name);
    } catch { /* ignore */ } finally { setBusy(''); }
  };

  const sorted = files ? [...files].sort((a, b) => b.modified - a.modified) : [];
  return (
    <div className="rw-drive-panel">
      <div className="rw-drive-head">
        <strong>{t('remote.console.drive.files')}</strong>
        <span className="rw-drive-spacer" />
        <button className="btn btn-secondary btn-sm" title={t('common.refresh')} onClick={reload}>↻</button>
        <button className="btn btn-secondary btn-sm" onClick={onClose}>✕</button>
      </div>
      <div className="rw-drive-list">
        {!files && <p className="text-secondary">{t('common.loading')}</p>}
        {files && files.length === 0 && <p className="text-secondary">{t('remote.console.drive.empty')}</p>}
        {sorted.map((f) => (
          <button key={f.name} className="rw-drive-row" disabled={busy === f.name} onClick={() => download(f.name)} title={f.name}>
            ⬇ <span className="rw-drive-name">{f.name}</span>
            <span className="rw-drive-size">{busy === f.name ? '…' : fmtSize(f.size)}</span>
          </button>
        ))}
      </div>
      <p className="rw-drive-note">{t('remote.console.drive.downloadNote')}</p>
    </div>
  );
}
