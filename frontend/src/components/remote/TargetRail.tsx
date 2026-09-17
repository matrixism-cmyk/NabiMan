import React from 'react';
import { RemoteServer } from '../../types';
import { useT } from '../../i18n';
import { Selection } from './workspaceState';

interface Props {
  servers: RemoteServer[];
  selection: Selection;
  online: number;
  filter: string;
  checkingAll: boolean;
  localSessions: number;
  sessionsFor: (server: RemoteServer) => number;
  onSelect: (next: Selection) => void;
  onFilter: (value: string) => void;
  onCheckAll: () => void;
}

/** Everything you can connect to, in one list: the fleet, the servers, the
 *  local shell, and a one-off target that is not worth saving. */
export default function TargetRail({
  servers, selection, online, filter, checkingAll, localSessions,
  sessionsFor, onSelect, onFilter, onCheckAll,
}: Props) {
  const { t } = useT();
  const query = filter.trim().toLowerCase();
  const visible = query
    ? servers.filter(s => s.name.toLowerCase().includes(query) || s.host.toLowerCase().includes(query)
        || s.tags.some(tag => tag.toLowerCase().includes(query)) || s.memo.toLowerCase().includes(query))
    : servers;

  return (
    <aside className="rw-rail">
        <div className="rw-rail-head">
          <span className="rw-rail-count">
            <strong>{online}</strong>/{servers.length} {t('common.online')}
          </span>
          <button className="btn btn-secondary btn-sm" onClick={onCheckAll} disabled={checkingAll || servers.length === 0}>
            {checkingAll ? t('remote.checking') : t('remote.checkAll')}
          </button>
        </div>

        {servers.length > 5 && (
          <input className="filter-input rw-rail-filter" value={filter} onChange={e => onFilter(e.target.value)}
            placeholder={t('remote.filterPlaceholder')} />
        )}

        <div className="rw-rail-list">
          <button className={`rw-item ${selection.kind === 'overview' ? 'is-active' : ''}`}
            onClick={() => onSelect({ kind: 'overview' })}>
            <span className="rw-item-name">{t('remote.overview')}</span>
            <span className="rw-item-sub">{t('remote.overviewHint')}</span>
          </button>

          <div className="rw-rail-label">{t('remote.serversGroup')}</div>
          {visible.map(server => {
            const count = sessionsFor(server);
            const state = server.status === 'online' ? 'up' : server.status === 'offline' ? 'down' : 'unknown';
            return (
              <button key={server.id}
                className={`rw-item ${selection.kind === 'server' && selection.id === server.id ? 'is-active' : ''}`}
                onClick={() => onSelect({ kind: 'server', id: server.id })}>
                <span className={`rw-lamp rw-lamp-${state}`} />
                <span className="rw-item-name">{server.name}</span>
                {count > 0 && <span className="rw-item-count" title={t('remote.liveSessions')}>{count}</span>}
                <span className="rw-item-sub">{server.user}@{server.host}:{server.port}</span>
              </button>
            );
          })}
          {servers.length === 0 && <p className="rw-rail-empty">{t('remote.noServers')}</p>}
          <button className={`rw-item rw-item-add ${selection.kind === 'new' ? 'is-active' : ''}`}
            onClick={() => onSelect({ kind: 'new' })}>{t('remote.addServer')}</button>

          <div className="rw-rail-label">{t('remote.otherGroup')}</div>
          <button className={`rw-item ${selection.kind === 'local' ? 'is-active' : ''}`}
            onClick={() => onSelect({ kind: 'local' })}>
            <span className="rw-lamp rw-lamp-local" />
            <span className="rw-item-name">{t('remote.localShell')}</span>
            {localSessions > 0 && <span className="rw-item-count">{localSessions}</span>}
            <span className="rw-item-sub">{t('remote.localShellHint')}</span>
          </button>
          <button className={`rw-item ${selection.kind === 'adhoc' ? 'is-active' : ''}`}
            onClick={() => onSelect({ kind: 'adhoc' })}>
            <span className="rw-item-name">{t('remote.adhoc')}</span>
            <span className="rw-item-sub">{t('remote.adhocHint')}</span>
          </button>
        </div>
    </aside>
  );
}
