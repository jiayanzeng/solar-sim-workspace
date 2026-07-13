//! WP5/WP8/WP9 — deterministic command consumption, camera travel, time, and replay.
//!
//! This module is the user-state mutation boundary. `CameraController` keeps
//! its fields private, and `consume_sim_command` is the only function that
//! applies user intent to simulation state. Per-frame clock, propagation, and
//! moving-focus evolution are deterministic updates driven by explicit inputs.

use crate::{
    propagate_into, BodyStates, LoadedCatalog, PropagationError, DEFAULT_CAMERA_DISTANCE_UNITS,
    KM_PER_RENDER_UNIT,
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
    Orbit { delta_yaw: f64, delta_pitch: f64 },
    Dolly { delta: f64 },
    SetTime(f64),
    SetRate(RateIndex),
    StepRate(i8),
    Play,
    Pause,
    TogglePlay,
    SnapToLive,
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
    }
    report
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
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReplayStream {
    pub entries: Vec<StampedCommand>,
}

impl ReplayStream {
    const HEADER: &'static str = "solar-sim-replay-v1";

    pub fn to_text(&self) -> String {
        let mut output = String::from(Self::HEADER);
        output.push('\n');
        for entry in &self.entries {
            output.push_str(&serialize_entry(entry));
            output.push('\n');
        }
        output
    }

    pub fn from_text(text: &str) -> Result<Self, ReplayParseError> {
        let mut lines = text.lines();
        if lines.next() != Some(Self::HEADER) {
            return Err(ReplayParseError(vec!["missing replay-v1 header".into()]));
        }
        let mut entries = Vec::new();
        let mut errors = Vec::new();
        for (index, line) in lines.enumerate() {
            if line.is_empty() {
                continue;
            }
            match parse_entry(line) {
                Ok(entry) => entries.push(entry),
                Err(message) => errors.push(format!("line {}: {message}", index + 2)),
            }
        }
        if errors.is_empty() {
            Ok(Self { entries })
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

    pub fn step(
        &mut self,
        wall_dt_s: f64,
        commands: &[SimCommand],
        mut recording: Option<&mut CommandRecording>,
    ) -> Result<(), PropagationError> {
        for command in commands {
            if let Some(recorder) = recording.as_deref_mut() {
                recorder.record(self.frame, self.clock.t(), command.clone());
            }
            consume_sim_command(command, &mut self.clock, &mut self.camera, &self.loaded);
        }
        self.wall_now_t += wall_dt_s;
        self.clock.tick(wall_dt_s, self.wall_now_t);
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

pub fn replay_headless(
    catalog: &Catalog,
    stream: &ReplayStream,
    total_frames: u64,
    wall_dt_s: f64,
) -> Result<HeadlessSimulation, ReplayRunError> {
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
        simulation
            .step(wall_dt_s, &commands, None)
            .map_err(ReplayRunError::Propagation)?;
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
    }
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
    let id = fields[3];
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err("body id is not a stable catalog id".into());
    }
    Ok(id.to_string())
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
    const PORTABLE_REPLAY_HASH: u64 = 11_614_332_433_107_791_956;

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
    fn corrupt_replay_inputs_are_rejected_without_panicking() {
        let text = concat!(
            "solar-sim-replay-v1\n",
            "bad|timestamp|play\n",
            "2|7ff0000000000000|dolly|0000000000000000\n",
            "3|0000000000000000|set-time|7ff0000000000000\n"
        );
        let result = std::panic::catch_unwind(|| ReplayStream::from_text(text));
        assert!(result.is_ok());
        let message = result.unwrap().unwrap_err().to_string();
        assert!(message.contains("line 2"));
        assert!(message.contains("line 3"));
        assert!(message.contains("line 4"));

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
}
