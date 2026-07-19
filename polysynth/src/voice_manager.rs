//! Fixed-pool polyphonic voice management with voice stealing.

use crate::dsp::voice::Voice;
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
        params: &SynthParams,
        mut on_terminated: impl FnMut(TerminatedVoice),
    ) {
        for slot in self.voices.iter_mut() {
            if let Some(voice) = slot {
                voice.render(output, params);
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
}

fn voice_matches(voice: &Voice, voice_id: Option<i32>, channel: u8, note: u8) -> bool {
    match voice_id {
        Some(id) => voice.voice_id == Some(id),
        None => voice.channel == channel && voice.note == note,
    }
}
