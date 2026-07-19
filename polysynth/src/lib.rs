use nice_plug::prelude::*;
use std::sync::Arc;

mod params;

use params::SynthParams;

pub struct PolySynth {
    params: Arc<SynthParams>,
}

impl Default for PolySynth {
    fn default() -> Self {
        Self {
            params: Arc::new(SynthParams::default()),
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
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        true
    }

    fn reset(&mut self) {}

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        for channel_samples in buffer.iter_samples() {
            let gain = self.params.gain.smoothed.next();
            for sample in channel_samples {
                *sample = 0.0 * gain;
            }
        }

        ProcessStatus::Normal
    }
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
