//! Band-limited oscillator using PolyBLEP (polynomial band-limited step)
//! correction for the discontinuous waveforms, with a leaky integrator over the
//! band-limited square for the triangle.

use std::f32::consts::TAU;

use nice_plug::prelude::Enum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum Waveform {
    Sine,
    Triangle,
    Saw,
    Square,
    Pulse,
}

/// Two-sample polynomial correction for a downward unit step at phase 0/1.
/// `t` is the current phase in [0, 1), `dt` the per-sample phase increment.
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        2.0 * t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

#[derive(Debug, Clone)]
pub struct Oscillator {
    inv_sample_rate: f32,
    phase: f32,
    phase_inc: f32,
    /// Leaky-integrator state for the triangle waveform.
    tri_state: f32,
}

impl Oscillator {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            inv_sample_rate: 1.0 / sample_rate,
            phase: 0.0,
            phase_inc: 0.0,
            // The triangle is at its minimum when the square starts high, so
            // starting the integrator here avoids a large DC transient.
            tri_state: -1.0,
        }
    }

    pub fn set_frequency(&mut self, freq_hz: f32) {
        // Keep the increment below Nyquist so PolyBLEP's `1 - dt` guard stays valid.
        self.phase_inc = (freq_hz * self.inv_sample_rate).min(0.5);
    }

    #[inline]
    pub fn next(&mut self, waveform: Waveform, pulse_width: f32) -> f32 {
        let t = self.phase;
        let dt = self.phase_inc;

        let out = match waveform {
            Waveform::Sine => (t * TAU).sin(),
            Waveform::Saw => {
                // Naive saw rises from -1 to 1, with a downward step at wrap.
                (2.0 * t - 1.0) - poly_blep(t, dt)
            }
            Waveform::Square => self.blep_pulse(t, dt, 0.5),
            Waveform::Pulse => self.blep_pulse(t, dt, pulse_width),
            Waveform::Triangle => {
                // Integrate the band-limited square; the small leak removes DC.
                let square = self.blep_pulse(t, dt, 0.5);
                self.tri_state = self.tri_state * (1.0 - 0.25 * dt) + 4.0 * dt * square;
                self.tri_state
            }
        };

        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        out
    }

    #[inline]
    fn blep_pulse(&self, t: f32, dt: f32, width: f32) -> f32 {
        let naive = if t < width { 1.0 } else { -1.0 };
        // Upward step at phase 0, downward step at phase `width`.
        let mut down = t - width;
        if down < 0.0 {
            down += 1.0;
        }
        naive + poly_blep(t, dt) - poly_blep(down, dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Render `n` samples of a fixed-frequency waveform.
    fn render(waveform: Waveform, freq: f32, sample_rate: f32, n: usize) -> Vec<f32> {
        let mut osc = Oscillator::new(sample_rate);
        osc.set_frequency(freq);
        (0..n).map(|_| osc.next(waveform, 0.5)).collect()
    }

    /// Render `n` samples of a naive (non-band-limited) saw for comparison.
    fn render_naive_saw(freq: f32, sample_rate: f32, n: usize) -> Vec<f32> {
        let dt = freq / sample_rate;
        let mut phase = 0.0f32;
        (0..n)
            .map(|_| {
                let out = 2.0 * phase - 1.0;
                phase += dt;
                if phase >= 1.0 {
                    phase -= 1.0;
                }
                out
            })
            .collect()
    }

    /// Ratio (in dB) of energy in non-harmonic bins to energy in harmonic bins.
    /// Full-band alias totals are dominated by near-Nyquist residuals that
    /// 2-point PolyBLEP suppresses least (and the ear notices least), so
    /// `alias_band_hz` allows measuring just the audibly-sensitive band.
    fn alias_ratio_band_db(signal: &[f32], freq: f32, sample_rate: f32, alias_band_hz: f32) -> f32 {
        use rustfft::{num_complex::Complex, FftPlanner};

        let n = signal.len();
        let mut buf: Vec<Complex<f32>> = signal
            .iter()
            .enumerate()
            .map(|(i, &x)| {
                // 4-term Blackman-Harris window: sidelobes at -92 dB so the
                // window leakage doesn't put a floor under the alias readings.
                let p = TAU * i as f32 / (n as f32 - 1.0);
                let w = 0.35875 - 0.48829 * p.cos() + 0.14128 * (2.0 * p).cos()
                    - 0.01168 * (3.0 * p).cos();
                Complex::new(x * w, 0.0)
            })
            .collect();
        FftPlanner::new().plan_fft_forward(n).process(&mut buf);

        let bin_hz = sample_rate / n as f32;
        let mut harmonic_energy = 0.0f64;
        let mut alias_energy = 0.0f64;
        // Skip the lowest bins (DC + window leakage skirt).
        for (bin, value) in buf.iter().enumerate().take(n / 2).skip(4) {
            let bin_freq = bin as f32 * bin_hz;
            let harmonic = (bin_freq / freq).round();
            let is_harmonic = harmonic >= 1.0 && (bin_freq - harmonic * freq).abs() < 6.0 * bin_hz;
            let energy = (value.norm_sqr()) as f64;
            if is_harmonic {
                harmonic_energy += energy;
            } else if bin_freq < alias_band_hz {
                alias_energy += energy;
            }
        }

        10.0 * (alias_energy / harmonic_energy).log10() as f32
    }

    fn alias_ratio_db(signal: &[f32], freq: f32, sample_rate: f32) -> f32 {
        alias_ratio_band_db(signal, freq, sample_rate, sample_rate)
    }

    #[test]
    fn polyblep_saw_suppresses_aliasing() {
        let sample_rate = 44_100.0;
        // Deliberately awkward frequency: harmonics fold back inharmonically.
        let freq = 2_637.02;
        let n = 1 << 15;

        let blep = alias_ratio_db(
            &render(Waveform::Saw, freq, sample_rate, n),
            freq,
            sample_rate,
        );
        let naive = alias_ratio_db(&render_naive_saw(freq, sample_rate, n), freq, sample_rate);

        // Two-point PolyBLEP reaches roughly -30 dB total alias energy at this
        // deliberately hostile fundamental (matches the published SNR figures
        // for the method); the naive saw sits around -11 dB. The comparative
        // assertion guards against silently disabling the BLEP correction.
        assert!(blep < -25.0, "PolyBLEP saw aliasing too high: {blep:.1} dB");
        assert!(naive > -15.0, "naive saw unexpectedly clean: {naive:.1} dB");
        assert!(
            blep < naive - 12.0,
            "BLEP ({blep:.1} dB) should be much cleaner than naive ({naive:.1} dB)"
        );
    }

    #[test]
    fn polyblep_saw_is_clean_in_musical_range() {
        let sample_rate = 44_100.0;
        let freq = 523.25; // C5
        let n = 1 << 15;

        let signal = render(Waveform::Saw, freq, sample_rate, n);
        // In the band the ear is most sensitive to aliasing, suppression must
        // be strong even though the near-Nyquist residual is larger.
        let audible = alias_ratio_band_db(&signal, freq, sample_rate, 12_000.0);
        let full = alias_ratio_db(&signal, freq, sample_rate);
        eprintln!("saw C5: audible-band {audible:.1} dB, full-band {full:.1} dB");
        assert!(
            audible < -50.0,
            "saw at C5 audible-band aliasing: {audible:.1} dB"
        );
        assert!(full < -30.0, "saw at C5 full-band aliasing: {full:.1} dB");
    }

    #[test]
    fn square_wave_suppresses_aliasing() {
        let sample_rate = 44_100.0;
        let freq = 1_567.98;
        let n = 1 << 15;

        let blep = alias_ratio_db(
            &render(Waveform::Square, freq, sample_rate, n),
            freq,
            sample_rate,
        );
        assert!(
            blep < -28.0,
            "PolyBLEP square aliasing too high: {blep:.1} dB"
        );
    }

    #[test]
    fn waveforms_stay_bounded() {
        let sample_rate = 44_100.0;
        for waveform in [
            Waveform::Sine,
            Waveform::Triangle,
            Waveform::Saw,
            Waveform::Square,
            Waveform::Pulse,
        ] {
            for freq in [55.0, 440.0, 4_000.0, 12_000.0] {
                let signal = render(waveform, freq, sample_rate, 44_100);
                assert!(
                    signal.iter().all(|x| x.is_finite() && x.abs() <= 1.5),
                    "{waveform:?} at {freq} Hz out of bounds"
                );
            }
        }
    }
}
