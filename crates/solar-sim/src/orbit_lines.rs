//! WP6/UIO-2/UIP-7 — retained orbit hierarchy and bounded reuse (Rev E §10.2).
//!
//! Orbit geometry stays in f64 kilometers in the orbiting body's parent
//! frame (Rev C §3 invariant 6). Only the retained gizmo asset contains f32
//! render vertices; its entity translation is independently rebased around
//! the camera focus each frame. Ellipses use uniform true-anomaly spacing,
//! which puts shorter chords near perihelion, while the hyperbolic branch is
//! a strictly open ±25-Julian-year arc centered on perihelion (Rev C §10.2).
//!
//! The cache key remains the complete drifted [`Elements`], effective mean
//! motion, and parent GM. Non-secular paths retain exact-key reuse. For the
//! eight secular planet paths, UIP-7 supersedes the former "no screen-space
//! approximations" note by ruling: retained geometry may lag a fresh sample
//! only while an analytic element-drift bound stays below one quarter logical
//! pixel at the current conservative camera scale. The bound accumulates from
//! the last sampled key, so crossing it deterministically refreshes the path.

use crate::scene_polish::OrbitEmphasisSet;
use crate::selection::{SelectionAccent, SelectionAccentSet, SELECTION_ACCENT_RGB};
use crate::{
    left_panel::body_passes_contextual_moon_visibility, rebase_position, BodyStates,
    CameraController, LayerId, LayerState, LoadedCatalog, OrbitEmphasisState, SimulationClock,
    SimulationSet, ViewOptionsState,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use sim_core::catalog::{BodyRecord, Category, Elements, Orbit};
use sim_core::kepler::{elements_at, solve_hyperbolic, state_from_elements, KeplerError};
use sim_core::time::JULIAN_YEAR_S;

pub const MIN_ORBIT_VERTICES: usize = 256;
pub const MAX_ORBIT_VERTICES: usize = 768;
pub const HYPERBOLIC_HALF_SPAN_S: f64 = 25.0 * JULIAN_YEAR_S;

pub const ORBIT_LINE_BASE_WIDTH_LOGICAL_PX: f32 = 1.5;
// A small negative bias brings a line forward just enough to avoid flicker
// where the path crosses its body without turning it into an overlay.
const ORBIT_DEPTH_BIAS: f32 = -0.001;
const FADE_OUT_ANGULAR_RADIUS: f64 = 0.000_1;
const FULL_ALPHA_ANGULAR_RADIUS: f64 = 0.002;
const EDGE_ON_ALPHA_FACTOR: f64 = 0.2;
const MAX_TEMPORAL_REUSE_ERROR_LOGICAL_PX: f64 = 0.25;

/// Rev E's category hierarchy. The star has no orbit path.
pub const fn orbit_line_width_logical_px(category: Category) -> f32 {
    match category {
        Category::Star => 0.0,
        Category::Planet => ORBIT_LINE_BASE_WIDTH_LOGICAL_PX * 3.0,
        Category::DwarfPlanet | Category::Moon => ORBIT_LINE_BASE_WIDTH_LOGICAL_PX * 2.0,
        Category::Asteroid | Category::Comet => ORBIT_LINE_BASE_WIDTH_LOGICAL_PX,
    }
}

/// Parent-frame f64 truth for one sampled conic.
#[derive(Debug, Clone, PartialEq)]
pub struct OrbitPath {
    vertices_parent_km: Vec<[f64; 3]>,
    time_offsets_from_perihelion_s: Option<Vec<f64>>,
    cache_key: OrbitGeometryCacheKey,
    plane_normal: [f64; 3],
    characteristic_radius_km: f64,
}

/// Complete exact inputs that can change sampled conic geometry.
///
/// The catalog is immutable after startup, but retaining all three inputs in
/// the key makes the reuse proof explicit for both fitted mean motion and the
/// two-body parent-GM path.
#[derive(Debug, Clone, Copy, PartialEq)]
struct OrbitGeometryCacheKey {
    elements: Elements,
    mean_motion_rad_per_s: f64,
    mu_parent_km3_s2: f64,
}

impl OrbitPath {
    pub fn vertices_parent_km(&self) -> &[[f64; 3]] {
        &self.vertices_parent_km
    }

    /// Present only for the hyperbolic branch. Its first and last values are
    /// exactly -25 and +25 Julian years from perihelion.
    pub fn time_offsets_from_perihelion_s(&self) -> Option<&[f64]> {
        self.time_offsets_from_perihelion_s.as_deref()
    }

    pub fn is_closed(&self) -> bool {
        self.time_offsets_from_perihelion_s.is_none()
            && self.vertices_parent_km.first() == self.vertices_parent_km.last()
    }

    pub fn elements(&self) -> Elements {
        self.cache_key.elements
    }
}

/// Render brightness input reserved for WP13's orbit-emphasis mode. Values
/// above one brighten RGB without changing geometry or simulation state.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct OrbitLineBrightness(pub f32);

impl Default for OrbitLineBrightness {
    fn default() -> Self {
        Self(1.0)
    }
}

impl OrbitLineBrightness {
    fn sanitized(self) -> f32 {
        if self.0.is_finite() {
            self.0.max(0.0)
        } else {
            0.0
        }
    }
}

/// Vertex count rises linearly with eccentricity and stays inside the WP6
/// contract. Hyperbolas use 767 points so perihelion is an exact center
/// vertex as well as both ±25-year endpoints being exact vertices.
pub fn orbit_vertex_count(e: f64) -> usize {
    if !e.is_finite() {
        return MIN_ORBIT_VERTICES;
    }
    let fraction = e.clamp(0.0, 1.0);
    let count = (MIN_ORBIT_VERTICES as f64
        + (MAX_ORBIT_VERTICES - MIN_ORBIT_VERTICES) as f64 * fraction)
        .round() as usize;
    if e > 1.0 && count.is_multiple_of(2) {
        count - 1
    } else {
        count
    }
}

/// Samples the current osculating conic from the same drifted elements and
/// fitted/two-body mean motion used by `sim_core::kepler::state_at`.
pub fn sample_orbit(
    orbit: &Orbit,
    mu_parent_km3_s2: f64,
    current_t_s: f64,
) -> Result<OrbitPath, KeplerError> {
    let cache_key = orbit_geometry_cache_key(orbit, mu_parent_km3_s2, current_t_s)?;
    sample_orbit_from_key(cache_key)
}

fn orbit_geometry_cache_key(
    orbit: &Orbit,
    mu_parent_km3_s2: f64,
    current_t_s: f64,
) -> Result<OrbitGeometryCacheKey, KeplerError> {
    if !current_t_s.is_finite() {
        return Err(KeplerError::NonFinite);
    }
    let elements = elements_at(orbit, current_t_s);
    let mean_motion_rad_per_s = orbit.mean_motion_rad_per_s(mu_parent_km3_s2);
    // Validate before the hyperbolic endpoint solve so both branches preserve
    // `state_at`'s error semantics for bad elements, mean motion, and parent GM.
    state_from_elements(&elements, mu_parent_km3_s2, mean_motion_rad_per_s, 0.0)?;
    Ok(OrbitGeometryCacheKey {
        elements,
        mean_motion_rad_per_s,
        mu_parent_km3_s2,
    })
}

fn sample_orbit_from_key(cache_key: OrbitGeometryCacheKey) -> Result<OrbitPath, KeplerError> {
    if cache_key.elements.is_hyperbolic() {
        sample_hyperbolic(cache_key)
    } else {
        sample_elliptic(cache_key)
    }
}

fn retained_orbit_path(
    path: &OrbitPath,
    orbit: &Orbit,
    mu_parent_km3_s2: f64,
    current_t_s: f64,
    maximum_drift_km: f64,
) -> Result<Option<OrbitPath>, KeplerError> {
    let cache_key = orbit_geometry_cache_key(orbit, mu_parent_km3_s2, current_t_s)?;
    if cache_key == path.cache_key {
        return Ok(None);
    }

    let exact_non_element_inputs_match = cache_key.mean_motion_rad_per_s
        == path.cache_key.mean_motion_rad_per_s
        && cache_key.mu_parent_km3_s2 == path.cache_key.mu_parent_km3_s2;
    if orbit.secular.is_some()
        && exact_non_element_inputs_match
        && orbit_vertex_count(path.cache_key.elements.e) == orbit_vertex_count(cache_key.elements.e)
        && secular_vertex_drift_bound_km(path.cache_key.elements, cache_key.elements)
            < maximum_drift_km
    {
        return Ok(None);
    }

    sample_orbit_from_key(cache_key).map(Some)
}

/// Conservative displacement bound for samples at the same true anomaly.
/// The maximum of the cached/current `a` and `e` keeps the linearized terms
/// conservative in either drift direction; angular deltas intentionally stay
/// unwrapped because shortening them modulo one turn could understate drift.
fn secular_vertex_drift_bound_km(cached: Elements, current: Elements) -> f64 {
    let a_km = cached.a_km.abs().max(current.a_km.abs());
    let e = cached.e.max(current.e);
    let delta_a_km = (current.a_km - cached.a_km).abs();
    let delta_e = (current.e - cached.e).abs();
    let angular_delta_rad = (current.i_deg - cached.i_deg).abs().to_radians()
        + (current.raan_deg - cached.raan_deg).abs().to_radians()
        + (current.argp_deg - cached.argp_deg).abs().to_radians();
    let bound_km = delta_a_km * (1.0 + e)
        + a_km * (delta_e * (1.0 + e))
        + a_km * (1.0 + e) * angular_delta_rad;
    if bound_km.is_finite() && bound_km >= 0.0 {
        bound_km
    } else {
        f64::INFINITY
    }
}

/// Converts the quarter-logical-pixel contract to kilometers at the closest
/// possible point on the cached orbit's bounding sphere. Solving with the
/// bound on both displacement and closest depth avoids spending pixels that
/// the candidate drift itself could consume.
fn temporal_reuse_tolerance_km(
    camera_distance_km: f64,
    characteristic_radius_km: f64,
    projection: &Projection,
    viewport_height_logical_px: f64,
) -> f64 {
    let Projection::Perspective(perspective) = projection else {
        return 0.0;
    };
    let closest_depth_km = camera_distance_km - characteristic_radius_km;
    let half_fov_tangent = (f64::from(perspective.fov) * 0.5).tan();
    if !closest_depth_km.is_finite()
        || closest_depth_km <= 0.0
        || !viewport_height_logical_px.is_finite()
        || viewport_height_logical_px <= 0.0
        || !half_fov_tangent.is_finite()
        || half_fov_tangent <= 0.0
    {
        return 0.0;
    }
    let depth_fraction =
        2.0 * half_fov_tangent * MAX_TEMPORAL_REUSE_ERROR_LOGICAL_PX / viewport_height_logical_px;
    depth_fraction * closest_depth_km / (1.0 + depth_fraction)
}

fn sample_elliptic(cache_key: OrbitGeometryCacheKey) -> Result<OrbitPath, KeplerError> {
    let elements = cache_key.elements;
    let count = orbit_vertex_count(elements.e);
    let unique_count = count - 1;
    let mut vertices = Vec::with_capacity(count);

    for index in 0..unique_count {
        let true_anomaly = std::f64::consts::TAU * index as f64 / unique_count as f64;
        let (sin_nu, cos_nu) = true_anomaly.sin_cos();
        let eccentric_anomaly =
            ((1.0 - elements.e * elements.e).sqrt() * sin_nu).atan2(elements.e + cos_nu);
        let mean_anomaly = eccentric_anomaly - elements.e * eccentric_anomaly.sin();
        vertices.push(
            state_from_elements(
                &elements,
                cache_key.mu_parent_km3_s2,
                cache_key.mean_motion_rad_per_s,
                mean_anomaly,
            )?
            .position_km,
        );
    }
    // Copy, rather than recompute, so the seam is bit-identically closed.
    vertices.push(vertices[0]);

    Ok(OrbitPath {
        vertices_parent_km: vertices,
        time_offsets_from_perihelion_s: None,
        cache_key,
        plane_normal: plane_normal(elements),
        characteristic_radius_km: elements.a_km * (1.0 + elements.e),
    })
}

fn sample_hyperbolic(cache_key: OrbitGeometryCacheKey) -> Result<OrbitPath, KeplerError> {
    let elements = cache_key.elements;
    let count = orbit_vertex_count(elements.e);
    let last = count - 1;
    let mean_anomaly_limit = cache_key.mean_motion_rad_per_s * HYPERBOLIC_HALF_SPAN_S;
    let hyperbolic_anomaly_limit = solve_hyperbolic(mean_anomaly_limit, elements.e)?;
    let mut vertices = Vec::with_capacity(count);
    let mut offsets = Vec::with_capacity(count);

    for index in 0..count {
        let (mean_anomaly, offset_s) = if index == 0 {
            (-mean_anomaly_limit, -HYPERBOLIC_HALF_SPAN_S)
        } else if index == last {
            (mean_anomaly_limit, HYPERBOLIC_HALF_SPAN_S)
        } else {
            let h = -hyperbolic_anomaly_limit
                + 2.0 * hyperbolic_anomaly_limit * index as f64 / last as f64;
            let mean_anomaly = elements.e * h.sinh() - h;
            (mean_anomaly, mean_anomaly / cache_key.mean_motion_rad_per_s)
        };
        vertices.push(
            state_from_elements(
                &elements,
                cache_key.mu_parent_km3_s2,
                cache_key.mean_motion_rad_per_s,
                mean_anomaly,
            )?
            .position_km,
        );
        offsets.push(offset_s);
    }

    let characteristic_radius_km = vertices
        .iter()
        .map(|position| norm(*position))
        .fold(0.0_f64, f64::max);
    Ok(OrbitPath {
        vertices_parent_km: vertices,
        time_offsets_from_perihelion_s: Some(offsets),
        cache_key,
        plane_normal: plane_normal(elements),
        characteristic_radius_km,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct OrbitPaletteEntry {
    rgb: [u8; 3],
    base_alpha: f32,
}

fn orbit_palette(body: &BodyRecord) -> OrbitPaletteEntry {
    let (red, green, blue) = body.orbit_color_srgb;
    let rgb = [red, green, blue];
    let base_alpha = match body.category {
        Category::Planet => 0.8,
        Category::DwarfPlanet => 0.6,
        Category::Asteroid => 0.45,
        Category::Moon => 0.4,
        Category::Comet => 0.7,
        Category::Star => 0.0,
    };
    OrbitPaletteEntry { rgb, base_alpha }
}

#[derive(Component)]
pub(crate) struct OrbitLine {
    body_index: usize,
    parent_index: usize,
    palette: OrbitPaletteEntry,
    path: OrbitPath,
    displayed_alpha: f32,
    displayed_brightness: f32,
    displayed_selected: bool,
}

impl OrbitLine {
    pub(crate) const fn body_index(&self) -> usize {
        self.body_index
    }

    pub(crate) const fn is_pick_visible(&self) -> bool {
        self.displayed_alpha > 0.0
    }

    pub(crate) fn render_vertices(&self) -> impl Iterator<Item = Vec3> + '_ {
        self.path
            .vertices_parent_km
            .iter()
            .copied()
            .map(parent_relative_render_position)
    }

    #[cfg(test)]
    pub(crate) fn for_pick_test(
        body_index: usize,
        parent_index: usize,
        path: OrbitPath,
        displayed_alpha: f32,
    ) -> Self {
        Self {
            body_index,
            parent_index,
            palette: OrbitPaletteEntry {
                rgb: [255; 3],
                base_alpha: 1.0,
            },
            path,
            displayed_alpha,
            displayed_brightness: 1.0,
            displayed_selected: false,
        }
    }
}

#[derive(SystemParam)]
struct OrbitLineRenderOptions<'w> {
    brightness: Res<'w, OrbitLineBrightness>,
    emphasis: Option<Res<'w, OrbitEmphasisState>>,
    view_options: Option<Res<'w, ViewOptionsState>>,
    layers: Option<Res<'w, LayerState>>,
    selection: Option<Res<'w, SelectionAccent>>,
}

#[derive(SystemParam)]
struct OrbitLineTemporalInputs<'w, 's> {
    clock: Res<'w, SimulationClock>,
    cameras: Query<'w, 's, (&'static Camera, &'static Projection), With<Camera3d>>,
}

impl OrbitLineRenderOptions<'_> {
    fn body_brightness(&self, body_index: usize) -> f32 {
        self.brightness.sanitized()
            * self
                .emphasis
                .as_ref()
                .map_or(1.0, |emphasis| emphasis.orbit_brightness(body_index))
    }

    fn body_is_selected(&self, loaded: &LoadedCatalog, body_index: usize) -> bool {
        self.selection
            .as_ref()
            .is_some_and(|selection| selection.accents_orbit(loaded, body_index))
    }
}

fn orbit_passes_presentation_visibility(
    body_index: usize,
    body: &BodyRecord,
    loaded: &LoadedCatalog,
    focus_body_index: Option<usize>,
    layers: Option<&LayerState>,
    view_options: Option<&ViewOptionsState>,
) -> bool {
    if view_options.is_some_and(|options| !options.local_orbit_visible(&body.id)) {
        return false;
    }
    if body.category != Category::Moon {
        return true;
    }
    layers.is_none_or(|layers| layers.is_visible(LayerId::Moons))
        && focus_body_index.is_some_and(|focus| {
            body_passes_contextual_moon_visibility(body_index, focus, loaded, view_options)
        })
}

pub struct OrbitLinesPlugin;

impl Plugin for OrbitLinesPlugin {
    fn build(&self, app: &mut App) {
        crate::record_architecture_plugin(app, "OrbitLinesPlugin");
        app.init_resource::<OrbitLineBrightness>()
            .add_systems(Startup, spawn_orbit_lines)
            .add_systems(
                Update,
                update_orbit_lines
                    .in_set(SimulationSet::Render)
                    .after(OrbitEmphasisSet)
                    .after(SelectionAccentSet),
            );
    }
}

fn spawn_orbit_lines(
    mut commands: Commands,
    loaded: Option<Res<LoadedCatalog>>,
    clock: Res<SimulationClock>,
    options: OrbitLineRenderOptions,
    camera: Option<Res<CameraController>>,
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
) {
    let Some(loaded) = loaded else {
        return;
    };
    for (body_index, body) in loaded.catalog.bodies.iter().enumerate() {
        let (Some(parent_id), Some(orbit)) = (body.parent.as_deref(), body.orbit.as_ref()) else {
            continue;
        };
        let Some(parent_index) = loaded.index_of(parent_id) else {
            continue;
        };
        let Some(mu_parent_km3_s2) = loaded.catalog.bodies[parent_index].gm_km3_s2 else {
            continue;
        };
        let path = match sample_orbit(orbit, mu_parent_km3_s2, clock.0.t()) {
            Ok(path) => path,
            Err(error) => {
                error!("could not sample orbit line for '{}': {error}", body.id);
                continue;
            }
        };
        let palette = orbit_palette(body);
        let orbit_layer_visible = options
            .layers
            .as_ref()
            .is_none_or(|layers| layers.is_visible(LayerId::Orbits));
        let presentation_visible = orbit_passes_presentation_visibility(
            body_index,
            body,
            &loaded,
            camera.as_ref().map(|camera| camera.focus_body_index()),
            options.layers.as_deref(),
            options.view_options.as_deref(),
        );
        let displayed_alpha = if orbit_layer_visible && presentation_visible {
            palette.base_alpha
        } else {
            0.0
        };
        let brightness = options.body_brightness(body_index);
        let selected = options.body_is_selected(&loaded, body_index);
        let mut asset = GizmoAsset::default();
        rebuild_asset(
            &mut asset,
            &path,
            line_color(palette.rgb, displayed_alpha, brightness, selected),
        );
        let handle = gizmo_assets.add(asset);

        commands.spawn((
            Name::new(format!("{} orbit", body.name)),
            OrbitLine {
                body_index,
                parent_index,
                palette,
                path,
                displayed_alpha,
                displayed_brightness: brightness,
                displayed_selected: selected,
            },
            Gizmo {
                handle,
                line_config: GizmoLineConfig {
                    width: orbit_line_width_logical_px(body.category),
                    perspective: false,
                    style: GizmoLineStyle::Solid,
                    joints: GizmoLineJoint::Round(2),
                },
                depth_bias: ORBIT_DEPTH_BIAS,
            },
            Transform::default(),
        ));
    }
}

fn update_orbit_lines(
    loaded: Option<Res<LoadedCatalog>>,
    states: Option<Res<BodyStates>>,
    camera: Res<CameraController>,
    temporal: OrbitLineTemporalInputs,
    options: OrbitLineRenderOptions,
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
    mut lines: Query<(&mut OrbitLine, &mut Transform, &Gizmo)>,
) {
    let (Some(loaded), Some(states)) = (loaded, states) else {
        return;
    };
    let focus_position_km = camera.focus_position_km();
    let camera_position_km = camera.camera_position_km();
    let projection_scale = temporal
        .cameras
        .single()
        .ok()
        .and_then(|(camera, projection)| {
            camera
                .logical_viewport_size()
                .map(|size| (projection, f64::from(size.y)))
        });
    for (mut line, mut transform, gizmo) in &mut lines {
        let Some(parent_state) = states.0.get(line.parent_index) else {
            continue;
        };
        let desired_translation = rebase_position(parent_state.position_km, focus_position_km);
        if transform.translation != desired_translation {
            transform.translation = desired_translation;
        }

        let body = &loaded.catalog.bodies[line.body_index];
        let Some(orbit) = body.orbit.as_ref() else {
            continue;
        };
        let Some(mu_parent_km3_s2) = loaded.catalog.bodies[line.parent_index].gm_km3_s2 else {
            continue;
        };

        let parent_to_camera = sub(camera_position_km, parent_state.position_km);
        let camera_distance_km = norm(parent_to_camera);
        let maximum_drift_km = projection_scale.map_or(0.0, |(projection, viewport_height)| {
            temporal_reuse_tolerance_km(
                camera_distance_km,
                line.path.characteristic_radius_km,
                projection,
                viewport_height,
            )
        });

        let mut rebuilt = false;
        // Evaluate the retained key even when time is paused: a zoom, resize,
        // or projection change can reduce the current pixel-space allowance
        // enough to require a deterministic refresh at the same simulation
        // time.
        match retained_orbit_path(
            &line.path,
            orbit,
            mu_parent_km3_s2,
            temporal.clock.0.t(),
            maximum_drift_km,
        ) {
            Ok(Some(path)) => {
                rebuilt = path.vertices_parent_km != line.path.vertices_parent_km;
                line.path = path;
            }
            Ok(None) => {}
            Err(error) => {
                error!("could not refresh orbit line for '{}': {error}", body.id);
                continue;
            }
        }

        let view_angle_cos = if camera_distance_km > 0.0 {
            dot(line.path.plane_normal, parent_to_camera) / camera_distance_km
        } else {
            1.0
        };
        let orbit_layer_visible = options
            .layers
            .as_ref()
            .is_none_or(|layers| layers.is_visible(LayerId::Orbits));
        let presentation_visible = orbit_passes_presentation_visibility(
            line.body_index,
            body,
            &loaded,
            Some(camera.focus_body_index()),
            options.layers.as_deref(),
            options.view_options.as_deref(),
        );
        let displayed_alpha = if orbit_layer_visible && presentation_visible {
            quantized_alpha(orbit_alpha(
                line.palette.base_alpha,
                camera_distance_km,
                line.path.characteristic_radius_km,
                view_angle_cos,
            ))
        } else {
            0.0
        };
        let brightness = options.body_brightness(line.body_index);
        let selected = options.body_is_selected(&loaded, line.body_index);
        let color_changed = displayed_alpha != line.displayed_alpha
            || brightness != line.displayed_brightness
            || selected != line.displayed_selected;
        if displayed_alpha != line.displayed_alpha {
            line.displayed_alpha = displayed_alpha;
        }
        if brightness != line.displayed_brightness {
            line.displayed_brightness = brightness;
        }
        if selected != line.displayed_selected {
            line.displayed_selected = selected;
        }

        if !rebuilt && !color_changed {
            continue;
        }
        let color = line_color(line.palette.rgb, displayed_alpha, brightness, selected);
        let Some(mut asset) = gizmo_assets.get_mut(&gizmo.handle) else {
            continue;
        };
        if rebuilt {
            rebuild_asset(&mut asset, &line.path, color);
        } else if color_changed {
            update_asset_color(&mut asset, color);
        }
    }
}

fn rebuild_asset(asset: &mut GizmoAsset, path: &OrbitPath, color: LinearRgba) {
    asset.clear();
    asset.linestrip(
        path.vertices_parent_km
            .iter()
            .copied()
            .map(parent_relative_render_position),
        color,
    );
}

fn update_asset_color(asset: &mut GizmoAsset, color: LinearRgba) {
    let color_count = asset.strip_colors.len().saturating_sub(1);
    asset.strip_colors[..color_count].fill(color);
}

#[cfg(test)]
pub(crate) fn rendered_orbit_brightness(world: &mut World, body_index: usize) -> Option<f32> {
    let mut lines = world.query::<&OrbitLine>();
    lines
        .iter(world)
        .find_map(|line| (line.body_index == body_index).then_some(line.displayed_brightness))
}

fn parent_relative_render_position(position_km: [f64; 3]) -> Vec3 {
    rebase_position(position_km, [0.0; 3])
}

fn line_color(rgb: [u8; 3], alpha: f32, brightness: f32, selected: bool) -> LinearRgba {
    let rgb = if selected { SELECTION_ACCENT_RGB } else { rgb };
    let mut color = LinearRgba::from(Color::srgba_u8(rgb[0], rgb[1], rgb[2], 255));
    color.red *= brightness;
    color.green *= brightness;
    color.blue *= brightness;
    color.alpha = alpha;
    color
}

fn orbit_alpha(
    base_alpha: f32,
    camera_distance_km: f64,
    characteristic_radius_km: f64,
    view_angle_cos: f64,
) -> f32 {
    let angular_radius = if camera_distance_km > 0.0 {
        characteristic_radius_km / camera_distance_km
    } else {
        f64::INFINITY
    };
    let distance_fade = smoothstep(
        FADE_OUT_ANGULAR_RADIUS,
        FULL_ALPHA_ANGULAR_RADIUS,
        angular_radius,
    );
    let angle_fade = EDGE_ON_ALPHA_FACTOR
        + (1.0 - EDGE_ON_ALPHA_FACTOR) * smoothstep(0.02, 0.35, view_angle_cos.abs());
    (f64::from(base_alpha) * distance_fade * angle_fade).clamp(0.0, 1.0) as f32
}

fn quantized_alpha(alpha: f32) -> f32 {
    (alpha.clamp(0.0, 1.0) * 255.0).round() / 255.0
}

fn smoothstep(edge0: f64, edge1: f64, value: f64) -> f64 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn plane_normal(elements: Elements) -> [f64; 3] {
    let inclination = elements.i_deg.to_radians();
    let raan = elements.raan_deg.to_radians();
    let (sin_i, cos_i) = inclination.sin_cos();
    let (sin_raan, cos_raan) = raan.sin_cos();
    [sin_raan * sin_i, -cos_raan * sin_i, cos_i]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn norm(vector: [f64; 3]) -> f64 {
    dot(vector, vector).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::zoom_limits;
    use crate::{
        load_catalog_text, propagate_catalog, ScenePolishPlugin, SimulationTickAdvance,
        EMPHASIS_CROSSFADE_S, EMPHASIZED_ORBIT_BRIGHTNESS,
    };
    use bevy::camera::{CameraProjection, ComputedCameraValues, RenderTargetInfo, Viewport};
    use sim_core::catalog::Catalog;
    use sim_core::kepler::state_at;
    use sim_core::time::{
        t_from_jd_tdb, RateIndex, SimClock, StartMode, DAY_S, DEFAULT_START_EPOCH_JD_TDB,
        T_HIGH_CONFIDENCE_MAX_S, T_MAX_S, T_MIN_S,
    };
    use std::time::Duration;

    const REAL_CATALOG: &str = include_str!("../../../assets/catalog.ron");

    fn catalog() -> Catalog {
        load_catalog_text(REAL_CATALOG).expect("committed catalog must load")
    }

    fn body_orbit<'a>(catalog: &'a Catalog, id: &str) -> (&'a Orbit, f64) {
        let index = catalog.id_index();
        let body = &catalog.bodies[*index.get(id).expect("fixture body")];
        let parent = &catalog.bodies[*index
            .get(body.parent.as_deref().expect("orbiting body has parent"))
            .expect("fixture parent")];
        (
            body.orbit.as_ref().expect("orbiting body has orbit"),
            parent.gm_km3_s2.expect("parent has GM"),
        )
    }

    fn camera_for_reuse_test(width: u32, height: u32) -> (Camera, Projection) {
        let mut perspective = PerspectiveProjection::default();
        perspective.update(width as f32, height as f32);
        let camera = Camera {
            computed: ComputedCameraValues {
                clip_from_view: perspective.get_clip_from_view(),
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::new(width, height),
                    scale_factor: 1.0,
                }),
                ..default()
            },
            viewport: Some(Viewport {
                physical_size: UVec2::new(width, height),
                ..default()
            }),
            ..default()
        };
        (camera, Projection::Perspective(perspective))
    }

    #[derive(Debug, Clone, PartialEq)]
    struct RenderedOrbitSnapshot {
        handle: Handle<GizmoAsset>,
        vertices_parent_km: Vec<[f64; 3]>,
        colors: Vec<LinearRgba>,
        displayed_alpha: f32,
        displayed_brightness: f32,
    }

    fn emphasis_orbit_app() -> App {
        let catalog = catalog();
        let t_s = t_from_jd_tdb(2_461_042.0);
        let states = propagate_catalog(&catalog, t_s).unwrap();
        let loaded = LoadedCatalog::new(catalog);
        let sun = loaded.index_of("sun").unwrap();
        let camera = CameraController::new(sun, states.0[sun].position_km, 3_000_000.0);

        let mut app = App::new();
        app.init_resource::<Time<Real>>()
            .init_resource::<Assets<GizmoAsset>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<SimulationTickAdvance>()
            .insert_resource(loaded)
            .insert_resource(states)
            .insert_resource(camera)
            .insert_resource(SimulationClock(SimClock::new(
                StartMode::FixedEpoch {
                    jd_tdb: 2_461_042.0,
                },
                t_s,
            )))
            .insert_resource(LayerState::default())
            .insert_resource(ViewOptionsState::default())
            .add_plugins((ScenePolishPlugin, OrbitLinesPlugin));
        render_emphasis_frame(&mut app, 0.0, 0.0);
        app
    }

    fn render_emphasis_frame(app: &mut App, simulated_step_s: f64, wall_delta_s: f64) {
        app.world_mut()
            .resource_mut::<SimulationTickAdvance>()
            .seconds = simulated_step_s;
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_secs_f64(wall_delta_s));
        app.update();
    }

    fn rendered_orbit(app: &mut App, body_index: usize) -> RenderedOrbitSnapshot {
        let (handle, vertices_parent_km, displayed_alpha, displayed_brightness) = {
            let world = app.world_mut();
            let mut query = world.query::<(&OrbitLine, &Gizmo)>();
            let (line, gizmo) = query
                .iter(world)
                .find(|(line, _)| line.body_index == body_index)
                .expect("body orbit must exist");
            (
                gizmo.handle.clone(),
                line.path.vertices_parent_km.clone(),
                line.displayed_alpha,
                line.displayed_brightness,
            )
        };
        let assets = app.world().resource::<Assets<GizmoAsset>>();
        let asset = assets.get(&handle).expect("retained orbit asset");
        let color_count = asset.strip_colors.len().saturating_sub(1);
        RenderedOrbitSnapshot {
            handle,
            vertices_parent_km,
            colors: asset.strip_colors[..color_count].to_vec(),
            displayed_alpha,
            displayed_brightness,
        }
    }

    #[test]
    fn vertex_counts_stay_bounded_and_rise_with_eccentricity() {
        let eccentricities = [0.0, 0.1, 0.5, 0.75, 0.99, 1.2, 6.0];
        let counts: Vec<_> = eccentricities
            .iter()
            .map(|e| orbit_vertex_count(*e))
            .collect();
        assert!(counts
            .iter()
            .all(|count| (MIN_ORBIT_VERTICES..=MAX_ORBIT_VERTICES).contains(count)));
        assert!(counts.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(orbit_vertex_count(0.0), MIN_ORBIT_VERTICES);
        assert_eq!(orbit_vertex_count(0.75), 640);
        assert_eq!(orbit_vertex_count(1.2), 767);
    }

    #[test]
    fn elliptic_paths_close_bit_exactly_and_all_vertices_are_finite() {
        let catalog = catalog();
        let (orbit, mu) = body_orbit(&catalog, "nereid");
        let path = sample_orbit(orbit, mu, t_from_jd_tdb(2_461_042.0)).unwrap();
        assert!(path.is_closed());
        assert_eq!(
            path.vertices_parent_km.first(),
            path.vertices_parent_km.last()
        );
        assert!(path
            .vertices_parent_km
            .iter()
            .flatten()
            .all(|value| value.is_finite()));
    }

    #[test]
    fn nereid_chords_are_visibly_denser_near_perihelion() {
        let catalog = catalog();
        let (orbit, mu) = body_orbit(&catalog, "nereid");
        let path = sample_orbit(orbit, mu, t_from_jd_tdb(2_461_042.0)).unwrap();
        let near_perihelion = norm(sub(path.vertices_parent_km[1], path.vertices_parent_km[0]));
        let apoapsis_index = (path.vertices_parent_km.len() - 1) / 2;
        let near_apoapsis = norm(sub(
            path.vertices_parent_km[apoapsis_index + 1],
            path.vertices_parent_km[apoapsis_index],
        ));
        assert!(
            near_perihelion < near_apoapsis * 0.2,
            "perihelion chord {near_perihelion} was not much shorter than apoapsis chord {near_apoapsis}"
        );
    }

    #[test]
    fn atlas_is_an_open_arc_with_exact_twenty_five_year_endpoints() {
        let catalog = catalog();
        let (orbit, mu) = body_orbit(&catalog, "3i_atlas");
        let current_t = t_from_jd_tdb(2_461_042.0);
        let path = sample_orbit(orbit, mu, current_t).unwrap();
        let offsets = path.time_offsets_from_perihelion_s().unwrap();
        assert_eq!(offsets.first(), Some(&-HYPERBOLIC_HALF_SPAN_S));
        assert_eq!(offsets.last(), Some(&HYPERBOLIC_HALF_SPAN_S));
        assert_ne!(
            path.vertices_parent_km.first(),
            path.vertices_parent_km.last()
        );
        assert!(!path.is_closed());
        assert!(path
            .vertices_parent_km
            .iter()
            .flatten()
            .all(|value| value.is_finite()));

        let perihelion_t = t_from_jd_tdb(orbit.epoch_jd_tdb)
            - orbit.elements.m0_deg.to_radians() / orbit.mean_motion_rad_per_s(mu);
        let expected_start = state_at(orbit, mu, perihelion_t - HYPERBOLIC_HALF_SPAN_S)
            .unwrap()
            .position_km;
        let expected_end = state_at(orbit, mu, perihelion_t + HYPERBOLIC_HALF_SPAN_S)
            .unwrap()
            .position_km;
        // The direct path subtracts two large absolute epoch values while the
        // sampler advances from perihelion by the exact relative span. Their
        // resulting positions agree to centimeters across a 45-billion-km arc.
        assert!(norm(sub(path.vertices_parent_km[0], expected_start)) <= 1.0e-4);
        assert!(norm(sub(*path.vertices_parent_km.last().unwrap(), expected_end)) <= 1.0e-4);
    }

    #[test]
    fn sampled_perihelia_match_both_conic_distance_formulas() {
        let catalog = catalog();
        for id in ["nereid", "3i_atlas"] {
            let (orbit, mu) = body_orbit(&catalog, id);
            let path = sample_orbit(orbit, mu, t_from_jd_tdb(2_461_042.0)).unwrap();
            let perihelion_index = if path.elements().is_hyperbolic() {
                path.vertices_parent_km.len() / 2
            } else {
                0
            };
            let sampled = norm(path.vertices_parent_km[perihelion_index]);
            let expected = if path.elements().is_hyperbolic() {
                path.elements().a_km.abs() * (path.elements().e - 1.0)
            } else {
                path.elements().a_km * (1.0 - path.elements().e)
            };
            let relative_error = (sampled - expected).abs() / expected;
            assert!(relative_error <= 1.0e-12, "{id}: {relative_error:e}");
        }
    }

    #[test]
    fn secular_planet_paths_use_elements_at_the_current_time() {
        let catalog = catalog();
        let (orbit, mu) = body_orbit(&catalog, "earth");
        let epoch_t = t_from_jd_tdb(orbit.epoch_jd_tdb);
        let later_t = epoch_t + 100.0 * JULIAN_YEAR_S;
        let epoch_path = sample_orbit(orbit, mu, epoch_t).unwrap();
        let later_path = sample_orbit(orbit, mu, later_t).unwrap();
        assert_eq!(epoch_path.elements(), elements_at(orbit, epoch_t));
        assert_eq!(later_path.elements(), elements_at(orbit, later_t));
        assert_ne!(epoch_path.elements(), later_path.elements());
        assert_ne!(
            epoch_path.vertices_parent_km[0],
            later_path.vertices_parent_km[0]
        );
    }

    #[test]
    fn secular_drift_bound_contains_fresh_vertex_displacement_across_supported_time() {
        const PLANETS: [&str; 8] = [
            "mercury", "venus", "earth", "mars", "jupiter", "saturn", "uranus", "neptune",
        ];
        let catalog = catalog();
        let catalog_epoch_t = t_from_jd_tdb(DEFAULT_START_EPOCH_JD_TDB);
        let anchors = [T_MIN_S, catalog_epoch_t, T_HIGH_CONFIDENCE_MAX_S];
        let deltas = [DAY_S, JULIAN_YEAR_S, 10.0 * JULIAN_YEAR_S];

        for id in PLANETS {
            let (orbit, mu) = body_orbit(&catalog, id);
            assert!(orbit.secular.is_some(), "{id} must exercise secular drift");
            for anchor_t_s in anchors {
                let cached = sample_orbit(orbit, mu, anchor_t_s).unwrap();
                for delta_s in deltas {
                    let current_t_s = (anchor_t_s + delta_s).min(T_MAX_S);
                    let fresh = sample_orbit(orbit, mu, current_t_s).unwrap();
                    let bound_km =
                        secular_vertex_drift_bound_km(cached.elements(), fresh.elements());
                    let maximum_vertex_displacement_km = cached
                        .vertices_parent_km
                        .iter()
                        .zip(&fresh.vertices_parent_km)
                        .map(|(left, right)| norm(sub(*left, *right)))
                        .fold(0.0_f64, f64::max);
                    assert!(
                        maximum_vertex_displacement_km <= bound_km,
                        "{id} at t={current_t_s}: vertex drift {maximum_vertex_displacement_km:e} km exceeded analytic bound {bound_km:e} km"
                    );
                    assert!(
                        retained_orbit_path(
                            &cached,
                            orbit,
                            mu,
                            current_t_s,
                            bound_km + bound_km.abs() * f64::EPSILON * 4.0 + f64::MIN_POSITIVE,
                        )
                        .unwrap()
                        .is_none(),
                        "{id} at t={current_t_s} must reuse below the analytic threshold"
                    );
                    assert_eq!(
                        retained_orbit_path(&cached, orbit, mu, current_t_s, bound_km).unwrap(),
                        Some(fresh),
                        "{id} at t={current_t_s} must resample when the bound is reached"
                    );
                }
            }
        }

        let (earth, mu) = body_orbit(&catalog, "earth");
        let mut density_crossing = earth.clone();
        density_crossing.secular.as_mut().unwrap().e_per_cy = 0.1;
        let epoch_t_s = t_from_jd_tdb(density_crossing.epoch_jd_tdb);
        let cached = sample_orbit(&density_crossing, mu, epoch_t_s).unwrap();
        let current_t_s = epoch_t_s + 10.0 * JULIAN_YEAR_S;
        let fresh = sample_orbit(&density_crossing, mu, current_t_s).unwrap();
        assert_ne!(
            cached.vertices_parent_km.len(),
            fresh.vertices_parent_km.len(),
            "fixture must cross an adaptive sampling-density boundary"
        );
        assert_eq!(
            retained_orbit_path(&cached, &density_crossing, mu, current_t_s, f64::INFINITY,)
                .unwrap(),
            Some(fresh),
            "a vertex-count change must refresh even under an unlimited pixel allowance"
        );
    }

    #[test]
    fn temporal_reuse_tolerance_is_quarter_pixel_at_the_nearest_orbit_depth() {
        let projection = Projection::Perspective(PerspectiveProjection::default());
        let camera_distance_km = 10_000_000.0;
        let orbit_radius_km = 2_000_000.0;
        let viewport_height = 1_200.0;
        let tolerance_km = temporal_reuse_tolerance_km(
            camera_distance_km,
            orbit_radius_km,
            &projection,
            viewport_height,
        );
        let Projection::Perspective(perspective) = projection else {
            unreachable!();
        };
        let closest_depth_after_drift_km = camera_distance_km - orbit_radius_km - tolerance_km;
        let projected_drift_px = tolerance_km * viewport_height
            / (2.0 * closest_depth_after_drift_km * (f64::from(perspective.fov) * 0.5).tan());
        assert!(
            (projected_drift_px - MAX_TEMPORAL_REUSE_ERROR_LOGICAL_PX).abs() <= f64::EPSILON,
            "projected tolerance was {projected_drift_px} logical px"
        );
        assert_eq!(
            temporal_reuse_tolerance_km(
                orbit_radius_km,
                orbit_radius_km,
                &Projection::Perspective(perspective),
                viewport_height,
            ),
            0.0,
            "a camera inside the orbit must fall back to exact refreshes"
        );
    }

    #[test]
    fn exact_cache_geometry_is_zoom_independent_across_supported_limits() {
        let mut app = emphasis_orbit_app();
        let (saturn, saturn_position, minimum, maximum, fresh_vertices) = {
            let loaded = app.world().resource::<LoadedCatalog>();
            let states = app.world().resource::<BodyStates>();
            let saturn = loaded.index_of("saturn").unwrap();
            let body = &loaded.catalog.bodies[saturn];
            let parent = loaded.index_of(body.parent.as_deref().unwrap()).unwrap();
            let mu = loaded.catalog.bodies[parent].gm_km3_s2.unwrap();
            let t_s = app.world().resource::<SimulationClock>().0.t();
            let smallest_body = loaded
                .catalog
                .bodies
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| left.radius_km.total_cmp(&right.radius_km))
                .map(|(index, _)| index)
                .unwrap();
            (
                saturn,
                states.0[saturn].position_km,
                zoom_limits(loaded, smallest_body).0,
                zoom_limits(loaded, saturn).1,
                sample_orbit(body.orbit.as_ref().unwrap(), mu, t_s)
                    .unwrap()
                    .vertices_parent_km,
            )
        };
        assert!(minimum > 0.0 && maximum > minimum);

        let baseline = rendered_orbit(&mut app, saturn);
        assert_eq!(baseline.vertices_parent_km, fresh_vertices);
        for step in 0..=16 {
            let fraction = f64::from(step) / 16.0;
            let distance = minimum * (maximum / minimum).powf(fraction);
            app.insert_resource(CameraController::new(saturn, saturn_position, distance));
            render_emphasis_frame(&mut app, 0.0, 0.0);
            let retained = rendered_orbit(&mut app, saturn);
            assert_eq!(retained.handle, baseline.handle, "zoom step {step}");
            assert_eq!(
                retained.vertices_parent_km, fresh_vertices,
                "zoom step {step} at {distance} render units"
            );
        }
    }

    #[test]
    fn hyperbolic_cache_key_covers_fitted_mean_motion_and_parent_gm() {
        let catalog = catalog();
        let (orbit, mu) = body_orbit(&catalog, "3i_atlas");
        let t_s = t_from_jd_tdb(orbit.epoch_jd_tdb);
        let original = sample_orbit(orbit, mu, t_s).unwrap();
        assert!(
            retained_orbit_path(&original, orbit, mu, t_s, f64::INFINITY)
                .unwrap()
                .is_none()
        );
        assert!(
            retained_orbit_path(
                &original,
                orbit,
                mu,
                t_s + 100.0 * JULIAN_YEAR_S,
                f64::INFINITY,
            )
            .unwrap()
            .is_none(),
            "non-secular geometry must remain exact across time"
        );

        let mut fitted = orbit.clone();
        fitted.mean_motion_deg_per_day =
            Some((orbit.mean_motion_rad_per_s(mu) * DAY_S).to_degrees() * 1.01);
        let retained_fitted = retained_orbit_path(&original, &fitted, mu, t_s, f64::INFINITY)
            .unwrap()
            .expect("fitted mean motion is part of the exact key");
        let fresh_fitted = sample_orbit(&fitted, mu, t_s).unwrap();
        assert_eq!(retained_fitted, fresh_fitted);
        assert_ne!(
            retained_fitted.cache_key.mean_motion_rad_per_s,
            original.cache_key.mean_motion_rad_per_s
        );
        assert_ne!(
            retained_fitted.vertices_parent_km,
            original.vertices_parent_km
        );

        let mut two_body = orbit.clone();
        two_body.mean_motion_deg_per_day = None;
        let two_body_original = sample_orbit(&two_body, mu, t_s).unwrap();
        let changed_mu = mu * 1.01;
        let retained_mu = retained_orbit_path(
            &two_body_original,
            &two_body,
            changed_mu,
            t_s,
            f64::INFINITY,
        )
        .unwrap()
        .expect("parent GM is part of the exact key");
        let fresh_mu = sample_orbit(&two_body, changed_mu, t_s).unwrap();
        assert_eq!(retained_mu, fresh_mu);
        assert_eq!(retained_mu.cache_key.mu_parent_km3_s2, changed_mu);
        assert_ne!(
            retained_mu.cache_key.mean_motion_rad_per_s,
            two_body_original.cache_key.mean_motion_rad_per_s
        );
        assert_ne!(
            retained_mu.vertices_parent_km,
            two_body_original.vertices_parent_km
        );
    }

    #[test]
    fn category_widths_and_manifest_colors_reach_all_sixty_five_paths() {
        assert_eq!(orbit_line_width_logical_px(Category::Star), 0.0);
        assert_eq!(orbit_line_width_logical_px(Category::Planet), 4.5);
        assert_eq!(orbit_line_width_logical_px(Category::DwarfPlanet), 3.0);
        assert_eq!(orbit_line_width_logical_px(Category::Moon), 3.0);
        assert_eq!(orbit_line_width_logical_px(Category::Asteroid), 1.5);
        assert_eq!(orbit_line_width_logical_px(Category::Comet), 1.5);

        let catalog = catalog();
        let expected = catalog
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, body)| body.category != Category::Star)
            .map(|(index, body)| {
                (
                    index,
                    body.category,
                    body.orbit_color_srgb,
                    body.orbit.as_ref().map(|orbit| orbit.elements),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(expected.len(), 65);

        let mut app = emphasis_orbit_app();
        let world = app.world_mut();
        let mut query = world.query::<(&OrbitLine, &Gizmo)>();
        let mut rendered = query
            .iter(world)
            .map(|(line, gizmo)| {
                (
                    line.body_index,
                    line.palette.rgb,
                    gizmo.line_config.width,
                    line.path.elements(),
                )
            })
            .collect::<Vec<_>>();
        rendered.sort_by_key(|row| row.0);
        assert_eq!(rendered.len(), 65);

        for ((body_index, category, rgb, elements), rendered) in expected.into_iter().zip(rendered)
        {
            assert_eq!(rendered.0, body_index);
            assert_eq!(rendered.1, [rgb.0, rgb.1, rgb.2]);
            assert_eq!(rendered.2, orbit_line_width_logical_px(category));
            assert_eq!(rendered.3, elements.unwrap());
        }
    }

    #[test]
    fn selection_accents_parent_and_child_orbits_without_rebuilding_paths() {
        let mut app = emphasis_orbit_app();
        let (jupiter, io, saturn) = {
            let loaded = app.world().resource::<LoadedCatalog>();
            (
                loaded.index_of("jupiter").unwrap(),
                loaded.index_of("io").unwrap(),
                loaded.index_of("saturn").unwrap(),
            )
        };
        let jupiter_base = rendered_orbit(&mut app, jupiter);
        let io_base = rendered_orbit(&mut app, io);
        let saturn_base = rendered_orbit(&mut app, saturn);

        app.insert_resource(SelectionAccent::for_selected(jupiter));
        app.update();
        let jupiter_selected = rendered_orbit(&mut app, jupiter);
        let io_selected = rendered_orbit(&mut app, io);
        let saturn_unselected = rendered_orbit(&mut app, saturn);

        for (base, selected) in [(&jupiter_base, &jupiter_selected), (&io_base, &io_selected)] {
            assert_eq!(selected.handle, base.handle);
            assert_eq!(selected.vertices_parent_km, base.vertices_parent_km);
            assert_ne!(selected.colors, base.colors);
        }
        assert_eq!(saturn_unselected, saturn_base);
        let accent = LinearRgba::from(Color::srgba_u8(
            SELECTION_ACCENT_RGB[0],
            SELECTION_ACCENT_RGB[1],
            SELECTION_ACCENT_RGB[2],
            255,
        ));
        for selected in [&jupiter_selected, &io_selected] {
            assert!(selected.colors.iter().all(|color| {
                color.red.to_bits() == accent.red.to_bits()
                    && color.green.to_bits() == accent.green.to_bits()
                    && color.blue.to_bits() == accent.blue.to_bits()
            }));
        }

        app.insert_resource(SelectionAccent::for_selected(saturn));
        app.update();
        assert_eq!(rendered_orbit(&mut app, jupiter), jupiter_base);
        assert_eq!(rendered_orbit(&mut app, io), io_base);
        assert_ne!(rendered_orbit(&mut app, saturn), saturn_base);

        let stable = rendered_orbit(&mut app, saturn);
        app.update();
        assert_eq!(rendered_orbit(&mut app, saturn), stable);
    }

    #[test]
    fn camera_distance_and_edge_on_views_reduce_alpha() {
        let face_on_near = orbit_alpha(0.8, 1.0e6, 1.0e6, 1.0);
        let face_on_far = orbit_alpha(0.8, 1.0e11, 1.0e6, 1.0);
        let edge_on_near = orbit_alpha(0.8, 1.0e6, 1.0e6, 0.0);
        assert!(face_on_near > face_on_far);
        assert!(face_on_near > edge_on_near);
        assert!(edge_on_near > 0.0);
    }

    #[test]
    fn actual_hundred_year_step_fades_mercury_through_saturn_and_restores_orbits_exactly() {
        #[derive(Resource, Default)]
        struct ChangedOrbitLines(Vec<usize>);

        fn count_changed_orbit_lines(
            lines: Query<&OrbitLine, Changed<OrbitLine>>,
            mut changed: ResMut<ChangedOrbitLines>,
        ) {
            changed.0.extend(lines.iter().map(|line| line.body_index));
        }

        const FRAME_S: f64 = 1.0 / 60.0;
        const INNER_PLANETS: [&str; 6] = ["mercury", "venus", "earth", "mars", "jupiter", "saturn"];

        let mut app = emphasis_orbit_app();
        app.init_resource::<ChangedOrbitLines>()
            .add_systems(Update, count_changed_orbit_lines.after(update_orbit_lines));
        app.update();
        app.world_mut()
            .resource_mut::<ChangedOrbitLines>()
            .0
            .clear();
        let body_indices: Vec<_> = {
            let loaded = app.world().resource::<LoadedCatalog>();
            INNER_PLANETS
                .iter()
                .map(|id| loaded.index_of(id).unwrap())
                .collect()
        };
        let uranus = app
            .world()
            .resource::<LoadedCatalog>()
            .index_of("uranus")
            .unwrap();
        let baseline: Vec<_> = body_indices
            .iter()
            .map(|index| rendered_orbit(&mut app, *index))
            .collect();
        let uranus_baseline = rendered_orbit(&mut app, uranus);
        assert!(baseline.iter().all(|orbit| orbit.displayed_alpha > 0.0));
        assert!(baseline
            .iter()
            .all(|orbit| orbit.displayed_brightness == 1.0));

        let hundred_year_step = RateIndex::MAX.seconds_per_second() * FRAME_S;
        render_emphasis_frame(&mut app, hundred_year_step, FRAME_S);
        {
            let changed = &app.world().resource::<ChangedOrbitLines>().0;
            assert!(!changed.contains(&uranus));
            for index in &body_indices {
                assert!(changed.contains(index));
            }
        }
        {
            let emphasis = app.world().resource::<OrbitEmphasisState>();
            for (id, index) in INNER_PLANETS.iter().zip(&body_indices) {
                let body = emphasis.body(*index).unwrap();
                assert!(body.is_engaged(), "{id}");
                assert!((0.0..1.0).contains(&emphasis.body_alpha(*index)), "{id}");
                assert!(
                    (1.0..EMPHASIZED_ORBIT_BRIGHTNESS).contains(&emphasis.orbit_brightness(*index)),
                    "{id}"
                );
            }
            assert!(!emphasis.body(uranus).unwrap().is_engaged());
            assert_eq!(emphasis.body_alpha(uranus), 1.0);
            assert_eq!(emphasis.orbit_brightness(uranus), 1.0);
        }
        for ((id, index), original) in INNER_PLANETS.iter().zip(&body_indices).zip(&baseline) {
            let expected_brightness = app
                .world()
                .resource::<OrbitEmphasisState>()
                .orbit_brightness(*index);
            let intermediate = rendered_orbit(&mut app, *index);
            assert_eq!(
                intermediate.displayed_brightness, expected_brightness,
                "{id}"
            );
            assert_eq!(
                intermediate.displayed_alpha, original.displayed_alpha,
                "{id}"
            );
            assert_eq!(intermediate.handle, original.handle, "{id}");
            assert_eq!(
                intermediate.vertices_parent_km, original.vertices_parent_km,
                "{id}"
            );
            assert_ne!(intermediate.colors, original.colors, "{id}");
        }

        let fade_frames = (f64::from(EMPHASIS_CROSSFADE_S) / FRAME_S).ceil() as usize;
        for _ in 1..fade_frames {
            render_emphasis_frame(&mut app, hundred_year_step, FRAME_S);
        }
        for (id, index) in INNER_PLANETS.iter().zip(&body_indices) {
            let emphasis = app.world().resource::<OrbitEmphasisState>();
            assert_eq!(emphasis.body_alpha(*index), 0.0, "{id}");
            assert_eq!(
                emphasis.orbit_brightness(*index),
                EMPHASIZED_ORBIT_BRIGHTNESS,
                "{id}"
            );
            let emphasized = rendered_orbit(&mut app, *index);
            let original = &baseline[INNER_PLANETS.iter().position(|value| value == id).unwrap()];
            assert_eq!(emphasized.handle, original.handle, "{id}");
            assert_eq!(
                emphasized.vertices_parent_km, original.vertices_parent_km,
                "{id}"
            );
            assert_eq!(emphasized.displayed_alpha, original.displayed_alpha, "{id}");
            assert_eq!(
                emphasized.displayed_brightness, EMPHASIZED_ORBIT_BRIGHTNESS,
                "{id}"
            );
            assert!(emphasized
                .colors
                .iter()
                .zip(&original.colors)
                .all(|(actual, base)| {
                    actual.red > base.red
                        && actual.green > base.green
                        && actual.blue > base.blue
                        && actual.alpha == base.alpha
                }));
        }
        assert_eq!(rendered_orbit(&mut app, uranus), uranus_baseline);

        let saturn = *body_indices.last().unwrap();
        app.world_mut()
            .resource_mut::<LayerState>()
            .set_visible(LayerId::Orbits, false);
        render_emphasis_frame(&mut app, hundred_year_step, FRAME_S);
        let globally_hidden = rendered_orbit(&mut app, saturn);
        assert_eq!(globally_hidden.displayed_alpha, 0.0);
        assert_eq!(
            globally_hidden.displayed_brightness,
            EMPHASIZED_ORBIT_BRIGHTNESS
        );
        assert!(globally_hidden
            .colors
            .iter()
            .all(|color| color.alpha == 0.0));

        app.world_mut()
            .resource_mut::<LayerState>()
            .set_visible(LayerId::Orbits, true);
        app.world_mut()
            .resource_mut::<ViewOptionsState>()
            .set_local_orbit_visible("saturn", false);
        render_emphasis_frame(&mut app, hundred_year_step, FRAME_S);
        let locally_hidden = rendered_orbit(&mut app, saturn);
        assert_eq!(locally_hidden.displayed_alpha, 0.0);
        assert_eq!(
            locally_hidden.displayed_brightness,
            EMPHASIZED_ORBIT_BRIGHTNESS
        );
        assert!(locally_hidden.colors.iter().all(|color| color.alpha == 0.0));
        assert!(rendered_orbit(&mut app, body_indices[0]).displayed_alpha > 0.0);

        app.world_mut()
            .resource_mut::<ViewOptionsState>()
            .set_local_orbit_visible("saturn", true);
        render_emphasis_frame(&mut app, hundred_year_step, FRAME_S);
        assert_eq!(
            rendered_orbit(&mut app, saturn).displayed_alpha,
            baseline.last().unwrap().displayed_alpha
        );

        render_emphasis_frame(&mut app, FRAME_S, FRAME_S);
        {
            let emphasis = app.world().resource::<OrbitEmphasisState>();
            for (id, index) in INNER_PLANETS.iter().zip(&body_indices) {
                assert!(!emphasis.body(*index).unwrap().is_engaged(), "{id}");
                assert!((0.0..1.0).contains(&emphasis.body_alpha(*index)), "{id}");
                assert!(
                    (1.0..EMPHASIZED_ORBIT_BRIGHTNESS).contains(&emphasis.orbit_brightness(*index)),
                    "{id}"
                );
            }
        }
        for ((id, index), original) in INNER_PLANETS.iter().zip(&body_indices).zip(&baseline) {
            let expected_brightness = app
                .world()
                .resource::<OrbitEmphasisState>()
                .orbit_brightness(*index);
            let restoring = rendered_orbit(&mut app, *index);
            assert_eq!(restoring.displayed_brightness, expected_brightness, "{id}");
            assert_eq!(restoring.displayed_alpha, original.displayed_alpha, "{id}");
            assert_eq!(restoring.handle, original.handle, "{id}");
            assert_eq!(
                restoring.vertices_parent_km, original.vertices_parent_km,
                "{id}"
            );
            assert_ne!(restoring.colors, original.colors, "{id}");
        }
        for _ in 1..fade_frames {
            render_emphasis_frame(&mut app, FRAME_S, FRAME_S);
        }
        for ((id, index), original) in INNER_PLANETS.iter().zip(&body_indices).zip(&baseline) {
            let emphasis = app.world().resource::<OrbitEmphasisState>();
            assert_eq!(emphasis.body_alpha(*index), 1.0, "{id}");
            assert_eq!(emphasis.orbit_brightness(*index), 1.0, "{id}");
            assert_eq!(rendered_orbit(&mut app, *index), *original, "{id}");
        }
    }

    #[test]
    fn sampler_rejects_nonfinite_time_and_bad_parent_gm_without_panicking() {
        let catalog = catalog();
        let (orbit, mu) = body_orbit(&catalog, "nereid");
        let nonfinite = std::panic::catch_unwind(|| sample_orbit(orbit, mu, f64::NAN));
        assert!(nonfinite.is_ok());
        assert_eq!(nonfinite.unwrap().unwrap_err(), KeplerError::NonFinite);
        assert_eq!(
            sample_orbit(orbit, -mu, t_from_jd_tdb(2_461_042.0)).unwrap_err(),
            KeplerError::BadMu
        );
    }

    #[test]
    fn all_orbits_spawn_and_reanchor_without_changing_parent_vertices() {
        #[derive(Resource, Default)]
        struct RetainedWriteCounts {
            lines: usize,
            transforms: usize,
            assets_changed: bool,
        }

        fn count_retained_writes(
            lines: Query<(), Changed<OrbitLine>>,
            transforms: Query<(), (With<OrbitLine>, Changed<Transform>)>,
            assets: Res<Assets<GizmoAsset>>,
            mut counts: ResMut<RetainedWriteCounts>,
        ) {
            counts.lines += lines.iter().count();
            counts.transforms += transforms.iter().count();
            counts.assets_changed |= assets.is_changed();
        }

        let catalog = catalog();
        let t_s = t_from_jd_tdb(2_461_042.0);
        let states = propagate_catalog(&catalog, t_s).unwrap();
        let loaded = LoadedCatalog::new(catalog);
        let mercury = loaded.index_of("mercury").unwrap();
        let nereid = loaded.index_of("nereid").unwrap();
        let neptune = loaded.index_of("neptune").unwrap();
        let camera = CameraController::new(mercury, states.0[mercury].position_km, 1.0e8);
        let expected_anchor =
            rebase_position(states.0[neptune].position_km, states.0[mercury].position_km);

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<GizmoAsset>>()
            .insert_resource(loaded)
            .insert_resource(states)
            .insert_resource(camera)
            .insert_resource(SimulationClock(SimClock::new(
                StartMode::FixedEpoch {
                    jd_tdb: 2_461_042.0,
                },
                t_s,
            )))
            .insert_resource(OrbitLineBrightness::default())
            .init_resource::<RetainedWriteCounts>()
            .add_systems(Startup, spawn_orbit_lines)
            .add_systems(Update, update_orbit_lines)
            .add_systems(Update, count_retained_writes.after(update_orbit_lines));
        app.update();

        let mut query = app.world_mut().query::<(&OrbitLine, &Transform, &Gizmo)>();
        let lines: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(lines.len(), 65);
        let (line, transform, gizmo) = lines
            .iter()
            .find(|(line, _, _)| line.body_index == nereid)
            .unwrap();
        assert_eq!(transform.translation, expected_anchor);
        assert!(gizmo.depth_bias < 0.0);
        assert_eq!(gizmo.line_config.joints, GizmoLineJoint::Round(2));
        let vertices_before = line.path.vertices_parent_km.clone();

        *app.world_mut().resource_mut::<RetainedWriteCounts>() = RetainedWriteCounts::default();
        app.update();
        let counts = app.world().resource::<RetainedWriteCounts>();
        assert_eq!(counts.lines, 0);
        assert_eq!(counts.transforms, 0);
        assert!(
            !counts.assets_changed,
            "a stable frame must not acquire any retained Gizmo asset mutably"
        );

        let sedna = app
            .world()
            .resource::<LoadedCatalog>()
            .index_of("sedna")
            .unwrap();
        let states = app.world().resource::<BodyStates>();
        let sedna_position = states.0[sedna].position_km;
        let neptune_position = states.0[neptune].position_km;
        let expected_reanchored = rebase_position(neptune_position, sedna_position);
        app.world_mut()
            .insert_resource(CameraController::new(sedna, sedna_position, 1.0e8));
        app.update();

        let mut query = app.world_mut().query::<(&OrbitLine, &Transform)>();
        let (line, transform) = query
            .iter(app.world())
            .find(|(line, _)| line.body_index == nereid)
            .unwrap();
        assert_eq!(line.path.vertices_parent_km, vertices_before);
        assert_eq!(transform.translation, expected_reanchored);
    }

    #[test]
    fn one_year_per_second_reuses_eight_secular_assets_until_bound_trips() {
        const SECULAR_PLANETS: [&str; 8] = [
            "mercury", "venus", "earth", "mars", "jupiter", "saturn", "uranus", "neptune",
        ];

        #[derive(Resource, Default)]
        struct RetainedWriteCounts {
            lines: usize,
            assets_changed: bool,
        }

        fn count_retained_writes(
            lines: Query<(), Changed<OrbitLine>>,
            assets: Res<Assets<GizmoAsset>>,
            mut counts: ResMut<RetainedWriteCounts>,
        ) {
            counts.lines += lines.iter().count();
            counts.assets_changed |= assets.is_changed();
        }

        let catalog = catalog();
        let t_s = t_from_jd_tdb(DEFAULT_START_EPOCH_JD_TDB);
        let states = propagate_catalog(&catalog, t_s).unwrap();
        let loaded = LoadedCatalog::new(catalog);
        let sun = loaded.index_of("sun").unwrap();
        assert_eq!(
            loaded
                .catalog
                .bodies
                .iter()
                .filter(|body| body
                    .orbit
                    .as_ref()
                    .is_some_and(|orbit| orbit.secular.is_some()))
                .count(),
            SECULAR_PLANETS.len()
        );
        let controller = CameraController::new(sun, states.0[sun].position_km, 10_000_000.0);
        let (render_camera, projection) = camera_for_reuse_test(960, 600);

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<GizmoAsset>>()
            .insert_resource(loaded)
            .insert_resource(states)
            .insert_resource(controller)
            .insert_resource(SimulationClock(SimClock::new(
                StartMode::FixedEpoch {
                    jd_tdb: DEFAULT_START_EPOCH_JD_TDB,
                },
                t_s,
            )))
            .insert_resource(OrbitLineBrightness::default())
            .init_resource::<RetainedWriteCounts>()
            .add_systems(Startup, spawn_orbit_lines)
            .add_systems(Update, update_orbit_lines)
            .add_systems(Update, count_retained_writes.after(update_orbit_lines));
        app.world_mut()
            .spawn((Camera3d::default(), render_camera, projection));
        app.update();

        app.world_mut()
            .resource_mut::<SimulationClock>()
            .0
            .set_rate(RateIndex::new(8).unwrap());
        *app.world_mut().resource_mut::<RetainedWriteCounts>() = RetainedWriteCounts::default();
        for frame in 1..=60 {
            app.world_mut()
                .resource_mut::<SimulationClock>()
                .0
                .tick(1.0 / 60.0, f64::from(frame) / 60.0);
            app.update();
        }
        let counts = app.world().resource::<RetainedWriteCounts>();
        assert_eq!(
            counts.lines, 0,
            "the eight secular paths must produce no retained-line writes over one simulated year"
        );
        assert!(
            !counts.assets_changed,
            "stable-camera temporal reuse must not acquire retained assets mutably"
        );

        let retained_t_s = app.world().resource::<SimulationClock>().0.t();
        let expected_planet_elements: Vec<_> = {
            let loaded = app.world().resource::<LoadedCatalog>();
            SECULAR_PLANETS
                .iter()
                .map(|id| {
                    let index = loaded.index_of(id).unwrap();
                    let orbit = loaded.catalog.bodies[index].orbit.as_ref().unwrap();
                    (index, elements_at(orbit, retained_t_s))
                })
                .collect()
        };
        let retained_planet_count = {
            let world = app.world_mut();
            let mut lines = world.query::<&OrbitLine>();
            lines
                .iter(world)
                .filter(|line| {
                    expected_planet_elements
                        .iter()
                        .find(|(index, _)| *index == line.body_index)
                        .is_some_and(|(_, expected)| line.path.elements() != *expected)
                })
                .count()
        };
        assert_eq!(retained_planet_count, SECULAR_PLANETS.len());

        // Tightening the camera scale while time is unchanged must re-evaluate
        // the pixel allowance. Placing the camera inside every planet orbit
        // makes the conservative allowance zero and refreshes all eight keys.
        app.world_mut()
            .insert_resource(CameraController::new(sun, [0.0; 3], 1.0));
        app.update();
        {
            let world = app.world_mut();
            let mut lines = world.query::<&OrbitLine>();
            for (body_index, expected) in &expected_planet_elements {
                let line = lines
                    .iter(world)
                    .find(|line| line.body_index == *body_index)
                    .unwrap();
                assert_eq!(line.path.elements(), *expected);
            }
        }

        *app.world_mut().resource_mut::<RetainedWriteCounts>() = RetainedWriteCounts::default();
        app.world_mut()
            .resource_mut::<SimulationClock>()
            .0
            .set_t(T_MAX_S);
        app.update();
        let counts = app.world().resource::<RetainedWriteCounts>();
        assert_eq!(
            counts.lines,
            SECULAR_PLANETS.len(),
            "only the eight secular paths should resample after accumulated bounds trip"
        );
        assert!(counts.assets_changed);
    }

    #[test]
    fn local_orbit_setting_hides_only_the_selected_body_path() {
        let catalog = catalog();
        let t_s = t_from_jd_tdb(2_461_042.0);
        let states = propagate_catalog(&catalog, t_s).unwrap();
        let loaded = LoadedCatalog::new(catalog);
        let neptune = loaded.index_of("neptune").unwrap();
        let nereid = loaded.index_of("nereid").unwrap();
        let mut options = ViewOptionsState::default();
        options.set_local_orbit_visible("nereid", false);

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<GizmoAsset>>()
            .insert_resource(loaded)
            .insert_resource(states.clone())
            .insert_resource(CameraController::new(
                neptune,
                states.0[neptune].position_km,
                10_000.0,
            ))
            .insert_resource(SimulationClock(SimClock::new(
                StartMode::FixedEpoch {
                    jd_tdb: 2_461_042.0,
                },
                t_s,
            )))
            .insert_resource(OrbitLineBrightness::default())
            .insert_resource(options)
            .add_systems(Startup, spawn_orbit_lines)
            .add_systems(Update, update_orbit_lines);
        app.update();

        let mut query = app.world_mut().query::<&OrbitLine>();
        let nereid_line = query
            .iter(app.world())
            .find(|line| line.body_index == nereid)
            .unwrap();
        assert_eq!(nereid_line.displayed_alpha, 0.0);
        assert!(query
            .iter(app.world())
            .any(|line| line.body_index != nereid && line.displayed_alpha > 0.0));

        app.world_mut()
            .resource_mut::<ViewOptionsState>()
            .set_local_orbit_visible("nereid", true);
        app.update();
        let mut query = app.world_mut().query::<&OrbitLine>();
        let nereid_line = query
            .iter(app.world())
            .find(|line| line.body_index == nereid)
            .unwrap();
        assert!(nereid_line.displayed_alpha > 0.0);
    }

    #[test]
    fn initial_full_system_view_hides_every_moon_orbit() {
        let mut app = emphasis_orbit_app();
        let moon_indices = app
            .world()
            .resource::<LoadedCatalog>()
            .catalog
            .bodies
            .iter()
            .enumerate()
            .filter_map(|(index, body)| (body.category == Category::Moon).then_some(index))
            .collect::<std::collections::HashSet<_>>();
        let mut query = app.world_mut().query::<&OrbitLine>();
        let lines = query
            .iter(app.world())
            .map(|line| (line.body_index, line.displayed_alpha))
            .collect::<Vec<_>>();

        assert!(lines
            .iter()
            .filter(|(body_index, _)| moon_indices.contains(body_index))
            .all(|(_, alpha)| *alpha == 0.0));
        assert!(lines
            .iter()
            .any(|(body_index, alpha)| !moon_indices.contains(body_index) && *alpha > 0.0));
    }

    #[test]
    fn moon_focus_maps_to_its_parent_system_before_layer_and_local_gates() {
        let catalog = catalog();
        let loaded = LoadedCatalog::new(catalog);
        let io = loaded.index_of("io").unwrap();
        let himalia = loaded.index_of("himalia").unwrap();
        let nereid = loaded.index_of("nereid").unwrap();
        let layers = LayerState::default();
        let mut options = ViewOptionsState::default();

        assert!(orbit_passes_presentation_visibility(
            io,
            &loaded.catalog.bodies[io],
            &loaded,
            Some(io),
            Some(&layers),
            Some(&options),
        ));
        assert!(orbit_passes_presentation_visibility(
            himalia,
            &loaded.catalog.bodies[himalia],
            &loaded,
            Some(io),
            Some(&layers),
            Some(&options),
        ));
        assert!(!orbit_passes_presentation_visibility(
            nereid,
            &loaded.catalog.bodies[nereid],
            &loaded,
            Some(io),
            Some(&layers),
            Some(&options),
        ));

        options.set_local_orbit_visible("himalia", false);
        assert!(!orbit_passes_presentation_visibility(
            himalia,
            &loaded.catalog.bodies[himalia],
            &loaded,
            Some(io),
            Some(&layers),
            Some(&options),
        ));
        let mut hidden_layers = layers;
        hidden_layers.set_visible(LayerId::Moons, false);
        assert!(!orbit_passes_presentation_visibility(
            io,
            &loaded.catalog.bodies[io],
            &loaded,
            Some(io),
            Some(&hidden_layers),
            Some(&options),
        ));
    }

    #[test]
    fn major_and_global_layers_filter_orbits_and_restore_them() {
        let catalog = catalog();
        let t_s = t_from_jd_tdb(2_461_042.0);
        let states = propagate_catalog(&catalog, t_s).unwrap();
        let loaded = LoadedCatalog::new(catalog);
        let jupiter = loaded.index_of("jupiter").unwrap();
        let earth = loaded.index_of("earth").unwrap();
        let io = loaded.index_of("io").unwrap();
        let himalia = loaded.index_of("himalia").unwrap();
        let mut options = ViewOptionsState::default();
        options.set_moon_visibility("jupiter", crate::MoonVisibilityMode::Major);

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<GizmoAsset>>()
            .insert_resource(loaded)
            .insert_resource(states.clone())
            .insert_resource(CameraController::new(
                jupiter,
                states.0[jupiter].position_km,
                10_000.0,
            ))
            .insert_resource(SimulationClock(SimClock::new(
                StartMode::FixedEpoch {
                    jd_tdb: 2_461_042.0,
                },
                t_s,
            )))
            .insert_resource(OrbitLineBrightness::default())
            .insert_resource(LayerState::default())
            .insert_resource(options)
            .add_systems(Startup, spawn_orbit_lines)
            .add_systems(Update, update_orbit_lines);
        app.update();

        let mut query = app.world_mut().query::<&OrbitLine>();
        assert!(query
            .iter(app.world())
            .find(|line| line.body_index == io)
            .is_some_and(|line| line.displayed_alpha > 0.0));
        assert_eq!(
            query
                .iter(app.world())
                .find(|line| line.body_index == himalia)
                .unwrap()
                .displayed_alpha,
            0.0
        );

        app.world_mut()
            .resource_mut::<ViewOptionsState>()
            .set_moon_visibility("jupiter", crate::MoonVisibilityMode::All);
        app.update();
        let mut query = app.world_mut().query::<&OrbitLine>();
        assert!(query
            .iter(app.world())
            .find(|line| line.body_index == himalia)
            .is_some_and(|line| line.displayed_alpha > 0.0));

        app.world_mut()
            .resource_mut::<LayerState>()
            .set_visible(LayerId::Moons, false);
        app.update();
        let moon_indices = app
            .world()
            .resource::<LoadedCatalog>()
            .catalog
            .bodies
            .iter()
            .enumerate()
            .filter_map(|(index, body)| (body.category == Category::Moon).then_some(index))
            .collect::<std::collections::HashSet<_>>();
        let mut query = app.world_mut().query::<&OrbitLine>();
        assert!(query
            .iter(app.world())
            .filter(|line| moon_indices.contains(&line.body_index))
            .all(|line| line.displayed_alpha == 0.0));
        assert!(query
            .iter(app.world())
            .find(|line| line.body_index == earth)
            .is_some_and(|line| line.displayed_alpha > 0.0));

        {
            let mut layers = app.world_mut().resource_mut::<LayerState>();
            layers.set_visible(LayerId::Moons, true);
            layers.set_visible(LayerId::Orbits, false);
        }
        app.update();
        let mut query = app.world_mut().query::<&OrbitLine>();
        assert!(query
            .iter(app.world())
            .all(|line| line.displayed_alpha == 0.0));
    }

    #[test]
    fn parent_relative_render_conversion_never_adds_the_parent_position() {
        let local = [1_000.0, 2_000.0, 3_000.0];
        assert_eq!(
            parent_relative_render_position(local),
            Vec3::new(1.0, 3.0, 2.0)
        );
        assert_eq!(
            crate::add_f64(local, [10_000.0; 3]),
            [11_000.0, 12_000.0, 13_000.0]
        );
    }
}
