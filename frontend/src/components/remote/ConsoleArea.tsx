import React from 'react';
import { useT } from '../../shared/i18n';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import { ConsoleSuspense, RdpConsoleView, VncConsoleView } from './lazyConsoles';
import VncConsoleForm from './VncConsoleForm';
import { deleteConsole, VmConsole } from './vncApi';
import { Selection } from './workspaceState';
import type { ConsolePlace } from './useConsoleWindows';
import { consoleKeeper } from './consoleKeeper';
import { consoleSnapshots } from './consoleSnapshots';
import { ErrorBoundary } from '../../shared/ui/ErrorBoundary';

/** The VM-console work area: a live console (with edit/delete for admins on a
 *  registered one) or the add/edit form. Keeps that logic out of the workspace. */
export default function ConsoleArea({ selection, consoles, canManage, onSelect, onChanged, onAccess, onShare, place }: {
  selection: Selection;
  consoles: VmConsole[];
  canManage: boolean;
  onSelect: (s: Selection) => void;
  onChanged: () => void | Promise<void>;
  onAccess: (key: string, label: string) => void;
  onShare: (key: string, label: string) => void;
  /** Where the selected console shows: here, or in a window of its own. */
  place: ConsolePlace;
}) {
  const { t } = useT();

  if (selection.kind === 'console') {
    const vm = selection.vm; const label = selection.label;
    const rec = consoles.find((x) => x.name === vm);
    // Open as a window: this pane does not take it away, it says where it is.
    if (place.windowed) {
      return (
        <div className="rw-view rw-console-missing rw-console-elsewhere" role="status">
          <p>{t('remote.console.windowed')}</p>
          <div className="btn-group">
            <button type="button" className="btn btn-primary btn-sm" onClick={place.goToWindow}>{t('remote.console.goToWindow')}</button>
            <button type="button" className="btn btn-secondary btn-sm" onClick={place.bringHere}>{t('remote.console.bringHere')}</button>
          </div>
        </div>
      );
    }
    const it = { name: vm, label, builtin: selection.builtin, protocol: rec?.protocol };
    const canMng = Boolean(rec?.can_manage);
    const edit = canMng && rec
      ? () => onSelect({ kind: 'console-form', edit: { id: rec.name, label: rec.label, host: rec.host || '', port: rec.port || (rec.protocol === 'rdp' ? 3389 : 5900), protocol: rec.protocol || 'vnc', user: rec.user || '', builtin: Boolean(rec.builtin) } })
      : undefined;
    const del = async () => {
      if (!(await confirmDialog({ message: t('remote.console.deleteConfirm', { label }), danger: true, confirmLabel: t('common.delete') }))) return;
      await deleteConsole(vm); consoleKeeper.drop(vm); consoleSnapshots.forget(vm); await onChanged(); onSelect({ kind: 'overview' });
    };
    const common = { label, canEdit: canMng, onEdit: edit, onDelete: del, canGrant: canManage, onGrant: () => onAccess(vm, label), canShare: true, onShare: () => onShare(vm, label),
      onDetach: () => place.detach(it), onPopOut: () => place.popOut(it) };
    return <ErrorBoundary region="console" resetKey={vm}><ConsoleSuspense>{rec?.protocol === 'rdp'
      ? <RdpConsoleView key={vm} target={vm} {...common} />
      : <VncConsoleView key={vm} vm={vm} {...common} />}</ConsoleSuspense></ErrorBoundary>;
  }
  if (selection.kind === 'console-form') {
    const edit = selection.edit;
    return (
      <VncConsoleForm edit={edit}
        onSaved={async (c) => { consoleKeeper.drop(c.name); consoleSnapshots.forget(c.name); await onChanged(); onSelect({ kind: 'console', vm: c.name, label: c.label, builtin: false }); }}
        onCancel={() => onSelect(edit ? { kind: 'console', vm: edit.id, label: edit.label, builtin: false } : { kind: 'overview' })} />
    );
  }
  return null;
}
