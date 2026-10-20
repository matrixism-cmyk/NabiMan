import { useCallback, useEffect, useState } from 'react';
import './remote-mobile.css';

/** Where the remote workspace switches to its phone layout. */
export const COMPACT_QUERY = '(max-width: 760px)';

const matches = () => typeof window !== 'undefined' && typeof window.matchMedia === 'function'
  && window.matchMedia(COMPACT_QUERY).matches;

/** True on a phone-sized screen, following rotation and resizing. */
export function useCompact(): boolean {
  const [compact, setCompact] = useState(matches);
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return undefined;
    const query = window.matchMedia(COMPACT_QUERY);
    const change = () => setCompact(query.matches);
    query.addEventListener('change', change);
    return () => query.removeEventListener('change', change);
  }, []);
  return compact;
}

export type Pane = 'list' | 'work' | 'both';

/**
 * Which half a phone shows: the list of targets, or the work on one. Opening
 * one adds a history entry, so the phone's own Back returns to the list.
 */
export function useMobilePane(compact: boolean, startOnWork: boolean) {
  const [pane, setPane] = useState<'list' | 'work'>(startOnWork ? 'work' : 'list');
  const showWork = useCallback(() => {
    setPane('work');
    if (compact && window.history.state?.rwPane !== 'work') {
      window.history.pushState({ ...(window.history.state || {}), rwPane: 'work' }, '');
    }
  }, [compact]);
  const showList = useCallback(() => {
    if (window.history.state?.rwPane === 'work') window.history.back();
    else setPane('list');
  }, []);
  useEffect(() => {
    const pop = () => setPane(window.history.state?.rwPane === 'work' ? 'work' : 'list');
    window.addEventListener('popstate', pop);
    return () => window.removeEventListener('popstate', pop);
  }, []);
  return { pane: (compact ? pane : 'both') as Pane, showWork, showList };
}

/**
 * While a phone shows the work full screen, it is sized to the part of the
 * screen the keyboard leaves visible (`--vv-height`, `--vv-top`), so the
 * prompt and the key bar stay above the keyboard; and the floating feedback
 * button steps aside so it does not cover the keys.
 */
export function useFullScreenPane(active: boolean): void {
  useEffect(() => {
    if (!active) return undefined;
    const root = document.documentElement;
    const viewport = window.visualViewport;
    const update = () => {
      root.style.setProperty('--vv-height', `${Math.round(viewport ? viewport.height : window.innerHeight)}px`);
      root.style.setProperty('--vv-top', `${Math.round(viewport ? viewport.offsetTop : 0)}px`);
    };
    update();
    document.body.classList.add('rw-fullscreen');
    viewport?.addEventListener('resize', update);
    viewport?.addEventListener('scroll', update);
    window.addEventListener('resize', update);
    return () => {
      viewport?.removeEventListener('resize', update);
      viewport?.removeEventListener('scroll', update);
      window.removeEventListener('resize', update);
      document.body.classList.remove('rw-fullscreen');
      root.style.removeProperty('--vv-height');
      root.style.removeProperty('--vv-top');
    };
  }, [active]);
}
