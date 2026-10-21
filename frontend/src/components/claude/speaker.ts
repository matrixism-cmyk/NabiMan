/**
 * Reading sentences aloud one after another, the next one fetched while the
 * current one plays, so the answer flows without gaps. One audio element
 * plays everything: a phone lets a page play sound only through an element
 * a tap has started once (see `unlock`).
 */

import { markActivity } from '../../shared/auth/useIdleLogout';

export type Fetch = (text: string, signal: AbortSignal) => Promise<Blob>;

/** A tenth of a second of silence, to unlock playback during a tap. */
const SILENT = 'data:audio/wav;base64,UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAA=';

interface Item { text: string; blob?: Promise<Blob> }

export class Speaker {
  private audio: HTMLAudioElement;
  private items: Item[] = [];
  private abort = new AbortController();
  private running = false;
  private url = '';
  /** Playback speed, 0.5 to 2; the pitch stays. */
  rate = 1;

  constructor(private fetch: Fetch, private onSpeaking: (speaking: boolean) => void, private onError?: (message: string) => void) {
    this.audio = typeof Audio === 'undefined' ? ({} as HTMLAudioElement) : new Audio();
  }

  /** Call from a tap: lets later sentences play without one. */
  unlock(): void {
    try {
      this.audio.src = SILENT;
      this.audio.play?.()?.catch(() => undefined);
    } catch { /* nothing to unlock */ }
  }

  get speaking(): boolean { return this.running; }

  say(text: string): void {
    if (!text.trim()) return;
    markActivity(); // listening to an answer is using NabiMan
    this.items.push({ text });
    this.prefetch();
    if (!this.running) { this.running = true; this.onSpeaking(true); void this.loop(); }
  }

  /** Stops at once and forgets what was still to be said. */
  stop(): void {
    this.abort.abort();
    this.abort = new AbortController();
    this.items = [];
    try { this.audio.pause?.(); } catch { /* not playing */ }
    this.release();
    if (this.running) { this.running = false; this.onSpeaking(false); }
  }

  private prefetch(): void {
    // The one playing and the next are fetched; the rest wait their turn.
    for (const item of this.items.slice(0, 2)) {
      if (!item.blob) {
        item.blob = this.fetch(item.text, this.abort.signal);
        item.blob.catch(() => undefined);
      }
    }
  }

  private release(): void {
    if (this.url) { URL.revokeObjectURL(this.url); this.url = ''; }
  }

  private async loop(): Promise<void> {
    const signal = this.abort.signal;
    while (this.items.length && !signal.aborted) {
      this.prefetch();
      const item = this.items[0];
      try {
        const blob = await item.blob!;
        if (signal.aborted) return;
        await this.play(blob, signal);
      } catch (e) {
        if (signal.aborted) return;
        this.onError?.(e instanceof Error ? e.message : String(e));
      }
      this.items.shift();
    }
    if (!signal.aborted) { this.running = false; this.onSpeaking(false); }
  }

  private play(blob: Blob, signal: AbortSignal): Promise<void> {
    return new Promise((resolve, reject) => {
      this.release();
      this.url = URL.createObjectURL(blob);
      const a = this.audio;
      const done = () => { a.onended = null; a.onerror = null; signal.removeEventListener('abort', done); resolve(); };
      a.onended = done;
      a.onerror = () => { a.onended = null; a.onerror = null; reject(new Error('This sound cannot be played')); };
      signal.addEventListener('abort', done);
      a.src = this.url;
      a.defaultPlaybackRate = this.rate;
      a.playbackRate = this.rate;
      (a as HTMLAudioElement & { preservesPitch?: boolean }).preservesPitch = true;
      a.play().catch(reject);
    });
  }
}
