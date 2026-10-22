/**
 * The microphone's sound on the way to the speech service: Opus in VSP/1
 * bundles where the browser can, PCM where not.
 */
import { MakeEncoder, opusFrame, opusUplink, pcmUplink } from './uplink';

/** An encoder that makes one 3-byte packet per 20 ms (320 samples) at once. */
const fakeEncoder: MakeEncoder = async (onPacket) => {
  let k = 0;
  let held = 0;
  return {
    encode(samples) {
      held += samples.length;
      while (held >= 320) { onPacket(k, Uint8Array.of(k & 255, 0xaa, 0xbb)); k += 1; held -= 320; }
    },
    async flush() { if (held) { onPacket(k, Uint8Array.of(k & 255)); k += 1; held = 0; } },
    close() {},
  };
};

/** A bundle read back: [version, flags, t_ms, packets…]. */
function read(buf: ArrayBuffer) {
  const b = new Uint8Array(buf);
  const dv = new DataView(buf);
  const packets: number[][] = [];
  let o = 7;
  for (let i = 0; i < b[6]; i += 1) { const n = dv.getUint16(o, true); packets.push(Array.from(b.slice(o + 2, o + 2 + n))); o += 2 + n; }
  return { version: b[0], flags: b[1], t: dv.getUint32(2, true), packets };
}

const frame = (samples = 1600) => new Int16Array(samples).buffer;

test('a bundle is version 1, flags, the first packet\'s time and the packets with their lengths', () => {
  expect(Array.from(new Uint8Array(opusFrame(260, [Uint8Array.of(9, 8), Uint8Array.of(7)], 1))))
    .toEqual([1, 1, 4, 1, 0, 0, 2, 2, 0, 9, 8, 1, 0, 7]);
  expect(new Uint8Array(opusFrame(400, [])).length).toBe(7);
});

test('100 ms of sound goes as one bundle of five 20 ms packets, the opening says Opus', async () => {
  const sent: ArrayBuffer[] = [];
  const up = await opusUplink((d) => sent.push(d), fakeEncoder);
  expect(up?.config).toEqual({ codec: 'opus', bitrate: 32000, sample_rate: 16000 });
  up!.push(frame(), false);
  up!.push(frame(), false);
  expect(sent.map(read)).toEqual([
    { version: 1, flags: 0, t: 0, packets: [0, 1, 2, 3, 4].map((k) => [k, 0xaa, 0xbb]) },
    { version: 1, flags: 0, t: 100, packets: [5, 6, 7, 8, 9].map((k) => [k, 0xaa, 0xbb]) },
  ]);
});

test('muted, only "silent until here" every 200 ms; speaking again is marked SILENCE', async () => {
  const sent: ArrayBuffer[] = [];
  const up = (await opusUplink((d) => sent.push(d), fakeEncoder))!;
  up.push(frame(), false);       // frames 0-4
  up.push(frame(), true);        // 5-9: muted
  up.push(frame(), true);        // 10-14
  up.push(frame(), true);        // 15-19
  up.push(frame(), false);       // 20-24
  const got = sent.map(read);
  expect(got.map((b) => [b.t, b.packets.length, b.flags])).toEqual([
    [0, 5, 0],
    [120, 0, 0],    // frame 5 dropped: silent until its end
    [320, 0, 0],    // frame 15, 200 ms on
    [400, 5, 1],    // sound again, after silence
  ]);
  expect(sent.slice(1, 3).every((b) => b.byteLength === 7)).toBe(true);
});

test('finishing sends what is still in the encoder, before "stop" can go', async () => {
  const sent: ArrayBuffer[] = [];
  const up = (await opusUplink((d) => sent.push(d), fakeEncoder))!;
  up.push(frame(960 + 100), false);   // three whole packets, and a bit
  expect(sent).toHaveLength(0);
  await up.finish();
  expect(sent.map(read)).toEqual([{ version: 1, flags: 0, t: 0, packets: [[0, 0xaa, 0xbb], [1, 0xaa, 0xbb], [2, 0xaa, 0xbb], [3]] }]);
});

test('without an encoder it is PCM, as before: said by nothing, silence while muted', async () => {
  expect(await opusUplink(() => undefined, async () => null)).toBeNull();
  const sent: ArrayBuffer[] = [];
  const up = pcmUplink((d) => sent.push(d));
  expect(up.config).toEqual({});
  const loud = new Int16Array([1000, -1000]).buffer;
  up.push(loud, false);
  up.push(loud, true);
  expect(Array.from(new Int16Array(sent[0]))).toEqual([1000, -1000]);
  expect(Array.from(new Int16Array(sent[1]))).toEqual([0, 0]);
});
