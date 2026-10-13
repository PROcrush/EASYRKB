import type { MeterChannel } from "@/lib/vuMeter";
import type { VuMeterMode } from "@/lib/preferences";
import styles from "./VuMeterFill.module.css";

export function VuMeterFill({ channel, mode }: { channel: MeterChannel; mode: VuMeterMode }) {
  return <span className={styles.well} data-mode={mode} aria-hidden>
    <span className={styles.peak} style={{ clipPath: `inset(0 ${(1 - channel.peak) * 100}% 0 0)` }} />
    {mode === "fabulous" ? <span className={styles.rms} data-testid="vu-rms"
      style={{ clipPath: `inset(0 ${(1 - channel.rms) * 100}% 0 0)` }} /> : null}
    {channel.marker > 0 ? <span className={styles.marker} style={{ left: `${channel.marker * 100}%` }} /> : null}
  </span>;
}
