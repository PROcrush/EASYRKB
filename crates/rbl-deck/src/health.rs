//! Audio callback deadline usage. The callback writes atomics; readers never
//! take a lock on the audio thread or allocate in it.
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

#[derive(Debug, Default, Clone, Copy)]
pub struct AudioHealth {
    /// Smoothed fraction of the buffer deadline consumed, clamped to 0..=1.
    pub load: f32,
    /// Callbacks whose processing exceeded the duration of their audio.
    pub xruns: u64,
}

#[derive(Default)]
pub(crate) struct HealthMeter {
    load: AtomicU32,
    xruns: AtomicU64,
}

impl HealthMeter {
    /// One writer: the device callback. Elapsed covers rendering and conversion.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    pub fn record(&self, elapsed: Duration, frames: usize, rate: u32) {
        if frames == 0 || rate == 0 { return; }
        let block = frames as f64 / f64::from(rate);
        let instant = elapsed.as_secs_f64() / block;
        let previous = f32::from_bits(self.load.load(Ordering::Relaxed));
        let load = (f64::from(previous) + 0.2 * (instant - f64::from(previous))).clamp(0.0, 1.0);
        self.load.store((load as f32).to_bits(), Ordering::Relaxed);
        if instant > 1.0 { self.xruns.fetch_add(1, Ordering::Relaxed); }
    }

    pub fn snapshot(&self, running: bool) -> AudioHealth {
        AudioHealth {
            load: if running { f32::from_bits(self.load.load(Ordering::Relaxed)) } else { 0.0 },
            xruns: self.xruns.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[allow(clippy::float_cmp, reason = "load is clamped to exactly 1.0 and reset to exactly 0.0")]
    fn measures_the_deadline_smooths_and_counts_only_overruns() {
        let meter = HealthMeter::default();
        meter.record(Duration::from_millis(5), 480, 48_000);
        assert!((meter.snapshot(true).load - 0.1).abs() < 0.0001);
        meter.record(Duration::from_millis(20), 480, 48_000);
        assert!((meter.snapshot(true).load - 0.48).abs() < 0.0001);
        assert_eq!(meter.snapshot(true).xruns, 1);
        meter.record(Duration::from_secs(1), 480, 48_000);
        assert_eq!(meter.snapshot(true).load, 1.0);
        assert_eq!(meter.snapshot(false).load, 0.0);
        assert_eq!(meter.snapshot(false).xruns, 2);
        meter.record(Duration::from_secs(1), 0, 0);
        assert_eq!(meter.snapshot(false).xruns, 2);
    }
}
