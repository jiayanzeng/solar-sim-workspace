//! WP7 theme tokens — Rev C §§8.4 and 9.
//!
//! Values live in a resource so every later HUD package consumes the same
//! palette, spacing, typography, and wide-tracking decisions. The snapshot
//! test intentionally makes visual-token drift an explicit review event.

use bevy::prelude::{Color, Resource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiColorToken(pub [u8; 4]);

impl UiColorToken {
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self([red, green, blue, 255])
    }

    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self([red, green, blue, alpha])
    }

    pub const fn color(self) -> Color {
        Color::srgba_u8(self.0[0], self.0[1], self.0[2], self.0[3])
    }

    fn snapshot(self) -> String {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiColors {
    pub background: UiColorToken,
    pub top_bar: UiColorToken,
    pub panel: UiColorToken,
    pub panel_elevated: UiColorToken,
    pub separator: UiColorToken,
    pub accent: UiColorToken,
    pub status_live: UiColorToken,
    pub text_primary: UiColorToken,
    pub text_muted: UiColorToken,
    pub text_disabled: UiColorToken,
    pub scrim: UiColorToken,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiSpacing {
    pub hairline_px: f32,
    pub radius_px: f32,
    pub xs_px: f32,
    pub sm_px: f32,
    pub md_px: f32,
    pub lg_px: f32,
    pub xl_px: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiTypeScale {
    pub caption_px: f32,
    pub label_px: f32,
    pub body_px: f32,
    pub title_px: f32,
    pub product_px: f32,
    pub uppercase_tracking_px: f32,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct UiTheme {
    pub colors: UiColors,
    pub spacing: UiSpacing,
    pub type_scale: UiTypeScale,
}

impl UiTheme {
    pub const DARK: Self = Self {
        colors: UiColors {
            background: UiColorToken::rgb(7, 10, 15),
            top_bar: UiColorToken::rgba(10, 15, 23, 246),
            panel: UiColorToken::rgba(14, 21, 31, 244),
            panel_elevated: UiColorToken::rgba(20, 29, 41, 250),
            separator: UiColorToken::rgba(63, 78, 96, 180),
            accent: UiColorToken::rgb(76, 211, 255),
            status_live: UiColorToken::rgb(86, 211, 139),
            text_primary: UiColorToken::rgb(231, 240, 249),
            text_muted: UiColorToken::rgb(132, 149, 168),
            text_disabled: UiColorToken::rgb(77, 90, 105),
            scrim: UiColorToken::rgba(3, 6, 10, 218),
        },
        spacing: UiSpacing {
            hairline_px: 1.0,
            radius_px: 6.0,
            xs_px: 4.0,
            sm_px: 8.0,
            md_px: 12.0,
            lg_px: 16.0,
            xl_px: 24.0,
        },
        type_scale: UiTypeScale {
            caption_px: 11.0,
            label_px: 12.0,
            body_px: 13.0,
            title_px: 16.0,
            product_px: 18.0,
            uppercase_tracking_px: 1.8,
        },
    };

    pub fn snapshot(self) -> String {
        format!(
            concat!(
                "background={};top_bar={};panel={};panel_elevated={};",
                "separator={};accent={};status_live={};text_primary={};text_muted={};",
                "text_disabled={};scrim={};spacing={:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1};",
                "type={:.1},{:.1},{:.1},{:.1},{:.1};tracking={:.1}"
            ),
            self.colors.background.snapshot(),
            self.colors.top_bar.snapshot(),
            self.colors.panel.snapshot(),
            self.colors.panel_elevated.snapshot(),
            self.colors.separator.snapshot(),
            self.colors.accent.snapshot(),
            self.colors.status_live.snapshot(),
            self.colors.text_primary.snapshot(),
            self.colors.text_muted.snapshot(),
            self.colors.text_disabled.snapshot(),
            self.colors.scrim.snapshot(),
            self.spacing.hairline_px,
            self.spacing.radius_px,
            self.spacing.xs_px,
            self.spacing.sm_px,
            self.spacing.md_px,
            self.spacing.lg_px,
            self.spacing.xl_px,
            self.type_scale.caption_px,
            self.type_scale.label_px,
            self.type_scale.body_px,
            self.type_scale.title_px,
            self.type_scale.product_px,
            self.type_scale.uppercase_tracking_px,
        )
    }
}

impl Default for UiTheme {
    fn default() -> Self {
        Self::DARK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_tokens_match_the_reviewed_dark_snapshot() {
        assert_eq!(
            UiTheme::default().snapshot(),
            concat!(
                "background=#070A0FFF;top_bar=#0A0F17F6;panel=#0E151FF4;",
                "panel_elevated=#141D29FA;separator=#3F4E60B4;accent=#4CD3FFFF;",
                "status_live=#56D38BFF;",
                "text_primary=#E7F0F9FF;text_muted=#8495A8FF;",
                "text_disabled=#4D5A69FF;scrim=#03060ADA;",
                "spacing=1.0,6.0,4.0,8.0,12.0,16.0,24.0;",
                "type=11.0,12.0,13.0,16.0,18.0;tracking=1.8"
            )
        );
    }
}
