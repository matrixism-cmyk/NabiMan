import React, { useCallback, useState } from 'react';
import { RemoteServer } from './remoteTypes';
import { useT } from '../../shared/i18n';
import RailContextMenu, { RailMenuItem } from './RailContextMenu';
import { VmConsole } from './vncApi';
import { useLongPress } from '../../shared/ui/useLongPress';
import ProtocolBadges from './ProtocolBadges';
import {
  addressOf, hasFiles, hasShell, hasTerminal, matchesProtocol, PROTOCOL_FILTERS, protocolName, protocolsOf, ProtocolFilter,
} from './protocols';
import { RailAction, Selection } from './workspaceState';
import { assignedTo, useGrants, useMember } from './member';

interface Props {
  /** A phone: no floating windows, and a long press opens the menu. */
  compact?: boolean;
  servers: RemoteServer[];
  selection: Selection;
  online: number;
  filter: string;
  checkingAll: boolean;
  localSessions: number;
  consoles?: VmConsole[];
  canManageConsoles?: boolean;
  canAddConsole?: boolean;
  onAddConsole?: () => void;
  onConsoleAction?: (console: VmConsole, action: string) => void;
  sessionsFor: (server: RemoteServer) => number;
  onSelect: (next: Selection) => void;
  /** Something picked from an entry's right-click menu. */
  onAction: (target: Selection, action: RailAction) => void;
  onFilter: (value: string) => void;
  onCheckAll: () => void;
}

type MenuTarget = { kind: 'server'; server: RemoteServer } | { kind: 'local' } | { kind: 'console'; console: VmConsole };

const FOLD_KEY = 'nabiman_rail_folded';

interface OpenMenu { x: number; y: number; target: MenuTarget; opener: HTMLElement }

/** Everything you can connect to, in one list: the fleet, the servers, the
 *  local shell, and a one-off target that is not worth saving. */
export default function TargetRail({
  compact, servers, selection, online, filter, checkingAll, localSessions, consoles = [], canManageConsoles, canAddConsole, onAddConsole, onConsoleAction,
  sessionsFor, onSelect, onAction, onFilter, onCheckAll,
}: Props) {
  const { t } = useT();
  const member = useMember();
  // A member adds servers when allowed, up to the number allowed.
  const grants = useGrants();
  const own = servers.filter(s => s.owner_id).length;
  const canAdd = !grants || grants.register_servers;
  const full = Boolean(grants?.max_servers && own >= grants.max_servers);
  const [menu, setMenu] = useState<OpenMenu | null>(null);
  const [protocol, setProtocol] = useState<ProtocolFilter>('');
  // Folded to a thin tab, for the most room to work; a phone shows the list on its own anyway.
  const [foldedPref, setFoldedPref] = useState(() => { try { return localStorage.getItem(FOLD_KEY) === '1'; } catch { return false; } });
  const folded = foldedPref && !compact;
  const fold = (next: boolean) => {
    setFoldedPref(next);
    try { localStorage.setItem(FOLD_KEY, next ? '1' : '0'); } catch { /* private mode */ }
  };
  const closeMenu = useCallback(() => setMenu(null), []);

  // A finger has no right button: holding it on an entry opens the same menu.
  const press = useLongPress<MenuTarget>((at, target) => setMenu({ x: at.x, y: at.y, target, opener: at.el }));
  const pick = (next: Selection) => { if (!press.wasLong()) onSelect(next); };

  const openMenu = (e: React.MouseEvent<HTMLButtonElement>, target: MenuTarget) => {
    e.preventDefault();
    if (press.justFired()) return;
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
    if (target.kind === 'console') {
      const c = target.console;
      const act = (a: string) => () => onConsoleAction?.(c, a);
      const items: RailMenuItem[] = [
        { key: 'open', icon: '▶', label: t('remote.menuConnect'), onSelect: act('open') },
        { key: 'detach', icon: '⧉', label: t('remote.console.detach'), onSelect: act('detach') },
      ];
      items.push({ key: 'share', icon: '↗', label: t('remote.console.share.btn'), separated: true, onSelect: act('share') });
      if (canManageConsoles) {
        items.push({ key: 'access', icon: '◍', label: t('remote.console.access.btn'), onSelect: act('access') });
        items.push({ key: 'edit', icon: '✎', label: t('remote.menuEdit'), onSelect: act('edit') });
        if (!c.builtin) {
          items.push({ key: 'delete', icon: '✕', label: t('common.delete'), danger: true, onSelect: act('delete') });
        }
      }
      return items;
    }
    const selectionOf: Selection = target.kind === 'server'
      ? { kind: 'server', id: target.server.id } : { kind: 'local' };
    const run = (action: RailAction) => () => onAction(selectionOf, action);
    const count = target.kind === 'server' ? sessionsFor(target.server) : localSessions;
    // A server offers only what it is set up for: no terminal, no sessions.
    const terminal = target.kind === 'local' || hasTerminal(target.server);
    const items: RailMenuItem[] = terminal ? [
      { key: 'connect', icon: '▶', label: t('remote.menuConnect'), onSelect: run('connect') },
      { key: 'newSession', icon: '＋', label: t('remote.menuNewSession'), onSelect: run('newSession') },
      {
        key: 'sessions', icon: '☰',
        label: count > 0 ? `${t('remote.menuSessions')} (${count})` : t('remote.menuSessions'),
        onSelect: run('sessions'),
      },
      // A floating window has no room on a phone.
      ...(compact ? [] : [{ key: 'window', icon: '⧉', label: t('remote.menuWindow'), onSelect: run('window') }]),
    ] : [];
    if (target.kind === 'server') {
      if (!terminal && hasFiles(target.server)) {
        items.push({ key: 'files', icon: '📁', label: t('remote.menuFiles'), onSelect: run('files') });
      }
      items.push(
        { key: 'status', icon: '◔', label: t('remote.statusView'), separated: items.length > 0, onSelect: run('status') },
        ...(hasShell(target.server) && !member ? [{ key: 'exec', icon: '›', label: t('remote.menuExec'), onSelect: run('exec') }] : []),
        { key: 'check', icon: '↻', label: t('remote.menuCheck'), onSelect: run('check') },
        ...(assignedTo(member, target.server) ? [] : [
          { key: 'edit', icon: '✎', label: t('remote.menuEdit'), separated: true, onSelect: run('edit') },
          { key: 'delete', icon: '✕', label: t('common.delete'), danger: true, onSelect: run('delete') },
        ]),
      );
    }
    return items;
  };
  const query = filter.trim().toLowerCase();
  const visible = servers.filter(s => matchesProtocol(s, protocol) && (!query
    || s.name.toLowerCase().includes(query) || s.host.toLowerCase().includes(query)
    || s.tags.some(tag => tag.toLowerCase().includes(query)) || s.memo.toLowerCase().includes(query)));
  // The protocol filter appears once the list holds more than one kind.
  const kinds = new Set(servers.flatMap(protocolsOf).map(p => (p === 'ftps' ? 'ftp' : p)));

  if (folded) {
    return (
      <aside className="rw-rail is-folded">
        <button type="button" className="rw-split-tab rw-rail-tab" onClick={() => fold(false)}
          title={t('remote.railShow')} aria-label={t('remote.railShow')} aria-expanded={false}>
          <span className="rw-split-tab-arrow" aria-hidden="true">»</span>{t('remote.railTab')}
        </button>
      </aside>
    );
  }

  return (
    <aside className="rw-rail">
        <div className="rw-rail-head">
          <span className="rw-rail-count">
            <strong>{online}</strong>/{servers.length} {t('common.online')}
          </span>
          {!member && (
            <button className="btn btn-secondary btn-sm" onClick={onCheckAll} disabled={checkingAll || servers.length === 0}>
              {checkingAll ? t('remote.checking') : t('remote.checkAll')}
            </button>
          )}
          {/* The remote workspace alone, at /remote (this tab keeps its sign-in). */}
          {window.location.pathname !== '/remote' && (
            <a className="btn btn-secondary btn-sm rw-standalone" href="/remote" title={t('remote.openStandalone')}
              aria-label={t('remote.openStandalone')}>↗</a>
          )}
          {!compact && (
            <button type="button" className="btn btn-secondary btn-sm rw-rail-fold" onClick={() => fold(true)}
              title={t('remote.railHide')} aria-label={t('remote.railHide')} aria-expanded>«</button>
          )}
        </div>

        {(servers.length > 5 || kinds.size > 1) && (
          <div className="rw-rail-filters">
            {servers.length > 5 && (
              <input className="filter-input" value={filter} onChange={e => onFilter(e.target.value)}
                placeholder={t('remote.filterPlaceholder')} />
            )}
            {kinds.size > 1 && (
              <select className="select-input" value={protocol} aria-label={t('remote.proto.filter')}
                onChange={e => setProtocol(e.target.value as ProtocolFilter)}>
                <option value="">{t('remote.proto.filter')}</option>
                {PROTOCOL_FILTERS.filter(p => kinds.has(p)).map(p => (
                  <option key={p} value={p}>{p === 'ftp' ? 'FTP/FTPS' : protocolName(p)}</option>
                ))}
              </select>
            )}
          </div>
        )}

        <div className="rw-rail-list">
          <button className={`rw-item ${selection.kind === 'overview' ? 'is-active' : ''}`}
            onClick={() => onSelect({ kind: 'overview' })}>
            <span className="rw-item-name">{t('remote.overview')}</span>
            <span className="rw-item-sub">{t(member ? 'remote.overviewHintMember' : 'remote.overviewHint')}</span>
          </button>

          <div className="rw-rail-label">{t('remote.serversGroup')}</div>
          {visible.map(server => {
            const count = sessionsFor(server);
            const state = server.status === 'online' ? 'up' : server.status === 'offline' ? 'down'
              : server.status === 'partial' ? 'partial' : 'unknown';
            return (
              <button key={server.id}
                className={`rw-item ${selection.kind === 'server' && selection.id === server.id ? 'is-active' : ''}`}
                title={t(compact ? 'remote.railLongPressHint' : 'remote.railMenuHint')}
                aria-haspopup="menu"
                {...press.bind({ kind: 'server', server })}
                onClick={() => pick({ kind: 'server', id: server.id })}
                onContextMenu={(e) => openMenu(e, { kind: 'server', server })}>
                <span className={`rw-lamp rw-lamp-${state}`} />
                <span className="rw-item-name">{server.name}</span>
                {count > 0 && <span className="rw-item-count" title={t('remote.liveSessions')}>{count}</span>}
                <span className="rw-item-sub"><ProtocolBadges server={server} />{addressOf(server)}
                  {assignedTo(member, server) && <span className="tag-badge rw-assigned">{t('remote.assigned')}</span>}</span>
              </button>
            );
          })}
          {servers.length === 0 && <p className="rw-rail-empty">{t('remote.noServers')}</p>}
          {canAdd && (
            <button className={`rw-item rw-item-add ${selection.kind === 'new' ? 'is-active' : ''}`} disabled={full}
              title={full ? t('remote.serverLimit', { count: grants?.max_servers || 0 }) : undefined}
              onClick={() => onSelect({ kind: 'new' })}>
              {t('remote.addServer')}{grants?.max_servers ? ` (${own}/${grants.max_servers})` : ''}
            </button>
          )}

          {(consoles.length > 0 || canAddConsole) && <>
            <div className="rw-rail-label">{t('remote.consoleGroup')}</div>
            {consoles.map(vm => (
              <button key={vm.name}
                className={`rw-item ${selection.kind === 'console' && selection.vm === vm.name ? 'is-active' : ''}`}
                aria-haspopup="menu"
                onClick={() => onSelect({ kind: 'console', vm: vm.name, label: vm.label, builtin: vm.builtin })}
                onContextMenu={(e) => openMenu(e, { kind: 'console', console: vm })}>
                <span className="rw-lamp rw-lamp-local" />
                <span className="rw-item-name">{vm.label}</span>
                <span className="rw-item-sub">{vm.protocol === 'rdp' ? t('remote.consoleHintRdp') : (vm.builtin ? t('remote.consoleHint') : t('remote.consoleHintExternal'))}</span>
              </button>
            ))}
            {canAddConsole && (
              <button className={`rw-item rw-item-add ${selection.kind === 'console-form' ? 'is-active' : ''}`}
                onClick={() => onAddConsole?.()}>
                {t('remote.console.add')}
              </button>
            )}
          </>}
          <div className="rw-rail-label">{t('remote.otherGroup')}</div>
          {!member && (
          <button className={`rw-item ${selection.kind === 'local' ? 'is-active' : ''}`}
            title={t(compact ? 'remote.railLongPressHint' : 'remote.railMenuHint')}
            aria-haspopup="menu"
            {...press.bind({ kind: 'local' })}
            onClick={() => pick({ kind: 'local' })}
            onContextMenu={(e) => openMenu(e, { kind: 'local' })}>
            <span className="rw-lamp rw-lamp-local" />
            <span className="rw-item-name">{t('remote.localShell')}</span>
            {localSessions > 0 && <span className="rw-item-count">{localSessions}</span>}
            <span className="rw-item-sub">{t('remote.localShellHint')}</span>
          </button>
          )}
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
            title={menu.target.kind === 'server' ? menu.target.server.name : menu.target.kind === 'console' ? menu.target.console.label : t('remote.localShell')}
            items={itemsFor(menu.target)}
            opener={menu.opener}
            onClose={closeMenu}
          />
        )}
    </aside>
  );
}
