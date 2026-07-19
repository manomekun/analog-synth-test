//! Colors and style for the editor.

use egui::{Color32, Context, CornerRadius, Visuals};

pub const BACKGROUND: Color32 = Color32::from_rgb(0x1a, 0x1c, 0x22);
pub const SECTION_BG: Color32 = Color32::from_rgb(0x23, 0x26, 0x2e);
pub const ACCENT: Color32 = Color32::from_rgb(0xe8, 0xa3, 0x3d);
pub const KNOB_TRACK: Color32 = Color32::from_rgb(0x3a, 0x3e, 0x48);
pub const KNOB_BODY: Color32 = Color32::from_rgb(0x2b, 0x2e, 0x36);
pub const TEXT: Color32 = Color32::from_rgb(0xd8, 0xda, 0xde);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x8a, 0x8e, 0x98);

pub fn apply(ctx: &Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BACKGROUND;
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);
    visuals.selection.bg_fill = ACCENT.linear_multiply(0.4);
    ctx.set_visuals(visuals);
}
