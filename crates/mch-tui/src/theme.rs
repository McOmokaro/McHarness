//! Dark palette: black + charcoal panels with neon-green and orange accents.

use ratatui::style::Color;

/// Near-black canvas.
pub const BG: Color = Color::Rgb(8, 9, 12);
/// Charcoal panel fill.
pub const PANEL: Color = Color::Rgb(20, 22, 28);
/// Subtle panel border.
pub const BORDER: Color = Color::Rgb(46, 49, 60);
/// Focused/active border.
pub const BORDER_ACTIVE: Color = Color::Rgb(90, 96, 116);
/// Neon green accent (primary).
pub const NEON: Color = Color::Rgb(57, 255, 20);
/// Orange accent (secondary / warnings-in-progress).
pub const ORANGE: Color = Color::Rgb(255, 140, 0);
/// Primary text.
pub const TEXT: Color = Color::Rgb(222, 224, 232);
/// Muted text.
pub const MUTED: Color = Color::Rgb(120, 126, 142);
/// Danger.
pub const RED: Color = Color::Rgb(255, 78, 78);
/// Caution.
pub const YELLOW: Color = Color::Rgb(255, 205, 70);

/// Rate-limit / health indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Green,
    Yellow,
    Red,
}

impl Health {
    /// The palette color for this health.
    pub fn color(self) -> Color {
        match self {
            Health::Green => NEON,
            Health::Yellow => YELLOW,
            Health::Red => RED,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Health::Green => "OK",
            Health::Yellow => "WARN",
            Health::Red => "DOWN",
        }
    }
}
