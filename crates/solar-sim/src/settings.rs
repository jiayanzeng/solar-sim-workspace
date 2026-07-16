//! WP14 persistent product settings and render recovery — Rev C §§4.2 and 8.5.
//!
//! Bevy's 0.19 settings framework owns disk I/O. This module owns the stable
//! reflected schema, applies loaded values at presentation boundaries, exposes
//! one settings-screen model, and installs the renderer's explicit recovery
//! policy. The core simulation only receives the persisted `StartMode` at boot.

use crate::control::{SimCommand, SimCommandQueue};
use crate::input_intent::UiScrollSurface;
use crate::layers::{HudSurface, LayerId, LayerState, LayerStateSnapshot, PresentationState};
use crate::ui_kit::{
    checkbox_row, chip, section_header, slider, UiTheme, WidgetSpec, WidgetVisualState,
    INTER_FONT_ASSET,
};
use crate::SimulationSet;
use bevy::{
    input::mouse::MouseScrollUnit,
    input_focus::{
        tab_navigation::{TabGroup, TabIndex},
        FocusCause, InputFocus,
    },
    prelude::*,
    render::{
        error_handler::{ErrorType, RenderError, RenderErrorHandler, RenderErrorPolicy},
        renderer::RenderDevice,
        settings::RenderCreation,
        view::Msaa,
    },
    settings::{ReflectSettingsGroup, SaveSettingsDeferred, SaveSettingsSync, SettingsGroup},
    ui::UiScale,
    ui_widgets::Activate,
    window::{MonitorSelection, PresentMode, PrimaryWindow, WindowCloseRequested, WindowMode},
    winit::{UpdateMode, WinitSettings},
};
use sim_core::time::{SimClock, StartMode, DEFAULT_START_EPOCH_JD_TDB};
use std::time::Duration;

pub const SETTINGS_IDENTIFIER: &str = "com.github.jiayanzeng.solar-sim";
const SETTINGS_SAVE_DELAY: Duration = Duration::from_millis(100);
const SETTINGS_Z_INDEX: i32 = 140;

#[derive(Reflect, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum DisplayModeSetting {
    #[default]
    Windowed,
    BorderlessFullscreen,
}

impl DisplayModeSetting {
    const ALL: [Self; 2] = [Self::Windowed, Self::BorderlessFullscreen];

    fn label(self) -> &'static str {
        match self {
            Self::Windowed => "WINDOWED",
            Self::BorderlessFullscreen => "FULLSCREEN",
        }
    }

    pub const fn is_fullscreen(self) -> bool {
        matches!(self, Self::BorderlessFullscreen)
    }
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct ResolutionSetting {
    pub width: u32,
    pub height: u32,
}

impl Default for ResolutionSetting {
    fn default() -> Self {
        Self {
            width: 1_600,
            height: 900,
        }
    }
}

impl ResolutionSetting {
    const PRESETS: [Self; 4] = [
        Self {
            width: 1_280,
            height: 720,
        },
        Self {
            width: 1_600,
            height: 900,
        },
        Self {
            width: 1_920,
            height: 1_080,
        },
        Self {
            width: 2_560,
            height: 1_440,
        },
    ];

    fn label(self) -> String {
        format!("{}×{}", self.width, self.height)
    }
}

#[derive(Reflect, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum FrameCap {
    Fps30,
    Fps60,
    #[default]
    Fps120,
    Fps240,
    Unlimited,
}

impl FrameCap {
    const ALL: [Self; 5] = [
        Self::Fps30,
        Self::Fps60,
        Self::Fps120,
        Self::Fps240,
        Self::Unlimited,
    ];

    pub const fn hz(self) -> Option<u32> {
        match self {
            Self::Fps30 => Some(30),
            Self::Fps60 => Some(60),
            Self::Fps120 => Some(120),
            Self::Fps240 => Some(240),
            Self::Unlimited => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Fps30 => "30 FPS",
            Self::Fps60 => "60 FPS",
            Self::Fps120 => "120 FPS",
            Self::Fps240 => "240 FPS",
            Self::Unlimited => "UNLIMITED",
        }
    }
}

#[derive(Reflect, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum QualityPreset {
    Low,
    Medium,
    #[default]
    High,
    Ultra,
}

impl QualityPreset {
    const ALL: [Self; 4] = [Self::Low, Self::Medium, Self::High, Self::Ultra];

    fn label(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
            Self::Ultra => "ULTRA",
        }
    }

    const fn msaa(self) -> Msaa {
        match self {
            Self::Low => Msaa::Off,
            Self::Medium => Msaa::Sample2,
            Self::High => Msaa::Sample4,
            Self::Ultra => Msaa::Sample8,
        }
    }
}

#[derive(Reflect, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum DistanceUnit {
    #[default]
    Kilometers,
    Miles,
    AstronomicalUnits,
}

impl DistanceUnit {
    const ALL: [Self; 3] = [Self::Kilometers, Self::Miles, Self::AstronomicalUnits];

    fn label(self) -> &'static str {
        match self {
            Self::Kilometers => "KM",
            Self::Miles => "MI",
            Self::AstronomicalUnits => "AU",
        }
    }
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum StartModeSetting {
    FixedEpoch { jd_tdb: f64 },
    Live,
}

impl Default for StartModeSetting {
    fn default() -> Self {
        Self::FixedEpoch {
            jd_tdb: DEFAULT_START_EPOCH_JD_TDB,
        }
    }
}

impl StartModeSetting {
    pub const fn to_core(self) -> StartMode {
        match self {
            Self::FixedEpoch { jd_tdb } => StartMode::FixedEpoch { jd_tdb },
            Self::Live => StartMode::Live,
        }
    }

    fn fixed_epoch(self) -> f64 {
        match self {
            Self::FixedEpoch { jd_tdb } => jd_tdb,
            Self::Live => DEFAULT_START_EPOCH_JD_TDB,
        }
    }
}

impl From<StartMode> for StartModeSetting {
    fn from(value: StartMode) -> Self {
        match value {
            StartMode::FixedEpoch { jd_tdb } => Self::FixedEpoch { jd_tdb },
            StartMode::Live => Self::Live,
        }
    }
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct PersistedLayerState {
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

impl Default for PersistedLayerState {
    fn default() -> Self {
        Self::from_snapshot(LayerState::default().persistence_snapshot())
    }
}

impl PersistedLayerState {
    pub const fn from_snapshot(snapshot: LayerStateSnapshot) -> Self {
        Self {
            user_interface: snapshot.user_interface,
            planets: snapshot.planets,
            dwarf_planets: snapshot.dwarf_planets,
            asteroids: snapshot.asteroids,
            comets: snapshot.comets,
            moons: snapshot.moons,
            orbits: snapshot.orbits,
            labels: snapshot.labels,
            icons: snapshot.icons,
        }
    }

    pub const fn snapshot(self) -> LayerStateSnapshot {
        LayerStateSnapshot {
            user_interface: self.user_interface,
            planets: self.planets,
            dwarf_planets: self.dwarf_planets,
            asteroids: self.asteroids,
            comets: self.comets,
            moons: self.moons,
            orbits: self.orbits,
            labels: self.labels,
            icons: self.icons,
        }
    }

    fn visible(self, layer: LayerId) -> bool {
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

    fn toggle(&mut self, layer: LayerId) {
        match layer {
            LayerId::UserInterface => self.user_interface = !self.user_interface,
            LayerId::Planets => self.planets = !self.planets,
            LayerId::DwarfPlanets => self.dwarf_planets = !self.dwarf_planets,
            LayerId::Asteroids => self.asteroids = !self.asteroids,
            LayerId::Comets => self.comets = !self.comets,
            LayerId::Moons => self.moons = !self.moons,
            LayerId::Orbits => self.orbits = !self.orbits,
            LayerId::Labels => self.labels = !self.labels,
            LayerId::Icons => self.icons = !self.icons,
        }
    }
}

/// The complete persisted WP14 contract. Adding a field here changes the
/// settings snapshot test and is therefore always deliberate.
#[derive(Resource, SettingsGroup, Reflect, Debug, Clone, PartialEq)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "solar_sim")]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct AppSettings {
    pub display_mode: DisplayModeSetting,
    pub resolution: ResolutionSetting,
    pub vsync: bool,
    pub frame_cap: FrameCap,
    pub quality: QualityPreset,
    pub ui_scale: f32,
    pub units: DistanceUnit,
    pub start_mode: StartModeSetting,
    pub invert_horizontal: bool,
    pub invert_vertical: bool,
    pub layers: PersistedLayerState,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            display_mode: DisplayModeSetting::default(),
            resolution: ResolutionSetting::default(),
            vsync: true,
            frame_cap: FrameCap::default(),
            quality: QualityPreset::default(),
            ui_scale: 1.0,
            units: DistanceUnit::default(),
            start_mode: StartModeSetting::default(),
            invert_horizontal: false,
            invert_vertical: false,
            layers: PersistedLayerState::default(),
        }
    }
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        self.resolution.width = self.resolution.width.clamp(800, 7_680);
        self.resolution.height = self.resolution.height.clamp(600, 4_320);
        self.ui_scale = if self.ui_scale.is_finite() {
            self.ui_scale.clamp(0.75, 2.0)
        } else {
            1.0
        };
        if let StartModeSetting::FixedEpoch { jd_tdb } = &mut self.start_mode {
            if !jd_tdb.is_finite() {
                *jd_tdb = DEFAULT_START_EPOCH_JD_TDB;
            }
        }
        self
    }

    pub fn initial_layer_state(&self) -> LayerState {
        let mut layers = LayerState::default();
        layers.restore_persistence_snapshot(self.layers.snapshot());
        layers
    }

    pub fn initial_clock(&self, wall_now_t: f64) -> SimClock {
        SimClock::new(self.start_mode.to_core(), wall_now_t)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RenderFailureKind {
    #[default]
    DeviceLost,
    OutOfMemory,
    Unexpected,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RenderRecoveryPhase {
    #[default]
    Rendering,
    Recovering,
    StoppedOutOfMemory,
    StoppedUnexpected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryDirective {
    Recover,
    StopRendering,
}

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderRecoveryStatus {
    phase: RenderRecoveryPhase,
}

impl RenderRecoveryStatus {
    pub const fn phase(self) -> RenderRecoveryPhase {
        self.phase
    }

    pub fn handle_failure(&mut self, failure: RenderFailureKind) -> RecoveryDirective {
        match failure {
            RenderFailureKind::DeviceLost => {
                self.phase = RenderRecoveryPhase::Recovering;
                RecoveryDirective::Recover
            }
            RenderFailureKind::OutOfMemory => {
                self.phase = RenderRecoveryPhase::StoppedOutOfMemory;
                RecoveryDirective::StopRendering
            }
            RenderFailureKind::Unexpected => {
                self.phase = RenderRecoveryPhase::StoppedUnexpected;
                RecoveryDirective::StopRendering
            }
        }
    }

    fn recovered(&mut self) {
        if self.phase == RenderRecoveryPhase::Recovering {
            self.phase = RenderRecoveryPhase::Rendering;
        }
    }
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct SettingsScreenRoot;

#[derive(Component, Debug, Clone, Copy, Default)]
struct SettingsScrollArea;

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RenderErrorScreen;

#[derive(Resource, Debug, Clone, Default)]
pub(crate) struct SettingsScreenState {
    open: bool,
    draft: AppSettings,
    dirty: bool,
    scroll_y: f32,
    restore_focus: Option<SettingAction>,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct SettingsSaveRequest(bool);

impl SettingsScreenState {
    pub(crate) const fn is_open(&self) -> bool {
        self.open
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
enum SettingAction {
    Close,
    Apply,
    Revert,
    RestoreDefaults,
    SetDisplayMode(DisplayModeSetting),
    SetResolution(ResolutionSetting),
    ToggleVsync,
    SetFrameCap(FrameCap),
    SetQuality(QualityPreset),
    CycleUiScale,
    SetUnits(DistanceUnit),
    SetStartLive,
    SetStartFixed,
    AdjustStartEpoch(f64),
    ToggleInvertHorizontal,
    ToggleInvertVertical,
    ToggleLayer(LayerId),
}

pub struct ProductSettingsPlugin;

impl Plugin for ProductSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SettingsScreenState>()
            .init_resource::<SettingsSaveRequest>()
            .init_resource::<RenderRecoveryStatus>()
            .insert_resource(RenderErrorHandler(product_render_error_policy))
            .add_systems(
                Update,
                (
                    sync_requested_settings_screen,
                    apply_settings_to_runtime,
                    sync_external_presentation_to_settings,
                    persist_requested_settings,
                    rebuild_settings_screen,
                    sync_recovery_completion,
                    sync_render_error_screen,
                )
                    .chain()
                    .in_set(SimulationSet::Render),
            )
            .add_systems(Update, save_settings_on_window_close);

        #[cfg(debug_assertions)]
        install_debug_device_loss(app);
    }
}

pub(crate) fn consume_settings_command(
    command: &SimCommand,
    screen: &mut SettingsScreenState,
    settings: &mut AppSettings,
    save: &mut SettingsSaveRequest,
) {
    match command {
        SimCommand::ApplySettings(committed) => {
            let committed = committed.as_ref().clone().normalized();
            *settings = committed.clone();
            screen.draft = committed;
            screen.dirty = true;
            save.0 = true;
        }
        SimCommand::RestorePresentationDefaults => {
            let defaults = AppSettings::default();
            settings.layers = defaults.layers;
            screen.draft.layers = defaults.layers;
            screen.dirty = true;
            save.0 = true;
        }
        _ => {}
    }
}

pub(crate) fn reset_persisted_settings(world: &mut World) {
    *world.resource_mut::<AppSettings>() = AppSettings::default().normalized();
    SaveSettingsSync::Always.apply(world);
}

fn sync_requested_settings_screen(
    mut presentation: ResMut<PresentationState>,
    settings: Res<AppSettings>,
    mut screen: ResMut<SettingsScreenState>,
    mut focus: ResMut<InputFocus>,
) {
    if presentation.settings_close_requested() {
        presentation
            .bypass_change_detection()
            .take_settings_close_request();
        presentation.set_changed();
        screen.open = false;
        screen.draft = settings.clone();
        screen.dirty = true;
        screen.scroll_y = 0.0;
        screen.restore_focus = None;
        focus.clear();
    }
    if presentation.settings_requested() {
        presentation
            .bypass_change_detection()
            .take_settings_request();
        presentation.set_changed();
        screen.open = true;
        screen.draft = settings.clone();
        screen.dirty = true;
        screen.scroll_y = 0.0;
        screen.restore_focus = None;
        focus.clear();
    }
}

fn apply_settings_to_runtime(
    settings: Res<AppSettings>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut ui_scale: ResMut<UiScale>,
    mut winit: ResMut<WinitSettings>,
    mut cameras: Query<&mut Msaa, With<Camera3d>>,
) {
    if !settings.is_changed() {
        return;
    }
    let normalized = settings.clone().normalized();
    for mut window in &mut windows {
        window.mode = if normalized.display_mode.is_fullscreen() {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };
        window.resolution.set(
            normalized.resolution.width as f32,
            normalized.resolution.height as f32,
        );
        window.present_mode = if normalized.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        };
    }
    ui_scale.0 = normalized.ui_scale;
    let update_mode = normalized
        .frame_cap
        .hz()
        .map_or(UpdateMode::Continuous, |hz| {
            UpdateMode::reactive(Duration::from_secs_f64(1.0 / f64::from(hz)))
        });
    winit.focused_mode = update_mode;
    winit.unfocused_mode = update_mode;
    for mut msaa in &mut cameras {
        *msaa = normalized.quality.msaa();
    }
}

fn sync_external_presentation_to_settings(
    layers: Res<LayerState>,
    presentation: Res<PresentationState>,
    mut settings: ResMut<AppSettings>,
    mut commands: Commands,
) {
    // A settings-screen commit is authoritative for this frame. On the next
    // command pass its queued layer/fullscreen commands make both resources
    // converge, avoiding a one-frame write-back of the old state.
    if settings.is_changed() {
        return;
    }
    let persisted_layers = PersistedLayerState::from_snapshot(layers.persistence_snapshot());
    let display_mode = if presentation.is_fullscreen() {
        DisplayModeSetting::BorderlessFullscreen
    } else {
        DisplayModeSetting::Windowed
    };
    if settings.layers != persisted_layers || settings.display_mode != display_mode {
        settings.layers = persisted_layers;
        settings.display_mode = display_mode;
        commands.queue(SaveSettingsDeferred(SETTINGS_SAVE_DELAY));
    }
}

fn persist_requested_settings(mut request: ResMut<SettingsSaveRequest>, mut commands: Commands) {
    if std::mem::take(&mut request.0) {
        commands.queue(SaveSettingsDeferred(SETTINGS_SAVE_DELAY));
    }
}

fn save_settings_on_window_close(
    mut close: MessageReader<WindowCloseRequested>,
    mut commands: Commands,
) {
    if close.read().next().is_some() {
        commands.queue(SaveSettingsSync::IfChanged);
        commands.write_message(AppExit::Success);
    }
}

fn rebuild_settings_screen(
    mut commands: Commands,
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
    mut screen: ResMut<SettingsScreenState>,
    roots: Query<Entity, With<SettingsScreenRoot>>,
) {
    if !screen.dirty {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    if !screen.open {
        screen.dirty = false;
        screen.restore_focus = None;
        return;
    }

    let root = commands
        .spawn((
            Name::new("Settings screen"),
            SettingsScreenRoot,
            HudSurface,
            AccessibleLabel::new("Solar Sim settings"),
            TabGroup::modal(),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme.colors.scrim.color()),
            Pickable::default(),
            GlobalZIndex(SETTINGS_Z_INDEX),
        ))
        .id();
    let panel = commands
        .spawn((
            Name::new("Settings panel"),
            Node {
                width: percent(84),
                height: percent(90),
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
    commands.spawn((
        Text::new("SETTINGS"),
        TextFont {
            font: asset_server.load(INTER_FONT_ASSET).into(),
            font_size: theme.type_scale.product_px.into(),
            ..default()
        },
        TextColor(theme.colors.text_primary.color()),
        Pickable::IGNORE,
        ChildOf(panel),
    ));
    let scroll_position = ScrollPosition(Vec2::new(0.0, screen.scroll_y));
    let content = commands
        .spawn((
            Name::new("Settings scroll area"),
            SettingsScrollArea,
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
            scroll_position,
            ChildOf(panel),
        ))
        .observe(scroll_settings_content)
        .id();

    let draft = screen.draft.clone();
    spawn_section(&mut commands, content, *theme, "DISPLAY MODE");
    spawn_choices(
        &mut commands,
        content,
        *theme,
        DisplayModeSetting::ALL.map(|value| {
            (
                value.label().to_string(),
                draft.display_mode == value,
                SettingAction::SetDisplayMode(value),
            )
        }),
    );
    spawn_section(&mut commands, content, *theme, "RESOLUTION");
    spawn_choices(
        &mut commands,
        content,
        *theme,
        ResolutionSetting::PRESETS.map(|value| {
            (
                value.label(),
                draft.resolution == value,
                SettingAction::SetResolution(value),
            )
        }),
    );
    spawn_checkbox(
        &mut commands,
        content,
        *theme,
        "Vertical sync",
        draft.vsync,
        SettingAction::ToggleVsync,
    );
    spawn_section(&mut commands, content, *theme, "FRAME CAP");
    spawn_choices(
        &mut commands,
        content,
        *theme,
        FrameCap::ALL.map(|value| {
            (
                value.label().to_string(),
                draft.frame_cap == value,
                SettingAction::SetFrameCap(value),
            )
        }),
    );
    spawn_section(&mut commands, content, *theme, "QUALITY PRESET");
    spawn_choices(
        &mut commands,
        content,
        *theme,
        QualityPreset::ALL.map(|value| {
            (
                value.label().to_string(),
                draft.quality == value,
                SettingAction::SetQuality(value),
            )
        }),
    );
    commands
        .spawn_scene(slider(
            *theme,
            WidgetSpec::new(
                format!("UI SCALE  {:.0}%", draft.ui_scale * 100.0),
                "Cycle user interface scale",
                WidgetVisualState::Active,
            ),
        ))
        .insert((SettingAction::CycleUiScale, TabIndex(200), ChildOf(content)))
        .observe(activate_setting_action);

    spawn_section(&mut commands, content, *theme, "DISTANCE UNITS");
    spawn_choices(
        &mut commands,
        content,
        *theme,
        DistanceUnit::ALL.map(|value| {
            (
                value.label().to_string(),
                draft.units == value,
                SettingAction::SetUnits(value),
            )
        }),
    );

    spawn_section(&mut commands, content, *theme, "START TIME");
    let fixed_epoch = draft.start_mode.fixed_epoch();
    spawn_choices(
        &mut commands,
        content,
        *theme,
        [
            (
                "LIVE".to_string(),
                matches!(draft.start_mode, StartModeSetting::Live),
                SettingAction::SetStartLive,
            ),
            (
                format!("FIXED JD {fixed_epoch:.1}"),
                matches!(draft.start_mode, StartModeSetting::FixedEpoch { .. }),
                SettingAction::SetStartFixed,
            ),
            (
                "−1 YEAR".to_string(),
                false,
                SettingAction::AdjustStartEpoch(-365.25),
            ),
            (
                "+1 YEAR".to_string(),
                false,
                SettingAction::AdjustStartEpoch(365.25),
            ),
        ],
    );
    spawn_checkbox(
        &mut commands,
        content,
        *theme,
        "Invert horizontal orbit axis",
        draft.invert_horizontal,
        SettingAction::ToggleInvertHorizontal,
    );
    spawn_checkbox(
        &mut commands,
        content,
        *theme,
        "Invert vertical orbit axis",
        draft.invert_vertical,
        SettingAction::ToggleInvertVertical,
    );

    spawn_section(&mut commands, content, *theme, "LAYERS");
    for layer in LayerId::ALL {
        spawn_checkbox(
            &mut commands,
            content,
            *theme,
            layer.label(),
            draft.layers.visible(layer),
            SettingAction::ToggleLayer(layer),
        );
    }

    let footer = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(42),
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                column_gap: px(theme.spacing.sm_px),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    spawn_choices(
        &mut commands,
        footer,
        *theme,
        [
            ("CLOSE".to_string(), false, SettingAction::Close),
            ("REVERT".to_string(), false, SettingAction::Revert),
            (
                "RESTORE DEFAULTS".to_string(),
                false,
                SettingAction::RestoreDefaults,
            ),
            ("APPLY".to_string(), true, SettingAction::Apply),
        ],
    );
    if let Some(action) = screen.restore_focus.take() {
        commands.queue(move |world: &mut World| {
            let focused = {
                let mut actions = world.query::<(Entity, &SettingAction)>();
                actions
                    .iter(world)
                    .find_map(|(entity, candidate)| (*candidate == action).then_some(entity))
            };
            if let Some(entity) = focused {
                world
                    .resource_mut::<InputFocus>()
                    .set(entity, FocusCause::Navigated);
            }
        });
    }
    screen.dirty = false;
}

fn scroll_settings_content(
    mut scroll: On<Pointer<Scroll>>,
    mut areas: Query<(&mut ScrollPosition, &ComputedNode), With<SettingsScrollArea>>,
    mut screen: ResMut<SettingsScreenState>,
) {
    let Ok((mut position, node)) = areas.get_mut(scroll.entity) else {
        return;
    };
    position.y = next_settings_scroll_y(
        position.y,
        scroll.y,
        scroll.unit,
        node.content_size().y,
        node.size().y,
        node.inverse_scale_factor,
    );
    screen.scroll_y = position.y;
    scroll.propagate(false);
}

fn next_settings_scroll_y(
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

fn spawn_section(commands: &mut Commands, parent: Entity, theme: UiTheme, label: &str) {
    commands
        .spawn_scene(section_header(
            theme,
            WidgetSpec::new(
                label,
                format!("{label} settings"),
                WidgetVisualState::Default,
            ),
        ))
        .insert(ChildOf(parent));
}

fn spawn_choices<const N: usize>(
    commands: &mut Commands,
    parent: Entity,
    theme: UiTheme,
    choices: [(String, bool, SettingAction); N],
) {
    let row = commands
        .spawn((
            Node {
                width: percent(100),
                min_height: px(34),
                align_items: AlignItems::Center,
                column_gap: px(theme.spacing.sm_px),
                flex_wrap: FlexWrap::Wrap,
                row_gap: px(theme.spacing.sm_px),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    for (label, selected, action) in choices {
        let mut entity = commands.spawn_scene(chip(
            theme,
            WidgetSpec::new(
                &label,
                format!("Set {label}"),
                if selected {
                    WidgetVisualState::Active
                } else {
                    WidgetVisualState::Default
                },
            ),
        ));
        entity
            .insert((action, TabIndex(200), ChildOf(row)))
            .observe(activate_setting_action);
    }
}

fn spawn_checkbox(
    commands: &mut Commands,
    parent: Entity,
    theme: UiTheme,
    label: &str,
    checked: bool,
    action: SettingAction,
) {
    commands
        .spawn_scene(checkbox_row(
            theme,
            WidgetSpec::new(
                label,
                format!("Toggle {label}"),
                if checked {
                    WidgetVisualState::Active
                } else {
                    WidgetVisualState::Default
                },
            ),
        ))
        .insert((action, TabIndex(200), ChildOf(parent)))
        .observe(activate_setting_action);
}

#[allow(clippy::too_many_arguments)]
fn activate_setting_action(
    activate: On<Activate>,
    actions: Query<&SettingAction>,
    mut screen: ResMut<SettingsScreenState>,
    settings: Res<AppSettings>,
    focus: Res<InputFocus>,
    mut sim_commands: ResMut<SimCommandQueue>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    if focus.get() == Some(activate.entity)
        && !matches!(
            action,
            SettingAction::Close | SettingAction::Apply | SettingAction::RestoreDefaults
        )
    {
        screen.restore_focus = Some(*action);
    }
    match *action {
        SettingAction::Close => {
            sim_commands.push(SimCommand::CloseSettings);
        }
        SettingAction::Apply => {
            let committed = screen.draft.clone().normalized();
            sim_commands.push(SimCommand::ApplySettings(Box::new(committed)));
            sim_commands.push(SimCommand::CloseSettings);
        }
        SettingAction::Revert => screen.draft = settings.clone(),
        SettingAction::RestoreDefaults => {
            sim_commands.push(SimCommand::ApplySettings(Box::default()));
            sim_commands.push(SimCommand::RestorePresentationDefaults);
            sim_commands.push(SimCommand::CloseSettings);
        }
        SettingAction::SetDisplayMode(value) => screen.draft.display_mode = value,
        SettingAction::SetResolution(value) => screen.draft.resolution = value,
        SettingAction::ToggleVsync => screen.draft.vsync = !screen.draft.vsync,
        SettingAction::SetFrameCap(value) => screen.draft.frame_cap = value,
        SettingAction::SetQuality(value) => screen.draft.quality = value,
        SettingAction::CycleUiScale => {
            screen.draft.ui_scale = match screen.draft.ui_scale {
                value if value < 1.0 => 1.0,
                value if value < 1.25 => 1.25,
                value if value < 1.5 => 1.5,
                _ => 0.75,
            };
        }
        SettingAction::SetUnits(value) => screen.draft.units = value,
        SettingAction::SetStartLive => screen.draft.start_mode = StartModeSetting::Live,
        SettingAction::SetStartFixed => {
            screen.draft.start_mode = StartModeSetting::FixedEpoch {
                jd_tdb: screen.draft.start_mode.fixed_epoch(),
            };
        }
        SettingAction::AdjustStartEpoch(delta_days) => {
            screen.draft.start_mode = StartModeSetting::FixedEpoch {
                jd_tdb: screen.draft.start_mode.fixed_epoch() + delta_days,
            };
        }
        SettingAction::ToggleInvertHorizontal => {
            screen.draft.invert_horizontal = !screen.draft.invert_horizontal;
        }
        SettingAction::ToggleInvertVertical => {
            screen.draft.invert_vertical = !screen.draft.invert_vertical;
        }
        SettingAction::ToggleLayer(layer) => screen.draft.layers.toggle(layer),
    }
    screen.dirty = true;
}

fn product_render_error_policy(
    error: &RenderError,
    main_world: &mut World,
    _render_world: &mut World,
) -> RenderErrorPolicy {
    let failure = match error.ty {
        ErrorType::DeviceLost => RenderFailureKind::DeviceLost,
        ErrorType::OutOfMemory => RenderFailureKind::OutOfMemory,
        _ => RenderFailureKind::Unexpected,
    };
    let directive = main_world
        .resource_mut::<RenderRecoveryStatus>()
        .handle_failure(failure);
    if directive == RecoveryDirective::StopRendering {
        let mut windows = main_world.query_filtered::<&mut Window, With<PrimaryWindow>>();
        for mut window in windows.iter_mut(main_world) {
            window.title = if failure == RenderFailureKind::OutOfMemory {
                "Solar Sim — out of graphics memory; rendering stopped".into()
            } else {
                "Solar Sim — rendering stopped after an unexpected GPU error".into()
            };
        }
    }
    match directive {
        RecoveryDirective::Recover => RenderErrorPolicy::Recover(RenderCreation::default()),
        RecoveryDirective::StopRendering => RenderErrorPolicy::StopRendering,
    }
}

fn sync_recovery_completion(
    device: Option<Res<RenderDevice>>,
    mut recovery: ResMut<RenderRecoveryStatus>,
) {
    if recovery.phase == RenderRecoveryPhase::Recovering
        && device.is_some_and(|device| device.is_changed())
    {
        recovery.recovered();
    }
}

fn sync_render_error_screen(
    mut commands: Commands,
    recovery: Res<RenderRecoveryStatus>,
    screens: Query<Entity, With<RenderErrorScreen>>,
) {
    let message = match recovery.phase {
        RenderRecoveryPhase::StoppedOutOfMemory => Some(
            "SOLAR SIM RAN OUT OF GRAPHICS MEMORY\n\nRendering has stopped safely. Close the app, free graphics memory, and relaunch.",
        ),
        RenderRecoveryPhase::StoppedUnexpected => Some(
            "SOLAR SIM STOPPED RENDERING\n\nAn unexpected graphics error occurred. Close and relaunch the app.",
        ),
        _ => None,
    };
    if let Some(message) = message {
        if screens.is_empty() {
            commands.spawn((
                Name::new("Render error screen"),
                RenderErrorScreen,
                AccessibleLabel::new(message),
                Text::new(message),
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(15),
                    right: percent(15),
                    top: percent(30),
                    ..default()
                },
                GlobalZIndex(SETTINGS_Z_INDEX + 20),
            ));
        }
    } else {
        for screen in &screens {
            commands.entity(screen).despawn();
        }
    }
}

#[cfg(debug_assertions)]
#[derive(Resource, bevy::render::extract_resource::ExtractResource, Clone, Default)]
pub(crate) struct DebugDeviceLossRequest {
    token: u64,
}

#[cfg(debug_assertions)]
pub(crate) fn request_debug_device_loss(request: &mut DebugDeviceLossRequest) {
    request.token = request.token.wrapping_add(1).max(1);
}

#[cfg(debug_assertions)]
fn install_debug_device_loss(app: &mut App) {
    use bevy::render::{extract_resource::ExtractResourcePlugin, Render, RenderApp, RenderSystems};

    app.init_resource::<DebugDeviceLossRequest>()
        .add_plugins(ExtractResourcePlugin::<DebugDeviceLossRequest>::default());
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.add_systems(
            Render,
            simulate_device_loss.in_set(RenderSystems::PostCleanup),
        );
    }
}

#[cfg(debug_assertions)]
fn simulate_device_loss(
    request: Option<Res<DebugDeviceLossRequest>>,
    device: Res<RenderDevice>,
    mut last_token: Local<u64>,
) {
    use bevy::render::render_resource::PollType;

    let Some(request) = request else {
        return;
    };
    if request.token == 0 || request.token == *last_token {
        return;
    }
    *last_token = request.token;
    device.wgpu_device().destroy();
    if let Err(error) = device.poll(PollType::wait_indefinitely()) {
        bevy::log::warn!("debug device-loss poll returned {error:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WidgetRoot;
    use bevy::{
        a11y::AccessibilityNode,
        app::TaskPoolPlugin,
        asset::{AssetApp, AssetPlugin},
        scene::ScenePlugin,
        settings::SettingsPlugin as BevySettingsPlugin,
        text::Font,
    };
    use sim_core::time::T_MAX_S;
    use std::process::Command as ProcessCommand;

    fn nondefault_settings() -> AppSettings {
        AppSettings {
            display_mode: DisplayModeSetting::BorderlessFullscreen,
            resolution: ResolutionSetting {
                width: 2_560,
                height: 1_440,
            },
            vsync: false,
            frame_cap: FrameCap::Fps240,
            quality: QualityPreset::Ultra,
            ui_scale: 1.5,
            units: DistanceUnit::AstronomicalUnits,
            start_mode: StartModeSetting::FixedEpoch {
                jd_tdb: 2_451_545.25,
            },
            invert_horizontal: true,
            invert_vertical: true,
            layers: PersistedLayerState {
                user_interface: true,
                planets: false,
                dwarf_planets: true,
                asteroids: false,
                comets: true,
                moons: false,
                orbits: true,
                labels: false,
                icons: true,
            },
        }
    }

    fn persistence_test_app() -> App {
        let mut app = App::new();
        app.register_type::<AppSettings>()
            .add_plugins(BevySettingsPlugin::new(SETTINGS_IDENTIFIER));
        app
    }

    #[test]
    fn full_settings_struct_round_trips_through_serde() {
        let settings = nondefault_settings();
        let encoded = ron::to_string(&settings).expect("settings serialize");
        let decoded: AppSettings = ron::from_str(&encoded).expect("settings deserialize");
        assert_eq!(decoded, settings);
    }

    #[test]
    fn settings_survive_full_process_relaunch() {
        const PHASE_ENV: &str = "SOLAR_SIM_SETTINGS_TEST_PHASE";
        match std::env::var(PHASE_ENV).as_deref() {
            Ok("write") => {
                let mut app = persistence_test_app();
                *app.world_mut().resource_mut::<AppSettings>() = nondefault_settings();
                SaveSettingsSync::Always.apply(app.world_mut());
                return;
            }
            Ok("read") => {
                let app = persistence_test_app();
                assert_eq!(
                    app.world().resource::<AppSettings>(),
                    &nondefault_settings()
                );
                return;
            }
            Ok("reset") => {
                let mut app = persistence_test_app();
                reset_persisted_settings(app.world_mut());
                return;
            }
            Ok("read-reset") => {
                let app = persistence_test_app();
                assert_eq!(
                    app.world().resource::<AppSettings>(),
                    &AppSettings::default()
                );
                return;
            }
            _ => {}
        }

        let temporary_home = std::env::temp_dir().join(format!(
            "solar-sim-settings-relaunch-test-{}",
            std::process::id()
        ));
        std::fs::create_dir(&temporary_home).expect("create isolated settings HOME");
        let executable = std::env::current_exe().expect("current test executable");
        for phase in ["write", "read", "reset", "read-reset"] {
            let status = ProcessCommand::new(&executable)
                .arg("settings::tests::settings_survive_full_process_relaunch")
                .arg("--exact")
                .arg("--nocapture")
                .env("HOME", &temporary_home)
                .env(PHASE_ENV, phase)
                .status()
                .expect("launch isolated settings process");
            assert!(status.success(), "settings {phase} process failed");
        }
        std::fs::remove_dir_all(&temporary_home).expect("remove isolated settings HOME");
    }

    #[test]
    fn fixed_and_live_start_modes_initialize_the_requested_clock() {
        let wall_now_t = 12_345.0;
        let fixed = AppSettings {
            start_mode: StartModeSetting::FixedEpoch {
                jd_tdb: 2_451_545.25,
            },
            ..default()
        };
        assert_eq!(fixed.initial_clock(wall_now_t).t(), 21_600.0);

        let live = AppSettings {
            start_mode: StartModeSetting::Live,
            ..default()
        };
        assert_eq!(live.initial_clock(wall_now_t).t(), wall_now_t);

        let beyond_range = AppSettings {
            start_mode: StartModeSetting::FixedEpoch {
                jd_tdb: 3_000_000.0,
            },
            ..default()
        };
        assert_eq!(beyond_range.initial_clock(wall_now_t).t(), T_MAX_S);
    }

    #[test]
    fn recovery_state_machine_recovers_loss_and_stops_on_oom() {
        let mut state = RenderRecoveryStatus::default();
        assert_eq!(
            state.handle_failure(RenderFailureKind::DeviceLost),
            RecoveryDirective::Recover
        );
        assert_eq!(state.phase(), RenderRecoveryPhase::Recovering);
        state.recovered();
        assert_eq!(state.phase(), RenderRecoveryPhase::Rendering);

        assert_eq!(
            state.handle_failure(RenderFailureKind::OutOfMemory),
            RecoveryDirective::StopRendering
        );
        assert_eq!(state.phase(), RenderRecoveryPhase::StoppedOutOfMemory);
    }

    #[test]
    fn persisted_layers_restore_every_layer_exactly() {
        let mut original = LayerState::default();
        original.set_visible(LayerId::Asteroids, false);
        original.set_visible(LayerId::Labels, false);
        let persisted = PersistedLayerState::from_snapshot(original.persistence_snapshot());
        let settings = AppSettings {
            layers: persisted,
            ..default()
        };
        assert_eq!(settings.initial_layer_state(), original);
    }

    #[test]
    fn display_mode_is_staged_until_apply_and_revert_discards_it() {
        let mut app = App::new();
        app.insert_resource(AppSettings::default())
            .init_resource::<SettingsScreenState>()
            .init_resource::<SettingsSaveRequest>()
            .init_resource::<LayerState>()
            .init_resource::<PresentationState>()
            .init_resource::<InputFocus>()
            .init_resource::<SimCommandQueue>();
        let set = app
            .world_mut()
            .spawn(SettingAction::SetDisplayMode(
                DisplayModeSetting::BorderlessFullscreen,
            ))
            .observe(activate_setting_action)
            .id();
        let revert = app
            .world_mut()
            .spawn(SettingAction::Revert)
            .observe(activate_setting_action)
            .id();
        let apply = app
            .world_mut()
            .spawn(SettingAction::Apply)
            .observe(activate_setting_action)
            .id();

        app.world_mut().trigger(Activate { entity: set });
        assert_eq!(
            app.world().resource::<AppSettings>().display_mode,
            DisplayModeSetting::Windowed
        );
        app.world_mut().trigger(Activate { entity: revert });
        assert_eq!(
            app.world()
                .resource::<SettingsScreenState>()
                .draft
                .display_mode,
            DisplayModeSetting::Windowed
        );

        app.world_mut().trigger(Activate { entity: set });
        app.world_mut().trigger(Activate { entity: apply });
        assert_eq!(
            app.world().resource::<AppSettings>().display_mode,
            DisplayModeSetting::Windowed,
            "widget activation must not bypass the command reducer"
        );
        let commands: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(commands.len(), 2);
        assert!(matches!(
            &commands[0],
            SimCommand::ApplySettings(settings)
                if settings.display_mode == DisplayModeSetting::BorderlessFullscreen
        ));
        assert_eq!(commands[1], SimCommand::CloseSettings);
        let apply = commands[0].clone();
        let mut screen = app
            .world_mut()
            .remove_resource::<SettingsScreenState>()
            .unwrap();
        let mut settings = app.world_mut().remove_resource::<AppSettings>().unwrap();
        let mut save = app
            .world_mut()
            .remove_resource::<SettingsSaveRequest>()
            .unwrap();
        consume_settings_command(&apply, &mut screen, &mut settings, &mut save);
        app.insert_resource(screen)
            .insert_resource(settings)
            .insert_resource(save);
        assert_eq!(
            app.world().resource::<AppSettings>().display_mode,
            DisplayModeSetting::BorderlessFullscreen
        );
    }

    #[test]
    fn close_action_routes_through_the_command_queue() {
        let mut app = App::new();
        app.insert_resource(AppSettings::default())
            .init_resource::<SettingsScreenState>()
            .init_resource::<LayerState>()
            .init_resource::<PresentationState>()
            .init_resource::<InputFocus>()
            .init_resource::<SimCommandQueue>();
        let close = app
            .world_mut()
            .spawn(SettingAction::Close)
            .observe(activate_setting_action)
            .id();

        app.world_mut().trigger(Activate { entity: close });
        let commands: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(commands, vec![SimCommand::CloseSettings]);
    }

    #[test]
    fn restore_defaults_routes_commit_presentation_and_close_in_order() {
        let mut app = App::new();
        app.insert_resource(nondefault_settings())
            .insert_resource(SettingsScreenState {
                open: true,
                draft: nondefault_settings(),
                dirty: false,
                scroll_y: 90.0,
                restore_focus: None,
            })
            .init_resource::<SettingsSaveRequest>()
            .init_resource::<InputFocus>()
            .init_resource::<SimCommandQueue>();
        let restore = app
            .world_mut()
            .spawn(SettingAction::RestoreDefaults)
            .observe(activate_setting_action)
            .id();

        app.world_mut().trigger(Activate { entity: restore });
        let commands: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(
            commands,
            vec![
                SimCommand::ApplySettings(Box::default()),
                SimCommand::RestorePresentationDefaults,
                SimCommand::CloseSettings,
            ]
        );

        let mut screen = app
            .world_mut()
            .remove_resource::<SettingsScreenState>()
            .unwrap();
        let mut settings = app.world_mut().remove_resource::<AppSettings>().unwrap();
        let mut save = app
            .world_mut()
            .remove_resource::<SettingsSaveRequest>()
            .unwrap();
        for command in &commands {
            consume_settings_command(command, &mut screen, &mut settings, &mut save);
        }
        assert_eq!(settings, AppSettings::default());
        assert_eq!(screen.draft, AppSettings::default());
        assert!(screen.dirty);
        assert!(save.0);
    }

    #[test]
    fn requested_close_dismisses_the_modal_and_clears_focus() {
        let mut app = App::new();
        app.insert_resource(AppSettings::default())
            .insert_resource(SettingsScreenState {
                open: true,
                draft: AppSettings::default(),
                dirty: false,
                scroll_y: 0.0,
                restore_focus: None,
            })
            .init_resource::<PresentationState>()
            .init_resource::<InputFocus>()
            .add_systems(Update, sync_requested_settings_screen);
        app.world_mut()
            .resource_mut::<PresentationState>()
            .request_settings_close();

        app.update();

        let screen = app.world().resource::<SettingsScreenState>();
        assert!(!screen.open);
        assert!(screen.dirty);
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
    }

    #[test]
    fn settings_scroll_clamps_for_line_and_pixel_input() {
        assert_eq!(
            next_settings_scroll_y(100.0, -2.0, MouseScrollUnit::Line, 1_000.0, 600.0, 1.0,),
            156.0
        );
        assert_eq!(
            next_settings_scroll_y(390.0, -50.0, MouseScrollUnit::Pixel, 1_000.0, 600.0, 1.0,),
            400.0
        );
        assert_eq!(
            next_settings_scroll_y(10.0, 50.0, MouseScrollUnit::Pixel, 1_000.0, 600.0, 1.0,),
            0.0
        );
    }

    #[test]
    fn settings_screen_renders_every_control_with_accessibility_labels() {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ))
        .init_asset::<Font>()
        .insert_resource(UiTheme::default())
        .insert_resource(AppSettings::default())
        .init_resource::<LayerState>()
        .init_resource::<PresentationState>()
        .init_resource::<SimCommandQueue>()
        .init_resource::<InputFocus>()
        .insert_resource(SettingsScreenState {
            open: true,
            draft: AppSettings::default(),
            dirty: true,
            scroll_y: 0.0,
            restore_focus: None,
        })
        .add_systems(Startup, rebuild_settings_screen)
        .add_systems(
            Update,
            (sync_requested_settings_screen, rebuild_settings_screen).chain(),
        );
        app.update();
        {
            let mut screen = app.world_mut().resource_mut::<SettingsScreenState>();
            screen.scroll_y = 137.0;
            screen.dirty = true;
        }
        app.update();

        let vsync = {
            let world = app.world_mut();
            let mut actions = world.query::<(Entity, &SettingAction)>();
            actions
                .iter(world)
                .find_map(|(entity, action)| {
                    (*action == SettingAction::ToggleVsync).then_some(entity)
                })
                .unwrap()
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(vsync, FocusCause::Navigated);
        app.world_mut().trigger(Activate { entity: vsync });
        app.update();
        let focused = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().entity(focused).get::<SettingAction>(),
            Some(&SettingAction::ToggleVsync)
        );

        let close = {
            let world = app.world_mut();
            let mut roots = world.query::<(&SettingsScreenRoot, &TabGroup)>();
            let roots: Vec<_> = roots.iter(world).map(|(_, group)| group.modal).collect();
            assert_eq!(roots.len(), 1);
            assert!(roots[0]);
            let mut scroll_areas = world.query::<(&SettingsScrollArea, &ScrollPosition)>();
            let scroll_positions: Vec<_> = scroll_areas
                .iter(world)
                .map(|(_, position)| position.y)
                .collect();
            assert_eq!(scroll_positions, vec![137.0]);
            let mut close_controls =
                world.query_filtered::<(Entity, &SettingAction), With<WidgetRoot>>();
            let close_controls: Vec<_> = close_controls
                .iter(world)
                .filter_map(|(entity, action)| {
                    matches!(action, SettingAction::Close).then_some(entity)
                })
                .collect();
            assert_eq!(close_controls.len(), 1);
            let mut controls = world.query::<(
                &SettingAction,
                &WidgetRoot,
                &bevy::ui_widgets::Button,
                &AccessibleLabel,
                &AccessibilityNode,
            )>();
            let controls: Vec<_> = controls.iter(world).collect();
            assert_eq!(controls.len(), 39);
            assert!(controls
                .iter()
                .all(|(_, _, _, label, _)| !label.0.trim().is_empty()));
            close_controls[0]
        };

        app.world_mut().trigger(Activate { entity: close });
        let commands: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(commands, vec![SimCommand::CloseSettings]);
        app.world_mut()
            .resource_mut::<PresentationState>()
            .request_settings_close();
        app.update();
        let world = app.world_mut();
        let mut roots = world.query::<&SettingsScreenRoot>();
        assert_eq!(roots.iter(world).count(), 0);
    }
}
