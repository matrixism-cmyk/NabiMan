import React from 'react';
import { useT } from '../../../i18n';
import { formatSize } from './sftpApi';
import { TransferRow } from './useTransfers';

interface Props {
  rows: TransferRow[];
  onCancel: (id: number) => void;
  onClear: () => void;
}

/** Uploads and downloads in progress, and how the finished ones ended. */
export default function SftpTransfers({ rows, onCancel, onClear }: Props) {
  const { t } = useT();
  if (!rows.length) return null;
  const finished = rows.some((r) => r.status !== 'waiting' && r.status !== 'running');
  return (
    <div className="sftp-transfers" aria-live="polite">
      <div className="sftp-transfers-head">
        <span>{t('sftp.transfers')}</span>
        {finished && <button type="button" className="sftp-link" onClick={onClear}>{t('sftp.clearDone')}</button>}
      </div>
      {rows.map((row) => {
        const percent = row.total > 0 ? Math.min(100, Math.round((row.loaded / row.total) * 100)) : null;
        const active = row.status === 'waiting' || row.status === 'running';
        return (
          <div key={row.id} className={`sftp-transfer is-${row.status}`} title={row.message || row.name}>
            <span className="sftp-transfer-dir" aria-hidden="true">{row.direction === 'up' ? '↑' : '↓'}</span>
            <span className="sftp-transfer-name">{row.name}</span>
            <span className="sftp-transfer-state">
              {row.status === 'running' && (percent !== null ? `${percent}%` : formatSize(row.loaded))}
              {row.status === 'waiting' && t('sftp.waiting')}
              {row.status === 'done' && (row.message || t('sftp.done'))}
              {row.status === 'failed' && (row.message || t('sftp.failed'))}
              {row.status === 'cancelled' && (row.message || t('sftp.cancelled'))}
            </span>
            {active && (
              <button type="button" className="sftp-link" onClick={() => onCancel(row.id)}
                aria-label={t('sftp.cancel')}>✕</button>
            )}
            {row.status === 'running' && percent !== null && (
              <span className="sftp-bar" style={{ width: `${percent}%` }} aria-hidden="true" />
            )}
          </div>
        );
      })}
    </div>
  );
}
