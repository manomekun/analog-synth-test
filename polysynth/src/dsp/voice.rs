//! A single synthesizer voice: 2 oscillators + noise -> mixer -> amp envelope.

use crate::dsp::envelope::Envelope;
use crate::dsp::filter::Svf;
use crate::dsp::lfo::LfoDestination;
use crate::dsp::noise::Noise;
use crate::dsp::oscillator::{Oscillator, Waveform};
use crate::params::SynthParams;

use nice_plug::util;

/// Per-voice gain modulation state driven by CLAP `PolyModulation` /
/// `MonoAutomation` events. `current` chases `target` with a short one-pole
/// smoother so per-voice modulation doesn't zipper.
#[derive(Debug, Clone, Copy)]
pub struct GainMod {
    /// The host-provided normalized modulation offset for this voice.
    pub normalized_offset: f32,
    current: f32,
    target: f32,
}

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
    pub filter_cutoff: &'a [f32],
    pub filter_resonance: &'a [f32],
    pub filter_env_semitones: &'a [f32],
    pub filter_drive: &'a [f32],
    /// Keytrack amount 0..1 (unsmoothed).
    pub filter_keytrack: f32,
    /// Global LFO output per sample, bipolar -1..1.
    pub lfo: &'a [f32],
    pub lfo_amount: &'a [f32],
    pub lfo_dest: LfoDestination,
    /// Master gain per sample; used unless the voice has per-voice modulation.
    pub master_gain: &'a [f32],
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
    sample_rate: f32,
    velocity_gain: f32,
    osc1: Oscillator,
    osc2: Oscillator,
    noise: Noise,
    filter: Svf,
    amp_env: Envelope,
    filt_env: Envelope,
    gain_mod: Option<GainMod>,
    /// One-pole coefficient for the per-voice gain smoother (~5 ms).
    gain_mod_coef: f32,
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
        let mut filt_env = Envelope::new(sample_rate);
        filt_env.note_on(
            params.filt_attack.value(),
            params.filt_decay.value(),
            params.filt_sustain.value(),
            params.filt_release.value(),
        );

        Self {
            voice_id,
            channel,
            note,
            internal_id,
            base_freq,
            sample_rate,
            velocity_gain: velocity,
            osc1,
            osc2,
            // Vary the seed per voice so unison noise doesn't correlate.
            noise: Noise::new(0x9E37_79B9 ^ internal_id as u32),
            filter: Svf::new(),
            amp_env,
            filt_env,
            gain_mod: None,
            gain_mod_coef: 1.0 - (-1.0 / (0.005 * sample_rate)).exp(),
        }
    }

    /// Apply a host `PolyModulation` event: this voice's gain now follows its
    /// own modulated value instead of the global one.
    pub fn set_gain_modulation(&mut self, normalized_offset: f32, target_plain: f32) {
        let current = match self.gain_mod {
            Some(m) => m.current,
            None => target_plain,
        };
        self.gain_mod = Some(GainMod {
            normalized_offset,
            current,
            target: target_plain,
        });
    }

    /// Apply a host `MonoAutomation` event to a voice that has a poly
    /// modulation offset.
    pub fn update_gain_target(&mut self, target_plain: f32) {
        if let Some(m) = &mut self.gain_mod {
            m.target = target_plain;
        }
    }

    pub fn gain_mod_offset(&self) -> Option<f32> {
        self.gain_mod.map(|m| m.normalized_offset)
    }

    pub fn note_off(&mut self) {
        self.amp_env.note_off();
        self.filt_env.note_off();
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
        // Keytracking offset relative to middle C, in semitones.
        let keytrack_semis = rp.filter_keytrack * (self.note as f32 - 60.0);
        let max_cutoff = 0.45 * self.sample_rate;

        for (i, sample) in output.iter_mut().enumerate() {
            let env = self.amp_env.next();
            if self.amp_env.is_finished() {
                break;
            }

            let lfo = rp.lfo[i];
            let lfo_amount = rp.lfo_amount[i];

            // LFO -> pitch: up to +-1200 cents, squared for fine control at
            // small amounts.
            let pitch_mult = if rp.lfo_dest == LfoDestination::Pitch {
                (lfo * lfo_amount * lfo_amount).exp2()
            } else {
                1.0
            };

            self.osc1.set_frequency(self.base_freq * pitch_mult);
            let osc2_freq = self.base_freq
                * pitch_mult
                * rp.osc2_octave_mult
                * (rp.osc2_detune_cents[i] / 1200.0).exp2();
            self.osc2.set_frequency(osc2_freq);

            let mixed = self.osc1.next(rp.osc1_wave, rp.osc1_pw[i]) * rp.osc1_level[i]
                + self.osc2.next(rp.osc2_wave, rp.osc2_pw[i]) * rp.osc2_level[i]
                + self.noise.next() * rp.noise_level[i];

            // Modulate the cutoff in the exponential (semitone) domain.
            let filt_env = self.filt_env.next();
            let mut semis = keytrack_semis + rp.filter_env_semitones[i] * filt_env;
            if rp.lfo_dest == LfoDestination::Cutoff {
                semis += lfo * lfo_amount * 48.0;
            }
            let cutoff = (rp.filter_cutoff[i] * (semis / 12.0).exp2()).clamp(20.0, max_cutoff);
            let g = Svf::g(cutoff, self.sample_rate);
            let k = Svf::k(rp.filter_resonance[i]);

            // Gain-compensated tanh drive on the filter input.
            let drive = rp.filter_drive[i];
            let driven = (mixed * drive).tanh() / drive.tanh();
            let filtered = self.filter.lowpass(driven, g, k);

            // LFO -> amp: tremolo between full level and (1 - depth).
            let tremolo = if rp.lfo_dest == LfoDestination::Amp {
                1.0 - lfo_amount * (0.5 + 0.5 * lfo)
            } else {
                1.0
            };

            // Per-voice modulated gain if the host sent PolyModulation events,
            // otherwise the global master gain.
            let gain = match &mut self.gain_mod {
                Some(m) => {
                    m.current += self.gain_mod_coef * (m.target - m.current);
                    m.current
                }
                None => rp.master_gain[i],
            };

            *sample += filtered * env * self.velocity_gain * tremolo * gain;
        }
    }
}
