import React from 'react';
import { useT } from '../../shared/i18n';

interface Props {
  title: string;
  address: string;
  /** Sessions already running on the target, offered instead of a new one. */
  liveCount: number;
  /** The target has a stored password, so typing one by hand is a choice. */
  hasPassword: boolean;
  onConnect: (manualPassword: boolean) => void;
  onShowRunning: () => void;
}

/** What a target shows before it is dialled: connect, or join what is running. */
export default function ConnectCard({ title, address, liveCount, hasPassword, onConnect, onShowRunning }: Props) {
  const { t } = useT();
  return (
    <div className="rw-view rw-connect">
      <div className="rw-connect-card">
        <h3>{title}</h3>
        <code>{address}</code>
        <p className="text-secondary">{t('remote.connectHint')}</p>
        <div className="btn-group">
          <button className="btn btn-primary" onClick={() => onConnect(false)}>{t('remote.connect')}</button>
          {liveCount > 0 && (
            <button className="btn btn-secondary" onClick={onShowRunning}>
              {t('remote.showRunning', { count: liveCount })}
            </button>
          )}
          {hasPassword && (
            <button className="btn btn-secondary" onClick={() => onConnect(true)} title={t('remote.manualPasswordHint')}>
              ⌨ {t('remote.manualPassword')}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
