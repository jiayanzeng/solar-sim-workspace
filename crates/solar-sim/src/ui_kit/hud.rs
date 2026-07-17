//! WP7 top bar and breadcrumb binding — Rev C §9.1.
//!
//! WP12 binds behavior onto the labelled search and Menu controls established
//! here. Keeping their component markers in `ui_kit` preserves WP7's stable
//! top-bar scene signature while search internals remain independently owned.

use super::{NavigationDestination, NavigationStack, UiTheme, INTER_FONT_ASSET};
use crate::control::{SimCommand, SimCommandQueue};
use crate::input_intent::UiScrollSurface;
use crate::layers::HudSurface;
use bevy::{
    input::mouse::MouseScrollUnit,
    input_focus::{
        tab_navigation::{TabGroup, TabIndex},
        FocusCause, InputFocus,
    },
    picking::events::{Pointer, Scroll},
    prelude::*,
    text::{EditableText, FontSourceTemplate, LetterSpacing, LineBreak, TextLayout},
    ui_widgets::Activate,
};

pub const TOP_BAR_HEIGHT_PX: f32 = 64.0;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
pub struct TopBarRoot;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
pub struct BreadcrumbText;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
pub struct SearchPlaceholder;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
pub struct SearchInput;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
pub struct SearchHint;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
pub struct MenuBrowseButton;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct BreadcrumbOverlayRoot;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
pub struct BreadcrumbHost;

#[derive(Component, Debug, Clone)]
pub(super) struct BreadcrumbAction {
    depth: usize,
    target_id: String,
    destination: NavigationDestination,
}

pub fn top_bar(theme: UiTheme, breadcrumb: String) -> impl Scene {
    let tracking = theme.type_scale.uppercase_tracking_px;
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: px(0),
            width: percent(100),
            height: px(TOP_BAR_HEIGHT_PX),
            padding: UiRect::horizontal(px(theme.spacing.lg_px)),
            border: UiRect::bottom(px(theme.spacing.hairline_px)),
            align_items: AlignItems::Center,
            column_gap: px(theme.spacing.xs_px),
        }
        TopBarRoot
        HudSurface
        AccessibleLabel("Solar Sim top bar")
        template_value(TabGroup::new(0))
        BackgroundColor({theme.colors.top_bar.color()})
        BorderColor::all(theme.colors.separator.color())
        GlobalZIndex(100)
        Children [
            logo(theme),
            (
                Text("SOLAR SIM")
                TextFont {
                    font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                    font_size: px(theme.type_scale.product_px),
                }
                TextColor({theme.colors.text_primary.color()})
                template_value(LetterSpacing::Px(tracking))
            ),
            (
                Node {
                    width: px(theme.spacing.hairline_px),
                    height: px(24),
                }
                BackgroundColor({theme.colors.separator.color()})
            ),
            (
                Node {
                    flex_grow: 1.0,
                    min_width: px(0),
                    align_items: AlignItems::Center,
                    overflow: Overflow::scroll_x(),
                }
                BreadcrumbHost
                UiScrollSurface
                template_value(ScrollPosition::default())
                on(scroll_breadcrumb)
                Children [
                    (
                        Text(breadcrumb)
                        BreadcrumbText
                        AccessibleLabel("Current navigation breadcrumb")
                        TextFont {
                            font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                            font_size: px(theme.type_scale.body_px),
                        }
                        TextColor({theme.colors.text_muted.color()})
                    ),
                ]
            ),
            menu_button(theme),
            search_placeholder(theme),
        ]
    }
}

fn menu_button(theme: UiTheme) -> impl Scene {
    let tracking = theme.type_scale.uppercase_tracking_px;
    bsn! {
        Node {
            width: px(74),
            height: px(36),
            border: UiRect::all(px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        bevy::ui_widgets::Button
        MenuBrowseButton
        AccessibleLabel("Open body browse menu")
        TabIndex(100)
        BackgroundColor({theme.colors.panel.color()})
        BorderColor::all(theme.colors.separator.color())
        Children [(
            Text("MENU")
            TextFont {
                font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                font_size: px(theme.type_scale.caption_px),
            }
            TextColor({theme.colors.text_primary.color()})
            template_value(LetterSpacing::Px(tracking))
            Pickable::IGNORE
        )]
    }
}

fn logo(theme: UiTheme) -> impl Scene {
    bsn! {
        Node {
            width: px(30),
            height: px(30),
            border: UiRect::all(px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::MAX,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        AccessibleLabel("Solar Sim orbital logo")
        BorderColor::all(theme.colors.accent.color())
        Children [(
            Node {
                width: px(6),
                height: px(6),
                border_radius: BorderRadius::MAX,
            }
            BackgroundColor({theme.colors.accent.color()})
        )]
    }
}

fn search_placeholder(theme: UiTheme) -> impl Scene {
    bsn! {
        Node {
            width: px(280),
            min_width: px(120),
            height: px(36),
            flex_shrink: 1.0,
            padding: UiRect::horizontal(px(theme.spacing.md_px)),
            border: UiRect::all(px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
            align_items: AlignItems::Center,
        }
        SearchPlaceholder
        BackgroundColor({theme.colors.background.color()})
        BorderColor::all(theme.colors.separator.color())
        Children [
            (
                Text("Search bodies…")
                SearchHint
                Node {
                    position_type: PositionType::Absolute,
                    left: px(theme.spacing.md_px),
                }
                TextFont {
                    font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                    font_size: px(theme.type_scale.body_px),
                }
                TextColor({theme.colors.text_muted.color()})
                Pickable::IGNORE
            ),
            (
                template_value(EditableText::new(""))
                SearchInput
                AccessibleLabel("Search bodies")
                TabIndex(101)
                Node { width: percent(100) }
                TextFont {
                    font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                    font_size: px(theme.type_scale.body_px),
                }
                TextColor({theme.colors.text_primary.color()})
                TextLayout {
                    linebreak: LineBreak::NoWrap,
                }
            ),
        ]
    }
}

pub(super) fn spawn_top_bar(
    mut commands: Commands,
    theme: Res<UiTheme>,
    navigation: Res<NavigationStack>,
) {
    commands.spawn_scene(top_bar(*theme, navigation.label()));
}

pub(super) fn update_breadcrumb(
    navigation: Res<NavigationStack>,
    mut breadcrumbs: Query<&mut Text, With<BreadcrumbText>>,
) {
    if !navigation.is_changed() {
        return;
    }
    let label = navigation.label();
    for mut breadcrumb in &mut breadcrumbs {
        if breadcrumb.as_str() != label {
            **breadcrumb = label.clone();
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild_actionable_breadcrumb(
    mut commands: Commands,
    navigation: Res<NavigationStack>,
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
    roots: Query<Entity, With<BreadcrumbOverlayRoot>>,
    hosts: Query<Entity, With<BreadcrumbHost>>,
    actions: Query<&BreadcrumbAction>,
    focus: Res<InputFocus>,
    mut legacy_text: Query<&mut Visibility, With<BreadcrumbText>>,
) {
    if !navigation.is_changed() {
        return;
    }
    let Ok(host) = hosts.single() else {
        return;
    };
    let focused_action = focus
        .get()
        .and_then(|entity| actions.get(entity).ok())
        .cloned();
    for root in &roots {
        commands.entity(root).despawn();
    }
    for mut visibility in &mut legacy_text {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    }
    let root = commands
        .spawn((
            Name::new("Actionable breadcrumb"),
            BreadcrumbOverlayRoot,
            AccessibleLabel::new("Current navigation breadcrumb"),
            Node {
                flex_shrink: 0.0,
                height: percent(100),
                align_items: AlignItems::Center,
                column_gap: px(theme.spacing.xs_px),
                ..default()
            },
            ChildOf(host),
        ))
        .id();
    let last = navigation.items().len().saturating_sub(1);
    for (depth, item) in navigation.items().iter().enumerate() {
        if depth > 0 {
            commands.spawn((
                Text::new("›"),
                TextFont {
                    font: asset_server.load(INTER_FONT_ASSET).into(),
                    font_size: theme.type_scale.body_px.into(),
                    ..default()
                },
                TextColor(theme.colors.text_disabled.color()),
                Pickable::IGNORE,
                ChildOf(root),
            ));
        }
        let button = commands
            .spawn((
                bevy::ui_widgets::Button,
                BreadcrumbAction {
                    depth,
                    target_id: item.id.clone(),
                    destination: item.destination.clone(),
                },
                AccessibleLabel::new(format!("Navigate to {}", item.label)),
                TabIndex(2 + depth as i32),
                Node {
                    height: px(32),
                    padding: UiRect::horizontal(px(theme.spacing.xs_px)),
                    border_radius: BorderRadius::all(px(theme.spacing.xs_px)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                ChildOf(root),
            ))
            .observe(activate_breadcrumb)
            .id();
        commands.spawn((
            Text::new(item.label.clone()),
            TextFont {
                font: asset_server.load(INTER_FONT_ASSET).into(),
                font_size: theme.type_scale.body_px.into(),
                ..default()
            },
            TextColor(if depth == last {
                theme.colors.text_primary.color()
            } else {
                theme.colors.text_muted.color()
            }),
            Pickable::IGNORE,
            ChildOf(button),
        ));
    }
    if let Some(focused_action) = focused_action {
        queue_breadcrumb_focus_restore(&mut commands, focused_action);
    }
}

fn queue_breadcrumb_focus_restore(commands: &mut Commands, requested: BreadcrumbAction) {
    commands.queue(move |world: &mut World| {
        let mut actions = world.query::<(Entity, &BreadcrumbAction)>();
        let mut exact = None;
        let mut current = None;
        for (entity, action) in actions.iter(world) {
            if action.target_id == requested.target_id
                && action.destination == requested.destination
            {
                exact = Some(entity);
            }
            if current.is_none_or(|(_, depth)| action.depth > depth) {
                current = Some((entity, action.depth));
            }
        }
        if let Some(entity) = exact.or_else(|| current.map(|(entity, _)| entity)) {
            world
                .resource_mut::<InputFocus>()
                .set(entity, FocusCause::Navigated);
        }
    });
}

fn scroll_breadcrumb(
    mut scroll: On<Pointer<Scroll>>,
    mut hosts: Query<(&mut ScrollPosition, &ComputedNode), With<BreadcrumbHost>>,
) {
    let Ok((mut position, node)) = hosts.get_mut(scroll.entity) else {
        return;
    };
    let input = if scroll.x.abs() > f32::EPSILON {
        scroll.x
    } else {
        scroll.y
    };
    let delta = match scroll.unit {
        MouseScrollUnit::Line => input * 28.0,
        MouseScrollUnit::Pixel => input,
    };
    let range = (node.content_size().x - node.size().x).max(0.0) * node.inverse_scale_factor;
    position.x = (position.x - delta).clamp(0.0, range);
    scroll.propagate(false);
}

fn activate_breadcrumb(
    activate: On<Activate>,
    actions: Query<&BreadcrumbAction>,
    mut commands: ResMut<SimCommandQueue>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    commands.push(SimCommand::NavigateBreadcrumb {
        depth: action.depth,
        target_id: action.target_id.clone(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::test_layout;

    #[test]
    fn breadcrumb_text_tracks_navigation_resource_changes() {
        let mut app = App::new();
        app.insert_resource(NavigationStack::root())
            .add_systems(Update, update_breadcrumb);
        let entity = app
            .world_mut()
            .spawn((Text::new("stale"), BreadcrumbText))
            .id();

        app.world_mut()
            .resource_mut::<NavigationStack>()
            .push("jupiter", "Jupiter");
        app.world_mut()
            .resource_mut::<NavigationStack>()
            .push_collection("jupiter", "Moons");
        app.update();

        assert_eq!(
            app.world().entity(entity).get::<Text>().unwrap().as_str(),
            "Solar System › Jupiter › Moons"
        );
    }

    #[test]
    fn activating_an_ancestor_breadcrumb_queues_one_navigation_command() {
        let mut app = App::new();
        app.init_resource::<SimCommandQueue>();
        let jupiter = app
            .world_mut()
            .spawn(BreadcrumbAction {
                depth: 1,
                target_id: "jupiter".into(),
                destination: NavigationDestination::Body {
                    body_id: "jupiter".into(),
                },
            })
            .observe(activate_breadcrumb)
            .id();

        app.world_mut().trigger(Activate { entity: jupiter });

        let queued: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(
            queued,
            vec![SimCommand::NavigateBreadcrumb {
                depth: 1,
                target_id: "jupiter".into(),
            }]
        );
    }

    #[test]
    fn breadcrumb_rebuild_restores_focus_to_a_live_semantic_route() {
        let mut navigation = NavigationStack::root();
        navigation.push("jupiter", "Jupiter");
        navigation.push("io", "Io");
        let mut app = test_layout::app(960, 600, 1.0);
        app.insert_resource(UiTheme::default())
            .insert_resource(navigation)
            .add_systems(Startup, spawn_top_bar)
            .add_systems(Update, rebuild_actionable_breadcrumb);
        test_layout::settle(&mut app);

        let jupiter = {
            let world = app.world_mut();
            world
                .query::<(Entity, &BreadcrumbAction)>()
                .iter(world)
                .find_map(|(entity, action)| (action.target_id == "jupiter").then_some(entity))
                .unwrap()
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(jupiter, FocusCause::Navigated);
        app.world_mut()
            .resource_mut::<NavigationStack>()
            .truncate(2);
        test_layout::settle(&mut app);

        let focused = app.world().resource::<InputFocus>().get().unwrap();
        assert_ne!(focused, jupiter);
        assert!(app.world().get_entity(jupiter).is_err());
        assert!(app.world().get_entity(focused).is_ok());
        let action = app
            .world()
            .entity(focused)
            .get::<BreadcrumbAction>()
            .unwrap();
        assert_eq!(action.target_id, "jupiter");
        assert_eq!(
            action.destination,
            NavigationDestination::Body {
                body_id: "jupiter".into(),
            }
        );

        let focused_jupiter = focused;
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(focused_jupiter, FocusCause::Navigated);
        {
            let mut navigation = app.world_mut().resource_mut::<NavigationStack>();
            navigation.truncate(1);
            navigation.push("earth", "Earth");
        }
        test_layout::settle(&mut app);

        let fallback = app.world().resource::<InputFocus>().get().unwrap();
        assert!(app.world().get_entity(fallback).is_ok());
        let action = app
            .world()
            .entity(fallback)
            .get::<BreadcrumbAction>()
            .unwrap();
        assert_eq!(action.target_id, "earth");
    }

    #[test]
    fn stable_navigation_retains_breadcrumb_entity_identity() {
        let mut navigation = NavigationStack::root();
        navigation.push("jupiter", "Jupiter");
        let mut app = test_layout::app(960, 600, 1.0);
        app.insert_resource(UiTheme::default())
            .insert_resource(navigation)
            .add_systems(Startup, spawn_top_bar)
            .add_systems(Update, rebuild_actionable_breadcrumb);
        test_layout::settle(&mut app);
        let root = app
            .world_mut()
            .query_filtered::<Entity, With<BreadcrumbOverlayRoot>>()
            .single(app.world())
            .unwrap();

        test_layout::settle(&mut app);

        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<BreadcrumbOverlayRoot>>()
                .single(app.world())
                .unwrap(),
            root
        );
    }

    #[test]
    fn top_bar_controls_fit_every_required_viewport_and_scale() {
        for (width, height, scale) in test_layout::required_viewports() {
            let mut navigation = NavigationStack::root();
            navigation.push("jupiter", "Jupiter");
            navigation.push_collection("jupiter", "Moons");
            let mut app = test_layout::app(width, height, scale);
            app.insert_resource(UiTheme::default())
                .insert_resource(navigation)
                .add_systems(Startup, spawn_top_bar)
                .add_systems(Update, rebuild_actionable_breadcrumb);
            test_layout::settle(&mut app);

            let world = app.world_mut();
            let root = world
                .query_filtered::<Entity, With<TopBarRoot>>()
                .single(world)
                .unwrap();
            let group = world.get::<TabGroup>(root).unwrap();
            assert_eq!(group.order, 0);
            assert!(!group.modal);
            let root_rect = node_rect(world, root);
            let menu = world
                .query_filtered::<Entity, With<MenuBrowseButton>>()
                .single(world)
                .unwrap();
            let search = world
                .query_filtered::<Entity, With<SearchPlaceholder>>()
                .single(world)
                .unwrap();
            for entity in [menu, search] {
                let rect = node_rect(world, entity);
                assert!(
                    rect.min.x >= root_rect.min.x - 1.0
                        && rect.max.x <= root_rect.max.x + 1.0
                        && rect.min.y >= root_rect.min.y - 1.0
                        && rect.max.y <= root_rect.max.y + 1.0,
                    "{width}×{height} scale {scale}: control {entity:?} {rect:?} escaped {root_rect:?}"
                );
            }
            let host = world
                .query_filtered::<Entity, With<BreadcrumbHost>>()
                .single(world)
                .unwrap();
            assert!(
                world.get::<ComputedNode>(host).unwrap().size().x > 0.0,
                "{width}×{height} scale {scale}: breadcrumb has no reachable viewport"
            );
            assert_eq!(world.get::<TabIndex>(menu), Some(&TabIndex(100)));
            let search_input = world
                .query_filtered::<Entity, With<SearchInput>>()
                .single(world)
                .unwrap();
            assert_eq!(world.get::<TabIndex>(search_input), Some(&TabIndex(101)));
        }
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
}
