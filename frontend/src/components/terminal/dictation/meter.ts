/**
 * How loud the microphone has been lately, for the waveform in the button:
 * one bar per 60 ms, the newest last, each 0 (quiet) to 1 (loud).
 */

export const METER_BARS = 16;
const BAR_SECONDS = 0.06;
/** Loudness shown, in dBFS: a quiet room near the bottom, speech (about -30 to -10) in the upper half. */
const QUIET_DB = -55;
const LOUD_DB = 0;

export class LevelMeter {
  readonly bars = new Float32Array(METER_BARS);
  /** Goes up with every new bar, so a drawing knows when to redraw. */
  count = 0;
  private every = 48000 * BAR_SECONDS;
  private sum = 0;
  private n = 0;

  /** Starts over, for a microphone running at `sampleRate`. */
  reset(sampleRate: number) {
    this.bars.fill(0);
    this.every = Math.max(1, Math.round(sampleRate * BAR_SECONDS));
    this.sum = 0;
    this.n = 0;
    this.count += 1;
  }

  push(samples: Float32Array) {
    for (let i = 0; i < samples.length; i += 1) {
      this.sum += samples[i] * samples[i];
      this.n += 1;
      if (this.n >= this.every) this.bar();
    }
  }

  private bar() {
    const db = 10 * Math.log10(this.sum / this.n + 1e-12);
    this.bars.copyWithin(0, 1);
    this.bars[METER_BARS - 1] = Math.min(1, Math.max(0, (db - QUIET_DB) / (LOUD_DB - QUIET_DB)));
    this.sum = 0;
    this.n = 0;
    this.count += 1;
  }
}
