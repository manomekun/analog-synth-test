//! Oscillator. M1: naive sine only; band-limited PolyBLEP waveforms arrive in M2.

use std::f32::consts::TAU;

#[derive(Debug, Clone)]
pub struct Oscillator {
    sample_rate: f32,
    phase: f32,
    phase_inc: f32,
}

impl Oscillator {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            phase: 0.0,
            phase_inc: 0.0,
        }
    }

    pub fn set_frequency(&mut self, freq_hz: f32) {
        self.phase_inc = freq_hz / self.sample_rate;
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        let out = (self.phase * TAU).sin();
        self.phase += self.phase_inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        out
    }
}
