//! WP11 global layers, right rail, and presentation mode — Rev C §§9.3–9.4.
//!
//! Every application-visible layer and panel choice reduces from `SimCommand`
//! into shared deterministic state. Render packages read those resources
//! without owning duplicate switches; only scroll and focus mechanics remain
//! local to the retained UI surface.

use crate::control::{SimCommand, SimCommandQueue};
use crate::input_intent::{dolly_command, ModalSurfaceSet, UiScrollSurface};
use crate::search::{BrowseMenuRoot, BrowseUiState, SEARCH_DROPDOWN_Z_INDEX};
use crate::settings::SettingsScreenRoot;
use crate::ui_kit::{
    checkbox_row, UiTheme, WidgetSpec, WidgetVisualState, INTER_FONT_ASSET, TOP_BAR_HEIGHT_PX,
};
use crate::{SimulationSet, TIME_BAR_HEIGHT_PX};
#[cfg(test)]
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};
use bevy::{
    ecs::system::SystemParam,
    input::mouse::MouseScrollUnit,
    input_focus::{
        tab_navigation::{TabGroup, TabIndex},
        FocusCause, InputFocus,
    },
    prelude::*,
    text::LetterSpacing,
    ui::UiSystems,
    ui_widgets::Activate,
};
use sim_core::catalog::Category;

const RAIL_Z_INDEX: i32 = 92;
const LAYERS_PANEL_Z_INDEX: i32 = 91;
const RESTORE_Z_INDEX: i32 = 120;
// Recovery must remain visible over the ordinary HUD without competing with
// transient Search or modal surfaces for either pixels or pointer ownership.
const CUE_RECOVERY_Z_INDEX: i32 = SEARCH_DROPDOWN_Z_INDEX - 1;
const RAIL_BUTTON_SIZE_PX: f32 = 42.0;
const LAYERS_PANEL_WIDTH_PX: f32 = 280.0;
const RAIL_TAB_GROUP_ORDER: i32 = 30;
const LAYERS_PANEL_TAB_GROUP_ORDER: i32 = 31;
const UI_RESTORE_TAB_INDEX: i32 = 0;
const DISABLED_TAB_INDEX: i32 = -1;

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
    settings_open: bool,
    layers_panel_open: bool,
}

impl PresentationState {
    pub(crate) const fn with_fullscreen(fullscreen: bool) -> Self {
        Self {
            fullscreen,
            settings_open: false,
            layers_panel_open: false,
        }
    }

    pub const fn is_fullscreen(self) -> bool {
        self.fullscreen
    }

    pub const fn is_settings_open(self) -> bool {
        self.settings_open
    }

    pub const fn is_layers_panel_open(self) -> bool {
        self.layers_panel_open
    }

    pub(crate) fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
    }

    pub(crate) fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
    }

    pub(crate) fn open_settings(&mut self) {
        self.settings_open = true;
    }

    pub(crate) fn close_settings(&mut self) {
        self.settings_open = false;
    }

    pub(crate) fn set_layers_panel_open(&mut self, open: bool) {
        self.layers_panel_open = open;
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
struct UiRestoreTabGroup;

#[derive(Component, Debug, Clone, Copy)]
pub struct VisualCueRecoveryRoot;

#[derive(Component, Debug, Clone, Copy)]
struct LayerToggle(LayerId);

#[derive(Component, Debug, Clone, Copy)]
struct LayerGroupSeparator;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
enum RailAction {
    Zoom(f64),
    ToggleLayersPanel,
    ToggleFullscreen,
    OpenSettings,
}

#[derive(Resource, Debug, Clone, Copy)]
struct RailUiState {
    rail_scroll_y: f32,
    layers_scroll_y: f32,
    restore_focus: Option<RailFocusTarget>,
    rendered_layers: Option<LayerState>,
    rendered_fullscreen: Option<bool>,
    rendered_layers_panel_open: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RailFocusTarget {
    Action(RailAction),
    Layer(LayerId),
}

#[derive(SystemParam)]
struct RailRenderParams<'w, 's> {
    theme: Res<'w, UiTheme>,
    asset_server: Res<'w, AssetServer>,
    layers: Res<'w, LayerState>,
    presentation: Res<'w, PresentationState>,
    ui_state: ResMut<'w, RailUiState>,
    rail_roots: Query<'w, 's, (Entity, &'static ScrollPosition), With<RightRailRoot>>,
    panel_roots: Query<'w, 's, (Entity, &'static ScrollPosition), With<LayersPanelRoot>>,
    focus: Res<'w, InputFocus>,
    rail_actions: Query<'w, 's, &'static RailAction>,
    layer_toggles: Query<'w, 's, &'static LayerToggle>,
}

struct RailButtonSpec<'a> {
    glyph: &'a str,
    accessible_label: &'a str,
    action: RailAction,
    tab_index: i32,
}

#[derive(SystemParam)]
struct CueRecoveryParams<'w, 's> {
    layers: Res<'w, LayerState>,
    theme: Res<'w, UiTheme>,
    asset_server: Res<'w, AssetServer>,
    roots: Query<'w, 's, Entity, With<VisualCueRecoveryRoot>>,
    browse: Option<Res<'w, BrowseUiState>>,
    presentation: Option<Res<'w, PresentationState>>,
    browse_roots: Query<'w, 's, Entity, With<BrowseMenuRoot>>,
    settings_roots: Query<'w, 's, Entity, With<SettingsScreenRoot>>,
    tab_indices: Query<'w, 's, (Entity, &'static TabIndex), Without<UiRestoreAffordance>>,
    parents: Query<'w, 's, &'static ChildOf>,
    rail_actions: Query<'w, 's, (Entity, &'static RailAction)>,
    focus: ResMut<'w, InputFocus>,
    ui_state: ResMut<'w, RailUiState>,
}

impl Default for RailUiState {
    fn default() -> Self {
        Self {
            rail_scroll_y: 0.0,
            layers_scroll_y: 0.0,
            restore_focus: None,
            rendered_layers: None,
            rendered_fullscreen: None,
            rendered_layers_panel_open: None,
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
                (sync_visual_cue_recovery, rebuild_right_rail)
                    .chain()
                    .after(ModalSurfaceSet::Rebuild)
                    .before(ModalSurfaceSet::Focus)
                    .in_set(SimulationSet::Render),
            )
            .add_systems(PostUpdate, sync_hud_visibility.before(UiSystems::Prepare));
    }
}

fn sync_visual_cue_recovery(mut commands: Commands, params: CueRecoveryParams) {
    let CueRecoveryParams {
        layers,
        theme,
        asset_server,
        roots,
        browse,
        presentation,
        browse_roots,
        settings_roots,
        tab_indices,
        parents,
        rail_actions,
        mut focus,
        mut ui_state,
    } = params;
    let needs_recovery = visual_cue_recovery_needed(*layers);
    if !needs_recovery {
        let focused_recovery = focus.get().is_some_and(|focused| {
            roots
                .iter()
                .any(|root| is_descendant_of(focused, root, &parents))
        });
        if focused_recovery && layers.is_visible(LayerId::UserInterface) {
            let browse_open = browse.as_deref().is_some_and(BrowseUiState::is_open);
            let settings_open = presentation
                .as_deref()
                .is_some_and(|presentation| presentation.is_settings_open());
            let modal_root = if browse_open {
                browse_roots.single().ok()
            } else if settings_open {
                settings_roots.single().ok()
            } else {
                None
            };
            let modal_focus =
                modal_root.and_then(|root| first_tabbable_descendant(root, &tab_indices, &parents));
            if let Some(destination) = modal_focus {
                focus.set(destination, FocusCause::Navigated);
            } else if browse_open || settings_open {
                // A modal rebuilt later in this frame will seed its own focus.
                // Clear the doomed recovery entity until that canonical pass.
                focus.clear();
            } else {
                ui_state.restore_focus =
                    Some(RailFocusTarget::Action(RailAction::ToggleLayersPanel));
                if let Some(destination) = rail_actions.iter().find_map(|(entity, action)| {
                    (*action == RailAction::ToggleLayersPanel).then_some(entity)
                }) {
                    // Keep focus live before deferred cue teardown, then let
                    // the semantic target survive any same-frame rail rebuild.
                    focus.set(destination, FocusCause::Navigated);
                } else {
                    focus.clear();
                }
            }
        }
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    }
    if !roots.is_empty() {
        return;
    }

    let root = commands
        .spawn((
            Name::new("Hidden visual cues recovery"),
            VisualCueRecoveryRoot,
            HudSurface,
            TabGroup::new(32),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let button = commands
        .spawn((
            Name::new("Restore hidden visual cues"),
            bevy::ui_widgets::Button,
            AccessibleLabel::new(
                "Orbit, label, and icon cues are hidden. Restore presentation defaults.",
            ),
            TabIndex(0),
            Node {
                position_type: PositionType::Absolute,
                left: px(theme.spacing.lg_px),
                right: px(theme.spacing.lg_px),
                top: px(TOP_BAR_HEIGHT_PX + theme.spacing.lg_px),
                min_height: px(42),
                padding: UiRect::horizontal(px(theme.spacing.lg_px)),
                border: UiRect::all(px(theme.spacing.hairline_px)),
                border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme.colors.panel_elevated.color()),
            BorderColor::all(theme.colors.accent.color()),
            GlobalZIndex(CUE_RECOVERY_Z_INDEX),
            ChildOf(root),
        ))
        .observe(restore_presentation_defaults)
        .id();
    commands.spawn((
        Text::new("VIEW CUES HIDDEN  ·  RESTORE DEFAULT VIEW"),
        TextFont {
            font: asset_server.load(INTER_FONT_ASSET).into(),
            font_size: theme.type_scale.caption_px.into(),
            ..default()
        },
        TextColor(theme.colors.text_primary.color()),
        LetterSpacing::Px(theme.type_scale.uppercase_tracking_px),
        Pickable::IGNORE,
        ChildOf(button),
    ));
}

pub fn visual_cue_recovery_needed(layers: LayerState) -> bool {
    layers.is_visible(LayerId::UserInterface)
        && !layers.is_visible(LayerId::Orbits)
        && !layers.is_visible(LayerId::Labels)
        && !layers.is_visible(LayerId::Icons)
}

fn spawn_restore_affordance(
    mut commands: Commands,
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
) {
    // Bevy tab navigation does not filter hidden descendants. Start this
    // retained group disabled and activate it only while UI-off is canonical.
    let root = commands
        .spawn((
            Name::new("Restore user interface focus group"),
            UiRestoreTabGroup,
            TabGroup::new(0),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                ..default()
            },
            Pickable::IGNORE,
            Visibility::Hidden,
        ))
        .id();
    let button = commands
        .spawn((
            Name::new("Restore user interface"),
            UiRestoreAffordance,
            bevy::ui_widgets::Button,
            AccessibleLabel::new("Restore user interface"),
            TabIndex(DISABLED_TAB_INDEX),
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
            ChildOf(root),
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
        focus,
        rail_actions,
        layer_toggles,
    } = params;
    let render_inputs_changed = ui_state.rendered_layers != Some(*layers)
        || ui_state.rendered_fullscreen != Some(presentation.is_fullscreen())
        || ui_state.rendered_layers_panel_open != Some(presentation.is_layers_panel_open());
    if !render_inputs_changed {
        if layers.is_visible(LayerId::UserInterface)
            && presentation.is_changed()
            && !presentation.is_settings_open()
        {
            if let Some(target) = ui_state.restore_focus.take() {
                queue_rail_focus_restore(&mut commands, target);
            }
        }
        return;
    }
    ui_state.rendered_layers = Some(*layers);
    ui_state.rendered_fullscreen = Some(presentation.is_fullscreen());
    ui_state.rendered_layers_panel_open = Some(presentation.is_layers_panel_open());
    if let Some(focused) = focus.get() {
        if let Ok(action) = rail_actions.get(focused) {
            ui_state.restore_focus = Some(RailFocusTarget::Action(*action));
        } else if let Ok(toggle) = layer_toggles.get(focused) {
            ui_state.restore_focus = Some(RailFocusTarget::Layer(toggle.0));
        }
    }
    for (entity, position) in &rail_roots {
        ui_state.rail_scroll_y = position.y;
        commands.entity(entity).despawn();
    }
    for (entity, position) in &panel_roots {
        ui_state.layers_scroll_y = position.y;
        commands.entity(entity).despawn();
    }

    let rail = commands
        .spawn((
            Name::new("Right rail"),
            RightRailRoot,
            HudSurface,
            AccessibleLabel::new("View controls"),
            TabGroup::new(RAIL_TAB_GROUP_ORDER),
            UiScrollSurface,
            Node {
                position_type: PositionType::Absolute,
                right: px(theme.spacing.lg_px),
                top: px(TOP_BAR_HEIGHT_PX + theme.spacing.lg_px),
                bottom: px(TIME_BAR_HEIGHT_PX + theme.spacing.lg_px),
                width: px(RAIL_BUTTON_SIZE_PX),
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.spacing.sm_px),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition(Vec2::new(0.0, ui_state.rail_scroll_y)),
            GlobalZIndex(RAIL_Z_INDEX),
        ))
        .observe(scroll_right_rail)
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
            tab_index: 0,
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
            tab_index: 1,
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
            tab_index: 2,
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
            tab_index: 3,
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
            tab_index: 4,
        },
    );

    if presentation.is_layers_panel_open() {
        spawn_layers_panel(
            &mut commands,
            *theme,
            &asset_server,
            &layers,
            ui_state.layers_scroll_y,
        );
    }
    if layers.is_visible(LayerId::UserInterface) && !presentation.is_settings_open() {
        if let Some(target) = ui_state.restore_focus.take() {
            queue_rail_focus_restore(&mut commands, target);
        }
    }
}

fn queue_rail_focus_restore(commands: &mut Commands, target: RailFocusTarget) {
    commands.queue(move |world: &mut World| {
        let focused = match target {
            RailFocusTarget::Action(action) => {
                let mut actions = world.query::<(Entity, &RailAction)>();
                actions
                    .iter(world)
                    .find_map(|(entity, candidate)| (*candidate == action).then_some(entity))
            }
            RailFocusTarget::Layer(layer) => {
                let mut toggles = world.query::<(Entity, &LayerToggle)>();
                let exact = toggles
                    .iter(world)
                    .find_map(|(entity, candidate)| (candidate.0 == layer).then_some(entity));
                exact.or_else(|| {
                    let mut actions = world.query::<(Entity, &RailAction)>();
                    actions.iter(world).find_map(|(entity, candidate)| {
                        (*candidate == RailAction::ToggleLayersPanel).then_some(entity)
                    })
                })
            }
        };
        if let Some(entity) = focused {
            world
                .resource_mut::<InputFocus>()
                .set(entity, FocusCause::Navigated);
        }
    });
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
    scroll_y: f32,
) {
    let panel = commands
        .spawn((
            Name::new("Layers quick panel"),
            LayersPanelRoot,
            HudSurface,
            AccessibleLabel::new("Layers quick panel"),
            TabGroup::new(LAYERS_PANEL_TAB_GROUP_ORDER),
            UiScrollSurface,
            Node {
                position_type: PositionType::Absolute,
                right: px(theme.spacing.lg_px * 2.0 + RAIL_BUTTON_SIZE_PX),
                top: px(TOP_BAR_HEIGHT_PX + theme.spacing.lg_px),
                bottom: px(TIME_BAR_HEIGHT_PX + theme.spacing.lg_px),
                width: px(LAYERS_PANEL_WIDTH_PX),
                min_height: px(0),
                padding: UiRect::all(px(theme.spacing.md_px)),
                border: UiRect::all(px(theme.spacing.hairline_px)),
                border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.spacing.sm_px),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition(Vec2::new(0.0, scroll_y)),
            BackgroundColor(theme.colors.panel_elevated.color()),
            BorderColor::all(theme.colors.separator.color()),
            GlobalZIndex(LAYERS_PANEL_Z_INDEX),
        ))
        .observe(scroll_layers_panel)
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
    let mut tab_index = 0;
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

fn scroll_right_rail(
    mut scroll: On<Pointer<Scroll>>,
    mut rails: Query<(&mut ScrollPosition, &ComputedNode), With<RightRailRoot>>,
    mut state: ResMut<RailUiState>,
) {
    let Ok((mut position, node)) = rails.get_mut(scroll.entity) else {
        return;
    };
    position.y = next_surface_scroll_y(
        position.y,
        scroll.y,
        scroll.unit,
        node.content_size().y,
        node.size().y,
        node.inverse_scale_factor,
    );
    state.rail_scroll_y = position.y;
    scroll.propagate(false);
}

fn scroll_layers_panel(
    mut scroll: On<Pointer<Scroll>>,
    mut panels: Query<(&mut ScrollPosition, &ComputedNode), With<LayersPanelRoot>>,
    mut state: ResMut<RailUiState>,
) {
    let Ok((mut position, node)) = panels.get_mut(scroll.entity) else {
        return;
    };
    position.y = next_surface_scroll_y(
        position.y,
        scroll.y,
        scroll.unit,
        node.content_size().y,
        node.size().y,
        node.inverse_scale_factor,
    );
    state.layers_scroll_y = position.y;
    scroll.propagate(false);
}

fn next_surface_scroll_y(
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

fn activate_rail_action(
    activate: On<Activate>,
    actions: Query<&RailAction>,
    focus: Res<InputFocus>,
    presentation: Res<PresentationState>,
    mut ui_state: ResMut<RailUiState>,
    mut commands: ResMut<SimCommandQueue>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    if focus.get() == Some(activate.entity)
        && matches!(
            action,
            RailAction::ToggleLayersPanel | RailAction::ToggleFullscreen | RailAction::OpenSettings
        )
    {
        ui_state.restore_focus = Some(RailFocusTarget::Action(*action));
    }
    match *action {
        RailAction::Zoom(delta) => commands.push(dolly_command(delta)),
        RailAction::ToggleLayersPanel => commands.push(SimCommand::SetLayersPanelOpen(
            !presentation.is_layers_panel_open(),
        )),
        RailAction::ToggleFullscreen => commands.push(SimCommand::ToggleFullscreen),
        RailAction::OpenSettings => commands.push(SimCommand::OpenSettings),
    }
}

fn activate_layer_toggle(
    activate: On<Activate>,
    toggles: Query<&LayerToggle>,
    layers: Res<LayerState>,
    focus: Res<InputFocus>,
    mut ui_state: ResMut<RailUiState>,
    mut commands: ResMut<SimCommandQueue>,
) {
    let Ok(toggle) = toggles.get(activate.entity) else {
        return;
    };
    if focus.get() == Some(activate.entity) {
        ui_state.restore_focus = Some(RailFocusTarget::Layer(toggle.0));
    }
    commands.push(SimCommand::SetLayerVisibility {
        layer: toggle.0,
        visible: !layers.is_visible(toggle.0),
    });
}

fn restore_user_interface(
    _activate: On<Activate>,
    mut ui_state: ResMut<RailUiState>,
    mut commands: ResMut<SimCommandQueue>,
) {
    if ui_state.restore_focus.is_none() {
        ui_state.restore_focus = Some(RailFocusTarget::Action(RailAction::ToggleLayersPanel));
    }
    commands.push(SimCommand::SetLayerVisibility {
        layer: LayerId::UserInterface,
        visible: true,
    });
}

fn restore_presentation_defaults(
    activate: On<Activate>,
    focus: Option<Res<InputFocus>>,
    ui_state: Option<ResMut<RailUiState>>,
    mut commands: ResMut<SimCommandQueue>,
) {
    if focus.as_deref().and_then(InputFocus::get) == Some(activate.entity) {
        if let Some(mut ui_state) = ui_state {
            ui_state.restore_focus = Some(RailFocusTarget::Action(RailAction::ToggleLayersPanel));
        }
    }
    commands.push(SimCommand::RestorePresentationDefaults);
}

type HudVisibilityFilter = (
    With<HudSurface>,
    Without<UiRestoreAffordance>,
    Without<UiRestoreTabGroup>,
);
type RestoreGroupFilter = (With<UiRestoreTabGroup>, Without<UiRestoreAffordance>);

#[derive(SystemParam)]
struct HudVisibilityParams<'w, 's> {
    layers: Res<'w, LayerState>,
    browse: Option<Res<'w, BrowseUiState>>,
    presentation: Option<Res<'w, PresentationState>>,
    hud: Query<'w, 's, &'static mut Visibility, HudVisibilityFilter>,
    restore_groups:
        Query<'w, 's, (&'static mut Visibility, &'static mut TabGroup), RestoreGroupFilter>,
    restore: Query<
        'w,
        's,
        (
            Entity,
            &'static mut Visibility,
            Option<&'static mut TabIndex>,
        ),
        With<UiRestoreAffordance>,
    >,
    browse_roots: Query<'w, 's, Entity, With<BrowseMenuRoot>>,
    settings_roots: Query<'w, 's, Entity, With<SettingsScreenRoot>>,
    tab_indices: Query<'w, 's, (Entity, &'static TabIndex), Without<UiRestoreAffordance>>,
    parents: Query<'w, 's, &'static ChildOf>,
    rail_actions: Query<'w, 's, (Entity, &'static RailAction)>,
    focus: ResMut<'w, InputFocus>,
}

fn sync_hud_visibility(params: HudVisibilityParams) {
    let HudVisibilityParams {
        layers,
        browse,
        presentation,
        mut hud,
        mut restore_groups,
        mut restore,
        browse_roots,
        settings_roots,
        tab_indices,
        parents,
        rail_actions,
        mut focus,
    } = params;
    let ui_visible = layers.is_visible(LayerId::UserInterface);
    let desired_hud_visibility = if ui_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut hud {
        if *visibility != desired_hud_visibility {
            *visibility = desired_hud_visibility;
        }
    }
    let desired_restore_visibility = if ui_visible {
        Visibility::Hidden
    } else {
        Visibility::Visible
    };
    for (mut visibility, mut group) in &mut restore_groups {
        if *visibility != desired_restore_visibility {
            *visibility = desired_restore_visibility;
        }
        if group.modal == ui_visible {
            group.modal = !ui_visible;
        }
    }
    let mut restore_entity = None;
    for (entity, mut visibility, tab_index) in &mut restore {
        restore_entity = Some(entity);
        if *visibility != desired_restore_visibility {
            *visibility = desired_restore_visibility;
        }
        if let Some(mut tab_index) = tab_index {
            let desired_tab_index = if ui_visible {
                DISABLED_TAB_INDEX
            } else {
                UI_RESTORE_TAB_INDEX
            };
            if tab_index.0 != desired_tab_index {
                tab_index.0 = desired_tab_index;
            }
        }
        if !ui_visible && focus.get() != Some(entity) {
            focus.set(entity, FocusCause::Navigated);
        }
    }
    if !ui_visible {
        return;
    }

    let focused_needs_recovery = focus
        .get()
        .is_some_and(|focused| Some(focused) == restore_entity);
    if !focused_needs_recovery {
        return;
    }

    // A replay or external settings command has restored UI without going
    // through the SHOW UI observer. Resolve its now-hidden focus immediately,
    // preserving the same Browse-before-Settings modal priority as input.
    let browse_open = browse.as_deref().is_some_and(BrowseUiState::is_open);
    let settings_open = presentation
        .as_deref()
        .is_some_and(|presentation| presentation.is_settings_open());
    let modal_root = if browse_open {
        browse_roots.single().ok()
    } else if settings_open {
        settings_roots.single().ok()
    } else {
        None
    };
    let modal_focus =
        modal_root.and_then(|root| first_tabbable_descendant(root, &tab_indices, &parents));
    let fallback = if browse_open || settings_open {
        None
    } else {
        rail_actions.iter().find_map(|(entity, action)| {
            (*action == RailAction::ToggleLayersPanel).then_some(entity)
        })
    };
    if let Some(destination) = modal_focus.or(fallback) {
        focus.set(destination, FocusCause::Navigated);
    } else {
        focus.clear();
    }
}

fn first_tabbable_descendant(
    root: Entity,
    tab_indices: &Query<(Entity, &TabIndex), Without<UiRestoreAffordance>>,
    parents: &Query<&ChildOf>,
) -> Option<Entity> {
    tab_indices
        .iter()
        .filter(|(entity, index)| index.0 >= 0 && is_descendant_of(*entity, root, parents))
        .min_by_key(|(entity, index)| (index.0, entity.to_bits()))
        .map(|(entity, _)| entity)
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
        if window.mode != desired {
            window.mode = desired;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::consume_presentation_command;
    use crate::search::{consume_search_command, MENU_Z_INDEX};
    use crate::settings::{
        consume_settings_command, converge_presentation_settings, SettingsSaveRequest,
        SettingsScreenState, SETTINGS_Z_INDEX,
    };
    use crate::ui_kit::test_layout;
    use crate::{AppSettings, PersistedLayerState, WidgetRoot};
    use bevy::{
        app::TaskPoolPlugin,
        asset::{AssetApp, AssetPlugin},
        scene::ScenePlugin,
        text::Font,
    };
    use std::collections::HashSet;

    #[derive(Resource, Debug, Default)]
    struct HudComponentWrites {
        visibility: usize,
        tab_groups: usize,
        tab_indices: usize,
    }

    fn count_hud_component_writes(
        visibility: Query<Entity, Changed<Visibility>>,
        tab_groups: Query<Entity, Changed<TabGroup>>,
        tab_indices: Query<Entity, Changed<TabIndex>>,
        mut writes: ResMut<HudComponentWrites>,
    ) {
        writes.visibility = visibility.iter().count();
        writes.tab_groups = tab_groups.iter().count();
        writes.tab_indices = tab_indices.iter().count();
    }

    fn reduce_queued_layer_commands(
        mut commands: ResMut<SimCommandQueue>,
        mut layers: ResMut<LayerState>,
        mut presentation: ResMut<PresentationState>,
    ) {
        for command in commands.drain() {
            consume_presentation_command(&command, &mut layers, &mut presentation);
        }
    }

    fn reduce_queued_recovery_commands(
        mut commands: ResMut<SimCommandQueue>,
        mut layers: ResMut<LayerState>,
        mut presentation: ResMut<PresentationState>,
        mut settings: ResMut<AppSettings>,
        mut screen: ResMut<SettingsScreenState>,
        mut save: ResMut<SettingsSaveRequest>,
    ) {
        for command in commands.drain() {
            consume_presentation_command(&command, &mut layers, &mut presentation);
            consume_settings_command(&command, &mut screen, &mut settings, &mut save);
            if converge_presentation_settings(&layers, &presentation, &mut settings) {
                save.request();
            }
        }
    }

    fn cue_less_layers() -> LayerState {
        let mut layers = LayerState::default();
        for layer in [LayerId::Orbits, LayerId::Labels, LayerId::Icons] {
            layers.set_visible(layer, false);
        }
        layers
    }

    fn cue_recovery_app() -> App {
        let layers = cue_less_layers();
        let settings = AppSettings {
            layers: PersistedLayerState::from_snapshot(layers.persistence_snapshot()),
            ..default()
        };
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ))
        .init_asset::<Font>()
        .insert_resource(UiTheme::default())
        .insert_resource(layers)
        .insert_resource(settings)
        .init_resource::<PresentationState>()
        .init_resource::<BrowseUiState>()
        .init_resource::<InputFocus>()
        .init_resource::<SimCommandQueue>()
        .init_resource::<SettingsScreenState>()
        .init_resource::<SettingsSaveRequest>()
        .init_resource::<RailUiState>()
        .add_systems(Startup, spawn_restore_affordance)
        .add_systems(
            Update,
            (
                reduce_queued_recovery_commands,
                sync_visual_cue_recovery,
                rebuild_right_rail,
            )
                .chain(),
        )
        .add_systems(PostUpdate, sync_hud_visibility);
        app.update();
        app
    }

    fn cue_recovery_root(world: &mut World) -> Entity {
        world
            .query_filtered::<Entity, With<VisualCueRecoveryRoot>>()
            .single(world)
            .expect("exactly one cue-recovery notice")
    }

    fn cue_recovery_button(world: &mut World) -> Entity {
        let root = cue_recovery_root(world);
        world
            .query::<(
                Entity,
                &ChildOf,
                &bevy::ui_widgets::Button,
                &AccessibleLabel,
            )>()
            .iter(world)
            .find_map(|(entity, parent, _, label)| {
                (parent.parent() == root && !label.0.trim().is_empty()).then_some(entity)
            })
            .expect("cue-recovery notice has one accessible action")
    }

    fn toggle_layers_rail_action(world: &mut World) -> Entity {
        world
            .query::<(Entity, &RailAction)>()
            .iter(world)
            .find_map(|(entity, action)| {
                (*action == RailAction::ToggleLayersPanel).then_some(entity)
            })
            .expect("Toggle Layers rail action")
    }

    fn layout_node_rect(world: &World, entity: Entity) -> Rect {
        let node = world.get::<ComputedNode>(entity).unwrap();
        let center = world
            .get::<UiGlobalTransform>(entity)
            .unwrap()
            .affine()
            .translation;
        Rect::from_center_size(center, node.size())
    }

    fn layout_rect_contains(outer: Rect, inner: Rect) -> bool {
        inner.min.x >= outer.min.x - 1.0
            && inner.max.x <= outer.max.x + 1.0
            && inner.min.y >= outer.min.y - 1.0
            && inner.max.y <= outer.max.y + 1.0
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
        let mut presentation = PresentationState::default();
        presentation.set_layers_panel_open(true);
        app.insert_resource(layers)
            .init_resource::<SimCommandQueue>()
            .insert_resource(presentation)
            .init_resource::<InputFocus>()
            .insert_resource(RailUiState {
                rail_scroll_y: 0.0,
                layers_scroll_y: 0.0,
                restore_focus: None,
                ..default()
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
        assert!(app
            .world()
            .resource::<PresentationState>()
            .is_layers_panel_open());
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
        assert!(app
            .world()
            .resource::<PresentationState>()
            .is_layers_panel_open());
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
    fn cue_less_visible_ui_exposes_one_explicit_restore_command() {
        let mut layers = LayerState::default();
        layers.set_visible(LayerId::Orbits, false);
        layers.set_visible(LayerId::Labels, false);
        layers.set_visible(LayerId::Icons, false);
        assert!(visual_cue_recovery_needed(layers));

        let mut ui_off = layers;
        ui_off.set_visible(LayerId::UserInterface, false);
        assert!(!visual_cue_recovery_needed(ui_off));

        let mut one_cue = layers;
        one_cue.set_visible(LayerId::Labels, true);
        assert!(!visual_cue_recovery_needed(one_cue));

        let mut app = App::new();
        app.init_resource::<SimCommandQueue>();
        let restore = app
            .world_mut()
            .spawn(bevy::ui_widgets::Button)
            .observe(restore_presentation_defaults)
            .id();
        app.world_mut().trigger(Activate { entity: restore });
        let queued: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(queued, vec![SimCommand::RestorePresentationDefaults]);
    }

    #[test]
    fn cue_recovery_appears_once_without_mutating_commands_or_persistence() {
        let mut app = cue_recovery_app();
        let initial_root = cue_recovery_root(app.world_mut());
        let initial_settings = app.world().resource::<AppSettings>().clone();

        for _ in 0..3 {
            app.update();
            assert_eq!(cue_recovery_root(app.world_mut()), initial_root);
        }

        let world = app.world_mut();
        let accessible_actions = world
            .query::<(&ChildOf, &bevy::ui_widgets::Button, &AccessibleLabel)>()
            .iter(world)
            .filter(|(parent, _, label)| {
                parent.parent() == initial_root && !label.0.trim().is_empty()
            })
            .count();
        assert_eq!(accessible_actions, 1);
        assert_eq!(world.resource::<AppSettings>(), &initial_settings);
        assert!(!world.resource::<SettingsSaveRequest>().is_requested());
        assert_eq!(world.resource_mut::<SimCommandQueue>().drain().count(), 0);
    }

    #[test]
    fn cue_recovery_activation_queues_one_semantic_restore_and_converges_defaults() {
        let mut app = cue_recovery_app();
        let restore = cue_recovery_button(app.world_mut());
        app.world_mut().trigger(Activate { entity: restore });

        let queued = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(queued, vec![SimCommand::RestorePresentationDefaults]);
        for command in queued {
            app.world_mut()
                .resource_mut::<SimCommandQueue>()
                .push(command);
        }
        app.update();

        assert_eq!(*app.world().resource::<LayerState>(), LayerState::default());
        assert_eq!(
            app.world().resource::<AppSettings>(),
            &AppSettings::default()
        );
        assert!(app.world().resource::<SettingsSaveRequest>().is_requested());
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<VisualCueRecoveryRoot>>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn ui_off_removes_cue_recovery_and_yields_to_exactly_one_show_ui_action() {
        let mut app = cue_recovery_app();
        let cue = cue_recovery_button(app.world_mut());
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(cue, FocusCause::Navigated);
        app.world_mut()
            .resource_mut::<SimCommandQueue>()
            .push(SimCommand::SetLayerVisibility {
                layer: LayerId::UserInterface,
                visible: false,
            });
        app.update();

        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<VisualCueRecoveryRoot>>()
                .iter(app.world())
                .count(),
            0
        );
        let restore = {
            let world = app.world_mut();
            let actions = world
                .query_filtered::<
                    (Entity, &Visibility, &TabIndex, &AccessibleLabel),
                    With<UiRestoreAffordance>,
                >()
                .iter(world)
                .map(|(entity, visibility, tab_index, label)| {
                    (entity, *visibility, *tab_index, label.0.clone())
                })
                .collect::<Vec<_>>();
            assert_eq!(actions.len(), 1);
            let (entity, visibility, tab_index, label) = &actions[0];
            assert_eq!(*visibility, Visibility::Visible);
            assert_eq!(*tab_index, TabIndex(UI_RESTORE_TAB_INDEX));
            assert_eq!(label, "Restore user interface");
            *entity
        };
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(restore));
    }

    #[test]
    fn cue_recovery_z_order_yields_to_search_browse_and_settings() {
        const {
            // The debug diagnostics overlay is the highest ordinary HUD at 110.
            assert!(CUE_RECOVERY_Z_INDEX > 110);
            assert!(CUE_RECOVERY_Z_INDEX < SEARCH_DROPDOWN_Z_INDEX);
            assert!(SEARCH_DROPDOWN_Z_INDEX < MENU_Z_INDEX);
            assert!(MENU_Z_INDEX < SETTINGS_Z_INDEX);
        }
    }

    #[test]
    fn external_layer_restore_moves_cue_focus_to_live_toggle_layers_action() {
        let mut app = cue_recovery_app();
        let cue = cue_recovery_button(app.world_mut());
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(cue, FocusCause::Navigated);
        app.world_mut()
            .resource_mut::<SimCommandQueue>()
            .push(SimCommand::SetLayerVisibility {
                layer: LayerId::Labels,
                visible: true,
            });
        app.update();

        assert!(app.world().get_entity(cue).is_err());
        let fallback = toggle_layers_rail_action(app.world_mut());
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(fallback));
    }

    #[test]
    fn external_cue_restore_prefers_browse_then_settings_modal_focus() {
        let mut browse_app = cue_recovery_app();
        let cue = cue_recovery_button(browse_app.world_mut());
        browse_app
            .world_mut()
            .resource_mut::<InputFocus>()
            .set(cue, FocusCause::Navigated);
        consume_search_command(
            &SimCommand::SetBrowseOpen(true),
            &mut browse_app.world_mut().resource_mut::<BrowseUiState>(),
        );
        let browse_root = browse_app.world_mut().spawn(BrowseMenuRoot).id();
        let browse_later = browse_app
            .world_mut()
            .spawn((TabIndex(9), ChildOf(browse_root)))
            .id();
        let browse_first = browse_app
            .world_mut()
            .spawn((TabIndex(3), ChildOf(browse_root)))
            .id();
        browse_app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .push(SimCommand::SetLayerVisibility {
                layer: LayerId::Icons,
                visible: true,
            });
        browse_app.update();
        assert!(browse_app.world().get_entity(cue).is_err());
        assert_eq!(
            browse_app.world().resource::<InputFocus>().get(),
            Some(browse_first)
        );
        assert_ne!(
            browse_app.world().resource::<InputFocus>().get(),
            Some(browse_later)
        );

        let mut settings_app = cue_recovery_app();
        let cue = cue_recovery_button(settings_app.world_mut());
        settings_app
            .world_mut()
            .resource_mut::<InputFocus>()
            .set(cue, FocusCause::Navigated);
        settings_app
            .world_mut()
            .resource_mut::<PresentationState>()
            .open_settings();
        let settings_root = settings_app.world_mut().spawn(SettingsScreenRoot).id();
        let settings_later = settings_app
            .world_mut()
            .spawn((TabIndex(8), ChildOf(settings_root)))
            .id();
        let settings_first = settings_app
            .world_mut()
            .spawn((TabIndex(2), ChildOf(settings_root)))
            .id();
        settings_app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .push(SimCommand::ApplySettings(Box::default()));
        settings_app.update();
        assert!(settings_app.world().get_entity(cue).is_err());
        assert_eq!(
            settings_app.world().resource::<InputFocus>().get(),
            Some(settings_first)
        );
        assert_ne!(
            settings_app.world().resource::<InputFocus>().get(),
            Some(settings_later)
        );
    }

    #[test]
    fn every_toggle_command_reduces_into_panel_state_within_one_update() {
        let mut app = App::new();
        app.init_resource::<SimCommandQueue>()
            .init_resource::<LayerState>()
            .init_resource::<InputFocus>()
            .init_resource::<RailUiState>()
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

        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(toggles[0], FocusCause::Navigated);
        for entity in &toggles {
            app.world_mut().trigger(Activate { entity: *entity });
        }
        assert_eq!(
            app.world().resource::<RailUiState>().restore_focus,
            Some(RailFocusTarget::Layer(LayerId::UserInterface))
        );
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
        spawn_layers_panel(&mut commands, *theme, &asset_server, &layers, 0.0);
    }

    fn rendered_rail_app(layers_panel_open: bool) -> App {
        let mut app = App::new();
        let mut presentation = PresentationState::default();
        presentation.set_layers_panel_open(layers_panel_open);
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ))
        .init_asset::<Font>()
        .insert_resource(UiTheme::default())
        .init_resource::<LayerState>()
        .insert_resource(presentation)
        .init_resource::<InputFocus>()
        .init_resource::<SimCommandQueue>()
        .insert_resource(RailUiState {
            rail_scroll_y: 13.0,
            layers_scroll_y: 29.0,
            restore_focus: None,
            ..default()
        })
        .add_systems(Startup, spawn_restore_affordance)
        .add_systems(
            Update,
            (
                reduce_queued_layer_commands,
                sync_visual_cue_recovery,
                rebuild_right_rail,
            )
                .chain(),
        )
        .add_systems(PostUpdate, sync_hud_visibility);
        app.update();
        app
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
        let panel = world
            .query_filtered::<Entity, With<LayersPanelRoot>>()
            .single(world)
            .unwrap();
        let group = world.entity(panel).get::<TabGroup>().unwrap();
        assert_eq!(group.order, LAYERS_PANEL_TAB_GROUP_ORDER);
        assert!(!group.modal);
        assert!(world.entity(panel).contains::<UiScrollSurface>());
        assert_eq!(world.entity(panel).get::<ScrollPosition>().unwrap().y, 0.0);
        let node = world.entity(panel).get::<Node>().unwrap();
        assert_eq!(
            node.top,
            px(TOP_BAR_HEIGHT_PX + UiTheme::default().spacing.lg_px)
        );
        assert_eq!(
            node.bottom,
            px(TIME_BAR_HEIGHT_PX + UiTheme::default().spacing.lg_px)
        );
        assert_eq!(node.overflow, Overflow::scroll_y());

        let indices: HashSet<_> = world
            .query::<(&LayerToggle, &TabIndex)>()
            .iter(world)
            .map(|(_, index)| index.0)
            .collect();
        assert_eq!(indices.len(), LayerId::ALL.len());
    }

    #[test]
    fn rail_and_layers_scroll_clamp_line_and_pixel_input() {
        assert_eq!(
            next_surface_scroll_y(100.0, -2.0, MouseScrollUnit::Line, 1_000.0, 600.0, 1.0,),
            156.0
        );
        assert_eq!(
            next_surface_scroll_y(390.0, -50.0, MouseScrollUnit::Pixel, 1_000.0, 600.0, 1.0,),
            400.0
        );
        assert_eq!(
            next_surface_scroll_y(10.0, 50.0, MouseScrollUnit::Pixel, 1_000.0, 600.0, 1.0,),
            0.0
        );
    }

    #[test]
    fn constrained_rail_is_an_ordered_retained_scroll_surface() {
        let mut app = rendered_rail_app(true);
        let world = app.world_mut();
        let rail = world
            .query_filtered::<Entity, With<RightRailRoot>>()
            .single(world)
            .unwrap();
        let group = world.entity(rail).get::<TabGroup>().unwrap();
        assert_eq!(group.order, RAIL_TAB_GROUP_ORDER);
        assert!(!group.modal);
        assert!(world.entity(rail).contains::<UiScrollSurface>());
        assert_eq!(world.entity(rail).get::<ScrollPosition>().unwrap().y, 13.0);
        let node = world.entity(rail).get::<Node>().unwrap();
        assert_eq!(
            node.top,
            px(TOP_BAR_HEIGHT_PX + UiTheme::default().spacing.lg_px)
        );
        assert_eq!(
            node.bottom,
            px(TIME_BAR_HEIGHT_PX + UiTheme::default().spacing.lg_px)
        );
        assert_eq!(node.overflow, Overflow::scroll_y());
    }

    #[test]
    fn stable_rail_and_layers_surface_retain_entity_identity() {
        let mut app = rendered_rail_app(true);
        let (rail, panel) = {
            let world = app.world_mut();
            (
                world
                    .query_filtered::<Entity, With<RightRailRoot>>()
                    .single(world)
                    .unwrap(),
                world
                    .query_filtered::<Entity, With<LayersPanelRoot>>()
                    .single(world)
                    .unwrap(),
            )
        };

        app.update();

        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<RightRailRoot>>()
                .single(world)
                .unwrap(),
            rail
        );
        assert_eq!(
            world
                .query_filtered::<Entity, With<LayersPanelRoot>>()
                .single(world)
                .unwrap(),
            panel
        );

        app.world_mut()
            .resource_mut::<PresentationState>()
            .open_settings();
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<RightRailRoot>>()
                .single(world)
                .unwrap(),
            rail
        );
        assert_eq!(
            world
                .query_filtered::<Entity, With<LayersPanelRoot>>()
                .single(world)
                .unwrap(),
            panel
        );
    }

    #[test]
    fn command_driven_panel_rebuilds_preserve_semantic_focus_and_scroll() {
        let mut app = rendered_rail_app(false);
        let toggle = toggle_layers_rail_action(app.world_mut());
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(toggle, FocusCause::Navigated);
        app.world_mut()
            .query_filtered::<&mut ScrollPosition, With<RightRailRoot>>()
            .single_mut(app.world_mut())
            .unwrap()
            .y = 17.0;
        app.world_mut().trigger(Activate { entity: toggle });
        app.update();

        assert!(app
            .world()
            .resource::<PresentationState>()
            .is_layers_panel_open());
        let opened_toggle = toggle_layers_rail_action(app.world_mut());
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(opened_toggle)
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<&ScrollPosition, With<RightRailRoot>>()
                .single(app.world())
                .unwrap()
                .y,
            17.0
        );
        let panel = app
            .world_mut()
            .query_filtered::<Entity, With<LayersPanelRoot>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world().entity(panel).get::<ScrollPosition>().unwrap().y,
            29.0
        );

        app.world_mut()
            .entity_mut(panel)
            .get_mut::<ScrollPosition>()
            .unwrap()
            .y = 41.0;
        app.world_mut().trigger(Activate {
            entity: opened_toggle,
        });
        app.update();
        assert!(!app
            .world()
            .resource::<PresentationState>()
            .is_layers_panel_open());
        assert!(app
            .world_mut()
            .query_filtered::<Entity, With<LayersPanelRoot>>()
            .single(app.world())
            .is_err());

        let closed_toggle = toggle_layers_rail_action(app.world_mut());
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(closed_toggle)
        );
        app.world_mut().trigger(Activate {
            entity: closed_toggle,
        });
        app.update();
        let reopened_panel = app
            .world_mut()
            .query_filtered::<Entity, With<LayersPanelRoot>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world()
                .entity(reopened_panel)
                .get::<ScrollPosition>()
                .unwrap()
                .y,
            41.0
        );
    }

    #[test]
    fn stable_hud_visibility_does_not_rewrite_components() {
        let mut app = App::new();
        app.init_resource::<LayerState>()
            .init_resource::<InputFocus>()
            .init_resource::<HudComponentWrites>()
            .add_systems(
                PostUpdate,
                (sync_hud_visibility, count_hud_component_writes).chain(),
            );
        app.world_mut().spawn((HudSurface, Visibility::Visible));
        app.world_mut()
            .spawn((UiRestoreTabGroup, Visibility::Hidden, TabGroup::new(0)));
        app.world_mut().spawn((
            UiRestoreAffordance,
            Visibility::Hidden,
            TabIndex(DISABLED_TAB_INDEX),
        ));
        app.update();

        *app.world_mut().resource_mut::<HudComponentWrites>() = HudComponentWrites::default();
        app.update();

        let writes = app.world().resource::<HudComponentWrites>();
        assert_eq!(writes.visibility, 0);
        assert_eq!(writes.tab_groups, 0);
        assert_eq!(writes.tab_indices, 0);
    }

    #[test]
    fn rail_and_layers_reach_last_actions_for_required_viewports() {
        for (width, height, scale) in test_layout::required_viewports() {
            let mut app = test_layout::app(width, height, scale);
            let mut presentation = PresentationState::default();
            presentation.set_layers_panel_open(true);
            app.insert_resource(UiTheme::default())
                .init_resource::<LayerState>()
                .insert_resource(presentation)
                .init_resource::<InputFocus>()
                .insert_resource(RailUiState {
                    rail_scroll_y: 0.0,
                    layers_scroll_y: 0.0,
                    restore_focus: None,
                    ..default()
                })
                .add_systems(Update, rebuild_right_rail);
            test_layout::settle(&mut app);

            let rail = app
                .world_mut()
                .query_filtered::<Entity, With<RightRailRoot>>()
                .single(app.world())
                .unwrap();
            let panel = app
                .world_mut()
                .query_filtered::<Entity, With<LayersPanelRoot>>()
                .single(app.world())
                .unwrap();
            let viewport = Rect::from_corners(Vec2::ZERO, Vec2::new(width as f32, height as f32));
            for surface in [rail, panel] {
                let rect = layout_node_rect(app.world(), surface);
                assert!(
                    rect.height() > 0.0 && layout_rect_contains(viewport, rect),
                    "{width}×{height} scale {scale}: surface {surface:?} {rect:?} escaped viewport"
                );
                app.world_mut()
                    .entity_mut(surface)
                    .get_mut::<ScrollPosition>()
                    .unwrap()
                    .y = f32::MAX;
            }
            test_layout::settle(&mut app);

            let settings = app
                .world_mut()
                .query::<(Entity, &RailAction)>()
                .iter(app.world())
                .find_map(|(entity, action)| {
                    (*action == RailAction::OpenSettings).then_some(entity)
                })
                .unwrap();
            let icons = app
                .world_mut()
                .query::<(Entity, &LayerToggle)>()
                .iter(app.world())
                .find_map(|(entity, toggle)| (toggle.0 == LayerId::Icons).then_some(entity))
                .unwrap();
            assert!(
                layout_rect_contains(
                    layout_node_rect(app.world(), rail),
                    layout_node_rect(app.world(), settings),
                ),
                "{width}×{height} scale {scale}: Settings rail action is unreachable"
            );
            assert!(
                layout_rect_contains(
                    layout_node_rect(app.world(), panel),
                    layout_node_rect(app.world(), icons),
                ),
                "{width}×{height} scale {scale}: Icons layer action is unreachable"
            );
        }
    }

    #[test]
    fn focused_layer_and_settings_actions_restore_after_rebuilds() {
        let mut app = rendered_rail_app(true);
        let icons = {
            let world = app.world_mut();
            world
                .query::<(Entity, &LayerToggle)>()
                .iter(world)
                .find_map(|(entity, toggle)| (toggle.0 == LayerId::Icons).then_some(entity))
                .unwrap()
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(icons, FocusCause::Navigated);
        app.world_mut().trigger(Activate { entity: icons });
        app.update();

        let rebuilt_icons = {
            let world = app.world_mut();
            world
                .query::<(Entity, &LayerToggle)>()
                .iter(world)
                .find_map(|(entity, toggle)| (toggle.0 == LayerId::Icons).then_some(entity))
                .unwrap()
        };
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(rebuilt_icons)
        );
        let panel_scroll_y = {
            let world = app.world_mut();
            world
                .query_filtered::<&ScrollPosition, With<LayersPanelRoot>>()
                .single(world)
                .unwrap()
                .y
        };
        assert_eq!(panel_scroll_y, 29.0);

        let settings = {
            let world = app.world_mut();
            world
                .query::<(Entity, &RailAction)>()
                .iter(world)
                .find_map(|(entity, action)| {
                    (*action == RailAction::OpenSettings).then_some(entity)
                })
                .unwrap()
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(settings, FocusCause::Navigated);
        app.world_mut().trigger(Activate { entity: settings });
        app.update();
        assert!(app
            .world()
            .resource::<PresentationState>()
            .is_settings_open());
        assert_eq!(
            app.world().resource::<RailUiState>().restore_focus,
            Some(RailFocusTarget::Action(RailAction::OpenSettings))
        );

        app.world_mut()
            .resource_mut::<SimCommandQueue>()
            .push(SimCommand::CloseSettings);
        app.update();
        let rebuilt_settings = {
            let world = app.world_mut();
            world
                .query::<(Entity, &RailAction)>()
                .iter(world)
                .find_map(|(entity, action)| {
                    (*action == RailAction::OpenSettings).then_some(entity)
                })
                .unwrap()
        };
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(rebuilt_settings)
        );
    }

    #[test]
    fn ui_off_focuses_show_ui_and_restoration_returns_to_the_layer_toggle() {
        let mut app = rendered_rail_app(true);
        let ui_toggle = {
            let world = app.world_mut();
            world
                .query::<(Entity, &LayerToggle)>()
                .iter(world)
                .find_map(|(entity, toggle)| (toggle.0 == LayerId::UserInterface).then_some(entity))
                .unwrap()
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(ui_toggle, FocusCause::Navigated);
        app.world_mut().trigger(Activate { entity: ui_toggle });
        app.update();

        let restore = {
            let world = app.world_mut();
            world
                .query_filtered::<Entity, With<UiRestoreAffordance>>()
                .single(world)
                .unwrap()
        };
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(restore));
        assert_eq!(
            app.world().entity(restore).get::<Visibility>(),
            Some(&Visibility::Visible)
        );
        let restore_group = app
            .world()
            .entity(restore)
            .get::<ChildOf>()
            .map(ChildOf::parent)
            .unwrap();
        assert!(app
            .world()
            .entity(restore_group)
            .get::<TabGroup>()
            .is_some_and(|group| group.modal));
        assert_eq!(
            app.world().resource::<RailUiState>().restore_focus,
            Some(RailFocusTarget::Layer(LayerId::UserInterface))
        );

        // External/replayed modal transitions may occur while the complete
        // HUD is hidden. They must not consume the rail target before SHOW UI
        // can restore it.
        app.world_mut()
            .resource_mut::<PresentationState>()
            .open_settings();
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(restore));
        assert_eq!(
            app.world().resource::<RailUiState>().restore_focus,
            Some(RailFocusTarget::Layer(LayerId::UserInterface))
        );
        app.world_mut()
            .resource_mut::<PresentationState>()
            .close_settings();
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(restore));
        assert_eq!(
            app.world().resource::<RailUiState>().restore_focus,
            Some(RailFocusTarget::Layer(LayerId::UserInterface))
        );

        app.world_mut().trigger(Activate { entity: restore });
        app.update();
        let rebuilt_ui_toggle = {
            let world = app.world_mut();
            world
                .query::<(Entity, &LayerToggle)>()
                .iter(world)
                .find_map(|(entity, toggle)| (toggle.0 == LayerId::UserInterface).then_some(entity))
                .unwrap()
        };
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(rebuilt_ui_toggle)
        );
        assert_eq!(
            app.world().entity(restore).get::<Visibility>(),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().entity(restore).get::<TabIndex>(),
            Some(&TabIndex(DISABLED_TAB_INDEX))
        );
        assert!(app
            .world()
            .entity(restore_group)
            .get::<TabGroup>()
            .is_some_and(|group| !group.modal));
        assert_eq!(
            app.world().entity(restore_group).get::<Visibility>(),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn external_ui_restore_commands_choose_active_modal_or_stable_rail_fallback() {
        let mut app = rendered_rail_app(true);
        let mut browse = BrowseUiState::default();
        consume_search_command(&SimCommand::SetBrowseOpen(true), &mut browse);
        app.insert_resource(browse);
        let browse_root = app.world_mut().spawn((BrowseMenuRoot, HudSurface)).id();
        let browse_button = app
            .world_mut()
            .spawn((TabIndex(0), ChildOf(browse_root)))
            .id();
        let settings_root = app.world_mut().spawn((SettingsScreenRoot, HudSurface)).id();
        let settings_button = app
            .world_mut()
            .spawn((TabIndex(0), ChildOf(settings_root)))
            .id();

        app.world_mut()
            .resource_mut::<SimCommandQueue>()
            .push(SimCommand::SetLayerVisibility {
                layer: LayerId::UserInterface,
                visible: false,
            });
        app.update();
        let restore = app
            .world_mut()
            .query_filtered::<Entity, With<UiRestoreAffordance>>()
            .single(app.world())
            .unwrap();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(restore));

        app.world_mut()
            .resource_mut::<PresentationState>()
            .open_settings();
        app.world_mut()
            .resource_mut::<SimCommandQueue>()
            .push(SimCommand::SetLayerVisibility {
                layer: LayerId::UserInterface,
                visible: true,
            });
        app.update();
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(browse_button),
            "Browse wins malformed hidden-modal competition when UI returns"
        );

        consume_search_command(
            &SimCommand::SetBrowseOpen(false),
            &mut app.world_mut().resource_mut::<BrowseUiState>(),
        );
        app.world_mut()
            .resource_mut::<SimCommandQueue>()
            .push(SimCommand::SetLayerVisibility {
                layer: LayerId::UserInterface,
                visible: false,
            });
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(restore));

        app.world_mut()
            .resource_mut::<SimCommandQueue>()
            .push(SimCommand::SetLayerVisibility {
                layer: LayerId::UserInterface,
                visible: true,
            });
        app.update();
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(settings_button)
        );

        app.world_mut()
            .resource_mut::<PresentationState>()
            .close_settings();
        app.world_mut()
            .resource_mut::<SimCommandQueue>()
            .push(SimCommand::SetLayerVisibility {
                layer: LayerId::UserInterface,
                visible: false,
            });
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(restore));

        app.world_mut()
            .resource_mut::<SimCommandQueue>()
            .push(SimCommand::RestorePresentationDefaults);
        app.update();
        let rail_fallback = {
            let world = app.world_mut();
            world
                .query::<(Entity, &RailAction)>()
                .iter(world)
                .find_map(|(entity, action)| {
                    (*action == RailAction::ToggleLayersPanel).then_some(entity)
                })
                .unwrap()
        };
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(rail_fallback)
        );
        assert_ne!(app.world().resource::<InputFocus>().get(), Some(restore));
        assert_eq!(
            app.world().entity(restore).get::<Visibility>(),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn focused_cue_recovery_despawn_restores_to_a_live_rail_entity() {
        let mut app = rendered_rail_app(true);
        for layer in [LayerId::Orbits, LayerId::Labels, LayerId::Icons] {
            app.world_mut().resource_mut::<SimCommandQueue>().push(
                SimCommand::SetLayerVisibility {
                    layer,
                    visible: false,
                },
            );
        }
        app.update();

        let recovery_root = app
            .world_mut()
            .query_filtered::<Entity, With<VisualCueRecoveryRoot>>()
            .single(app.world())
            .unwrap();
        let recovery_button = {
            let world = app.world_mut();
            world
                .query::<(Entity, &ChildOf, &bevy::ui_widgets::Button)>()
                .iter(world)
                .find_map(|(entity, parent, _)| {
                    (parent.parent() == recovery_root).then_some(entity)
                })
                .unwrap()
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(recovery_button, FocusCause::Navigated);
        app.world_mut().trigger(Activate {
            entity: recovery_button,
        });
        let queued: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(queued, vec![SimCommand::RestorePresentationDefaults]);
        for command in queued {
            app.world_mut()
                .resource_mut::<SimCommandQueue>()
                .push(command);
        }

        app.update();

        assert!(app.world().get_entity(recovery_button).is_err());
        let focused = app
            .world()
            .resource::<InputFocus>()
            .get()
            .expect("recovery must leave a live focus target");
        assert!(app.world().get_entity(focused).is_ok());
        assert!(app
            .world()
            .entity(focused)
            .get::<RailAction>()
            .is_some_and(|action| *action == RailAction::ToggleLayersPanel));
    }

    #[test]
    fn rail_actions_enqueue_commands_without_mutating_canonical_panel_state() {
        let mut app = App::new();
        app.init_resource::<SimCommandQueue>()
            .init_resource::<LayerState>()
            .init_resource::<PresentationState>()
            .init_resource::<InputFocus>()
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
                SimCommand::SetLayersPanelOpen(true),
            ]
        );
        assert!(!app
            .world()
            .resource::<PresentationState>()
            .is_layers_panel_open());
    }

    #[test]
    fn rail_observer_statically_preserves_the_command_only_boundary() {
        let source = include_str!("layers.rs");
        let observer = source
            .split_once("fn activate_rail_action(")
            .expect("rail observer exists")
            .1
            .split_once("\nfn activate_layer_toggle(")
            .expect("layer observer follows rail observer")
            .0;
        assert!(observer.contains("commands.push(SimCommand::SetLayersPanelOpen("));
        assert!(!observer.contains("set_layers_panel_open"));
        assert!(!observer.contains("layers_panel_open ="));
    }

    #[test]
    fn presentation_reducer_exposes_fullscreen_and_wp14_settings_hooks() {
        let mut layers = LayerState::default();
        let mut presentation = PresentationState::default();
        consume_presentation_command(
            &SimCommand::SetLayersPanelOpen(true),
            &mut layers,
            &mut presentation,
        );
        assert!(presentation.is_layers_panel_open());
        consume_presentation_command(
            &SimCommand::SetLayersPanelOpen(true),
            &mut layers,
            &mut presentation,
        );
        assert!(presentation.is_layers_panel_open());
        consume_presentation_command(
            &SimCommand::SetLayersPanelOpen(false),
            &mut layers,
            &mut presentation,
        );
        consume_presentation_command(
            &SimCommand::SetLayersPanelOpen(false),
            &mut layers,
            &mut presentation,
        );
        assert!(!presentation.is_layers_panel_open());
        consume_presentation_command(
            &SimCommand::ToggleFullscreen,
            &mut layers,
            &mut presentation,
        );
        assert!(presentation.is_fullscreen());
        consume_presentation_command(&SimCommand::OpenSettings, &mut layers, &mut presentation);
        assert!(presentation.is_settings_open());
        consume_presentation_command(&SimCommand::OpenSettings, &mut layers, &mut presentation);
        assert!(presentation.is_settings_open());
        consume_presentation_command(&SimCommand::CloseSettings, &mut layers, &mut presentation);
        assert!(!presentation.is_settings_open());
        consume_presentation_command(&SimCommand::CloseSettings, &mut layers, &mut presentation);
        assert!(!presentation.is_settings_open());
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
