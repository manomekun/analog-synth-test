//! Topology-preserving-transform (zero-delay feedback) state variable filter,
//! after Andrew Simper's "Solving the continuous SVF equations using
//! trapezoidal integration" (Cytomic technical paper). Numerically stable under
//! fast cutoff modulation, which is exactly what a filter envelope produces.

use std::f32::consts::PI;

/// Below this magnitude the integrator states are snapped to zero to keep
/// denormals out of the feedback path on platforms without FTZ.
const DENORMAL_EPSILON: f32 = 1e-18;

#[derive(Debug, Clone, Default)]
pub struct Svf {
    ic1eq: f32,
    ic2eq: f32,
}

impl Svf {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    /// Integrator gain for a cutoff in Hz. Callers should clamp the cutoff to
    /// well below Nyquist before calling.
    #[inline]
    pub fn g(cutoff_hz: f32, sample_rate: f32) -> f32 {
        (PI * cutoff_hz / sample_rate).tan()
    }

    /// Damping from a 0..1 resonance amount. 1/Q = k: 2.0 is critically
    /// damped, small values approach self-oscillation.
    #[inline]
    pub fn k(resonance: f32) -> f32 {
        2.0 - 1.95 * resonance.clamp(0.0, 1.0)
    }

    /// Process one sample as a lowpass with the given coefficients.
    #[inline]
    pub fn lowpass(&mut self, input: f32, g: f32, k: f32) -> f32 {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        let v3 = input - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        if self.ic1eq.abs() < DENORMAL_EPSILON {
            self.ic1eq = 0.0;
        }
        if self.ic2eq.abs() < DENORMAL_EPSILON {
            self.ic2eq = 0.0;
        }

        v2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    /// Steady-state gain of the filter for a sine at `freq`.
    fn measure_gain(cutoff: f32, resonance: f32, freq: f32, sample_rate: f32) -> f32 {
        let mut svf = Svf::new();
        let g = Svf::g(cutoff, sample_rate);
        let k = Svf::k(resonance);

        let settle = (sample_rate * 0.2) as usize;
        let measure = (sample_rate * 0.5) as usize;
        let mut in_energy = 0.0f64;
        let mut out_energy = 0.0f64;
        for i in 0..settle + measure {
            let x = (TAU * freq * i as f32 / sample_rate).sin();
            let y = svf.lowpass(x, g, k);
            if i >= settle {
                in_energy += (x * x) as f64;
                out_energy += (y * y) as f64;
            }
        }
        10.0 * (out_energy / in_energy).log10() as f32
    }

    #[test]
    fn lowpass_response_matches_butterworth() {
        let sample_rate = 44_100.0;
        let cutoff = 1_000.0;
        // k = sqrt(2) (Butterworth damping) => resonance ~ 0.3
        let resonance = (2.0 - std::f32::consts::SQRT_2) / 1.95;

        let passband = measure_gain(cutoff, resonance, 100.0, sample_rate);
        let at_cutoff = measure_gain(cutoff, resonance, cutoff, sample_rate);
        let two_octaves_up = measure_gain(cutoff, resonance, 4_000.0, sample_rate);

        assert!(passband.abs() < 0.5, "passband gain {passband:.2} dB");
        assert!(
            (at_cutoff + 3.0).abs() < 0.7,
            "cutoff gain {at_cutoff:.2} dB, expected -3 dB"
        );
        // 12 dB/oct slope: two octaves above cutoff ~= -24 dB
        assert!(
            (two_octaves_up + 24.0).abs() < 2.5,
            "stopband gain {two_octaves_up:.2} dB, expected about -24 dB"
        );
    }

    #[test]
    fn resonance_peaks_at_cutoff() {
        let sample_rate = 44_100.0;
        let cutoff = 1_000.0;

        let flat = measure_gain(cutoff, 0.2, cutoff, sample_rate);
        let resonant = measure_gain(cutoff, 0.9, cutoff, sample_rate);
        assert!(
            resonant > flat + 6.0,
            "high resonance ({resonant:.1} dB) should peak well above low resonance ({flat:.1} dB)"
        );
    }

    #[test]
    fn survives_random_cutoff_jumps() {
        let sample_rate = 44_100.0;
        let mut svf = Svf::new();
        let k = Svf::k(0.95);
        let mut noise = crate::dsp::noise::Noise::new(0xDEAD_BEEF);
        let mut rng = crate::dsp::noise::Noise::new(0x1234_5678);

        let mut peak = 0.0f32;
        for _ in 0..(sample_rate as usize) {
            // Random cutoff anywhere from 20 Hz to 20 kHz, every sample.
            let cutoff = 20.0 * (10.0f32).powf((rng.next() * 0.5 + 0.5) * 3.0);
            let g = Svf::g(cutoff, sample_rate);
            let y = svf.lowpass(noise.next(), g, k);
            assert!(y.is_finite(), "filter output went non-finite");
            peak = peak.max(y.abs());
        }
        assert!(peak < 20.0, "filter output blew up: peak {peak}");
    }
}
