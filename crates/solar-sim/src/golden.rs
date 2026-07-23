//! WP15 — deterministic renderer capture definitions for golden screenshots.
//!
//! The application owns the canonical camera/layer state and emits PPM so the
//! dependency-free xtask comparator can evaluate the same six views on Metal
//! and DX12. Captures wait for every referenced texture before reading back.

use bevy::{
    app::AppExit,
    asset::LoadState,
    ecs::system::SystemParam,
    prelude::*,
    render::render_resource::TextureFormat,
    render::renderer::RenderAdapterInfo,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    ui::UiSystems,
};
use std::{fs, path::PathBuf, time::Instant};

use crate::layers::{HudSurface, HudVisibilitySyncSet, UiRestoreAffordance};
use crate::ui_kit::SearchHint;
use crate::{BodySizeScale, LayerId, LayerState, SimulationSet, SimulationTickAdvance};
use sim_core::time::RateIndex;

type GoldenHudFilter = Or<(
    With<HudSurface>,
    With<UiRestoreAffordance>,
    With<SearchHint>,
)>;

pub const GOLDEN_WIDTH: u32 = 960;
pub const GOLDEN_HEIGHT: u32 = 600;
const MIN_SETTLE_FRAMES: u32 = 30;
const MIN_SETTLE_SECONDS: f64 = 5.0;
const RETRY_SETTLE_SECONDS: f64 = 2.0;
const MAX_SETTLE_SECONDS: f64 = 30.0;
const MAX_CAPTURE_ATTEMPTS: u8 = 3;
const EMPHASIS_REVIEW_REFERENCE_FPS: f64 = 30.0;

fn readback_settle_complete(
    settled_frames: u32,
    settled_seconds: f64,
    minimum_seconds: f64,
    all_loaded: bool,
) -> bool {
    all_loaded && settled_frames >= MIN_SETTLE_FRAMES && settled_seconds >= minimum_seconds
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenCaptureOptions {
    pub view: String,
    pub backend: String,
    pub output: PathBuf,
    pub reject_software_adapter: bool,
}

#[derive(Resource)]
pub(crate) struct GoldenRenderTarget(pub Handle<Image>);

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
    pub show_asteroids: bool,
    pub show_comets: bool,
    pub show_labels: bool,
    pub show_icons: bool,
    pub body_size: BodySizeScale,
    /// Capture-only fixed phase step used to review high-rate emphasis
    /// without advancing catalog truth or making the image wall-time-dependent.
    pub force_high_rate_emphasis: bool,
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
        show_asteroids: false,
        show_comets: false,
        show_labels: true,
        show_icons: true,
        body_size: BodySizeScale::X1,
        force_high_rate_emphasis: false,
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
        show_asteroids: false,
        show_comets: false,
        show_labels: false,
        show_icons: false,
        body_size: BodySizeScale::X1,
        force_high_rate_emphasis: false,
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
        show_asteroids: false,
        show_comets: false,
        show_labels: false,
        show_icons: false,
        body_size: BodySizeScale::X1,
        force_high_rate_emphasis: false,
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
        show_asteroids: false,
        show_comets: false,
        show_labels: true,
        show_icons: true,
        body_size: BodySizeScale::X1,
        force_high_rate_emphasis: false,
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
        show_asteroids: false,
        show_comets: false,
        show_labels: false,
        show_icons: false,
        body_size: BodySizeScale::X1,
        force_high_rate_emphasis: false,
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
        show_asteroids: false,
        show_comets: false,
        show_labels: false,
        show_icons: false,
        body_size: BodySizeScale::X1,
        force_high_rate_emphasis: false,
    },
];

pub const ORBIT_REVIEW_VIEWS: [GoldenViewSpec; 10] = [
    orbit_review_view("orbit-full-normal", "sun", None, 0.0, 0.35, false),
    orbit_review_view("orbit-full-emphasis", "sun", None, 0.0, 0.35, true),
    orbit_review_view(
        "orbit-belt-normal",
        "sun",
        Some(crate::BELT_REGION_FRAMING_DISTANCE_KM / crate::KM_PER_RENDER_UNIT),
        0.35,
        1.05,
        false,
    ),
    orbit_review_view(
        "orbit-belt-emphasis",
        "sun",
        Some(crate::BELT_REGION_FRAMING_DISTANCE_KM / crate::KM_PER_RENDER_UNIT),
        0.35,
        1.05,
        true,
    ),
    orbit_review_view(
        "orbit-jupiter-normal",
        "jupiter",
        Some(5_500.0),
        0.1,
        0.42,
        false,
    ),
    orbit_review_view(
        "orbit-jupiter-emphasis",
        "jupiter",
        Some(5_500.0),
        0.1,
        0.42,
        true,
    ),
    orbit_review_view(
        "orbit-saturn-normal",
        "saturn",
        Some(5_500.0),
        0.3,
        0.32,
        false,
    ),
    orbit_review_view(
        "orbit-saturn-emphasis",
        "saturn",
        Some(5_500.0),
        0.3,
        0.32,
        true,
    ),
    orbit_review_view(
        "orbit-comet-normal",
        "halley",
        Some(50_000.0),
        0.2,
        0.5,
        false,
    ),
    orbit_review_view(
        "orbit-comet-emphasis",
        "halley",
        Some(50_000.0),
        0.2,
        0.5,
        true,
    ),
];

const fn orbit_review_view(
    slug: &'static str,
    focus_id: &'static str,
    distance_units: Option<f64>,
    yaw_rad: f64,
    pitch_rad: f64,
    force_high_rate_emphasis: bool,
) -> GoldenViewSpec {
    GoldenViewSpec {
        slug,
        focus_id,
        yaw_rad,
        pitch_rad,
        distance_units,
        face_sun: true,
        show_ui: false,
        show_orbits: true,
        show_asteroids: true,
        show_comets: true,
        show_labels: true,
        show_icons: true,
        body_size: BodySizeScale::X1,
        force_high_rate_emphasis,
    }
}

pub const SCALE_REVIEW_VIEWS: [GoldenViewSpec; 14] = [
    scale_review_view("scale-ceres-floor-x1", "ceres", 50_000.0, BodySizeScale::X1),
    scale_review_view(
        "scale-ceres-floor-x10",
        "ceres",
        50_000.0,
        BodySizeScale::X10,
    ),
    scale_review_view(
        "scale-ceres-floor-x50",
        "ceres",
        50_000.0,
        BodySizeScale::X50,
    ),
    scale_review_view(
        "scale-earth-overview-x1",
        "earth",
        340_000.0,
        BodySizeScale::X1,
    ),
    scale_review_view(
        "scale-earth-overview-x10",
        "earth",
        340_000.0,
        BodySizeScale::X10,
    ),
    scale_review_view(
        "scale-earth-overview-x50",
        "earth",
        340_000.0,
        BodySizeScale::X50,
    ),
    scale_review_view(
        "scale-saturn-rings-x1",
        "saturn",
        10_000.0,
        BodySizeScale::X1,
    ),
    scale_review_view(
        "scale-saturn-rings-x10",
        "saturn",
        10_000.0,
        BodySizeScale::X10,
    ),
    scale_review_view(
        "scale-saturn-rings-x50",
        "saturn",
        10_000.0,
        BodySizeScale::X50,
    ),
    scale_review_view("scale-ceres-close-x1", "ceres", 10.0, BodySizeScale::X1),
    scale_review_view("scale-ceres-close-x10", "ceres", 10.0, BodySizeScale::X10),
    scale_review_view("scale-ceres-close-x50", "ceres", 10.0, BodySizeScale::X50),
    scale_review_view("appearance-pluto", "pluto", 10.0, BodySizeScale::X1),
    scale_review_view("appearance-charon", "charon", 6.0, BodySizeScale::X1),
];

const fn scale_review_view(
    slug: &'static str,
    focus_id: &'static str,
    distance_units: f64,
    body_size: BodySizeScale,
) -> GoldenViewSpec {
    GoldenViewSpec {
        slug,
        focus_id,
        yaw_rad: 0.16,
        pitch_rad: 0.18,
        distance_units: Some(distance_units),
        face_sun: true,
        show_ui: false,
        show_orbits: false,
        show_asteroids: false,
        show_comets: false,
        show_labels: false,
        show_icons: false,
        body_size,
        force_high_rate_emphasis: false,
    }
}

pub fn golden_view(slug: &str) -> Option<GoldenViewSpec> {
    GOLDEN_VIEWS
        .iter()
        .chain(&ORBIT_REVIEW_VIEWS)
        .chain(&SCALE_REVIEW_VIEWS)
        .copied()
        .find(|view| view.slug == slug)
}

pub(crate) fn layer_state_for_view(view: GoldenViewSpec) -> LayerState {
    let mut layers = LayerState::default();
    layers.set_visible(LayerId::Orbits, view.show_orbits);
    layers.set_visible(LayerId::Asteroids, view.show_asteroids);
    layers.set_visible(LayerId::Comets, view.show_comets);
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

#[derive(SystemParam)]
pub(crate) struct ReferencedTextureInputs<'w, 's> {
    asset_server: Res<'w, AssetServer>,
    materials: Res<'w, Assets<StandardMaterial>>,
    material_handles: Query<'w, 's, &'static MeshMaterial3d<StandardMaterial>>,
}

impl ReferencedTextureInputs<'_, '_> {
    /// The shared initial-readback condition: the render path has completed
    /// its minimum settling window, each material entity exists, and every
    /// referenced base-color image is loaded.
    pub(crate) fn ready_for_initial_readback(
        &self,
        settled_frames: u32,
        settled_seconds: f64,
    ) -> Result<bool, String> {
        self.ready_after_settle(settled_frames, settled_seconds, MIN_SETTLE_SECONDS)
    }

    fn ready_after_settle(
        &self,
        settled_frames: u32,
        settled_seconds: f64,
        minimum_seconds: f64,
    ) -> Result<bool, String> {
        let mut all_loaded = true;
        for material_handle in &self.material_handles {
            let Some(material) = self.materials.get(material_handle) else {
                all_loaded = false;
                continue;
            };
            let Some(texture) = material.base_color_texture.as_ref() else {
                continue;
            };
            match self.asset_server.load_state(texture) {
                LoadState::Failed(error) => {
                    return Err(format!("referenced texture failed to load: {error}"));
                }
                LoadState::Loaded => {}
                LoadState::NotLoaded | LoadState::Loading => all_loaded = false,
            }
        }
        Ok(readback_settle_complete(
            settled_frames,
            settled_seconds,
            minimum_seconds,
            all_loaded,
        ))
    }
}

#[derive(SystemParam)]
struct GoldenCaptureInputs<'w, 's> {
    textures: ReferencedTextureInputs<'w, 's>,
    target: Res<'w, GoldenRenderTarget>,
    adapter: Option<Res<'w, RenderAdapterInfo>>,
}

pub(crate) fn configure_golden_capture(app: &mut App, options: GoldenCaptureOptions) {
    let view = golden_view(&options.view);
    let hide_hud = view.is_some_and(|view| !view.show_ui);
    let target = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            GOLDEN_WIDTH,
            GOLDEN_HEIGHT,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    app.insert_resource(GoldenRenderTarget(target))
        .insert_resource(GoldenCaptureState {
            options,
            frames: 0,
            started: None,
            attempts: 0,
            requested: false,
        })
        .add_systems(Update, request_golden_capture.in_set(SimulationSet::Render));
    if view.is_some_and(|view| view.force_high_rate_emphasis) {
        app.add_systems(
            Update,
            force_golden_orbit_emphasis
                .in_set(SimulationSet::Render)
                .before(crate::scene_polish::OrbitEmphasisSet),
        );
    }
    if hide_hud {
        // The normal layer reconciliation owns both HUD surfaces and the
        // restore affordance. Override it before Bevy prepares UI nodes for
        // render extraction so `show_ui: false` is actually reviewable.
        app.add_systems(
            PostUpdate,
            hide_golden_hud
                .after(HudVisibilitySyncSet)
                .before(UiSystems::Prepare),
        );
    }
}

fn force_golden_orbit_emphasis(mut advance: ResMut<SimulationTickAdvance>) {
    *advance = SimulationTickAdvance::between(
        0.0,
        RateIndex::MAX.seconds_per_second() / EMPHASIS_REVIEW_REFERENCE_FPS,
    );
}

fn hide_golden_hud(
    roots: Query<Entity, GoldenHudFilter>,
    children: Query<&Children>,
    mut visibility: Query<&mut Visibility>,
) {
    let mut pending = roots.iter().collect::<Vec<_>>();
    while let Some(entity) = pending.pop() {
        if let Ok(mut entity_visibility) = visibility.get_mut(entity) {
            *entity_visibility = Visibility::Hidden;
        }
        if let Ok(entity_children) = children.get(entity) {
            pending.extend(entity_children.iter());
        }
    }
}

fn request_golden_capture(
    mut commands: Commands,
    mut state: ResMut<GoldenCaptureState>,
    inputs: GoldenCaptureInputs,
    mut exit: MessageWriter<AppExit>,
) {
    if state.requested {
        return;
    }
    if state.options.reject_software_adapter {
        let Some(adapter) = inputs.adapter else {
            error!(
                "golden '{}' cannot inspect RenderAdapterInfo",
                state.options.view
            );
            exit.write(AppExit::error());
            state.requested = true;
            return;
        };
        let device_type = format!("{:?}", adapter.device_type);
        if !crate::software_adapter_allowed(true, &device_type) {
            error!(
                "golden '{}' rejected software adapter '{}' with device_type {device_type}",
                state.options.view, adapter.name
            );
            exit.write(AppExit::error());
            state.requested = true;
            return;
        }
    }
    let started = *state.started.get_or_insert_with(Instant::now);
    state.frames += 1;
    let settled_seconds = started.elapsed().as_secs_f64();
    let settle_seconds = if state.attempts == 0 {
        MIN_SETTLE_SECONDS
    } else {
        RETRY_SETTLE_SECONDS
    };
    let ready =
        match inputs
            .textures
            .ready_after_settle(state.frames, settled_seconds, settle_seconds)
        {
            Ok(ready) => ready,
            Err(error) => {
                error!("golden '{}' {error}", state.options.view);
                exit.write(AppExit::error());
                state.requested = true;
                return;
            }
        };
    if settled_seconds > MAX_SETTLE_SECONDS {
        error!(
            "golden '{}' did not finish loading in {MAX_SETTLE_SECONDS:.0} seconds",
            state.options.view
        );
        exit.write(AppExit::error());
        state.requested = true;
        return;
    }
    if !ready {
        return;
    }
    info!(
        "capturing golden '{}' for {} to {}",
        state.options.view,
        state.options.backend,
        state.options.output.display()
    );
    commands
        .spawn(Screenshot::image(inputs.target.0.clone()))
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
                println!(
                    "golden-attempts view={} attempts={}",
                    state.options.view, state.attempts
                );
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

    fn show_test_hud(mut surfaces: Query<&mut Visibility, GoldenHudFilter>) {
        for mut visibility in &mut surfaces {
            *visibility = Visibility::Visible;
        }
    }

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
    fn orbit_review_views_pair_five_scenes_at_normal_and_emphasized_rates() {
        assert_eq!(ORBIT_REVIEW_VIEWS.len(), 10);
        let slugs = ORBIT_REVIEW_VIEWS
            .iter()
            .map(|view| view.slug)
            .collect::<HashSet<_>>();
        assert_eq!(slugs.len(), ORBIT_REVIEW_VIEWS.len());
        for prefix in [
            "orbit-full",
            "orbit-belt",
            "orbit-jupiter",
            "orbit-saturn",
            "orbit-comet",
        ] {
            let normal = golden_view(&format!("{prefix}-normal")).unwrap();
            let emphasized = golden_view(&format!("{prefix}-emphasis")).unwrap();
            assert!(!normal.force_high_rate_emphasis);
            assert!(emphasized.force_high_rate_emphasis);
            assert_eq!(
                GoldenViewSpec {
                    slug: emphasized.slug,
                    force_high_rate_emphasis: false,
                    ..emphasized
                },
                GoldenViewSpec {
                    slug: emphasized.slug,
                    ..normal
                }
            );
            assert!(normal.show_orbits);
            assert!(normal.show_asteroids);
            assert!(normal.show_comets);
            assert!(!normal.show_ui);
        }

        let catalog = crate::load_catalog_text(include_str!("../../../assets/catalog.ron"))
            .expect("committed catalog must load");
        let sun = catalog.bodies.iter().find(|body| body.id == "sun").unwrap();
        let halley = catalog
            .bodies
            .iter()
            .find(|body| body.id == "halley")
            .unwrap();
        let halley_period_s = halley
            .orbit
            .as_ref()
            .unwrap()
            .period_s(sun.gm_km3_s2.unwrap())
            .unwrap();
        let reviewed_phase_step = crate::scene_polish::phase_step_rad(
            RateIndex::MAX.seconds_per_second(),
            1.0 / EMPHASIS_REVIEW_REFERENCE_FPS,
            halley_period_s,
        );
        assert!(
            reviewed_phase_step >= crate::scene_polish::EMPHASIS_ENGAGE_PHASE_RAD,
            "the review cadence must actually engage Halley's orbit emphasis"
        );
    }

    #[test]
    fn scale_review_views_hold_camera_constant_across_one_ten_fifty() {
        assert_eq!(SCALE_REVIEW_VIEWS.len(), 14);
        let slugs = SCALE_REVIEW_VIEWS
            .iter()
            .map(|view| view.slug)
            .collect::<HashSet<_>>();
        assert_eq!(slugs.len(), SCALE_REVIEW_VIEWS.len());

        for prefix in [
            "scale-ceres-floor",
            "scale-earth-overview",
            "scale-saturn-rings",
            "scale-ceres-close",
        ] {
            let x1 = golden_view(&format!("{prefix}-x1")).unwrap();
            for (suffix, body_size) in [
                ("x1", BodySizeScale::X1),
                ("x10", BodySizeScale::X10),
                ("x50", BodySizeScale::X50),
            ] {
                let view = golden_view(&format!("{prefix}-{suffix}")).unwrap();
                assert_eq!(view.body_size, body_size);
                assert_eq!(
                    GoldenViewSpec {
                        slug: x1.slug,
                        body_size: BodySizeScale::X1,
                        ..view
                    },
                    x1
                );
            }
        }

        assert_eq!(
            golden_view("appearance-pluto").unwrap().body_size,
            BodySizeScale::X1
        );
        assert_eq!(
            golden_view("appearance-charon").unwrap().body_size,
            BodySizeScale::X1
        );
    }

    #[test]
    fn golden_render_target_is_fixed_size_srgb_and_cpu_readable() {
        let mut app = App::new();
        app.insert_resource(Assets::<Image>::default());
        configure_golden_capture(
            &mut app,
            GoldenCaptureOptions {
                view: "earth-texture".into(),
                backend: "test".into(),
                output: PathBuf::from("ignored.ppm"),
                reject_software_adapter: false,
            },
        );
        let target = app.world().resource::<GoldenRenderTarget>();
        let image = app
            .world()
            .resource::<Assets<Image>>()
            .get(&target.0)
            .unwrap();
        assert_eq!(image.width(), GOLDEN_WIDTH);
        assert_eq!(image.height(), GOLDEN_HEIGHT);
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        );
        assert!(image.data.is_some());
        assert!(!readback_settle_complete(
            MIN_SETTLE_FRAMES - 1,
            MIN_SETTLE_SECONDS,
            MIN_SETTLE_SECONDS,
            true,
        ));
        assert!(!readback_settle_complete(
            MIN_SETTLE_FRAMES,
            MIN_SETTLE_SECONDS - 0.001,
            MIN_SETTLE_SECONDS,
            true,
        ));
        assert!(!readback_settle_complete(
            MIN_SETTLE_FRAMES,
            MIN_SETTLE_SECONDS,
            MIN_SETTLE_SECONDS,
            false,
        ));
        assert!(readback_settle_complete(
            MIN_SETTLE_FRAMES,
            MIN_SETTLE_SECONDS,
            MIN_SETTLE_SECONDS,
            true,
        ));
    }

    #[test]
    fn ui_free_goldens_hide_hud_and_restore_after_post_update_reconciliation() {
        let mut app = App::new();
        app.insert_resource(Assets::<Image>::default());
        configure_golden_capture(
            &mut app,
            GoldenCaptureOptions {
                view: "orbit-full-normal".into(),
                backend: "test".into(),
                output: PathBuf::from("ignored.ppm"),
                reject_software_adapter: false,
            },
        );
        app.add_systems(PostUpdate, show_test_hud.in_set(HudVisibilitySyncSet));
        let hud = app
            .world_mut()
            .spawn((HudSurface, Visibility::Visible))
            .id();
        let hud_child = app.world_mut().spawn(Visibility::Visible).id();
        app.world_mut().entity_mut(hud).add_child(hud_child);
        let detached_search_hint = app
            .world_mut()
            .spawn((SearchHint, Visibility::Visible))
            .id();
        let restore = app
            .world_mut()
            .spawn((UiRestoreAffordance, Visibility::Visible))
            .id();

        app.world_mut().run_schedule(PostUpdate);

        assert_eq!(
            app.world().get::<Visibility>(hud),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().get::<Visibility>(hud_child),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().get::<Visibility>(detached_search_hint),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().get::<Visibility>(restore),
            Some(&Visibility::Hidden)
        );
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
        let belt = layer_state_for_view(golden_view("orbit-belt-normal").unwrap());
        assert!(belt.is_visible(LayerId::Asteroids));
        assert!(belt.is_visible(LayerId::Comets));
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
