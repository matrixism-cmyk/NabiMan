import React, { useState } from 'react';
import { useT } from '../../shared/i18n';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import VncConsoleView from './VncConsoleView';
import RdpConsoleView from './RdpConsoleView';
import VncConsoleForm from './VncConsoleForm';
import { deleteConsole, VmConsole } from './vncApi';
import ConsoleAccessModal from './ConsoleAccessModal';
import { Selection } from './workspaceState';

/** The VM-console work area: a live console (with edit/delete for admins on a
 *  registered one) or the add/edit form. Keeps that logic out of the workspace. */
export default function ConsoleArea({ selection, consoles, canManage, onSelect, onChanged }: {
  selection: Selection;
  consoles: VmConsole[];
  canManage: boolean;
  onSelect: (s: Selection) => void;
  onChanged: () => void | Promise<void>;
}) {
  const { t } = useT();
  const [access, setAccess] = useState<{ key: string; label: string } | null>(null);

  if (selection.kind === 'console') {
    const vm = selection.vm; const label = selection.label;
    const rec = consoles.find((x) => x.name === vm);
    const edit = canManage && rec && !rec.builtin
      ? () => onSelect({ kind: 'console-form', edit: { id: rec.name, label: rec.label, host: rec.host || '', port: rec.port || (rec.protocol === 'rdp' ? 3389 : 5900), protocol: rec.protocol || 'vnc', user: rec.user || '' } })
      : undefined;
    const del = async () => {
      if (!(await confirmDialog({ message: t('remote.console.deleteConfirm', { label }), danger: true, confirmLabel: t('common.delete') }))) return;
      await deleteConsole(vm); await onChanged(); onSelect({ kind: 'overview' });
    };
    const common = { label, canEdit: canManage && !selection.builtin, onEdit: edit, onDelete: del, canGrant: canManage, onGrant: () => setAccess({ key: vm, label }) };
    return (
      <>
        {rec?.protocol === 'rdp'
          ? <RdpConsoleView target={vm} {...common} />
          : <VncConsoleView vm={vm} {...common} />}
        {access && <ConsoleAccessModal console={access.key} label={access.label} onClose={() => setAccess(null)} />}
      </>
    );
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
