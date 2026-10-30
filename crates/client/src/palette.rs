//! Colours. Clean and light (decision 20): a dark field, quiet lanes, and
//! one strong hue per hidden power, drawn from the faction identities in the
//! design doc (Mafia oxblood, Church violet, Unions hazard orange).

use bevy::prelude::*;
use grandspace_sim::Power;

pub const BACKGROUND: Color = Color::srgb(0.035, 0.04, 0.06);
pub const PANEL: Color = Color::srgba(0.07, 0.08, 0.11, 0.94);
pub const PANEL_EDGE: Color = Color::srgb(0.18, 0.2, 0.26);
pub const TEXT: Color = Color::srgb(0.88, 0.89, 0.92);
pub const TEXT_DIM: Color = Color::srgb(0.55, 0.58, 0.65);
pub const LANE: Color = Color::srgba(0.45, 0.5, 0.6, 0.28);
pub const STAR: Color = Color::srgb(0.92, 0.93, 0.96);
pub const CONTESTED: Color = Color::srgb(0.6, 0.62, 0.68);
pub const SELECTED: Color = Color::srgb(1.0, 1.0, 1.0);
pub const BUTTON: Color = Color::srgb(0.13, 0.15, 0.2);
pub const BUTTON_HOVER: Color = Color::srgb(0.2, 0.23, 0.3);
pub const BUTTON_ACTIVE: Color = Color::srgb(0.26, 0.32, 0.45);
pub const WARNING: Color = Color::srgb(0.98, 0.72, 0.25);
pub const DANGER: Color = Color::srgb(0.95, 0.35, 0.35);

pub fn power(p: Power) -> Color {
    match p {
        Power::Mafia => Color::srgb(0.86, 0.27, 0.33),
        Power::Church => Color::srgb(0.66, 0.5, 0.98),
        Power::Unions => Color::srgb(0.98, 0.62, 0.18),
    }
}

/// Factions of the same power get lighter shades of its hue.
pub fn faction(p: Power, id: u32) -> Color {
    let t = ((id / 3) as f32 * 0.18).min(0.5);
    let c = power(p).to_srgba();
    let lift = |v: f32| v + (1.0 - v) * t;
    Color::srgb(lift(c.red), lift(c.green), lift(c.blue))
}
