import React, { useCallback, useEffect, useState } from 'react';
import { useT } from '../../shared/i18n';
import { DriveFile, fetchDriveFile, listDriveFiles } from './vncApi';

const fmtSize = (n: number) =>
  (n < 1024 ? `${n} B` : n < 1048576 ? `${Math.round(n / 1024)} KB` : `${(n / 1048576).toFixed(1)} MB`);

/**
 * Browse and download the VM's redirected "NabiMan" drive. The drive is a host
 * directory guacd shares into Windows: drag-and-drop uploads land there, and a
 * file the user copies onto that drive in the VM shows up here to download.
 * Read server-side (NabiMan lists the directory), not back over RDPDR.
 */
export default function RdpDrivePanel({ target, onClose }: { target: string; onClose: () => void }) {
  const { t } = useT();
  const [files, setFiles] = useState<DriveFile[] | null>(null);
  const [busy, setBusy] = useState('');

  const reload = useCallback(() => {
    setFiles(null);
    listDriveFiles(target).then((r) => setFiles(r.success && r.data ? r.data : [])).catch(() => setFiles([]));
  }, [target]);
  useEffect(reload, [reload]);

  const download = async (name: string) => {
    setBusy(name);
    try {
      const res = await fetchDriveFile(target, name);
      if (!res.ok) throw new Error('download failed');
      const url = URL.createObjectURL(await res.blob());
      const a = document.createElement('a');
      a.href = url; a.download = name;
      document.body.appendChild(a); a.click(); a.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 10000);
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
    </div>
  );
}
