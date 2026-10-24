import { RefObject, useCallback, useEffect, useRef, useState } from 'react';

/**
 * Toggle the browser Fullscreen API on one element — a console window, so it
 * works the same in the embedded workspace, a detached popout, and a shared
 * link. `onFit` runs (on the next frame) whenever fullscreen enters or leaves
 * or the window resizes, letting a manually scaled display (RDP) refit; noVNC
 * rescales itself, so it needs none.
 */
export function useFullscreen(
  ref: RefObject<HTMLElement | null>,
  onFit?: () => void,
): { full: boolean; toggle: () => void } {
  const [full, setFull] = useState(false);
  const fitRef = useRef(onFit);
  fitRef.current = onFit;

  const toggle = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    if (document.fullscreenElement) document.exitFullscreen().catch(() => { /* ignore */ });
    else el.requestFullscreen?.().catch(() => { /* denied or unsupported */ });
  }, [ref]);

  useEffect(() => {
    const refit = () => requestAnimationFrame(() => fitRef.current?.());
    const onFs = () => { setFull(!!document.fullscreenElement); refit(); };
    document.addEventListener('fullscreenchange', onFs);
    window.addEventListener('resize', refit);
    return () => {
      document.removeEventListener('fullscreenchange', onFs);
      window.removeEventListener('resize', refit);
    };
  }, []);

  return { full, toggle };
}
