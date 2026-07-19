//! egui editor: a fixed panel of sections mirroring the signal flow.

use std::sync::Arc;

use egui::{self, Frame, Layout, Margin, RichText, Ui};
use nice_plug::prelude::*;
use nice_plug_egui::widgets::ParamSlider;
use nice_plug_egui::{create_egui_editor, EguiSettings, EguiState};

use crate::params::SynthParams;

mod knob;
mod theme;

use knob::Knob;

pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(900, 400)
}

pub fn create(params: Arc<SynthParams>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        (),
        EguiSettings::default(),
        |ctx, _queue, _state| theme::apply(ctx),
        move |ui, setter, _queue, _state| draw(ui, setter, &params),
    )
}

fn draw(ui: &mut Ui, setter: &ParamSetter, params: &SynthParams) {
    ui.vertical(|ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new("PolySynth")
                    .size(18.0)
                    .strong()
                    .color(theme::ACCENT),
            );
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("virtual analog")
                        .size(11.0)
                        .color(theme::TEXT_DIM),
                );
            });
        });
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            section(ui, "OSC 1", |ui| {
                ui.add(ParamSlider::for_param(&params.osc1_wave, setter).with_width(110.0));
                ui.horizontal(|ui| {
                    ui.add(Knob::for_param(&params.osc1_pulse_width, setter));
                });
            });
            section(ui, "OSC 2", |ui| {
                ui.add(ParamSlider::for_param(&params.osc2_wave, setter).with_width(110.0));
                ui.horizontal(|ui| {
                    ui.add(Knob::for_param(&params.osc2_octave, setter));
                    ui.add(Knob::for_param(&params.osc2_detune, setter));
                    ui.add(Knob::for_param(&params.osc2_pulse_width, setter));
                });
            });
            section(ui, "MIXER", |ui| {
                ui.horizontal(|ui| {
                    ui.add(Knob::for_param(&params.osc1_level, setter));
                    ui.add(Knob::for_param(&params.osc2_level, setter));
                    ui.add(Knob::for_param(&params.noise_level, setter));
                });
            });
            section(ui, "FILTER", |ui| {
                ui.horizontal(|ui| {
                    ui.add(Knob::for_param(&params.filter_cutoff, setter));
                    ui.add(Knob::for_param(&params.filter_resonance, setter));
                    ui.add(Knob::for_param(&params.filter_drive, setter));
                });
                ui.horizontal(|ui| {
                    ui.add(Knob::for_param(&params.filter_env_amount, setter));
                    ui.add(Knob::for_param(&params.filter_keytrack, setter));
                });
            });
        });

        ui.add_space(2.0);

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            section(ui, "AMP ENV", |ui| {
                ui.horizontal(|ui| {
                    ui.add(Knob::for_param(&params.amp_attack, setter));
                    ui.add(Knob::for_param(&params.amp_decay, setter));
                    ui.add(Knob::for_param(&params.amp_sustain, setter));
                    ui.add(Knob::for_param(&params.amp_release, setter));
                });
            });
            section(ui, "FILTER ENV", |ui| {
                ui.horizontal(|ui| {
                    ui.add(Knob::for_param(&params.filt_attack, setter));
                    ui.add(Knob::for_param(&params.filt_decay, setter));
                    ui.add(Knob::for_param(&params.filt_sustain, setter));
                    ui.add(Knob::for_param(&params.filt_release, setter));
                });
            });
            section(ui, "LFO", |ui| {
                ui.add(ParamSlider::for_param(&params.lfo_wave, setter).with_width(110.0));
                ui.add(ParamSlider::for_param(&params.lfo_dest, setter).with_width(110.0));
                ui.horizontal(|ui| {
                    ui.add(Knob::for_param(&params.lfo_rate, setter));
                    ui.add(Knob::for_param(&params.lfo_amount, setter));
                });
            });
            section(ui, "MASTER", |ui| {
                ui.horizontal(|ui| {
                    ui.add(Knob::for_param(&params.gain, setter));
                    ui.add(Knob::for_param(&params.polyphony, setter));
                    ui.add(Knob::for_param(&params.velocity_sens, setter));
                });
            });
        });
    });
}

fn section(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(theme::SECTION_BG)
        .corner_radius(6.0)
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(title)
                        .size(11.0)
                        .strong()
                        .color(theme::TEXT_DIM),
                );
                ui.add_space(4.0);
                add_contents(ui);
            });
        });
}
