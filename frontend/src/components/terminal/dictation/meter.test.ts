/**
 * The loudness the waveform in the microphone button draws.
 */
import { LevelMeter, METER_BARS } from './meter';

const tone = (amplitude: number, samples: number) => Float32Array.from({ length: samples }, (_, i) => amplitude * Math.sin(i / 3));

test('one bar per 60 ms of sound, newest last, quiet at 0 and loud near the top', () => {
  const m = new LevelMeter();
  m.reset(16000);                       // 960 samples a bar
  const start = m.count;
  m.push(tone(0.0001, 960));            // about -83 dB
  m.push(tone(1, 960));                 // about -3 dB
  m.push(tone(0.02, 959));              // one sample short of a bar
  expect(m.count - start).toBe(2);
  expect(m.bars[METER_BARS - 2]).toBe(0);
  expect(m.bars[METER_BARS - 1]).toBeGreaterThan(0.9);
  m.push(tone(0.02, 1));                // about -37 dB: in between
  expect(m.bars[METER_BARS - 1]).toBeGreaterThan(0.25);
  expect(m.bars[METER_BARS - 1]).toBeLessThan(0.5);
  expect(m.bars[METER_BARS - 3]).toBe(0);              // the older bars moved left
  expect(m.bars[METER_BARS - 2]).toBeGreaterThan(0.9);
});

test('starting over clears the bars and follows the microphone\'s rate', () => {
  const m = new LevelMeter();
  m.reset(48000);
  m.push(tone(0.5, 2880));
  expect(m.bars[METER_BARS - 1]).toBeGreaterThan(0.8);
  m.reset(8000);
  expect(Array.from(m.bars).every((v) => v === 0)).toBe(true);
  const before = m.count;
  m.push(tone(0.5, 480));
  expect(m.count - before).toBe(1);
});
