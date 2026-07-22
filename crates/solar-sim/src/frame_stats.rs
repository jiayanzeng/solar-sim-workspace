//! UIP-1 — opt-in CPU frame-time measurement and debug presentation.
//!
//! The shipped path does not install Bevy's frame-time diagnostics. They are
//! enabled only for an explicit frame-stats run or an ordinary debug build;
//! golden capture never installs the overlay.

#[cfg(debug_assertions)]
use crate::layers::HudSurface;
use crate::settings::{AppSettings, QualityPreset, RenderRecoveryPhase, RenderRecoveryStatus};
#[cfg(debug_assertions)]
use crate::ui_kit::{UiTheme, INTER_FONT_ASSET};
use crate::SimulationSet;
#[cfg(debug_assertions)]
use crate::TIME_BAR_HEIGHT_PX;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::window::PrimaryWindow;
use std::fmt::{self, Write as _};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(crate) const FRAME_STATS_SCHEMA: &str = "solar-sim-frame-stats-v1";

#[derive(Debug, Clone, PartialEq)]
pub struct FrameStatsOptions {
    pub duration_s: f64,
    pub output: PathBuf,
    pub view: Option<String>,
    pub quality: Option<QualityPreset>,
}

#[derive(Resource)]
struct FrameStatsCollector {
    options: FrameStatsOptions,
    samples_ms: Vec<f64>,
    sampled_duration_ms: f64,
    last_measurement_at: Option<Instant>,
    finished: bool,
}

impl FrameStatsCollector {
    fn new(options: FrameStatsOptions) -> Self {
        Self {
            options,
            samples_ms: Vec::new(),
            sampled_duration_ms: 0.0,
            last_measurement_at: None,
            finished: false,
        }
    }
}

#[cfg(debug_assertions)]
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiagnosticsOverlayState {
    visible: bool,
}

#[cfg(debug_assertions)]
impl DiagnosticsOverlayState {
    pub(crate) fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    #[cfg(test)]
    fn is_visible(self) -> bool {
        self.visible
    }
}

#[cfg(debug_assertions)]
#[derive(Component)]
struct DiagnosticsOverlayText;

#[derive(Debug, Clone, PartialEq)]
struct FrameStatsMetadata {
    view: String,
    width_px: u32,
    height_px: u32,
    msaa: String,
    quality: String,
    vsync: bool,
    frame_cap: String,
    adapter_name: String,
    adapter_type: String,
    adapter_backend: String,
    adapter_driver: String,
    adapter_driver_info: String,
    adapter_vendor: u32,
    adapter_device: u32,
}

#[derive(Debug, Clone, PartialEq)]
struct FrameStatsSummary {
    metadata: FrameStatsMetadata,
    duration_s: f64,
    frames: usize,
    min_ms: f64,
    mean_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    fps: f64,
}

#[derive(Debug)]
enum FrameStatsError {
    EmptySamples,
    NonFiniteSample,
    RuntimeMetadata(&'static str),
    Write { path: PathBuf, message: String },
}

impl fmt::Display for FrameStatsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySamples => f.write_str("frame-time diagnostic produced no samples"),
            Self::NonFiniteSample => {
                f.write_str("frame-time diagnostic produced a non-finite sample")
            }
            Self::RuntimeMetadata(field) => {
                write!(f, "frame-stats runtime metadata is unavailable: {field}")
            }
            Self::Write { path, message } => {
                write!(f, "could not write '{}': {message}", path.display())
            }
        }
    }
}

pub(crate) fn configure_diagnostics(
    app: &mut App,
    frame_stats: Option<FrameStatsOptions>,
    debug_overlay: bool,
) {
    if frame_stats.is_none() && !(cfg!(debug_assertions) && debug_overlay) {
        return;
    }
    app.add_plugins(FrameTimeDiagnosticsPlugin::default());
    if let Some(options) = frame_stats {
        app.insert_resource(FrameStatsCollector::new(options))
            .add_systems(
                Update,
                collect_frame_stats
                    .after(FrameTimeDiagnosticsPlugin::diagnostic_system)
                    .in_set(SimulationSet::Render),
            );
    }
    #[cfg(debug_assertions)]
    if debug_overlay {
        app.insert_resource(DiagnosticsOverlayState::default())
            .add_systems(Startup, spawn_diagnostics_overlay)
            .add_systems(
                Update,
                update_diagnostics_overlay
                    .after(FrameTimeDiagnosticsPlugin::diagnostic_system)
                    .in_set(SimulationSet::Render),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_frame_stats(
    diagnostics: Res<DiagnosticsStore>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<&Msaa, With<Camera3d>>,
    settings: Res<AppSettings>,
    recovery: Res<RenderRecoveryStatus>,
    adapter: Option<Res<RenderAdapterInfo>>,
    mut collector: ResMut<FrameStatsCollector>,
    mut exit: MessageWriter<AppExit>,
) {
    if collector.finished {
        return;
    }
    if recovery.phase() != RenderRecoveryPhase::Rendering {
        collector.finished = true;
        eprintln!(
            "frame-stats: rendering left the healthy state ({:?}); refusing invalid evidence",
            recovery.phase()
        );
        exit.write(AppExit::error());
        return;
    }
    let Some(measurement) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|diagnostic| diagnostic.measurement())
    else {
        return;
    };
    if collector.last_measurement_at == Some(measurement.time) {
        return;
    }
    collector.last_measurement_at = Some(measurement.time);
    if !measurement.value.is_finite() || measurement.value <= 0.0 {
        return;
    }
    collector.samples_ms.push(measurement.value);
    collector.sampled_duration_ms += measurement.value;
    if collector.sampled_duration_ms < collector.options.duration_s * 1_000.0 {
        return;
    }

    let result = metadata_from_runtime(
        &collector.options,
        &windows,
        &cameras,
        &settings,
        adapter.as_deref(),
    )
    .and_then(|metadata| summary_from_samples(&collector.samples_ms, metadata))
    .and_then(|summary| write_report(&collector.options.output, &summary, &collector.samples_ms));
    collector.finished = true;
    match result {
        Ok(line) => {
            println!("{line}");
            exit.write(AppExit::Success);
        }
        Err(error) => {
            eprintln!("frame-stats: {error}");
            exit.write(AppExit::error());
        }
    }
}

fn metadata_from_runtime(
    options: &FrameStatsOptions,
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<&Msaa, With<Camera3d>>,
    settings: &AppSettings,
    adapter: Option<&RenderAdapterInfo>,
) -> Result<FrameStatsMetadata, FrameStatsError> {
    let window = windows
        .iter()
        .next()
        .ok_or(FrameStatsError::RuntimeMetadata("primary window"))?;
    let msaa = cameras
        .iter()
        .next()
        .ok_or(FrameStatsError::RuntimeMetadata("camera MSAA"))?;
    let adapter = adapter.ok_or(FrameStatsError::RuntimeMetadata("render adapter"))?;
    Ok(FrameStatsMetadata {
        view: options.view.clone().unwrap_or_else(|| "interactive".into()),
        width_px: window.physical_width(),
        height_px: window.physical_height(),
        msaa: msaa_label(*msaa).into(),
        quality: settings.quality.slug().into(),
        vsync: settings.vsync,
        frame_cap: settings.frame_cap.slug().into(),
        adapter_name: adapter.name.clone(),
        adapter_type: format!("{:?}", adapter.device_type),
        adapter_backend: adapter.backend.to_string(),
        adapter_driver: adapter.driver.clone(),
        adapter_driver_info: adapter.driver_info.clone(),
        adapter_vendor: adapter.vendor,
        adapter_device: adapter.device,
    })
}

fn msaa_label(msaa: Msaa) -> &'static str {
    match msaa {
        Msaa::Off => "off",
        Msaa::Sample2 => "2x",
        Msaa::Sample4 => "4x",
        Msaa::Sample8 => "8x",
    }
}

fn summary_from_samples(
    samples_ms: &[f64],
    metadata: FrameStatsMetadata,
) -> Result<FrameStatsSummary, FrameStatsError> {
    if samples_ms.is_empty() {
        return Err(FrameStatsError::EmptySamples);
    }
    if samples_ms
        .iter()
        .any(|sample| !sample.is_finite() || *sample <= 0.0)
    {
        return Err(FrameStatsError::NonFiniteSample);
    }
    let mut sorted = samples_ms.to_vec();
    sorted.sort_by(f64::total_cmp);
    let duration_ms = samples_ms.iter().sum::<f64>();
    let mean_ms = duration_ms / samples_ms.len() as f64;
    Ok(FrameStatsSummary {
        metadata,
        duration_s: duration_ms / 1_000.0,
        frames: samples_ms.len(),
        min_ms: sorted[0],
        mean_ms,
        p95_ms: nearest_rank(&sorted, 95),
        p99_ms: nearest_rank(&sorted, 99),
        fps: 1_000.0 / mean_ms,
    })
}

fn nearest_rank(sorted: &[f64], percentile: usize) -> f64 {
    let rank = (percentile * sorted.len()).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn summary_line(summary: &FrameStatsSummary) -> String {
    let metadata = &summary.metadata;
    format!(
        concat!(
            "{{\"schema\":\"{}\",\"view\":{},\"duration_s\":{:.6},",
            "\"frames\":{},\"min_ms\":{:.6},\"mean_ms\":{:.6},",
            "\"p95_ms\":{:.6},\"p99_ms\":{:.6},\"fps\":{:.3},",
            "\"width_px\":{},\"height_px\":{},\"msaa\":{},",
            "\"quality\":{},\"vsync\":{},\"frame_cap\":{},",
            "\"adapter_name\":{},\"adapter_type\":{},\"backend\":{},",
            "\"adapter_driver\":{},\"adapter_driver_info\":{},",
            "\"adapter_vendor\":{},\"adapter_device\":{}}}"
        ),
        FRAME_STATS_SCHEMA,
        json_string(&metadata.view),
        summary.duration_s,
        summary.frames,
        summary.min_ms,
        summary.mean_ms,
        summary.p95_ms,
        summary.p99_ms,
        summary.fps,
        metadata.width_px,
        metadata.height_px,
        json_string(&metadata.msaa),
        json_string(&metadata.quality),
        metadata.vsync,
        json_string(&metadata.frame_cap),
        json_string(&metadata.adapter_name),
        json_string(&metadata.adapter_type),
        json_string(&metadata.adapter_backend),
        json_string(&metadata.adapter_driver),
        json_string(&metadata.adapter_driver_info),
        metadata.adapter_vendor,
        metadata.adapter_device,
    )
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            control if control <= '\u{1f}' => {
                let _ = write!(output, "\\u{:04x}", control as u32);
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn csv_text(samples_ms: &[f64]) -> String {
    let mut output = String::from("frame,frame_time_ms\n");
    for (index, sample) in samples_ms.iter().enumerate() {
        let _ = writeln!(output, "{},{sample:.6}", index + 1);
    }
    output
}

fn raw_csv_path(output: &Path) -> PathBuf {
    let mut path = output.as_os_str().to_os_string();
    path.push(".csv");
    PathBuf::from(path)
}

fn write_report(
    output: &Path,
    summary: &FrameStatsSummary,
    samples_ms: &[f64],
) -> Result<String, FrameStatsError> {
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| FrameStatsError::Write {
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    let line = summary_line(summary);
    fs::write(output, format!("{line}\n")).map_err(|error| FrameStatsError::Write {
        path: output.to_path_buf(),
        message: error.to_string(),
    })?;
    let csv_path = raw_csv_path(output);
    fs::write(&csv_path, csv_text(samples_ms)).map_err(|error| FrameStatsError::Write {
        path: csv_path.clone(),
        message: error.to_string(),
    })?;
    Ok(line)
}

#[cfg(debug_assertions)]
fn spawn_diagnostics_overlay(
    mut commands: Commands,
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Text::new("frame: -- ms | mean: -- ms"),
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
        Visibility::Hidden,
        GlobalZIndex(110),
        AccessibleLabel::new("Frame-time diagnostic; toggle with F10"),
        HudSurface,
        DiagnosticsOverlayText,
    ));
}

#[cfg(debug_assertions)]
fn update_diagnostics_overlay(
    state: Res<DiagnosticsOverlayState>,
    diagnostics: Res<DiagnosticsStore>,
    mut overlay: Query<(&mut Text, &mut Visibility), With<DiagnosticsOverlayText>>,
) {
    let desired_visibility = if state.visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    let frame_time = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|diagnostic| diagnostic.value());
    let mean = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|diagnostic| diagnostic.average());
    for (mut text, mut visibility) in &mut overlay {
        if *visibility != desired_visibility {
            *visibility = desired_visibility;
        }
        if state.visible {
            if let (Some(frame_time), Some(mean)) = (frame_time, mean) {
                let desired = format!("frame: {frame_time:.2} ms | mean: {mean:.2} ms");
                if **text != desired {
                    **text = desired;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> FrameStatsMetadata {
        FrameStatsMetadata {
            view: "full-system".into(),
            width_px: 3456,
            height_px: 2160,
            msaa: "4x".into(),
            quality: "high".into(),
            vsync: true,
            frame_cap: "120".into(),
            adapter_name: "Apple M2 Pro \"GPU\"".into(),
            adapter_type: "IntegratedGpu".into(),
            adapter_backend: "metal".into(),
            adapter_driver: "".into(),
            adapter_driver_info: "line 1\nline 2".into(),
            adapter_vendor: 0,
            adapter_device: 0,
        }
    }

    #[test]
    fn absent_frame_stats_installs_no_measurement_resource() {
        let mut app = App::new();
        configure_diagnostics(&mut app, None, false);

        assert!(app.world().get_resource::<FrameStatsCollector>().is_none());
        assert!(app
            .world()
            .get_resource::<DiagnosticsOverlayState>()
            .is_none());
        assert!(app.world().get_resource::<DiagnosticsStore>().is_none());
    }

    #[test]
    fn summary_schema_is_an_exact_golden_line() {
        let summary = summary_from_samples(&[10.0, 20.0, 30.0, 40.0], metadata()).unwrap();
        assert_eq!(
            summary_line(&summary),
            concat!(
                "{\"schema\":\"solar-sim-frame-stats-v1\",\"view\":\"full-system\",",
                "\"duration_s\":0.100000,\"frames\":4,\"min_ms\":10.000000,",
                "\"mean_ms\":25.000000,\"p95_ms\":40.000000,",
                "\"p99_ms\":40.000000,\"fps\":40.000,\"width_px\":3456,",
                "\"height_px\":2160,\"msaa\":\"4x\",\"quality\":\"high\",",
                "\"vsync\":true,\"frame_cap\":\"120\",",
                "\"adapter_name\":\"Apple M2 Pro \\\"GPU\\\"\",",
                "\"adapter_type\":\"IntegratedGpu\",\"backend\":\"metal\",",
                "\"adapter_driver\":\"\",",
                "\"adapter_driver_info\":\"line 1\\nline 2\",",
                "\"adapter_vendor\":0,\"adapter_device\":0}"
            )
        );
    }

    #[test]
    fn percentiles_use_nearest_rank_over_the_complete_series() {
        let samples = (1..=100).map(|value| value as f64).collect::<Vec<_>>();
        let summary = summary_from_samples(&samples, metadata()).unwrap();

        assert_eq!(summary.min_ms, 1.0);
        assert_eq!(summary.mean_ms, 50.5);
        assert_eq!(summary.p95_ms, 95.0);
        assert_eq!(summary.p99_ms, 99.0);
    }

    #[test]
    fn raw_series_csv_has_a_stable_header_and_fixed_precision() {
        assert_eq!(
            csv_text(&[8.0, 8.125]),
            "frame,frame_time_ms\n1,8.000000\n2,8.125000\n"
        );
        assert_eq!(
            raw_csv_path(Path::new("target/perf/full-system-high.json")),
            PathBuf::from("target/perf/full-system-high.json.csv")
        );
    }

    #[test]
    fn overlay_is_hidden_until_the_recorded_toggle_command_applies() {
        let mut state = DiagnosticsOverlayState::default();
        assert!(!state.is_visible());
        state.toggle();
        assert!(state.is_visible());
        state.toggle();
        assert!(!state.is_visible());
    }
}
