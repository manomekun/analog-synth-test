//! A rotary knob widget bound to a nice-plug parameter through `ParamSetter`,
//! so host automation gestures (begin/change/end) are reported correctly.

use nice_plug::prelude::{Param, ParamSetter};
use egui::{
    self, Align2, FontId, Response, Sense, Stroke, Ui, Vec2, Widget, epaint,
};

use super::theme;

/// 270-degree sweep starting at the lower left.
const START_ANGLE: f32 = 0.75 * std::f32::consts::PI;
const SWEEP: f32 = 1.5 * std::f32::consts::PI;
const DRAG_SENSITIVITY: f32 = 0.005;
const FINE_DRAG_MULTIPLIER: f32 = 0.1;

pub struct Knob<'a, P: Param> {
    param: &'a P,
    setter: &'a ParamSetter<'a>,
    diameter: f32,
}

impl<'a, P: Param> Knob<'a, P> {
    pub fn for_param(param: &'a P, setter: &'a ParamSetter<'a>) -> Self {
        Self {
            param,
            setter,
            diameter: 36.0,
        }
    }
}

impl<P: Param> Widget for Knob<'_, P> {
    fn ui(self, ui: &mut Ui) -> Response {
        let label_height = 26.0;
        let width = self.diameter.max(52.0);
        let desired_size = Vec2::new(width, self.diameter + label_height);
        let (rect, mut response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        // --- Interaction ---
        if response.drag_started() {
            self.setter.begin_set_parameter(self.param);
        }
        if response.dragged() {
            let step = -response.drag_delta().y * DRAG_SENSITIVITY;
            let step = if ui.input(|i| i.modifiers.shift) {
                step * FINE_DRAG_MULTIPLIER
            } else {
                step
            };
            if step != 0.0 {
                let new_value = (self.param.unmodulated_normalized_value() + step).clamp(0.0, 1.0);
                self.setter
                    .set_parameter_normalized(self.param, new_value);
                response.mark_changed();
            }
        }
        if response.drag_stopped() {
            self.setter.end_set_parameter(self.param);
        }
        if response.double_clicked() {
            self.setter.begin_set_parameter(self.param);
            self.setter
                .set_parameter_normalized(self.param, self.param.default_normalized_value());
            self.setter.end_set_parameter(self.param);
            response.mark_changed();
        }

        // --- Painting ---
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let radius = self.diameter / 2.0;
            let center = egui::pos2(rect.center().x, rect.top() + radius + 2.0);
            let value = self.param.unmodulated_normalized_value();

            painter.circle_filled(center, radius, theme::KNOB_BODY);
            paint_arc(painter, center, radius - 1.5, 0.0, 1.0, theme::KNOB_TRACK, 3.0);
            paint_arc(painter, center, radius - 1.5, 0.0, value, theme::ACCENT, 3.0);

            // Pointer line
            let angle = START_ANGLE + SWEEP * value;
            let dir = Vec2::angled(angle);
            painter.line_segment(
                [center + dir * (radius * 0.35), center + dir * (radius * 0.85)],
                Stroke::new(2.0, theme::TEXT),
            );

            // Name, and value while interacting/hovering
            let show_value = response.hovered() || response.dragged();
            let label = if show_value {
                self.param.to_string()
            } else {
                self.param.name().to_string()
            };
            painter.text(
                egui::pos2(rect.center().x, rect.bottom() - 10.0),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(10.5),
                if show_value { theme::TEXT } else { theme::TEXT_DIM },
            );
        }

        response
    }
}

/// Stroke a partial circle between two normalized positions along the sweep.
fn paint_arc(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    from: f32,
    to: f32,
    color: egui::Color32,
    width: f32,
) {
    let steps = 48;
    let points: Vec<egui::Pos2> = (0..=steps)
        .map(|i| {
            let t = from + (to - from) * i as f32 / steps as f32;
            let angle = START_ANGLE + SWEEP * t;
            center + Vec2::angled(angle) * radius
        })
        .collect();
    painter.add(epaint::PathShape::line(points, Stroke::new(width, color)));
}
