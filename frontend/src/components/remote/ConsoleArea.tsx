import React from 'react';
import { useT } from '../../shared/i18n';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import VncConsoleView from './VncConsoleView';
import RdpConsoleView from './RdpConsoleView';
import VncConsoleForm from './VncConsoleForm';
import { deleteConsole, VmConsole } from './vncApi';
import { Selection } from './workspaceState';

/** The VM-console work area: a live console (with edit/delete for admins on a
 *  registered one) or the add/edit form. Keeps that logic out of the workspace. */
export default function ConsoleArea({ selection, consoles, canManage, onSelect, onChanged, onAccess, onShare }: {
  selection: Selection;
  consoles: VmConsole[];
  canManage: boolean;
  onSelect: (s: Selection) => void;
  onChanged: () => void | Promise<void>;
  onAccess: (key: string, label: string) => void;
  onShare: (key: string, label: string) => void;
}) {
  const { t } = useT();

  if (selection.kind === 'console') {
    const vm = selection.vm; const label = selection.label;
    const rec = consoles.find((x) => x.name === vm);
    const canMng = Boolean(rec?.can_manage);
    const edit = canMng && rec
      ? () => onSelect({ kind: 'console-form', edit: { id: rec.name, label: rec.label, host: rec.host || '', port: rec.port || (rec.protocol === 'rdp' ? 3389 : 5900), protocol: rec.protocol || 'vnc', user: rec.user || '', builtin: Boolean(rec.builtin) } })
      : undefined;
    const del = async () => {
      if (!(await confirmDialog({ message: t('remote.console.deleteConfirm', { label }), danger: true, confirmLabel: t('common.delete') }))) return;
      await deleteConsole(vm); await onChanged(); onSelect({ kind: 'overview' });
    };
    const common = { label, canEdit: canMng, onEdit: edit, onDelete: del, canGrant: canManage, onGrant: () => onAccess(vm, label), canShare: true, onShare: () => onShare(vm, label) };
    return rec?.protocol === 'rdp'
      ? <RdpConsoleView target={vm} {...common} />
      : <VncConsoleView vm={vm} {...common} />;
  }
  if (selection.kind === 'console-form') {
    const edit = selection.edit;
    return (
      <VncConsoleForm edit={edit}
        onSaved={async (c) => { await onChanged(); onSelect({ kind: 'console', vm: c.name, label: c.label, builtin: false }); }}
        onCancel={() => onSelect(edit ? { kind: 'console', vm: edit.id, label: edit.label, builtin: false } : { kind: 'overview' })} />
    );
  }
  return null;
}
