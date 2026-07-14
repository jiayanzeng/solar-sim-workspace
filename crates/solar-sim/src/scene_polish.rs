//! WP13 temporal-aliasing emphasis and scene lighting — Rev C §§7, 10.4–10.5.
//!
//! Phase thresholds are derived once from the catalog's elliptic periods.
//! Runtime state is presentation-only: it cross-fades body materials, label
//! colors, and orbit brightness without changing propagation or picking truth.

use crate::{BodyVisual, LoadedCatalog, SimulationClock, SimulationSet};
use bevy::{color::Alpha, prelude::*};
use sim_core::catalog::Category;

pub const EMPHASIS_ENGAGE_PHASE_RAD: f64 = 0.15;
pub const EMPHASIS_RELEASE_PHASE_RAD: f64 = 0.12;
pub const EMPHASIS_CROSSFADE_S: f32 = 0.25;
pub const EMPHASIZED_ORBIT_BRIGHTNESS: f32 = 3.5;
pub const SUN_LIGHT_INTENSITY_LUMENS: f32 = 3.0e15;
pub const SUN_LIGHT_RANGE_UNITS: f32 = 2.0e8;
pub const AMBIENT_BRIGHTNESS: f32 = 20.0;

#[derive(Component, Debug, Clone, Copy)]
pub struct SunLight;

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrbitEmphasisOnset;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyOrbitEmphasis {
    pub period_s: f64,
    pub engage_simulated_step_s: f64,
    pub release_simulated_step_s: f64,
    engaged: bool,
    blend: f32,
}

impl BodyOrbitEmphasis {
    fn from_period(period_s: f64) -> Option<Self> {
        if !period_s.is_finite() || period_s <= 0.0 {
            return None;
        }
        Some(Self {
            period_s,
            engage_simulated_step_s: simulated_step_for_phase(EMPHASIS_ENGAGE_PHASE_RAD, period_s),
            release_simulated_step_s: simulated_step_for_phase(
                EMPHASIS_RELEASE_PHASE_RAD,
                period_s,
            ),
            engaged: false,
            blend: 0.0,
        })
    }

    pub fn is_engaged(self) -> bool {
        self.engaged
    }

    pub fn blend(self) -> f32 {
        self.blend
    }
}

#[derive(Resource, Debug, Default)]
pub struct OrbitEmphasisState {
    bodies: Vec<Option<BodyOrbitEmphasis>>,
    any_engaged: bool,
}

impl OrbitEmphasisState {
    pub fn body(&self, body_index: usize) -> Option<BodyOrbitEmphasis> {
        self.bodies.get(body_index).copied().flatten()
    }

    pub fn body_alpha(&self, body_index: usize) -> f32 {
        self.body(body_index).map_or(1.0, |body| 1.0 - body.blend)
    }

    pub fn orbit_brightness(&self, body_index: usize) -> f32 {
        self.body(body_index).map_or(1.0, |body| {
            1.0 + (EMPHASIZED_ORBIT_BRIGHTNESS - 1.0) * body.blend
        })
    }

    pub fn any_engaged(&self) -> bool {
        self.any_engaged
    }

    fn update(&mut self, simulated_step_s: f64, wall_delta_s: f32) -> bool {
        let was_engaged = self.any_engaged;
        let fade_step = if EMPHASIS_CROSSFADE_S > 0.0 {
            wall_delta_s.max(0.0) / EMPHASIS_CROSSFADE_S
        } else {
            1.0
        };
        for body in self.bodies.iter_mut().flatten() {
            body.engaged = hysteresis_state(
                body.engaged,
                simulated_step_s,
                body.engage_simulated_step_s,
                body.release_simulated_step_s,
            );
            body.blend = move_toward(body.blend, f32::from(body.engaged), fade_step);
        }
        self.any_engaged = self.bodies.iter().flatten().any(|body| body.engaged);
        !was_engaged && self.any_engaged
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct OrbitEmphasisSet;

pub struct ScenePolishPlugin;

impl Plugin for ScenePolishPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.18, 0.22, 0.3),
            brightness: AMBIENT_BRIGHTNESS,
            affects_lightmapped_meshes: true,
        })
        .init_resource::<OrbitEmphasisState>()
        .add_message::<OrbitEmphasisOnset>()
        .add_systems(Startup, initialize_orbit_emphasis)
        .add_systems(
            Update,
            (update_orbit_emphasis, apply_body_emphasis_alpha)
                .chain()
                .in_set(SimulationSet::Render)
                .in_set(OrbitEmphasisSet),
        );
    }
}

/// Parent-relative phase advance for one rendered frame.
pub fn phase_step_rad(
    rate_seconds_per_second: f64,
    wall_delta_s: f64,
    orbital_period_s: f64,
) -> f64 {
    if !rate_seconds_per_second.is_finite()
        || !wall_delta_s.is_finite()
        || !orbital_period_s.is_finite()
        || orbital_period_s <= 0.0
    {
        return 0.0;
    }
    std::f64::consts::TAU * rate_seconds_per_second.abs() * wall_delta_s.max(0.0) / orbital_period_s
}

pub fn simulated_step_for_phase(phase_rad: f64, orbital_period_s: f64) -> f64 {
    if !phase_rad.is_finite()
        || phase_rad < 0.0
        || !orbital_period_s.is_finite()
        || orbital_period_s <= 0.0
    {
        return 0.0;
    }
    phase_rad * orbital_period_s / std::f64::consts::TAU
}

pub fn hysteresis_state(
    engaged: bool,
    simulated_step_s: f64,
    engage_step_s: f64,
    release_step_s: f64,
) -> bool {
    if !simulated_step_s.is_finite()
        || !engage_step_s.is_finite()
        || !release_step_s.is_finite()
        || engage_step_s <= release_step_s
    {
        return false;
    }
    if engaged {
        simulated_step_s > release_step_s
    } else {
        simulated_step_s >= engage_step_s
    }
}

fn move_toward(current: f32, target: f32, maximum_delta: f32) -> f32 {
    let delta = target - current;
    if delta.abs() <= maximum_delta {
        target
    } else {
        current + delta.signum() * maximum_delta.max(0.0)
    }
}

fn initialize_orbit_emphasis(
    loaded: Option<Res<LoadedCatalog>>,
    mut emphasis: ResMut<OrbitEmphasisState>,
) {
    let Some(loaded) = loaded else {
        return;
    };
    emphasis.bodies = loaded
        .catalog
        .bodies
        .iter()
        .map(|body| {
            let parent_id = body.parent.as_deref()?;
            let parent_index = loaded.index_of(parent_id)?;
            let parent_gm = loaded.catalog.bodies.get(parent_index)?.gm_km3_s2?;
            let period_s = body.orbit.as_ref()?.period_s(parent_gm)?;
            BodyOrbitEmphasis::from_period(period_s)
        })
        .collect();
    emphasis.any_engaged = false;
}

fn update_orbit_emphasis(
    clock: Res<SimulationClock>,
    time: Res<Time<Real>>,
    mut emphasis: ResMut<OrbitEmphasisState>,
    mut onsets: MessageWriter<OrbitEmphasisOnset>,
) {
    let simulated_step_s = if clock.0.is_playing() {
        clock.0.rate().seconds_per_second().abs() * time.delta_secs_f64()
    } else {
        0.0
    };
    if emphasis.update(simulated_step_s, time.delta_secs()) {
        onsets.write(OrbitEmphasisOnset);
    }
}

fn apply_body_emphasis_alpha(
    loaded: Option<Res<LoadedCatalog>>,
    emphasis: Res<OrbitEmphasisState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    bodies: Query<(&BodyVisual, &MeshMaterial3d<StandardMaterial>)>,
) {
    let Some(loaded) = loaded else {
        return;
    };
    for (visual, material_handle) in &bodies {
        let Some(body) = loaded.catalog.bodies.get(visual.index) else {
            continue;
        };
        let Some(mut material) = materials.get_mut(&material_handle.0) else {
            continue;
        };
        let alpha = emphasis.body_alpha(visual.index);
        material.base_color = if body.texture.is_some() && material.base_color_texture.is_some() {
            Color::WHITE.with_alpha(alpha)
        } else {
            let (red, green, blue) = body.color_srgb;
            Color::srgb_u8(red, green, blue).with_alpha(alpha)
        };
        material.alpha_mode = if body.category != Category::Star && alpha < 0.999 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_catalog_text;
    use sim_core::time::{RateIndex, JULIAN_YEAR_S};

    const REAL_CATALOG: &str = include_str!("../../../assets/catalog.ron");

    fn emphasis_for_real_catalog() -> (LoadedCatalog, OrbitEmphasisState) {
        let loaded = LoadedCatalog::new(load_catalog_text(REAL_CATALOG).unwrap());
        let mut app = App::new();
        app.insert_resource(loaded)
            .init_resource::<OrbitEmphasisState>()
            .add_systems(Startup, initialize_orbit_emphasis);
        app.update();
        let loaded = app.world_mut().remove_resource::<LoadedCatalog>().unwrap();
        let state = app
            .world_mut()
            .remove_resource::<OrbitEmphasisState>()
            .unwrap();
        (loaded, state)
    }

    #[test]
    fn rate_period_math_predicts_mercury_but_not_neptune_at_one_hundred_years() {
        let (loaded, mut state) = emphasis_for_real_catalog();
        let mercury = loaded.index_of("mercury").unwrap();
        let neptune = loaded.index_of("neptune").unwrap();
        let sedna = loaded.index_of("sedna").unwrap();
        let halley = loaded.index_of("halley").unwrap();
        let hale_bopp = loaded.index_of("hale_bopp").unwrap();
        let rate = RateIndex::new(12).unwrap().seconds_per_second();
        let frame_s = 1.0 / 60.0;

        let mercury_phase = phase_step_rad(rate, frame_s, state.body(mercury).unwrap().period_s);
        let neptune_phase = phase_step_rad(rate, frame_s, state.body(neptune).unwrap().period_s);
        let sedna_phase = phase_step_rad(rate, frame_s, state.body(sedna).unwrap().period_s);
        let halley_phase = phase_step_rad(rate, frame_s, state.body(halley).unwrap().period_s);
        let hale_bopp_phase =
            phase_step_rad(rate, frame_s, state.body(hale_bopp).unwrap().period_s);
        assert!(mercury_phase > EMPHASIS_ENGAGE_PHASE_RAD);
        assert!(neptune_phase < EMPHASIS_RELEASE_PHASE_RAD);
        assert!(sedna_phase < neptune_phase);
        assert!(halley_phase < EMPHASIS_ENGAGE_PHASE_RAD);
        assert!(hale_bopp_phase < EMPHASIS_RELEASE_PHASE_RAD);

        let direct = std::f64::consts::TAU * (100.0 * JULIAN_YEAR_S / 60.0)
            / state.body(mercury).unwrap().period_s;
        assert!((mercury_phase - direct).abs() < 1.0e-12);

        assert!(state.update(rate * frame_s, EMPHASIS_CROSSFADE_S));
        for id in ["mercury", "venus", "earth", "mars", "jupiter", "saturn"] {
            let index = loaded.index_of(id).unwrap();
            assert_eq!(state.body_alpha(index), 0.0, "{id}");
            assert_eq!(
                state.orbit_brightness(index),
                EMPHASIZED_ORBIT_BRIGHTNESS,
                "{id}"
            );
        }
        for id in ["uranus", "neptune", "sedna", "halley", "hale_bopp"] {
            let index = loaded.index_of(id).unwrap();
            assert_eq!(state.body_alpha(index), 1.0, "{id}");
            assert_eq!(state.orbit_brightness(index), 1.0, "{id}");
        }
    }

    #[test]
    fn hysteresis_has_one_onset_and_no_boundary_flicker() {
        let period_s = 100.0;
        let engage = simulated_step_for_phase(EMPHASIS_ENGAGE_PHASE_RAD, period_s);
        let release = simulated_step_for_phase(EMPHASIS_RELEASE_PHASE_RAD, period_s);
        let mut state = OrbitEmphasisState {
            bodies: vec![BodyOrbitEmphasis::from_period(period_s)],
            any_engaged: false,
        };

        assert!(!state.update(engage.next_down(), 0.1));
        assert!(state.update(engage, 0.1));
        assert!(!state.update((engage + release) * 0.5, 0.1));
        assert!(state.body(0).unwrap().is_engaged());
        assert!(!state.update(release.next_up(), 0.1));
        assert!(state.body(0).unwrap().is_engaged());
        assert!(!state.update(release, 0.1));
        assert!(!state.body(0).unwrap().is_engaged());
        assert!(state.update(engage, 0.1), "a new crossing is a new onset");
    }

    #[test]
    fn hyperbolic_bodies_have_no_period_threshold() {
        let (loaded, state) = emphasis_for_real_catalog();
        let atlas = loaded.index_of("3i_atlas").unwrap();
        assert_eq!(state.body(atlas), None);
    }

    #[test]
    fn scene_uses_low_cool_ambient_light() {
        let mut app = App::new();
        app.add_plugins(ScenePolishPlugin);
        let ambient = app.world().resource::<GlobalAmbientLight>();
        assert_eq!(ambient.brightness, AMBIENT_BRIGHTNESS);
        assert_eq!(ambient.color, Color::srgb(0.18, 0.22, 0.3));
    }
}
