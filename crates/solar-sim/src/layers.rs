//! WP11 global layers, right rail, and presentation mode — Rev C §§9.3–9.4.
//!
//! Every persistent visibility choice reduces from `SimCommand` into one
//! bit-stable resource. Render packages read that resource without owning
//! duplicate switches; transient panel-open state remains local UI state.

use crate::control::{SimCommand, SimCommandQueue};
use crate::input_intent::dolly_command;
use crate::ui_kit::{
    checkbox_row, UiTheme, WidgetSpec, WidgetVisualState, INTER_FONT_ASSET, TOP_BAR_HEIGHT_PX,
};
use crate::{SimulationSet, TIME_BAR_HEIGHT_PX};
use bevy::{
    ecs::system::SystemParam,
    input_focus::tab_navigation::TabIndex,
    prelude::*,
    text::LetterSpacing,
    ui::UiSystems,
    ui_widgets::Activate,
    window::{MonitorSelection, PrimaryWindow, WindowMode},
};
use sim_core::catalog::Category;

const RAIL_Z_INDEX: i32 = 92;
const LAYERS_PANEL_Z_INDEX: i32 = 91;
const RESTORE_Z_INDEX: i32 = 120;
const RAIL_BUTTON_SIZE_PX: f32 = 42.0;
const LAYERS_PANEL_WIDTH_PX: f32 = 280.0;

pub const ZOOM_IN_DOLLY_DELTA: f64 = 1.0;
pub const ZOOM_OUT_DOLLY_DELTA: f64 = -1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LayerId {
    UserInterface = 0,
    Planets = 1,
    DwarfPlanets = 2,
    Asteroids = 3,
    Comets = 4,
    Moons = 5,
    Orbits = 6,
    Labels = 7,
    Icons = 8,
}

impl LayerId {
    pub const ALL: [Self; 9] = [
        Self::UserInterface,
        Self::Planets,
        Self::DwarfPlanets,
        Self::Asteroids,
        Self::Comets,
        Self::Moons,
        Self::Orbits,
        Self::Labels,
        Self::Icons,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::UserInterface => "User Interface",
            Self::Planets => "Planets",
            Self::DwarfPlanets => "Dwarf Planets",
            Self::Asteroids => "Asteroids",
            Self::Comets => "Comets",
            Self::Moons => "Moons",
            Self::Orbits => "Orbits",
            Self::Labels => "Labels",
            Self::Icons => "Icons",
        }
    }

    pub const fn replay_slug(self) -> &'static str {
        match self {
            Self::UserInterface => "ui",
            Self::Planets => "planets",
            Self::DwarfPlanets => "dwarf-planets",
            Self::Asteroids => "asteroids",
            Self::Comets => "comets",
            Self::Moons => "moons",
            Self::Orbits => "orbits",
            Self::Labels => "labels",
            Self::Icons => "icons",
        }
    }

    pub fn from_replay_slug(slug: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|layer| layer.replay_slug() == slug)
    }

    const fn bit(self) -> u16 {
        1_u16 << self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerStateSnapshot {
    pub user_interface: bool,
    pub planets: bool,
    pub dwarf_planets: bool,
    pub asteroids: bool,
    pub comets: bool,
    pub moons: bool,
    pub orbits: bool,
    pub labels: bool,
    pub icons: bool,
}

impl LayerStateSnapshot {
    const fn is_visible(self, layer: LayerId) -> bool {
        match layer {
            LayerId::UserInterface => self.user_interface,
            LayerId::Planets => self.planets,
            LayerId::DwarfPlanets => self.dwarf_planets,
            LayerId::Asteroids => self.asteroids,
            LayerId::Comets => self.comets,
            LayerId::Moons => self.moons,
            LayerId::Orbits => self.orbits,
            LayerId::Labels => self.labels,
            LayerId::Icons => self.icons,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerState {
    visible_mask: u16,
}

impl Default for LayerState {
    fn default() -> Self {
        Self {
            visible_mask: (1_u16 << LayerId::ALL.len()) - 1,
        }
    }
}

impl LayerState {
    pub const fn is_visible(self, layer: LayerId) -> bool {
        self.visible_mask & layer.bit() != 0
    }

    /// Returns whether the reducer changed state. Re-applying the same desired
    /// value is deliberately idempotent for duplicate UI/replay delivery.
    pub fn set_visible(&mut self, layer: LayerId, visible: bool) -> bool {
        let before = self.visible_mask;
        if visible {
            self.visible_mask |= layer.bit();
        } else {
            self.visible_mask &= !layer.bit();
        }
        self.visible_mask != before
    }

    pub fn toggle(&mut self, layer: LayerId) -> bool {
        let visible = !self.is_visible(layer);
        self.set_visible(layer, visible);
        visible
    }

    pub fn body_category_visible(self, category: Category) -> bool {
        match category {
            Category::Star => true,
            Category::Planet => self.is_visible(LayerId::Planets),
            Category::DwarfPlanet => self.is_visible(LayerId::DwarfPlanets),
            Category::Asteroid => self.is_visible(LayerId::Asteroids),
            Category::Comet => self.is_visible(LayerId::Comets),
            Category::Moon => self.is_visible(LayerId::Moons),
        }
    }

    pub const fn persistence_snapshot(self) -> LayerStateSnapshot {
        LayerStateSnapshot {
            user_interface: self.is_visible(LayerId::UserInterface),
            planets: self.is_visible(LayerId::Planets),
            dwarf_planets: self.is_visible(LayerId::DwarfPlanets),
            asteroids: self.is_visible(LayerId::Asteroids),
            comets: self.is_visible(LayerId::Comets),
            moons: self.is_visible(LayerId::Moons),
            orbits: self.is_visible(LayerId::Orbits),
            labels: self.is_visible(LayerId::Labels),
            icons: self.is_visible(LayerId::Icons),
        }
    }

    pub fn restore_persistence_snapshot(&mut self, snapshot: LayerStateSnapshot) {
        for layer in LayerId::ALL {
            self.set_visible(layer, snapshot.is_visible(layer));
        }
    }

    pub fn stable_hash(self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for layer in LayerId::ALL {
            hash ^= u64::from(layer as u8);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            hash ^= u64::from(self.is_visible(layer));
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }
}

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PresentationState {
    fullscreen: bool,
    settings_requested: bool,
}

impl PresentationState {
    pub(crate) const fn with_fullscreen(fullscreen: bool) -> Self {
        Self {
            fullscreen,
            settings_requested: false,
        }
    }

    pub const fn is_fullscreen(self) -> bool {
        self.fullscreen
    }

    pub const fn settings_requested(self) -> bool {
        self.settings_requested
    }

    pub fn take_settings_request(&mut self) -> bool {
        std::mem::take(&mut self.settings_requested)
    }

    pub(crate) fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
    }

    pub(crate) fn request_settings(&mut self) {
        self.settings_requested = true;
    }
}

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
pub struct HudSurface;

#[derive(Component, Debug, Clone, Copy)]
pub struct RightRailRoot;

#[derive(Component, Debug, Clone, Copy)]
pub struct LayersPanelRoot;

#[derive(Component, Debug, Clone, Copy)]
pub struct UiRestoreAffordance;

#[derive(Component, Debug, Clone, Copy)]
struct LayerToggle(LayerId);

#[derive(Component, Debug, Clone, Copy)]
struct LayerGroupSeparator;

#[derive(Component, Debug, Clone, Copy)]
enum RailAction {
    Zoom(f64),
    ToggleLayersPanel,
    ToggleFullscreen,
    OpenSettings,
}

#[derive(Resource, Debug, Clone, Copy)]
struct RailUiState {
    layers_panel_open: bool,
    dirty: bool,
}

#[derive(SystemParam)]
struct RailRenderParams<'w, 's> {
    theme: Res<'w, UiTheme>,
    asset_server: Res<'w, AssetServer>,
    layers: Res<'w, LayerState>,
    presentation: Res<'w, PresentationState>,
    ui_state: ResMut<'w, RailUiState>,
    rail_roots: Query<'w, 's, Entity, With<RightRailRoot>>,
    panel_roots: Query<'w, 's, Entity, With<LayersPanelRoot>>,
}

struct RailButtonSpec<'a> {
    glyph: &'a str,
    accessible_label: &'a str,
    action: RailAction,
    tab_index: i32,
}

impl Default for RailUiState {
    fn default() -> Self {
        Self {
            layers_panel_open: false,
            dirty: true,
        }
    }
}

pub struct LayersPlugin;

impl Plugin for LayersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LayerState>()
            .init_resource::<PresentationState>()
            .init_resource::<RailUiState>()
            .add_systems(Startup, spawn_restore_affordance)
            .add_systems(
                Update,
                (rebuild_right_rail, sync_window_mode)
                    .chain()
                    .in_set(SimulationSet::Render),
            )
            .add_systems(PostUpdate, sync_hud_visibility.before(UiSystems::Prepare));
    }
}

fn spawn_restore_affordance(
    mut commands: Commands,
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
) {
    let button = commands
        .spawn((
            Name::new("Restore user interface"),
            UiRestoreAffordance,
            bevy::ui_widgets::Button,
            AccessibleLabel::new("Restore user interface"),
            TabIndex(1),
            Node {
                position_type: PositionType::Absolute,
                right: px(theme.spacing.md_px),
                top: px(theme.spacing.md_px),
                height: px(34),
                padding: UiRect::horizontal(px(theme.spacing.md_px)),
                border: UiRect::all(px(theme.spacing.hairline_px)),
                border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme.colors.panel_elevated.color()),
            BorderColor::all(theme.colors.accent.color()),
            GlobalZIndex(RESTORE_Z_INDEX),
            Visibility::Hidden,
        ))
        .observe(restore_user_interface)
        .id();
    commands.spawn((
        Text::new("SHOW UI"),
        TextFont {
            font: asset_server.load(INTER_FONT_ASSET).into(),
            font_size: theme.type_scale.label_px.into(),
            ..default()
        },
        TextColor(theme.colors.text_primary.color()),
        LetterSpacing::Px(theme.type_scale.uppercase_tracking_px),
        Pickable::IGNORE,
        ChildOf(button),
    ));
}

fn rebuild_right_rail(mut commands: Commands, params: RailRenderParams) {
    let RailRenderParams {
        theme,
        asset_server,
        layers,
        presentation,
        mut ui_state,
        rail_roots,
        panel_roots,
    } = params;
    if !ui_state.dirty && !layers.is_changed() && !presentation.is_changed() {
        return;
    }
    for entity in &rail_roots {
        commands.entity(entity).despawn();
    }
    for entity in &panel_roots {
        commands.entity(entity).despawn();
    }

    let rail = commands
        .spawn((
            Name::new("Right rail"),
            RightRailRoot,
            HudSurface,
            AccessibleLabel::new("View controls"),
            Node {
                position_type: PositionType::Absolute,
                right: px(theme.spacing.lg_px),
                top: px(TOP_BAR_HEIGHT_PX + theme.spacing.lg_px),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.spacing.sm_px),
                ..default()
            },
            GlobalZIndex(RAIL_Z_INDEX),
        ))
        .id();
    spawn_rail_button(
        &mut commands,
        rail,
        *theme,
        &asset_server,
        RailButtonSpec {
            glyph: "+",
            accessible_label: "Zoom in",
            action: RailAction::Zoom(ZOOM_IN_DOLLY_DELTA),
            tab_index: 20,
        },
    );
    spawn_rail_button(
        &mut commands,
        rail,
        *theme,
        &asset_server,
        RailButtonSpec {
            glyph: "-",
            accessible_label: "Zoom out",
            action: RailAction::Zoom(ZOOM_OUT_DOLLY_DELTA),
            tab_index: 21,
        },
    );
    spawn_rail_button(
        &mut commands,
        rail,
        *theme,
        &asset_server,
        RailButtonSpec {
            glyph: "L",
            accessible_label: "Toggle layers panel",
            action: RailAction::ToggleLayersPanel,
            tab_index: 22,
        },
    );
    spawn_rail_button(
        &mut commands,
        rail,
        *theme,
        &asset_server,
        RailButtonSpec {
            glyph: if presentation.is_fullscreen() {
                "EX"
            } else {
                "FS"
            },
            accessible_label: if presentation.is_fullscreen() {
                "Exit fullscreen"
            } else {
                "Enter fullscreen"
            },
            action: RailAction::ToggleFullscreen,
            tab_index: 23,
        },
    );
    spawn_rail_button(
        &mut commands,
        rail,
        *theme,
        &asset_server,
        RailButtonSpec {
            glyph: "S",
            accessible_label: "Open settings",
            action: RailAction::OpenSettings,
            tab_index: 24,
        },
    );

    if ui_state.layers_panel_open {
        spawn_layers_panel(&mut commands, *theme, &asset_server, &layers);
    }
    ui_state.dirty = false;
}

fn spawn_rail_button(
    commands: &mut Commands,
    parent: Entity,
    theme: UiTheme,
    asset_server: &AssetServer,
    spec: RailButtonSpec<'_>,
) {
    let button = commands
        .spawn((
            bevy::ui_widgets::Button,
            spec.action,
            AccessibleLabel::new(spec.accessible_label),
            TabIndex(spec.tab_index),
            Node {
                width: px(RAIL_BUTTON_SIZE_PX),
                height: px(RAIL_BUTTON_SIZE_PX),
                border: UiRect::all(px(theme.spacing.hairline_px)),
                border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme.colors.panel.color()),
            BorderColor::all(theme.colors.separator.color()),
            ChildOf(parent),
        ))
        .observe(activate_rail_action)
        .id();
    commands.spawn((
        Text::new(spec.glyph),
        TextFont {
            font: asset_server.load(INTER_FONT_ASSET).into(),
            font_size: theme.type_scale.title_px.into(),
            ..default()
        },
        TextColor(theme.colors.text_primary.color()),
        Pickable::IGNORE,
        ChildOf(button),
    ));
}

fn spawn_layers_panel(
    commands: &mut Commands,
    theme: UiTheme,
    asset_server: &AssetServer,
    layers: &LayerState,
) {
    let panel = commands
        .spawn((
            Name::new("Layers quick panel"),
            LayersPanelRoot,
            HudSurface,
            AccessibleLabel::new("Layers quick panel"),
            Node {
                position_type: PositionType::Absolute,
                right: px(theme.spacing.lg_px * 2.0 + RAIL_BUTTON_SIZE_PX),
                bottom: px(TIME_BAR_HEIGHT_PX + theme.spacing.lg_px),
                width: px(LAYERS_PANEL_WIDTH_PX),
                padding: UiRect::all(px(theme.spacing.md_px)),
                border: UiRect::all(px(theme.spacing.hairline_px)),
                border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.spacing.sm_px),
                ..default()
            },
            BackgroundColor(theme.colors.panel_elevated.color()),
            BorderColor::all(theme.colors.separator.color()),
            GlobalZIndex(LAYERS_PANEL_Z_INDEX),
        ))
        .id();
    commands.spawn((
        Text::new("LAYERS"),
        TextFont {
            font: asset_server.load(INTER_FONT_ASSET).into(),
            font_size: theme.type_scale.label_px.into(),
            ..default()
        },
        TextColor(theme.colors.text_primary.color()),
        LetterSpacing::Px(theme.type_scale.uppercase_tracking_px),
        Pickable::IGNORE,
        ChildOf(panel),
    ));

    let groups: [&[LayerId]; 4] = [
        &[LayerId::UserInterface],
        &[
            LayerId::Planets,
            LayerId::DwarfPlanets,
            LayerId::Asteroids,
            LayerId::Comets,
        ],
        &[LayerId::Moons],
        &[LayerId::Orbits, LayerId::Labels, LayerId::Icons],
    ];
    let mut tab_index = 30;
    for (group_index, group) in groups.into_iter().enumerate() {
        if group_index > 0 {
            commands.spawn((
                LayerGroupSeparator,
                Node {
                    width: percent(100),
                    height: px(theme.spacing.hairline_px),
                    ..default()
                },
                BackgroundColor(theme.colors.separator.color()),
                Pickable::IGNORE,
                ChildOf(panel),
            ));
        }
        for layer in group {
            let state = if layers.is_visible(*layer) {
                WidgetVisualState::Active
            } else {
                WidgetVisualState::Default
            };
            commands
                .spawn_scene(checkbox_row(
                    theme,
                    WidgetSpec::new(
                        layer.label(),
                        format!("Toggle {} layer", layer.label()),
                        state,
                    ),
                ))
                .insert((LayerToggle(*layer), TabIndex(tab_index), ChildOf(panel)))
                .observe(activate_layer_toggle);
            tab_index += 1;
        }
    }
}

fn activate_rail_action(
    activate: On<Activate>,
    actions: Query<&RailAction>,
    mut ui_state: ResMut<RailUiState>,
    mut commands: ResMut<SimCommandQueue>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    match *action {
        RailAction::Zoom(delta) => commands.push(dolly_command(delta)),
        RailAction::ToggleLayersPanel => {
            ui_state.layers_panel_open = !ui_state.layers_panel_open;
            ui_state.dirty = true;
        }
        RailAction::ToggleFullscreen => commands.push(SimCommand::ToggleFullscreen),
        RailAction::OpenSettings => commands.push(SimCommand::OpenSettings),
    }
}

fn activate_layer_toggle(
    activate: On<Activate>,
    toggles: Query<&LayerToggle>,
    layers: Res<LayerState>,
    mut commands: ResMut<SimCommandQueue>,
) {
    let Ok(toggle) = toggles.get(activate.entity) else {
        return;
    };
    commands.push(SimCommand::SetLayerVisibility {
        layer: toggle.0,
        visible: !layers.is_visible(toggle.0),
    });
}

fn restore_user_interface(_activate: On<Activate>, mut commands: ResMut<SimCommandQueue>) {
    commands.push(SimCommand::SetLayerVisibility {
        layer: LayerId::UserInterface,
        visible: true,
    });
}

fn sync_hud_visibility(
    layers: Res<LayerState>,
    mut hud: Query<&mut Visibility, (With<HudSurface>, Without<UiRestoreAffordance>)>,
    mut restore: Query<&mut Visibility, With<UiRestoreAffordance>>,
) {
    let ui_visible = layers.is_visible(LayerId::UserInterface);
    for mut visibility in &mut hud {
        *visibility = if ui_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for mut visibility in &mut restore {
        *visibility = if ui_visible {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

fn sync_window_mode(
    presentation: Res<PresentationState>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !presentation.is_changed() {
        return;
    }
    let desired = if presentation.is_fullscreen() {
        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
    } else {
        WindowMode::Windowed
    };
    for mut window in &mut windows {
        window.mode = desired;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::consume_presentation_command;
    use crate::WidgetRoot;
    use bevy::{
        app::TaskPoolPlugin,
        asset::{AssetApp, AssetPlugin},
        scene::ScenePlugin,
        text::Font,
    };

    fn reduce_queued_layer_commands(
        mut commands: ResMut<SimCommandQueue>,
        mut layers: ResMut<LayerState>,
        mut presentation: ResMut<PresentationState>,
    ) {
        for command in commands.drain() {
            consume_presentation_command(&command, &mut layers, &mut presentation);
        }
    }

    #[test]
    fn reducer_sets_idempotently_and_keeps_groups_independent() {
        assert_eq!(
            LayerId::ALL.map(LayerId::label),
            [
                "User Interface",
                "Planets",
                "Dwarf Planets",
                "Asteroids",
                "Comets",
                "Moons",
                "Orbits",
                "Labels",
                "Icons",
            ]
        );
        let mut state = LayerState::default();
        assert!(state.set_visible(LayerId::Planets, false));
        let once = state;
        assert!(!state.set_visible(LayerId::Planets, false));
        assert_eq!(state, once);
        assert!(!state.is_visible(LayerId::Planets));
        assert!(state.is_visible(LayerId::DwarfPlanets));
        assert!(state.is_visible(LayerId::Orbits));

        assert!(!state.toggle(LayerId::Labels));
        assert!(state.toggle(LayerId::Labels));
        assert!(state.is_visible(LayerId::Labels));

        let snapshot = state.persistence_snapshot();
        let mut restored = LayerState::default();
        restored.set_visible(LayerId::Comets, false);
        restored.restore_persistence_snapshot(snapshot);
        assert_eq!(restored, state);
        assert_eq!(restored.stable_hash(), state.stable_hash());
    }

    #[test]
    fn ui_off_hides_every_hud_surface_and_only_shows_restore() {
        let mut app = App::new();
        let mut layers = LayerState::default();
        layers.set_visible(LayerId::Orbits, false);
        let expected_after_restore = layers;
        app.insert_resource(layers)
            .init_resource::<SimCommandQueue>()
            .init_resource::<PresentationState>()
            .insert_resource(RailUiState {
                layers_panel_open: true,
                dirty: false,
            })
            .add_systems(
                Update,
                (reduce_queued_layer_commands, sync_hud_visibility).chain(),
            );
        let first = app
            .world_mut()
            .spawn((HudSurface, Visibility::Visible))
            .id();
        let second = app
            .world_mut()
            .spawn((HudSurface, Visibility::Visible))
            .id();
        let restore = app
            .world_mut()
            .spawn((UiRestoreAffordance, Visibility::Hidden))
            .observe(restore_user_interface)
            .id();

        app.world_mut()
            .resource_mut::<LayerState>()
            .set_visible(LayerId::UserInterface, false);
        app.update();
        assert!(!app
            .world()
            .resource::<LayerState>()
            .is_visible(LayerId::Orbits));
        assert!(app.world().resource::<RailUiState>().layers_panel_open);
        assert_eq!(
            app.world().entity(first).get::<Visibility>(),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().entity(second).get::<Visibility>(),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().entity(restore).get::<Visibility>(),
            Some(&Visibility::Visible)
        );

        app.world_mut().trigger(Activate { entity: restore });
        app.update();
        assert_eq!(
            *app.world().resource::<LayerState>(),
            expected_after_restore
        );
        assert!(app.world().resource::<RailUiState>().layers_panel_open);
        assert_eq!(
            app.world().entity(first).get::<Visibility>(),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            app.world().entity(second).get::<Visibility>(),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            app.world().entity(restore).get::<Visibility>(),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn every_toggle_command_reduces_into_panel_state_within_one_update() {
        let mut app = App::new();
        app.init_resource::<SimCommandQueue>()
            .init_resource::<LayerState>()
            .init_resource::<PresentationState>()
            .add_systems(Update, reduce_queued_layer_commands);
        let toggles: Vec<_> = LayerId::ALL
            .into_iter()
            .map(|layer| {
                app.world_mut()
                    .spawn(LayerToggle(layer))
                    .observe(activate_layer_toggle)
                    .id()
            })
            .collect();

        for entity in &toggles {
            app.world_mut().trigger(Activate { entity: *entity });
        }
        app.update();
        for layer in LayerId::ALL {
            assert!(!app.world().resource::<LayerState>().is_visible(layer));
        }

        for entity in toggles {
            app.world_mut().trigger(Activate { entity });
        }
        app.update();
        assert_eq!(*app.world().resource::<LayerState>(), LayerState::default());
    }

    fn spawn_panel_fixture(
        mut commands: Commands,
        theme: Res<UiTheme>,
        asset_server: Res<AssetServer>,
        layers: Res<LayerState>,
    ) {
        spawn_layers_panel(&mut commands, *theme, &asset_server, &layers);
    }

    #[test]
    fn quick_panel_has_exact_grouping_accessibility_and_reducer_state() {
        let mut layers = LayerState::default();
        layers.set_visible(LayerId::Asteroids, false);
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ))
        .init_asset::<Font>()
        .insert_resource(UiTheme::default())
        .insert_resource(layers)
        .add_systems(Startup, spawn_panel_fixture);
        app.update();

        let world = app.world_mut();
        let rows: Vec<_> = world
            .query::<(&LayerToggle, &WidgetRoot, &AccessibleLabel)>()
            .iter(world)
            .map(|(toggle, root, _)| (toggle.0, root.state))
            .collect();
        assert_eq!(rows.len(), LayerId::ALL.len());
        for layer in LayerId::ALL {
            let state = rows
                .iter()
                .find_map(|(candidate, state)| (*candidate == layer).then_some(*state))
                .unwrap();
            assert_eq!(
                state,
                if layer == LayerId::Asteroids {
                    WidgetVisualState::Default
                } else {
                    WidgetVisualState::Active
                }
            );
        }
        assert_eq!(world.query::<&LayerGroupSeparator>().iter(world).count(), 3);
    }

    #[test]
    fn rail_actions_enqueue_commands_and_preserve_transient_panel_layout() {
        let mut app = App::new();
        app.init_resource::<SimCommandQueue>()
            .init_resource::<LayerState>()
            .init_resource::<RailUiState>();
        let zoom = app
            .world_mut()
            .spawn(RailAction::Zoom(ZOOM_IN_DOLLY_DELTA))
            .observe(activate_rail_action)
            .id();
        let fullscreen = app
            .world_mut()
            .spawn(RailAction::ToggleFullscreen)
            .observe(activate_rail_action)
            .id();
        let settings = app
            .world_mut()
            .spawn(RailAction::OpenSettings)
            .observe(activate_rail_action)
            .id();
        let panel = app
            .world_mut()
            .spawn(RailAction::ToggleLayersPanel)
            .observe(activate_rail_action)
            .id();

        app.world_mut().trigger(Activate { entity: zoom });
        app.world_mut().trigger(Activate { entity: fullscreen });
        app.world_mut().trigger(Activate { entity: settings });
        app.world_mut().trigger(Activate { entity: panel });
        let commands: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(
            commands,
            vec![
                dolly_command(ZOOM_IN_DOLLY_DELTA),
                SimCommand::ToggleFullscreen,
                SimCommand::OpenSettings,
            ]
        );
        assert!(app.world().resource::<RailUiState>().layers_panel_open);
    }

    #[test]
    fn presentation_reducer_exposes_fullscreen_and_wp14_settings_hooks() {
        let mut layers = LayerState::default();
        let mut presentation = PresentationState::default();
        consume_presentation_command(
            &SimCommand::ToggleFullscreen,
            &mut layers,
            &mut presentation,
        );
        assert!(presentation.is_fullscreen());
        consume_presentation_command(&SimCommand::OpenSettings, &mut layers, &mut presentation);
        assert!(presentation.settings_requested());
        assert!(presentation.take_settings_request());
        assert!(!presentation.take_settings_request());
        consume_presentation_command(
            &SimCommand::ToggleFullscreen,
            &mut layers,
            &mut presentation,
        );
        assert!(!presentation.is_fullscreen());
    }

    #[test]
    fn fullscreen_state_updates_the_primary_window_mode() {
        let mut app = App::new();
        app.insert_resource(PresentationState::default())
            .add_systems(Update, sync_window_mode);
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        app.update();
        assert_eq!(
            app.world().entity(window).get::<Window>().unwrap().mode,
            WindowMode::Windowed
        );

        app.world_mut()
            .resource_mut::<PresentationState>()
            .toggle_fullscreen();
        app.update();
        assert!(matches!(
            app.world().entity(window).get::<Window>().unwrap().mode,
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        ));
    }
}
