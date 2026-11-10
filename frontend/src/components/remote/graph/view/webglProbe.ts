/**
 * Whether this browser can draw WebGL, asked once per page. Each probe makes
 * a context, and the browser keeps only a few alive (16 in Chrome): asked on
 * every render, the probes pushed the map's own context out — the canvas went
 * blank until the browser gave it back. So the answer is kept, and the probe's
 * context is handed back at once.
 */
let known: boolean | null = null;

interface ProbeDoc { createElement(tag: 'canvas'): { getContext(kind: string): unknown } }
interface LoseContext { loseContext(): void }
interface ProbeContext { getExtension(name: string): unknown }

export function hasWebGL(doc: ProbeDoc = document): boolean {
  if (known !== null) return known;
  try {
    const canvas = doc.createElement('canvas');
    const gl = (canvas.getContext('webgl2') || canvas.getContext('webgl')) as ProbeContext | null;
    known = !!gl;
    (gl?.getExtension('WEBGL_lose_context') as LoseContext | null)?.loseContext();
  } catch {
    known = false;
  }
  return known;
}

/** For tests: forget the answer. */
export function resetWebGLProbe() { known = null; }
