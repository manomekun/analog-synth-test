//! A single synthesizer voice. M1: one sine oscillator through the amp envelope.

use crate::dsp::envelope::Envelope;
use crate::dsp::oscillator::Oscillator;
use crate::params::SynthParams;

use nice_plug::util;

#[derive(Debug, Clone)]
pub struct Voice {
    /// The voice ID the host assigned to this note, if any (CLAP note events).
    pub voice_id: Option<i32>,
    pub channel: u8,
    pub note: u8,
    /// Monotonically increasing counter used for oldest-voice stealing.
    pub internal_id: u64,

    velocity_gain: f32,
    osc: Oscillator,
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
        let mut osc = Oscillator::new(sample_rate);
        osc.set_frequency(util::midi_note_to_freq(note));

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
            velocity_gain: velocity,
            osc,
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
    pub fn render(&mut self, output: &mut [f32], _params: &SynthParams) {
        for sample in output.iter_mut() {
            let env = self.amp_env.next();
            if self.amp_env.is_finished() {
                break;
            }
            *sample += self.osc.next() * env * self.velocity_gain;
        }
    }
}
