//! UIO-3b — render-only category apparent size and density fallback (Rev E §10.1).
//!
//! The catalog radius and f64 propagated state remain authoritative. This
//! module applies a projection-derived ×1 floor before optional visual
//! exaggeration, only when writing a sphere's render transform.

use bevy::prelude::*;
use sim_core::catalog::Category;

use crate::{rendered_body_radius_units, BodyVisual, LoadedCatalog, ViewOptionsState};

pub const PLANET_MIN_BODY_DIAMETER_LOGICAL_PX: f64 = 12.0;
pub const DWARF_PLANET_MIN_BODY_DIAMETER_LOGICAL_PX: f64 = 8.0;
pub const OTHER_MIN_BODY_DIAMETER_LOGICAL_PX: f64 = 3.0;
pub const DENSE_PLANET_MIN_BODY_DIAMETER_LOGICAL_PX: f64 = 8.0;
pub const DENSE_DWARF_PLANET_MIN_BODY_DIAMETER_LOGICAL_PX: f64 = 6.0;

type ApparentSizeCameraQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Camera, &'static Projection, &'static Transform),
    (With<Camera3d>, Without<BodyVisual>),
>;
type ApparentSizeBodyQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static BodyVisual,
        &'static mut Transform,
        Option<&'static Visibility>,
    ),
    Without<Camera3d>,
>;

pub const fn category_min_body_diameter_logical_px(category: Category) -> f64 {
    match category {
        Category::Star => 0.0,
        Category::Planet => PLANET_MIN_BODY_DIAMETER_LOGICAL_PX,
        Category::DwarfPlanet => DWARF_PLANET_MIN_BODY_DIAMETER_LOGICAL_PX,
        Category::Asteroid | Category::Comet | Category::Moon => OTHER_MIN_BODY_DIAMETER_LOGICAL_PX,
    }
}

/// Reduces only an unselected planet/dwarf floor when projected centers
/// converge. The nearest-center distance makes the transition continuous;
/// the reviewed 8/6 px lower bounds prevent disappearance in dense overviews.
pub fn density_adjusted_body_diameter_logical_px(
    category: Category,
    selected: bool,
    nearest_center_distance_logical_px: Option<f64>,
) -> f64 {
    let base = category_min_body_diameter_logical_px(category);
    let dense_minimum = match category {
        Category::Planet => DENSE_PLANET_MIN_BODY_DIAMETER_LOGICAL_PX,
        Category::DwarfPlanet => DENSE_DWARF_PLANET_MIN_BODY_DIAMETER_LOGICAL_PX,
        _ => return base,
    };
    if selected {
        return base;
    }
    nearest_center_distance_logical_px
        .filter(|distance| distance.is_finite() && *distance >= 0.0)
        .map_or(base, |distance| distance.clamp(dense_minimum, base))
}

/// Projected diameter for a centered sphere under the app's perspective
/// projection. `None` means the inputs cannot describe a visible projection.
pub fn projected_body_diameter_logical_px(
    radius_units: f64,
    camera_distance_units: f64,
    projection: &Projection,
    viewport_height_logical_px: f64,
) -> Option<f64> {
    let Projection::Perspective(perspective) = projection else {
        return None;
    };
    if !radius_units.is_finite()
        || radius_units <= 0.0
        || !camera_distance_units.is_finite()
        || camera_distance_units <= 0.0
        || !viewport_height_logical_px.is_finite()
        || viewport_height_logical_px <= 0.0
    {
        return None;
    }
    let half_fov_tangent = (f64::from(perspective.fov) * 0.5).tan();
    if !half_fov_tangent.is_finite() || half_fov_tangent <= 0.0 {
        return None;
    }
    Some(radius_units * viewport_height_logical_px / (camera_distance_units * half_fov_tangent))
}

/// Applies the ×1 logical-pixel diameter floor before visual exaggeration.
/// Non-perspective or invalid projection inputs preserve the physical render
/// radius times the requested multiplier rather than inventing a camera model.
pub fn clamped_body_radius_units(
    physical_radius_units: f64,
    visual_multiplier: f64,
    minimum_diameter_logical_px: f64,
    camera_distance_units: f64,
    projection: &Projection,
    viewport_height_logical_px: f64,
) -> f64 {
    let physical_render_radius = physical_radius_units * visual_multiplier;
    let Projection::Perspective(perspective) = projection else {
        return physical_render_radius;
    };
    if !physical_radius_units.is_finite()
        || physical_radius_units <= 0.0
        || !visual_multiplier.is_finite()
        || visual_multiplier <= 0.0
        || !minimum_diameter_logical_px.is_finite()
        || minimum_diameter_logical_px < 0.0
        || !camera_distance_units.is_finite()
        || camera_distance_units <= 0.0
        || !viewport_height_logical_px.is_finite()
        || viewport_height_logical_px <= 0.0
    {
        return physical_render_radius;
    }
    let half_fov_tangent = (f64::from(perspective.fov) * 0.5).tan();
    if !half_fov_tangent.is_finite() || half_fov_tangent <= 0.0 {
        return physical_render_radius;
    }
    let minimum_radius_at_x1 =
        minimum_diameter_logical_px * camera_distance_units * half_fov_tangent
            / viewport_height_logical_px;
    physical_radius_units.max(minimum_radius_at_x1) * visual_multiplier
}

pub(crate) fn apply_minimum_apparent_body_size(
    settings: Res<ViewOptionsState>,
    loaded: Option<Res<LoadedCatalog>>,
    camera_controller: Option<Res<crate::CameraController>>,
    cameras: ApparentSizeCameraQuery,
    mut bodies: ApparentSizeBodyQuery,
) {
    let Some(loaded) = loaded else {
        return;
    };
    let Ok((camera, projection, camera_transform)) = cameras.single() else {
        return;
    };
    let Some(viewport_size) = camera.logical_viewport_size() else {
        return;
    };
    let viewport_height = f64::from(viewport_size.y);
    let camera_global = GlobalTransform::from(*camera_transform);
    let projected_density_centers = bodies
        .iter_mut()
        .filter_map(|(visual, transform, visibility)| {
            if visibility.is_some_and(|visibility| *visibility == Visibility::Hidden) {
                return None;
            }
            let body = loaded.catalog.bodies.get(visual.index)?;
            if !matches!(body.category, Category::Planet | Category::DwarfPlanet) {
                return None;
            }
            camera
                .world_to_viewport(&camera_global, transform.translation)
                .ok()
                .map(|center| (visual.index, center.as_dvec2()))
        })
        .collect::<Vec<_>>();
    let selected_body_index = camera_controller.map(|controller| controller.selected_body_index());
    // Bodies and the camera are expressed relative to the same identity focus
    // anchor during Update, after the Origin and Camera sets have run. Using
    // local translations here therefore avoids consulting or mutating f64
    // simulation truth and still reflects the current frame's camera motion.
    for (visual, mut transform, _) in &mut bodies {
        let Some(body) = loaded.catalog.bodies.get(visual.index) else {
            continue;
        };
        let physical_radius = f64::from(rendered_body_radius_units(
            body.radius_km,
            crate::BodySizeScale::X1,
        ));
        let multiplier = f64::from(settings.body_size().multiplier());
        let desired_radius = if body.category == Category::Star {
            physical_radius * multiplier
        } else {
            let camera_distance =
                f64::from(camera_transform.translation.distance(transform.translation));
            let nearest_center_distance = projected_density_centers
                .iter()
                .find(|(index, _)| *index == visual.index)
                .and_then(|(_, center)| {
                    projected_density_centers
                        .iter()
                        .filter(|(index, _)| *index != visual.index)
                        .map(|(_, other)| center.distance(*other))
                        .reduce(f64::min)
                });
            let minimum_diameter = density_adjusted_body_diameter_logical_px(
                body.category,
                selected_body_index == Some(visual.index),
                nearest_center_distance,
            );
            clamped_body_radius_units(
                physical_radius,
                multiplier,
                minimum_diameter,
                camera_distance,
                projection,
                viewport_height,
            )
        };
        if !desired_radius.is_finite() || desired_radius <= 0.0 {
            continue;
        }
        let desired_scale = Vec3::splat(desired_radius as f32);
        if transform.scale != desired_scale {
            transform.scale = desired_scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{full_system_framing_distance_units, HeadlessSimulation};
    use crate::labels::inflated_pick_radius;
    use crate::surface_textures::SATURN_RING_OUTER_RADIUS;
    use crate::{
        load_catalog_text, propagate_catalog, rebase_position, BodySizeScale, CameraController,
        KM_PER_RENDER_UNIT,
    };
    use bevy::camera::{CameraProjection, ComputedCameraValues, RenderTargetInfo, Viewport};
    use sim_core::time::{t_from_jd_tdb, DEFAULT_START_EPOCH_JD_TDB};

    const REAL_CATALOG: &str = include_str!("../../../assets/catalog.ron");

    fn perspective() -> Projection {
        Projection::Perspective(PerspectiveProjection::default())
    }

    fn camera_for_test(width: u32, height: u32) -> (Camera, Projection) {
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

    #[test]
    fn category_floors_and_density_fallback_are_exact_and_selection_safe() {
        assert_eq!(category_min_body_diameter_logical_px(Category::Star), 0.0);
        assert_eq!(
            category_min_body_diameter_logical_px(Category::Planet),
            12.0
        );
        assert_eq!(
            category_min_body_diameter_logical_px(Category::DwarfPlanet),
            8.0
        );
        for category in [Category::Moon, Category::Asteroid, Category::Comet] {
            assert_eq!(category_min_body_diameter_logical_px(category), 3.0);
        }

        assert_eq!(
            density_adjusted_body_diameter_logical_px(Category::Planet, false, None),
            12.0
        );
        assert_eq!(
            density_adjusted_body_diameter_logical_px(Category::Planet, false, Some(10.0)),
            10.0
        );
        assert_eq!(
            density_adjusted_body_diameter_logical_px(Category::Planet, false, Some(0.0)),
            8.0
        );
        assert_eq!(
            density_adjusted_body_diameter_logical_px(Category::Planet, true, Some(0.0)),
            12.0
        );
        assert_eq!(
            density_adjusted_body_diameter_logical_px(Category::DwarfPlanet, false, Some(7.0)),
            7.0
        );
        assert_eq!(
            density_adjusted_body_diameter_logical_px(Category::DwarfPlanet, false, Some(0.0)),
            6.0
        );
        assert_eq!(
            density_adjusted_body_diameter_logical_px(Category::DwarfPlanet, true, Some(0.0)),
            8.0
        );
        assert_eq!(
            density_adjusted_body_diameter_logical_px(Category::Moon, false, Some(0.0)),
            3.0
        );
    }

    #[test]
    fn floor_before_exaggeration_is_continuous_and_preserves_one_ten_fifty() {
        let projection = perspective();
        let distance = 1_000.0;
        let viewport_height = 600.0;
        let minimum_diameter = DWARF_PLANET_MIN_BODY_DIAMETER_LOGICAL_PX;
        let Projection::Perspective(perspective) = &projection else {
            unreachable!();
        };
        let boundary = minimum_diameter * distance * (f64::from(perspective.fov) * 0.5).tan()
            / viewport_height;

        assert_eq!(
            clamped_body_radius_units(
                boundary * 0.5,
                1.0,
                minimum_diameter,
                distance,
                &projection,
                viewport_height
            )
            .to_bits(),
            boundary.to_bits()
        );
        assert_eq!(
            clamped_body_radius_units(
                boundary,
                1.0,
                minimum_diameter,
                distance,
                &projection,
                viewport_height
            )
            .to_bits(),
            boundary.to_bits()
        );
        assert_eq!(
            clamped_body_radius_units(
                boundary * 2.0,
                1.0,
                minimum_diameter,
                distance,
                &projection,
                viewport_height
            )
            .to_bits(),
            (boundary * 2.0).to_bits()
        );
        let diameter =
            projected_body_diameter_logical_px(boundary, distance, &projection, viewport_height)
                .unwrap();
        assert!((diameter - minimum_diameter).abs() < 1.0e-12);

        let outputs = BodySizeScale::ALL.map(|scale| {
            clamped_body_radius_units(
                boundary * 0.5,
                f64::from(scale.multiplier()),
                minimum_diameter,
                distance,
                &projection,
                viewport_height,
            )
        });
        assert_eq!(outputs, [boundary, boundary * 10.0, boundary * 50.0]);
    }

    #[test]
    fn every_non_sun_catalog_body_reaches_its_category_floor_in_the_full_system_view() {
        let catalog = load_catalog_text(REAL_CATALOG).unwrap();
        let loaded = LoadedCatalog::new(catalog.clone());
        let t = t_from_jd_tdb(DEFAULT_START_EPOCH_JD_TDB);
        let states = propagate_catalog(&catalog, t).unwrap();
        let sun = loaded.index_of("sun").unwrap();
        let distance = full_system_framing_distance_units(&loaded);
        let mut controller = CameraController::new(sun, states.0[sun].position_km, distance);
        controller.set_initial_pose(0.0, 0.35, distance);
        let camera_position = controller.render_translation();
        let projection = perspective();

        for (index, body) in catalog.bodies.iter().enumerate() {
            let physical_radius = body.radius_km / KM_PER_RENDER_UNIT;
            if body.id == "sun" {
                continue;
            }
            let body_position =
                rebase_position(states.0[index].position_km, states.0[sun].position_km);
            let camera_distance = f64::from(camera_position.distance(body_position));
            let minimum_diameter = category_min_body_diameter_logical_px(body.category);
            let rendered_radius = clamped_body_radius_units(
                physical_radius,
                1.0,
                minimum_diameter,
                camera_distance,
                &projection,
                600.0,
            );
            let diameter = projected_body_diameter_logical_px(
                rendered_radius,
                camera_distance,
                &projection,
                600.0,
            )
            .unwrap();
            assert!(
                diameter + 1.0e-12 >= minimum_diameter,
                "{} projected to {diameter} logical px, below its {minimum_diameter} px floor",
                body.id
            );
        }
    }

    #[test]
    fn reviewed_bodies_and_saturn_rings_preserve_one_ten_fifty_projected_diameters() {
        let catalog = load_catalog_text(REAL_CATALOG).unwrap();
        let loaded = LoadedCatalog::new(catalog);
        let projection = perspective();
        let viewport_height = 600.0;
        let full_system_distance = full_system_framing_distance_units(&loaded);
        let cases = [
            ("ceres", full_system_distance, 1.0),
            ("earth", full_system_distance, 1.0),
            ("saturn", 430.0, f64::from(SATURN_RING_OUTER_RADIUS)),
            ("earth", 100.0, 1.0),
        ];

        for (body_id, camera_distance, aggregate_radius_multiplier) in cases {
            let index = loaded.index_of(body_id).unwrap();
            let body = &loaded.catalog.bodies[index];
            let physical_radius = body.radius_km / KM_PER_RENDER_UNIT;
            let minimum_diameter = category_min_body_diameter_logical_px(body.category);
            let projected = BodySizeScale::ALL.map(|scale| {
                let rendered_radius = clamped_body_radius_units(
                    physical_radius,
                    f64::from(scale.multiplier()),
                    minimum_diameter,
                    camera_distance,
                    &projection,
                    viewport_height,
                ) * aggregate_radius_multiplier;
                projected_body_diameter_logical_px(
                    rendered_radius,
                    camera_distance,
                    &projection,
                    viewport_height,
                )
                .unwrap()
            });
            assert!(
                (projected[1] / projected[0] - 10.0).abs() < 1.0e-12,
                "{body_id} ×10 ratio was {}",
                projected[1] / projected[0]
            );
            assert!(
                (projected[2] / projected[0] - 50.0).abs() < 1.0e-12,
                "{body_id} ×50 ratio was {}",
                projected[2] / projected[0]
            );
        }
    }

    #[test]
    fn render_system_clamps_a_tiny_body_and_leaves_the_sun_exact() {
        let catalog = load_catalog_text(REAL_CATALOG).unwrap();
        let loaded = LoadedCatalog::new(catalog);
        let sun = loaded.index_of("sun").unwrap();
        let tiny = loaded.index_of("3i_atlas").unwrap();
        let sun_radius =
            rendered_body_radius_units(loaded.catalog.bodies[sun].radius_km, BodySizeScale::X1);
        let tiny_radius =
            rendered_body_radius_units(loaded.catalog.bodies[tiny].radius_km, BodySizeScale::X1);
        let camera_translation = Vec3::new(0.0, 0.0, 10_000.0);
        let (camera, projection) = camera_for_test(960, 600);

        let mut app = App::new();
        app.insert_resource(loaded)
            .insert_resource(ViewOptionsState::default())
            .add_systems(Update, apply_minimum_apparent_body_size);
        app.world_mut().spawn((
            Camera3d::default(),
            camera,
            projection.clone(),
            Transform::from_translation(camera_translation),
        ));
        let sun_entity = app
            .world_mut()
            .spawn((
                BodyVisual { index: sun },
                Transform::from_scale(Vec3::splat(sun_radius)),
            ))
            .id();
        let tiny_entity = app
            .world_mut()
            .spawn((
                BodyVisual { index: tiny },
                Transform::from_scale(Vec3::splat(tiny_radius)),
            ))
            .id();
        app.update();

        let sun_scale = app
            .world()
            .entity(sun_entity)
            .get::<Transform>()
            .unwrap()
            .scale;
        let tiny_scale = app
            .world()
            .entity(tiny_entity)
            .get::<Transform>()
            .unwrap()
            .scale;
        assert_eq!(sun_scale, Vec3::splat(sun_radius));
        assert!(tiny_scale.x > tiny_radius);
        let diameter = projected_body_diameter_logical_px(
            f64::from(tiny_scale.x),
            f64::from(camera_translation.length()),
            &projection,
            600.0,
        )
        .unwrap();
        assert!(diameter >= OTHER_MIN_BODY_DIAMETER_LOGICAL_PX - 1.0e-6);
    }

    #[test]
    fn render_clamp_changes_neither_pick_radius_nor_replay_hash() {
        let catalog = load_catalog_text(REAL_CATALOG).unwrap();
        let simulation = HeadlessSimulation::new(&catalog).unwrap();
        let hash_before = simulation.state_hash();
        let projection = perspective();
        let physical_radius = 0.25;
        let camera_distance = 10_000.0;

        let rendered_radius = clamped_body_radius_units(
            physical_radius,
            50.0,
            OTHER_MIN_BODY_DIAMETER_LOGICAL_PX,
            camera_distance,
            &projection,
            600.0,
        );
        assert!(rendered_radius > physical_radius);
        let Projection::Perspective(perspective) = &projection else {
            unreachable!();
        };
        let expected_pick_radius =
            2.0 * camera_distance * (f64::from(perspective.fov) * 0.5).tan() / 600.0 * 10.0;
        assert_eq!(
            inflated_pick_radius(physical_radius, camera_distance, &projection, 600.0),
            expected_pick_radius,
        );
        assert!(rendered_radius > expected_pick_radius);
        assert_eq!(simulation.state_hash(), hash_before);
    }
}
