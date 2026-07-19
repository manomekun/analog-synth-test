//! Fixed-pool polyphonic voice management with voice stealing.

use crate::dsp::voice::{RenderParams, Voice};
use crate::params::SynthParams;

pub const MAX_VOICES: usize = 16;

/// A voice that ended this block and whose ID must be reported to the host via
/// a `VoiceTerminated` event.
#[derive(Debug, Clone, Copy)]
pub struct TerminatedVoice {
    pub voice_id: Option<i32>,
    pub channel: u8,
    pub note: u8,
}

pub struct VoiceManager {
    sample_rate: f32,
    voices: [Option<Voice>; MAX_VOICES],
    /// Counter for age-based voice stealing.
    next_internal_id: u64,
}

impl VoiceManager {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            voices: [const { None }; MAX_VOICES],
            next_internal_id: 0,
        }
    }

    pub fn reset(&mut self) {
        self.voices = [const { None }; MAX_VOICES];
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.is_some()).count()
    }

    pub fn note_on(
        &mut self,
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        velocity: f32,
        params: &SynthParams,
    ) -> Option<TerminatedVoice> {
        let internal_id = self.next_internal_id;
        self.next_internal_id += 1;

        let voice = Voice::new(
            self.sample_rate,
            voice_id,
            channel,
            note,
            velocity,
            internal_id,
            params,
        );

        // Free slot available: no stealing needed.
        if let Some(slot) = self.voices.iter_mut().find(|v| v.is_none()) {
            *slot = Some(voice);
            return None;
        }

        // Steal: prefer the oldest releasing voice, otherwise the oldest voice.
        let steal_idx = self
            .voice_indices()
            .filter(|&i| self.voices[i].as_ref().unwrap().is_releasing())
            .min_by_key(|&i| self.voices[i].as_ref().unwrap().internal_id)
            .or_else(|| {
                self.voice_indices()
                    .min_by_key(|&i| self.voices[i].as_ref().unwrap().internal_id)
            })
            .expect("pool is full, so a steal candidate must exist");

        let stolen = self.voices[steal_idx].as_ref().unwrap();
        let terminated = TerminatedVoice {
            voice_id: stolen.voice_id,
            channel: stolen.channel,
            note: stolen.note,
        };
        // v1 simplification: the stolen voice is replaced immediately instead of
        // being cross-faded out; the envelope's fast-release path is only used
        // for host `Choke` events.
        self.voices[steal_idx] = Some(voice);

        Some(terminated)
    }

    /// Release matching voices. CLAP hosts may address a specific voice by ID;
    /// otherwise all voices matching (channel, note) are released.
    pub fn note_off(&mut self, voice_id: Option<i32>, channel: u8, note: u8) {
        for voice in self.voices.iter_mut().flatten() {
            if voice_matches(voice, voice_id, channel, note) {
                voice.note_off();
            }
        }
    }

    /// Immediately choke matching voices (fast anti-click fade).
    pub fn choke(&mut self, voice_id: Option<i32>, channel: u8, note: u8) {
        for voice in self.voices.iter_mut().flatten() {
            if voice_matches(voice, voice_id, channel, note) {
                voice.fast_release();
            }
        }
    }

    /// Render all active voices additively into a mono buffer, then drop
    /// finished voices. Returns finished voices through the callback so the
    /// caller can emit `VoiceTerminated` events.
    pub fn render(
        &mut self,
        output: &mut [f32],
        rp: &RenderParams,
        mut on_terminated: impl FnMut(TerminatedVoice),
    ) {
        for slot in self.voices.iter_mut() {
            if let Some(voice) = slot {
                voice.render(output, rp);
                if voice.is_finished() {
                    on_terminated(TerminatedVoice {
                        voice_id: voice.voice_id,
                        channel: voice.channel,
                        note: voice.note,
                    });
                    *slot = None;
                }
            }
        }
    }

    fn voice_indices(&self) -> impl Iterator<Item = usize> + '_ {
        (0..MAX_VOICES).filter(|&i| self.voices[i].is_some())
    }

    pub fn voice_by_id_mut(&mut self, voice_id: i32) -> Option<&mut Voice> {
        self.voices
            .iter_mut()
            .flatten()
            .find(|v| v.voice_id == Some(voice_id))
    }

    pub fn voices_mut(&mut self) -> impl Iterator<Item = &mut Voice> {
        self.voices.iter_mut().flatten()
    }
}

fn voice_matches(voice: &Voice, voice_id: Option<i32>, channel: u8, note: u8) -> bool {
    match voice_id {
        Some(id) => voice.voice_id == Some(id),
        None => voice.channel == channel && voice.note == note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::oscillator::Waveform;

    const CUTOFFS: [f32; 64] = [20_000.0; 64];
    const DRIVES: [f32; 64] = [1.0; 64];
    const UNITY: [f32; 64] = [1.0; 64];

    fn render_params<'a>(levels: &'a [f32; 64], zeros: &'a [f32; 64]) -> RenderParams<'a> {
        RenderParams {
            osc1_wave: Waveform::Saw,
            osc2_wave: Waveform::Saw,
            osc2_octave_mult: 1.0,
            osc1_pw: &levels[..],
            osc2_pw: &levels[..],
            osc2_detune_cents: &zeros[..],
            osc1_level: &levels[..],
            osc2_level: &zeros[..],
            noise_level: &zeros[..],
            filter_cutoff: &CUTOFFS[..],
            filter_resonance: &zeros[..],
            filter_env_semitones: &zeros[..],
            filter_drive: &DRIVES[..],
            filter_keytrack: 0.0,
            lfo: &zeros[..],
            lfo_amount: &zeros[..],
            lfo_dest: crate::dsp::lfo::LfoDestination::None,
            master_gain: &UNITY[..],
        }
    }

    #[test]
    fn steals_oldest_voice_when_pool_is_full() {
        let params = SynthParams::default();
        let mut vm = VoiceManager::new(44_100.0);

        for note in 0..MAX_VOICES as u8 {
            assert!(vm.note_on(Some(note as i32), 0, 60 + note, 0.8, &params).is_none());
        }
        assert_eq!(vm.active_voices(), MAX_VOICES);

        // The 17th note steals the oldest voice (the first note-on).
        let stolen = vm.note_on(Some(100), 0, 40, 0.8, &params);
        assert_eq!(vm.active_voices(), MAX_VOICES);
        let stolen = stolen.expect("a voice must have been stolen");
        assert_eq!(stolen.voice_id, Some(0));
        assert_eq!(stolen.note, 60);
    }

    #[test]
    fn prefers_stealing_releasing_voices() {
        let params = SynthParams::default();
        let mut vm = VoiceManager::new(44_100.0);

        for note in 0..MAX_VOICES as u8 {
            vm.note_on(Some(note as i32), 0, 60 + note, 0.8, &params);
        }
        // Voice 5 is releasing; despite not being the oldest it gets stolen.
        vm.note_off(Some(5), 0, 65);
        let stolen = vm.note_on(Some(100), 0, 40, 0.8, &params).unwrap();
        assert_eq!(stolen.voice_id, Some(5));
    }

    #[test]
    fn note_off_matches_by_channel_and_note_without_voice_id() {
        let params = SynthParams::default();
        let mut vm = VoiceManager::new(44_100.0);
        let levels = [0.5f32; 64];
        let zeros = [0.0f32; 64];

        vm.note_on(None, 0, 60, 0.8, &params);
        vm.note_on(None, 1, 60, 0.8, &params);
        vm.note_off(None, 0, 60);

        // Render until the released voice's envelope finishes; only the voice
        // on channel 0 must terminate.
        let mut terminated = Vec::new();
        for _ in 0..1000 {
            let mut buf = [0.0f32; 64];
            let rp = render_params(&levels, &zeros);
            vm.render(&mut buf, &rp, |t| terminated.push(t));
        }
        assert_eq!(terminated.len(), 1);
        assert_eq!(terminated[0].channel, 0);
        assert_eq!(vm.active_voices(), 1);
    }

    #[test]
    fn finished_voices_are_removed_and_reported_once() {
        let params = SynthParams::default();
        let mut vm = VoiceManager::new(44_100.0);
        let levels = [0.5f32; 64];
        let zeros = [0.0f32; 64];

        vm.note_on(Some(1), 0, 60, 0.8, &params);
        vm.note_off(Some(1), 0, 60);

        let mut terminated = 0;
        for _ in 0..1000 {
            let mut buf = [0.0f32; 64];
            let rp = render_params(&levels, &zeros);
            vm.render(&mut buf, &rp, |_| terminated += 1);
        }
        assert_eq!(terminated, 1);
        assert_eq!(vm.active_voices(), 0);
    }

    #[test]
    fn rendered_audio_is_nonzero_while_held() {
        let params = SynthParams::default();
        let mut vm = VoiceManager::new(44_100.0);
        let levels = [0.8f32; 64];
        let zeros = [0.0f32; 64];

        vm.note_on(Some(1), 0, 69, 1.0, &params);
        let mut energy = 0.0f32;
        for _ in 0..100 {
            let mut buf = [0.0f32; 64];
            let rp = render_params(&levels, &zeros);
            vm.render(&mut buf, &rp, |_| {});
            energy += buf.iter().map(|x| x * x).sum::<f32>();
        }
        assert!(energy > 1.0, "held voice should produce audio, got {energy}");
    }
}
