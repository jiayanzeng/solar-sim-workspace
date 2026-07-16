//! WP5/WP8/WP9/WP14 — command consumption, camera travel, time, and replay.
//!
//! This module is the user-state mutation boundary. `CameraController` keeps
//! its fields private, and `consume_sim_command` is the only function that
//! applies user intent to simulation state. Per-frame clock, propagation, and
//! moving-focus evolution are deterministic updates driven by explicit inputs.

use crate::{
    propagate_into, AppSettings, BodySizeScale, BodyStates, LayerId, LayerState, LeftPanelTab,
    LoadedCatalog, MoonVisibilityMode, PresentationState, PropagationError,
    DEFAULT_CAMERA_DISTANCE_UNITS, KM_PER_RENDER_UNIT,
};
use bevy::prelude::{Resource, Vec3};
use sim_core::catalog::{Catalog, CatalogError, Category};
use sim_core::time::{RateIndex, SimClock, StartMode, TickReport};
use std::fmt;

const TRAVEL_DURATION_S: f64 = 1.25;
const MIN_PITCH_RAD: f64 = -1.5;
const MAX_PITCH_RAD: f64 = 1.5;
const ORBIT_RADIANS_PER_PIXEL: f64 = 0.005;

/// Stable, serializable user actions. Body references are catalog ids, never
/// display names, so command recordings survive localization and UI changes.
#[derive(Debug, Clone, PartialEq)]
pub enum SimCommand {
    SelectBody(String),
    TravelToBody(String),
    Orbit {
        delta_yaw: f64,
        delta_pitch: f64,
    },
    Dolly {
        delta: f64,
    },
    SetTime(f64),
    SetRate(RateIndex),
    StepRate(i8),
    Play,
    Pause,
    TogglePlay,
    SnapToLive,
    SetLayerVisibility {
        layer: LayerId,
        visible: bool,
    },
    SetBodySize(BodySizeScale),
    SetMoonVisibility {
        system_id: String,
        mode: MoonVisibilityMode,
    },
    SetLocalOrbitVisibility {
        body_id: String,
        visible: bool,
    },
    SetLeftPanelCollapsed(bool),
    SetLeftPanelTab(LeftPanelTab),
    SetBrowseOpen(bool),
    SetBrowseColumnExpanded {
        column: u8,
        expanded: bool,
    },
    ApplySettings(Box<AppSettings>),
    RestorePresentationDefaults,
    NavigateBreadcrumb {
        depth: usize,
        target_id: String,
    },
    ToggleFullscreen,
    OpenSettings,
    CloseSettings,
    /// Debug-only input requests a real renderer device-loss cycle. The
    /// variant stays stable in recordings even though release input never emits it.
    SimulateDeviceLoss,
}

#[derive(Resource, Default)]
pub(crate) struct SimCommandQueue(Vec<SimCommand>);

impl SimCommandQueue {
    pub(crate) fn push(&mut self, command: SimCommand) {
        self.0.push(command);
    }

    pub(crate) fn drain(&mut self) -> impl Iterator<Item = SimCommand> + '_ {
        self.0.drain(..)
    }
}

#[derive(Debug, Clone, Copy)]
struct TravelTween {
    target_index: usize,
    elapsed_s: f64,
    duration_s: f64,
    start_focus_km: [f64; 3],
    start_distance_units: f64,
    target_distance_units: f64,
}

/// The camera's simulation-side truth. All values remain f64 until
/// `render_translation` is called by the render-only camera system.
#[derive(Resource, Debug, Clone)]
pub struct CameraController {
    selected_body_index: usize,
    focus_body_index: usize,
    focus_position_km: [f64; 3],
    yaw_rad: f64,
    pitch_rad: f64,
    distance_units: f64,
    travel: Option<TravelTween>,
}

impl CameraController {
    pub(crate) fn new(
        focus_body_index: usize,
        focus_position_km: [f64; 3],
        distance_units: f64,
    ) -> Self {
        Self {
            selected_body_index: focus_body_index,
            focus_body_index,
            focus_position_km,
            yaw_rad: 0.0,
            pitch_rad: 0.35,
            distance_units,
            travel: None,
        }
    }

    pub(crate) fn unavailable() -> Self {
        Self::new(0, [0.0; 3], DEFAULT_CAMERA_DISTANCE_UNITS)
    }

    pub(crate) fn set_initial_pose(&mut self, yaw_rad: f64, pitch_rad: f64, distance_units: f64) {
        debug_assert!(yaw_rad.is_finite());
        debug_assert!(pitch_rad.is_finite());
        debug_assert!(distance_units.is_finite() && distance_units > 0.0);
        self.yaw_rad = yaw_rad;
        self.pitch_rad = pitch_rad;
        self.distance_units = distance_units;
    }

    pub fn selected_body_index(&self) -> usize {
        self.selected_body_index
    }

    pub fn focus_body_index(&self) -> usize {
        self.focus_body_index
    }

    pub fn focus_position_km(&self) -> [f64; 3] {
        self.focus_position_km
    }

    pub fn yaw_rad(&self) -> f64 {
        self.yaw_rad
    }

    pub fn pitch_rad(&self) -> f64 {
        self.pitch_rad
    }

    pub fn distance_units(&self) -> f64 {
        self.distance_units
    }

    pub fn is_travelling(&self) -> bool {
        self.travel.is_some()
    }

    pub(crate) fn render_translation(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw_rad.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch_rad.sin_cos();
        Vec3::new(
            (cos_yaw * cos_pitch * self.distance_units) as f32,
            (sin_pitch * self.distance_units) as f32,
            (sin_yaw * cos_pitch * self.distance_units) as f32,
        )
    }

    /// Camera position in the simulation's f64 ecliptic frame. Render-only
    /// systems use this for view-dependent fades before the final f32 rebase.
    pub(crate) fn camera_position_km(&self) -> [f64; 3] {
        let (sin_yaw, cos_yaw) = self.yaw_rad.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch_rad.sin_cos();
        let distance_km = self.distance_units * KM_PER_RENDER_UNIT;
        [
            self.focus_position_km[0] + cos_yaw * cos_pitch * distance_km,
            self.focus_position_km[1] + sin_yaw * cos_pitch * distance_km,
            self.focus_position_km[2] + sin_pitch * distance_km,
        ]
    }
}

/// USER_STATE_MUTATION_GATE: this is the sole match over `SimCommand` that
/// mutates the clock or camera controller. Input and UI code may only enqueue.
pub(crate) fn consume_sim_command(
    command: &SimCommand,
    clock: &mut SimClock,
    camera: &mut CameraController,
    loaded: &LoadedCatalog,
) -> TickReport {
    let mut report = TickReport::default();
    match command {
        SimCommand::SelectBody(id) | SimCommand::TravelToBody(id) => {
            let Some(target_index) = loaded.index_of(id) else {
                return report;
            };
            camera.selected_body_index = target_index;
            camera.travel = Some(TravelTween {
                target_index,
                elapsed_s: 0.0,
                duration_s: TRAVEL_DURATION_S,
                start_focus_km: camera.focus_position_km,
                start_distance_units: camera.distance_units,
                target_distance_units: framing_distance_units(loaded, target_index),
            });
        }
        SimCommand::Orbit {
            delta_yaw,
            delta_pitch,
        } => {
            camera.yaw_rad -= delta_yaw * ORBIT_RADIANS_PER_PIXEL;
            camera.pitch_rad = (camera.pitch_rad + delta_pitch * ORBIT_RADIANS_PER_PIXEL)
                .clamp(MIN_PITCH_RAD, MAX_PITCH_RAD);
        }
        SimCommand::Dolly { delta } => {
            let factor = (1.0 - delta * 0.1).clamp(0.1, 10.0);
            let (minimum, maximum) = zoom_limits(loaded, camera.selected_body_index);
            camera.distance_units = (camera.distance_units * factor).clamp(minimum, maximum);
            if let Some(travel) = camera.travel.as_mut() {
                travel.target_distance_units = camera.distance_units;
            }
        }
        SimCommand::SetTime(t_s) => {
            if t_s.is_finite() {
                report.clamped = clock.set_t(*t_s);
            }
        }
        SimCommand::SetRate(rate) => clock.set_rate(*rate),
        SimCommand::StepRate(delta) => clock.step_rate(*delta),
        SimCommand::Play => clock.play(),
        SimCommand::Pause => clock.pause(),
        SimCommand::TogglePlay => clock.toggle_play(),
        SimCommand::SnapToLive => clock.snap_to_live(),
        SimCommand::NavigateBreadcrumb { target_id, .. } => {
            let resolved_id = if target_id == "solar_system" {
                "sun"
            } else {
                target_id.as_str()
            };
            let Some(target_index) = loaded.index_of(resolved_id) else {
                return report;
            };
            camera.selected_body_index = target_index;
            camera.travel = Some(TravelTween {
                target_index,
                elapsed_s: 0.0,
                duration_s: TRAVEL_DURATION_S,
                start_focus_km: camera.focus_position_km,
                start_distance_units: camera.distance_units,
                target_distance_units: framing_distance_units(loaded, target_index),
            });
        }
        SimCommand::SetLayerVisibility { .. }
        | SimCommand::SetBodySize(_)
        | SimCommand::SetMoonVisibility { .. }
        | SimCommand::SetLocalOrbitVisibility { .. }
        | SimCommand::SetLeftPanelCollapsed(_)
        | SimCommand::SetLeftPanelTab(_)
        | SimCommand::SetBrowseOpen(_)
        | SimCommand::SetBrowseColumnExpanded { .. }
        | SimCommand::ApplySettings(_)
        | SimCommand::RestorePresentationDefaults
        | SimCommand::ToggleFullscreen
        | SimCommand::OpenSettings
        | SimCommand::CloseSettings
        | SimCommand::SimulateDeviceLoss => {}
    }
    report
}

/// Deterministic presentation reducer. It is called beside the simulation
/// reducer by both the desktop command gate and the headless replay runner.
pub(crate) fn consume_presentation_command(
    command: &SimCommand,
    layers: &mut LayerState,
    presentation: &mut PresentationState,
) {
    match command {
        SimCommand::SetLayerVisibility { layer, visible } => {
            layers.set_visible(*layer, *visible);
        }
        SimCommand::ApplySettings(settings) => {
            *layers = settings.initial_layer_state();
            presentation.set_fullscreen(settings.display_mode.is_fullscreen());
        }
        SimCommand::RestorePresentationDefaults => *layers = LayerState::default(),
        SimCommand::ToggleFullscreen => presentation.toggle_fullscreen(),
        SimCommand::OpenSettings => presentation.request_settings(),
        SimCommand::CloseSettings => presentation.request_settings_close(),
        SimCommand::SimulateDeviceLoss => {}
        SimCommand::SetBodySize(_)
        | SimCommand::SetMoonVisibility { .. }
        | SimCommand::SetLocalOrbitVisibility { .. }
        | SimCommand::SetLeftPanelCollapsed(_)
        | SimCommand::SetLeftPanelTab(_)
        | SimCommand::SetBrowseOpen(_)
        | SimCommand::SetBrowseColumnExpanded { .. }
        | SimCommand::NavigateBreadcrumb { .. } => {}
        _ => {}
    }
}

/// Evolves Follow/travel from explicit f64 state. A completed tween writes the
/// target's current position before switching to Follow, preventing a landing
/// snap even while the target moves during the transition.
pub(crate) fn advance_camera_controller(
    camera: &mut CameraController,
    states: &BodyStates,
    wall_dt_s: f64,
) {
    let Some(travel) = camera.travel else {
        if let Some(state) = states.0.get(camera.focus_body_index) {
            camera.focus_position_km = state.position_km;
        }
        return;
    };
    let Some(target) = states.0.get(travel.target_index) else {
        return;
    };

    let elapsed_s = (travel.elapsed_s + wall_dt_s.max(0.0)).min(travel.duration_s);
    let progress = if travel.duration_s > 0.0 {
        elapsed_s / travel.duration_s
    } else {
        1.0
    };
    let eased = progress * progress * (3.0 - 2.0 * progress);
    camera.focus_position_km = lerp3(travel.start_focus_km, target.position_km, eased);
    camera.distance_units = lerp(
        travel.start_distance_units,
        travel.target_distance_units,
        eased,
    );

    if elapsed_s >= travel.duration_s {
        camera.focus_body_index = travel.target_index;
        camera.focus_position_km = target.position_km;
        camera.distance_units = travel.target_distance_units;
        camera.travel = None;
    } else if let Some(active) = camera.travel.as_mut() {
        active.elapsed_s = elapsed_s;
    }
}

pub(crate) fn framing_distance_units(loaded: &LoadedCatalog, body_index: usize) -> f64 {
    let body = &loaded.catalog.bodies[body_index];
    let framing_radius_km = loaded
        .catalog
        .bodies
        .iter()
        .filter(|candidate| {
            candidate.category == Category::Moon
                && candidate.parent.as_deref() == Some(body.id.as_str())
        })
        .filter_map(|moon| moon.orbit.as_ref())
        .map(|orbit| orbit.elements.a_km.abs() * (1.0 + orbit.elements.e))
        .fold(body.radius_km, f64::max);
    // The established four-radius body framing also gives a focused planetary
    // system enough room for every modeled moon, including the major moons
    // that WP9 must label immediately after travel.
    let desired = 4.0 * framing_radius_km / KM_PER_RENDER_UNIT;
    let (minimum, maximum) = zoom_limits(loaded, body_index);
    desired.clamp(minimum, maximum)
}

pub(crate) fn full_system_framing_distance_units(loaded: &LoadedCatalog) -> f64 {
    loaded
        .catalog
        .bodies
        .iter()
        .filter(|body| body.category == Category::Planet)
        .filter_map(|body| body.orbit.as_ref())
        .map(|orbit| orbit.elements.a_km.abs() * (1.0 + orbit.elements.e))
        .reduce(f64::max)
        .map_or(DEFAULT_CAMERA_DISTANCE_UNITS, |outermost_planet_km| {
            4.0 * outermost_planet_km / KM_PER_RENDER_UNIT
        })
}

pub(crate) fn zoom_limits(loaded: &LoadedCatalog, body_index: usize) -> (f64, f64) {
    let minimum = 1.2 * loaded.catalog.bodies[body_index].radius_km / KM_PER_RENDER_UNIT;
    let sedna_aphelion_km = loaded
        .index_of("sedna")
        .and_then(|index| loaded.catalog.bodies[index].orbit.as_ref())
        .map_or(1.0e12, |orbit| {
            orbit.elements.a_km * (1.0 + orbit.elements.e)
        });
    let maximum = (1.5 * sedna_aphelion_km / KM_PER_RENDER_UNIT).max(minimum);
    (minimum, maximum)
}

fn lerp(start: f64, end: f64, t: f64) -> f64 {
    start + (end - start) * t
}

fn lerp3(start: [f64; 3], end: [f64; 3], t: f64) -> [f64; 3] {
    [
        lerp(start[0], end[0], t),
        lerp(start[1], end[1], t),
        lerp(start[2], end[2], t),
    ]
}

#[derive(Debug, Clone, PartialEq)]
pub struct StampedCommand {
    pub frame: u64,
    pub sim_time_s: f64,
    pub command: SimCommand,
}

/// Text replay format v1. Floating-point values are stored by raw bits, so a
/// serialization round-trip cannot alter an input before deterministic replay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayFrameInput {
    pub frame: u64,
    pub wall_dt_s: f64,
    pub wall_now_t: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReplayStream {
    pub frames: Vec<ReplayFrameInput>,
    pub entries: Vec<StampedCommand>,
}

impl ReplayStream {
    const HEADER_V1: &'static str = "solar-sim-replay-v1";
    const HEADER_V2: &'static str = "solar-sim-replay-v2";

    pub fn to_text(&self) -> String {
        let mut output = String::from(Self::HEADER_V2);
        output.push('\n');
        for frame in &self.frames {
            output.push_str(&format!(
                "@frame|{}|{:016x}|{:016x}\n",
                frame.frame,
                frame.wall_dt_s.to_bits(),
                frame.wall_now_t.to_bits()
            ));
        }
        for entry in &self.entries {
            output.push_str(&serialize_entry(entry));
            output.push('\n');
        }
        output
    }

    pub fn from_text(text: &str) -> Result<Self, ReplayParseError> {
        let mut lines = text.lines();
        let header = lines.next();
        if !matches!(header, Some(Self::HEADER_V1 | Self::HEADER_V2)) {
            return Err(ReplayParseError(vec!["missing replay-v1 header".into()]));
        }
        let is_v2 = header == Some(Self::HEADER_V2);
        let mut frames = Vec::new();
        let mut entries = Vec::new();
        let mut errors = Vec::new();
        for (index, line) in lines.enumerate() {
            if line.is_empty() {
                continue;
            }
            if line.starts_with("@frame|") {
                if !is_v2 {
                    errors.push(format!(
                        "line {}: frame input requires replay-v2",
                        index + 2
                    ));
                    continue;
                }
                match parse_frame_input(line) {
                    Ok(frame) => frames.push(frame),
                    Err(message) => errors.push(format!("line {}: {message}", index + 2)),
                }
                continue;
            }
            match parse_entry(line) {
                Ok(entry) => entries.push(entry),
                Err(message) => errors.push(format!("line {}: {message}", index + 2)),
            }
        }
        if errors.is_empty() {
            Ok(Self { frames, entries })
        } else {
            Err(ReplayParseError(errors))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayParseError(Vec<String>);

impl fmt::Display for ReplayParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "replay stream is invalid")?;
        for error in &self.0 {
            write!(f, "\n- {error}")?;
        }
        Ok(())
    }
}

#[derive(Resource, Debug, Default)]
pub struct CommandRecording {
    stream: ReplayStream,
}

impl CommandRecording {
    pub fn stream(&self) -> &ReplayStream {
        &self.stream
    }

    pub(crate) fn record(&mut self, frame: u64, sim_time_s: f64, command: SimCommand) {
        self.stream.entries.push(StampedCommand {
            frame,
            sim_time_s,
            command,
        });
    }

    pub(crate) fn record_frame(&mut self, frame: u64, wall_dt_s: f64, wall_now_t: f64) {
        if self
            .stream
            .frames
            .last()
            .is_some_and(|input| input.frame == frame)
        {
            return;
        }
        self.stream.frames.push(ReplayFrameInput {
            frame,
            wall_dt_s,
            wall_now_t,
        });
    }
}

#[derive(Resource, Debug, Default)]
pub(crate) struct SimulationFrame(pub(crate) u64);

#[derive(Debug)]
pub enum ReplayRunError {
    InvalidCatalog(Vec<CatalogError>),
    MissingSun,
    Propagation(PropagationError),
    EntriesNotOrdered {
        previous: u64,
        next: u64,
    },
    EntryAfterLastFrame {
        frame: u64,
        total_frames: u64,
    },
    TimestampMismatch {
        frame: u64,
        expected: f64,
        actual: f64,
    },
    FrameInputsIncomplete {
        expected: u64,
        actual: usize,
    },
    FrameInputOutOfOrder {
        expected: u64,
        actual: u64,
    },
    InvalidFrameInput {
        frame: u64,
    },
}

impl fmt::Display for ReplayRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplayRunError::InvalidCatalog(errors) => {
                write!(f, "replay catalog is invalid")?;
                for error in errors {
                    write!(f, "\n- {error}")?;
                }
                Ok(())
            }
            ReplayRunError::MissingSun => write!(f, "replay catalog has no 'sun' id"),
            ReplayRunError::Propagation(error) => write!(f, "replay propagation failed: {error}"),
            ReplayRunError::EntriesNotOrdered { previous, next } => write!(
                f,
                "replay entries are not frame-ordered ({previous} before {next})"
            ),
            ReplayRunError::EntryAfterLastFrame {
                frame,
                total_frames,
            } => write!(
                f,
                "replay command frame {frame} is outside {total_frames} frames"
            ),
            ReplayRunError::TimestampMismatch {
                frame,
                expected,
                actual,
            } => write!(
                f,
                "replay timestamp mismatch at frame {frame}: expected {expected}, got {actual}"
            ),
            ReplayRunError::FrameInputsIncomplete { expected, actual } => write!(
                f,
                "replay-v2 has {actual} frame inputs; expected {expected}"
            ),
            ReplayRunError::FrameInputOutOfOrder { expected, actual } => {
                write!(f, "replay-v2 expected frame input {expected}, got {actual}")
            }
            ReplayRunError::InvalidFrameInput { frame } => {
                write!(f, "replay-v2 frame {frame} has invalid wall inputs")
            }
        }
    }
}

/// Render-free deterministic simulation used by the record/replay CI gate.
/// It executes the same command consumer, clock, propagation, and tween code
/// as the desktop app and never constructs an f32 transform.
pub struct HeadlessSimulation {
    loaded: LoadedCatalog,
    clock: SimClock,
    states: BodyStates,
    camera: CameraController,
    layers: LayerState,
    presentation: PresentationState,
    view_options: crate::ViewOptionsState,
    app_settings: AppSettings,
    left_panel_tab: LeftPanelTab,
    browse_open: bool,
    browse_expanded: [bool; 3],
    navigation_depth: usize,
    frame: u64,
    wall_now_t: f64,
}

impl HeadlessSimulation {
    pub fn new(catalog: &Catalog) -> Result<Self, ReplayRunError> {
        catalog.validate().map_err(ReplayRunError::InvalidCatalog)?;
        let loaded = LoadedCatalog::new(catalog.clone());
        let wall_now_t = 0.0;
        let clock = SimClock::new(StartMode::default(), wall_now_t);
        let states = crate::propagate_catalog(&loaded.catalog, clock.t())
            .map_err(ReplayRunError::Propagation)?;
        let focus_body_index = loaded.index_of("sun").ok_or(ReplayRunError::MissingSun)?;
        let focus_position_km = states
            .0
            .get(focus_body_index)
            .ok_or(ReplayRunError::MissingSun)?
            .position_km;
        let camera = CameraController::new(
            focus_body_index,
            focus_position_km,
            DEFAULT_CAMERA_DISTANCE_UNITS,
        );
        Ok(Self {
            loaded,
            clock,
            states,
            camera,
            layers: LayerState::default(),
            presentation: PresentationState::default(),
            view_options: crate::ViewOptionsState::default(),
            app_settings: AppSettings::default(),
            left_panel_tab: LeftPanelTab::Info,
            browse_open: false,
            browse_expanded: [false; 3],
            navigation_depth: 0,
            frame: 0,
            wall_now_t,
        })
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }

    pub fn clock(&self) -> &SimClock {
        &self.clock
    }

    pub fn states(&self) -> &BodyStates {
        &self.states
    }

    pub fn camera(&self) -> &CameraController {
        &self.camera
    }

    pub fn layer_state(&self) -> &LayerState {
        &self.layers
    }

    pub fn layer_state_hash(&self) -> u64 {
        self.layers.stable_hash()
    }

    pub fn step(
        &mut self,
        wall_dt_s: f64,
        commands: &[SimCommand],
        recording: Option<&mut CommandRecording>,
    ) -> Result<(), PropagationError> {
        let wall_now_t = self.wall_now_t + wall_dt_s;
        self.step_with_wall_time(wall_dt_s, wall_now_t, commands, recording)
    }

    pub fn step_with_wall_time(
        &mut self,
        wall_dt_s: f64,
        wall_now_t: f64,
        commands: &[SimCommand],
        mut recording: Option<&mut CommandRecording>,
    ) -> Result<(), PropagationError> {
        if let Some(recorder) = recording.as_deref_mut() {
            recorder.record_frame(self.frame, wall_dt_s, wall_now_t);
        }
        for command in commands {
            if let Some(recorder) = recording.as_deref_mut() {
                recorder.record(self.frame, self.clock.t(), command.clone());
            }
            consume_presentation_command(command, &mut self.layers, &mut self.presentation);
            consume_sim_command(command, &mut self.clock, &mut self.camera, &self.loaded);
            consume_headless_application_command(
                command,
                &mut self.view_options,
                &mut self.app_settings,
                &mut self.left_panel_tab,
                &mut self.browse_open,
                &mut self.browse_expanded,
                &mut self.navigation_depth,
            );
        }
        self.wall_now_t = wall_now_t;
        self.clock.tick(wall_dt_s, wall_now_t);
        propagate_into(&self.loaded.catalog, self.clock.t(), &mut self.states)?;
        advance_camera_controller(&mut self.camera, &self.states, wall_dt_s);
        self.frame += 1;
        Ok(())
    }

    /// Cross-platform replay hash over f64 simulation truth only. Values are
    /// quantized on a canonical 1 km / 1 mm·s⁻¹ grid before hashing to avoid
    /// platform libm last-bit noise while still catching visible divergence.
    /// Render state is deliberately absent.
    pub fn state_hash(&self) -> u64 {
        let mut hash = Fnv1a::new();
        hash.u64(self.frame);
        hash.i64(quantize(self.clock.t(), 1.0e-6));
        hash.i8(self.clock.rate().get());
        hash.u8(u8::from(self.clock.is_playing()));
        hash.u8(u8::from(self.clock.is_snapping()));
        hash.u64(self.camera.selected_body_index as u64);
        hash.u64(self.camera.focus_body_index as u64);
        for value in self.camera.focus_position_km {
            hash.i64(quantize(value, 1.0));
        }
        hash.i64(quantize(self.camera.yaw_rad, 1.0e-12));
        hash.i64(quantize(self.camera.pitch_rad, 1.0e-12));
        hash.i64(quantize(self.camera.distance_units, 1.0e-9));
        hash_view_options(&mut hash, &self.view_options);
        hash_app_settings(&mut hash, &self.app_settings);
        hash.u8(match self.left_panel_tab {
            LeftPanelTab::Info => 0,
            LeftPanelTab::Collection => 1,
            LeftPanelTab::ViewOptions => 2,
        });
        hash.u8(u8::from(self.browse_open));
        for expanded in self.browse_expanded {
            hash.u8(u8::from(expanded));
        }
        hash.u64(self.navigation_depth as u64);
        match self.camera.travel {
            Some(travel) => {
                hash.u8(1);
                hash.u64(travel.target_index as u64);
                hash.i64(quantize(travel.elapsed_s, 1.0e-9));
                hash.i64(quantize(travel.duration_s, 1.0e-9));
                for value in travel.start_focus_km {
                    hash.i64(quantize(value, 1.0));
                }
                hash.i64(quantize(travel.start_distance_units, 1.0e-9));
                hash.i64(quantize(travel.target_distance_units, 1.0e-9));
            }
            None => hash.u8(0),
        }
        for state in &self.states.0 {
            for value in state.position_km {
                hash.i64(quantize(value, 1.0));
            }
            for value in state.velocity_km_s {
                hash.i64(quantize(value, 1.0e-6));
            }
        }
        hash.finish()
    }
}

#[allow(clippy::too_many_arguments)]
fn consume_headless_application_command(
    command: &SimCommand,
    view_options: &mut crate::ViewOptionsState,
    app_settings: &mut AppSettings,
    left_panel_tab: &mut LeftPanelTab,
    browse_open: &mut bool,
    browse_expanded: &mut [bool; 3],
    navigation_depth: &mut usize,
) {
    match command {
        SimCommand::SetBodySize(scale) => view_options.set_body_size(*scale),
        SimCommand::SetMoonVisibility { system_id, mode } => {
            view_options.set_moon_visibility(system_id.clone(), *mode);
        }
        SimCommand::SetLocalOrbitVisibility { body_id, visible } => {
            view_options.set_local_orbit_visible(body_id.clone(), *visible);
        }
        SimCommand::SetLeftPanelCollapsed(collapsed) => {
            view_options.set_panel_collapsed(*collapsed);
        }
        SimCommand::SetLeftPanelTab(tab) => *left_panel_tab = *tab,
        SimCommand::SetBrowseOpen(open) => *browse_open = *open,
        SimCommand::SetBrowseColumnExpanded { column, expanded } => {
            if let Some(value) = browse_expanded.get_mut(usize::from(*column)) {
                *value = *expanded;
            }
        }
        SimCommand::ApplySettings(settings) => {
            *app_settings = settings.as_ref().clone().normalized()
        }
        SimCommand::RestorePresentationDefaults => {
            *view_options = crate::ViewOptionsState::default();
            app_settings.layers = crate::PersistedLayerState::default();
            *browse_open = false;
            *browse_expanded = [false; 3];
            *left_panel_tab = LeftPanelTab::Info;
            *navigation_depth = 0;
        }
        SimCommand::NavigateBreadcrumb { depth, .. } => *navigation_depth = *depth,
        _ => {}
    }
}

fn hash_view_options(hash: &mut Fnv1a, options: &crate::ViewOptionsState) {
    let snapshot = options.persistence_snapshot();
    hash.u8(u8::from(snapshot.panel_collapsed));
    hash.u8(match snapshot.body_size {
        BodySizeScale::X1 => 0,
        BodySizeScale::X10 => 1,
        BodySizeScale::X50 => 2,
    });
    for (system_id, mode) in snapshot.moon_visibility_by_system {
        hash.bytes(system_id.as_bytes());
        hash.u8(match mode {
            MoonVisibilityMode::Major => 0,
            MoonVisibilityMode::All => 1,
        });
    }
    for (body_id, visible) in snapshot.local_orbit_visibility {
        hash.bytes(body_id.as_bytes());
        hash.u8(u8::from(visible));
    }
}

fn hash_app_settings(hash: &mut Fnv1a, settings: &AppSettings) {
    hash.u8(match settings.display_mode {
        crate::DisplayModeSetting::Windowed => 0,
        crate::DisplayModeSetting::BorderlessFullscreen => 1,
    });
    hash.u64(u64::from(settings.resolution.width));
    hash.u64(u64::from(settings.resolution.height));
    hash.u8(u8::from(settings.vsync));
    hash.u8(match settings.frame_cap {
        crate::FrameCap::Fps30 => 0,
        crate::FrameCap::Fps60 => 1,
        crate::FrameCap::Fps120 => 2,
        crate::FrameCap::Fps240 => 3,
        crate::FrameCap::Unlimited => 4,
    });
    hash.u8(match settings.quality {
        crate::QualityPreset::Low => 0,
        crate::QualityPreset::Medium => 1,
        crate::QualityPreset::High => 2,
        crate::QualityPreset::Ultra => 3,
    });
    hash.u64(u64::from(settings.ui_scale.to_bits()));
    hash.u8(match settings.units {
        crate::DistanceUnit::Kilometers => 0,
        crate::DistanceUnit::Miles => 1,
        crate::DistanceUnit::AstronomicalUnits => 2,
    });
    match settings.start_mode {
        crate::StartModeSetting::FixedEpoch { jd_tdb } => {
            hash.u8(0);
            hash.u64(jd_tdb.to_bits());
        }
        crate::StartModeSetting::Live => hash.u8(1),
    }
    hash.u8(u8::from(settings.invert_horizontal));
    hash.u8(u8::from(settings.invert_vertical));
    for visible in [
        settings.layers.user_interface,
        settings.layers.planets,
        settings.layers.dwarf_planets,
        settings.layers.asteroids,
        settings.layers.comets,
        settings.layers.moons,
        settings.layers.orbits,
        settings.layers.labels,
        settings.layers.icons,
    ] {
        hash.u8(u8::from(visible));
    }
}

pub fn replay_headless(
    catalog: &Catalog,
    stream: &ReplayStream,
    total_frames: u64,
    wall_dt_s: f64,
) -> Result<HeadlessSimulation, ReplayRunError> {
    if !stream.frames.is_empty() {
        if stream.frames.len() != total_frames as usize {
            return Err(ReplayRunError::FrameInputsIncomplete {
                expected: total_frames,
                actual: stream.frames.len(),
            });
        }
        for (expected, input) in stream.frames.iter().enumerate() {
            let expected = expected as u64;
            if input.frame != expected {
                return Err(ReplayRunError::FrameInputOutOfOrder {
                    expected,
                    actual: input.frame,
                });
            }
            if !input.wall_dt_s.is_finite()
                || input.wall_dt_s < 0.0
                || !input.wall_now_t.is_finite()
            {
                return Err(ReplayRunError::InvalidFrameInput { frame: expected });
            }
        }
    }
    for pair in stream.entries.windows(2) {
        if pair[0].frame > pair[1].frame {
            return Err(ReplayRunError::EntriesNotOrdered {
                previous: pair[0].frame,
                next: pair[1].frame,
            });
        }
    }
    if let Some(entry) = stream.entries.last() {
        if entry.frame >= total_frames {
            return Err(ReplayRunError::EntryAfterLastFrame {
                frame: entry.frame,
                total_frames,
            });
        }
    }

    let mut simulation = HeadlessSimulation::new(catalog)?;
    let mut entry_index = 0;
    for frame in 0..total_frames {
        let start = entry_index;
        while entry_index < stream.entries.len() && stream.entries[entry_index].frame == frame {
            let entry = &stream.entries[entry_index];
            if entry.sim_time_s.to_bits() != simulation.clock.t().to_bits() {
                return Err(ReplayRunError::TimestampMismatch {
                    frame,
                    expected: entry.sim_time_s,
                    actual: simulation.clock.t(),
                });
            }
            entry_index += 1;
        }
        let commands: Vec<_> = stream.entries[start..entry_index]
            .iter()
            .map(|entry| entry.command.clone())
            .collect();
        if let Some(input) = stream.frames.get(frame as usize) {
            simulation
                .step_with_wall_time(input.wall_dt_s, input.wall_now_t, &commands, None)
                .map_err(ReplayRunError::Propagation)?;
        } else {
            simulation
                .step(wall_dt_s, &commands, None)
                .map_err(ReplayRunError::Propagation)?;
        }
    }
    Ok(simulation)
}

fn serialize_entry(entry: &StampedCommand) -> String {
    let prefix = format!("{}|{:016x}", entry.frame, entry.sim_time_s.to_bits());
    match &entry.command {
        SimCommand::SelectBody(id) => format!("{prefix}|select|{id}"),
        SimCommand::TravelToBody(id) => format!("{prefix}|travel|{id}"),
        SimCommand::Orbit {
            delta_yaw,
            delta_pitch,
        } => format!(
            "{prefix}|orbit|{:016x}|{:016x}",
            delta_yaw.to_bits(),
            delta_pitch.to_bits()
        ),
        SimCommand::Dolly { delta } => {
            format!("{prefix}|dolly|{:016x}", delta.to_bits())
        }
        SimCommand::SetTime(t_s) => format!("{prefix}|set-time|{:016x}", t_s.to_bits()),
        SimCommand::SetRate(rate) => format!("{prefix}|set-rate|{}", rate.get()),
        SimCommand::StepRate(delta) => format!("{prefix}|step-rate|{delta}"),
        SimCommand::Play => format!("{prefix}|play"),
        SimCommand::Pause => format!("{prefix}|pause"),
        SimCommand::TogglePlay => format!("{prefix}|toggle-play"),
        SimCommand::SnapToLive => format!("{prefix}|snap-live"),
        SimCommand::SetLayerVisibility { layer, visible } => format!(
            "{prefix}|layer|{}|{}",
            layer.replay_slug(),
            u8::from(*visible)
        ),
        SimCommand::SetBodySize(scale) => {
            let value = match scale {
                BodySizeScale::X1 => "1",
                BodySizeScale::X10 => "10",
                BodySizeScale::X50 => "50",
            };
            format!("{prefix}|body-size|{value}")
        }
        SimCommand::SetMoonVisibility { system_id, mode } => format!(
            "{prefix}|moon-visibility|{system_id}|{}",
            match mode {
                MoonVisibilityMode::Major => "major",
                MoonVisibilityMode::All => "all",
            }
        ),
        SimCommand::SetLocalOrbitVisibility { body_id, visible } => {
            format!("{prefix}|local-orbit|{body_id}|{}", u8::from(*visible))
        }
        SimCommand::SetLeftPanelCollapsed(collapsed) => {
            format!("{prefix}|panel-collapsed|{}", u8::from(*collapsed))
        }
        SimCommand::SetLeftPanelTab(tab) => format!(
            "{prefix}|panel-tab|{}",
            match tab {
                LeftPanelTab::Info => "info",
                LeftPanelTab::Collection => "collection",
                LeftPanelTab::ViewOptions => "view",
            }
        ),
        SimCommand::SetBrowseOpen(open) => {
            format!("{prefix}|browse-open|{}", u8::from(*open))
        }
        SimCommand::SetBrowseColumnExpanded { column, expanded } => {
            format!("{prefix}|browse-expanded|{column}|{}", u8::from(*expanded))
        }
        SimCommand::ApplySettings(settings) => serialize_settings_command(&prefix, settings),
        SimCommand::RestorePresentationDefaults => format!("{prefix}|restore-presentation"),
        SimCommand::NavigateBreadcrumb { depth, target_id } => {
            format!("{prefix}|navigate-breadcrumb|{depth}|{target_id}")
        }
        SimCommand::ToggleFullscreen => format!("{prefix}|toggle-fullscreen"),
        SimCommand::OpenSettings => format!("{prefix}|open-settings"),
        SimCommand::CloseSettings => format!("{prefix}|close-settings"),
        SimCommand::SimulateDeviceLoss => format!("{prefix}|simulate-device-loss"),
    }
}

fn serialize_settings_command(prefix: &str, settings: &AppSettings) -> String {
    let display = match settings.display_mode {
        crate::DisplayModeSetting::Windowed => "windowed",
        crate::DisplayModeSetting::BorderlessFullscreen => "fullscreen",
    };
    let frame_cap = match settings.frame_cap {
        crate::FrameCap::Fps30 => "30",
        crate::FrameCap::Fps60 => "60",
        crate::FrameCap::Fps120 => "120",
        crate::FrameCap::Fps240 => "240",
        crate::FrameCap::Unlimited => "unlimited",
    };
    let quality = match settings.quality {
        crate::QualityPreset::Low => "low",
        crate::QualityPreset::Medium => "medium",
        crate::QualityPreset::High => "high",
        crate::QualityPreset::Ultra => "ultra",
    };
    let units = match settings.units {
        crate::DistanceUnit::Kilometers => "km",
        crate::DistanceUnit::Miles => "mi",
        crate::DistanceUnit::AstronomicalUnits => "au",
    };
    let (start_kind, start_value) = match settings.start_mode {
        crate::StartModeSetting::FixedEpoch { jd_tdb } => {
            ("fixed", format!("{:016x}", jd_tdb.to_bits()))
        }
        crate::StartModeSetting::Live => ("live", "-".to_string()),
    };
    format!(
        "{prefix}|apply-settings|{display}|{}|{}|{}|{frame_cap}|{quality}|{:08x}|{units}|{start_kind}|{start_value}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        settings.resolution.width,
        settings.resolution.height,
        u8::from(settings.vsync),
        settings.ui_scale.to_bits(),
        u8::from(settings.invert_horizontal),
        u8::from(settings.invert_vertical),
        u8::from(settings.layers.user_interface),
        u8::from(settings.layers.planets),
        u8::from(settings.layers.dwarf_planets),
        u8::from(settings.layers.asteroids),
        u8::from(settings.layers.comets),
        u8::from(settings.layers.moons),
        u8::from(settings.layers.orbits),
        u8::from(settings.layers.labels),
        u8::from(settings.layers.icons),
    )
}

fn parse_settings_command(fields: &[&str]) -> Result<AppSettings, String> {
    if fields.len() != 24 {
        return Err("apply-settings has the wrong field count".into());
    }
    let display_mode = match fields[3] {
        "windowed" => crate::DisplayModeSetting::Windowed,
        "fullscreen" => crate::DisplayModeSetting::BorderlessFullscreen,
        _ => return Err("unknown display mode".into()),
    };
    let width = fields[4]
        .parse::<u32>()
        .map_err(|_| "settings width is not a u32")?;
    let height = fields[5]
        .parse::<u32>()
        .map_err(|_| "settings height is not a u32")?;
    let vsync = parse_bool(fields[6], "vsync")?;
    let frame_cap = match fields[7] {
        "30" => crate::FrameCap::Fps30,
        "60" => crate::FrameCap::Fps60,
        "120" => crate::FrameCap::Fps120,
        "240" => crate::FrameCap::Fps240,
        "unlimited" => crate::FrameCap::Unlimited,
        _ => return Err("unknown frame cap".into()),
    };
    let quality = match fields[8] {
        "low" => crate::QualityPreset::Low,
        "medium" => crate::QualityPreset::Medium,
        "high" => crate::QualityPreset::High,
        "ultra" => crate::QualityPreset::Ultra,
        _ => return Err("unknown quality preset".into()),
    };
    let ui_scale_bits =
        u32::from_str_radix(fields[9], 16).map_err(|_| "UI scale is not f32 bits")?;
    let ui_scale = f32::from_bits(ui_scale_bits);
    if !ui_scale.is_finite() {
        return Err("UI scale is not finite".into());
    }
    let units = match fields[10] {
        "km" => crate::DistanceUnit::Kilometers,
        "mi" => crate::DistanceUnit::Miles,
        "au" => crate::DistanceUnit::AstronomicalUnits,
        _ => return Err("unknown distance unit".into()),
    };
    let start_mode = match fields[11] {
        "fixed" => crate::StartModeSetting::FixedEpoch {
            jd_tdb: parse_f64_bits(fields[12], "start epoch")?,
        },
        "live" if fields[12] == "-" => crate::StartModeSetting::Live,
        _ => return Err("invalid start mode".into()),
    };
    Ok(AppSettings {
        display_mode,
        resolution: crate::ResolutionSetting { width, height },
        vsync,
        frame_cap,
        quality,
        ui_scale,
        units,
        start_mode,
        invert_horizontal: parse_bool(fields[13], "invert horizontal")?,
        invert_vertical: parse_bool(fields[14], "invert vertical")?,
        layers: crate::PersistedLayerState {
            user_interface: parse_bool(fields[15], "UI layer")?,
            planets: parse_bool(fields[16], "planets layer")?,
            dwarf_planets: parse_bool(fields[17], "dwarf-planets layer")?,
            asteroids: parse_bool(fields[18], "asteroids layer")?,
            comets: parse_bool(fields[19], "comets layer")?,
            moons: parse_bool(fields[20], "moons layer")?,
            orbits: parse_bool(fields[21], "orbits layer")?,
            labels: parse_bool(fields[22], "labels layer")?,
            icons: parse_bool(fields[23], "icons layer")?,
        },
    }
    .normalized())
}

fn parse_frame_input(line: &str) -> Result<ReplayFrameInput, String> {
    let fields: Vec<_> = line.split('|').collect();
    if fields.len() != 4 || fields[0] != "@frame" {
        return Err("malformed replay frame input".into());
    }
    let frame = fields[1]
        .parse::<u64>()
        .map_err(|_| "frame input index is not a u64")?;
    let wall_dt_s = parse_f64_bits(fields[2], "wall delta")?;
    let wall_now_t = parse_f64_bits(fields[3], "wall time")?;
    if wall_dt_s < 0.0 {
        return Err("wall delta is negative".into());
    }
    Ok(ReplayFrameInput {
        frame,
        wall_dt_s,
        wall_now_t,
    })
}

fn parse_entry(line: &str) -> Result<StampedCommand, String> {
    let fields: Vec<_> = line.split('|').collect();
    if fields.len() < 3 {
        return Err("expected frame|timestamp|command".into());
    }
    let frame = fields[0].parse::<u64>().map_err(|_| "frame is not a u64")?;
    let sim_time_s = parse_f64_bits(fields[1], "timestamp")?;
    let command = match fields[2] {
        "select" => SimCommand::SelectBody(parse_body_id(&fields, 4)?),
        "travel" => SimCommand::TravelToBody(parse_body_id(&fields, 4)?),
        "orbit" if fields.len() == 5 => SimCommand::Orbit {
            delta_yaw: parse_f64_bits(fields[3], "orbit yaw")?,
            delta_pitch: parse_f64_bits(fields[4], "orbit pitch")?,
        },
        "dolly" if fields.len() == 4 => SimCommand::Dolly {
            delta: parse_f64_bits(fields[3], "dolly")?,
        },
        "set-time" if fields.len() == 4 => {
            let t_s = parse_f64_bits(fields[3], "time")?;
            if !t_s.is_finite() {
                return Err("time is not finite".into());
            }
            SimCommand::SetTime(t_s)
        }
        "set-rate" if fields.len() == 4 => {
            let raw = fields[3].parse::<i8>().map_err(|_| "rate is not an i8")?;
            let rate = RateIndex::new(raw).ok_or("rate is outside -12..=-1 or 1..=12")?;
            SimCommand::SetRate(rate)
        }
        "step-rate" if fields.len() == 4 => SimCommand::StepRate(
            fields[3]
                .parse::<i8>()
                .map_err(|_| "rate step is not an i8")?,
        ),
        "play" if fields.len() == 3 => SimCommand::Play,
        "pause" if fields.len() == 3 => SimCommand::Pause,
        "toggle-play" if fields.len() == 3 => SimCommand::TogglePlay,
        "snap-live" if fields.len() == 3 => SimCommand::SnapToLive,
        "layer" if fields.len() == 5 => {
            let layer = LayerId::from_replay_slug(fields[3]).ok_or("unknown layer id")?;
            let visible = match fields[4] {
                "0" => false,
                "1" => true,
                _ => return Err("layer visibility must be 0 or 1".into()),
            };
            SimCommand::SetLayerVisibility { layer, visible }
        }
        "body-size" if fields.len() == 4 => SimCommand::SetBodySize(match fields[3] {
            "1" => BodySizeScale::X1,
            "10" => BodySizeScale::X10,
            "50" => BodySizeScale::X50,
            _ => return Err("body size must be 1, 10, or 50".into()),
        }),
        "moon-visibility" if fields.len() == 5 => SimCommand::SetMoonVisibility {
            system_id: parse_stable_id(fields[3])?,
            mode: match fields[4] {
                "major" => MoonVisibilityMode::Major,
                "all" => MoonVisibilityMode::All,
                _ => return Err("moon visibility must be major or all".into()),
            },
        },
        "local-orbit" if fields.len() == 5 => SimCommand::SetLocalOrbitVisibility {
            body_id: parse_stable_id(fields[3])?,
            visible: parse_bool(fields[4], "local orbit visibility")?,
        },
        "panel-collapsed" if fields.len() == 4 => {
            SimCommand::SetLeftPanelCollapsed(parse_bool(fields[3], "panel collapsed")?)
        }
        "panel-tab" if fields.len() == 4 => SimCommand::SetLeftPanelTab(match fields[3] {
            "info" => LeftPanelTab::Info,
            "collection" => LeftPanelTab::Collection,
            "view" => LeftPanelTab::ViewOptions,
            _ => return Err("panel tab must be info, collection, or view".into()),
        }),
        "browse-open" if fields.len() == 4 => {
            SimCommand::SetBrowseOpen(parse_bool(fields[3], "browse open")?)
        }
        "browse-expanded" if fields.len() == 5 => {
            let column = fields[3]
                .parse::<u8>()
                .map_err(|_| "browse column is not a u8")?;
            if column >= 3 {
                return Err("browse column is outside 0..3".into());
            }
            SimCommand::SetBrowseColumnExpanded {
                column,
                expanded: parse_bool(fields[4], "browse expanded")?,
            }
        }
        "apply-settings" => SimCommand::ApplySettings(Box::new(parse_settings_command(&fields)?)),
        "restore-presentation" if fields.len() == 3 => SimCommand::RestorePresentationDefaults,
        "navigate-breadcrumb" if fields.len() == 5 => SimCommand::NavigateBreadcrumb {
            depth: fields[3]
                .parse::<usize>()
                .map_err(|_| "breadcrumb depth is not a usize")?,
            target_id: parse_stable_id(fields[4])?,
        },
        "toggle-fullscreen" if fields.len() == 3 => SimCommand::ToggleFullscreen,
        "open-settings" if fields.len() == 3 => SimCommand::OpenSettings,
        "close-settings" if fields.len() == 3 => SimCommand::CloseSettings,
        "simulate-device-loss" if fields.len() == 3 => SimCommand::SimulateDeviceLoss,
        command => return Err(format!("unknown or malformed command '{command}'")),
    };
    Ok(StampedCommand {
        frame,
        sim_time_s,
        command,
    })
}

fn parse_body_id(fields: &[&str], expected_len: usize) -> Result<String, String> {
    if fields.len() != expected_len {
        return Err("body command has the wrong field count".into());
    }
    parse_stable_id(fields[3])
}

fn parse_stable_id(id: &str) -> Result<String, String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err("body id is not a stable catalog id".into());
    }
    Ok(id.to_string())
}

fn parse_bool(value: &str, label: &str) -> Result<bool, String> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(format!("{label} must be 0 or 1")),
    }
}

fn parse_f64_bits(field: &str, label: &str) -> Result<f64, String> {
    let bits = u64::from_str_radix(field, 16).map_err(|_| format!("{label} is not f64 bits"))?;
    let value = f64::from_bits(bits);
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{label} is not finite"))
    }
}

fn quantize(value: f64, quantum: f64) -> i64 {
    (value / quantum).round() as i64
}

struct Fnv1a(u64);

impl Fnv1a {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&value.to_le_bytes());
    }

    fn i8(&mut self, value: i8) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.bytes(&value.to_le_bytes());
    }

    fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_catalog_text;
    use sim_core::time::{t_from_jd_tdb, T_MIN_S};

    const REAL_CATALOG: &str = include_str!("../../../assets/catalog.ron");
    const FRAME_DT_S: f64 = 1.0 / 60.0;
    const PORTABLE_REPLAY_HASH: u64 = 11_341_847_874_983_838_712;

    fn catalog() -> Catalog {
        load_catalog_text(REAL_CATALOG).expect("committed catalog must load")
    }

    #[test]
    fn framing_includes_planetary_moon_systems_and_the_eight_planet_view() {
        let loaded = LoadedCatalog::new(catalog());
        let jupiter = loaded.index_of("jupiter").unwrap();
        let jupiter_id = loaded.catalog.bodies[jupiter].id.as_str();
        let outermost_jovian_moon_km = loaded
            .catalog
            .bodies
            .iter()
            .filter(|body| {
                body.category == Category::Moon && body.parent.as_deref() == Some(jupiter_id)
            })
            .filter_map(|body| body.orbit.as_ref())
            .map(|orbit| orbit.elements.a_km.abs() * (1.0 + orbit.elements.e))
            .reduce(f64::max)
            .unwrap();
        assert_eq!(
            framing_distance_units(&loaded, jupiter),
            4.0 * outermost_jovian_moon_km / KM_PER_RENDER_UNIT
        );

        let outermost_planet_km = loaded
            .catalog
            .bodies
            .iter()
            .filter(|body| body.category == Category::Planet)
            .filter_map(|body| body.orbit.as_ref())
            .map(|orbit| orbit.elements.a_km.abs() * (1.0 + orbit.elements.e))
            .reduce(f64::max)
            .unwrap();
        assert_eq!(
            full_system_framing_distance_units(&loaded),
            4.0 * outermost_planet_km / KM_PER_RENDER_UNIT
        );
    }

    #[test]
    fn moving_io_travel_converges_then_follows_without_a_snap() {
        let loaded = LoadedCatalog::new(catalog());
        let sun = loaded.index_of("sun").unwrap();
        let io = loaded.index_of("io").unwrap();
        let mercury = loaded.index_of("mercury").unwrap();
        let mut clock = SimClock::new(StartMode::default(), 0.0);
        let mut states = crate::propagate_catalog(&loaded.catalog, clock.t()).unwrap();
        let mut camera = CameraController::new(
            sun,
            states.0[sun].position_km,
            DEFAULT_CAMERA_DISTANCE_UNITS,
        );

        consume_sim_command(
            &SimCommand::TravelToBody("io".into()),
            &mut clock,
            &mut camera,
            &loaded,
        );
        for _ in 0..30 {
            clock.tick(FRAME_DT_S, 0.0);
            propagate_into(&loaded.catalog, clock.t(), &mut states).unwrap();
            advance_camera_controller(&mut camera, &states, FRAME_DT_S);
        }
        assert!(camera.is_travelling());

        // A new selection starts from the in-flight f64 focus and replaces Io.
        let interrupted_focus = camera.focus_position_km();
        consume_sim_command(
            &SimCommand::SelectBody("mercury".into()),
            &mut clock,
            &mut camera,
            &loaded,
        );
        assert_eq!(camera.travel.unwrap().start_focus_km, interrupted_focus);
        assert_eq!(camera.selected_body_index(), mercury);

        for _ in 0..76 {
            clock.tick(FRAME_DT_S, 0.0);
            propagate_into(&loaded.catalog, clock.t(), &mut states).unwrap();
            advance_camera_controller(&mut camera, &states, FRAME_DT_S);
        }
        assert!(!camera.is_travelling());
        assert_eq!(camera.focus_body_index(), mercury);
        assert_eq!(camera.focus_position_km(), states.0[mercury].position_km);

        clock.tick(FRAME_DT_S, 0.0);
        propagate_into(&loaded.catalog, clock.t(), &mut states).unwrap();
        advance_camera_controller(&mut camera, &states, FRAME_DT_S);
        assert_eq!(camera.focus_position_km(), states.0[mercury].position_km);

        consume_sim_command(
            &SimCommand::TravelToBody("io".into()),
            &mut clock,
            &mut camera,
            &loaded,
        );
        for _ in 0..76 {
            clock.tick(FRAME_DT_S, 0.0);
            propagate_into(&loaded.catalog, clock.t(), &mut states).unwrap();
            advance_camera_controller(&mut camera, &states, FRAME_DT_S);
        }
        assert!(!camera.is_travelling());
        assert_eq!(camera.focus_body_index(), io);
        assert_eq!(camera.focus_position_km(), states.0[io].position_km);

        clock.tick(FRAME_DT_S, 0.0);
        propagate_into(&loaded.catalog, clock.t(), &mut states).unwrap();
        advance_camera_controller(&mut camera, &states, FRAME_DT_S);
        assert_eq!(camera.focus_position_km(), states.0[io].position_km);
    }

    #[test]
    fn dolly_clamps_at_body_surface_and_sedna_aphelion_limits() {
        let loaded = LoadedCatalog::new(catalog());
        let mercury = loaded.index_of("mercury").unwrap();
        let mut clock = SimClock::new(StartMode::default(), 0.0);
        let states = crate::propagate_catalog(&loaded.catalog, clock.t()).unwrap();
        let mut camera = CameraController::new(
            mercury,
            states.0[mercury].position_km,
            DEFAULT_CAMERA_DISTANCE_UNITS,
        );
        let (minimum, maximum) = zoom_limits(&loaded, mercury);

        for _ in 0..30 {
            consume_sim_command(
                &SimCommand::Dolly { delta: 100.0 },
                &mut clock,
                &mut camera,
                &loaded,
            );
        }
        assert_eq!(camera.distance_units(), minimum);

        for _ in 0..30 {
            consume_sim_command(
                &SimCommand::Dolly { delta: -100.0 },
                &mut clock,
                &mut camera,
                &loaded,
            );
        }
        assert_eq!(camera.distance_units(), maximum);
        assert!(maximum > 1.0e8, "full-system limit was {maximum}");
    }

    #[test]
    fn replay_round_trip_of_500_plus_mixed_commands_has_portable_state_hash() {
        let catalog = catalog();
        let mut original = HeadlessSimulation::new(&catalog).unwrap();
        let mut recording = CommandRecording::default();

        for frame in 0..600_u64 {
            let commands = mixed_commands(frame);
            original
                .step(FRAME_DT_S, &commands, Some(&mut recording))
                .unwrap();
        }
        assert!(recording.stream().entries.len() > 500);

        let serialized = recording.stream().to_text();
        let parsed = ReplayStream::from_text(&serialized).unwrap();
        assert_eq!(&parsed, recording.stream());
        let replayed = replay_headless(&catalog, &parsed, 600, FRAME_DT_S).unwrap();
        assert_eq!(replayed.frame(), original.frame());
        assert_eq!(replayed.state_hash(), original.state_hash());
        assert_eq!(original.state_hash(), PORTABLE_REPLAY_HASH);
    }

    #[test]
    fn replay_v2_reproduces_variable_wall_time_live_snap_and_view_commands() {
        let catalog = catalog();
        let mut original = HeadlessSimulation::new(&catalog).unwrap();
        let mut recording = CommandRecording::default();
        let mut wall_now_t = original.clock().t() + 30.0;

        for frame in 0..180_u64 {
            let wall_dt_s = match frame % 3 {
                0 => 1.0 / 48.0,
                1 => 1.0 / 60.0,
                _ => 1.0 / 90.0,
            };
            wall_now_t += wall_dt_s;
            let commands = match frame {
                0 => vec![
                    SimCommand::SetBodySize(BodySizeScale::X10),
                    SimCommand::SetMoonVisibility {
                        system_id: "jupiter".into(),
                        mode: MoonVisibilityMode::Major,
                    },
                    SimCommand::SetLocalOrbitVisibility {
                        body_id: "io".into(),
                        visible: false,
                    },
                ],
                1 => vec![SimCommand::SetLayerVisibility {
                    layer: LayerId::Icons,
                    visible: false,
                }],
                2 => vec![SimCommand::ApplySettings(Box::new(AppSettings {
                    units: crate::DistanceUnit::Miles,
                    ui_scale: 1.25,
                    ..AppSettings::default()
                }))],
                5 => vec![SimCommand::SetRate(RateIndex::new(12).unwrap())],
                20 => vec![SimCommand::SetRate(RateIndex::REAL), SimCommand::SnapToLive],
                30 => vec![
                    SimCommand::SetLeftPanelTab(LeftPanelTab::ViewOptions),
                    SimCommand::SetBrowseOpen(true),
                    SimCommand::SetBrowseColumnExpanded {
                        column: 1,
                        expanded: true,
                    },
                ],
                31 => vec![SimCommand::SetBrowseOpen(false)],
                _ => Vec::new(),
            };
            original
                .step_with_wall_time(wall_dt_s, wall_now_t, &commands, Some(&mut recording))
                .unwrap();
        }

        assert_eq!(recording.stream().frames.len(), 180);
        let encoded = recording.stream().to_text();
        assert!(encoded.starts_with("solar-sim-replay-v2\n"));
        let decoded = ReplayStream::from_text(&encoded).unwrap();
        let replayed = replay_headless(&catalog, &decoded, 180, FRAME_DT_S).unwrap();
        assert_eq!(replayed.state_hash(), original.state_hash());
        assert!(replayed.clock().is_live(wall_now_t));

        let mut invalid = decoded;
        invalid.frames[12].wall_dt_s = f64::NAN;
        assert!(matches!(
            replay_headless(&catalog, &invalid, 180, FRAME_DT_S),
            Err(ReplayRunError::InvalidFrameInput { frame: 12 })
        ));
    }

    #[test]
    fn corrupt_replay_inputs_are_rejected_without_panicking() {
        let text = concat!(
            "solar-sim-replay-v1\n",
            "bad|timestamp|play\n",
            "2|7ff0000000000000|dolly|0000000000000000\n",
            "3|0000000000000000|set-time|7ff0000000000000\n",
            "4|0000000000000000|layer|unknown|1\n",
            "5|0000000000000000|layer|labels|maybe\n"
        );
        let result = std::panic::catch_unwind(|| ReplayStream::from_text(text));
        assert!(result.is_ok());
        let message = result.unwrap().unwrap_err().to_string();
        assert!(message.contains("line 2"));
        assert!(message.contains("line 3"));
        assert!(message.contains("line 4"));
        assert!(message.contains("line 5"));
        assert!(message.contains("line 6"));

        let mut invalid_catalog = catalog();
        invalid_catalog.bodies.clear();
        let result = std::panic::catch_unwind(|| HeadlessSimulation::new(&invalid_catalog));
        assert!(result.is_ok());
        match result.unwrap() {
            Err(ReplayRunError::InvalidCatalog(errors)) => assert!(!errors.is_empty()),
            Err(other) => panic!("unexpected headless error: {other}"),
            Ok(_) => panic!("invalid catalog entered headless replay"),
        }
    }

    #[test]
    fn time_commands_round_trip_and_typed_clamps_emit_the_core_report() {
        let loaded = LoadedCatalog::new(catalog());
        let sun = loaded.index_of("sun").unwrap();
        let mut clock = SimClock::new(StartMode::default(), 0.0);
        let states = crate::propagate_catalog(&loaded.catalog, clock.t()).unwrap();
        let mut camera = CameraController::new(
            sun,
            states.0[sun].position_km,
            DEFAULT_CAMERA_DISTANCE_UNITS,
        );

        let target = T_MIN_S - 1.0;
        let report = consume_sim_command(
            &SimCommand::SetTime(target),
            &mut clock,
            &mut camera,
            &loaded,
        );
        assert_eq!(report.clamped, Some(sim_core::time::RangeEdge::AtMin));
        assert_eq!(clock.t(), T_MIN_S);

        let stream = ReplayStream {
            frames: Vec::new(),
            entries: vec![
                StampedCommand {
                    frame: 7,
                    sim_time_s: 123.5,
                    command: SimCommand::SetTime(target),
                },
                StampedCommand {
                    frame: 8,
                    sim_time_s: T_MIN_S,
                    command: SimCommand::SnapToLive,
                },
                StampedCommand {
                    frame: 9,
                    sim_time_s: T_MIN_S,
                    command: SimCommand::ToggleFullscreen,
                },
                StampedCommand {
                    frame: 10,
                    sim_time_s: T_MIN_S,
                    command: SimCommand::OpenSettings,
                },
                StampedCommand {
                    frame: 11,
                    sim_time_s: T_MIN_S,
                    command: SimCommand::CloseSettings,
                },
                StampedCommand {
                    frame: 12,
                    sim_time_s: T_MIN_S,
                    command: SimCommand::SimulateDeviceLoss,
                },
            ],
        };
        assert_eq!(ReplayStream::from_text(&stream.to_text()).unwrap(), stream);
    }

    fn mixed_commands(frame: u64) -> Vec<SimCommand> {
        let mut commands = vec![SimCommand::Orbit {
            delta_yaw: frame.rem_euclid(7) as f64 - 3.0,
            delta_pitch: frame.rem_euclid(5) as f64 - 2.0,
        }];
        if frame.is_multiple_of(3) {
            commands.push(SimCommand::Dolly {
                delta: if frame.is_multiple_of(2) { 0.2 } else { -0.2 },
            });
        }
        match frame {
            0 => commands.push(SimCommand::TravelToBody("io".into())),
            40 => commands.push(SimCommand::SelectBody("mercury".into())),
            100 => commands.push(SimCommand::TravelToBody("sedna".into())),
            180 => commands.push(SimCommand::TravelToBody("earth".into())),
            260 => commands.push(SimCommand::SelectBody("io".into())),
            340 => commands.push(SimCommand::TravelToBody("pluto".into())),
            420 => commands.push(SimCommand::TravelToBody("jupiter".into())),
            500 => commands.push(SimCommand::TravelToBody("io".into())),
            10 => commands.push(SimCommand::SetRate(RateIndex::new(2).unwrap())),
            90 => commands.push(SimCommand::StepRate(1)),
            150 => commands.push(SimCommand::Pause),
            151 => commands.push(SimCommand::Play),
            300 => commands.push(SimCommand::TogglePlay),
            301 => commands.push(SimCommand::TogglePlay),
            450 => commands.push(SimCommand::SetRate(RateIndex::REAL)),
            _ => {}
        }
        commands
    }

    #[test]
    fn replay_timestamp_is_seconds_since_j2000_tdb() {
        let mut simulation = HeadlessSimulation::new(&catalog()).unwrap();
        let mut recording = CommandRecording::default();
        simulation
            .step(FRAME_DT_S, &[SimCommand::Play], Some(&mut recording))
            .unwrap();
        assert_eq!(
            recording.stream().entries[0].sim_time_s,
            t_from_jd_tdb(2_461_042.0)
        );
    }

    #[test]
    fn recorded_layer_session_replays_to_the_same_final_layer_hash() {
        let catalog = catalog();
        let mut original = HeadlessSimulation::new(&catalog).unwrap();
        let mut recording = CommandRecording::default();
        let frames = [
            vec![
                SimCommand::SetLayerVisibility {
                    layer: LayerId::Planets,
                    visible: false,
                },
                SimCommand::SetLayerVisibility {
                    layer: LayerId::Labels,
                    visible: false,
                },
            ],
            vec![
                SimCommand::SetLayerVisibility {
                    layer: LayerId::Planets,
                    visible: false,
                },
                SimCommand::SetLayerVisibility {
                    layer: LayerId::Icons,
                    visible: false,
                },
            ],
            vec![
                SimCommand::SetLayerVisibility {
                    layer: LayerId::Labels,
                    visible: true,
                },
                SimCommand::SetLayerVisibility {
                    layer: LayerId::Orbits,
                    visible: false,
                },
            ],
        ];
        for commands in &frames {
            original
                .step(FRAME_DT_S, commands, Some(&mut recording))
                .unwrap();
        }

        let encoded = recording.stream().to_text();
        let decoded = ReplayStream::from_text(&encoded).unwrap();
        let replayed =
            replay_headless(&catalog, &decoded, frames.len() as u64, FRAME_DT_S).unwrap();

        assert_eq!(replayed.layer_state_hash(), original.layer_state_hash());
        assert_eq!(replayed.layer_state(), original.layer_state());
        assert!(!replayed.layer_state().is_visible(LayerId::Planets));
        assert!(replayed.layer_state().is_visible(LayerId::Labels));
        assert!(!replayed.layer_state().is_visible(LayerId::Icons));
        assert!(!replayed.layer_state().is_visible(LayerId::Orbits));
        assert!(replayed.layer_state().is_visible(LayerId::Moons));
    }
}
