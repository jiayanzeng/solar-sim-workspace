//! WP14 persistent product settings and render recovery — Rev C §§4.2 and 8.5.
//!
//! Bevy's 0.19 settings framework owns disk I/O. This module owns the stable
//! reflected schema, applies loaded values at presentation boundaries, exposes
//! one settings-screen model, and installs the renderer's explicit recovery
//! policy. The core simulation only receives the persisted `StartMode` at boot.

use crate::control::{SimCommand, SimCommandQueue};
use crate::input_intent::{ModalSurfaceSet, UiScrollSurface};
use crate::layers::{HudSurface, LayerId, LayerState, LayerStateSnapshot, PresentationState};
use crate::native_error_surface::show_out_of_memory_alert;
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
    post_process::bloom::Bloom,
    prelude::*,
    render::{
        error_handler::{ErrorType, RenderError, RenderErrorHandler, RenderErrorPolicy},
        render_resource::TextureFormat,
        renderer::{RenderAdapter, RenderDevice},
        settings::RenderCreation,
        view::Msaa,
        RenderApp,
    },
    settings::{ReflectSettingsGroup, SaveSettingsDeferred, SaveSettingsSync, SettingsGroup},
    ui::UiScale,
    ui_widgets::Activate,
    window::{MonitorSelection, PresentMode, PrimaryWindow, WindowCloseRequested, WindowMode},
    winit::{UpdateMode, WinitSettings},
};
use sim_core::time::{
    jd_tdb_from_t, RateIndex, SimClock, StartMode, DEFAULT_START_EPOCH_JD_TDB, T_MAX_S, T_MIN_S,
};
use std::{
    sync::Arc,
    thread::{self, ThreadId},
    time::Duration,
};

pub const SETTINGS_IDENTIFIER: &str = "com.github.jiayanzeng.solar-sim";
const SETTINGS_SAVE_DELAY: Duration = Duration::from_millis(100);
pub(crate) const SETTINGS_Z_INDEX: i32 = 140;
const SETTINGS_FIRST_TAB_INDEX: i32 = 200;

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

    pub(crate) const fn slug(self) -> &'static str {
        match self {
            Self::Fps30 => "30",
            Self::Fps60 => "60",
            Self::Fps120 => "120",
            Self::Fps240 => "240",
            Self::Unlimited => "unlimited",
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

    pub(crate) const fn slug(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Ultra => "ultra",
        }
    }

    pub(crate) fn from_slug(value: &str) -> Option<Self> {
        match value {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "ultra" => Some(Self::Ultra),
            _ => None,
        }
    }

    pub(crate) const fn requested_msaa(self) -> Msaa {
        match self {
            Self::Low => Msaa::Off,
            Self::Medium => Msaa::Sample2,
            Self::High => Msaa::Sample4,
            Self::Ultra => Msaa::Sample8,
        }
    }

    fn display_label(self, capabilities: MsaaCapabilities) -> String {
        let requested = self.requested_msaa();
        let effective = capabilities.resolve(requested);
        let label = format!("{} — {}", self.label(), msaa_ui_label(requested));
        if requested == effective {
            label
        } else {
            format!("{label} ({} ON THIS DEVICE)", msaa_ui_label(effective))
        }
    }

    const fn bloom_enabled(self) -> bool {
        !matches!(self, Self::Low)
    }

    const fn scale_factor_override(self, retina_rendering: bool) -> Option<f32> {
        if matches!(self, Self::Low) || !retina_rendering {
            Some(1.0)
        } else {
            None
        }
    }
}

const fn msaa_sample_count(msaa: Msaa) -> u8 {
    match msaa {
        Msaa::Off => 1,
        Msaa::Sample2 => 2,
        Msaa::Sample4 => 4,
        Msaa::Sample8 => 8,
    }
}

const fn msaa_ui_label(msaa: Msaa) -> &'static str {
    match msaa {
        Msaa::Off => "OFF",
        Msaa::Sample2 => "2×",
        Msaa::Sample4 => "4×",
        Msaa::Sample8 => "8×",
    }
}

/// Intersection of the HDR color and depth-format sample counts reported by
/// the active adapter. Presets retain stable requests; only this resource
/// resolves the value that may reach a camera.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MsaaCapabilities {
    supported_mask: u8,
}

impl Default for MsaaCapabilities {
    fn default() -> Self {
        Self {
            supported_mask: 1 | 2 | 4 | 8,
        }
    }
}

impl MsaaCapabilities {
    fn from_adapter(adapter: &RenderAdapter) -> Self {
        let color = adapter
            .get_texture_format_features(TextureFormat::Rgba16Float)
            .flags;
        let depth = adapter
            .get_texture_format_features(TextureFormat::Depth32Float)
            .flags;
        let mut supported_mask = 1;
        for count in [2, 4, 8] {
            if color.sample_count_supported(count) && depth.sample_count_supported(count) {
                supported_mask |= count as u8;
            }
        }
        Self { supported_mask }
    }

    #[cfg(test)]
    fn from_supported_counts(color: &[u8], depth: &[u8]) -> Self {
        let mut supported_mask = 1;
        for count in [2, 4, 8] {
            if color.contains(&count) && depth.contains(&count) {
                supported_mask |= count;
            }
        }
        Self { supported_mask }
    }

    pub(crate) fn resolve(self, requested: Msaa) -> Msaa {
        let requested = msaa_sample_count(requested);
        for (count, msaa) in [
            (8, Msaa::Sample8),
            (4, Msaa::Sample4),
            (2, Msaa::Sample2),
            (1, Msaa::Off),
        ] {
            if count <= requested && self.supported_mask & count != 0 {
                return msaa;
            }
        }
        Msaa::Off
    }
}

const fn default_retina_rendering() -> bool {
    true
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

fn fixed_epoch_bounds_jd_tdb() -> (f64, f64) {
    (jd_tdb_from_t(T_MIN_S), jd_tdb_from_t(T_MAX_S))
}

fn normalized_fixed_epoch_jd_tdb(jd_tdb: f64) -> f64 {
    if !jd_tdb.is_finite() {
        return DEFAULT_START_EPOCH_JD_TDB;
    }
    let (minimum, maximum) = fixed_epoch_bounds_jd_tdb();
    jd_tdb.clamp(minimum, maximum)
}

fn fixed_epoch_label(jd_tdb: f64) -> String {
    // f64's display form is the shortest representation that round-trips,
    // so the edge shown to the user describes the exact persisted value.
    format!("FIXED JD {jd_tdb}")
}

impl From<StartMode> for StartModeSetting {
    fn from(value: StartMode) -> Self {
        match value {
            StartMode::FixedEpoch { jd_tdb } => Self::FixedEpoch { jd_tdb },
            StartMode::Live => Self::Live,
        }
    }
}

/// Persisted startup selection over the frozen 24-step simulation-rate ladder.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct StartupRateSetting(i8);

impl Default for StartupRateSetting {
    fn default() -> Self {
        // The core ladder freezes +4 as +1 day/s. Keep the product default in
        // the settings layer so SimClock construction remains exactly +REAL.
        Self(4)
    }
}

impl StartupRateSetting {
    pub(crate) const fn from_raw(value: i8) -> Self {
        Self(value)
    }

    pub fn rate(self) -> RateIndex {
        RateIndex::new(self.0).unwrap_or_else(|| Self::default().rate())
    }

    pub(crate) fn real_time() -> Self {
        Self(RateIndex::REAL.get())
    }

    fn normalized(self) -> Self {
        Self(self.rate().get())
    }

    fn next(self) -> Self {
        let current = self.rate().get();
        Self(match current {
            12 => -12,
            -1 => 1,
            value => value + 1,
        })
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
    #[cfg_attr(test, serde(default = "default_retina_rendering"))]
    pub retina_rendering: bool,
    pub ui_scale: f32,
    pub units: DistanceUnit,
    pub start_mode: StartModeSetting,
    #[cfg_attr(test, serde(default))]
    pub startup_rate: StartupRateSetting,
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
            retina_rendering: default_retina_rendering(),
            ui_scale: 1.0,
            units: DistanceUnit::default(),
            start_mode: StartModeSetting::default(),
            startup_rate: StartupRateSetting::default(),
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
            *jd_tdb = normalized_fixed_epoch_jd_tdb(*jd_tdb);
        }
        self.startup_rate = self.startup_rate.normalized();
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
    native_oom_surface_invoked: bool,
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

    fn take_native_oom_surface_request(&mut self) -> bool {
        if self.phase == RenderRecoveryPhase::StoppedOutOfMemory && !self.native_oom_surface_invoked
        {
            self.native_oom_surface_invoked = true;
            true
        } else {
            false
        }
    }

    fn recovered(&mut self) {
        if self.phase == RenderRecoveryPhase::Recovering {
            self.phase = RenderRecoveryPhase::Rendering;
        }
    }
}

type NativeOomSurfaceCallback = dyn Fn(&str, &str) -> Result<(), String> + Send + Sync + 'static;

#[derive(Resource, Clone)]
struct NativeOomSurface(Arc<NativeOomSurfaceCallback>);

impl NativeOomSurface {
    #[cfg(test)]
    fn new(callback: impl Fn(&str, &str) -> Result<(), String> + Send + Sync + 'static) -> Self {
        Self(Arc::new(callback))
    }

    fn show(&self, title: &str, message: &str) -> Result<(), String> {
        (self.0)(title, message)
    }
}

impl Default for NativeOomSurface {
    fn default() -> Self {
        Self(Arc::new(show_out_of_memory_alert))
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
struct WinitApplicationThread(ThreadId);

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct SettingsScreenRoot;

#[derive(Component, Debug, Clone, Copy, Default)]
struct RetinaRenderingDescription;

#[derive(Component, Debug, Clone, Copy, Default)]
struct SettingsScrollArea;

#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub(crate) struct SettingsScreenState {
    open: bool,
    draft: AppSettings,
    dirty: bool,
    scroll_y: f32,
    restore_focus: Option<SettingAction>,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct SettingsSaveRequest(bool);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeSettingsKey {
    display_mode: DisplayModeSetting,
    resolution: ResolutionSetting,
    vsync: bool,
    frame_cap: FrameCap,
    quality: QualityPreset,
    retina_rendering: bool,
    ui_scale_bits: u32,
}

impl RuntimeSettingsKey {
    fn from_settings(settings: &AppSettings) -> Self {
        Self {
            display_mode: settings.display_mode,
            resolution: settings.resolution,
            vsync: settings.vsync,
            frame_cap: settings.frame_cap,
            quality: settings.quality,
            retina_rendering: settings.retina_rendering,
            ui_scale_bits: settings.ui_scale.to_bits(),
        }
    }
}

#[derive(Resource, Debug, Default)]
struct AppliedRuntimeSettings(Option<RuntimeSettingsKey>);

/// Controls whether runtime settings changes may reach the product settings
/// file. Golden capture deliberately uses transient overrides while ordinary
/// launches retain WP14 persistence.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum SettingsPersistencePolicy {
    #[default]
    Persistent,
    TransientRuntime,
}

impl SettingsPersistencePolicy {
    const fn allows_disk_writes(self) -> bool {
        matches!(self, Self::Persistent)
    }
}

impl SettingsSaveRequest {
    pub(crate) fn request(&mut self) {
        self.0 = true;
    }

    pub(crate) const fn is_requested(&self) -> bool {
        self.0
    }
}

impl SettingsScreenState {
    #[cfg(test)]
    pub(crate) const fn is_open(&self) -> bool {
        self.open
    }
}

pub(crate) fn reset_settings_screen(screen: &mut SettingsScreenState, settings: &AppSettings) {
    screen.open = false;
    screen.draft = settings.clone();
    screen.dirty = true;
    screen.scroll_y = 0.0;
    screen.restore_focus = None;
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
    ToggleRetinaRendering,
    CycleUiScale,
    SetUnits(DistanceUnit),
    SetStartLive,
    SetStartFixed,
    AdjustStartEpoch(f64),
    CycleStartupRate,
    ToggleInvertHorizontal,
    ToggleInvertVertical,
    ToggleLayer(LayerId),
}

/// Architecture-facing owner of settings schema convergence, persistence, and
/// the modal settings surface.
pub struct SettingsUiPlugin;

impl Plugin for SettingsUiPlugin {
    fn build(&self, app: &mut App) {
        crate::record_architecture_plugin(app, "SettingsUiPlugin");
        app.init_resource::<SettingsScreenState>()
            .init_resource::<SettingsSaveRequest>()
            .init_resource::<SettingsPersistencePolicy>()
            .init_resource::<MsaaCapabilities>()
            .add_systems(
                Update,
                (
                    sync_settings_screen,
                    sync_external_presentation_to_settings,
                    persist_requested_settings,
                    rebuild_settings_screen.in_set(ModalSurfaceSet::Rebuild),
                )
                    .chain()
                    .in_set(SimulationSet::Render),
            )
            .add_systems(Update, save_settings_on_window_close);
    }
}

/// Internal implementation composed only by the architecture-facing
/// `PlatformPlugin`: window/runtime settings and renderer recovery.
pub(crate) struct PlatformRuntimePlugin;

impl Plugin for PlatformRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AppliedRuntimeSettings>()
            .init_resource::<MsaaCapabilities>()
            .init_resource::<RenderRecoveryStatus>()
            .init_resource::<NativeOomSurface>()
            .insert_resource(WinitApplicationThread(thread::current().id()))
            .insert_resource(RenderErrorHandler(product_render_error_policy))
            .add_systems(
                Update,
                (
                    apply_settings_to_runtime.after(sync_external_presentation_to_settings),
                    sync_recovery_completion,
                )
                    .chain()
                    .in_set(SimulationSet::Render),
            );

        #[cfg(debug_assertions)]
        install_debug_device_loss(app);
    }

    fn finish(&self, app: &mut App) {
        let capabilities = app
            .get_sub_app(RenderApp)
            .and_then(|render_app| render_app.world().get_resource::<RenderAdapter>())
            .map(MsaaCapabilities::from_adapter);
        if let Some(capabilities) = capabilities {
            app.insert_resource(capabilities);
        }
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
            save.0 = true;
            if *settings != committed {
                *settings = committed.clone();
            }
            if screen.draft != committed {
                screen.draft = committed;
                screen.dirty = true;
            }
        }
        SimCommand::RestorePresentationDefaults => {
            let defaults = AppSettings::default();
            save.0 = true;
            if settings.layers != defaults.layers {
                settings.layers = defaults.layers;
            }
            if screen.draft.layers != defaults.layers {
                screen.draft.layers = defaults.layers;
                screen.dirty = true;
            }
        }
        _ => {}
    }
}

/// Resolves loaded settings before any runtime state is derived from them.
///
/// An explicit startup reset crosses the same semantic settings reducer as the
/// in-product RESTORE DEFAULTS action. The synchronous write is intentional:
/// the launch flag is a recovery boundary and must be durable before normal
/// startup continues.
pub(crate) fn bootstrap_app_settings(
    world: &mut World,
    reset_requested: bool,
    persistence: SettingsPersistencePolicy,
) -> AppSettings {
    if reset_requested {
        let mut settings = world.resource::<AppSettings>().clone();
        let mut screen = SettingsScreenState::default();
        let mut save = SettingsSaveRequest::default();
        consume_settings_command(
            &SimCommand::ApplySettings(Box::default()),
            &mut screen,
            &mut settings,
            &mut save,
        );
        *world.resource_mut::<AppSettings>() = settings;
        SaveSettingsSync::Always.apply(world);
    }
    // Install the runtime policy only after an explicit reset has reached the
    // production settings file. Capture-only overrides after this point must
    // remain transient.
    world.insert_resource(persistence);
    let loaded = world.resource::<AppSettings>().clone();
    let normalized = loaded.clone().normalized();
    if loaded != normalized {
        *world.resource_mut::<AppSettings>() = normalized.clone();
        if persistence.allows_disk_writes() {
            SaveSettingsSync::Always.apply(world);
        }
    }
    normalized
}

pub(crate) fn sync_settings_screen_state(
    presentation: &PresentationState,
    settings: &AppSettings,
    screen: &mut SettingsScreenState,
) -> bool {
    let open = presentation.is_settings_open();
    if screen.open == open {
        return false;
    }
    screen.open = open;
    screen.draft = settings.clone();
    screen.dirty = true;
    screen.scroll_y = 0.0;
    screen.restore_focus = None;
    true
}

fn sync_settings_screen(
    presentation: Res<PresentationState>,
    settings: Res<AppSettings>,
    mut screen: ResMut<SettingsScreenState>,
    mut focus: ResMut<InputFocus>,
) {
    let changed =
        sync_settings_screen_state(&presentation, &settings, screen.bypass_change_detection());
    if changed {
        screen.set_changed();
        focus.clear();
    }
}

pub(crate) fn converge_presentation_settings(
    layers: &LayerState,
    presentation: &PresentationState,
    settings: &mut AppSettings,
) -> bool {
    let persisted_layers = PersistedLayerState::from_snapshot(layers.persistence_snapshot());
    let display_mode = if presentation.is_fullscreen() {
        DisplayModeSetting::BorderlessFullscreen
    } else {
        DisplayModeSetting::Windowed
    };
    if settings.layers == persisted_layers && settings.display_mode == display_mode {
        return false;
    }
    settings.layers = persisted_layers;
    settings.display_mode = display_mode;
    true
}

#[allow(clippy::too_many_arguments)]
fn apply_settings_to_runtime(
    settings: Res<AppSettings>,
    msaa_capabilities: Res<MsaaCapabilities>,
    mut applied: ResMut<AppliedRuntimeSettings>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut ui_scale: ResMut<UiScale>,
    mut winit: ResMut<WinitSettings>,
    mut cameras: Query<(Entity, &mut Msaa, Has<Bloom>), With<Camera3d>>,
    mut commands: Commands,
) {
    if !settings.is_changed() {
        return;
    }
    let normalized = settings.clone().normalized();
    let key = RuntimeSettingsKey::from_settings(&normalized);
    if applied.0 == Some(key) {
        return;
    }
    for mut window in &mut windows {
        let desired_mode = if normalized.display_mode.is_fullscreen() {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };
        if window.mode != desired_mode {
            window.mode = desired_mode;
        }
        let desired_width = normalized.resolution.width as f32;
        let desired_height = normalized.resolution.height as f32;
        if window.resolution.width() != desired_width
            || window.resolution.height() != desired_height
        {
            window.resolution.set(desired_width, desired_height);
        }
        let desired_scale_factor = normalized
            .quality
            .scale_factor_override(normalized.retina_rendering);
        if window.resolution.scale_factor_override() != desired_scale_factor {
            window
                .resolution
                .set_scale_factor_override(desired_scale_factor);
        }
        let desired_present_mode = if normalized.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        };
        if window.present_mode != desired_present_mode {
            window.present_mode = desired_present_mode;
        }
    }
    if ui_scale.0 != normalized.ui_scale {
        ui_scale.0 = normalized.ui_scale;
    }
    let update_mode = normalized
        .frame_cap
        .hz()
        .map_or(UpdateMode::Continuous, |hz| {
            UpdateMode::reactive(Duration::from_secs_f64(1.0 / f64::from(hz)))
        });
    if winit.focused_mode != update_mode {
        winit.focused_mode = update_mode;
    }
    if winit.unfocused_mode != update_mode {
        winit.unfocused_mode = update_mode;
    }
    let desired_msaa = msaa_capabilities.resolve(normalized.quality.requested_msaa());
    let desired_bloom = normalized.quality.bloom_enabled();
    for (entity, mut msaa, has_bloom) in &mut cameras {
        if *msaa != desired_msaa {
            *msaa = desired_msaa;
        }
        match (desired_bloom, has_bloom) {
            (true, false) => {
                commands.entity(entity).insert(Bloom::NATURAL);
            }
            (false, true) => {
                commands.entity(entity).remove::<Bloom>();
            }
            _ => {}
        }
    }
    applied.0 = Some(key);
}

fn sync_external_presentation_to_settings(
    layers: Res<LayerState>,
    presentation: Res<PresentationState>,
    persistence: Res<SettingsPersistencePolicy>,
    startup: Option<Res<crate::control::SessionStartupSnapshot>>,
    mut settings: ResMut<AppSettings>,
    mut commands: Commands,
) {
    // A settings-screen commit is authoritative for this frame. The shared
    // command reducer already converges layers/fullscreen into AppSettings, so
    // a changed settings resource must not be overwritten by stale externals.
    if settings.is_changed() {
        return;
    }
    if startup
        .as_deref()
        .is_some_and(crate::control::SessionStartupSnapshot::nonpersistent_presentation_override)
    {
        return;
    }
    if converge_presentation_settings(&layers, &presentation, settings.bypass_change_detection()) {
        settings.set_changed();
        if persistence.allows_disk_writes() {
            commands.queue(SaveSettingsDeferred(SETTINGS_SAVE_DELAY));
        }
    }
}

fn persist_requested_settings(
    persistence: Res<SettingsPersistencePolicy>,
    mut request: ResMut<SettingsSaveRequest>,
    mut commands: Commands,
) {
    if !request.0 {
        return;
    }
    request.0 = false;
    if persistence.allows_disk_writes() {
        commands.queue(SaveSettingsDeferred(SETTINGS_SAVE_DELAY));
    }
}

fn save_settings_on_window_close(
    mut close: MessageReader<WindowCloseRequested>,
    persistence: Res<SettingsPersistencePolicy>,
    mut commands: Commands,
) {
    if close.read().next().is_some() {
        if persistence.allows_disk_writes() {
            commands.queue(SaveSettingsSync::IfChanged);
        }
        commands.write_message(AppExit::Success);
    }
}

#[allow(clippy::too_many_arguments)]
fn rebuild_settings_screen(
    mut commands: Commands,
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
    mut screen: ResMut<SettingsScreenState>,
    roots: Query<Entity, With<SettingsScreenRoot>>,
    actions: Query<&SettingAction>,
    scroll_areas: Query<&ScrollPosition, With<SettingsScrollArea>>,
    focus: Res<InputFocus>,
    msaa_capabilities: Res<MsaaCapabilities>,
) {
    if !screen.dirty {
        return;
    }
    if screen.open {
        if screen.restore_focus.is_none() {
            screen.restore_focus = focus
                .get()
                .and_then(|entity| actions.get(entity).ok().copied());
        }
        if let Ok(position) = scroll_areas.single() {
            screen.scroll_y = position.y;
        }
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
    let mut next_tab_index = SETTINGS_FIRST_TAB_INDEX;
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
        &mut next_tab_index,
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
        &mut next_tab_index,
    );
    spawn_checkbox(
        &mut commands,
        content,
        *theme,
        "Vertical sync",
        draft.vsync,
        SettingAction::ToggleVsync,
        &mut next_tab_index,
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
        &mut next_tab_index,
    );
    spawn_section(&mut commands, content, *theme, "QUALITY PRESET");
    spawn_choices(
        &mut commands,
        content,
        *theme,
        QualityPreset::ALL.map(|value| {
            (
                value.display_label(*msaa_capabilities),
                draft.quality == value,
                SettingAction::SetQuality(value),
            )
        }),
        &mut next_tab_index,
    );
    spawn_checkbox(
        &mut commands,
        content,
        *theme,
        "Retina rendering",
        draft.retina_rendering,
        SettingAction::ToggleRetinaRendering,
        &mut next_tab_index,
    );
    commands.spawn((
        RetinaRenderingDescription,
        Text::new("Takes effect in windowed mode; fullscreen renders at display resolution."),
        TextFont {
            font: asset_server.load(INTER_FONT_ASSET).into(),
            font_size: theme.type_scale.caption_px.into(),
            ..default()
        },
        TextColor(theme.colors.text_muted.color()),
        Pickable::IGNORE,
        ChildOf(content),
    ));
    let ui_scale_tab_index = next_settings_tab_index(&mut next_tab_index);
    commands
        .spawn_scene(slider(
            *theme,
            WidgetSpec::new(
                format!("UI SCALE  {:.0}%", draft.ui_scale * 100.0),
                "Cycle user interface scale",
                WidgetVisualState::Active,
            ),
        ))
        .insert((
            SettingAction::CycleUiScale,
            ui_scale_tab_index,
            ChildOf(content),
        ))
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
        &mut next_tab_index,
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
                fixed_epoch_label(fixed_epoch),
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
        &mut next_tab_index,
    );
    let startup_rate_tab_index = next_settings_tab_index(&mut next_tab_index);
    commands
        .spawn_scene(slider(
            *theme,
            WidgetSpec::new(
                format!("STARTUP RATE  {}", draft.startup_rate.rate().label()),
                "Cycle startup simulation rate",
                WidgetVisualState::Active,
            ),
        ))
        .insert((
            SettingAction::CycleStartupRate,
            startup_rate_tab_index,
            ChildOf(content),
        ))
        .observe(activate_setting_action);
    spawn_checkbox(
        &mut commands,
        content,
        *theme,
        "Invert horizontal orbit axis",
        draft.invert_horizontal,
        SettingAction::ToggleInvertHorizontal,
        &mut next_tab_index,
    );
    spawn_checkbox(
        &mut commands,
        content,
        *theme,
        "Invert vertical orbit axis",
        draft.invert_vertical,
        SettingAction::ToggleInvertVertical,
        &mut next_tab_index,
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
            &mut next_tab_index,
        );
    }

    let footer = commands
        .spawn((
            Node {
                width: percent(100),
                height: auto(),
                min_height: px(42),
                flex_shrink: 0.0,
                justify_content: JustifyContent::FlexEnd,
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
        &mut next_tab_index,
    );
    debug_assert_eq!(next_tab_index, SETTINGS_FIRST_TAB_INDEX + 41);
    let action = screen.restore_focus.take().unwrap_or(SettingAction::Close);
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
    next_tab_index: &mut i32,
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
            .insert((
                action,
                next_settings_tab_index(next_tab_index),
                ChildOf(row),
            ))
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
    next_tab_index: &mut i32,
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
        .insert((
            action,
            next_settings_tab_index(next_tab_index),
            ChildOf(parent),
        ))
        .observe(activate_setting_action);
}

fn next_settings_tab_index(next_tab_index: &mut i32) -> TabIndex {
    let tab_index = *next_tab_index;
    *next_tab_index += 1;
    TabIndex(tab_index)
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
    let draft_before = screen.draft.clone();
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
        SettingAction::ToggleRetinaRendering => {
            screen.draft.retina_rendering = !screen.draft.retina_rendering;
        }
        SettingAction::CycleUiScale => {
            screen.draft.ui_scale = next_ui_scale(screen.draft.ui_scale);
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
                jd_tdb: normalized_fixed_epoch_jd_tdb(
                    screen.draft.start_mode.fixed_epoch() + delta_days,
                ),
            };
        }
        SettingAction::CycleStartupRate => {
            screen.draft.startup_rate = screen.draft.startup_rate.next();
        }
        SettingAction::ToggleInvertHorizontal => {
            screen.draft.invert_horizontal = !screen.draft.invert_horizontal;
        }
        SettingAction::ToggleInvertVertical => {
            screen.draft.invert_vertical = !screen.draft.invert_vertical;
        }
        SettingAction::ToggleLayer(layer) => screen.draft.layers.toggle(layer),
    }
    if screen.draft != draft_before {
        screen.dirty = true;
    }
}

fn next_ui_scale(current: f32) -> f32 {
    match current {
        value if value < 1.0 => 1.0,
        value if value < 1.25 => 1.25,
        value if value < 1.5 => 1.5,
        value if value < 2.0 => 2.0,
        _ => 0.75,
    }
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
    let (directive, invoke_native_oom_surface) = {
        let mut recovery = main_world.resource_mut::<RenderRecoveryStatus>();
        let directive = recovery.handle_failure(failure);
        let invoke =
            failure == RenderFailureKind::OutOfMemory && recovery.take_native_oom_surface_request();
        (directive, invoke)
    };
    if invoke_native_oom_surface {
        invoke_native_oom_surface_on_winit_thread(main_world);
    }
    match directive {
        RecoveryDirective::Recover => RenderErrorPolicy::Recover(RenderCreation::default()),
        RecoveryDirective::StopRendering => RenderErrorPolicy::StopRendering,
    }
}

fn invoke_native_oom_surface_on_winit_thread(main_world: &World) {
    const TITLE: &str = "Solar Sim — Graphics Memory Exhausted";
    const MESSAGE: &str = "Rendering has stopped safely because Solar Sim ran out of graphics memory. Close Solar Sim, free graphics memory, and relaunch.";

    let expected_thread = main_world.resource::<WinitApplicationThread>().0;
    let current_thread = thread::current().id();
    if current_thread != expected_thread {
        error!(
            "refusing to invoke the native OOM surface off the winit application thread: expected {expected_thread:?}, got {current_thread:?}"
        );
        return;
    }
    let surface = main_world.resource::<NativeOomSurface>().clone();
    if let Err(error) = surface.show(TITLE, MESSAGE) {
        error!("native OOM surface failed: {error}");
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
    use crate::ui_kit::test_layout;
    use crate::WidgetRoot;
    use bevy::{
        a11y::AccessibilityNode,
        app::TaskPoolPlugin,
        asset::{AssetApp, AssetPlugin},
        scene::ScenePlugin,
        settings::SettingsPlugin as BevySettingsPlugin,
        text::Font,
        time::TimeUpdateStrategy,
    };

    #[derive(Resource, Debug, Default)]
    struct AppSettingsChangeCount(usize);

    fn count_app_settings_changes(
        settings: Res<AppSettings>,
        mut count: ResMut<AppSettingsChangeCount>,
    ) {
        count.0 = usize::from(settings.is_changed());
    }
    use sim_core::time::{DAY_S, T_MAX_S};
    use std::{
        path::PathBuf,
        process::Command as ProcessCommand,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

    const SETTINGS_TEST_PHASE_ENV: &str = "SOLAR_SIM_SETTINGS_TEST_PHASE";
    const SETTINGS_TEST_IDENTIFIER_ENV: &str = "SOLAR_SIM_SETTINGS_TEST_IDENTIFIER";

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
            retina_rendering: false,
            ui_scale: 1.5,
            units: DistanceUnit::AstronomicalUnits,
            start_mode: StartModeSetting::FixedEpoch {
                jd_tdb: 2_451_545.25,
            },
            startup_rate: StartupRateSetting::from_raw(-7),
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

    fn persistence_test_app(identifier: &str) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .register_type::<AppSettings>()
            .add_plugins(BevySettingsPlugin::new(identifier));
        app
    }

    fn child_settings_identifier() -> String {
        std::env::var(SETTINGS_TEST_IDENTIFIER_ENV)
            .expect("settings child process must receive an isolated identifier")
    }

    fn write_nondefault_settings(identifier: &str) {
        let mut app = persistence_test_app(identifier);
        *app.world_mut().resource_mut::<AppSettings>() = nondefault_settings();
        SaveSettingsSync::Always.apply(app.world_mut());
    }

    fn settings_file_path(identifier: &str) -> PathBuf {
        bevy::platform::dirs::preferences_dir()
            .unwrap()
            .join(identifier)
            .join("settings.toml")
    }

    fn remove_startup_rate_from_persisted_settings(identifier: &str) {
        let path = settings_file_path(identifier);
        let text = std::fs::read_to_string(&path).unwrap();
        let mut removed = 0;
        let legacy = text
            .lines()
            .filter(|line| {
                let keep = !line.trim_start().starts_with("startup_rate =");
                removed += usize::from(!keep);
                keep
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(removed, 1, "persisted settings must contain startup_rate");
        std::fs::write(path, format!("{legacy}\n")).unwrap();
    }

    fn settings_with_fixed_epoch(jd_tdb: f64) -> AppSettings {
        AppSettings {
            start_mode: StartModeSetting::FixedEpoch { jd_tdb },
            ..default()
        }
    }

    fn write_fixed_epoch_settings(identifier: &str, jd_tdb: f64) {
        let mut app = persistence_test_app(identifier);
        *app.world_mut().resource_mut::<AppSettings>() = settings_with_fixed_epoch(jd_tdb);
        SaveSettingsSync::Always.apply(app.world_mut());
    }

    fn fixed_epoch_value(settings: &AppSettings) -> f64 {
        match settings.start_mode {
            StartModeSetting::FixedEpoch { jd_tdb } => jd_tdb,
            StartModeSetting::Live => panic!("expected a fixed epoch"),
        }
    }

    fn assert_fixed_epoch_convergence(settings: &AppSettings, expected_jd_tdb: f64) {
        assert_eq!(
            fixed_epoch_value(settings).to_bits(),
            expected_jd_tdb.to_bits()
        );
        let mut presentation = PresentationState::default();
        presentation.open_settings();
        let mut screen = SettingsScreenState::default();
        assert!(sync_settings_screen_state(
            &presentation,
            settings,
            &mut screen
        ));
        assert_eq!(
            fixed_epoch_value(&screen.draft).to_bits(),
            expected_jd_tdb.to_bits()
        );
        let displayed_jd_tdb = fixed_epoch_label(expected_jd_tdb)
            .strip_prefix("FIXED JD ")
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert_eq!(displayed_jd_tdb.to_bits(), expected_jd_tdb.to_bits());
        let clock_jd_tdb = settings.initial_clock(0.0).jd_tdb();
        assert!(
            (clock_jd_tdb - expected_jd_tdb).abs() * DAY_S <= 0.001,
            "clock {clock_jd_tdb} differs from settings edge {expected_jd_tdb} by more than 1 ms"
        );
    }

    fn restore_defaults_through_in_product_action(identifier: &str) {
        let mut app = persistence_test_app(identifier);
        assert_eq!(
            app.world().resource::<AppSettings>(),
            &nondefault_settings(),
            "the in-product reset phase must begin from exact persisted input"
        );
        app.init_resource::<InputFocus>()
            .init_resource::<SimCommandQueue>()
            .init_resource::<SettingsSaveRequest>()
            .insert_resource(SettingsScreenState {
                open: true,
                draft: nondefault_settings(),
                dirty: false,
                scroll_y: 0.0,
                restore_focus: None,
            });
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
            .expect("settings screen state");
        let mut settings = app
            .world_mut()
            .remove_resource::<AppSettings>()
            .expect("loaded app settings");
        let mut save = app
            .world_mut()
            .remove_resource::<SettingsSaveRequest>()
            .expect("settings save request");
        let mut layers = settings.initial_layer_state();
        let mut presentation = PresentationState::with_fullscreen(
            settings.display_mode == DisplayModeSetting::BorderlessFullscreen,
        );
        presentation.open_settings();
        let mut view_options = crate::ViewOptionsState::default();
        let mut left_panel = crate::left_panel::LeftPanelUiState::default();
        let mut navigation = crate::NavigationStack::default();
        let mut browse = crate::search::BrowseUiState::default();
        for command in &commands {
            crate::control::consume_application_command(
                command,
                None,
                &mut layers,
                &mut presentation,
                &mut view_options,
                &mut left_panel,
                &mut navigation,
                &mut browse,
                &mut settings,
                &mut screen,
                &mut save,
            );
        }
        assert_eq!(settings, AppSettings::default());
        assert_eq!(layers, LayerState::default());
        assert!(!presentation.is_settings_open());
        assert!(save.is_requested());
        app.insert_resource(screen)
            .insert_resource(settings)
            .insert_resource(save);
        // This is the synchronous close-path half of the product's
        // deferred-save plus SaveSettingsSync::IfChanged persistence policy.
        SaveSettingsSync::IfChanged.apply(app.world_mut());
    }

    fn reset_interface_preserves_settings_bytes(identifier: &str) {
        let mut app = persistence_test_app(identifier);
        let initial_settings = app.world().resource::<AppSettings>().clone();
        assert_eq!(initial_settings, nondefault_settings());

        let clock = initial_settings.initial_clock(0.0);
        let camera = crate::CameraController::unavailable();
        let mut layers = initial_settings.initial_layer_state();
        let startup_layers = layers;
        let mut presentation = PresentationState::with_fullscreen(
            initial_settings.display_mode == DisplayModeSetting::BorderlessFullscreen,
        );
        let mut view_options = crate::ViewOptionsState::default();
        let mut left_panel = crate::left_panel::LeftPanelUiState::default();
        let mut navigation = crate::NavigationStack::default();
        let mut browse = crate::search::BrowseUiState::default();
        let mut settings = initial_settings;
        let mut settings_screen = SettingsScreenState::default();
        let mut settings_save = SettingsSaveRequest::default();
        let mut startup = crate::control::SessionStartupSnapshot::default();
        startup.capture_if_missing(
            &clock,
            &camera,
            layers,
            presentation,
            &view_options,
            &left_panel,
            &navigation,
            &browse,
        );

        crate::control::consume_application_command_with_startup(
            &SimCommand::SetLayerVisibility {
                layer: LayerId::Labels,
                visible: true,
            },
            None,
            &mut layers,
            &mut presentation,
            &mut view_options,
            &mut left_panel,
            &mut navigation,
            &mut browse,
            &mut settings,
            &mut settings_screen,
            &mut settings_save,
            &mut startup,
        );
        assert!(layers.is_visible(LayerId::Labels));
        assert!(settings.layers.labels);
        assert!(settings_save.is_requested());

        // Establish the exact persisted/runtime state immediately before
        // Reset Interface, as if the preceding explicit layer action had
        // already crossed WP14's persistence boundary.
        *app.world_mut().resource_mut::<AppSettings>() = settings.clone();
        SaveSettingsSync::Always.apply(app.world_mut());
        settings_save = SettingsSaveRequest::default();
        let path = settings_file_path(identifier);
        let bytes_before_reset = std::fs::read(&path).unwrap();

        crate::control::consume_application_command_with_startup(
            &SimCommand::ResetInterface,
            None,
            &mut layers,
            &mut presentation,
            &mut view_options,
            &mut left_panel,
            &mut navigation,
            &mut browse,
            &mut settings,
            &mut settings_screen,
            &mut settings_save,
            &mut startup,
        );
        assert_eq!(layers, startup_layers);
        assert!(!layers.is_visible(LayerId::Labels));
        assert!(settings.layers.labels);
        assert!(!settings_save.is_requested());
        assert!(startup.nonpersistent_presentation_override());

        // Exercise both ordinary deferred convergence and the synchronous
        // close path. The restored launch presentation intentionally differs
        // from AppSettings here; the reset override must keep that difference
        // session-local and leave the file byte-for-byte untouched.
        app.insert_resource(layers)
            .insert_resource(presentation)
            .insert_resource(settings)
            .insert_resource(settings_save)
            .insert_resource(startup)
            .insert_resource(SettingsPersistencePolicy::Persistent)
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                SETTINGS_SAVE_DELAY + Duration::from_millis(1),
            ))
            .add_message::<WindowCloseRequested>()
            .add_message::<AppExit>()
            .add_systems(
                Update,
                (
                    sync_external_presentation_to_settings,
                    persist_requested_settings,
                    save_settings_on_window_close,
                )
                    .chain(),
            );
        app.update();
        app.update();
        app.world_mut().write_message(WindowCloseRequested {
            window: Entity::PLACEHOLDER,
        });
        app.update();

        let expected = AppSettings {
            layers: PersistedLayerState {
                labels: true,
                ..nondefault_settings().layers
            },
            ..nondefault_settings()
        };
        assert_eq!(app.world().resource::<AppSettings>(), &expected);
        assert_eq!(std::fs::read(path).unwrap(), bytes_before_reset);
    }

    fn exercise_transient_capture_runtime(identifier: &str, reset_requested: bool) {
        let mut app = persistence_test_app(identifier);
        assert_eq!(
            app.world().resource::<AppSettings>(),
            &nondefault_settings()
        );
        let bootstrapped = bootstrap_app_settings(
            app.world_mut(),
            reset_requested,
            SettingsPersistencePolicy::TransientRuntime,
        );
        assert_eq!(
            bootstrapped,
            if reset_requested {
                AppSettings::default()
            } else {
                nondefault_settings()
            }
        );
        assert_eq!(
            *app.world().resource::<SettingsPersistencePolicy>(),
            SettingsPersistencePolicy::TransientRuntime
        );

        // Match build_app's golden-only runtime override, then force a second
        // layer convergence so every product-originated write path is armed.
        let capture_settings = AppSettings {
            resolution: ResolutionSetting {
                width: crate::GOLDEN_WIDTH,
                height: crate::GOLDEN_HEIGHT,
            },
            vsync: false,
            frame_cap: FrameCap::Unlimited,
            ..default()
        }
        .normalized();
        *app.world_mut().resource_mut::<AppSettings>() = capture_settings;
        let mut capture_layers = LayerState::default();
        for layer in [LayerId::Orbits, LayerId::Labels, LayerId::Icons] {
            capture_layers.set_visible(layer, false);
        }
        app.insert_resource(capture_layers)
            .insert_resource(PresentationState::default())
            .init_resource::<SettingsSaveRequest>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                SETTINGS_SAVE_DELAY + Duration::from_millis(1),
            ))
            .add_message::<WindowCloseRequested>()
            .add_message::<AppExit>()
            .add_systems(
                Update,
                (
                    sync_external_presentation_to_settings,
                    persist_requested_settings,
                    save_settings_on_window_close,
                )
                    .chain(),
            );
        app.world_mut()
            .resource_mut::<SettingsSaveRequest>()
            .request();

        // First update consumes the explicit save request. The second changes
        // persisted layer fields through convergence and attempts close-save.
        app.update();
        app.world_mut().write_message(WindowCloseRequested {
            window: Entity::PLACEHOLDER,
        });
        app.update();

        assert!(!app.world().resource::<SettingsSaveRequest>().is_requested());
        let runtime = app.world().resource::<AppSettings>();
        assert_eq!(runtime.resolution.width, crate::GOLDEN_WIDTH);
        assert!(!runtime.layers.orbits);
        assert!(!runtime.layers.labels);
        assert!(!runtime.layers.icons);
    }

    fn settings_screen_test_app() -> App {
        let mut presentation = PresentationState::default();
        presentation.open_settings();
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ))
        .init_asset::<Font>()
        .insert_resource(UiTheme::default())
        .insert_resource(AppSettings::default())
        .insert_resource(MsaaCapabilities::from_supported_counts(
            &[1, 2, 4, 8],
            &[1, 2, 4],
        ))
        .init_resource::<LayerState>()
        .insert_resource(presentation)
        .init_resource::<SimCommandQueue>()
        .init_resource::<SettingsSaveRequest>()
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
            (sync_settings_screen, rebuild_settings_screen).chain(),
        );
        app
    }

    fn settings_action_entity(world: &mut World, expected: SettingAction) -> Entity {
        let mut actions = world.query::<(Entity, &SettingAction)>();
        actions
            .iter(world)
            .find_map(|(entity, action)| (*action == expected).then_some(entity))
            .expect("settings action must exist")
    }

    fn settings_scroll_entity(world: &mut World) -> Entity {
        let mut areas = world.query_filtered::<Entity, With<SettingsScrollArea>>();
        areas.single(world).expect("one settings scroll area")
    }

    fn expected_settings_tab_order() -> Vec<SettingAction> {
        let mut actions = Vec::with_capacity(41);
        actions.extend(DisplayModeSetting::ALL.map(SettingAction::SetDisplayMode));
        actions.extend(ResolutionSetting::PRESETS.map(SettingAction::SetResolution));
        actions.push(SettingAction::ToggleVsync);
        actions.extend(FrameCap::ALL.map(SettingAction::SetFrameCap));
        actions.extend(QualityPreset::ALL.map(SettingAction::SetQuality));
        actions.push(SettingAction::ToggleRetinaRendering);
        actions.push(SettingAction::CycleUiScale);
        actions.extend(DistanceUnit::ALL.map(SettingAction::SetUnits));
        actions.extend([
            SettingAction::SetStartLive,
            SettingAction::SetStartFixed,
            SettingAction::AdjustStartEpoch(-365.25),
            SettingAction::AdjustStartEpoch(365.25),
            SettingAction::CycleStartupRate,
            SettingAction::ToggleInvertHorizontal,
            SettingAction::ToggleInvertVertical,
        ]);
        actions.extend(LayerId::ALL.map(SettingAction::ToggleLayer));
        actions.extend([
            SettingAction::Close,
            SettingAction::Revert,
            SettingAction::RestoreDefaults,
            SettingAction::Apply,
        ]);
        actions
    }

    #[test]
    fn full_settings_struct_round_trips_through_serde() {
        let settings = nondefault_settings();
        let encoded = ron::to_string(&settings).expect("settings serialize");
        let decoded: AppSettings = ron::from_str(&encoded).expect("settings deserialize");
        assert_eq!(decoded, settings);
    }

    #[test]
    fn settings_without_gpu_toggle_or_startup_rate_migrate_to_defaults() {
        #[derive(serde::Serialize)]
        struct LegacyAppSettings {
            display_mode: DisplayModeSetting,
            resolution: ResolutionSetting,
            vsync: bool,
            frame_cap: FrameCap,
            quality: QualityPreset,
            ui_scale: f32,
            units: DistanceUnit,
            start_mode: StartModeSetting,
            invert_horizontal: bool,
            invert_vertical: bool,
            layers: PersistedLayerState,
        }

        let current = nondefault_settings();
        let legacy = LegacyAppSettings {
            display_mode: current.display_mode,
            resolution: current.resolution,
            vsync: current.vsync,
            frame_cap: current.frame_cap,
            quality: current.quality,
            ui_scale: current.ui_scale,
            units: current.units,
            start_mode: current.start_mode,
            invert_horizontal: current.invert_horizontal,
            invert_vertical: current.invert_vertical,
            layers: current.layers,
        };
        let encoded = ron::to_string(&legacy).unwrap();
        let migrated: AppSettings = ron::from_str(&encoded).unwrap();

        assert_eq!(migrated.startup_rate, StartupRateSetting::default());
        assert_eq!(migrated.startup_rate.rate().label(), "1 DAY/S");
        assert!(migrated.retina_rendering);
    }

    #[test]
    fn startup_rate_normalization_preserves_every_detent_and_defaults_invalid_values() {
        for rate in RateIndex::detents() {
            let settings = AppSettings {
                startup_rate: StartupRateSetting::from_raw(rate.get()),
                ..default()
            }
            .normalized();
            assert_eq!(settings.startup_rate.rate(), rate);
        }
        for invalid in [i8::MIN, -13, 0, 13, i8::MAX] {
            let settings = AppSettings {
                startup_rate: StartupRateSetting::from_raw(invalid),
                ..default()
            }
            .normalized();
            assert_eq!(settings.startup_rate, StartupRateSetting::default());
        }
    }

    #[test]
    fn explicit_reset_paths_survive_full_process_relaunch() {
        match std::env::var(SETTINGS_TEST_PHASE_ENV).as_deref() {
            Ok("write") | Ok("rewrite") => {
                write_nondefault_settings(&child_settings_identifier());
                return;
            }
            Ok("read-before") | Ok("read-before-product-reset") => {
                let app = persistence_test_app(&child_settings_identifier());
                assert_eq!(
                    app.world().resource::<AppSettings>(),
                    &nondefault_settings()
                );
                return;
            }
            Ok("write-legacy-without-startup-rate") => {
                write_nondefault_settings(&child_settings_identifier());
                remove_startup_rate_from_persisted_settings(&child_settings_identifier());
                return;
            }
            Ok("read-legacy-without-startup-rate") => {
                let app = persistence_test_app(&child_settings_identifier());
                let mut expected = nondefault_settings();
                expected.startup_rate = StartupRateSetting::default();
                assert_eq!(app.world().resource::<AppSettings>(), &expected);
                return;
            }
            Ok("cli-reset") => {
                let mut app = persistence_test_app(&child_settings_identifier());
                assert_eq!(
                    app.world().resource::<AppSettings>(),
                    &nondefault_settings()
                );
                let args = ["solar-sim", "--reset-settings"].map(str::to_owned);
                let options = crate::RunOptions::from_args(&args).expect("parse reset launch");
                let initial = bootstrap_app_settings(
                    app.world_mut(),
                    options.reset_settings,
                    SettingsPersistencePolicy::Persistent,
                );
                assert_eq!(initial, AppSettings::default());
                assert_eq!(app.world().resource::<AppSettings>(), &initial);
                return;
            }
            Ok("product-reset") => {
                restore_defaults_through_in_product_action(&child_settings_identifier());
                return;
            }
            Ok("read-after-cli") | Ok("read-after-product") => {
                let app = persistence_test_app(&child_settings_identifier());
                assert_eq!(
                    app.world().resource::<AppSettings>(),
                    &AppSettings::default()
                );
                return;
            }
            Ok("reset-interface-preserves-bytes") => {
                reset_interface_preserves_settings_bytes(&child_settings_identifier());
                return;
            }
            Ok("read-after-interface-reset") => {
                let app = persistence_test_app(&child_settings_identifier());
                let expected = AppSettings {
                    layers: PersistedLayerState {
                        labels: true,
                        ..nondefault_settings().layers
                    },
                    ..nondefault_settings()
                };
                assert_eq!(app.world().resource::<AppSettings>(), &expected);
                return;
            }
            Ok("capture-transient") => {
                exercise_transient_capture_runtime(&child_settings_identifier(), false);
                return;
            }
            Ok("read-after-capture") => {
                let app = persistence_test_app(&child_settings_identifier());
                assert_eq!(
                    app.world().resource::<AppSettings>(),
                    &nondefault_settings()
                );
                return;
            }
            Ok("reset-capture-transient") => {
                exercise_transient_capture_runtime(&child_settings_identifier(), true);
                return;
            }
            Ok("read-after-reset-capture") => {
                let app = persistence_test_app(&child_settings_identifier());
                assert_eq!(
                    app.world().resource::<AppSettings>(),
                    &AppSettings::default()
                );
                return;
            }
            Ok("write-below-range") => {
                let (minimum, _) = fixed_epoch_bounds_jd_tdb();
                write_fixed_epoch_settings(&child_settings_identifier(), minimum - 10_000.0);
                return;
            }
            Ok("normalize-below-range") => {
                let (minimum, _) = fixed_epoch_bounds_jd_tdb();
                let mut app = persistence_test_app(&child_settings_identifier());
                assert!(fixed_epoch_value(app.world().resource::<AppSettings>()) < minimum);
                let initial = bootstrap_app_settings(
                    app.world_mut(),
                    false,
                    SettingsPersistencePolicy::Persistent,
                );
                assert_fixed_epoch_convergence(&initial, minimum);
                assert_fixed_epoch_convergence(app.world().resource::<AppSettings>(), minimum);
                return;
            }
            Ok("read-after-below-range") => {
                let (minimum, _) = fixed_epoch_bounds_jd_tdb();
                let app = persistence_test_app(&child_settings_identifier());
                assert_fixed_epoch_convergence(app.world().resource::<AppSettings>(), minimum);
                return;
            }
            Ok("write-above-range") => {
                let (_, maximum) = fixed_epoch_bounds_jd_tdb();
                write_fixed_epoch_settings(&child_settings_identifier(), maximum + 10_000.0);
                return;
            }
            Ok("normalize-above-range") => {
                let (_, maximum) = fixed_epoch_bounds_jd_tdb();
                let mut app = persistence_test_app(&child_settings_identifier());
                assert!(fixed_epoch_value(app.world().resource::<AppSettings>()) > maximum);
                let initial = bootstrap_app_settings(
                    app.world_mut(),
                    false,
                    SettingsPersistencePolicy::Persistent,
                );
                assert_fixed_epoch_convergence(&initial, maximum);
                assert_fixed_epoch_convergence(app.world().resource::<AppSettings>(), maximum);
                return;
            }
            Ok("read-after-above-range") => {
                let (_, maximum) = fixed_epoch_bounds_jd_tdb();
                let app = persistence_test_app(&child_settings_identifier());
                assert_fixed_epoch_convergence(app.world().resource::<AppSettings>(), maximum);
                return;
            }
            _ => {}
        }

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after Unix epoch")
            .as_nanos();
        let temporary_home = std::env::temp_dir().join(format!(
            "solar-sim-settings-relaunch-test-{}-{nonce}",
            std::process::id(),
        ));
        std::fs::create_dir(&temporary_home).expect("create isolated settings HOME");
        // Bevy's Windows settings store resolves LocalAppData through the Known
        // Folders API. An absolute test-only application name makes PathBuf::join
        // retain this nonce-scoped directory on every desktop platform. Do not
        // reintroduce a relative identifier or override HOME/APPDATA/USERPROFILE
        // on the children: nonexistent profile overrides can make the Windows
        // Known Folders lookup fail before it ever joins this absolute path.
        let settings_directory = temporary_home.join("bevy-settings");
        let identifier = settings_directory
            .to_str()
            .expect("isolated settings path must be valid Unicode")
            .to_owned();
        let preferences = bevy::platform::dirs::preferences_dir()
            .expect("desktop settings persistence requires a preferences directory");
        assert_eq!(preferences.join(&identifier), settings_directory);
        let executable = std::env::current_exe().expect("current test executable");
        for phase in [
            "write",
            "read-before",
            "write-legacy-without-startup-rate",
            "read-legacy-without-startup-rate",
            "rewrite",
            "cli-reset",
            "read-after-cli",
            "rewrite",
            "read-before-product-reset",
            "product-reset",
            "read-after-product",
            "rewrite",
            "reset-interface-preserves-bytes",
            "read-after-interface-reset",
            "rewrite",
            "read-before",
            "capture-transient",
            "read-after-capture",
            "reset-capture-transient",
            "read-after-reset-capture",
            "write-below-range",
            "normalize-below-range",
            "read-after-below-range",
            "write-above-range",
            "normalize-above-range",
            "read-after-above-range",
        ] {
            let status = ProcessCommand::new(&executable)
                .arg("settings::tests::explicit_reset_paths_survive_full_process_relaunch")
                .arg("--exact")
                .arg("--nocapture")
                .env(SETTINGS_TEST_IDENTIFIER_ENV, &identifier)
                .env(SETTINGS_TEST_PHASE_ENV, phase)
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
        let fixed_clock = fixed.initial_clock(wall_now_t);
        assert_eq!(fixed_clock.t(), 21_600.0);
        assert_eq!(fixed_clock.rate(), RateIndex::REAL);

        let live = AppSettings {
            start_mode: StartModeSetting::Live,
            ..default()
        };
        let mut live_clock = live.initial_clock(wall_now_t);
        assert_eq!(live_clock.t(), wall_now_t);
        assert_eq!(live_clock.rate(), RateIndex::REAL);
        assert!(live_clock.is_live(wall_now_t));
        live_clock.set_rate(StartupRateSetting::default().rate());
        assert!(!live_clock.is_live(wall_now_t));
        live_clock.snap_to_live();
        let report = live_clock.tick(1.0, wall_now_t);
        assert!(report.snapped_live);
        assert_eq!(live_clock.rate(), RateIndex::REAL);
        assert!(live_clock.is_live(wall_now_t));

        let beyond_range = AppSettings {
            start_mode: StartModeSetting::FixedEpoch {
                jd_tdb: 3_000_000.0,
            },
            ..default()
        };
        assert_eq!(beyond_range.initial_clock(wall_now_t).t(), T_MAX_S);
    }

    #[test]
    fn fixed_epoch_normalization_is_shared_by_apply_display_and_clock() {
        let (minimum, maximum) = fixed_epoch_bounds_jd_tdb();
        for (requested, expected) in [
            (minimum - 1_000.0, minimum),
            (maximum + 1_000.0, maximum),
            (f64::NAN, DEFAULT_START_EPOCH_JD_TDB),
            (f64::INFINITY, DEFAULT_START_EPOCH_JD_TDB),
            (f64::NEG_INFINITY, DEFAULT_START_EPOCH_JD_TDB),
        ] {
            let requested = settings_with_fixed_epoch(requested);
            let mut settings = AppSettings::default();
            let mut screen = SettingsScreenState {
                draft: requested.clone(),
                ..default()
            };
            let mut save = SettingsSaveRequest::default();
            consume_settings_command(
                &SimCommand::ApplySettings(Box::new(requested)),
                &mut screen,
                &mut settings,
                &mut save,
            );

            assert_fixed_epoch_convergence(&settings, expected);
            assert_fixed_epoch_convergence(&screen.draft, expected);
            assert!(save.is_requested());
            let serialized = ron::to_string(&settings).unwrap();
            let restored: AppSettings = ron::from_str(&serialized).unwrap();
            assert_eq!(fixed_epoch_value(&restored).to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn outward_epoch_steps_at_range_edges_are_noops_and_inward_steps_resume() {
        let (minimum, maximum) = fixed_epoch_bounds_jd_tdb();
        for (edge, outward_delta, inward_delta) in
            [(minimum, -365.25, 365.25), (maximum, 365.25, -365.25)]
        {
            let edge_settings = settings_with_fixed_epoch(edge);
            let mut app = App::new();
            app.insert_resource(edge_settings.clone())
                .insert_resource(SettingsScreenState {
                    draft: edge_settings,
                    ..default()
                })
                .init_resource::<SettingsSaveRequest>()
                .init_resource::<InputFocus>()
                .init_resource::<SimCommandQueue>();
            let outward = app
                .world_mut()
                .spawn(SettingAction::AdjustStartEpoch(outward_delta))
                .observe(activate_setting_action)
                .id();
            let inward = app
                .world_mut()
                .spawn(SettingAction::AdjustStartEpoch(inward_delta))
                .observe(activate_setting_action)
                .id();

            app.world_mut().trigger(Activate { entity: outward });
            let screen = app.world().resource::<SettingsScreenState>();
            assert_eq!(fixed_epoch_value(&screen.draft).to_bits(), edge.to_bits());
            assert!(!screen.dirty);
            assert!(!app.world().resource::<SettingsSaveRequest>().is_requested());
            assert_eq!(
                app.world_mut()
                    .resource_mut::<SimCommandQueue>()
                    .drain()
                    .count(),
                0
            );

            app.world_mut().trigger(Activate { entity: inward });
            let screen = app.world().resource::<SettingsScreenState>();
            assert_eq!(
                fixed_epoch_value(&screen.draft).to_bits(),
                (edge + inward_delta).to_bits()
            );
            assert!(screen.dirty);
        }
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
    fn oom_handler_invokes_the_native_surface_once_on_the_winit_thread() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let recorded_calls = calls.clone();
        let expected_thread = thread::current().id();
        let mut main_world = World::new();
        main_world.insert_resource(RenderRecoveryStatus::default());
        main_world.insert_resource(WinitApplicationThread(expected_thread));
        main_world.insert_resource(NativeOomSurface::new(move |title, message| {
            recorded_calls.lock().unwrap().push((
                thread::current().id(),
                title.to_string(),
                message.to_string(),
            ));
            Ok(())
        }));
        let mut render_world = World::new();
        let error = RenderError {
            ty: ErrorType::OutOfMemory,
            description: "injected OOM".into(),
            source: None,
        };

        for _ in 0..2 {
            assert!(matches!(
                product_render_error_policy(&error, &mut main_world, &mut render_world),
                RenderErrorPolicy::StopRendering
            ));
        }

        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, expected_thread);
        assert!(calls[0].1.contains("Graphics Memory Exhausted"));
        assert!(calls[0].2.contains("Close Solar Sim"));
        assert!(calls[0].2.contains("free graphics memory"));
        assert!(calls[0].2.contains("relaunch"));
        assert_eq!(
            main_world.resource::<RenderRecoveryStatus>().phase(),
            RenderRecoveryPhase::StoppedOutOfMemory
        );
        assert!(
            main_world
                .resource::<RenderRecoveryStatus>()
                .native_oom_surface_invoked
        );
    }

    #[test]
    fn persisted_layers_restore_every_layer_exactly() {
        let mut original = LayerState::default();
        original.set_visible(LayerId::Asteroids, true);
        original.set_visible(LayerId::Comets, true);
        original.set_visible(LayerId::Labels, false);
        let persisted = PersistedLayerState::from_snapshot(original.persistence_snapshot());
        let settings = AppSettings {
            layers: persisted,
            ..default()
        };
        assert_eq!(settings.initial_layer_state(), original);
        assert!(settings
            .initial_layer_state()
            .is_visible(LayerId::Asteroids));
        assert!(settings.initial_layer_state().is_visible(LayerId::Comets));
    }

    #[test]
    fn identical_explicit_settings_commands_persist_without_repainting() {
        let mut settings = AppSettings::default();
        let mut screen = SettingsScreenState::default();
        let screen_before = screen.clone();
        let mut save = SettingsSaveRequest::default();

        consume_settings_command(
            &SimCommand::ApplySettings(Box::default()),
            &mut screen,
            &mut settings,
            &mut save,
        );
        assert_eq!(settings, AppSettings::default());
        assert_eq!(screen, screen_before);
        assert!(
            save.is_requested(),
            "explicit Apply remains a disk boundary"
        );

        save.0 = false;
        consume_settings_command(
            &SimCommand::RestorePresentationDefaults,
            &mut screen,
            &mut settings,
            &mut save,
        );
        assert_eq!(screen, screen_before);
        assert!(
            save.is_requested(),
            "explicit recovery remains durable even when defaults are already active"
        );
    }

    #[test]
    fn idle_settings_save_request_is_not_rewritten() {
        #[derive(Resource, Default)]
        struct SaveRequestChanged(bool);

        fn capture_change(
            request: Res<SettingsSaveRequest>,
            mut changed: ResMut<SaveRequestChanged>,
        ) {
            changed.0 = request.is_changed();
        }

        let mut app = App::new();
        app.init_resource::<SettingsSaveRequest>()
            .insert_resource(SettingsPersistencePolicy::TransientRuntime)
            .init_resource::<SaveRequestChanged>()
            .add_systems(Update, (persist_requested_settings, capture_change).chain());
        app.update();

        app.world_mut().resource_mut::<SaveRequestChanged>().0 = false;
        app.update();
        assert!(!app.world().resource::<SaveRequestChanged>().0);

        app.world_mut()
            .resource_mut::<SettingsSaveRequest>()
            .request();
        app.update();
        assert!(app.world().resource::<SaveRequestChanged>().0);
        assert!(!app.world().resource::<SettingsSaveRequest>().is_requested());
    }

    #[test]
    fn presentation_only_settings_changes_do_not_reapply_window_runtime_values() {
        let mut app = App::new();
        app.insert_resource(AppSettings::default())
            .init_resource::<AppliedRuntimeSettings>()
            .init_resource::<MsaaCapabilities>()
            .init_resource::<UiScale>()
            .init_resource::<WinitSettings>()
            .add_systems(Update, apply_settings_to_runtime);
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        app.world_mut().spawn((Camera3d::default(), Msaa::Off));
        app.update();

        app.world_mut()
            .entity_mut(window)
            .get_mut::<Window>()
            .unwrap()
            .resolution
            .set(1_111.0, 777.0);
        {
            let mut settings = app.world_mut().resource_mut::<AppSettings>();
            settings.layers.icons = false;
            settings.units = DistanceUnit::Miles;
            settings.start_mode = StartModeSetting::Live;
        }
        app.update();

        let resolution = &app
            .world()
            .entity(window)
            .get::<Window>()
            .unwrap()
            .resolution;
        assert_eq!((resolution.width(), resolution.height()), (1_111.0, 777.0));
    }

    #[test]
    fn quality_presets_compose_msaa_bloom_and_retina_scale_through_runtime_apply() {
        let defaults = AppSettings::default();
        assert_eq!(defaults.quality, QualityPreset::High);
        assert!(defaults.retina_rendering);

        let mut app = App::new();
        app.insert_resource(defaults)
            .init_resource::<AppliedRuntimeSettings>()
            .insert_resource(MsaaCapabilities::from_supported_counts(
                &[1, 2, 4, 8],
                &[1, 2, 4],
            ))
            .init_resource::<UiScale>()
            .init_resource::<WinitSettings>()
            .add_systems(Update, apply_settings_to_runtime);
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let camera = app
            .world_mut()
            .spawn((Camera3d::default(), Msaa::Off, Bloom::NATURAL))
            .id();

        for (quality, retina_rendering, expected_msaa, bloom, scale_factor) in [
            (QualityPreset::Low, true, Msaa::Off, false, Some(1.0)),
            (QualityPreset::Medium, true, Msaa::Sample2, true, None),
            (QualityPreset::High, false, Msaa::Sample4, true, Some(1.0)),
            (QualityPreset::Ultra, true, Msaa::Sample4, true, None),
        ] {
            *app.world_mut().resource_mut::<AppSettings>() = AppSettings {
                quality,
                retina_rendering,
                ..default()
            };
            app.update();

            assert_eq!(
                app.world().entity(camera).get::<Msaa>(),
                Some(&expected_msaa),
                "{quality:?} MSAA"
            );
            assert_eq!(
                app.world().entity(camera).contains::<Bloom>(),
                bloom,
                "{quality:?} bloom"
            );
            assert_eq!(
                app.world()
                    .entity(window)
                    .get::<Window>()
                    .unwrap()
                    .resolution
                    .scale_factor_override(),
                scale_factor,
                "{quality:?} Retina composition"
            );
        }
    }

    #[test]
    fn adapter_resolution_intersects_color_and_depth_without_changing_requests() {
        let capabilities = MsaaCapabilities::from_supported_counts(&[1, 2, 4, 8], &[1, 2, 4]);

        assert_eq!(
            QualityPreset::Ultra.requested_msaa(),
            Msaa::Sample8,
            "the stable preset request remains 8x"
        );
        assert_eq!(
            capabilities.resolve(QualityPreset::Ultra.requested_msaa()),
            Msaa::Sample4
        );
        assert_eq!(
            capabilities.resolve(QualityPreset::High.requested_msaa()),
            Msaa::Sample4
        );
        assert_eq!(
            QualityPreset::Ultra.display_label(capabilities),
            "ULTRA — 8× (4× ON THIS DEVICE)"
        );
    }

    #[test]
    fn stable_presentation_convergence_does_not_mark_settings_changed() {
        let mut app = App::new();
        app.insert_resource(AppSettings::default())
            .init_resource::<LayerState>()
            .init_resource::<PresentationState>()
            .init_resource::<SettingsPersistencePolicy>()
            .init_resource::<AppSettingsChangeCount>()
            .add_systems(
                Update,
                (
                    sync_external_presentation_to_settings,
                    count_app_settings_changes,
                )
                    .chain(),
            );
        app.update();

        app.world_mut().resource_mut::<AppSettingsChangeCount>().0 = 0;
        app.update();

        assert_eq!(app.world().resource::<AppSettingsChangeCount>().0, 0);
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
    fn canonical_close_dismisses_the_modal_and_clears_focus() {
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
            .add_systems(Update, sync_settings_screen);
        app.world_mut()
            .resource_mut::<PresentationState>()
            .close_settings();

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
    fn ui_scale_cycle_reaches_every_supported_scale() {
        let mut scale = 0.75;
        let mut observed = Vec::new();
        for _ in 0..5 {
            scale = next_ui_scale(scale);
            observed.push(scale);
        }
        assert_eq!(observed, vec![1.0, 1.25, 1.5, 2.0, 0.75]);
    }

    #[test]
    fn startup_rate_control_cycles_every_ladder_detent() {
        let start = StartupRateSetting::default();
        let mut current = start;
        let mut observed = std::collections::BTreeSet::new();
        for _ in 0..24 {
            observed.insert(current.rate());
            current = current.next();
        }
        assert_eq!(observed.len(), 24);
        assert_eq!(current, start);
    }

    #[test]
    fn settings_tab_indices_are_unique_and_follow_semantic_order() {
        let mut app = settings_screen_test_app();
        app.update();

        let mut controls = {
            let world = app.world_mut();
            let mut query = world.query::<(&SettingAction, &TabIndex)>();
            query
                .iter(world)
                .map(|(action, index)| (index.0, *action))
                .collect::<Vec<_>>()
        };
        controls.sort_by_key(|(index, _)| *index);
        assert_eq!(controls.len(), 41);
        assert_eq!(
            controls.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
            (SETTINGS_FIRST_TAB_INDEX..SETTINGS_FIRST_TAB_INDEX + 41).collect::<Vec<_>>()
        );
        assert_eq!(
            controls
                .into_iter()
                .map(|(_, action)| action)
                .collect::<Vec<_>>(),
            expected_settings_tab_order()
        );

        let close = settings_action_entity(app.world_mut(), SettingAction::Close);
        let row = app
            .world()
            .entity(close)
            .get::<ChildOf>()
            .expect("close action row")
            .parent();
        let footer = app
            .world()
            .entity(row)
            .get::<ChildOf>()
            .expect("footer parent")
            .parent();
        let row_node = app.world().entity(row).get::<Node>().expect("footer row");
        assert_eq!(row_node.flex_wrap, FlexWrap::Wrap);
        let footer_node = app
            .world()
            .entity(footer)
            .get::<Node>()
            .expect("settings footer");
        assert_eq!(footer_node.height, auto());
        assert_eq!(footer_node.min_height, px(42));
        assert_eq!(footer_node.flex_shrink, 0.0);
    }

    #[test]
    fn changed_draft_actions_restore_context_while_noops_retain_entity_identity() {
        let mut app = settings_screen_test_app();
        app.update();
        let actions = expected_settings_tab_order()
            .into_iter()
            .filter(|action| {
                !matches!(
                    action,
                    SettingAction::Close | SettingAction::Apply | SettingAction::RestoreDefaults
                )
            })
            .collect::<Vec<_>>();
        let mut no_op_count = 0;

        for action in actions {
            let entity = settings_action_entity(app.world_mut(), action);
            let original_scroll = settings_scroll_entity(app.world_mut());
            let root = app
                .world_mut()
                .query_filtered::<Entity, With<SettingsScreenRoot>>()
                .single(app.world())
                .unwrap();
            let draft_before = app.world().resource::<SettingsScreenState>().draft.clone();
            app.world_mut()
                .entity_mut(original_scroll)
                .get_mut::<ScrollPosition>()
                .expect("settings scroll position")
                .y = 137.0;
            app.world_mut()
                .resource_mut::<InputFocus>()
                .set(entity, FocusCause::Navigated);
            app.world_mut().trigger(Activate { entity });
            let draft_changed = app.world().resource::<SettingsScreenState>().draft != draft_before;
            app.update();

            let focused = app
                .world()
                .resource::<InputFocus>()
                .get()
                .expect("draft rebuild restores focus");
            assert_eq!(
                app.world().entity(focused).get::<SettingAction>(),
                Some(&action)
            );
            let scroll = settings_scroll_entity(app.world_mut());
            assert_eq!(
                app.world()
                    .entity(scroll)
                    .get::<ScrollPosition>()
                    .expect("rebuilt settings scroll position")
                    .y,
                137.0,
                "{action:?}"
            );
            let current_root = app
                .world_mut()
                .query_filtered::<Entity, With<SettingsScreenRoot>>()
                .single(app.world())
                .unwrap();
            if draft_changed {
                assert_ne!(current_root, root, "{action:?}");
                assert_eq!(
                    app.world().resource::<SettingsScreenState>().scroll_y,
                    137.0,
                    "{action:?}"
                );
            } else {
                no_op_count += 1;
                assert_eq!(current_root, root, "{action:?}");
                assert_eq!(scroll, original_scroll, "{action:?}");
            }
        }
        assert!(
            no_op_count > 0,
            "the matrix must exercise selected-value noops"
        );
    }

    #[test]
    fn model_driven_rebuild_preserves_actual_focus_and_scroll() {
        let mut app = settings_screen_test_app();
        app.update();
        let action = SettingAction::ToggleInvertVertical;
        let focused = settings_action_entity(app.world_mut(), action);
        let scroll = settings_scroll_entity(app.world_mut());
        app.world_mut()
            .entity_mut(scroll)
            .get_mut::<ScrollPosition>()
            .expect("settings scroll position")
            .y = 211.0;
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(focused, FocusCause::Navigated);

        let mut committed = app.world().resource::<AppSettings>().clone();
        committed.units = DistanceUnit::Miles;
        let mut screen = app
            .world_mut()
            .remove_resource::<SettingsScreenState>()
            .expect("settings screen state");
        let mut settings = app
            .world_mut()
            .remove_resource::<AppSettings>()
            .expect("app settings");
        let mut save = app
            .world_mut()
            .remove_resource::<SettingsSaveRequest>()
            .expect("settings save request");
        consume_settings_command(
            &SimCommand::ApplySettings(Box::new(committed)),
            &mut screen,
            &mut settings,
            &mut save,
        );
        assert_eq!(screen.restore_focus, None);
        app.insert_resource(screen)
            .insert_resource(settings)
            .insert_resource(save);

        app.update();

        let focused = app
            .world()
            .resource::<InputFocus>()
            .get()
            .expect("model rebuild restores focus");
        assert_eq!(
            app.world().entity(focused).get::<SettingAction>(),
            Some(&action)
        );
        let scroll = settings_scroll_entity(app.world_mut());
        assert_eq!(
            app.world()
                .entity(scroll)
                .get::<ScrollPosition>()
                .expect("model rebuild restores scroll")
                .y,
            211.0
        );
        assert_eq!(
            app.world().resource::<SettingsScreenState>().scroll_y,
            211.0
        );
    }

    #[test]
    fn settings_controls_are_reachable_for_required_viewports() {
        for (width, height, scale) in test_layout::required_viewports() {
            let mut app = test_layout::app(width, height, scale);
            app.insert_resource(UiTheme::default())
                .init_resource::<InputFocus>()
                .init_resource::<MsaaCapabilities>()
                .insert_resource(SettingsScreenState {
                    open: true,
                    draft: AppSettings::default(),
                    dirty: true,
                    scroll_y: 0.0,
                    restore_focus: None,
                })
                .add_systems(Update, rebuild_settings_screen);
            test_layout::settle(&mut app);

            let scroll_area = settings_scroll_entity(app.world_mut());
            let root = app
                .world_mut()
                .query_filtered::<Entity, With<SettingsScreenRoot>>()
                .single(app.world())
                .unwrap();
            let root_rect = node_rect(app.world(), root);
            let scroll_rect = node_rect(app.world(), scroll_area);
            assert!(
                scroll_rect.height() > 0.0 && rect_contains(root_rect, scroll_rect),
                "{width}×{height} scale {scale}: Settings scroll viewport {scroll_rect:?} is invalid inside {root_rect:?}"
            );

            for action in [
                SettingAction::Close,
                SettingAction::Revert,
                SettingAction::RestoreDefaults,
                SettingAction::Apply,
            ] {
                let entity = settings_action_entity(app.world_mut(), action);
                let rect = node_rect(app.world(), entity);
                assert!(
                    rect_contains(root_rect, rect),
                    "{width}×{height} scale {scale}: footer action {action:?} escaped Settings root"
                );
            }

            app.world_mut()
                .entity_mut(scroll_area)
                .get_mut::<ScrollPosition>()
                .unwrap()
                .y = f32::MAX;
            test_layout::settle(&mut app);
            let last_content =
                settings_action_entity(app.world_mut(), SettingAction::ToggleLayer(LayerId::Icons));
            let last_rect = node_rect(app.world(), last_content);
            let scroll_rect = node_rect(app.world(), scroll_area);
            assert!(
                rect_contains(scroll_rect, last_rect),
                "{width}×{height} scale {scale}: final Settings content action {last_rect:?} is not reachable inside {scroll_rect:?}"
            );
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

    fn rect_contains(outer: Rect, inner: Rect) -> bool {
        inner.min.x >= outer.min.x - 1.0
            && inner.max.x <= outer.max.x + 1.0
            && inner.min.y >= outer.min.y - 1.0
            && inner.max.y <= outer.max.y + 1.0
    }

    #[test]
    fn settings_screen_renders_every_control_with_accessibility_labels() {
        let mut app = settings_screen_test_app();
        app.update();
        let initially_focused = app
            .world()
            .resource::<InputFocus>()
            .get()
            .expect("Settings must seed focus inside its modal tab group");
        assert_eq!(
            app.world().entity(initially_focused).get::<SettingAction>(),
            Some(&SettingAction::Close)
        );
        let scroll = settings_scroll_entity(app.world_mut());
        app.world_mut()
            .entity_mut(scroll)
            .get_mut::<ScrollPosition>()
            .expect("settings scroll position")
            .y = 137.0;
        {
            let mut screen = app.world_mut().resource_mut::<SettingsScreenState>();
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
            assert_eq!(controls.len(), 41);
            assert!(controls
                .iter()
                .all(|(_, _, _, label, _)| !label.0.trim().is_empty()));
            assert!(controls.iter().any(|(_, _, _, label, _)| {
                label.0 == "Set ULTRA — 8× (4× ON THIS DEVICE)"
            }));
            let mut descriptions =
                world.query_filtered::<&Text, With<RetinaRenderingDescription>>();
            assert_eq!(
                descriptions
                    .single(world)
                    .expect("one Retina scope description")
                    .0,
                "Takes effect in windowed mode; fullscreen renders at display resolution."
            );
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
            .close_settings();
        app.update();
        let world = app.world_mut();
        let mut roots = world.query::<&SettingsScreenRoot>();
        assert_eq!(roots.iter(world).count(), 0);
    }
}
