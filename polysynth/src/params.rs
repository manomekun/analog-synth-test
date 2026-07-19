use std::sync::Arc;

use nice_plug::prelude::*;

use crate::dsp::oscillator::Waveform;

#[derive(Params)]
pub struct SynthParams {
    // --- Oscillator 1 ---
    #[id = "o1_wave"]
    pub osc1_wave: EnumParam<Waveform>,
    #[id = "o1_pw"]
    pub osc1_pulse_width: FloatParam,

    // --- Oscillator 2 ---
    #[id = "o2_wave"]
    pub osc2_wave: EnumParam<Waveform>,
    #[id = "o2_oct"]
    pub osc2_octave: IntParam,
    #[id = "o2_det"]
    pub osc2_detune: FloatParam,
    #[id = "o2_pw"]
    pub osc2_pulse_width: FloatParam,

    // --- Mixer ---
    #[id = "o1_lvl"]
    pub osc1_level: FloatParam,
    #[id = "o2_lvl"]
    pub osc2_level: FloatParam,
    #[id = "nz_lvl"]
    pub noise_level: FloatParam,

    // --- Filter ---
    #[id = "flt_cut"]
    pub filter_cutoff: FloatParam,
    #[id = "flt_res"]
    pub filter_resonance: FloatParam,
    #[id = "flt_env"]
    pub filter_env_amount: FloatParam,
    #[id = "flt_key"]
    pub filter_keytrack: FloatParam,
    #[id = "flt_drv"]
    pub filter_drive: FloatParam,

    // --- Filter envelope ---
    #[id = "flt_att"]
    pub filt_attack: FloatParam,
    #[id = "flt_dec"]
    pub filt_decay: FloatParam,
    #[id = "flt_sus"]
    pub filt_sustain: FloatParam,
    #[id = "flt_rel"]
    pub filt_release: FloatParam,

    // --- Amp envelope ---
    #[id = "amp_att"]
    pub amp_attack: FloatParam,
    #[id = "amp_dec"]
    pub amp_decay: FloatParam,
    #[id = "amp_sus"]
    pub amp_sustain: FloatParam,
    #[id = "amp_rel"]
    pub amp_release: FloatParam,

    // --- Master ---
    /// Master output gain, stored as a linear factor but displayed in dB.
    #[id = "gain"]
    pub gain: FloatParam,
}

/// Envelope time in seconds: 1 ms .. 10 s with a logarithmic-feeling skew.
fn envelope_time_param(name: &str, default_s: f32) -> FloatParam {
    FloatParam::new(
        name,
        default_s,
        FloatRange::Skewed {
            min: 0.001,
            max: 10.0,
            factor: FloatRange::skew_factor(-2.0),
        },
    )
    .with_value_to_string(v2s_f32_ms_then_s())
    .with_string_to_value(s2v_f32_ms_then_s())
}

/// Display sub-second values as milliseconds and longer values as seconds.
fn v2s_f32_ms_then_s() -> Arc<dyn Fn(f32) -> String + Send + Sync> {
    Arc::new(|value| {
        if value < 1.0 {
            format!("{:.1} ms", value * 1000.0)
        } else {
            format!("{value:.2} s")
        }
    })
}

fn s2v_f32_ms_then_s() -> Arc<dyn Fn(&str) -> Option<f32> + Send + Sync> {
    Arc::new(|string| {
        let string = string.trim();
        let (number_part, scale) = if let Some(stripped) =
            string.strip_suffix("ms").or_else(|| string.strip_suffix("mS"))
        {
            (stripped, 0.001)
        } else if let Some(stripped) = string.strip_suffix(['s', 'S']) {
            (stripped, 1.0)
        } else {
            (string, 1.0)
        };

        number_part.trim().parse::<f32>().ok().map(|v| v * scale)
    })
}

/// Pulse width: 1% .. 99%, only audible when the waveform is Pulse.
fn pulse_width_param(name: &str) -> FloatParam {
    FloatParam::new(
        name,
        0.5,
        FloatRange::Linear {
            min: 0.01,
            max: 0.99,
        },
    )
    .with_smoother(SmoothingStyle::Linear(10.0))
    .with_value_to_string(formatters::v2s_f32_percentage(0))
    .with_string_to_value(formatters::s2v_f32_percentage())
    .with_unit("%")
}

/// Mixer level: plain linear 0 .. 1.
fn level_param(name: &str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_smoother(SmoothingStyle::Linear(10.0))
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
        .with_unit("%")
}

impl Default for SynthParams {
    fn default() -> Self {
        Self {
            osc1_wave: EnumParam::new("Osc 1 Wave", Waveform::Saw),
            osc1_pulse_width: pulse_width_param("Osc 1 Pulse Width"),

            osc2_wave: EnumParam::new("Osc 2 Wave", Waveform::Saw),
            osc2_octave: IntParam::new("Osc 2 Octave", 0, IntRange::Linear { min: -2, max: 2 }),
            osc2_detune: FloatParam::new(
                "Osc 2 Detune",
                7.0,
                FloatRange::Linear {
                    min: -100.0,
                    max: 100.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit(" ct")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            osc2_pulse_width: pulse_width_param("Osc 2 Pulse Width"),

            osc1_level: level_param("Osc 1 Level", 1.0),
            osc2_level: level_param("Osc 2 Level", 0.5),
            noise_level: level_param("Noise Level", 0.0),

            filter_cutoff: FloatParam::new(
                "Filter Cutoff",
                20_000.0,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 20_000.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            filter_resonance: FloatParam::new(
                "Filter Resonance",
                0.1,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage())
            .with_unit("%"),
            filter_env_amount: FloatParam::new(
                "Filter Env Amount",
                24.0,
                FloatRange::Linear {
                    min: -96.0,
                    max: 96.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit(" st")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            filter_keytrack: FloatParam::new(
                "Filter Keytrack",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage())
            .with_unit("%"),
            filter_drive: FloatParam::new(
                "Filter Drive",
                1.0,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 10.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            filt_attack: envelope_time_param("Filter Attack", 0.001),
            filt_decay: envelope_time_param("Filter Decay", 0.3),
            filt_sustain: FloatParam::new(
                "Filter Sustain",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage())
            .with_unit("%"),
            filt_release: envelope_time_param("Filter Release", 0.2),

            amp_attack: envelope_time_param("Amp Attack", 0.005),
            amp_decay: envelope_time_param("Amp Decay", 0.2),
            amp_sustain: FloatParam::new(
                "Amp Sustain",
                0.8,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage())
            .with_unit("%"),
            amp_release: envelope_time_param("Amp Release", 0.1),

            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(-12.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-60.0),
                    max: util::db_to_gain(0.0),
                    factor: FloatRange::gain_skew_factor(-60.0, 0.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
        }
    }
}
