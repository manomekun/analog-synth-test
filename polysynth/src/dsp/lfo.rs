//! Low-frequency oscillator, global (shared phase across all voices).

use std::f32::consts::TAU;

use nice_plug::prelude::Enum;

use crate::dsp::noise::Noise;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum LfoWaveform {
    Sine,
    Triangle,
    Saw,
    Square,
    #[name = "Sample & Hold"]
    SampleHold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum LfoDestination {
    None,
    Pitch,
    Cutoff,
    Amp,
}

#[derive(Debug, Clone)]
pub struct Lfo {
    inv_sample_rate: f32,
    phase: f32,
    phase_inc: f32,
    noise: Noise,
    sh_value: f32,
}

impl Lfo {
    pub fn new(sample_rate: f32) -> Self {
        let mut noise = Noise::new(0xC0FF_EE00);
        let sh_value = noise.next();
        Self {
            inv_sample_rate: 1.0 / sample_rate,
            phase: 0.0,
            phase_inc: 0.0,
            noise,
            sh_value,
        }
    }

    pub fn set_rate(&mut self, rate_hz: f32) {
        self.phase_inc = (rate_hz * self.inv_sample_rate).min(0.49);
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Bipolar output in [-1, 1].
    #[inline]
    pub fn next(&mut self, waveform: LfoWaveform) -> f32 {
        let t = self.phase;
        let out = match waveform {
            LfoWaveform::Sine => (t * TAU).sin(),
            LfoWaveform::Triangle => {
                // Peak at t=0.25, trough at t=0.75, zero at t=0.
                if t < 0.25 {
                    4.0 * t
                } else if t < 0.75 {
                    2.0 - 4.0 * t
                } else {
                    4.0 * t - 4.0
                }
            }
            LfoWaveform::Saw => 1.0 - 2.0 * t,
            LfoWaveform::Square => {
                if t < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            LfoWaveform::SampleHold => self.sh_value,
        };

        self.phase += self.phase_inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.sh_value = self.noise.next();
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_waveforms_stay_bipolar() {
        for waveform in [
            LfoWaveform::Sine,
            LfoWaveform::Triangle,
            LfoWaveform::Saw,
            LfoWaveform::Square,
            LfoWaveform::SampleHold,
        ] {
            let mut lfo = Lfo::new(44_100.0);
            lfo.set_rate(5.0);
            for _ in 0..44_100 {
                let v = lfo.next(waveform);
                assert!((-1.0..=1.0).contains(&v), "{waveform:?} out of range: {v}");
            }
        }
    }

    #[test]
    fn rate_matches_zero_crossings() {
        let mut lfo = Lfo::new(44_100.0);
        lfo.set_rate(2.5);
        let mut crossings = 0;
        let mut prev = lfo.next(LfoWaveform::Sine);
        for _ in 0..44_100 {
            let v = lfo.next(LfoWaveform::Sine);
            // Strictly-negative previous sample excludes the initial rise from 0.
            if prev < 0.0 && v >= 0.0 {
                crossings += 1;
            }
            prev = v;
        }
        assert_eq!(
            crossings, 2,
            "2.5 Hz sine crosses upward from negative at 0.4 s and 0.8 s"
        );
    }
}
