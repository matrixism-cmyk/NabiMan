import React, { useEffect, useState } from 'react';
import { useT } from '../../shared/i18n';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import ConsoleAccessModal from './ConsoleAccessModal';
import ConsoleShareModal from './ConsoleShareModal';
import { deleteConsole, openConsoleWindow, VmConsole } from './vncApi';
import { useConsoleWindows } from './useConsoleWindows';
import type { Selection } from './workspaceState';
import { consoleKeeper } from './consoleKeeper';
import { consoleSnapshots } from './consoleSnapshots';

type Named = { key: string; label: string };

/** What a VM console's right-click menu does, and the access/share dialogs it opens. */
export function useConsoleActions({ select, showWork, selection, refetch, consoles }: {
  select: (next: Selection) => void;
  showWork: () => void;
  selection: Selection;
  refetch: () => unknown;
  /** The consoles the account may open, once loaded. */
  consoles: VmConsole[] | null;
}) {
  const { t } = useT();
  const [access, setAccess] = useState<Named | null>(null);
  const [share, setShare] = useState<Named | null>(null);
  // A console the account may no longer open is not kept connected either.
  useEffect(() => { if (consoles) consoleKeeper.retain(consoles.map((c) => c.name)); }, [consoles]);

  const place = useConsoleWindows({ selection, select, consoles });

  const act = async (c: VmConsole, action: string) => {
    if (action === 'open') { select({ kind: 'console', vm: c.name, label: c.label, builtin: c.builtin }); showWork(); }
    else if (action === 'hangUp') { consoleKeeper.drop(c.name); consoleSnapshots.forget(c.name); }
    // Into a window on this page; or out to a browser window, which takes the connection over.
    else if (action === 'detach') place.detach(c);
    else if (action === 'popOut') place.popOut(c);
    // A new connection opens its own window, so a multi-user VM can hold several.
    else if (action === 'newSession') openConsoleWindow(c, `con_${c.name}_${Date.now()}`);
    else if (action === 'edit') {
      select({ kind: 'console-form', edit: { id: c.name, label: c.label, host: c.host || '', port: c.port || (c.protocol === 'rdp' ? 3389 : 5900),
        protocol: c.protocol || 'vnc', user: c.user || '', builtin: Boolean(c.builtin) } });
    } else if (action === 'access') setAccess({ key: c.name, label: c.label });
    else if (action === 'share') setShare({ key: c.name, label: c.label });
    else if (action === 'delete') {
      if (!(await confirmDialog({ message: t('remote.console.deleteConfirm', { label: c.label }), danger: true, confirmLabel: t('common.delete') }))) return;
      await deleteConsole(c.name);
      consoleKeeper.drop(c.name);
      consoleSnapshots.forget(c.name);
      await refetch();
      if (selection.kind === 'console' && selection.vm === c.name) select({ kind: 'overview' });
    }
  };

  return {
    act,
    place,
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
