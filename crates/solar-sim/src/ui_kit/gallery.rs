//! Debug-only WP7 widget gallery — Rev C §8.4.
//!
//! The gallery is both the visual development scene and the exhaustive
//! accessibility fixture: every stable widget function is instantiated in
//! every documented visual state.

use super::{
    checkbox_row, chip, panel, section_header, slider, tab_bar, toast, UiTheme, WidgetKind,
    WidgetSpec, WidgetVisualState, INTER_FONT_ASSET,
};
use bevy::{
    prelude::*,
    text::{FontSourceTemplate, LetterSpacing},
};

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
pub struct WidgetGalleryRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetGalleryCell {
    pub kind: WidgetKind,
    pub state: WidgetVisualState,
}

fn gallery_root(theme: UiTheme) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: px(76),
            left: percent(3),
            width: percent(94),
            max_height: percent(88),
            padding: UiRect::all(px(theme.spacing.lg_px)),
            border: UiRect::all(px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
            display: Display::Grid,
            grid_template_columns: {vec![RepeatedGridTrack::flex(4, 1.0)]},
            row_gap: px(theme.spacing.md_px),
            column_gap: px(theme.spacing.md_px),
            overflow: Overflow::scroll_y(),
        }
        WidgetGalleryRoot
        AccessibleLabel("Widget gallery: all widgets in all states")
        BackgroundColor({theme.colors.scrim.color()})
        BorderColor::all(theme.colors.separator.color())
        GlobalZIndex(90)
    }
}

fn gallery_cell(theme: UiTheme, label: String, parent: Entity) -> impl Scene {
    let tracking = theme.type_scale.uppercase_tracking_px;
    bsn! {
        Node {
            min_width: px(180),
            min_height: px(92),
            padding: UiRect::all(px(theme.spacing.sm_px)),
            border: UiRect::all(px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
            flex_direction: FlexDirection::Column,
            row_gap: px(theme.spacing.sm_px),
        }
        ChildOf(parent)
        AccessibleLabel({label.clone()})
        BackgroundColor({theme.colors.background.color()})
        BorderColor::all(theme.colors.separator.color())
        Children [(
            Text(label)
            TextFont {
                font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                font_size: px(theme.type_scale.caption_px),
            }
            TextColor({theme.colors.text_muted.color()})
            template_value(LetterSpacing::Px(tracking))
        )]
    }
}

pub(super) fn spawn_widget_gallery(mut commands: Commands, theme: Res<UiTheme>) {
    let theme = *theme;
    let root = commands.spawn_scene(gallery_root(theme)).id();
    for kind in WidgetKind::ALL {
        for state in WidgetVisualState::ALL {
            let label = format!("{} · {}", kind.name(), state.name());
            let cell = commands
                .spawn_scene(gallery_cell(theme, label.clone(), root))
                .insert(WidgetGalleryCell { kind, state })
                .id();
            let spec = WidgetSpec::new(
                kind.name(),
                format!("{} widget, {} state", kind.name(), state.name()),
                state,
            );
            match kind {
                WidgetKind::Panel => {
                    commands
                        .spawn_scene(panel(theme, spec))
                        .insert(ChildOf(cell));
                }
                WidgetKind::TabBar => {
                    commands
                        .spawn_scene(tab_bar(theme, spec))
                        .insert(ChildOf(cell));
                }
                WidgetKind::CheckboxRow => {
                    commands
                        .spawn_scene(checkbox_row(theme, spec))
                        .insert(ChildOf(cell));
                }
                WidgetKind::SectionHeader => {
                    commands
                        .spawn_scene(section_header(theme, spec))
                        .insert(ChildOf(cell));
                }
                WidgetKind::Chip => {
                    commands
                        .spawn_scene(chip(theme, spec))
                        .insert(ChildOf(cell));
                }
                WidgetKind::Slider => {
                    commands
                        .spawn_scene(slider(theme, spec))
                        .insert(ChildOf(cell));
                }
                WidgetKind::Toast => {
                    commands
                        .spawn_scene(toast(theme, spec))
                        .insert(ChildOf(cell));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::WidgetRoot;
    use super::*;
    use bevy::{
        a11y::AccessibilityNode,
        app::TaskPoolPlugin,
        asset::{AssetApp, AssetPlugin},
        scene::ScenePlugin,
        text::Font,
    };
    use std::collections::HashSet;

    #[test]
    fn gallery_spawns_every_widget_state_with_accesskit_labels() {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ))
        .init_asset::<Font>()
        .insert_resource(UiTheme::default())
        .add_systems(Startup, spawn_widget_gallery);
        app.update();

        let expected: HashSet<_> = WidgetKind::ALL
            .into_iter()
            .flat_map(|kind| {
                WidgetVisualState::ALL
                    .into_iter()
                    .map(move |state| (kind, state))
            })
            .collect();
        let world = app.world_mut();
        let mut query = world.query::<(&WidgetRoot, &AccessibleLabel, &AccessibilityNode)>();
        let actual: HashSet<_> = query
            .iter(world)
            .map(|(root, label, _accesskit_node)| {
                assert!(!label.0.trim().is_empty());
                (root.kind, root.state)
            })
            .collect();

        assert_eq!(actual.len(), 28);
        assert_eq!(actual, expected);
    }
}
