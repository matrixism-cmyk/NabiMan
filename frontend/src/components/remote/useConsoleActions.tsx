import React, { useState } from 'react';
import { useT } from '../../shared/i18n';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import ConsoleAccessModal from './ConsoleAccessModal';
import ConsoleShareModal from './ConsoleShareModal';
import { deleteConsole, VmConsole } from './vncApi';
import type { Selection } from './workspaceState';

type Named = { key: string; label: string };
const POPUP = 'width=1300,height=900,menubar=no,toolbar=no,location=no';

/** What a VM console's right-click menu does, and the access/share dialogs it opens. */
export function useConsoleActions({ select, showWork, selection, refetch }: {
  select: (next: Selection) => void;
  showWork: () => void;
  selection: Selection;
  refetch: () => unknown;
}) {
  const { t } = useT();
  const [access, setAccess] = useState<Named | null>(null);
  const [share, setShare] = useState<Named | null>(null);
  const page = (c: VmConsole) => `/remote#${c.protocol === 'rdp' ? 'rdp' : 'console'}/${encodeURIComponent(c.name)}`;

  const act = async (c: VmConsole, action: string) => {
    if (action === 'open') { select({ kind: 'console', vm: c.name, label: c.label, builtin: c.builtin }); showWork(); }
    else if (action === 'detach') window.open(page(c), `con_${c.name}`, POPUP);
    // A new connection opens its own window, so a multi-user VM can hold several.
    else if (action === 'newSession') window.open(page(c), `con_${c.name}_${Date.now()}`, POPUP);
    else if (action === 'edit') {
      select({ kind: 'console-form', edit: { id: c.name, label: c.label, host: c.host || '', port: c.port || (c.protocol === 'rdp' ? 3389 : 5900),
        protocol: c.protocol || 'vnc', user: c.user || '', builtin: Boolean(c.builtin) } });
    } else if (action === 'access') setAccess({ key: c.name, label: c.label });
    else if (action === 'share') setShare({ key: c.name, label: c.label });
    else if (action === 'delete') {
      if (!(await confirmDialog({ message: t('remote.console.deleteConfirm', { label: c.label }), danger: true, confirmLabel: t('common.delete') }))) return;
      await deleteConsole(c.name);
      await refetch();
      if (selection.kind === 'console' && selection.vm === c.name) select({ kind: 'overview' });
    }
  };

  return {
    act,
    openAccess: (key: string, label: string) => setAccess({ key, label }),
    openShare: (key: string, label: string) => setShare({ key, label }),
    modals: (
      <>
        {access && <ConsoleAccessModal console={access.key} label={access.label} onClose={() => setAccess(null)} />}
        {share && <ConsoleShareModal console={share.key} label={share.label} onClose={() => setShare(null)} />}
      </>
    ),
  };
}
