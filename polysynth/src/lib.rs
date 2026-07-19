use nice_plug::prelude::*;
use std::sync::Arc;

mod dsp;
mod params;
mod voice_manager;

use dsp::voice::RenderParams;
use params::SynthParams;
use voice_manager::{TerminatedVoice, VoiceManager};

/// Processing happens in sub-blocks of at most this many samples, split at
/// note-event boundaries.
const MAX_BLOCK_SIZE: usize = 64;

pub struct PolySynth {
    params: Arc<SynthParams>,
    voices: VoiceManager,
}

impl Default for PolySynth {
    fn default() -> Self {
        Self {
            params: Arc::new(SynthParams::default()),
            voices: VoiceManager::new(44_100.0),
        }
    }
}

impl Plugin for PolySynth {
    const NAME: &'static str = "PolySynth";
    const VENDOR: &'static str = "manomekun";
    const URL: &'static str = "https://github.com/manomekun/analog-synth-test";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.voices = VoiceManager::new(buffer_config.sample_rate);
        true
    }

    fn reset(&mut self) {
        self.voices.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let num_samples = buffer.samples();
        let output = buffer.as_slice();

        let mut next_event = context.next_event();
        let mut block_start = 0;

        while block_start < num_samples {
            let mut block_end = (block_start + MAX_BLOCK_SIZE).min(num_samples);

            // Apply all events that fall on `block_start`, and shorten the
            // block so it ends at the next upcoming event.
            while let Some(event) = next_event {
                let timing = event.timing() as usize;
                if timing > block_start {
                    block_end = block_end.min(timing);
                    break;
                }

                self.handle_note_event(&event, context);
                next_event = context.next_event();
            }

            let block_len = block_end - block_start;
            let mut mono = [0.0f32; MAX_BLOCK_SIZE];
            let mono = &mut mono[..block_len];

            // Consume each smoothed param exactly once per sub-block; all
            // voices then read the same value arrays.
            let mut osc1_pw = [0.0f32; MAX_BLOCK_SIZE];
            let mut osc2_pw = [0.0f32; MAX_BLOCK_SIZE];
            let mut osc2_detune = [0.0f32; MAX_BLOCK_SIZE];
            let mut osc1_level = [0.0f32; MAX_BLOCK_SIZE];
            let mut osc2_level = [0.0f32; MAX_BLOCK_SIZE];
            let mut noise_level = [0.0f32; MAX_BLOCK_SIZE];
            let mut filter_cutoff = [0.0f32; MAX_BLOCK_SIZE];
            let mut filter_resonance = [0.0f32; MAX_BLOCK_SIZE];
            let mut filter_env = [0.0f32; MAX_BLOCK_SIZE];
            let mut filter_drive = [0.0f32; MAX_BLOCK_SIZE];
            let params = &self.params;
            params.osc1_pulse_width.smoothed.next_block(&mut osc1_pw, block_len);
            params.osc2_pulse_width.smoothed.next_block(&mut osc2_pw, block_len);
            params.osc2_detune.smoothed.next_block(&mut osc2_detune, block_len);
            params.osc1_level.smoothed.next_block(&mut osc1_level, block_len);
            params.osc2_level.smoothed.next_block(&mut osc2_level, block_len);
            params.noise_level.smoothed.next_block(&mut noise_level, block_len);
            params.filter_cutoff.smoothed.next_block(&mut filter_cutoff, block_len);
            params.filter_resonance.smoothed.next_block(&mut filter_resonance, block_len);
            params.filter_env_amount.smoothed.next_block(&mut filter_env, block_len);
            params.filter_drive.smoothed.next_block(&mut filter_drive, block_len);

            let render_params = RenderParams {
                osc1_wave: params.osc1_wave.value(),
                osc2_wave: params.osc2_wave.value(),
                osc2_octave_mult: (params.osc2_octave.value() as f32).exp2(),
                osc1_pw: &osc1_pw[..block_len],
                osc2_pw: &osc2_pw[..block_len],
                osc2_detune_cents: &osc2_detune[..block_len],
                osc1_level: &osc1_level[..block_len],
                osc2_level: &osc2_level[..block_len],
                noise_level: &noise_level[..block_len],
                filter_cutoff: &filter_cutoff[..block_len],
                filter_resonance: &filter_resonance[..block_len],
                filter_env_semitones: &filter_env[..block_len],
                filter_drive: &filter_drive[..block_len],
                filter_keytrack: params.filter_keytrack.value(),
            };

            self.voices.render(mono, &render_params, |terminated| {
                send_voice_terminated(context, terminated, block_end - 1);
            });

            let mut gain = [0.0f32; MAX_BLOCK_SIZE];
            let gain = &mut gain[..block_len];
            self.params.gain.smoothed.next_block(gain, block_len);

            for (i, sample) in mono.iter().enumerate() {
                let out = sample * gain[i];
                output[0][block_start + i] = out;
                output[1][block_start + i] = out;
            }

            block_start = block_end;
        }

        ProcessStatus::Normal
    }
}

impl PolySynth {
    fn handle_note_event(
        &mut self,
        event: &PluginNoteEvent<Self>,
        context: &mut impl ProcessContext<Self>,
    ) {
        match event {
            NoteEvent::NoteOn {
                timing,
                voice_id,
                channel,
                note,
                velocity,
            } => {
                if let Some(stolen) =
                    self.voices
                        .note_on(*voice_id, *channel, *note, *velocity, &self.params)
                {
                    send_voice_terminated(context, stolen, *timing as usize);
                }
            }
            NoteEvent::NoteOff {
                voice_id,
                channel,
                note,
                ..
            } => self.voices.note_off(*voice_id, *channel, *note),
            NoteEvent::Choke {
                voice_id,
                channel,
                note,
                ..
            } => self.voices.choke(*voice_id, *channel, *note),
            _ => {}
        }
    }
}

fn send_voice_terminated<P: Plugin>(
    context: &mut impl ProcessContext<P>,
    voice: TerminatedVoice,
    timing: usize,
) {
    context.send_event(NoteEvent::VoiceTerminated {
        timing: timing as u32,
        voice_id: voice.voice_id,
        channel: voice.channel,
        note: voice.note,
    });
}

impl ClapPlugin for PolySynth {
    const CLAP_ID: &'static str = "org.manomekun.polysynth";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("A polyphonic virtual-analog subtractive synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

nice_export_clap!(PolySynth);
