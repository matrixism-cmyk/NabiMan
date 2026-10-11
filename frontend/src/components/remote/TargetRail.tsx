import React, { useCallback, useState } from 'react';
import { RemoteServer } from '../../types';
import { useT } from '../../i18n';
import RailContextMenu, { RailMenuItem } from './RailContextMenu';
import { RailAction, Selection } from './workspaceState';

interface Props {
  servers: RemoteServer[];
  selection: Selection;
  online: number;
  filter: string;
  checkingAll: boolean;
  localSessions: number;
  sessionsFor: (server: RemoteServer) => number;
  onSelect: (next: Selection) => void;
  /** Something picked from an entry's right-click menu. */
  onAction: (target: Selection, action: RailAction) => void;
  onFilter: (value: string) => void;
  onCheckAll: () => void;
}

type MenuTarget = { kind: 'server'; server: RemoteServer } | { kind: 'local' };

interface OpenMenu { x: number; y: number; target: MenuTarget; opener: HTMLElement }

/** Everything you can connect to, in one list: the fleet, the servers, the
 *  local shell, and a one-off target that is not worth saving. */
export default function TargetRail({
  servers, selection, online, filter, checkingAll, localSessions,
  sessionsFor, onSelect, onAction, onFilter, onCheckAll,
}: Props) {
  const { t } = useT();
  const [menu, setMenu] = useState<OpenMenu | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);

  const openMenu = (e: React.MouseEvent<HTMLButtonElement>, target: MenuTarget) => {
    e.preventDefault();
    const opener = e.currentTarget;
    // The menu key and Shift+F10 report no pointer position: open by the entry.
    const box = opener.getBoundingClientRect();
    const fromKeyboard = e.clientX === 0 && e.clientY === 0;
    setMenu({
      x: fromKeyboard ? box.left + 24 : e.clientX,
      y: fromKeyboard ? box.bottom - 4 : e.clientY,
      target,
      opener,
    });
  };

  const itemsFor = (target: MenuTarget): RailMenuItem[] => {
    const selectionOf: Selection = target.kind === 'server'
      ? { kind: 'server', id: target.server.id } : { kind: 'local' };
    const run = (action: RailAction) => () => onAction(selectionOf, action);
    const count = target.kind === 'server' ? sessionsFor(target.server) : localSessions;
    const items: RailMenuItem[] = [
      { key: 'connect', icon: '▶', label: t('remote.menuConnect'), onSelect: run('connect') },
      { key: 'newSession', icon: '＋', label: t('remote.menuNewSession'), onSelect: run('newSession') },
      {
        key: 'sessions', icon: '☰',
        label: count > 0 ? `${t('remote.menuSessions')} (${count})` : t('remote.menuSessions'),
        onSelect: run('sessions'),
      },
      { key: 'window', icon: '⧉', label: t('remote.menuWindow'), onSelect: run('window') },
    ];
    if (target.kind === 'server') {
      items.push(
        { key: 'status', icon: '◔', label: t('remote.statusView'), separated: true, onSelect: run('status') },
        { key: 'exec', icon: '›', label: t('remote.menuExec'), onSelect: run('exec') },
        { key: 'check', icon: '↻', label: t('remote.menuCheck'), onSelect: run('check') },
        { key: 'edit', icon: '✎', label: t('remote.menuEdit'), separated: true, onSelect: run('edit') },
        { key: 'delete', icon: '✕', label: t('common.delete'), danger: true, onSelect: run('delete') },
      );
    }
    return items;
  };
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
                title={t('remote.railMenuHint')}
                aria-haspopup="menu"
                onClick={() => onSelect({ kind: 'server', id: server.id })}
                onContextMenu={(e) => openMenu(e, { kind: 'server', server })}>
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
            title={t('remote.railMenuHint')}
            aria-haspopup="menu"
            onClick={() => onSelect({ kind: 'local' })}
            onContextMenu={(e) => openMenu(e, { kind: 'local' })}>
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

        {menu && (
          <RailContextMenu
            x={menu.x}
            y={menu.y}
            title={menu.target.kind === 'server' ? menu.target.server.name : t('remote.localShell')}
            items={itemsFor(menu.target)}
            opener={menu.opener}
            onClose={closeMenu}
          />
        )}
    </aside>
  );
}
