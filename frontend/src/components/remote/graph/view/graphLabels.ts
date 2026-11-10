/**
 * Names as in Obsidian: hidden far off, fading in as the camera comes
 * closer, always shown for what is in focus, and never too many at once.
 * Plain numbers in and out, so it runs every frame without touching WebGL.
 */

/** At most this many names at once, the nearest first; the forced ones always count. */
export const LABEL_CAP = 36;

/** The distance past which a name is gone, for a threshold of 0–1. */
export const fadeDistance = (threshold: number) => 120 + Math.min(1, Math.max(0, threshold)) * 900;

/** How visible a name is at `dist` from the camera: 1 near, fading to 0 over the last 40 % before the fade distance. */
export function labelOpacity(dist: number, threshold: number): number {
  const far = fadeDistance(threshold);
  const near = far * 0.6;
  if (dist <= near) return 1;
  if (dist >= far) return 0;
  return 1 - (dist - near) / (far - near);
}

/**
 * Each name's target opacity. `forced` ones are 1 whatever the distance; of
 * the rest, only the nearest that fit under `cap` keep theirs. `out` and
 * `scratch` may be passed in to be reused, frame after frame.
 */
export function labelTargets(
  dist: ArrayLike<number>, forced: ArrayLike<boolean>, n: number, threshold: number, cap = LABEL_CAP,
  out: number[] = [], scratch: number[] = [],
): number[] {
  out.length = n;
  scratch.length = 0;
  let pinned = 0;
  for (let i = 0; i < n; i += 1) {
    if (forced[i]) { out[i] = 1; pinned += 1; continue; }
    const o = labelOpacity(dist[i], threshold);
    out[i] = o;
    if (o > 0) scratch.push(i);
  }
  const room = Math.max(0, cap - pinned);
  if (scratch.length > room) {
    scratch.sort((a, b) => dist[a] - dist[b]);
    for (let k = room; k < scratch.length; k += 1) out[scratch[k]] = 0;
  }
  return out;
}

/** One step of a smooth change towards `target`; `dt` in seconds. */
export const approach = (value: number, target: number, dt: number, rate = 9) => {
  const next = value + (target - value) * Math.min(1, dt * rate);
  return Math.abs(next - target) < 0.004 ? target : next;
};
