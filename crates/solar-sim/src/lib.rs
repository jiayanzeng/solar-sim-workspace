//! WP4–WP10 — simulation rendering, camera control, reusable HUD, and contextual UI.
//!
//! `sim-core` remains the f64 source of truth. This crate owns filesystem
//! loading, parent-to-heliocentric composition, the one f64→f32 render rebase,
//! explicit frame-flow ordering, WP5's single command-consumer boundary, and
//! WP6's retained orbit rendering. Raw device events are isolated in
//! `input_intent`; camera/control state is private to `control`, which also
//! supplies the headless replay gate.

mod control;
mod input_intent;
mod labels;
mod left_panel;
mod orbit_lines;
mod time_bar;
mod ui_kit;

pub use control::{
    replay_headless, CameraController, CommandRecording, HeadlessSimulation, ReplayParseError,
    ReplayRunError, ReplayStream, SimCommand, StampedCommand,
};
pub use labels::{
    declutter_labels, moon_label_is_contextually_visible, ray_sphere_hit_distance, BodyLabel,
    DeclutterCandidate, LabelPriority, LabelsPlugin, ScreenRect, SelectionPlugin,
};
pub use left_panel::{
    body_info_view_model, moon_collections, rendered_body_radius_units, BodyInfoViewModel,
    BodyLinkViewModel, BodySizeScale, DescriptionViewModel, InfoViewModelError, LeftPanelPlugin,
    LeftPanelRoot, LeftPanelTab, MoonCollectionViewModel, MoonVisibilityMode,
    OrbitalPeriodViewModel, ViewOptionsSnapshot, ViewOptionsState,
};
pub use orbit_lines::{
    orbit_vertex_count, sample_orbit, OrbitLineBrightness, OrbitLinesPlugin, OrbitPath,
    HYPERBOLIC_HALF_SPAN_S, MAX_ORBIT_VERTICES, MIN_ORBIT_VERTICES,
};
pub use time_bar::{
    commit_time_edit, live_chip_active, rate_for_slider_value, slider_value_for_rate,
    toasts_for_tick_report, TimeBarPlugin, TimeBarRoot, TimeEditField, TimeEditOutcome,
    TimeToastKind, TIME_BAR_HEIGHT_PX,
};
pub use ui_kit::{
    checkbox_row, chip, panel, section_header, slider, tab_bar, toast, top_bar, BreadcrumbText,
    NavigationItem, NavigationStack, SearchPlaceholder, TopBarRoot, UiColorToken, UiColors,
    UiKitPlugin, UiSpacing, UiTheme, UiTypeScale, WidgetKind, WidgetRoot, WidgetSpec,
    WidgetVisualState, BREADCRUMB_SEPARATOR, INTER_FONT_ASSET, TOP_BAR_HEIGHT_PX,
};
#[cfg(debug_assertions)]
pub use ui_kit::{WidgetGalleryCell, WidgetGalleryRoot};

#[cfg(debug_assertions)]
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use control::{
    advance_camera_controller, consume_sim_command, framing_distance_units,
    full_system_framing_distance_units, SimCommandQueue, SimulationFrame,
};
use input_intent::InputIntentPlugin;
use sim_core::catalog::{Catalog, CatalogError, Category};
use sim_core::kepler::{state_at, KeplerError, StateVector};
use sim_core::time::{t_from_unix_utc, SimClock, StartMode, TickReport};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const DEFAULT_CATALOG_PATH: &str = "assets/catalog.ron";
const DEFAULT_BEVY_ASSET_ROOT: &str = "../../assets";
pub const KM_PER_RENDER_UNIT: f64 = 1_000.0;
const DEFAULT_SMOKE_FRAMES: u32 = 60;
pub const DEFAULT_CAMERA_DISTANCE_UNITS: f64 = 250_000.0;

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub catalog_path: PathBuf,
    pub smoke_frames: Option<u32>,
    pub initial_focus_id: Option<String>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            catalog_path: PathBuf::from(DEFAULT_CATALOG_PATH),
            smoke_frames: None,
            initial_focus_id: None,
        }
    }
}

impl RunOptions {
    pub fn from_args(args: &[String]) -> Self {
        let mut options = Self::default();
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
                _ => {}
            }
            i += 1;
        }
        options
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

#[derive(Message, Debug, Clone, Copy)]
struct ClockTickReport(TickReport);

#[derive(Resource, Default)]
struct PropagationFault(Option<String>);

#[derive(Resource)]
struct SmokeFrames {
    target: Option<u32>,
    seen: u32,
    started: Option<Instant>,
}

impl SmokeFrames {
    fn new(target: Option<u32>) -> Self {
        Self {
            target,
            seen: 0,
            started: None,
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
    let options = RunOptions::from_args(&args);
    let catalog = load_catalog_from_path(&options.catalog_path);
    let mut app = build_app(options, catalog);
    app.run();
}

pub fn build_app(options: RunOptions, catalog: Result<Catalog, CatalogLoadError>) -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: DEFAULT_BEVY_ASSET_ROOT.to_string(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Solar Sim — WP10 Left panel".into(),
                    ..default()
                }),
                ..default()
            }),
    );
    configure_frame_flow(&mut app);

    let wall_now_t = wall_now_t();
    app.insert_resource(SimulationClock(SimClock::new(
        StartMode::default(),
        wall_now_t,
    )))
    .insert_resource(SimCommandQueue::default())
    .insert_resource(CommandRecording::default())
    .insert_resource(SimulationFrame::default())
    .insert_resource(PropagationFault::default())
    .insert_resource(SmokeFrames::new(options.smoke_frames))
    .add_message::<ClockTickReport>();

    match catalog.and_then(|catalog| {
        let t_s = SimClock::new(StartMode::default(), wall_now_t).t();
        let states = propagate_catalog(&catalog, t_s).map_err(CatalogLoadError::Propagation)?;
        Ok((catalog, states))
    }) {
        Ok((catalog, states)) => {
            let loaded = LoadedCatalog::new(catalog);
            let requested_focus = options
                .initial_focus_id
                .as_deref()
                .and_then(|id| loaded.index_of(id));
            let focus_index = requested_focus
                .or_else(|| loaded.index_of("sun"))
                .map_or(0, |index| index);
            let distance = if requested_focus.is_some() {
                framing_distance_units(&loaded, focus_index)
            } else {
                full_system_framing_distance_units(&loaded)
            };
            app.insert_resource(CameraController::new(
                focus_index,
                states.0[focus_index].position_km,
                distance,
            ))
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
        OrbitLinesPlugin,
        UiKitPlugin,
        TimeBarPlugin,
        LabelsPlugin,
        SelectionPlugin,
        LeftPanelPlugin,
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

    #[cfg(debug_assertions)]
    app.add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_systems(Startup, spawn_diag_overlay)
        .add_systems(Update, update_diag_overlay);

    app
}

fn wall_now_t() -> f64 {
    let unix_s = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs_f64(),
        Err(error) => -error.duration().as_secs_f64(),
    };
    t_from_unix_utc(unix_s)
}

fn apply_sim_commands(
    mut queue: ResMut<SimCommandQueue>,
    mut clock: ResMut<SimulationClock>,
    loaded: Option<Res<LoadedCatalog>>,
    camera: Option<ResMut<CameraController>>,
    frame: Res<SimulationFrame>,
    mut recording: ResMut<CommandRecording>,
    mut reports: MessageWriter<ClockTickReport>,
) {
    let (Some(loaded), Some(mut camera)) = (loaded, camera) else {
        queue.drain().for_each(drop);
        return;
    };
    let commands: Vec<_> = queue.drain().collect();
    for command in commands {
        recording.record(frame.0, clock.0.t(), command.clone());
        let report = consume_sim_command(&command, &mut clock.0, &mut camera, &loaded);
        write_tick_report(report, &mut reports);
    }
}

fn tick_clock(
    time: Res<Time>,
    mut clock: ResMut<SimulationClock>,
    mut reports: MessageWriter<ClockTickReport>,
) {
    let report = clock.0.tick(time.delta_secs_f64(), wall_now_t());
    write_tick_report(report, &mut reports);
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
) {
    let Some(loaded) = loaded else {
        return;
    };
    let unit_sphere = meshes.add(Sphere::new(1.0));
    for (index, body) in loaded.catalog.bodies.iter().enumerate() {
        let (r, g, b) = body.color_srgb;
        let color = Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
        let emissive = if body.category == Category::Star {
            LinearRgba::rgb(
                r as f32 / 255.0 * 4.0,
                g as f32 / 255.0 * 4.0,
                b as f32 / 255.0 * 4.0,
            )
        } else {
            LinearRgba::BLACK
        };
        let scale = (body.radius_km / KM_PER_RENDER_UNIT) as f32;
        commands.spawn((
            Name::new(body.name.clone()),
            BodyId(body.id.clone()),
            BodyVisual { index },
            Mesh3d(unit_sphere.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                emissive,
                unlit: true,
                ..default()
            })),
            Transform::from_scale(Vec3::splat(scale)),
        ));
    }
}

fn spawn_camera(
    mut commands: Commands,
    loaded: Option<Res<LoadedCatalog>>,
    camera: Res<CameraController>,
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
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            near: 1.0e-6,
            far: 1.0e9,
            ..default()
        }),
        ChildOf(focus_anchor),
        Transform::from_translation(translation).looking_at(Vec3::ZERO, Vec3::Y),
    ));
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

fn smoke_exit(mut smoke: ResMut<SmokeFrames>, mut exit: MessageWriter<AppExit>) {
    let Some(target) = smoke.target else {
        return;
    };
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
            "smoke: rendered {} frames; measured {} after {} warmup frames in {:.3}s ({:.1} fps)",
            smoke.seen, measured_frames, warmup_frames, elapsed, fps
        );
        println!(
            "smoke: rendered {} frames; measured {} after {} warmup frames in {:.3}s ({:.1} fps)",
            smoke.seen, measured_frames, warmup_frames, elapsed, fps
        );
        exit.write(AppExit::Success);
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
    use sim_core::time::t_from_jd_tdb;

    const REAL_CATALOG: &str = include_str!("../../../assets/catalog.ron");

    fn catalog() -> Catalog {
        load_catalog_text(REAL_CATALOG).expect("committed catalog must load")
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
            .query::<(&Camera3d, &ChildOf, &Projection)>();
        let cameras: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(cameras.len(), 1);
        let (_, child_of, projection) = cameras[0];
        assert!(
            app.world().get::<CameraFocusAnchor>(child_of.0).is_some(),
            "camera parent is not the moving focus anchor"
        );
        let Projection::Perspective(perspective) = projection else {
            panic!("WP5 camera must be perspective");
        };
        assert_eq!(perspective.near, 1.0e-6);
        assert_eq!(perspective.far, 1.0e9);
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
