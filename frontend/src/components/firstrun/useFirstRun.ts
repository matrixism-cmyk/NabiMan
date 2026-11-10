import { useCallback, useEffect, useState } from 'react';
import { FirstRunState, Step, looksFresh, readFirstRun, writeFirstRun } from './firstRunState';
import { rememberSelection } from '../remote/workspaceState';
import type { Category, Tab } from '../../nav';

/**
 * The first-run checklist for an administrator: shown by itself once on an
 * install that looks new, until it is dismissed; and whenever it is asked for
 * from the header menu ("처음 시작하기").
 */
export function useFirstRun(user: string | null, admin: boolean) {
  const [state, setState] = useState<FirstRunState>({ dismissed: true, done: [] });
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (!user || !admin) { setOpen(false); return undefined; }
    const saved = readFirstRun(user);
    setState(saved);
    if (saved.dismissed) return undefined;
    let live = true;
    looksFresh().then((fresh) => { if (live && fresh) setOpen(true); });
    return () => { live = false; };
  }, [user, admin]);

  const save = useCallback((next: FirstRunState) => {
    setState(next);
    if (user) writeFirstRun(user, next);
  }, [user]);
  const markDone = useCallback((step: Step) => {
    if (!state.done.includes(step)) save({ ...state, done: [...state.done, step] });
  }, [state, save]);
  const dismiss = useCallback(() => { setOpen(false); save({ ...state, dismissed: true }); }, [state, save]);
  const show = useCallback(() => setOpen(true), []);
  return { open: open && admin, done: state.done, markDone, dismiss, show };
}

/** Where each step leads: a dialog of the shell, or a screen (category, tab). */
export function takeStep(step: Step, go: { changePassword: () => void; navigate: (cat: Category, tab: Tab) => void }) {
  switch (step) {
    case 'password': go.changePassword(); break;
    // The remote workspace opens on what was last selected: the form for a new server, or this server's shell.
    case 'server': rememberSelection({ kind: 'new' }); go.navigate('remote', 'remote'); break;
    case 'terminal': rememberSelection({ kind: 'local' }); go.navigate('remote', 'remote'); break;
    case 'backup': go.navigate('management', 'backup'); break;
    case 'users': go.navigate('administration', 'usermgmt'); break;
    case 'guide': break; // read in place, in the checklist's own dialog
  }
}

/** Steps whose screen this account does not have. */
export function unavailableSteps(hidden: Set<Tab>): Step[] {
  const needs: [Step, Tab][] = [['server', 'remote'], ['terminal', 'remote'], ['backup', 'backup'], ['users', 'usermgmt'], ['guide', 'remote']];
  return needs.filter(([, tab]) => hidden.has(tab)).map(([step]) => step);
}
