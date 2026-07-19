//! A single synthesizer voice: 2 oscillators + noise -> mixer -> amp envelope.

use crate::dsp::envelope::Envelope;
use crate::dsp::noise::Noise;
use crate::dsp::oscillator::{Oscillator, Waveform};
use crate::params::SynthParams;

use nice_plug::util;

/// Per-block parameter values shared by all voices. The smoothed params are
/// consumed exactly once per sub-block into these slices by the process loop,
/// so that every voice sees the same values without advancing the smoothers.
pub struct RenderParams<'a> {
    pub osc1_wave: Waveform,
    pub osc2_wave: Waveform,
    /// Static frequency multiplier for osc 2 from the (unsmoothed) octave param.
    pub osc2_octave_mult: f32,
    pub osc1_pw: &'a [f32],
    pub osc2_pw: &'a [f32],
    pub osc2_detune_cents: &'a [f32],
    pub osc1_level: &'a [f32],
    pub osc2_level: &'a [f32],
    pub noise_level: &'a [f32],
}

#[derive(Debug, Clone)]
pub struct Voice {
    /// The voice ID the host assigned to this note, if any (CLAP note events).
    pub voice_id: Option<i32>,
    pub channel: u8,
    pub note: u8,
    /// Monotonically increasing counter used for oldest-voice stealing.
    pub internal_id: u64,

    base_freq: f32,
    velocity_gain: f32,
    osc1: Oscillator,
    osc2: Oscillator,
    noise: Noise,
    amp_env: Envelope,
}

impl Voice {
    pub fn new(
        sample_rate: f32,
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        velocity: f32,
        internal_id: u64,
        params: &SynthParams,
    ) -> Self {
        let base_freq = util::midi_note_to_freq(note);

        let mut osc1 = Oscillator::new(sample_rate);
        osc1.set_frequency(base_freq);
        let osc2 = Oscillator::new(sample_rate);

        let mut amp_env = Envelope::new(sample_rate);
        amp_env.note_on(
            params.amp_attack.value(),
            params.amp_decay.value(),
            params.amp_sustain.value(),
            params.amp_release.value(),
        );

        Self {
            voice_id,
            channel,
            note,
            internal_id,
            base_freq,
            velocity_gain: velocity,
            osc1,
            osc2,
            // Vary the seed per voice so unison noise doesn't correlate.
            noise: Noise::new(0x9E37_79B9 ^ internal_id as u32),
            amp_env,
        }
    }

    pub fn note_off(&mut self) {
        self.amp_env.note_off();
    }

    /// Begin the anti-click fade used when this voice is stolen or choked.
    pub fn fast_release(&mut self) {
        self.amp_env.fast_release();
    }

    pub fn is_finished(&self) -> bool {
        self.amp_env.is_finished()
    }

    pub fn is_releasing(&self) -> bool {
        self.amp_env.is_releasing()
    }

    /// Render this voice additively into a mono buffer.
    pub fn render(&mut self, output: &mut [f32], rp: &RenderParams) {
        for (i, sample) in output.iter_mut().enumerate() {
            let env = self.amp_env.next();
            if self.amp_env.is_finished() {
                break;
            }

            let osc2_freq =
                self.base_freq * rp.osc2_octave_mult * (rp.osc2_detune_cents[i] / 1200.0).exp2();
            self.osc2.set_frequency(osc2_freq);

            let mixed = self.osc1.next(rp.osc1_wave, rp.osc1_pw[i]) * rp.osc1_level[i]
                + self.osc2.next(rp.osc2_wave, rp.osc2_pw[i]) * rp.osc2_level[i]
                + self.noise.next() * rp.noise_level[i];

            *sample += mixed * env * self.velocity_gain;
        }
    }
}
