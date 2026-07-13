//! WP6 — parent-relative orbit sampling and retained line rendering.
//!
//! Orbit geometry stays in f64 kilometers in the orbiting body's parent
//! frame (Rev C §3 invariant 6). Only the retained gizmo asset contains f32
//! render vertices; its entity translation is independently rebased around
//! the camera focus each frame. Ellipses use uniform true-anomaly spacing,
//! which puts shorter chords near perihelion, while the hyperbolic branch is
//! a strictly open ±25-Julian-year arc centered on perihelion (Rev C §10.2).

use crate::{
    left_panel::body_passes_moon_visibility, rebase_position, BodyStates, CameraController,
    LayerId, LayerState, LoadedCatalog, SimulationClock, SimulationSet, ViewOptionsState,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use sim_core::catalog::{BodyRecord, Category, Elements, Orbit};
use sim_core::kepler::{elements_at, solve_hyperbolic, state_from_elements, KeplerError};
use sim_core::time::JULIAN_YEAR_S;

pub const MIN_ORBIT_VERTICES: usize = 256;
pub const MAX_ORBIT_VERTICES: usize = 768;
pub const HYPERBOLIC_HALF_SPAN_S: f64 = 25.0 * JULIAN_YEAR_S;

const LINE_WIDTH_PX: f32 = 1.5;
// A small negative bias brings a line forward just enough to avoid flicker
// where the path crosses its body without turning it into an overlay.
const ORBIT_DEPTH_BIAS: f32 = -0.001;
const FADE_OUT_ANGULAR_RADIUS: f64 = 0.000_1;
const FULL_ALPHA_ANGULAR_RADIUS: f64 = 0.002;
const EDGE_ON_ALPHA_FACTOR: f64 = 0.2;

/// Parent-frame f64 truth for one sampled conic.
#[derive(Debug, Clone, PartialEq)]
pub struct OrbitPath {
    vertices_parent_km: Vec<[f64; 3]>,
    time_offsets_from_perihelion_s: Option<Vec<f64>>,
    elements: Elements,
    plane_normal: [f64; 3],
    characteristic_radius_km: f64,
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
        self.elements
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
    if !current_t_s.is_finite() {
        return Err(KeplerError::NonFinite);
    }
    let elements = elements_at(orbit, current_t_s);
    let n_rad_s = orbit.mean_motion_rad_per_s(mu_parent_km3_s2);
    // Validate before the hyperbolic endpoint solve so both branches preserve
    // `state_at`'s error semantics for bad elements, mean motion, and parent GM.
    state_from_elements(&elements, mu_parent_km3_s2, n_rad_s, 0.0)?;
    if elements.is_hyperbolic() {
        sample_hyperbolic(elements, mu_parent_km3_s2, n_rad_s)
    } else {
        sample_elliptic(elements, mu_parent_km3_s2, n_rad_s)
    }
}

fn sample_elliptic(
    elements: Elements,
    mu_parent_km3_s2: f64,
    n_rad_s: f64,
) -> Result<OrbitPath, KeplerError> {
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
            state_from_elements(&elements, mu_parent_km3_s2, n_rad_s, mean_anomaly)?.position_km,
        );
    }
    // Copy, rather than recompute, so the seam is bit-identically closed.
    vertices.push(vertices[0]);

    Ok(OrbitPath {
        vertices_parent_km: vertices,
        time_offsets_from_perihelion_s: None,
        elements,
        plane_normal: plane_normal(elements),
        characteristic_radius_km: elements.a_km * (1.0 + elements.e),
    })
}

fn sample_hyperbolic(
    elements: Elements,
    mu_parent_km3_s2: f64,
    n_rad_s: f64,
) -> Result<OrbitPath, KeplerError> {
    let count = orbit_vertex_count(elements.e);
    let last = count - 1;
    let mean_anomaly_limit = n_rad_s * HYPERBOLIC_HALF_SPAN_S;
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
            (mean_anomaly, mean_anomaly / n_rad_s)
        };
        vertices.push(
            state_from_elements(&elements, mu_parent_km3_s2, n_rad_s, mean_anomaly)?.position_km,
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
        elements,
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
    let rgb = match (body.category, body.id.as_str()) {
        (Category::Planet, "mercury") => [158, 158, 158],
        (Category::Planet, "venus") => [222, 184, 135],
        (Category::Planet, "earth") => [86, 141, 235],
        (Category::Planet, "mars") => [204, 101, 66],
        (Category::Planet, "jupiter") => [211, 177, 140],
        (Category::Planet, "saturn") => [226, 205, 159],
        (Category::Planet, "uranus") => [148, 207, 216],
        (Category::Planet, "neptune") => [99, 125, 222],
        (Category::Planet, _) => [150, 180, 230],
        (Category::DwarfPlanet, _) => [186, 156, 255],
        (Category::Asteroid, _) => [214, 160, 92],
        (Category::Moon, _) => [198, 189, 175],
        (Category::Comet, _) => [96, 220, 238],
        (Category::Star, _) => [255, 214, 140],
    };
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
struct OrbitLine {
    body_index: usize,
    parent_index: usize,
    palette: OrbitPaletteEntry,
    path: OrbitPath,
    displayed_alpha: f32,
    displayed_brightness: f32,
}

#[derive(SystemParam)]
struct OrbitLineRenderOptions<'w> {
    brightness: Res<'w, OrbitLineBrightness>,
    view_options: Option<Res<'w, ViewOptionsState>>,
    layers: Option<Res<'w, LayerState>>,
}

pub struct OrbitLinesPlugin;

impl Plugin for OrbitLinesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OrbitLineBrightness>()
            .add_systems(Startup, spawn_orbit_lines)
            .add_systems(Update, update_orbit_lines.in_set(SimulationSet::Render));
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
    let brightness = options.brightness.sanitized();

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
        let selected = camera
            .as_ref()
            .is_some_and(|camera| camera.selected_body_index() == body_index);
        let orbit_layer_visible = options
            .layers
            .as_ref()
            .is_none_or(|layers| layers.is_visible(LayerId::Orbits));
        let local_visible = options.view_options.as_ref().is_none_or(|view_options| {
            view_options.local_orbit_visible(&body.id)
                && (selected || body_passes_moon_visibility(body, view_options))
        });
        let displayed_alpha = if orbit_layer_visible && local_visible {
            palette.base_alpha
        } else {
            0.0
        };
        let mut asset = GizmoAsset::default();
        rebuild_asset(
            &mut asset,
            &path,
            line_color(palette.rgb, displayed_alpha, brightness),
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
            },
            Gizmo {
                handle,
                line_config: GizmoLineConfig {
                    width: LINE_WIDTH_PX,
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
    clock: Res<SimulationClock>,
    options: OrbitLineRenderOptions,
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
    mut lines: Query<(&mut OrbitLine, &mut Transform, &Gizmo)>,
) {
    let (Some(loaded), Some(states)) = (loaded, states) else {
        return;
    };
    let focus_position_km = camera.focus_position_km();
    let camera_position_km = camera.camera_position_km();
    let brightness = options.brightness.sanitized();
    for (mut line, mut transform, gizmo) in &mut lines {
        let Some(parent_state) = states.0.get(line.parent_index) else {
            continue;
        };
        transform.translation = rebase_position(parent_state.position_km, focus_position_km);

        let body = &loaded.catalog.bodies[line.body_index];
        let Some(orbit) = body.orbit.as_ref() else {
            continue;
        };
        let Some(mu_parent_km3_s2) = loaded.catalog.bodies[line.parent_index].gm_km3_s2 else {
            continue;
        };

        let mut rebuilt = false;
        let current_elements = elements_at(orbit, clock.0.t());
        if current_elements != line.path.elements {
            match sample_orbit(orbit, mu_parent_km3_s2, clock.0.t()) {
                Ok(path) => {
                    line.path = path;
                    rebuilt = true;
                }
                Err(error) => {
                    error!("could not refresh orbit line for '{}': {error}", body.id);
                    continue;
                }
            }
        }

        let parent_to_camera = sub(camera_position_km, parent_state.position_km);
        let camera_distance_km = norm(parent_to_camera);
        let view_angle_cos = if camera_distance_km > 0.0 {
            dot(line.path.plane_normal, parent_to_camera) / camera_distance_km
        } else {
            1.0
        };
        let orbit_layer_visible = options
            .layers
            .as_ref()
            .is_none_or(|layers| layers.is_visible(LayerId::Orbits));
        let local_visible = options.view_options.as_ref().is_none_or(|view_options| {
            view_options.local_orbit_visible(&body.id)
                && (camera.selected_body_index() == line.body_index
                    || body_passes_moon_visibility(body, view_options))
        });
        let displayed_alpha = if orbit_layer_visible && local_visible {
            quantized_alpha(orbit_alpha(
                line.palette.base_alpha,
                camera_distance_km,
                line.path.characteristic_radius_km,
                view_angle_cos,
            ))
        } else {
            0.0
        };
        let color_changed =
            displayed_alpha != line.displayed_alpha || brightness != line.displayed_brightness;
        line.displayed_alpha = displayed_alpha;
        line.displayed_brightness = brightness;

        let Some(mut asset) = gizmo_assets.get_mut(&gizmo.handle) else {
            continue;
        };
        let color = line_color(line.palette.rgb, displayed_alpha, brightness);
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

fn parent_relative_render_position(position_km: [f64; 3]) -> Vec3 {
    rebase_position(position_km, [0.0; 3])
}

fn line_color(rgb: [u8; 3], alpha: f32, brightness: f32) -> LinearRgba {
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
    use crate::{load_catalog_text, propagate_catalog};
    use sim_core::catalog::Catalog;
    use sim_core::kepler::state_at;
    use sim_core::time::{t_from_jd_tdb, SimClock, StartMode};

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
            let perihelion_index = if path.elements.is_hyperbolic() {
                path.vertices_parent_km.len() / 2
            } else {
                0
            };
            let sampled = norm(path.vertices_parent_km[perihelion_index]);
            let expected = if path.elements.is_hyperbolic() {
                path.elements.a_km.abs() * (path.elements.e - 1.0)
            } else {
                path.elements.a_km * (1.0 - path.elements.e)
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
    fn palette_has_distinct_planets_and_shared_category_defaults() {
        let catalog = catalog();
        let index = catalog.id_index();
        let mut planet_colors: Vec<_> = [
            "mercury", "venus", "earth", "mars", "jupiter", "saturn", "uranus", "neptune",
        ]
        .iter()
        .map(|id| orbit_palette(&catalog.bodies[*index.get(*id).unwrap()]).rgb)
        .collect();
        planet_colors.sort_unstable();
        planet_colors.dedup();
        assert_eq!(planet_colors.len(), 8);

        let nereid = orbit_palette(&catalog.bodies[*index.get("nereid").unwrap()]);
        let triton = orbit_palette(&catalog.bodies[*index.get("triton").unwrap()]);
        assert_eq!(nereid, triton);

        let mut category_defaults: Vec<_> = ["pluto", "pallas", "nereid", "halley"]
            .iter()
            .map(|id| orbit_palette(&catalog.bodies[*index.get(*id).unwrap()]).rgb)
            .collect();
        category_defaults.sort_unstable();
        category_defaults.dedup();
        assert_eq!(category_defaults.len(), 4);
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
            .add_systems(Startup, spawn_orbit_lines)
            .add_systems(Update, update_orbit_lines);
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
    fn major_and_global_layers_filter_orbits_and_restore_them() {
        let catalog = catalog();
        let t_s = t_from_jd_tdb(2_461_042.0);
        let states = propagate_catalog(&catalog, t_s).unwrap();
        let loaded = LoadedCatalog::new(catalog);
        let jupiter = loaded.index_of("jupiter").unwrap();
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
            .set_visible(LayerId::Orbits, false);
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
