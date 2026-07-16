//! WP4–WP14 — simulation rendering, camera control, reusable HUD, and settings.
//!
//! `sim-core` remains the f64 source of truth. This crate owns filesystem
//! loading, parent-to-heliocentric composition, the one f64→f32 render rebase,
//! explicit frame-flow ordering, WP5's single command-consumer boundary, and
//! WP6's retained orbit rendering. Raw device events are isolated in
//! `input_intent`; camera/control state is private to `control`, which also
//! supplies the headless replay gate.

mod control;
mod formatting;
mod golden;
mod input_intent;
mod labels;
mod layers;
mod left_panel;
mod orbit_lines;
mod platform;
mod scene_polish;
mod search;
mod settings;
mod starfield;
#[cfg(any(feature = "steam", test))]
mod steam_app_id;
mod surface_textures;
mod time_bar;
mod ui_kit;

pub use control::{
    replay_headless, CameraController, CommandRecording, HeadlessSimulation, ReplayFrameInput,
    ReplayParseError, ReplayRunError, ReplayStream, ReplayVersion, SimCommand, StampedCommand,
};
pub use formatting::format_distance_km;
pub use golden::{
    golden_view, GoldenCaptureOptions, GoldenViewSpec, GOLDEN_HEIGHT, GOLDEN_VIEWS, GOLDEN_WIDTH,
};
pub use labels::{
    declutter_labels, moon_label_is_contextually_visible, ray_sphere_hit_distance, BodyLabel,
    DeclutterCandidate, LabelPriority, LabelsPlugin, ScreenRect, SelectionPlugin,
};
pub use layers::{
    visual_cue_recovery_needed, LayerId, LayerState, LayerStateSnapshot, LayersPanelRoot,
    LayersPlugin, PresentationState, RightRailRoot, UiRestoreAffordance, VisualCueRecoveryRoot,
    ZOOM_IN_DOLLY_DELTA, ZOOM_OUT_DOLLY_DELTA,
};
pub use left_panel::{
    body_info_view_model, body_info_view_model_with_units, moon_collections,
    rendered_body_radius_units, BodyInfoViewModel, BodyLinkViewModel, BodySizeScale,
    DescriptionViewModel, InfoViewModelError, LeftPanelPlugin, LeftPanelRoot, LeftPanelTab,
    MoonCollectionViewModel, MoonVisibilityMode, OrbitalPeriodViewModel, ViewOptionsSnapshot,
    ViewOptionsState,
};
pub use orbit_lines::{
    orbit_vertex_count, sample_orbit, OrbitLineBrightness, OrbitLinesPlugin, OrbitPath,
    HYPERBOLIC_HALF_SPAN_S, MAX_ORBIT_VERTICES, MIN_ORBIT_VERTICES,
};
pub use platform::{
    NoopPlatformServices, PlatformServices, PlatformServicesPlugin, PlatformStatus,
};
#[cfg(feature = "steam")]
pub use platform::{SteamPlatformServices, SteamPlugin};
pub use scene_polish::{
    hysteresis_state, phase_step_rad, simulated_step_for_phase, BodyOrbitEmphasis,
    OrbitEmphasisOnset, OrbitEmphasisState, ScenePolishPlugin, SunLight, AMBIENT_BRIGHTNESS,
    EMPHASIS_CROSSFADE_S, EMPHASIS_ENGAGE_PHASE_RAD, EMPHASIS_RELEASE_PHASE_RAD,
    EMPHASIZED_ORBIT_BRIGHTNESS, SUN_LIGHT_INTENSITY_LUMENS, SUN_LIGHT_RANGE_UNITS,
};
pub use search::{
    search_catalog, BrowseColumn, BrowseColumnKind, BrowseCounts, BrowseEntry, BrowseMenuRoot,
    BrowseModel, SearchDropdownRoot, SearchHit, SearchMatchKind, SearchPlugin,
};
pub use settings::{
    AppSettings, DisplayModeSetting, DistanceUnit, FrameCap, PersistedLayerState,
    ProductSettingsPlugin, QualityPreset, RecoveryDirective, RenderErrorScreen, RenderFailureKind,
    RenderRecoveryPhase, RenderRecoveryStatus, ResolutionSetting, SettingsScreenRoot,
    StartModeSetting, SETTINGS_IDENTIFIER,
};
pub use starfield::{
    decode_starfield, load_starfield, StarfieldAssetError, StarfieldPlugin, StarfieldPoint,
    StarfieldRoot, StarfieldSource, DEFAULT_STARFIELD_PATH, EXPECTED_STARFIELD_POINTS,
    STARFIELD_RADIUS_UNITS,
};
pub use time_bar::{
    commit_time_edit, live_chip_active, rate_for_slider_value, slider_value_for_rate,
    toasts_for_tick_report, TimeBarPlugin, TimeBarRoot, TimeEditField, TimeEditOutcome,
    TimeToastKind, TIME_BAR_HEIGHT_PX,
};
pub use ui_kit::{
    checkbox_row, chip, panel, section_header, slider, tab_bar, toast, top_bar, BreadcrumbText,
    MenuBrowseButton, NavigationDestination, NavigationItem, NavigationStack, SearchHint,
    SearchInput, SearchPlaceholder, TopBarRoot, UiColorToken, UiColors, UiKitPlugin, UiSpacing,
    UiTheme, UiTypeScale, WidgetKind, WidgetRoot, WidgetSpec, WidgetVisualState,
    BREADCRUMB_SEPARATOR, INTER_FONT_ASSET, ROOT_NAVIGATION_ID, TOP_BAR_HEIGHT_PX,
};
#[cfg(debug_assertions)]
pub use ui_kit::{WidgetGalleryCell, WidgetGalleryRoot};

use bevy::camera::{RenderTarget, ShadowLodOrigin};
#[cfg(debug_assertions)]
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::ecs::system::SystemParam;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::settings::SettingsPlugin;
use bevy::ui::IsDefaultUiCamera;
use bevy::window::ExitCondition;
use control::{
    advance_camera_controller, consume_application_command, consume_sim_command,
    framing_distance_units, full_system_framing_distance_units, SimCommandQueue, SimulationFrame,
};
use input_intent::InputIntentPlugin;
#[cfg(debug_assertions)]
use settings::{request_debug_device_loss, DebugDeviceLossRequest};
use sim_core::catalog::{Catalog, CatalogError, Category};
use sim_core::kepler::{state_at, KeplerError, StateVector};
use sim_core::time::{t_from_unix_utc, SimClock, TickReport};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const DEFAULT_CATALOG_PATH: &str = "assets/catalog.ron";
const DEFAULT_BEVY_ASSET_ROOT: &str = "../../assets";
pub const KM_PER_RENDER_UNIT: f64 = 1_000.0;
const DEFAULT_SMOKE_FRAMES: u32 = 60;
pub const DEFAULT_CAMERA_DISTANCE_UNITS: f64 = 250_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedBackend {
    Metal,
    Dx12,
    Vulkan,
}

impl ExpectedBackend {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "metal" => Some(Self::Metal),
            "dx12" => Some(Self::Dx12),
            "vulkan" => Some(Self::Vulkan),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Metal => "metal",
            Self::Dx12 => "dx12",
            Self::Vulkan => "vulkan",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptionsError(String);

impl fmt::Display for RunOptionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RunOptionsError {}

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub catalog_path: PathBuf,
    pub smoke_frames: Option<u32>,
    pub expected_backend: Option<ExpectedBackend>,
    pub reject_software_adapter: bool,
    pub assert_nonblack: bool,
    pub initial_focus_id: Option<String>,
    /// Debug-only renderer recovery probe. It is translated through the same
    /// recorded `SimCommand` path as the F9 binding.
    pub simulate_device_loss: bool,
    pub reset_settings: bool,
    pub golden_capture: Option<GoldenCaptureOptions>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            catalog_path: PathBuf::from(DEFAULT_CATALOG_PATH),
            smoke_frames: None,
            expected_backend: None,
            reject_software_adapter: false,
            assert_nonblack: false,
            initial_focus_id: None,
            simulate_device_loss: false,
            reset_settings: false,
            golden_capture: None,
        }
    }
}

impl RunOptions {
    pub fn from_args(args: &[String]) -> Result<Self, RunOptionsError> {
        let mut options = Self::default();
        let mut golden_view = None;
        let mut golden_backend = None;
        let mut golden_output = None;
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--smoke" => {
                    let smoke_frames =
                        match args.get(i + 1).and_then(|value| value.parse::<u32>().ok()) {
                            Some(frames) => frames,
                            None => DEFAULT_SMOKE_FRAMES,
                        };
                    options.smoke_frames = Some(smoke_frames);
                    if args
                        .get(i + 1)
                        .is_some_and(|value| value.parse::<u32>().is_ok())
                    {
                        i += 1;
                    }
                }
                "--expect-backend" => {
                    let value = args.get(i + 1).ok_or_else(|| {
                        RunOptionsError("--expect-backend requires metal, dx12, or vulkan".into())
                    })?;
                    options.expected_backend =
                        Some(ExpectedBackend::parse(value).ok_or_else(|| {
                            RunOptionsError(format!(
                                "unsupported backend '{value}'; expected metal, dx12, or vulkan"
                            ))
                        })?);
                    i += 1;
                }
                "--reject-software-adapter" => options.reject_software_adapter = true,
                "--assert-nonblack" => options.assert_nonblack = true,
                "--focus" => {
                    if let Some(value) = args.get(i + 1) {
                        options.initial_focus_id = Some(value.clone());
                        i += 1;
                    }
                }
                "--catalog" => {
                    if let Some(value) = args.get(i + 1) {
                        options.catalog_path = PathBuf::from(value);
                        i += 1;
                    }
                }
                "--golden-view" => {
                    if let Some(value) = args.get(i + 1) {
                        golden_view = Some(value.clone());
                        i += 1;
                    }
                }
                "--golden-backend" => {
                    if let Some(value) = args.get(i + 1) {
                        golden_backend = Some(value.clone());
                        i += 1;
                    }
                }
                "--golden-capture" => {
                    if let Some(value) = args.get(i + 1) {
                        golden_output = Some(PathBuf::from(value));
                        i += 1;
                    }
                }
                #[cfg(debug_assertions)]
                "--simulate-device-loss" => options.simulate_device_loss = true,
                "--reset-settings" => options.reset_settings = true,
                _ => {}
            }
            i += 1;
        }
        if let (Some(view), Some(backend), Some(output)) =
            (golden_view, golden_backend, golden_output)
        {
            options.golden_capture = Some(GoldenCaptureOptions {
                view,
                backend,
                output,
                reject_software_adapter: options.reject_software_adapter,
            });
        }
        if options.smoke_frames.is_none()
            && (options.expected_backend.is_some() || options.assert_nonblack)
        {
            return Err(RunOptionsError(
                "--expect-backend and --assert-nonblack require --smoke".into(),
            ));
        }
        if options.reject_software_adapter
            && options.smoke_frames.is_none()
            && options.golden_capture.is_none()
        {
            return Err(RunOptionsError(
                "--reject-software-adapter requires --smoke or golden capture options".into(),
            ));
        }
        Ok(options)
    }
}

#[derive(Debug)]
pub enum CatalogLoadError {
    Read { path: PathBuf, message: String },
    Parse(String),
    Validation(Vec<CatalogError>),
    Propagation(PropagationError),
}

impl fmt::Display for CatalogLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogLoadError::Read { path, message } => {
                write!(f, "could not read '{}': {message}", path.display())
            }
            CatalogLoadError::Parse(message) => write!(f, "catalog syntax error: {message}"),
            CatalogLoadError::Validation(errors) => {
                write!(f, "catalog validation failed")?;
                for error in errors {
                    write!(f, "\n- {error}")?;
                }
                Ok(())
            }
            CatalogLoadError::Propagation(error) => {
                write!(f, "catalog could not produce initial states: {error}")
            }
        }
    }
}

pub fn load_catalog_text(text: &str) -> Result<Catalog, CatalogLoadError> {
    let catalog =
        Catalog::from_ron_str(text).map_err(|error| CatalogLoadError::Parse(error.to_string()))?;
    catalog.validate().map_err(CatalogLoadError::Validation)?;
    Ok(catalog)
}

pub fn load_catalog_from_path(path: &Path) -> Result<Catalog, CatalogLoadError> {
    let text = std::fs::read_to_string(path).map_err(|error| CatalogLoadError::Read {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    load_catalog_text(&text)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropagationError {
    MissingParent { body: String },
    ParentNotBeforeChild { body: String, parent: String },
    MissingParentGm { body: String, parent: String },
    MissingOrbit { body: String },
    Kepler { body: String, source: KeplerError },
}

impl fmt::Display for PropagationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PropagationError::MissingParent { body } => {
                write!(f, "'{body}' has no parent")
            }
            PropagationError::ParentNotBeforeChild { body, parent } => {
                write!(f, "parent '{parent}' does not precede child '{body}'")
            }
            PropagationError::MissingParentGm { body, parent } => {
                write!(f, "parent '{parent}' has no GM for '{body}'")
            }
            PropagationError::MissingOrbit { body } => write!(f, "'{body}' has no orbit"),
            PropagationError::Kepler { body, source } => {
                write!(f, "could not propagate '{body}': {source}")
            }
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct BodyStates(pub Vec<StateVector>);

pub fn propagate_catalog(catalog: &Catalog, t_s: f64) -> Result<BodyStates, PropagationError> {
    let mut states = BodyStates(vec![StateVector::default(); catalog.bodies.len()]);
    propagate_into(catalog, t_s, &mut states)?;
    Ok(states)
}

fn propagate_into(
    catalog: &Catalog,
    t_s: f64,
    states: &mut BodyStates,
) -> Result<(), PropagationError> {
    states
        .0
        .resize(catalog.bodies.len(), StateVector::default());
    let indices: HashMap<&str, usize> = catalog
        .bodies
        .iter()
        .enumerate()
        .map(|(index, body)| (body.id.as_str(), index))
        .collect();

    for (body_index, body) in catalog.bodies.iter().enumerate() {
        if body.category == Category::Star {
            states.0[body_index] = StateVector::default();
            continue;
        }

        let parent_id = body
            .parent
            .as_deref()
            .ok_or_else(|| PropagationError::MissingParent {
                body: body.id.clone(),
            })?;
        let parent_index =
            indices
                .get(parent_id)
                .copied()
                .ok_or_else(|| PropagationError::MissingParent {
                    body: body.id.clone(),
                })?;
        if parent_index >= body_index {
            return Err(PropagationError::ParentNotBeforeChild {
                body: body.id.clone(),
                parent: parent_id.to_string(),
            });
        }
        let mu = catalog.bodies[parent_index].gm_km3_s2.ok_or_else(|| {
            PropagationError::MissingParentGm {
                body: body.id.clone(),
                parent: parent_id.to_string(),
            }
        })?;
        let orbit = body
            .orbit
            .as_ref()
            .ok_or_else(|| PropagationError::MissingOrbit {
                body: body.id.clone(),
            })?;
        let relative = state_at(orbit, mu, t_s).map_err(|source| PropagationError::Kepler {
            body: body.id.clone(),
            source,
        })?;
        let parent = states.0[parent_index];
        states.0[body_index] = StateVector {
            position_km: add_f64(parent.position_km, relative.position_km),
            velocity_km_s: add_f64(parent.velocity_km_s, relative.velocity_km_s),
        };
    }
    Ok(())
}

fn add_f64(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[derive(Resource)]
pub struct LoadedCatalog {
    pub catalog: Catalog,
    indices: HashMap<String, usize>,
}

impl LoadedCatalog {
    fn new(catalog: Catalog) -> Self {
        let indices = catalog
            .bodies
            .iter()
            .enumerate()
            .map(|(index, body)| (body.id.clone(), index))
            .collect();
        Self { catalog, indices }
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.indices.get(id).copied()
    }
}

#[derive(Resource)]
struct CatalogFailure(String);

#[derive(Resource)]
pub struct SimulationClock(SimClock);

/// Signed simulation-time advance produced by the clock tick for this frame.
///
/// The Commands set runs before `tick_clock`, so command jumps such as
/// `SetTime` are deliberately outside this value. Render-only consumers use
/// the actual post-clamp/post-snap advance instead of reconstructing it from
/// the configured rate, which would be wrong at the soft-range pins and while
/// snapping to LIVE.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct SimulationTickAdvance {
    seconds: f64,
}

impl SimulationTickAdvance {
    fn between(before_tick_t: f64, after_tick_t: f64) -> Self {
        let seconds = after_tick_t - before_tick_t;
        Self {
            seconds: if seconds.is_finite() { seconds } else { 0.0 },
        }
    }

    pub(crate) fn seconds(self) -> f64 {
        self.seconds
    }
}

#[derive(Message, Debug, Clone, Copy)]
struct ClockTickReport(TickReport);

#[derive(Resource, Default)]
struct PropagationFault(Option<String>);

#[derive(Resource)]
struct SmokeFrames {
    target: Option<u32>,
    expected_backend: Option<ExpectedBackend>,
    reject_software_adapter: bool,
    assert_nonblack: bool,
    seen: u32,
    started: Option<Instant>,
    screenshot_requested: bool,
    completed: bool,
}

impl SmokeFrames {
    fn new(
        target: Option<u32>,
        expected_backend: Option<ExpectedBackend>,
        reject_software_adapter: bool,
        assert_nonblack: bool,
    ) -> Self {
        Self {
            target,
            expected_backend,
            reject_software_adapter,
            assert_nonblack,
            seen: 0,
            started: None,
            screenshot_requested: false,
            completed: false,
        }
    }
}

#[derive(Component)]
pub struct BodyVisual {
    pub index: usize,
}

#[derive(Component)]
pub struct BodyId(pub String);

#[derive(Component)]
struct CatalogErrorScreen;

/// Render-space parent of the camera. Its local origin is the f64 moving
/// focus after rebasing, so following a body requires no camera translation
/// correction and remains emergent after a travel tween lands.
#[derive(Component)]
struct CameraFocusAnchor;

#[cfg(debug_assertions)]
#[derive(Component)]
struct DiagText;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimulationSet {
    Input,
    Commands,
    Clock,
    Propagation,
    Origin,
    Camera,
    Render,
}

fn configure_frame_flow(app: &mut App) {
    app.configure_sets(
        Update,
        (
            SimulationSet::Input,
            SimulationSet::Commands,
            SimulationSet::Clock,
            SimulationSet::Propagation,
            SimulationSet::Origin,
            SimulationSet::Camera,
            SimulationSet::Render,
        )
            .chain(),
    );
}

pub struct PropagationPlugin;

impl Plugin for PropagationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, propagate_bodies.in_set(SimulationSet::Propagation));
    }
}

pub struct OriginPlugin;

impl Plugin for OriginPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (advance_camera_focus, update_focus_and_rebase)
                .chain()
                .in_set(SimulationSet::Origin),
        );
    }
}

pub struct CameraRigPlugin;

impl Plugin for CameraRigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_camera.in_set(SimulationSet::Camera));
    }
}

pub fn run_from_env() {
    let args: Vec<String> = std::env::args().collect();
    let options = match RunOptions::from_args(&args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    if options
        .golden_capture
        .as_ref()
        .is_some_and(|capture| golden::golden_view(&capture.view).is_none())
    {
        eprintln!("unknown golden view");
        std::process::exit(2);
    }
    let catalog = load_catalog_from_path(&options.catalog_path);
    #[cfg(feature = "steam")]
    let mut app = build_app_with_platform(options, catalog, SteamPlugin);
    #[cfg(not(feature = "steam"))]
    let mut app = build_app(options, catalog);
    match app.run() {
        AppExit::Success => {}
        AppExit::Error(code) => std::process::exit(i32::from(code.get())),
    }
}

pub fn build_app(options: RunOptions, catalog: Result<Catalog, CatalogLoadError>) -> App {
    build_app_with_platform(options, catalog, PlatformServicesPlugin::default())
}

fn build_app_with_platform<P: Plugin>(
    options: RunOptions,
    catalog: Result<Catalog, CatalogLoadError>,
    platform_plugin: P,
) -> App {
    let mut app = App::new();
    // Steam's overlay must hook before Bevy creates the graphics device.
    // Do not move platform initialization below DefaultPlugins.
    app.add_plugins(platform_plugin);
    let golden_capture = options.golden_capture.clone();
    let golden_spec = golden_capture
        .as_ref()
        .and_then(|capture| golden::golden_view(&capture.view));
    #[cfg(debug_assertions)]
    if golden_capture.is_some() {
        app.insert_resource(ui_kit::WidgetGalleryEnabled(false));
    }
    let primary_window = if golden_capture.is_some() {
        Window {
            title: "Solar Sim Golden".into(),
            resolution: bevy::window::WindowResolution::new(
                golden::GOLDEN_WIDTH,
                golden::GOLDEN_HEIGHT,
            )
            .with_scale_factor_override(1.0),
            resizable: false,
            present_mode: bevy::window::PresentMode::AutoNoVsync,
            ..default()
        }
    } else {
        Window {
            title: "Solar Sim".into(),
            ..default()
        }
    };
    let default_plugins = DefaultPlugins
        .set(AssetPlugin {
            file_path: DEFAULT_BEVY_ASSET_ROOT.to_string(),
            ..default()
        })
        .set(WindowPlugin {
            primary_window: Some(Window { ..primary_window }),
            exit_condition: ExitCondition::DontExit,
            ..default()
        });
    app.add_plugins(default_plugins);
    app.register_type::<AppSettings>()
        .add_plugins(SettingsPlugin::new(SETTINGS_IDENTIFIER));
    // Resolve the explicit recovery boundary before any clock, layer, window,
    // or capture state is derived from the loaded settings.
    let persistence = if golden_capture.is_some() {
        settings::SettingsPersistencePolicy::TransientRuntime
    } else {
        settings::SettingsPersistencePolicy::Persistent
    };
    let bootstrapped_settings =
        settings::bootstrap_app_settings(app.world_mut(), options.reset_settings, persistence);
    let initial_settings = if golden_capture.is_some() {
        let settings = AppSettings {
            resolution: ResolutionSetting {
                width: golden::GOLDEN_WIDTH,
                height: golden::GOLDEN_HEIGHT,
            },
            vsync: false,
            frame_cap: FrameCap::Unlimited,
            ..default()
        };
        settings.normalized()
    } else {
        bootstrapped_settings
    };
    let initial_layers = golden_spec.map_or_else(
        || initial_settings.initial_layer_state(),
        golden::layer_state_for_view,
    );
    app.insert_resource(initial_settings.clone())
        .insert_resource(initial_layers)
        .insert_resource(PresentationState::with_fullscreen(
            initial_settings.display_mode.is_fullscreen(),
        ));
    configure_frame_flow(&mut app);

    let wall_now_t = wall_now_t();
    let initial_clock = initial_settings.initial_clock(wall_now_t);
    let initial_t_s = initial_clock.t();
    #[cfg(debug_assertions)]
    let initial_commands = {
        let mut commands = SimCommandQueue::default();
        if options.simulate_device_loss {
            commands.push(SimCommand::SimulateDeviceLoss);
        }
        commands
    };
    #[cfg(not(debug_assertions))]
    let initial_commands = SimCommandQueue::default();
    app.insert_resource(SimulationClock(initial_clock))
        .insert_resource(SimulationTickAdvance::default())
        .insert_resource(initial_commands)
        .insert_resource(CommandRecording::default())
        .insert_resource(SimulationFrame::default())
        .insert_resource(PropagationFault::default())
        .insert_resource(SmokeFrames::new(
            options.smoke_frames,
            options.expected_backend,
            options.reject_software_adapter,
            options.assert_nonblack,
        ))
        .add_message::<ClockTickReport>();

    match catalog.and_then(|catalog| {
        let states =
            propagate_catalog(&catalog, initial_t_s).map_err(CatalogLoadError::Propagation)?;
        Ok((catalog, states))
    }) {
        Ok((catalog, states)) => {
            let loaded = LoadedCatalog::new(catalog);
            let requested_focus = golden_spec
                .and_then(|view| loaded.index_of(view.focus_id))
                .or_else(|| {
                    options
                        .initial_focus_id
                        .as_deref()
                        .and_then(|id| loaded.index_of(id))
                });
            let focus_index = requested_focus
                .or_else(|| loaded.index_of("sun"))
                .map_or(0, |index| index);
            let distance = if golden_spec.is_some_and(|view| view.distance_units.is_none()) {
                full_system_framing_distance_units(&loaded)
            } else if requested_focus.is_some() {
                framing_distance_units(&loaded, focus_index)
            } else {
                full_system_framing_distance_units(&loaded)
            };
            let mut camera =
                CameraController::new(focus_index, states.0[focus_index].position_km, distance);
            if let Some(view) = golden_spec {
                let (yaw, pitch) = if view.face_sun {
                    let sun_index = loaded.index_of("sun").unwrap_or(focus_index);
                    golden::illuminated_pose(
                        states.0[focus_index].position_km,
                        states.0[sun_index].position_km,
                        view.yaw_rad,
                        view.pitch_rad,
                    )
                } else {
                    (view.yaw_rad, view.pitch_rad)
                };
                camera.set_initial_pose(yaw, pitch, view.distance_units.unwrap_or(distance));
            }
            app.insert_resource(camera)
                .insert_resource(loaded)
                .insert_resource(states);
        }
        Err(error) => {
            app.insert_resource(CatalogFailure(error.to_string()))
                .insert_resource(CameraController::unavailable());
        }
    }

    app.add_plugins((
        InputIntentPlugin,
        PropagationPlugin,
        OriginPlugin,
        CameraRigPlugin,
        ScenePolishPlugin,
        StarfieldPlugin,
        OrbitLinesPlugin,
        UiKitPlugin,
        SearchPlugin,
        LayersPlugin,
        TimeBarPlugin,
        LabelsPlugin,
        SelectionPlugin,
        LeftPanelPlugin,
        ProductSettingsPlugin,
    ))
    .add_systems(
        Startup,
        (spawn_body_spheres, spawn_camera, spawn_catalog_error),
    )
    .add_systems(Update, apply_sim_commands.in_set(SimulationSet::Commands))
    .add_systems(Update, tick_clock.in_set(SimulationSet::Clock))
    .add_systems(
        Update,
        (advance_simulation_frame, smoke_exit)
            .chain()
            .in_set(SimulationSet::Render),
    );

    if let Some(capture) = golden_capture {
        golden::configure_golden_capture(&mut app, capture);
    }

    #[cfg(debug_assertions)]
    if options.golden_capture.is_none() {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .add_systems(Startup, spawn_diag_overlay)
            .add_systems(Update, update_diag_overlay);
    }

    app
}

fn wall_now_t() -> f64 {
    let unix_s = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs_f64(),
        Err(error) => -error.duration().as_secs_f64(),
    };
    t_from_unix_utc(unix_s)
}

#[derive(SystemParam)]
struct SimCommandGate<'w> {
    queue: ResMut<'w, SimCommandQueue>,
    clock: ResMut<'w, SimulationClock>,
    loaded: Option<Res<'w, LoadedCatalog>>,
    camera: Option<ResMut<'w, CameraController>>,
    frame: Res<'w, SimulationFrame>,
    recording: ResMut<'w, CommandRecording>,
    reports: MessageWriter<'w, ClockTickReport>,
    layers: ResMut<'w, LayerState>,
    presentation: ResMut<'w, PresentationState>,
    view_options: ResMut<'w, ViewOptionsState>,
    left_panel: ResMut<'w, left_panel::LeftPanelUiState>,
    navigation: ResMut<'w, NavigationStack>,
    browse: ResMut<'w, search::BrowseUiState>,
    app_settings: ResMut<'w, AppSettings>,
    settings_screen: ResMut<'w, settings::SettingsScreenState>,
    settings_save: ResMut<'w, settings::SettingsSaveRequest>,
    #[cfg(debug_assertions)]
    debug_device_loss: ResMut<'w, DebugDeviceLossRequest>,
}

fn apply_sim_commands(gate: SimCommandGate) {
    let SimCommandGate {
        mut queue,
        mut clock,
        loaded,
        mut camera,
        frame,
        mut recording,
        mut reports,
        mut layers,
        mut presentation,
        mut view_options,
        mut left_panel,
        mut navigation,
        mut browse,
        mut app_settings,
        mut settings_screen,
        mut settings_save,
        #[cfg(debug_assertions)]
        mut debug_device_loss,
    } = gate;
    let commands: Vec<_> = queue.drain().collect();
    let frame_start_t = clock.0.t();
    for command in commands {
        recording.record(frame.0, frame_start_t, command.clone());
        #[cfg(debug_assertions)]
        if matches!(command, SimCommand::SimulateDeviceLoss) {
            request_debug_device_loss(&mut debug_device_loss);
        }
        let layers_before = *layers;
        let presentation_before = *presentation;
        consume_application_command(
            &command,
            loaded.as_deref(),
            layers.bypass_change_detection(),
            presentation.bypass_change_detection(),
            &mut view_options,
            &mut left_panel,
            &mut navigation,
            &mut browse,
            &mut app_settings,
            &mut settings_screen,
            &mut settings_save,
        );
        if *layers != layers_before {
            layers.set_changed();
        }
        if *presentation != presentation_before {
            presentation.set_changed();
        }
        if let (Some(loaded), Some(camera)) = (loaded.as_deref(), camera.as_deref_mut()) {
            let report = consume_sim_command(&command, &mut clock.0, camera, loaded, &navigation);
            write_tick_report(report, &mut reports);
            left_panel::sync_left_panel_selection_state(
                camera,
                loaded,
                &mut left_panel,
                &mut navigation,
            );
        }
    }
}

fn tick_clock(
    time: Res<Time>,
    mut clock: ResMut<SimulationClock>,
    mut tick_advance: ResMut<SimulationTickAdvance>,
    frame: Res<SimulationFrame>,
    mut recording: ResMut<CommandRecording>,
    mut reports: MessageWriter<ClockTickReport>,
) {
    let wall_dt_s = time.delta_secs_f64();
    let wall_now_t = wall_now_t();
    recording.record_frame(frame.0, wall_dt_s, wall_now_t);
    let before = (
        clock.0.t().to_bits(),
        clock.0.rate(),
        clock.0.is_playing(),
        clock.0.is_snapping(),
    );
    let (report, actual_advance) = tick_simulation_clock(
        &mut clock.bypass_change_detection().0,
        wall_dt_s,
        wall_now_t,
    );
    *tick_advance = actual_advance;
    let after = (
        clock.0.t().to_bits(),
        clock.0.rate(),
        clock.0.is_playing(),
        clock.0.is_snapping(),
    );
    if before != after || report != TickReport::default() {
        clock.set_changed();
    }
    write_tick_report(report, &mut reports);
}

fn tick_simulation_clock(
    clock: &mut SimClock,
    wall_dt_s: f64,
    wall_now_t: f64,
) -> (TickReport, SimulationTickAdvance) {
    let before_tick_t = clock.t();
    let report = clock.tick(wall_dt_s, wall_now_t);
    (
        report,
        SimulationTickAdvance::between(before_tick_t, clock.t()),
    )
}

fn write_tick_report(report: TickReport, reports: &mut MessageWriter<ClockTickReport>) {
    if report != TickReport::default() {
        reports.write(ClockTickReport(report));
    }
}

fn propagate_bodies(
    loaded: Option<Res<LoadedCatalog>>,
    clock: Res<SimulationClock>,
    states: Option<ResMut<BodyStates>>,
    mut fault: ResMut<PropagationFault>,
) {
    if !clock.is_changed() {
        return;
    }
    let (Some(loaded), Some(mut states)) = (loaded, states) else {
        return;
    };
    match propagate_into(&loaded.catalog, clock.0.t(), &mut states) {
        Ok(()) => fault.0 = None,
        Err(error) => {
            let message = error.to_string();
            if fault.0.as_deref() != Some(message.as_str()) {
                error!("{message}");
            }
            fault.0 = Some(message);
        }
    }
}

fn advance_camera_focus(
    time: Res<Time>,
    states: Option<Res<BodyStates>>,
    camera: Option<ResMut<CameraController>>,
) {
    let (Some(states), Some(mut camera)) = (states, camera) else {
        return;
    };
    advance_camera_controller(&mut camera, &states, time.delta_secs_f64());
}

pub fn rebase_position(position_km: [f64; 3], focus_km: [f64; 3]) -> Vec3 {
    let relative = [
        (position_km[0] - focus_km[0]) / KM_PER_RENDER_UNIT,
        (position_km[1] - focus_km[1]) / KM_PER_RENDER_UNIT,
        (position_km[2] - focus_km[2]) / KM_PER_RENDER_UNIT,
    ];
    // Ecliptic x-y is Bevy's ground x-z plane; ecliptic z is Bevy up.
    Vec3::new(relative[0] as f32, relative[2] as f32, relative[1] as f32)
}

fn update_focus_and_rebase(
    states: Option<Res<BodyStates>>,
    camera: Option<Res<CameraController>>,
    mut bodies: Query<(&BodyVisual, &mut Transform)>,
) {
    let (Some(states), Some(camera)) = (states, camera) else {
        return;
    };
    let focus_position_km = camera.focus_position_km();
    for (visual, mut transform) in &mut bodies {
        if let Some(state) = states.0.get(visual.index) {
            transform.translation = rebase_position(state.position_km, focus_position_km);
        }
    }
}

fn spawn_body_spheres(
    mut commands: Commands,
    loaded: Option<Res<LoadedCatalog>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Option<Res<AssetServer>>,
) {
    let Some(loaded) = loaded else {
        return;
    };
    let unit_sphere = meshes.add(Sphere::new(1.0));
    for (index, body) in loaded.catalog.bodies.iter().enumerate() {
        let is_star = body.category == Category::Star;
        let scale = (body.radius_km / KM_PER_RENDER_UNIT) as f32;
        let body_entity = commands
            .spawn((
                Name::new(body.name.clone()),
                BodyId(body.id.clone()),
                BodyVisual { index },
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(materials.add(surface_textures::body_material(
                    body,
                    asset_server.as_deref(),
                ))),
                Transform::from_scale(Vec3::splat(scale)),
            ))
            .id();
        if is_star {
            commands
                .entity(body_entity)
                .insert((SunLight, surface_textures::sun_light(body)));
        }
        if body.id == "saturn" {
            surface_textures::spawn_saturn_ring(
                &mut commands,
                body_entity,
                index,
                &mut meshes,
                &mut materials,
                asset_server.as_deref(),
            );
        }
    }
}

fn spawn_camera(
    mut commands: Commands,
    loaded: Option<Res<LoadedCatalog>>,
    camera: Res<CameraController>,
    golden_target: Option<Res<golden::GoldenRenderTarget>>,
) {
    if loaded.is_none() {
        return;
    }
    let translation = camera.render_translation();
    let focus_anchor = commands
        .spawn((
            Name::new("Camera focus anchor"),
            CameraFocusAnchor,
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    let render_target = golden_target
        .as_ref()
        .map_or_else(RenderTarget::default, |target| {
            RenderTarget::Image(target.0.clone().into())
        });
    let mut camera_entity = commands.spawn((
        Camera3d::default(),
        render_target,
        Bloom::NATURAL,
        Projection::Perspective(PerspectiveProjection {
            near: 1.0e-6,
            far: 1.0e9,
            ..default()
        }),
        ChildOf(focus_anchor),
        Transform::from_translation(translation).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    if golden_target.is_some() {
        // Offscreen cameras are not auto-selected for shadow LOD. Pin the
        // origin explicitly so the render path stays warning-free and stable.
        camera_entity.insert((ShadowLodOrigin, IsDefaultUiCamera));
    }
}

fn spawn_catalog_error(mut commands: Commands, failure: Option<Res<CatalogFailure>>) {
    let Some(failure) = failure else {
        return;
    };
    commands.spawn(Camera2d);
    commands.spawn((
        CatalogErrorScreen,
        Text::new(format!(
            "SOLAR-SIM COULD NOT LOAD THE BODY CATALOG\n\n{}\n\nThe simulation was not started.",
            failure.0
        )),
    ));
}

fn update_camera(
    controller: Res<CameraController>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    for mut transform in &mut camera {
        transform.translation = controller.render_translation();
        transform.look_at(Vec3::ZERO, Vec3::Y);
    }
}

fn advance_simulation_frame(mut frame: ResMut<SimulationFrame>) {
    frame.0 += 1;
}

fn smoke_exit(
    mut commands: Commands,
    mut smoke: ResMut<SmokeFrames>,
    adapter: Option<Res<RenderAdapterInfo>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(target) = smoke.target else {
        return;
    };
    if smoke.screenshot_requested || smoke.completed {
        return;
    }
    smoke.seen += 1;
    let warmup_frames = (target / 5).min(60);
    if smoke.started.is_none() && smoke.seen >= warmup_frames {
        smoke.started = Some(Instant::now());
    }
    if smoke.seen >= target {
        let started = match smoke.started {
            Some(started) => started,
            None => Instant::now(),
        };
        let elapsed = started.elapsed().as_secs_f64().max(f64::EPSILON);
        let measured_frames = smoke.seen.saturating_sub(warmup_frames).max(1);
        let fps = measured_frames as f64 / elapsed;
        info!(
            "smoke: completed {} update frames; measured {} after {} warmup frames in {:.3}s ({:.1} fps)",
            smoke.seen, measured_frames, warmup_frames, elapsed, fps
        );
        println!(
            "smoke: completed {} update frames; measured {} after {} warmup frames in {:.3}s ({:.1} fps)",
            smoke.seen, measured_frames, warmup_frames, elapsed, fps
        );
        if smoke.expected_backend.is_some() || smoke.reject_software_adapter {
            let Some(adapter) = adapter else {
                eprintln!("smoke: RenderAdapterInfo is unavailable");
                smoke.completed = true;
                exit.write(AppExit::error());
                return;
            };
            let actual = adapter.backend.to_string();
            let device_type = format!("{:?}", adapter.device_type);
            println!(
                "smoke: render adapter '{}' has device_type {device_type} and uses backend {actual}",
                adapter.name,
            );
            if !software_adapter_allowed(smoke.reject_software_adapter, &device_type) {
                eprintln!(
                    "smoke: rejected software adapter '{}' with device_type {device_type}",
                    adapter.name
                );
                smoke.completed = true;
                exit.write(AppExit::error());
                return;
            }
            if let Some(expected) = smoke.expected_backend {
                if !backend_matches(expected, &actual) {
                    eprintln!(
                        "smoke: expected backend {}, got {actual}",
                        expected.as_str()
                    );
                    smoke.completed = true;
                    exit.write(AppExit::error());
                    return;
                }
                println!("smoke: backend expectation {} passed", expected.as_str());
            }
        }
        if smoke.assert_nonblack {
            println!("smoke: requesting primary window readback");
            smoke.screenshot_requested = true;
            commands
                .spawn(Screenshot::primary_window())
                .observe(assert_window_nonblack_and_exit);
        } else {
            smoke.completed = true;
            exit.write(AppExit::Success);
        }
    }
}

fn assert_window_nonblack_and_exit(
    captured: On<ScreenshotCaptured>,
    mut smoke: ResMut<SmokeFrames>,
    mut exit: MessageWriter<AppExit>,
) {
    let result = captured
        .image
        .clone()
        .try_into_dynamic()
        .map_err(|error| error.to_string())
        .map(|image| image.to_rgb8())
        .and_then(|rgb| nonblack_rgb_dimensions(rgb.width(), rgb.height(), rgb.as_raw()));
    smoke.completed = true;
    match result {
        Ok((width, height)) => {
            println!("smoke: primary window readback {width}x{height} is nonblack");
            exit.write(AppExit::Success);
        }
        Err(error) => {
            eprintln!("smoke: {error}");
            exit.write(AppExit::error());
        }
    }
}

fn backend_matches(expected: ExpectedBackend, actual: &str) -> bool {
    expected.as_str() == actual
}

fn software_adapter_allowed(reject_software_adapter: bool, device_type: &str) -> bool {
    !reject_software_adapter || device_type != "Cpu"
}

fn nonblack_rgb_dimensions(width: u32, height: u32, rgb: &[u8]) -> Result<(u32, u32), String> {
    if rgb.iter().any(|channel| *channel != 0) {
        Ok((width, height))
    } else {
        Err("primary window readback is entirely black".into())
    }
}

#[cfg(debug_assertions)]
fn spawn_diag_overlay(mut commands: Commands, theme: Res<UiTheme>, asset_server: Res<AssetServer>) {
    commands.spawn((
        Text::new("fps: --"),
        TextFont {
            font: asset_server.load(INTER_FONT_ASSET).into(),
            font_size: theme.type_scale.caption_px.into(),
            ..default()
        },
        TextColor(theme.colors.text_muted.color()),
        Node {
            position_type: PositionType::Absolute,
            right: px(theme.spacing.sm_px),
            bottom: px(TIME_BAR_HEIGHT_PX + theme.spacing.sm_px),
            ..default()
        },
        GlobalZIndex(110),
        AccessibleLabel::new("Frame rate diagnostic"),
        layers::HudSurface,
        DiagText,
    ));
}

#[cfg(debug_assertions)]
fn update_diag_overlay(
    diagnostics: Res<DiagnosticsStore>,
    mut text: Query<&mut Text, With<DiagText>>,
) {
    if let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
    {
        for mut text in &mut text {
            **text = format!("fps: {fps:.0}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::time::{t_from_jd_tdb, RateIndex, StartMode, T_MAX_S};

    const REAL_CATALOG: &str = include_str!("../../../assets/catalog.ron");

    fn catalog() -> Catalog {
        load_catalog_text(REAL_CATALOG).expect("committed catalog must load")
    }

    #[test]
    fn smoke_cli_accepts_only_reviewed_backends_and_requires_smoke_mode() {
        for (name, expected) in [
            ("metal", ExpectedBackend::Metal),
            ("dx12", ExpectedBackend::Dx12),
            ("vulkan", ExpectedBackend::Vulkan),
        ] {
            let args = [
                "solar-sim",
                "--smoke",
                "60",
                "--expect-backend",
                name,
                "--reject-software-adapter",
                "--assert-nonblack",
            ]
            .map(str::to_owned);
            let options = RunOptions::from_args(&args).unwrap();
            assert_eq!(options.smoke_frames, Some(60));
            assert_eq!(options.expected_backend, Some(expected));
            assert!(options.reject_software_adapter);
            assert!(options.assert_nonblack);
        }

        let invalid = ["solar-sim", "--smoke", "60", "--expect-backend", "gl"].map(str::to_owned);
        assert!(RunOptions::from_args(&invalid).is_err());
        let missing_smoke = ["solar-sim", "--assert-nonblack"].map(str::to_owned);
        assert!(RunOptions::from_args(&missing_smoke).is_err());
        let unscoped_rejection = ["solar-sim", "--reject-software-adapter"].map(str::to_owned);
        assert!(RunOptions::from_args(&unscoped_rejection).is_err());
    }

    #[test]
    fn reset_settings_cli_is_explicit_and_off_by_default() {
        assert!(!RunOptions::default().reset_settings);
        let args = ["solar-sim", "--reset-settings"].map(str::to_owned);
        assert!(RunOptions::from_args(&args).unwrap().reset_settings);
    }

    #[test]
    fn smoke_backend_and_nonblack_checks_reject_mismatches_and_black_frames() {
        assert!(backend_matches(ExpectedBackend::Metal, "metal"));
        assert!(backend_matches(ExpectedBackend::Dx12, "dx12"));
        assert!(backend_matches(ExpectedBackend::Vulkan, "vulkan"));
        assert!(!backend_matches(ExpectedBackend::Metal, "vulkan"));

        assert_eq!(
            nonblack_rgb_dimensions(2, 1, &[0, 0, 0, 0, 1, 0]),
            Ok((2, 1))
        );
        assert_eq!(
            nonblack_rgb_dimensions(1, 1, &[0, 0, 0]),
            Err("primary window readback is entirely black".into())
        );
    }

    #[test]
    fn smoke_software_adapter_check_rejects_cpu_only_when_requested() {
        assert!(software_adapter_allowed(true, "IntegratedGpu"));
        assert!(software_adapter_allowed(true, "DiscreteGpu"));
        assert!(software_adapter_allowed(true, "VirtualGpu"));
        assert!(!software_adapter_allowed(true, "Cpu"));
        assert!(software_adapter_allowed(false, "Cpu"));
    }

    #[test]
    fn composed_io_state_matches_direct_core_reference_to_last_bit() {
        let catalog = catalog();
        let index = catalog.id_index();
        let t_s = t_from_jd_tdb(2_461_042.0);
        let states = propagate_catalog(&catalog, t_s).unwrap();
        let sun = *index.get("sun").unwrap();
        let jupiter = *index.get("jupiter").unwrap();
        let io = *index.get("io").unwrap();

        let jupiter_relative = state_at(
            catalog.bodies[jupiter].orbit.as_ref().unwrap(),
            catalog.bodies[sun].gm_km3_s2.unwrap(),
            t_s,
        )
        .unwrap();
        let io_relative = state_at(
            catalog.bodies[io].orbit.as_ref().unwrap(),
            catalog.bodies[jupiter].gm_km3_s2.unwrap(),
            t_s,
        )
        .unwrap();
        let expected = StateVector {
            position_km: add_f64(jupiter_relative.position_km, io_relative.position_km),
            velocity_km_s: add_f64(jupiter_relative.velocity_km_s, io_relative.velocity_km_s),
        };
        assert_eq!(states.0[io], expected);
    }

    #[test]
    fn planet_states_match_direct_core_output_bit_for_bit_at_catalog_epoch() {
        let catalog = catalog();
        let index = catalog.id_index();
        let t_s = t_from_jd_tdb(2_461_042.0);
        let states = propagate_catalog(&catalog, t_s).unwrap();
        let sun = &catalog.bodies[*index.get("sun").unwrap()];
        for id in [
            "mercury", "venus", "earth", "mars", "jupiter", "saturn", "uranus", "neptune",
        ] {
            let body_index = *index.get(id).unwrap();
            let direct = state_at(
                catalog.bodies[body_index].orbit.as_ref().unwrap(),
                sun.gm_km3_s2.unwrap(),
                t_s,
            )
            .unwrap();
            assert_eq!(states.0[body_index], direct, "{id}");
        }
    }

    #[test]
    fn tick_advance_reports_signed_motion_pause_and_the_range_pin_exactly() {
        let frame_s = 1.0 / 60.0;
        let mut clock = SimClock::new(StartMode::Live, 0.0);
        clock.set_rate(RateIndex::MIN);

        let (_report, reverse) = tick_simulation_clock(&mut clock, frame_s, 0.0);
        assert_eq!(
            reverse.seconds(),
            RateIndex::MIN.seconds_per_second() * frame_s
        );

        clock.pause();
        let (_report, paused) = tick_simulation_clock(&mut clock, frame_s, 0.0);
        assert_eq!(paused.seconds(), 0.0);

        clock.set_t(T_MAX_S);
        clock.set_rate(RateIndex::MAX);
        clock.play();
        let (report, pinned) = tick_simulation_clock(&mut clock, frame_s, 0.0);
        assert_eq!(report.clamped, Some(sim_core::time::RangeEdge::AtMax));
        assert_eq!(clock.t(), T_MAX_S);
        assert_eq!(pinned.seconds(), 0.0);
    }

    #[test]
    fn live_snap_reports_its_eased_actual_advance_instead_of_the_stale_rate() {
        let frame_s = 1.0 / 60.0;
        let wall_now_t = 1.0e9;
        let mut clock = SimClock::new(StartMode::Live, 0.0);
        clock.set_rate(RateIndex::MIN);
        clock.snap_to_live();

        let (_report, advance) = tick_simulation_clock(&mut clock, frame_s, wall_now_t);
        let expected = wall_now_t * (1.0 - (-frame_s / 0.12).exp());
        assert!((advance.seconds() - expected).abs() <= expected * 1.0e-15);
        assert!(advance.seconds() > 0.0);
        assert_ne!(
            advance.seconds(),
            RateIndex::MIN.seconds_per_second() * frame_s
        );
    }

    #[test]
    fn command_time_jump_is_excluded_from_the_following_tick_advance() {
        let frame_s = 1.0 / 60.0;
        let previous_frame_t = 100.0;
        let command_target_t = 1.0e9;
        let mut clock = SimClock::new(StartMode::Live, previous_frame_t);

        // This models `SetTime` in the Commands set. `tick_simulation_clock`
        // samples only after that boundary, exactly as `tick_clock` does.
        clock.set_t(command_target_t);
        let (_report, advance) = tick_simulation_clock(&mut clock, frame_s, command_target_t);
        assert!((advance.seconds() - frame_s).abs() < 1.0e-6);
        assert!(advance.seconds() < (command_target_t - previous_frame_t + frame_s).abs() * 1.0e-6);
    }

    #[test]
    fn focus_change_preserves_relative_positions_at_sedna_scale() {
        let a = [1.4e11, -8.0e10, 2.0e9];
        let b = [a[0] + 1_234.0, a[1] - 5_678.0, a[2] + 9_012.0];
        let relative_from_a = rebase_position(b, a) - rebase_position(a, a);
        let relative_from_b = rebase_position(b, b) - rebase_position(a, b);
        let error = (relative_from_a - relative_from_b).abs().max_element();
        assert!(
            error <= f32::EPSILON * relative_from_a.length().max(1.0),
            "relative render-space position changed by {error}"
        );
    }

    #[test]
    fn mercury_and_sedna_focus_points_rebase_to_exact_origin() {
        let catalog = catalog();
        let index = catalog.id_index();
        let states = propagate_catalog(&catalog, t_from_jd_tdb(2_461_042.0)).unwrap();
        for id in ["mercury", "sedna"] {
            let position = states.0[*index.get(id).unwrap()].position_km;
            assert_eq!(rebase_position(position, position), Vec3::ZERO, "{id}");

            let mut one_km_away = position;
            one_km_away[0] += 1.0;
            let rebased = rebase_position(one_km_away, position);
            assert!((rebased.x - 0.001).abs() <= 2.0e-8, "{id}: {rebased:?}");
        }
    }

    #[test]
    fn corrupt_catalog_is_rejected_without_panicking_and_has_error_screen() {
        let result = std::panic::catch_unwind(|| load_catalog_text("not valid RON"));
        assert!(result.is_ok(), "loader panicked on corrupt input");
        assert!(result.unwrap().is_err());

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(CatalogFailure("deliberate corrupt fixture".into()))
            .add_systems(Startup, spawn_catalog_error);
        app.update();
        let mut query = app.world_mut().query::<(&CatalogErrorScreen, &Text)>();
        let screens: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(screens.len(), 1);
        assert!(screens[0].1.contains("deliberate corrupt fixture"));
    }

    #[test]
    fn real_catalog_spawns_all_66_true_radius_spheres() {
        let catalog = catalog();
        assert!(catalog
            .lint()
            .iter()
            .all(|lint| !lint.contains("no texture assigned")));
        let loaded = LoadedCatalog::new(catalog);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(loaded)
            .add_systems(Startup, spawn_body_spheres);
        app.update();

        let mut query = app
            .world_mut()
            .query::<(&BodyVisual, &BodyId, &Transform)>();
        let bodies: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(bodies.len(), 66);
        let mercury = bodies.iter().find(|(_, id, _)| id.0 == "mercury").unwrap();
        assert_eq!(mercury.2.scale, Vec3::splat(2.4397));

        let mut material_query = app.world_mut().query::<(
            &BodyId,
            &MeshMaterial3d<StandardMaterial>,
            Option<&SunLight>,
            Option<&PointLight>,
        )>();
        let render_data: Vec<_> = material_query
            .iter(app.world())
            .map(|(id, material, sun_light, point_light)| {
                (
                    id.0.clone(),
                    material.0.clone(),
                    sun_light.is_some(),
                    point_light.cloned(),
                )
            })
            .collect();
        let sun = render_data.iter().find(|data| data.0 == "sun").unwrap();
        let mercury = render_data.iter().find(|data| data.0 == "mercury").unwrap();
        assert!(sun.2, "Sun must carry the light marker");
        let light = sun.3.as_ref().expect("Sun must carry a point light");
        assert_eq!(light.intensity, SUN_LIGHT_INTENSITY_LUMENS);
        assert_eq!(light.range, SUN_LIGHT_RANGE_UNITS);
        assert_eq!(render_data.iter().filter(|data| data.2).count(), 1);
        let saturn_entity = app
            .world_mut()
            .query::<(Entity, &BodyId)>()
            .iter(app.world())
            .find_map(|(entity, id)| (id.0 == "saturn").then_some(entity))
            .unwrap();
        let rings: Vec<_> = app
            .world_mut()
            .query::<(
                &surface_textures::SaturnRing,
                &ChildOf,
                &MeshMaterial3d<StandardMaterial>,
            )>()
            .iter(app.world())
            .map(|(ring, parent, material)| (ring.body_index, parent.0, material.0.clone()))
            .collect();
        assert_eq!(rings.len(), 1);
        let saturn_index = app
            .world()
            .resource::<LoadedCatalog>()
            .index_of("saturn")
            .unwrap();
        assert_eq!(rings[0].0, saturn_index);
        assert_eq!(rings[0].1, saturn_entity);

        let materials = app.world().resource::<Assets<StandardMaterial>>();
        let sun_material = materials.get(&sun.1).unwrap();
        let mercury_material = materials.get(&mercury.1).unwrap();
        assert!(sun_material.unlit);
        assert_ne!(sun_material.emissive, LinearRgba::BLACK);
        assert!(!mercury_material.unlit);
        assert_eq!(mercury_material.emissive, LinearRgba::BLACK);
        assert!(mercury_material.base_color_texture.is_none());
        assert_eq!(
            mercury_material.base_color,
            Color::srgb(158.0 / 255.0, 158.0 / 255.0, 158.0 / 255.0),
            "catalog color remains the complete headless/missing-asset fallback"
        );

        let ring_material = materials.get(&rings[0].2).unwrap();
        assert_eq!(ring_material.alpha_mode, AlphaMode::Blend);
        assert!(ring_material.double_sided);
    }

    #[test]
    fn camera_is_parented_to_the_focus_anchor_with_extreme_zoom_clip_planes() {
        let catalog = catalog();
        let states = propagate_catalog(&catalog, t_from_jd_tdb(2_461_042.0)).unwrap();
        let loaded = LoadedCatalog::new(catalog);
        let sun = loaded.index_of("sun").unwrap();
        let camera = CameraController::new(
            sun,
            states.0[sun].position_km,
            DEFAULT_CAMERA_DISTANCE_UNITS,
        );
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(loaded)
            .insert_resource(camera)
            .add_systems(Startup, spawn_camera);
        app.update();

        let mut query = app
            .world_mut()
            .query::<(&Camera3d, &ChildOf, &Projection, &Bloom)>();
        let cameras: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(cameras.len(), 1);
        let (_, child_of, projection, bloom) = cameras[0];
        assert!(
            app.world().get::<CameraFocusAnchor>(child_of.0).is_some(),
            "camera parent is not the moving focus anchor"
        );
        let Projection::Perspective(perspective) = projection else {
            panic!("WP5 camera must be perspective");
        };
        assert_eq!(perspective.near, 1.0e-6);
        assert_eq!(perspective.far, 1.0e9);
        assert_eq!(bloom.intensity, Bloom::NATURAL.intensity);
    }

    #[test]
    fn golden_camera_targets_the_offscreen_image_and_owns_the_ui() {
        let catalog = catalog();
        let states = propagate_catalog(&catalog, t_from_jd_tdb(2_461_042.0)).unwrap();
        let loaded = LoadedCatalog::new(catalog);
        let sun = loaded.index_of("sun").unwrap();
        let camera = CameraController::new(
            sun,
            states.0[sun].position_km,
            DEFAULT_CAMERA_DISTANCE_UNITS,
        );
        let target = Handle::<Image>::default();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(loaded)
            .insert_resource(camera)
            .insert_resource(golden::GoldenRenderTarget(target.clone()))
            .add_systems(Startup, spawn_camera);
        app.update();

        let mut query = app.world_mut().query_filtered::<
            (&RenderTarget, &ShadowLodOrigin, &IsDefaultUiCamera),
            With<Camera3d>,
        >();
        let cameras: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(cameras.len(), 1);
        assert_eq!(cameras[0].0.as_image(), Some(&target));
    }

    #[derive(Resource, Default)]
    struct FrameTrace(Vec<SimulationSet>);

    fn mark_input(mut trace: ResMut<FrameTrace>) {
        trace.0.push(SimulationSet::Input);
    }
    fn mark_commands(mut trace: ResMut<FrameTrace>) {
        trace.0.push(SimulationSet::Commands);
    }
    fn mark_clock(mut trace: ResMut<FrameTrace>) {
        trace.0.push(SimulationSet::Clock);
    }
    fn mark_propagation(mut trace: ResMut<FrameTrace>) {
        trace.0.push(SimulationSet::Propagation);
    }
    fn mark_origin(mut trace: ResMut<FrameTrace>) {
        trace.0.push(SimulationSet::Origin);
    }
    fn mark_camera(mut trace: ResMut<FrameTrace>) {
        trace.0.push(SimulationSet::Camera);
    }
    fn mark_render(mut trace: ResMut<FrameTrace>) {
        trace.0.push(SimulationSet::Render);
    }

    #[test]
    fn frame_sets_run_in_declared_order() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<FrameTrace>();
        configure_frame_flow(&mut app);
        app.add_systems(Update, mark_input.in_set(SimulationSet::Input))
            .add_systems(Update, mark_commands.in_set(SimulationSet::Commands))
            .add_systems(Update, mark_clock.in_set(SimulationSet::Clock))
            .add_systems(Update, mark_propagation.in_set(SimulationSet::Propagation))
            .add_systems(Update, mark_origin.in_set(SimulationSet::Origin))
            .add_systems(Update, mark_camera.in_set(SimulationSet::Camera))
            .add_systems(Update, mark_render.in_set(SimulationSet::Render));
        app.update();
        assert_eq!(
            app.world().resource::<FrameTrace>().0,
            vec![
                SimulationSet::Input,
                SimulationSet::Commands,
                SimulationSet::Clock,
                SimulationSet::Propagation,
                SimulationSet::Origin,
                SimulationSet::Camera,
                SimulationSet::Render,
            ]
        );
    }

    #[test]
    fn command_gate_reduces_application_state_before_catalog_loads() {
        let mut queue = SimCommandQueue::default();
        for command in [
            SimCommand::SetLayerVisibility {
                layer: LayerId::Labels,
                visible: false,
            },
            SimCommand::ToggleFullscreen,
            SimCommand::OpenSettings,
            SimCommand::SetBrowseOpen(true),
        ] {
            queue.push(command);
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(queue)
            .insert_resource(SimulationClock(SimClock::new(StartMode::default(), 0.0)))
            .init_resource::<SimulationFrame>()
            .init_resource::<CommandRecording>()
            .init_resource::<LayerState>()
            .init_resource::<PresentationState>()
            .init_resource::<ViewOptionsState>()
            .init_resource::<left_panel::LeftPanelUiState>()
            .init_resource::<NavigationStack>()
            .init_resource::<search::BrowseUiState>()
            .init_resource::<AppSettings>()
            .init_resource::<settings::SettingsScreenState>()
            .init_resource::<settings::SettingsSaveRequest>()
            .add_message::<ClockTickReport>()
            .add_systems(Update, apply_sim_commands);
        #[cfg(debug_assertions)]
        app.init_resource::<DebugDeviceLossRequest>();

        app.update();

        assert!(app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .next()
            .is_none());
        assert_eq!(
            app.world()
                .resource::<CommandRecording>()
                .stream()
                .entries()
                .len(),
            4
        );
        assert!(!app
            .world()
            .resource::<LayerState>()
            .is_visible(LayerId::Labels));
        let presentation = *app.world().resource::<PresentationState>();
        assert!(presentation.is_fullscreen());
        assert!(!presentation.is_settings_open());
        assert!(app.world().resource::<search::BrowseUiState>().is_open());
        let settings = app.world().resource::<AppSettings>();
        assert_eq!(
            settings.display_mode,
            DisplayModeSetting::BorderlessFullscreen
        );
        assert!(!settings.layers.labels);
    }

    #[test]
    fn desktop_command_gate_converges_navigation_after_each_ordered_command() {
        let catalog = catalog();
        let states = propagate_catalog(&catalog, t_from_jd_tdb(2_461_042.0)).unwrap();
        let loaded = LoadedCatalog::new(catalog);
        let sun = loaded.index_of("sun").unwrap();
        let jupiter = loaded.index_of("jupiter").unwrap();
        let io = loaded.index_of("io").unwrap();
        let camera = CameraController::new(
            sun,
            states.0[sun].position_km,
            DEFAULT_CAMERA_DISTANCE_UNITS,
        );

        let mut queue = SimCommandQueue::default();
        queue.push(SimCommand::TravelToBody("jupiter".into()));
        queue.push(SimCommand::SetLeftPanelTab(LeftPanelTab::Collection));
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(queue)
            .insert_resource(SimulationClock(SimClock::new(StartMode::default(), 0.0)))
            .insert_resource(loaded)
            .insert_resource(camera)
            .init_resource::<SimulationFrame>()
            .init_resource::<CommandRecording>()
            .init_resource::<LayerState>()
            .init_resource::<PresentationState>()
            .init_resource::<ViewOptionsState>()
            .init_resource::<left_panel::LeftPanelUiState>()
            .init_resource::<NavigationStack>()
            .init_resource::<search::BrowseUiState>()
            .init_resource::<AppSettings>()
            .init_resource::<settings::SettingsScreenState>()
            .init_resource::<settings::SettingsSaveRequest>()
            .add_message::<ClockTickReport>()
            .add_systems(Update, apply_sim_commands);
        #[cfg(debug_assertions)]
        app.init_resource::<DebugDeviceLossRequest>();

        app.update();
        assert_eq!(
            app.world()
                .resource::<CameraController>()
                .selected_body_index(),
            jupiter
        );
        assert_eq!(
            left_panel::left_panel_replay_state(
                app.world().resource::<left_panel::LeftPanelUiState>()
            )
            .1,
            LeftPanelTab::Collection
        );
        assert_eq!(
            app.world().resource::<NavigationStack>().label(),
            "Solar System › Jupiter › Moons"
        );

        app.world_mut()
            .resource_mut::<SimCommandQueue>()
            .push(SimCommand::TravelToBody("io".into()));
        app.update();
        assert_eq!(
            app.world()
                .resource::<CameraController>()
                .selected_body_index(),
            io
        );
        assert_eq!(
            app.world().resource::<NavigationStack>().label(),
            "Solar System › Jupiter › Io"
        );

        {
            let mut queue = app.world_mut().resource_mut::<SimCommandQueue>();
            queue.push(SimCommand::NavigateBreadcrumb {
                depth: 1,
                target_id: "jupiter".into(),
            });
            queue.push(SimCommand::SetLeftPanelTab(LeftPanelTab::ViewOptions));
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<CameraController>()
                .selected_body_index(),
            jupiter
        );
        assert_eq!(
            left_panel::left_panel_replay_state(
                app.world().resource::<left_panel::LeftPanelUiState>()
            )
            .1,
            LeftPanelTab::ViewOptions
        );
        assert_eq!(
            app.world().resource::<NavigationStack>().label(),
            "Solar System › Jupiter"
        );
    }

    #[test]
    fn raw_device_input_is_confined_to_the_intent_module() {
        let raw_input_names = [
            ["Button", "Input"].concat(),
            ["Mouse", "Motion"].concat(),
            ["Mouse", "Wheel"].concat(),
        ];
        let intent_source = include_str!("input_intent.rs");
        for name in &raw_input_names {
            assert!(intent_source.contains(name), "intent module lacks {name}");
        }
        for (path, source) in [
            ("lib.rs", include_str!("lib.rs")),
            ("control.rs", include_str!("control.rs")),
            ("main.rs", include_str!("main.rs")),
        ] {
            for name in &raw_input_names {
                assert!(
                    !source.contains(name),
                    "raw input type {name} escaped into {path}"
                );
            }
        }
        assert_eq!(
            include_str!("control.rs")
                .matches("USER_STATE_MUTATION_GATE")
                .count(),
            1
        );
    }
}
