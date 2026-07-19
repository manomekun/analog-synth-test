use std::sync::Arc;

use nice_plug::prelude::*;

#[derive(Params)]
pub struct SynthParams {
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

impl Default for SynthParams {
    fn default() -> Self {
        Self {
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
