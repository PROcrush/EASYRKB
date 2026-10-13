import type { VuMeterMode } from "./preferences";

export interface MeterChannel {
  /** Display fractions, not audio amplitudes. */
  peak: number;
  rms: number;
  marker: number;
}
export interface VuDisplay {
  mode: VuMeterMode;
  left: MeterChannel;
  right: MeterChannel;
}
export const emptyChannel = (): MeterChannel => ({ peak: 0, rms: 0, marker: 0 });
export const emptyVu = (mode: VuMeterMode): VuDisplay => ({ mode, left: emptyChannel(), right: emptyChannel() });

const dbOf = (value: number) => Number.isFinite(value) && value >= 2 ** -23 ? 20 * Math.log10(value) : -180;
const normalDb = (value: number) => value < -44 ? -100 : Math.min(0, Math.floor(value));
/** First match in [-100, -44, -43, -43, …, -1, -1, 0], divided by 88. */
export function normalHeight(db: number): number {
  return db < -44 ? 0 : db === -44 ? 1 / 88 : Math.min(1, (db + 44) / 44);
}
const fabulousHeight = (db: number) => Math.min(1, Math.max(0, (db + 32) / 32));

/** Rekordbox's quantized release and GUI marker, or a Pro-L-style 32 dB
 * display. Feed fresh interval peaks; RMS must come from audio samples.
 * Timing uses elapsed event time because our IPC doesn't publish peak offsets.
 */
export class VuMeter {
  private db = -100;
  private rmsDb = -180;
  private elapsed = 0;
  private rmsElapsed = 0;
  private markerDb = -180;
  private markerAge = 0;
  private pollMs = 0;
  private pendingZero = false;
  private shown = emptyChannel();

  constructor(private readonly mode: VuMeterMode) {}

  step(peak: number, rms: number, milliseconds: number): MeterChannel {
    const dt = Number.isFinite(milliseconds) ? Math.max(0, milliseconds) : 0;
    this.markerAge += dt;
    if (this.mode === "normal") {
      const reading = normalDb(dbOf(peak));
      if (reading >= this.db) {
        this.db = reading;
        this.elapsed = 0;
      } else {
        this.elapsed += dt;
        // Strictly greater than 20 ms, stepping before the first matching
        // threshold skips duplicate entries and falls one whole dB.
        const steps = Math.max(0, Math.ceil(this.elapsed / 20) - 1);
        this.elapsed -= steps * 20;
        this.db = Math.max(reading, normalDb(this.db - steps));
      }
      this.pollMs += dt;
      if (this.pollMs < 1000 / 15 && this.shown.peak !== 0) return this.shown;
      this.pollMs %= 1000 / 15;
      const height = normalHeight(this.db);
      const displayed = height === 0 && !this.pendingZero ? this.shown.peak : height;
      this.pendingZero = height === 0;
      if (this.markerAge >= 2000) this.markerDb = -180;
      if (height > normalHeight(this.markerDb)) {
        this.markerDb = this.db;
        this.markerAge = 0;
      }
      this.shown = { peak: displayed, rms: 0, marker: normalHeight(this.markerDb) };
    } else {
      const reading = dbOf(peak);
      this.elapsed += dt;
      if (reading > this.db) { this.db = reading; this.elapsed = 0; }
      else if (this.elapsed > 50) this.db = Math.max(reading, this.db - 8 * dt / 1000);
      const power = dbOf(rms);
      this.rmsElapsed += dt;
      if (power > this.rmsDb) { this.rmsDb = power; this.rmsElapsed = 0; }
      else if (this.rmsElapsed > 50) this.rmsDb = power;
      if (reading > this.markerDb) { this.markerDb = reading; this.markerAge = 0; }
      else if (this.markerAge > 2000) this.markerDb = Math.max(reading, this.markerDb - 15 * dt / 1000);
      this.shown = { peak: fabulousHeight(this.db), rms: fabulousHeight(this.rmsDb), marker: fabulousHeight(this.markerDb) };
    }
    return this.shown;
  }

  get active(): boolean { return this.shown.peak > 0 || this.shown.rms > 0 || this.shown.marker > 0; }
}
