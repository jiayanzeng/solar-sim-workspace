//! WP15 — deterministic renderer capture definitions for golden screenshots.
//!
//! The application owns the canonical camera/layer state and emits PPM so the
//! dependency-free xtask comparator can evaluate the same six views on Metal
//! and DX12. Captures wait for every referenced texture before reading back.

use bevy::{
    app::AppExit,
    asset::LoadState,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};
use std::{fs, path::PathBuf, time::Instant};

use crate::layers::{HudSurface, UiRestoreAffordance};
use crate::{LayerId, LayerState, SimulationSet};

pub const GOLDEN_WIDTH: u32 = 960;
pub const GOLDEN_HEIGHT: u32 = 600;
const MIN_SETTLE_FRAMES: u32 = 30;
const MIN_SETTLE_SECONDS: f64 = 5.0;
const RETRY_SETTLE_SECONDS: f64 = 2.0;
const MAX_SETTLE_SECONDS: f64 = 30.0;
const MAX_CAPTURE_ATTEMPTS: u8 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenCaptureOptions {
    pub view: String,
    pub backend: String,
    pub output: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoldenViewSpec {
    pub slug: &'static str,
    pub focus_id: &'static str,
    pub yaw_rad: f64,
    pub pitch_rad: f64,
    pub distance_units: Option<f64>,
    /// Orient the base camera vector toward the Sun from the focused body,
    /// then apply the stored yaw/pitch as reviewed offsets.
    pub face_sun: bool,
    pub show_ui: bool,
    pub show_orbits: bool,
    pub show_labels: bool,
    pub show_icons: bool,
}

pub const GOLDEN_VIEWS: [GoldenViewSpec; 6] = [
    GoldenViewSpec {
        slug: "full-system",
        focus_id: "sun",
        yaw_rad: 0.0,
        pitch_rad: 0.35,
        distance_units: None,
        face_sun: false,
        show_ui: true,
        show_orbits: true,
        show_labels: true,
        show_icons: true,
    },
    GoldenViewSpec {
        slug: "inner-orbits",
        focus_id: "sun",
        yaw_rad: 0.35,
        pitch_rad: 1.05,
        distance_units: Some(340_000.0),
        face_sun: false,
        show_ui: false,
        show_orbits: true,
        show_labels: false,
        show_icons: false,
    },
    GoldenViewSpec {
        slug: "earth-texture",
        focus_id: "earth",
        yaw_rad: 0.15,
        pitch_rad: 0.15,
        distance_units: Some(26.0),
        face_sun: true,
        show_ui: false,
        show_orbits: false,
        show_labels: false,
        show_icons: false,
    },
    GoldenViewSpec {
        slug: "jupiter-system",
        focus_id: "jupiter",
        yaw_rad: 0.1,
        pitch_rad: 0.42,
        distance_units: Some(5_500.0),
        face_sun: true,
        show_ui: false,
        show_orbits: true,
        show_labels: true,
        show_icons: true,
    },
    GoldenViewSpec {
        slug: "saturn-rings",
        focus_id: "saturn",
        yaw_rad: 0.3,
        pitch_rad: 0.32,
        distance_units: Some(430.0),
        face_sun: true,
        show_ui: false,
        show_orbits: false,
        show_labels: false,
        show_icons: false,
    },
    GoldenViewSpec {
        slug: "sun-bloom",
        focus_id: "sun",
        yaw_rad: 0.0,
        pitch_rad: 0.2,
        distance_units: Some(2_400.0),
        face_sun: false,
        show_ui: false,
        show_orbits: false,
        show_labels: false,
        show_icons: false,
    },
];

pub fn golden_view(slug: &str) -> Option<GoldenViewSpec> {
    GOLDEN_VIEWS.iter().copied().find(|view| view.slug == slug)
}

pub(crate) fn layer_state_for_view(view: GoldenViewSpec) -> LayerState {
    let mut layers = LayerState::default();
    layers.set_visible(LayerId::Orbits, view.show_orbits);
    layers.set_visible(LayerId::Labels, view.show_labels);
    layers.set_visible(LayerId::Icons, view.show_icons);
    layers
}

pub(crate) fn illuminated_pose(
    focus_km: [f64; 3],
    sun_km: [f64; 3],
    yaw_offset: f64,
    pitch_offset: f64,
) -> (f64, f64) {
    let delta = [
        sun_km[0] - focus_km[0],
        sun_km[1] - focus_km[1],
        sun_km[2] - focus_km[2],
    ];
    let yaw = delta[1].atan2(delta[0]) + yaw_offset;
    let pitch = delta[2].atan2(delta[0].hypot(delta[1])) + pitch_offset;
    (yaw, pitch.clamp(-1.45, 1.45))
}

#[derive(Resource)]
struct GoldenCaptureState {
    options: GoldenCaptureOptions,
    frames: u32,
    started: Option<Instant>,
    attempts: u8,
    requested: bool,
}

pub(crate) fn configure_golden_capture(app: &mut App, options: GoldenCaptureOptions) {
    let hide_hud = golden_view(&options.view).is_some_and(|view| !view.show_ui);
    app.insert_resource(GoldenCaptureState {
        options,
        frames: 0,
        started: None,
        attempts: 0,
        requested: false,
    })
    .add_systems(Update, request_golden_capture.in_set(SimulationSet::Render));
    if hide_hud {
        app.add_systems(
            Update,
            remove_golden_hud
                .in_set(SimulationSet::Render)
                .before(request_golden_capture),
        );
    }
}

fn remove_golden_hud(
    mut commands: Commands,
    hud: Query<Entity, With<HudSurface>>,
    restore: Query<Entity, With<UiRestoreAffordance>>,
) {
    for entity in hud.iter().chain(restore.iter()) {
        commands.entity(entity).despawn();
    }
}

fn request_golden_capture(
    mut commands: Commands,
    mut state: ResMut<GoldenCaptureState>,
    asset_server: Res<AssetServer>,
    materials: Res<Assets<StandardMaterial>>,
    material_handles: Query<&MeshMaterial3d<StandardMaterial>>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.requested {
        return;
    }
    let started = *state.started.get_or_insert_with(Instant::now);
    state.frames += 1;
    let mut all_loaded = true;
    for material_handle in &material_handles {
        let Some(material) = materials.get(material_handle) else {
            all_loaded = false;
            continue;
        };
        let Some(texture) = material.base_color_texture.as_ref() else {
            continue;
        };
        match asset_server.load_state(texture) {
            LoadState::Failed(error) => {
                error!(
                    "golden '{}' texture failed to load: {error}",
                    state.options.view
                );
                exit.write(AppExit::error());
                state.requested = true;
                return;
            }
            LoadState::Loaded => {}
            LoadState::NotLoaded | LoadState::Loading => all_loaded = false,
        }
    }
    if started.elapsed().as_secs_f64() > MAX_SETTLE_SECONDS {
        error!(
            "golden '{}' did not finish loading in {MAX_SETTLE_SECONDS:.0} seconds",
            state.options.view
        );
        exit.write(AppExit::error());
        state.requested = true;
        return;
    }
    let settle_seconds = if state.attempts == 0 {
        MIN_SETTLE_SECONDS
    } else {
        RETRY_SETTLE_SECONDS
    };
    if state.frames < MIN_SETTLE_FRAMES
        || started.elapsed().as_secs_f64() < settle_seconds
        || !all_loaded
    {
        return;
    }
    info!(
        "capturing golden '{}' for {} to {}",
        state.options.view,
        state.options.backend,
        state.options.output.display()
    );
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_ppm_and_exit(state.options.output.clone()));
    state.attempts += 1;
    state.requested = true;
}

fn save_ppm_and_exit(
    path: PathBuf,
) -> impl FnMut(On<ScreenshotCaptured>, MessageWriter<AppExit>, ResMut<GoldenCaptureState>) {
    move |captured, mut exit, mut state| {
        let decoded = captured
            .image
            .clone()
            .try_into_dynamic()
            .map_err(|error| error.to_string())
            .map(|image| image.to_rgb8());
        if let Ok(rgb) = decoded.as_ref() {
            if rgb.as_raw().iter().all(|channel| *channel == 0)
                && state.attempts < MAX_CAPTURE_ATTEMPTS
            {
                warn!(
                    "golden '{}' attempt {} was black; retrying after render-target pipeline specialization",
                    state.options.view,
                    state.attempts
                );
                state.frames = 0;
                state.started = Some(Instant::now());
                state.requested = false;
                return;
            }
        }
        let result = decoded
            .and_then(|rgb| encode_ppm(rgb.width(), rgb.height(), rgb.as_raw()))
            .and_then(|bytes| fs::write(&path, bytes).map_err(|error| error.to_string()));
        match result {
            Ok(()) => {
                info!("golden saved to {}", path.display());
                exit.write(AppExit::Success);
            }
            Err(error) => {
                error!("could not save golden '{}': {error}", path.display());
                exit.write(AppExit::error());
            }
        }
    }
}

fn encode_ppm(width: u32, height: u32, rgb: &[u8]) -> Result<Vec<u8>, String> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| "golden dimensions overflow".to_string())?;
    if width == 0 || height == 0 || rgb.len() != expected {
        return Err(format!(
            "golden RGB length {} does not match {width}x{height}",
            rgb.len()
        ));
    }
    if rgb.iter().all(|channel| *channel == 0) {
        return Err("golden is entirely black; render pipeline was not ready".into());
    }
    let header = format!("P6\n{width} {height}\n255\n");
    let mut bytes = Vec::with_capacity(header.len() + rgb.len());
    bytes.extend_from_slice(header.as_bytes());
    bytes.extend_from_slice(rgb);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn canonical_golden_views_are_exactly_six_unique_reviewed_scenes() {
        assert_eq!(GOLDEN_VIEWS.len(), 6);
        let slugs: HashSet<_> = GOLDEN_VIEWS.iter().map(|view| view.slug).collect();
        assert_eq!(slugs.len(), GOLDEN_VIEWS.len());
        for view in GOLDEN_VIEWS {
            assert_eq!(golden_view(view.slug), Some(view));
            assert!(view.distance_units.is_none_or(|distance| distance > 0.0));
        }
        assert!(golden_view("not-a-view").is_none());
    }

    #[test]
    fn view_layer_profiles_keep_full_system_hud_and_isolate_closeups() {
        let full = layer_state_for_view(golden_view("full-system").unwrap());
        assert!(full.is_visible(LayerId::UserInterface));
        assert!(full.is_visible(LayerId::Orbits));
        let earth = layer_state_for_view(golden_view("earth-texture").unwrap());
        assert!(earth.is_visible(LayerId::UserInterface));
        assert!(!golden_view("earth-texture").unwrap().show_ui);
        assert!(!earth.is_visible(LayerId::Orbits));
        assert!(!earth.is_visible(LayerId::Labels));
    }

    #[test]
    fn capture_encoder_is_strict_binary_ppm() {
        let bytes = encode_ppm(2, 1, &[1, 2, 3, 4, 5, 6]).unwrap();
        assert_eq!(&bytes[..11], b"P6\n2 1\n255\n");
        assert_eq!(&bytes[11..], &[1, 2, 3, 4, 5, 6]);
        assert!(encode_ppm(2, 1, &[0; 5]).is_err());
        assert!(encode_ppm(2, 1, &[0; 6]).is_err());
        assert!(encode_ppm(0, 1, &[]).is_err());
    }

    #[test]
    fn illuminated_pose_uses_the_parent_frame_sun_direction() {
        assert_eq!(
            illuminated_pose([0.0; 3], [2.0, 0.0, 0.0], 0.2, 0.3),
            (0.2, 0.3)
        );
        let (yaw, pitch) = illuminated_pose([0.0; 3], [0.0, 3.0, 0.0], 0.0, 0.0);
        assert!((yaw - std::f64::consts::FRAC_PI_2).abs() < 1.0e-12);
        assert_eq!(pitch, 0.0);
    }
}
