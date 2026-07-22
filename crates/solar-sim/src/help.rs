//! WP5/WP7 — command-routed gameplay help and control discoverability.
//!
//! Escape opens this retained modal only after text editing and higher-priority
//! modals decline the key. The surface owns focus and scrolling while open;
//! its actions return to the shared `SimCommand` boundary.

use crate::control::{SimCommand, SimCommandQueue};
use crate::input_intent::{ModalSurfaceSet, UiScrollSurface};
use crate::layers::{HudSurface, PresentationState};
use crate::ui_kit::{UiTheme, INTER_FONT_ASSET};
use crate::SimulationSet;
use bevy::{
    input::mouse::MouseScrollUnit,
    input_focus::{
        tab_navigation::{TabGroup, TabIndex},
        FocusCause, InputFocus,
    },
    prelude::*,
    text::{LineBreak, TextLayout},
    ui_widgets::Activate,
};

// Escape priority is TextEdit → Browse → Settings → Help. Keep the defensive
// visual stack below both higher-priority modals while remaining above the HUD.
const HELP_Z_INDEX: i32 = 113;
const HELP_CLOSE_TAB_INDEX: i32 = 0;
const HELP_GUIDE_FIRST_TAB_INDEX: i32 = 10;
const HELP_RESET_TAB_INDEX: i32 = 100;

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct HelpModalRoot;

#[derive(Component, Debug, Clone, Copy)]
struct HelpScrollArea;

#[derive(Component, Debug, Clone, Copy)]
struct HelpGuideEntry;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum HelpAction {
    Close,
    ResetView,
}

#[derive(Resource, Debug, Default)]
struct HelpUiState {
    rendered_open: bool,
    scroll_y: f32,
    previous_focus: Option<Entity>,
}

#[derive(Debug, Clone, Copy)]
struct GuideEntry {
    heading: &'static str,
    body: &'static str,
}

const GUIDE_ENTRIES: [GuideEntry; 7] = [
    GuideEntry {
        heading: "EXPLORE",
        body: "Left-drag or right-drag the scene to orbit the focused body. Scroll to dolly. A short primary click still selects a body.",
    },
    GuideEntry {
        heading: "RESET VIEW",
        body: "Press Home, or use RESET VIEW below, to return to the Sun-focused startup angle and full-system framing.",
    },
    GuideEntry {
        heading: "TIME RATE",
        body: "Left/Right Arrow or [/] steps down/up the signed rate ladder. Down Arrow selects +1 DAY/S. Key 1 selects REAL RATE.",
    },
    GuideEntry {
        heading: "PLAYBACK",
        body: "Space toggles play/pause. R plays and P pauses. The LIVE chip eases back to the moving wall-clock target at real rate.",
    },
    GuideEntry {
        heading: "QUICK TRAVEL",
        body: "O, M, S, and I travel to the Sun, Mercury, Sedna, and Io. Search, Browse, labels, and body clicks expose the complete catalog.",
    },
    GuideEntry {
        heading: "PANELS AND LAYERS",
        body: "Use the left panel for body information and view options. The right rail opens Layers, fullscreen, and Settings controls.",
    },
    GuideEntry {
        heading: "KEYBOARD AND ESCAPE",
        body: "Tab and Shift-Tab move through controls. Escape first reverts active text, then closes Browse, Settings, or this guide; from the scene it opens this guide.",
    },
];

pub(crate) struct HelpPlugin;

impl Plugin for HelpPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HelpUiState>().add_systems(
            Update,
            rebuild_help_modal
                .in_set(ModalSurfaceSet::Rebuild)
                .in_set(SimulationSet::Render),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn rebuild_help_modal(
    mut commands: Commands,
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
    presentation: Res<PresentationState>,
    mut state: ResMut<HelpUiState>,
    roots: Query<Entity, With<HelpModalRoot>>,
    parents: Query<&ChildOf>,
    focusables: Query<(), With<TabIndex>>,
    mut focus: ResMut<InputFocus>,
) {
    let open = presentation.is_help_open();
    let surface_matches = if open {
        roots.iter().count() == 1
    } else {
        roots.is_empty()
    };
    if state.rendered_open == open && surface_matches {
        return;
    }

    if open && !state.rendered_open {
        state.previous_focus = focus.get();
        state.scroll_y = 0.0;
    }

    let focused_help = focus.get().is_some_and(|focused| {
        roots
            .iter()
            .any(|root| is_descendant_of(focused, root, &parents))
    });
    for root in &roots {
        commands.entity(root).despawn();
    }

    if !open {
        if focused_help {
            if let Some(previous) = state
                .previous_focus
                .filter(|entity| focusables.get(*entity).is_ok())
            {
                focus.set(previous, FocusCause::Navigated);
            } else {
                focus.clear();
            }
        }
        state.rendered_open = false;
        state.previous_focus = None;
        return;
    }

    let font = asset_server.load(INTER_FONT_ASSET);
    let root = commands
        .spawn((
            Name::new("Controls and overview help"),
            HelpModalRoot,
            HudSurface,
            AccessibleLabel::new("Solar Sim overview and controls guide"),
            TabGroup::modal(),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(theme.spacing.lg_px)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme.colors.scrim.color()),
            Pickable::default(),
            GlobalZIndex(HELP_Z_INDEX),
        ))
        .id();
    let panel = commands
        .spawn((
            Name::new("Help panel"),
            Node {
                width: percent(90),
                max_width: px(720),
                height: percent(90),
                min_height: px(0),
                padding: UiRect::all(px(theme.spacing.lg_px)),
                border: UiRect::all(px(theme.spacing.hairline_px)),
                border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.spacing.sm_px),
                ..default()
            },
            BackgroundColor(theme.colors.background.color()),
            BorderColor::all(theme.colors.separator.color()),
            ChildOf(root),
        ))
        .id();

    let header = commands
        .spawn((
            Node {
                width: percent(100),
                min_height: px(42),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: px(theme.spacing.md_px),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        Text::new("SOLAR SIM GUIDE"),
        TextFont {
            font: font.clone().into(),
            font_size: theme.type_scale.product_px.into(),
            ..default()
        },
        TextColor(theme.colors.text_primary.color()),
        Pickable::IGNORE,
        ChildOf(header),
    ));
    let close = spawn_help_button(
        &mut commands,
        header,
        *theme,
        font.clone(),
        "CLOSE",
        "Close controls guide",
        HelpAction::Close,
        HELP_CLOSE_TAB_INDEX,
    );

    let scroll = commands
        .spawn((
            Name::new("Help scroll area"),
            HelpScrollArea,
            UiScrollSurface,
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                padding: UiRect::right(px(theme.spacing.sm_px)),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.spacing.sm_px),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition(Vec2::new(0.0, state.scroll_y)),
            ChildOf(panel),
        ))
        .observe(scroll_help_content)
        .id();

    for (index, entry) in GUIDE_ENTRIES.iter().enumerate() {
        let card = commands
            .spawn((
                Name::new(format!("{} help entry", entry.heading)),
                HelpGuideEntry,
                AccessibleLabel::new(format!("{}. {}", entry.heading, entry.body)),
                TabIndex(HELP_GUIDE_FIRST_TAB_INDEX + index as i32),
                Node {
                    width: percent(100),
                    min_height: px(58),
                    padding: UiRect::all(px(theme.spacing.md_px)),
                    border: UiRect::all(px(theme.spacing.hairline_px)),
                    border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(theme.spacing.xs_px),
                    ..default()
                },
                BackgroundColor(theme.colors.panel_elevated.color()),
                BorderColor::all(theme.colors.separator.color()),
                ChildOf(scroll),
            ))
            .id();
        commands.spawn((
            Text::new(entry.heading),
            TextFont {
                font: font.clone().into(),
                font_size: theme.type_scale.label_px.into(),
                ..default()
            },
            TextColor(theme.colors.accent.color()),
            Pickable::IGNORE,
            ChildOf(card),
        ));
        commands.spawn((
            Text::new(entry.body),
            TextFont {
                font: font.clone().into(),
                font_size: theme.type_scale.body_px.into(),
                ..default()
            },
            TextColor(theme.colors.text_muted.color()),
            TextLayout {
                linebreak: LineBreak::WordBoundary,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(card),
        ));
    }

    let footer = commands
        .spawn((
            Node {
                width: percent(100),
                min_height: px(42),
                flex_shrink: 0.0,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: px(theme.spacing.sm_px),
                row_gap: px(theme.spacing.sm_px),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        Text::new("ESC closes this guide"),
        TextFont {
            font: font.clone().into(),
            font_size: theme.type_scale.caption_px.into(),
            ..default()
        },
        TextColor(theme.colors.text_muted.color()),
        Pickable::IGNORE,
        ChildOf(footer),
    ));
    spawn_help_button(
        &mut commands,
        footer,
        *theme,
        font,
        "RESET VIEW",
        "Reset camera to the Sun-focused full-system view",
        HelpAction::ResetView,
        HELP_RESET_TAB_INDEX,
    );

    commands.queue(move |world: &mut World| {
        world
            .resource_mut::<InputFocus>()
            .set(close, FocusCause::Navigated);
    });
    state.rendered_open = true;
}

#[allow(clippy::too_many_arguments)]
fn spawn_help_button(
    commands: &mut Commands,
    parent: Entity,
    theme: UiTheme,
    font: Handle<Font>,
    label: &'static str,
    accessible_label: &'static str,
    action: HelpAction,
    tab_index: i32,
) -> Entity {
    let button = commands
        .spawn((
            Name::new(accessible_label),
            action,
            bevy::ui_widgets::Button,
            AccessibleLabel::new(accessible_label),
            TabIndex(tab_index),
            Node {
                min_width: px(108),
                min_height: px(38),
                padding: UiRect::horizontal(px(theme.spacing.md_px)),
                border: UiRect::all(px(theme.spacing.hairline_px)),
                border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme.colors.panel_elevated.color()),
            BorderColor::all(theme.colors.accent.color()),
            ChildOf(parent),
        ))
        .observe(activate_help_action)
        .id();
    commands.spawn((
        Text::new(label),
        TextFont {
            font: font.into(),
            font_size: theme.type_scale.label_px.into(),
            ..default()
        },
        TextColor(theme.colors.text_primary.color()),
        Pickable::IGNORE,
        ChildOf(button),
    ));
    button
}

fn activate_help_action(
    activate: On<Activate>,
    actions: Query<&HelpAction>,
    mut commands: ResMut<SimCommandQueue>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    commands.push(match action {
        HelpAction::Close => SimCommand::CloseHelp,
        HelpAction::ResetView => SimCommand::ResetView,
    });
}

fn scroll_help_content(
    mut scroll: On<Pointer<Scroll>>,
    mut areas: Query<(&mut ScrollPosition, &ComputedNode), With<HelpScrollArea>>,
    mut state: ResMut<HelpUiState>,
) {
    let Ok((mut position, node)) = areas.get_mut(scroll.entity) else {
        return;
    };
    position.y = next_help_scroll_y(
        position.y,
        scroll.y,
        scroll.unit,
        node.content_size().y,
        node.size().y,
        node.inverse_scale_factor,
    );
    state.scroll_y = position.y;
    scroll.propagate(false);
}

fn next_help_scroll_y(
    current: f32,
    input_y: f32,
    unit: MouseScrollUnit,
    content_height: f32,
    visible_height: f32,
    inverse_scale_factor: f32,
) -> f32 {
    let delta_y = match unit {
        MouseScrollUnit::Line => input_y * 28.0,
        MouseScrollUnit::Pixel => input_y,
    };
    let range = (content_height - visible_height).max(0.0) * inverse_scale_factor;
    (current - delta_y).clamp(0.0, range)
}

fn is_descendant_of(mut entity: Entity, ancestor: Entity, parents: &Query<&ChildOf>) -> bool {
    for _ in 0..32 {
        if entity == ancestor {
            return true;
        }
        let Ok(parent) = parents.get(entity) else {
            return false;
        };
        entity = parent.parent();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::test_layout;

    fn open_presentation() -> PresentationState {
        let mut presentation = PresentationState::default();
        presentation.open_help();
        presentation
    }

    #[test]
    fn help_actions_queue_only_their_semantic_commands() {
        let mut app = App::new();
        app.init_resource::<SimCommandQueue>();
        let close = app
            .world_mut()
            .spawn(HelpAction::Close)
            .observe(activate_help_action)
            .id();
        let reset = app
            .world_mut()
            .spawn(HelpAction::ResetView)
            .observe(activate_help_action)
            .id();

        app.world_mut().trigger(Activate { entity: close });
        app.world_mut().trigger(Activate { entity: reset });

        assert_eq!(
            app.world_mut()
                .resource_mut::<SimCommandQueue>()
                .drain()
                .collect::<Vec<_>>(),
            vec![SimCommand::CloseHelp, SimCommand::ResetView]
        );
    }

    #[test]
    fn help_scroll_clamps_line_and_pixel_input() {
        assert_eq!(
            next_help_scroll_y(10.0, -2.0, MouseScrollUnit::Line, 500.0, 200.0, 1.0),
            66.0
        );
        assert_eq!(
            next_help_scroll_y(290.0, -50.0, MouseScrollUnit::Pixel, 500.0, 200.0, 1.0),
            300.0
        );
        assert_eq!(
            next_help_scroll_y(10.0, 50.0, MouseScrollUnit::Pixel, 500.0, 200.0, 1.0),
            0.0
        );
    }

    #[test]
    fn help_copy_and_readme_document_the_same_control_aliases() {
        let guide = GUIDE_ENTRIES
            .iter()
            .map(|entry| entry.body)
            .collect::<Vec<_>>()
            .join(" ");
        let readme = include_str!("../../../README.md");
        for expected in [
            "Left-drag",
            "right-drag",
            "Home",
            "Left/Right Arrow",
            "Down Arrow",
            "Space",
            "Escape",
        ] {
            assert!(guide.contains(expected), "Help omitted {expected}");
        }
        for expected in [
            "left-drag",
            "right-drag",
            "`←` / `→`",
            "`↓`",
            "`Home`",
            "`Escape`",
        ] {
            assert!(readme.contains(expected), "README omitted {expected}");
        }
    }

    #[test]
    fn help_modal_traps_focus_retains_identity_and_restores_its_invoker() {
        let mut app = test_layout::app(960, 600, 1.0);
        let invoker = app
            .world_mut()
            .spawn((bevy::ui_widgets::Button, TabIndex(42)))
            .id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(invoker, FocusCause::Navigated);
        app.insert_resource(UiTheme::default())
            .insert_resource(open_presentation())
            .init_resource::<HelpUiState>()
            .add_systems(Update, rebuild_help_modal);
        test_layout::settle(&mut app);

        let root = app
            .world_mut()
            .query_filtered::<Entity, With<HelpModalRoot>>()
            .single(app.world())
            .unwrap();
        let focused = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().get::<HelpAction>(focused),
            Some(&HelpAction::Close)
        );
        assert!(app.world().get::<TabGroup>(root).unwrap().modal);

        test_layout::settle(&mut app);
        let stable_root = app
            .world_mut()
            .query_filtered::<Entity, With<HelpModalRoot>>()
            .single(app.world())
            .unwrap();
        assert_eq!(stable_root, root);

        app.world_mut()
            .resource_mut::<PresentationState>()
            .close_help();
        test_layout::settle(&mut app);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(invoker));
        assert!(app
            .world_mut()
            .query_filtered::<Entity, With<HelpModalRoot>>()
            .iter(app.world())
            .next()
            .is_none());
    }

    #[test]
    fn help_content_and_actions_are_reachable_at_every_required_layout() {
        for (width, height, scale) in test_layout::required_viewports() {
            let mut app = test_layout::app(width, height, scale);
            app.insert_resource(UiTheme::default())
                .insert_resource(open_presentation())
                .init_resource::<HelpUiState>()
                .init_resource::<InputFocus>()
                .add_systems(Update, rebuild_help_modal);
            test_layout::settle(&mut app);

            let root = app
                .world_mut()
                .query_filtered::<Entity, With<HelpModalRoot>>()
                .single(app.world())
                .unwrap();
            let scroll = app
                .world_mut()
                .query_filtered::<Entity, With<HelpScrollArea>>()
                .single(app.world())
                .unwrap();
            let viewport = Rect::from_corners(Vec2::ZERO, Vec2::new(width as f32, height as f32));
            assert!(rect_contains(viewport, node_rect(app.world(), root)));
            assert!(node_rect(app.world(), scroll).height() > 0.0);

            let close = action_entity(app.world_mut(), HelpAction::Close);
            let reset = action_entity(app.world_mut(), HelpAction::ResetView);
            assert!(rect_contains(
                node_rect(app.world(), root),
                node_rect(app.world(), close)
            ));
            assert!(rect_contains(
                node_rect(app.world(), root),
                node_rect(app.world(), reset)
            ));
            assert_eq!(
                app.world_mut()
                    .query::<(&HelpGuideEntry, &AccessibleLabel, &TabIndex)>()
                    .iter(app.world())
                    .filter(|(_, label, index)| !label.0.trim().is_empty() && index.0 >= 0)
                    .count(),
                GUIDE_ENTRIES.len()
            );

            app.world_mut()
                .entity_mut(scroll)
                .get_mut::<ScrollPosition>()
                .unwrap()
                .y = f32::MAX;
            test_layout::settle(&mut app);
            let last = app
                .world_mut()
                .query::<(Entity, &HelpGuideEntry, &TabIndex)>()
                .iter(app.world())
                .max_by_key(|(_, _, tab_index)| tab_index.0)
                .map(|(entity, _, _)| entity)
                .unwrap();
            assert!(
                rect_contains(node_rect(app.world(), scroll), node_rect(app.world(), last)),
                "{width}×{height} scale {scale}: final Help entry is unreachable"
            );
        }
    }

    fn action_entity(world: &mut World, action: HelpAction) -> Entity {
        world
            .query::<(Entity, &HelpAction)>()
            .iter(world)
            .find_map(|(entity, candidate)| (*candidate == action).then_some(entity))
            .unwrap()
    }

    fn node_rect(world: &World, entity: Entity) -> Rect {
        let node = world.get::<ComputedNode>(entity).unwrap();
        let center = world
            .get::<UiGlobalTransform>(entity)
            .unwrap()
            .affine()
            .translation;
        Rect::from_center_size(center, node.size())
    }

    fn rect_contains(outer: Rect, inner: Rect) -> bool {
        outer.min.x <= inner.min.x + 0.5
            && outer.min.y <= inner.min.y + 0.5
            && outer.max.x + 0.5 >= inner.max.x
            && outer.max.y + 0.5 >= inner.max.y
    }
}
