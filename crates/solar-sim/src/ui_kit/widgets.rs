//! WP7 call-site-stable BSN widget façade — Rev C §8.4.
//!
//! Later HUD packages depend on these functions and specs, not on their
//! internal Bevy component composition. That keeps the documented fallback
//! policy real: BSN internals can move to classic spawn or Feathers without
//! rewriting every call site.

use super::theme::{UiColorToken, UiTheme};
use bevy::{
    prelude::*,
    text::{FontSourceTemplate, LetterSpacing},
};

pub const INTER_FONT_ASSET: &str = "fonts/InterVariable.ttf";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum WidgetKind {
    #[default]
    Panel,
    TabBar,
    CheckboxRow,
    SectionHeader,
    Chip,
    Slider,
    Toast,
}

impl WidgetKind {
    pub const ALL: [Self; 7] = [
        Self::Panel,
        Self::TabBar,
        Self::CheckboxRow,
        Self::SectionHeader,
        Self::Chip,
        Self::Slider,
        Self::Toast,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Panel => "Panel",
            Self::TabBar => "Tab bar",
            Self::CheckboxRow => "Checkbox row",
            Self::SectionHeader => "Section header",
            Self::Chip => "Chip",
            Self::Slider => "Slider",
            Self::Toast => "Toast",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum WidgetVisualState {
    #[default]
    Default,
    Hovered,
    Active,
    Disabled,
}

impl WidgetVisualState {
    pub const ALL: [Self; 4] = [Self::Default, Self::Hovered, Self::Active, Self::Disabled];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Hovered => "Hovered",
            Self::Active => "Active",
            Self::Disabled => "Disabled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetSpec {
    pub text: String,
    pub accessible_label: String,
    pub state: WidgetVisualState,
}

impl WidgetSpec {
    pub fn new(
        text: impl Into<String>,
        accessible_label: impl Into<String>,
        state: WidgetVisualState,
    ) -> Self {
        Self {
            text: text.into(),
            accessible_label: accessible_label.into(),
            state,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, FromTemplate)]
pub struct WidgetRoot {
    pub kind: WidgetKind,
    pub state: WidgetVisualState,
}

#[derive(Debug, Clone, Copy)]
struct WidgetColors {
    background: Color,
    border: Color,
    text: Color,
}

fn colors_for(theme: UiTheme, state: WidgetVisualState) -> WidgetColors {
    match state {
        WidgetVisualState::Default => WidgetColors {
            background: theme.colors.panel.color(),
            border: theme.colors.separator.color(),
            text: theme.colors.text_primary.color(),
        },
        WidgetVisualState::Hovered => WidgetColors {
            background: theme.colors.panel_elevated.color(),
            border: alpha(theme.colors.accent, 190),
            text: theme.colors.text_primary.color(),
        },
        WidgetVisualState::Active => WidgetColors {
            background: alpha(theme.colors.accent, 42),
            border: theme.colors.accent.color(),
            text: theme.colors.text_primary.color(),
        },
        WidgetVisualState::Disabled => WidgetColors {
            background: alpha(theme.colors.background, 220),
            border: alpha(theme.colors.separator, 80),
            text: theme.colors.text_disabled.color(),
        },
    }
}

fn alpha(token: UiColorToken, alpha: u8) -> Color {
    Color::srgba_u8(token.0[0], token.0[1], token.0[2], alpha)
}

pub fn panel(theme: UiTheme, spec: WidgetSpec) -> impl Scene {
    let colors = colors_for(theme, spec.state);
    let text = spec.text;
    let accessible_label = spec.accessible_label;
    let state = spec.state;
    bsn! {
        Node {
            width: percent(100),
            min_height: px(52),
            padding: UiRect::all(px(theme.spacing.md_px)),
            border: UiRect::all(px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
            align_items: AlignItems::Center,
        }
        WidgetRoot { kind: WidgetKind::Panel, state }
        AccessibleLabel(accessible_label)
        BackgroundColor({colors.background})
        BorderColor::all(colors.border)
        Children [(
            Text(text)
            TextFont {
                font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                font_size: px(theme.type_scale.body_px),
            }
            TextColor({colors.text})
        )]
    }
}

pub fn tab_bar(theme: UiTheme, spec: WidgetSpec) -> impl Scene {
    let colors = colors_for(theme, spec.state);
    let text = spec.text;
    let accessible_label = spec.accessible_label;
    let state = spec.state;
    bsn! {
        Node {
            width: percent(100),
            height: px(38),
            padding: UiRect::all(px(theme.spacing.xs_px)),
            border: UiRect::all(px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
            align_items: AlignItems::Center,
            column_gap: px(theme.spacing.xs_px),
        }
        WidgetRoot { kind: WidgetKind::TabBar, state }
        AccessibleLabel(accessible_label)
        BackgroundColor({colors.background})
        BorderColor::all(colors.border)
        Children [
            tab_segment(theme, text, colors.text, state == WidgetVisualState::Active),
            tab_segment(theme, "DATA".to_string(), theme.colors.text_muted.color(), false),
        ]
    }
}

fn tab_segment(theme: UiTheme, text: String, text_color: Color, active: bool) -> impl Scene {
    let tracking = theme.type_scale.uppercase_tracking_px;
    let background = if active {
        alpha(theme.colors.accent, 34)
    } else {
        Color::NONE
    };
    bsn! {
        Button
        Node {
            flex_grow: 1.0,
            height: percent(100),
            border_radius: BorderRadius::all(px(theme.spacing.xs_px)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        BackgroundColor(background)
        Children [(
            Text(text)
            TextFont {
                font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                font_size: px(theme.type_scale.caption_px),
            }
            TextColor(text_color)
            template_value(LetterSpacing::Px(tracking))
        )]
    }
}

pub fn checkbox_row(theme: UiTheme, spec: WidgetSpec) -> impl Scene {
    let colors = colors_for(theme, spec.state);
    let text = spec.text;
    let accessible_label = spec.accessible_label;
    let state = spec.state;
    let check_background = if state == WidgetVisualState::Active {
        theme.colors.accent.color()
    } else {
        Color::NONE
    };
    bsn! {
        Button
        Node {
            width: percent(100),
            height: px(38),
            padding: UiRect::horizontal(px(theme.spacing.sm_px)),
            border: UiRect::all(px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
            align_items: AlignItems::Center,
            column_gap: px(theme.spacing.sm_px),
        }
        WidgetRoot { kind: WidgetKind::CheckboxRow, state }
        AccessibleLabel(accessible_label)
        BackgroundColor({colors.background})
        BorderColor::all(colors.border)
        Children [
            (
                Node {
                    width: px(16),
                    height: px(16),
                    border: UiRect::all(px(theme.spacing.hairline_px)),
                    border_radius: BorderRadius::all(px(3)),
                }
                BackgroundColor(check_background)
                BorderColor::all(colors.border)
            ),
            (
                Text(text)
                TextFont {
                    font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                    font_size: px(theme.type_scale.body_px),
                }
                TextColor({colors.text})
            ),
        ]
    }
}

pub fn section_header(theme: UiTheme, spec: WidgetSpec) -> impl Scene {
    let colors = colors_for(theme, spec.state);
    let text = spec.text;
    let accessible_label = spec.accessible_label;
    let state = spec.state;
    let tracking = theme.type_scale.uppercase_tracking_px;
    bsn! {
        Node {
            width: percent(100),
            height: px(32),
            padding: UiRect::horizontal(px(theme.spacing.xs_px)),
            border: UiRect::bottom(px(theme.spacing.hairline_px)),
            align_items: AlignItems::Center,
        }
        WidgetRoot { kind: WidgetKind::SectionHeader, state }
        AccessibleLabel(accessible_label)
        BackgroundColor({colors.background})
        BorderColor::all(colors.border)
        Children [(
            Text(text)
            TextFont {
                font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                font_size: px(theme.type_scale.label_px),
            }
            TextColor({colors.text})
            template_value(LetterSpacing::Px(tracking))
        )]
    }
}

pub fn chip(theme: UiTheme, spec: WidgetSpec) -> impl Scene {
    let colors = colors_for(theme, spec.state);
    let text = spec.text;
    let accessible_label = spec.accessible_label;
    let state = spec.state;
    let tracking = theme.type_scale.uppercase_tracking_px;
    bsn! {
        Button
        Node {
            height: px(28),
            padding: UiRect::horizontal(px(theme.spacing.sm_px)),
            border: UiRect::all(px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::MAX,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        WidgetRoot { kind: WidgetKind::Chip, state }
        AccessibleLabel(accessible_label)
        BackgroundColor({colors.background})
        BorderColor::all(colors.border)
        Children [(
            Text(text)
            TextFont {
                font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                font_size: px(theme.type_scale.caption_px),
            }
            TextColor({colors.text})
            template_value(LetterSpacing::Px(tracking))
        )]
    }
}

pub fn slider(theme: UiTheme, spec: WidgetSpec) -> impl Scene {
    let colors = colors_for(theme, spec.state);
    let text = spec.text;
    let accessible_label = spec.accessible_label;
    let state = spec.state;
    let fill_width = match state {
        WidgetVisualState::Default => 42.0,
        WidgetVisualState::Hovered => 54.0,
        WidgetVisualState::Active => 72.0,
        WidgetVisualState::Disabled => 42.0,
    };
    bsn! {
        Button
        Node {
            width: percent(100),
            height: px(46),
            padding: UiRect::all(px(theme.spacing.sm_px)),
            border: UiRect::all(px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
            flex_direction: FlexDirection::Column,
            row_gap: px(theme.spacing.sm_px),
        }
        WidgetRoot { kind: WidgetKind::Slider, state }
        AccessibleLabel(accessible_label)
        BackgroundColor({colors.background})
        BorderColor::all(colors.border)
        Children [
            (
                Text(text)
                TextFont {
                    font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                    font_size: px(theme.type_scale.caption_px),
                }
                TextColor({colors.text})
            ),
            (
                Node {
                    width: percent(100),
                    height: px(4),
                    border_radius: BorderRadius::MAX,
                }
                BackgroundColor({theme.colors.separator.color()})
                Children [(
                    Node {
                        width: percent(fill_width),
                        height: percent(100),
                        border_radius: BorderRadius::MAX,
                    }
                    BackgroundColor({theme.colors.accent.color()})
                )]
            ),
        ]
    }
}

pub fn toast(theme: UiTheme, spec: WidgetSpec) -> impl Scene {
    let colors = colors_for(theme, spec.state);
    let text = spec.text;
    let accessible_label = spec.accessible_label;
    let state = spec.state;
    bsn! {
        Node {
            width: percent(100),
            min_height: px(48),
            padding: UiRect::all(px(theme.spacing.md_px)),
            border: UiRect::new(px(3), px(theme.spacing.hairline_px), px(theme.spacing.hairline_px), px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
            align_items: AlignItems::Center,
        }
        WidgetRoot { kind: WidgetKind::Toast, state }
        AccessibleLabel(accessible_label)
        BackgroundColor({colors.background})
        BorderColor::all(colors.border)
        Children [(
            Text(text)
            TextFont {
                font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                font_size: px(theme.type_scale.body_px),
            }
            TextColor({colors.text})
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        a11y::AccessibilityNode,
        app::TaskPoolPlugin,
        asset::{AssetApp, AssetPlugin},
        scene::{ScenePlugin, WorldSceneExt},
        text::Font,
    };
    use std::collections::HashSet;
    use std::path::Path;

    #[test]
    fn every_gallery_widget_state_resolves_to_an_accesskit_label() {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ))
        .init_asset::<Font>();
        let theme = UiTheme::default();
        let mut covered = HashSet::new();

        for kind in WidgetKind::ALL {
            for state in WidgetVisualState::ALL {
                let spec = WidgetSpec::new(
                    kind.name(),
                    format!("{} widget, {} state", kind.name(), state.name()),
                    state,
                );
                let world = app.world_mut();
                let entity = match kind {
                    WidgetKind::Panel => world.spawn_scene(panel(theme, spec)).unwrap().id(),
                    WidgetKind::TabBar => world.spawn_scene(tab_bar(theme, spec)).unwrap().id(),
                    WidgetKind::CheckboxRow => {
                        world.spawn_scene(checkbox_row(theme, spec)).unwrap().id()
                    }
                    WidgetKind::SectionHeader => {
                        world.spawn_scene(section_header(theme, spec)).unwrap().id()
                    }
                    WidgetKind::Chip => world.spawn_scene(chip(theme, spec)).unwrap().id(),
                    WidgetKind::Slider => world.spawn_scene(slider(theme, spec)).unwrap().id(),
                    WidgetKind::Toast => world.spawn_scene(toast(theme, spec)).unwrap().id(),
                };
                let entity_ref = app.world().entity(entity);
                let root = entity_ref.get::<WidgetRoot>().unwrap();
                let label = entity_ref.get::<AccessibleLabel>().unwrap();
                assert!(!label.0.trim().is_empty());
                assert!(entity_ref.contains::<AccessibilityNode>());
                covered.insert((root.kind, root.state));
            }
        }

        assert_eq!(
            covered.len(),
            WidgetKind::ALL.len() * WidgetVisualState::ALL.len()
        );
    }

    #[test]
    fn inter_font_license_and_audit_metadata_are_vendored_beside_the_font() {
        let fonts = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(crate::DEFAULT_BEVY_ASSET_ROOT)
            .join("fonts");
        let font = std::fs::metadata(fonts.join("InterVariable.ttf")).unwrap();
        let license = std::fs::read_to_string(fonts.join("Inter-OFL-1.1.txt")).unwrap();
        let source = std::fs::read_to_string(fonts.join("Inter-SOURCE.md")).unwrap();

        assert!(font.len() > 100_000);
        assert!(license.contains("SIL OPEN FONT LICENSE Version 1.1"));
        assert!(source.contains("Family: Inter"));
        assert!(source.contains("Version: 4.1"));
        assert!(source.contains("https://github.com/rsms/inter"));
        assert!(source.contains("4989b125924991b90d05b2d16e0e388c48f7d5bb8b30539bbf9c755278d0ccaf"));
    }
}
