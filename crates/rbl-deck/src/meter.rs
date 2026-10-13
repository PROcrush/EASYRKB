//! Unweighted 400 ms stereo RMS of the actual device output. History is
//! allocated before opening the sink; processing never allocates or locks.

pub(crate) struct WindowRms {
    history: Vec<[f32; 2]>,
    sum: [f64; 2],
    at: usize,
    length: usize,
}

impl WindowRms {
    pub(crate) fn new() -> Self {
        Self { history: vec![[0.0; 2]; 384_000 * 2 / 5], sum: [0.0; 2], at: 0, length: 0 }
    }

    pub(crate) fn set_rate(&mut self, rate: u32) {
        let length = ((u64::from(rate) * 2 / 5) as usize).clamp(1, self.history.len());
        if length != self.length {
            self.history.fill([0.0; 2]);
            self.sum = [0.0; 2];
            self.at = 0;
            self.length = length;
        }
    }

    pub(crate) fn push(&mut self, left: f32, right: f32) {
        let Some(slot) = self.history.get_mut(self.at) else { return };
        for ((sum, old), sample) in self.sum.iter_mut().zip(slot.iter_mut()).zip([left, right]) {
            let power = if sample.is_finite() { sample * sample } else { 0.0 };
            *sum = (*sum + f64::from(power) - f64::from(*old)).max(0.0);
            *old = power;
        }
        self.at += 1;
        if self.at >= self.length { self.at = 0; }
    }

    pub(crate) fn levels(&self) -> [f32; 2] {
        self.sum.map(|sum| (sum / self.length.max(1) as f64).sqrt() as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_measures_power_per_channel_and_expires_after_400_ms() {
        for rate in [44_100, 48_000, 96_000] {
            let mut rms = WindowRms::new();
            rms.set_rate(rate);
            for n in 0..rate {
                let sine = (f64::from(n) * 1000.0 * std::f64::consts::TAU / f64::from(rate)).sin() as f32;
                rms.push(sine, sine * 0.5);
            }
            let [left, right] = rms.levels();
            assert!((left - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.0001);
            assert!((right - left * 0.5).abs() < 0.0001);
            for _ in 0..rate / 5 { rms.push(0.0, 0.0); }
            assert!((rms.levels()[0] - 0.5).abs() < 0.0001);
            for _ in 0..rate / 5 { rms.push(0.0, 0.0); }
            assert!(rms.levels().iter().all(|v| *v < 0.00001));
        }
    }

    #[test]
    #[allow(clippy::float_cmp, reason = "a reset writes literal 0.0, not a computed value")]
    fn startup_is_zero_padded_and_rate_change_resets_history() {
        let mut rms = WindowRms::new();
        rms.set_rate(1000);
        rms.push(1.0, f32::NAN);
        assert!((rms.levels()[0] - 0.05).abs() < 0.00001);
        assert_eq!(rms.levels()[1], 0.0);
        rms.set_rate(2000);
        assert_eq!(rms.levels(), [0.0; 2]);
    }
}
