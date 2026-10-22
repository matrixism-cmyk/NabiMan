/**
 * How the microphone's sound goes to the speech service: as Opus at 32 kbps
 * where the browser can make it (WebCodecs), one eighth of plain 16 kHz PCM
 * (256 kbps) and no worse to recognise — or as PCM where it cannot.
 *
 * Opus goes in VSP/1 bundles (v.bebo.kr's /docs 4.3): five 20 ms packets,
 * 100 ms, per message, each bundle carrying when its first packet starts.
 * Muted (Claude reading an answer aloud), packets are dropped and a 7-byte
 * "silent until here" bundle goes every 200 ms instead, so the service's
 * end-of-speech timing runs on; the first bundle after it is marked SILENCE.
 *
 * On a slow line the rate steps down (32 → 24 → 16 → 12 kbps) and back up once
 * it keeps up again. Slow is told by the service: every 2 s it says how far
 * into the sound it has got, and what was sent beyond that is still on the way
 * (a WebSocket's bufferedAmount stays 0 until the OS's buffer is full).
 */

const SR = 16000;
const FRAME_MS = 20;
const FRAME = SR * FRAME_MS / 1000;
const BUNDLE = 5;
const KEEP_EVERY = 10;
const FLAG_SILENCE = 1;
export const OPUS_BITRATE = 32000;
/** The rates stepped through on a slow line (v.bebo.kr's /docs: 12k still recognises well). */
export const OPUS_LADDER = [12000, 16000, 24000, 32000];
/** Sound still on the way, in ms, that counts as a slow line, or as one that keeps up. */
const SLOW_MS = 800;
const KEPT_UP_MS = 300;
/** No step sooner than this after the last one; up only after this long kept up. */
const STEP_GAP_MS = 3000;
const STEP_UP_MS = 8000;

/** A VSP/1 bundle: [version 1][flags][first packet's ms, u32 LE][count] + (u16 LE length + packet)×n. */
export function opusFrame(tMs: number, packets: Uint8Array[], flags = 0): ArrayBuffer {
  const buf = new Uint8Array(7 + packets.reduce((a, p) => a + 2 + p.length, 0));
  const dv = new DataView(buf.buffer);
  buf[0] = 1; buf[1] = flags; dv.setUint32(2, tMs >>> 0, true); buf[6] = packets.length;
  let o = 7;
  for (const p of packets) { dv.setUint16(o, p.length, true); buf.set(p, o + 2); o += 2 + p.length; }
  return buf.buffer;
}

export interface Uplink {
  /** What the opening message adds: the codec, and for Opus its rate and the asking for stats. */
  config: Record<string, string | number | boolean>;
  /** One 100 ms frame of 16 kHz PCM, as the resampler makes them. */
  push(frame: ArrayBuffer, muted: boolean): void;
  /** The service's "stats" message: how far into the sound it has got. */
  stats(message: { received_ms?: unknown }): void;
  /** Sends what is still in the encoder: before "stop", or the last words are lost. */
  finish(): Promise<void>;
  close(): void;
}

/** Plain PCM: frames as they come, silence while muted. */
export function pcmUplink(send: (data: ArrayBuffer) => void): Uplink {
  return {
    // Nothing to say: PCM is what the service takes by default.
    config: {},
    push: (frame, muted) => send(muted ? new ArrayBuffer(frame.byteLength) : frame),
    stats: () => undefined,
    finish: async () => undefined,
    close: () => undefined,
  };
}

/** Turns 16 kHz samples into numbered 20 ms Opus packets (frame k covers k×20 ms). */
export interface PacketEncoder {
  encode(samples: Float32Array): void;
  setBitrate(bitrate: number): void;
  flush(): Promise<void>;
  close(): void;
}
export type MakeEncoder = (onPacket: (k: number, packet: Uint8Array) => void) => Promise<PacketEncoder | null>;

const opusConfig = (bitrate = OPUS_BITRATE) => ({
  codec: 'opus', sampleRate: SR, numberOfChannels: 1, bitrate,
  opus: { format: 'opus', frameDuration: FRAME_MS * 1000, application: 'voip', signal: 'voice',
    complexity: typeof matchMedia === 'function' && matchMedia('(pointer: coarse)').matches ? 5 : 9 },
});

// What WebCodecs offers, as far as it is used here (not every lib.dom has it).
interface EncodedChunk { byteLength: number; timestamp: number; copyTo(b: Uint8Array): void }
interface AudioEncoderLike { state: string; configure(c: unknown): void; encode(d: unknown): void; flush(): Promise<void>; close(): void }
interface WebCodecs {
  AudioEncoder?: {
    new (init: { output: (c: EncodedChunk) => void; error: (e: unknown) => void }): AudioEncoderLike;
    isConfigSupported(c: unknown): Promise<{ supported?: boolean }>;
  };
  AudioData?: new (init: Record<string, unknown>) => { close(): void };
}

/** The browser's own Opus encoder (WebCodecs), or null where there is none. */
export const webCodecsEncoder: MakeEncoder = async (onPacket) => {
  const { AudioEncoder, AudioData } = window as unknown as WebCodecs;
  if (!AudioEncoder || !AudioData) return null;
  const config = opusConfig();
  try { if (!(await AudioEncoder.isConfigSupported(config)).supported) return null; } catch { return null; }
  const enc = new AudioEncoder({
    output: (chunk) => {
      const b = new Uint8Array(chunk.byteLength);
      chunk.copyTo(b);
      onPacket(Math.round(chunk.timestamp / (FRAME_MS * 1000)), b);
    },
    error: () => undefined,
  });
  enc.configure(config);
  let fed = 0;
  return {
    encode(samples) {
      if (enc.state !== 'configured' || !samples.length) return;
      // Timed by the samples fed, so packet k is always k×20 ms into the stream.
      const data = new AudioData({ format: 'f32', sampleRate: SR, numberOfFrames: samples.length, numberOfChannels: 1,
        timestamp: Math.round(fed * 1e6 / SR), data: samples });
      enc.encode(data);
      data.close();
      fed += samples.length;
    },
    setBitrate(bitrate) {
      // Packets keep their times: they are numbered by the samples fed, not by the encoder.
      try { if (enc.state === 'configured') enc.configure(opusConfig(bitrate)); } catch { /* the old rate goes on */ }
    },
    async flush() { try { await enc.flush(); } catch { /* closed meanwhile */ } },
    close() { try { enc.close(); } catch { /* already closed */ } },
  };
};

/** Opus over VSP/1 bundles, or null when the browser cannot encode it. */
export async function opusUplink(
  send: (data: ArrayBuffer) => void,
  make: MakeEncoder = webCodecsEncoder,
  clock: () => number = () => performance.now(),
): Promise<Uplink | null> {
  let batch: Uint8Array[] = [];
  let batchStart = 0;
  let batchFlags = 0;
  /** Frames fed while muted: their packets are not sent. */
  const muted = new Set<number>();
  let fedFrames = 0;
  let quiet = false;
  let lastKeep = -KEEP_EVERY;
  /** Where the sound sent so far ends, in ms. */
  let sentEnd = 0;

  const flushBatch = () => {
    if (!batch.length) return;
    send(opusFrame(batchStart * FRAME_MS, batch, batchFlags));
    sentEnd = (batchStart + batch.length) * FRAME_MS;
    batch = [];
    batchFlags = 0;
  };
  const onPacket = (k: number, packet: Uint8Array) => {
    if (muted.delete(k)) {
      // Silent until the end of this frame: said every 200 ms, nothing more.
      flushBatch();
      quiet = true;
      if (k - lastKeep >= KEEP_EVERY) { lastKeep = k; sentEnd = (k + 1) * FRAME_MS; send(opusFrame(sentEnd, [])); }
      return;
    }
    const flags = quiet ? FLAG_SILENCE : 0;
    quiet = false;
    if (batch.length && (k !== batchStart + batch.length || flags)) flushBatch();
    if (!batch.length) { batchStart = k; batchFlags = flags; }
    batch.push(packet);
    if (batch.length >= BUNDLE) flushBatch();
  };
  const encoder = await make(onPacket);
  if (!encoder) return null;

  let rate = OPUS_BITRATE;
  let lastStep = -Infinity;
  let keptUpSince = clock();
  let lastBehind = 0;
  const step = (by: number) => {
    const next = OPUS_LADDER[OPUS_LADDER.indexOf(rate) + by];
    if (!next) return;
    rate = next;
    lastStep = clock();
    encoder.setBitrate(rate);
  };
  return {
    config: { codec: 'opus', bitrate: OPUS_BITRATE, sample_rate: SR, stats: true },
    stats({ received_ms: received }) {
      if (typeof received !== 'number') return;
      const now = clock();
      const behind = sentEnd - received;
      // Down only while it keeps growing: a backlog already shrinking needs no help.
      if (behind > SLOW_MS && behind > lastBehind && now - lastStep > STEP_GAP_MS) { step(-1); keptUpSince = now; }
      else if (behind > KEPT_UP_MS) keptUpSince = now;
      else if (now - keptUpSince > STEP_UP_MS && now - lastStep > STEP_UP_MS) { step(1); keptUpSince = now; }
      lastBehind = behind;
    },
    push(frame, isMuted) {
      const pcm = new Int16Array(frame);
      const samples = new Float32Array(pcm.length);
      for (let i = 0; i < pcm.length; i += 1) samples[i] = pcm[i] / 0x8000;
      // The encoder keeps running while muted, so the timing stays simple.
      const frames = Math.floor(pcm.length / FRAME);
      if (isMuted) for (let i = 0; i < frames; i += 1) muted.add(fedFrames + i);
      fedFrames += frames;
      encoder.encode(samples);
    },
    async finish() {
      await encoder.flush();
      flushBatch();
    },
    close: () => encoder.close(),
  };
}
