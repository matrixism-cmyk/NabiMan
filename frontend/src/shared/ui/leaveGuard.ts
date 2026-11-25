/**
 * A page with work that would be lost asks before the app shows another
 * page. One guard at a time (the page on screen); the app's navigation goes
 * through `guardedLeave`.
 */
type Guard = () => Promise<boolean>;
let guard: Guard | null = null;

/** Ask `ask` before leaving; returns how to stop asking. */
export function setLeaveGuard(ask: Guard): () => void {
  guard = ask;
  return () => { if (guard === ask) guard = null; };
}

/** Run `go` now, or once the page on screen agrees to be left. */
export function guardedLeave(go: () => void): void {
  if (!guard) { go(); return; }
  void guard().then((ok) => { if (ok) go(); }, () => go());
}
