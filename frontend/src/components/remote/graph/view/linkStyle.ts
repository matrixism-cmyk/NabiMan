/**
 * How each link is drawn in either style: neutral grey and thin in the
 * simple one, coloured only while live or in focus; the rich one as before.
 */
import * as THREE from 'three';
import type { GraphSettings } from './graphSettings';
import type { Palette } from './graphTheme';
import type { LinkLook } from './sceneFx';

export type LinkKind = 'order' | 'host' | 'conn' | 'net' | 'ghost' | 'report';
export interface LinkData { id: string; live: boolean; strength: number; color: string; kind: LinkKind }

export const REPORT_COLOR = '#7dd3fc';

/** A colour dimmed towards the background, as an arrow fades once its work is done. */
export function dim(hex: string, strength: number) {
  const c = new THREE.Color(hex);
  return `#${c.lerp(new THREE.Color('#0b1220'), 1 - Math.max(0.25, strength)).getHexString()}`;
}

/** The hand-offs; the threads (host, connection, network) only hold the layout together. */
export const isOrder = (l: LinkData) => l.kind === 'order';
export const linkShown = (l: LinkData) => l.kind !== 'ghost' && l.kind !== 'report';

export function linkLook(l: LinkData, s: GraphSettings, p: Palette): LinkLook {
  if (s.style === 'rich') {
    const base = isOrder(l) ? dim(l.color, l.strength) : dim(l.color, l.kind === 'net' ? 0.6 : l.kind === 'conn' ? 0.45 : 0.3);
    return { base, hi: l.color, rest: p.linkOpacity, restHue: 0 };
  }
  if (isOrder(l)) {
    return { base: p.link, hi: l.color, rest: l.live ? 0.85 : p.linkOpacity * (0.6 + 0.4 * l.strength), restHue: l.live ? 1 : 0 };
  }
  return { base: p.link, hi: l.color, rest: p.threadOpacity, restHue: 0 };
}

export function linkWidth(l: LinkData, s: GraphSettings): number {
  const w = s.style === 'rich'
    ? (isOrder(l) ? (l.live ? 2 : 1) : l.kind === 'net' ? 0.6 : l.kind === 'conn' ? 0.5 : 0.25)
    : (isOrder(l) ? (l.live ? 0.9 : 0.5) : 0.3);
  return Math.max(0.1, w * s.linkThickness);
}

export const arrowLength = (l: LinkData, s: GraphSettings) => (!s.arrows || !isOrder(l) ? 0 : s.style === 'rich' ? 5.5 : 3.2 * Math.sqrt(s.linkThickness));
export const particleCount = (l: LinkData, s: GraphSettings) => (isOrder(l) && l.live ? (s.style === 'rich' ? 5 : 2) : 0);
export const particleWidth = (l: LinkData, s: GraphSettings) => (s.style === 'rich' ? (l.kind === 'report' ? 4.5 : 3.4) : l.kind === 'report' ? 2.2 : 1.7);
export const particleColor = (l: LinkData) => (l.kind === 'report' ? REPORT_COLOR : l.color);
