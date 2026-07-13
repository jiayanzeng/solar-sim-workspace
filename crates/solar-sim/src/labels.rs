//! WP9 projected labels, contextual moon visibility, and picking — Rev C §§8.4 and 10.3.
//!
//! Body positions remain simulation/render truth owned by WP4. This module
//! projects that truth into plain Bevy UI nodes, applies a deterministic
//! greedy declutter pass, and converts both label and inflated-sphere clicks
//! into the existing `SimCommand` mutation boundary.

use crate::control::{CameraController, SimCommand, SimCommandQueue};
use crate::ui_kit::{UiTheme, INTER_FONT_ASSET, TOP_BAR_HEIGHT_PX};
use crate::{rebase_position, BodyStates, LoadedCatalog, KM_PER_RENDER_UNIT, TIME_BAR_HEIGHT_PX};
use bevy::{
    camera::CameraUpdateSystems, input_focus::tab_navigation::TabIndex, prelude::*,
    text::LetterSpacing, transform::TransformSystems, ui::UiSystems, ui_widgets::Activate,
};
use sim_core::catalog::Category;
use std::collections::HashMap;

const LABEL_Z_INDEX: i32 = 70;
const PICK_SURFACE_Z_INDEX: i32 = -100;
const PRIMARY_LABEL_HEIGHT_PX: f32 = 24.0;
const SECONDARY_LABEL_HEIGHT_PX: f32 = 20.0;
const RETICLE_SIZE_PX: f32 = 12.0;
const RETICLE_GAP_PX: f32 = 6.0;
const LABEL_ANCHOR_GAP_PX: f32 = 8.0;
const VIEWPORT_MARGIN_PX: f32 = 4.0;
const MIN_PICK_RADIUS_PX: f64 = 10.0;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyLabel {
    pub index: usize,
}

#[derive(Component, Debug, Clone, Copy)]
struct LabelVisual {
    width_px: f32,
    height_px: f32,
    offset_px: Vec2,
}

#[derive(Component, Debug, Clone, Copy)]
struct BodyReticle;

#[derive(Component, Debug, Clone, Copy)]
struct ViewportPickSurface;

#[derive(Resource, Debug, Default)]
struct MoonSystemExtents(Vec<f64>);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub min: Vec2,
    pub max: Vec2,
}

impl ScreenRect {
    pub fn from_min_size(min: Vec2, size: Vec2) -> Self {
        Self {
            min,
            max: min + size,
        }
    }

    fn overlaps(self, other: Self) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
    }

    fn is_inside(self, bounds: Self) -> bool {
        self.min.x >= bounds.min.x
            && self.max.x <= bounds.max.x
            && self.min.y >= bounds.min.y
            && self.max.y <= bounds.max.y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LabelPriority {
    Selection,
    Planet,
    DwarfPlanet,
    Comet,
    FocusedSystemMoon,
    Asteroid,
    OtherMoon,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeclutterCandidate {
    pub body_index: usize,
    pub priority: LabelPriority,
    pub rect: ScreenRect,
}

/// Greedy label acceptance in architecture priority order. Catalog index is
/// the stable tie-break inside a tier, so entity allocation and query order
/// cannot make labels flicker while the projected inputs are unchanged.
pub fn declutter_labels(candidates: &[DeclutterCandidate]) -> Vec<usize> {
    let mut ordered = candidates.to_vec();
    ordered.sort_by_key(|candidate| (candidate.priority, candidate.body_index));

    let mut accepted_rects = Vec::with_capacity(ordered.len());
    let mut accepted_indices = Vec::with_capacity(ordered.len());
    for candidate in ordered {
        if accepted_rects
            .iter()
            .all(|accepted| !candidate.rect.overlaps(*accepted))
        {
            accepted_rects.push(candidate.rect);
            accepted_indices.push(candidate.body_index);
        }
    }
    accepted_indices
}

fn layout_projected_labels(
    candidates: &[DeclutterCandidate],
    viewport_bounds: ScreenRect,
) -> HashMap<usize, ScreenRect> {
    let mut ordered = candidates.to_vec();
    ordered.sort_by_key(|candidate| (candidate.priority, candidate.body_index));

    let mut accepted_rects = Vec::with_capacity(ordered.len());
    let mut placements = HashMap::with_capacity(ordered.len());
    for candidate in ordered {
        let may_nudge = matches!(
            candidate.priority,
            LabelPriority::Selection | LabelPriority::Planet | LabelPriority::FocusedSystemMoon
        );
        let mut alternatives = vec![candidate.rect];
        if may_nudge {
            let width = candidate.rect.max.x - candidate.rect.min.x;
            let height = candidate.rect.max.y - candidate.rect.min.y;
            let step = Vec2::new(width + 12.0, height + 8.0);
            for ring in 1_i32..=6 {
                for y in -ring..=ring {
                    for x in -ring..=ring {
                        if x.abs().max(y.abs()) != ring {
                            continue;
                        }
                        let offset = Vec2::new(x as f32 * step.x, y as f32 * step.y);
                        alternatives.push(ScreenRect {
                            min: candidate.rect.min + offset,
                            max: candidate.rect.max + offset,
                        });
                    }
                }
            }
        }

        if let Some(rect) = alternatives.into_iter().find(|rect| {
            rect.is_inside(viewport_bounds)
                && accepted_rects
                    .iter()
                    .all(|accepted| !rect.overlaps(*accepted))
        }) {
            accepted_rects.push(rect);
            placements.insert(candidate.body_index, rect);
        }
    }
    placements
}

/// Analytic forward-ray versus sphere intersection. The returned distance is
/// measured along the normalized ray; invalid inputs and intersections wholly
/// behind the ray origin are rejected.
pub fn ray_sphere_hit_distance(
    ray_origin: [f64; 3],
    ray_direction: [f64; 3],
    sphere_center: [f64; 3],
    sphere_radius: f64,
) -> Option<f64> {
    if sphere_radius <= 0.0
        || !sphere_radius.is_finite()
        || ray_origin.into_iter().any(|value| !value.is_finite())
        || ray_direction.into_iter().any(|value| !value.is_finite())
        || sphere_center.into_iter().any(|value| !value.is_finite())
    {
        return None;
    }
    let direction_norm = dot3(ray_direction, ray_direction).sqrt();
    if direction_norm <= f64::EPSILON {
        return None;
    }
    let direction = scale3(ray_direction, 1.0 / direction_norm);
    let origin_to_center = sub3(ray_origin, sphere_center);
    let projected = dot3(origin_to_center, direction);
    let discriminant = projected * projected
        - (dot3(origin_to_center, origin_to_center) - sphere_radius * sphere_radius);
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let near = -projected - root;
    if near >= 0.0 {
        Some(near)
    } else {
        let far = -projected + root;
        (far >= 0.0).then_some(far)
    }
}

pub struct LabelsPlugin;

impl Plugin for LabelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_labels).add_systems(
            PostUpdate,
            project_and_declutter_labels
                .after(TransformSystems::Propagate)
                .after(CameraUpdateSystems)
                .before(UiSystems::Prepare),
        );
    }
}

pub struct SelectionPlugin;

impl Plugin for SelectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_viewport_pick_surface);
    }
}

fn spawn_labels(
    mut commands: Commands,
    loaded: Option<Res<LoadedCatalog>>,
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
) {
    let Some(loaded) = loaded else {
        return;
    };
    commands.insert_resource(MoonSystemExtents(moon_system_extents(&loaded)));

    let font = asset_server.load(INTER_FONT_ASSET);
    for (index, body) in loaded.catalog.bodies.iter().enumerate() {
        let primary = matches!(body.category, Category::Star | Category::Planet);
        let text = if primary {
            body.name.to_uppercase()
        } else {
            body.name.clone()
        };
        let visual = label_visual(&text, primary, *theme);
        let root = commands
            .spawn((
                Name::new(format!("{} label", body.name)),
                BodyLabel { index },
                visual,
                bevy::ui_widgets::Button,
                AccessibleLabel::new(format!("Travel to {}", body.name)),
                TabIndex(100 + index as i32),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(-10_000),
                    top: px(-10_000),
                    width: px(visual.width_px),
                    height: px(visual.height_px),
                    display: Display::None,
                    align_items: AlignItems::Center,
                    column_gap: px(RETICLE_GAP_PX),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                GlobalZIndex(LABEL_Z_INDEX),
            ))
            .observe(activate_body_label)
            .id();

        if !primary {
            let (red, green, blue) = body.color_srgb;
            commands.spawn((
                BodyReticle,
                Node {
                    width: px(RETICLE_SIZE_PX),
                    height: px(RETICLE_SIZE_PX),
                    border: UiRect::all(px(theme.spacing.hairline_px)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BorderColor::all(Color::srgb_u8(red, green, blue)),
                BackgroundColor(Color::NONE),
                Pickable::IGNORE,
                ChildOf(root),
            ));
        }

        commands.spawn((
            Text::new(text),
            TextFont {
                font: font.clone().into(),
                font_size: if primary {
                    theme.type_scale.label_px.into()
                } else {
                    theme.type_scale.caption_px.into()
                },
                ..default()
            },
            TextColor(if primary {
                theme.colors.text_primary.color()
            } else {
                theme.colors.text_muted.color()
            }),
            LetterSpacing::Px(if primary {
                theme.type_scale.uppercase_tracking_px
            } else {
                0.0
            }),
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
}

fn label_visual(text: &str, primary: bool, theme: UiTheme) -> LabelVisual {
    let glyph_count = text.chars().count() as f32;
    if primary {
        let text_width = glyph_count
            * (theme.type_scale.label_px * 0.68 + theme.type_scale.uppercase_tracking_px);
        LabelVisual {
            width_px: text_width.max(28.0),
            height_px: PRIMARY_LABEL_HEIGHT_PX,
            offset_px: Vec2::new(LABEL_ANCHOR_GAP_PX, -PRIMARY_LABEL_HEIGHT_PX * 0.5),
        }
    } else {
        let text_width = glyph_count * theme.type_scale.caption_px * 0.62;
        LabelVisual {
            width_px: RETICLE_SIZE_PX + RETICLE_GAP_PX + text_width.max(18.0),
            height_px: SECONDARY_LABEL_HEIGHT_PX,
            offset_px: Vec2::new(-RETICLE_SIZE_PX * 0.5, -SECONDARY_LABEL_HEIGHT_PX * 0.5),
        }
    }
}

fn project_and_declutter_labels(
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    loaded: Option<Res<LoadedCatalog>>,
    states: Option<Res<BodyStates>>,
    controller: Option<Res<CameraController>>,
    extents: Option<Res<MoonSystemExtents>>,
    mut labels: Query<(&BodyLabel, &LabelVisual, &mut Node)>,
) {
    let (Some(loaded), Some(states), Some(controller), Some(extents)) =
        (loaded, states, controller, extents)
    else {
        hide_all_labels(&mut labels);
        return;
    };
    let Ok((camera, camera_transform)) = camera_query.single() else {
        hide_all_labels(&mut labels);
        return;
    };
    let Some(viewport_size) = camera.logical_viewport_size() else {
        hide_all_labels(&mut labels);
        return;
    };

    let viewport_bounds = ScreenRect {
        min: Vec2::new(VIEWPORT_MARGIN_PX, TOP_BAR_HEIGHT_PX + VIEWPORT_MARGIN_PX),
        max: Vec2::new(
            viewport_size.x - VIEWPORT_MARGIN_PX,
            viewport_size.y - TIME_BAR_HEIGHT_PX - VIEWPORT_MARGIN_PX,
        ),
    };
    let selected = controller.selected_body_index();
    let focus_system = focus_system_index(&loaded, controller.focus_body_index());
    let camera_position_km = controller.camera_position_km();
    let focus_position_km = controller.focus_position_km();
    let mut candidates = Vec::with_capacity(loaded.catalog.bodies.len());

    for (label, visual, _) in &mut labels {
        let Some(body) = loaded.catalog.bodies.get(label.index) else {
            continue;
        };
        if !body_is_contextually_visible(
            label.index,
            selected,
            focus_system,
            &loaded,
            &states,
            &extents,
            camera_position_km,
        ) {
            continue;
        }
        let Some(state) = states.0.get(label.index) else {
            continue;
        };
        let world_position = rebase_position(state.position_km, focus_position_km);
        let Ok(projected) = camera.world_to_viewport(camera_transform, world_position) else {
            continue;
        };
        if projected.x < viewport_bounds.min.x
            || projected.x > viewport_bounds.max.x
            || projected.y < viewport_bounds.min.y
            || projected.y > viewport_bounds.max.y
        {
            continue;
        }
        let rect = ScreenRect::from_min_size(
            projected + visual.offset_px,
            Vec2::new(visual.width_px, visual.height_px),
        );
        let priority = label_priority(body.category, label.index, selected, focus_system, &loaded);
        candidates.push(DeclutterCandidate {
            body_index: label.index,
            priority,
            rect,
        });
    }

    // Primary labels are nudged on the same deterministic greedy pass before
    // rejection. At the full-system scale the inner planets project into a
    // compact cluster; stable alternative slots preserve all eight without
    // allowing any lower-priority label to overlap them.
    let placements = layout_projected_labels(&candidates, viewport_bounds);
    for (label, _, mut node) in &mut labels {
        if let Some(rect) = placements.get(&label.index) {
            node.left = px(rect.min.x);
            node.top = px(rect.min.y);
            node.display = Display::Flex;
        } else {
            node.display = Display::None;
        }
    }
}

fn hide_all_labels(labels: &mut Query<(&BodyLabel, &LabelVisual, &mut Node)>) {
    for (_, _, mut node) in labels {
        node.display = Display::None;
    }
}

fn label_priority(
    category: Category,
    body_index: usize,
    selected: usize,
    focus_system: usize,
    loaded: &LoadedCatalog,
) -> LabelPriority {
    if body_index == selected {
        return LabelPriority::Selection;
    }
    match category {
        Category::Star | Category::Planet => LabelPriority::Planet,
        Category::DwarfPlanet => LabelPriority::DwarfPlanet,
        Category::Comet => LabelPriority::Comet,
        Category::Moon => {
            let is_focused_system = loaded.catalog.bodies[body_index]
                .parent
                .as_deref()
                .and_then(|parent| loaded.index_of(parent))
                == Some(focus_system);
            if is_focused_system {
                LabelPriority::FocusedSystemMoon
            } else {
                LabelPriority::OtherMoon
            }
        }
        Category::Asteroid => LabelPriority::Asteroid,
    }
}

fn body_is_contextually_visible(
    body_index: usize,
    selected: usize,
    focus_system: usize,
    loaded: &LoadedCatalog,
    states: &BodyStates,
    extents: &MoonSystemExtents,
    camera_position_km: [f64; 3],
) -> bool {
    let body = &loaded.catalog.bodies[body_index];
    if body.category != Category::Moon || body_index == selected {
        return true;
    }
    let Some(parent_index) = body.parent.as_deref().and_then(|id| loaded.index_of(id)) else {
        return false;
    };
    let Some(parent_state) = states.0.get(parent_index) else {
        return false;
    };
    moon_label_is_contextually_visible(
        false,
        parent_index,
        focus_system,
        distance3(camera_position_km, parent_state.position_km),
        extents.0.get(parent_index).copied().unwrap_or(0.0),
    )
}

pub fn moon_label_is_contextually_visible(
    is_selected: bool,
    parent_index: usize,
    focus_system_index: usize,
    camera_parent_distance_km: f64,
    system_extent_km: f64,
) -> bool {
    is_selected
        || parent_index == focus_system_index
        || (camera_parent_distance_km.is_finite()
            && system_extent_km > 0.0
            && camera_parent_distance_km <= system_extent_km)
}

fn focus_system_index(loaded: &LoadedCatalog, focus_index: usize) -> usize {
    let Some(focus) = loaded.catalog.bodies.get(focus_index) else {
        return focus_index;
    };
    if focus.category == Category::Moon {
        focus
            .parent
            .as_deref()
            .and_then(|parent| loaded.index_of(parent))
            .unwrap_or(focus_index)
    } else {
        focus_index
    }
}

fn moon_system_extents(loaded: &LoadedCatalog) -> Vec<f64> {
    let mut extents = vec![0.0_f64; loaded.catalog.bodies.len()];
    for body in &loaded.catalog.bodies {
        if body.category != Category::Moon {
            continue;
        }
        let Some(parent_index) = body.parent.as_deref().and_then(|id| loaded.index_of(id)) else {
            continue;
        };
        let Some(orbit) = body.orbit.as_ref() else {
            continue;
        };
        let apoapsis_km = orbit.elements.a_km.abs() * (1.0 + orbit.elements.e);
        extents[parent_index] = extents[parent_index].max(apoapsis_km);
    }
    extents
}

fn activate_body_label(
    activate: On<Activate>,
    labels: Query<&BodyLabel>,
    loaded: Res<LoadedCatalog>,
    mut commands: ResMut<SimCommandQueue>,
) {
    let Ok(label) = labels.get(activate.entity) else {
        return;
    };
    enqueue_travel(label.index, &loaded, &mut commands);
}

fn spawn_viewport_pick_surface(mut commands: Commands) {
    commands
        .spawn((
            Name::new("3D viewport pick surface"),
            ViewportPickSurface,
            AccessibleLabel::new("Solar system viewport"),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Pickable::default(),
            GlobalZIndex(PICK_SURFACE_Z_INDEX),
        ))
        .observe(pick_inflated_body_sphere);
}

fn pick_inflated_body_sphere(
    click: On<Pointer<Click>>,
    cameras: Query<(&Camera, &GlobalTransform, &Projection), With<Camera3d>>,
    bodies: Query<(&crate::BodyVisual, &GlobalTransform)>,
    loaded: Res<LoadedCatalog>,
    mut commands: ResMut<SimCommandQueue>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok((camera, camera_transform, projection)) = cameras.single() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(camera_transform, click.pointer_location.position)
    else {
        return;
    };
    let viewport_height = camera.logical_viewport_size().map_or(720.0, |size| size.y);
    let ray_origin = vec3_to_array(ray.origin);
    let ray_direction = vec3_to_array(*ray.direction);
    let mut nearest: Option<(f64, usize)> = None;

    for (visual, transform) in &bodies {
        let Some(body) = loaded.catalog.bodies.get(visual.index) else {
            continue;
        };
        let center = transform.translation();
        let camera_distance = f64::from(ray.origin.distance(center));
        let true_radius = body.radius_km / KM_PER_RENDER_UNIT;
        let pick_radius = inflated_pick_radius(
            true_radius,
            camera_distance,
            projection,
            f64::from(viewport_height),
        );
        let Some(distance) = ray_sphere_hit_distance(
            ray_origin,
            ray_direction,
            vec3_to_array(center),
            pick_radius,
        ) else {
            continue;
        };
        let replace = nearest.is_none_or(|(best_distance, best_index)| {
            distance < best_distance
                || (distance.to_bits() == best_distance.to_bits() && visual.index < best_index)
        });
        if replace {
            nearest = Some((distance, visual.index));
        }
    }

    if let Some((_distance, body_index)) = nearest {
        enqueue_travel(body_index, &loaded, &mut commands);
    }
}

fn inflated_pick_radius(
    true_radius: f64,
    camera_distance: f64,
    projection: &Projection,
    viewport_height: f64,
) -> f64 {
    let minimum_radius = match projection {
        Projection::Perspective(perspective) if viewport_height > 0.0 => {
            let world_height = 2.0 * camera_distance * (f64::from(perspective.fov) * 0.5).tan();
            world_height / viewport_height * MIN_PICK_RADIUS_PX
        }
        _ => true_radius,
    };
    true_radius.max(minimum_radius)
}

fn enqueue_travel(body_index: usize, loaded: &LoadedCatalog, commands: &mut SimCommandQueue) {
    if let Some(body) = loaded.catalog.bodies.get(body_index) {
        commands.push(SimCommand::TravelToBody(body.id.clone()));
    }
}

fn vec3_to_array(value: Vec3) -> [f64; 3] {
    [f64::from(value.x), f64::from(value.y), f64::from(value.z)]
}

fn dot3(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn sub3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale3(value: [f64; 3], scale: f64) -> [f64; 3] {
    [value[0] * scale, value[1] * scale, value[2] * scale]
}

fn distance3(left: [f64; 3], right: [f64; 3]) -> f64 {
    dot3(sub3(left, right), sub3(left, right)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_catalog_text;
    use bevy::{
        app::TaskPoolPlugin,
        asset::{AssetApp, AssetPlugin},
        text::Font,
    };

    const REAL_CATALOG: &str = include_str!("../../../assets/catalog.ron");

    fn catalog() -> LoadedCatalog {
        LoadedCatalog::new(load_catalog_text(REAL_CATALOG).unwrap())
    }

    fn rect(x: f32, y: f32) -> ScreenRect {
        ScreenRect::from_min_size(Vec2::new(x, y), Vec2::splat(10.0))
    }

    #[test]
    fn greedy_declutter_respects_priority_and_is_deterministic() {
        let candidates = vec![
            DeclutterCandidate {
                body_index: 8,
                priority: LabelPriority::Asteroid,
                rect: rect(0.0, 0.0),
            },
            DeclutterCandidate {
                body_index: 4,
                priority: LabelPriority::Comet,
                rect: rect(40.0, 0.0),
            },
            DeclutterCandidate {
                body_index: 1,
                priority: LabelPriority::Planet,
                rect: rect(0.0, 0.0),
            },
            DeclutterCandidate {
                body_index: 9,
                priority: LabelPriority::Selection,
                rect: rect(0.0, 0.0),
            },
            DeclutterCandidate {
                body_index: 3,
                priority: LabelPriority::Comet,
                rect: rect(40.0, 0.0),
            },
            DeclutterCandidate {
                body_index: 7,
                priority: LabelPriority::OtherMoon,
                rect: rect(80.0, 0.0),
            },
        ];

        let expected = vec![9, 3, 7];
        assert_eq!(declutter_labels(&candidates), expected);
        let reversed: Vec<_> = candidates.into_iter().rev().collect();
        assert_eq!(declutter_labels(&reversed), expected);
    }

    #[test]
    fn primary_layout_keeps_a_clustered_selection_and_all_eight_planets() {
        let bounds = ScreenRect {
            min: Vec2::ZERO,
            max: Vec2::new(800.0, 600.0),
        };
        let candidates: Vec<_> = (0..9)
            .map(|body_index| DeclutterCandidate {
                body_index,
                priority: if body_index == 0 {
                    LabelPriority::Selection
                } else {
                    LabelPriority::Planet
                },
                rect: ScreenRect::from_min_size(Vec2::new(380.0, 288.0), Vec2::new(72.0, 24.0)),
            })
            .collect();

        let placements = layout_projected_labels(&candidates, bounds);
        assert_eq!(placements.len(), 9);
        let mut rects: Vec<_> = placements.values().copied().collect();
        while let Some(rect) = rects.pop() {
            assert!(rects.iter().all(|other| !rect.overlaps(*other)));
        }
        let reversed: Vec<_> = candidates.into_iter().rev().collect();
        assert_eq!(layout_projected_labels(&reversed, bounds), placements);
    }

    #[test]
    fn focused_system_moons_receive_stable_nonoverlapping_slots() {
        let bounds = ScreenRect {
            min: Vec2::ZERO,
            max: Vec2::new(800.0, 600.0),
        };
        let mut candidates = vec![DeclutterCandidate {
            body_index: 0,
            priority: LabelPriority::Selection,
            rect: ScreenRect::from_min_size(Vec2::new(380.0, 288.0), Vec2::new(72.0, 24.0)),
        }];
        candidates.extend((1..=6).map(|body_index| DeclutterCandidate {
            body_index,
            priority: LabelPriority::FocusedSystemMoon,
            rect: ScreenRect::from_min_size(Vec2::new(380.0, 288.0), Vec2::new(68.0, 20.0)),
        }));

        let placements = layout_projected_labels(&candidates, bounds);
        assert_eq!(placements.len(), 7);
        let rects: Vec<_> = placements.values().copied().collect();
        for (index, rect) in rects.iter().enumerate() {
            assert!(rects[index + 1..]
                .iter()
                .all(|other| !rect.overlaps(*other)));
        }
    }

    #[test]
    fn ray_sphere_math_handles_hits_misses_tangency_and_inflation() {
        let origin = [0.0, 0.0, 0.0];
        let forward = [0.0, 0.0, -2.0];
        assert_eq!(
            ray_sphere_hit_distance(origin, forward, [0.0, 0.0, -10.0], 1.0),
            Some(9.0)
        );
        assert_eq!(
            ray_sphere_hit_distance(origin, forward, [2.0, 0.0, -10.0], 1.0),
            None
        );
        assert!(ray_sphere_hit_distance(origin, forward, [2.0, 0.0, -10.0], 2.0).is_some());
        assert_eq!(
            ray_sphere_hit_distance(origin, forward, [1.0, 0.0, -10.0], 1.0),
            Some(10.0)
        );
        assert_eq!(
            ray_sphere_hit_distance(origin, forward, [0.0, 0.0, 10.0], 1.0),
            None
        );
        assert_eq!(
            ray_sphere_hit_distance(origin, [0.0; 3], [0.0, 0.0, -10.0], 1.0),
            None
        );
    }

    #[test]
    fn focused_system_and_parent_distance_gate_moon_labels() {
        let jupiter = 5;
        let saturn = 6;
        let saturn_system_extent_km = 13_000_000.0;

        assert!(moon_label_is_contextually_visible(
            false,
            jupiter,
            jupiter,
            1.0e9,
            12_000_000.0,
        ));
        assert!(!moon_label_is_contextually_visible(
            false,
            saturn,
            jupiter,
            1.0e9,
            saturn_system_extent_km,
        ));
        assert!(moon_label_is_contextually_visible(
            false,
            saturn,
            jupiter,
            12_000_000.0,
            saturn_system_extent_km,
        ));
        assert!(moon_label_is_contextually_visible(
            true,
            saturn,
            jupiter,
            1.0e9,
            saturn_system_extent_km,
        ));
    }

    #[test]
    fn startup_spawns_every_accessible_label_and_reticle_style() {
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()))
            .init_asset::<Font>()
            .insert_resource(catalog())
            .insert_resource(UiTheme::default())
            .add_systems(Startup, spawn_labels);
        app.update();

        let world = app.world_mut();
        let labels = world
            .query::<(&BodyLabel, &AccessibleLabel, &bevy::ui_widgets::Button)>()
            .iter(world)
            .count();
        let reticles = world.query::<&BodyReticle>().iter(world).count();
        assert_eq!(labels, 66);
        assert_eq!(reticles, 57, "Sun and eight planets have text-only labels");
        assert_eq!(world.resource::<MoonSystemExtents>().0.len(), 66);
    }

    #[test]
    fn activating_a_label_queues_the_shared_travel_command() {
        let loaded = catalog();
        let jupiter = loaded.index_of("jupiter").unwrap();
        let mut app = App::new();
        app.insert_resource(loaded)
            .insert_resource(SimCommandQueue::default());
        let label = app
            .world_mut()
            .spawn(BodyLabel { index: jupiter })
            .observe(activate_body_label)
            .id();

        app.world_mut().trigger(Activate { entity: label });

        let queued: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(queued, vec![SimCommand::TravelToBody("jupiter".into())]);
    }
}
