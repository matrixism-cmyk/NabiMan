/**
 * The per-frame effects of the 3D map: focus (dimming all but a node, its
 * neighbours and their links, eased), names fading with distance, sessions
 * at work breathing, the lead's ring turning, a jump where a message lands.
 * The heavy part (distances, label cap, opacities) runs only when the camera
 * moved, the layout moved or the focus changed, and stops once all settled.
 */
import * as THREE from 'three';
import { approach, labelTargets, LABEL_CAP } from './graphLabels';
import type { Focus } from './graphFocus';
import type { NodeParts, Three } from './nodeParts';

export interface LinkLook {
  /** Colour at rest, and when in focus (or live). */
  base: string;
  hi: string;
  /** Opacity at rest. */
  rest: number;
  /** 0 (base colour) – 1 (its own colour) at rest: live links show their colour. */
  restHue: number;
}

interface LinkFx { mat: Three; base: Three; hi: Three; rest: number; restHue: number; a: number; h: number; data: Three }

const PING_MS = 700;

export class SceneFx {
  nodes = new Map<string, { obj: Three; parts: NodeParts }>();
  links = new Map<string, LinkFx>();
  private byData = new WeakMap<object, LinkFx>();
  focus: Focus | null = null;
  forced = new Set<string>();
  threshold = 0.5;
  /** Names fade with distance (simple style); the rich style shows them all. */
  fadeLabels = true;
  reduced = false;
  dimmed = 0.14;
  faded = 0.04;
  private dirty = true;
  private settling = true;
  private lastMs = 0;
  private cam = { x: NaN, y: NaN, z: NaN };
  private ids: string[] = [];
  private dist: number[] = [];
  private pinned: boolean[] = [];
  private out: number[] = [];
  private scratch: number[] = [];

  private repaint = false;

  /** Something changed that the names or opacities depend on; `repaint` when the library may have reset the arrows. */
  touch(repaint = false) { this.dirty = true; if (repaint) this.repaint = true; }

  private nodeTarget(id: string) { return this.focus ? (this.focus.nodes.has(id) ? 1 : this.dimmed) : 1; }

  addNode(id: string, obj: Three) {
    const parts: NodeParts | undefined = obj.userData?.parts;
    if (!parts) return;
    // Born at its target, so a refresh while something is in focus does not flash.
    parts.alpha = this.nodeTarget(id);
    for (const { m, base } of parts.mats) m.opacity = base * parts.alpha;
    this.nodes.set(id, { obj, parts });
    this.ids = Array.from(this.nodes.keys());
    this.dirty = true;
  }

  /** Forgets nodes and links no longer drawn. */
  keepOnly(nodeIds: Set<string>, linkIds: Set<string>) {
    this.nodes.forEach((_, id) => { if (!nodeIds.has(id)) this.nodes.delete(id); });
    this.links.forEach((_, id) => { if (!linkIds.has(id)) this.links.delete(id); });
    this.ids = Array.from(this.nodes.keys());
    this.dirty = true;
  }

  /** The material for one link, made once per link object; the effects then own its colour and opacity. */
  linkMaterial(data: { id: string }, look: LinkLook) {
    let fx = this.byData.get(data);
    if (!fx) {
      const mat = new THREE.MeshBasicMaterial({ color: look.base, transparent: true, opacity: look.rest, depthWrite: false });
      fx = { mat, base: new THREE.Color(look.base), hi: new THREE.Color(look.hi), rest: look.rest, restHue: look.restHue, a: look.rest, h: look.restHue, data };
      const inFocus = this.focus?.links.has(data.id);
      if (this.focus) { fx.a = inFocus ? 0.9 : this.faded; fx.h = inFocus ? 1 : fx.restHue; }
      this.paintLink(fx);
      this.byData.set(data, fx);
    }
    this.links.set(data.id, fx);
    this.settling = true;
    return fx.mat;
  }

  /** The style changed: links get new materials at the next draw. */
  resetLinks() { this.byData = new WeakMap(); }

  setFocus(focus: Focus | null, forced: Set<string>) {
    this.focus = focus;
    this.forced = forced;
    this.dirty = true;
  }

  ping(id: string, ms: number) {
    const n = this.nodes.get(id);
    if (n) n.parts.ping = ms;
  }

  private paintLink(fx: LinkFx) {
    fx.mat.opacity = fx.a;
    fx.mat.color.copy(fx.base).lerp(fx.hi, fx.h);
    const arrow = fx.data.__arrowObj?.material;
    if (arrow) { arrow.opacity = Math.min(1, fx.a * 1.6); arrow.color.copy(fx.mat.color); }
  }

  /** One frame: `ms` the animation clock, `camera` the scene's camera. */
  frame(ms: number, camera: Three) {
    const dt = Math.min(0.1, Math.max(0, (ms - this.lastMs) / 1000));
    this.lastMs = ms;
    const beat = 0.5 + 0.5 * Math.sin(ms / 520);
    this.nodes.forEach(({ parts }) => this.animate(parts, ms, beat));
    const p = camera.position;
    if (p.x !== this.cam.x || p.y !== this.cam.y || p.z !== this.cam.z) {
      this.cam.x = p.x; this.cam.y = p.y; this.cam.z = p.z;
      this.dirty = true;
    }
    if (this.dirty) { this.dirty = false; this.settling = true; this.placeLabels(p); }
    if (this.settling) this.settle(dt);
  }

  private animate(parts: NodeParts, ms: number, beat: number) {
    if (this.reduced) return;
    for (const o of parts.spin) o.rotation.z += 0.012;
    for (const o of parts.turn) { o.rotation.y += 0.004; o.rotation.x += 0.0015; }
    const core = parts.core;
    if (!core) return;
    const since = ms - parts.ping;
    const k = since >= 0 ? 1 - since / PING_MS : 0;
    const jump = k > 0 ? 1 + 0.6 * k * k : 1;
    if (parts.halo) {
      // The rich glow: brighter and wider with each breath.
      parts.halo.material.opacity = (0.1 + 0.22 * beat) * parts.alpha;
      parts.halo.scale.setScalar(1 + 0.18 * beat);
      if (core.material.emissiveIntensity !== undefined) core.material.emissiveIntensity = 0.6 + 0.6 * beat;
    }
    const breath = parts.busy && !parts.halo ? 1 + 0.08 * beat : 1;
    const s = breath * jump;
    if (s !== 1 || core.scale.x !== 1) core.scale.setScalar(s);
  }

  private placeLabels(cam: Three) {
    const n = this.ids.length;
    this.dist.length = n;
    this.pinned.length = n;
    for (let i = 0; i < n; i += 1) {
      const node = this.nodes.get(this.ids[i]);
      this.dist[i] = node ? cam.distanceTo(node.obj.position) : Infinity;
      this.pinned[i] = this.forced.has(this.ids[i]);
    }
    labelTargets(this.dist, this.pinned, n, this.threshold, LABEL_CAP, this.out, this.scratch);
  }

  private settle(dt: number) {
    let moving = false;
    for (let i = 0; i < this.ids.length; i += 1) {
      const id = this.ids[i];
      const node = this.nodes.get(id);
      if (!node) continue;
      const parts = node.parts;
      const target = this.nodeTarget(id);
      if (parts.alpha !== target) {
        parts.alpha = approach(parts.alpha, target, dt);
        for (const { m, base } of parts.mats) m.opacity = base * parts.alpha;
        moving = true;
      }
      if (parts.label) {
        const want = (this.fadeLabels ? this.out[i] ?? 0 : 1) * parts.alpha;
        if (parts.labelAlpha !== want) { parts.labelAlpha = approach(parts.labelAlpha, want, dt); moving = true; }
        parts.label.material.opacity = parts.labelAlpha;
        parts.label.visible = parts.labelAlpha > 0.01;
      }
      if (parts.detail) {
        const want = this.forced.has(id) ? parts.alpha : 0;
        if (parts.detailAlpha !== want) { parts.detailAlpha = approach(parts.detailAlpha, want, dt); moving = true; }
        parts.detail.material.opacity = parts.detailAlpha;
        parts.detail.visible = parts.detailAlpha > 0.01;
      }
    }
    this.links.forEach((fx, id) => {
      const inFocus = !!this.focus?.links.has(id);
      const a = this.focus ? (inFocus ? 0.9 : this.faded) : fx.rest;
      const h = this.focus && inFocus ? 1 : fx.restHue;
      if (fx.a === a && fx.h === h) { if (this.repaint) this.paintLink(fx); return; }
      fx.a = approach(fx.a, a, dt);
      fx.h = approach(fx.h, h, dt);
      this.paintLink(fx);
      moving = true;
    });
    this.repaint = false;
    this.settling = moving;
  }
}
