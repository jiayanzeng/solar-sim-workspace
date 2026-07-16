//! Headless Bevy UI layout fixture for responsive acceptance tests.

use super::INTER_FONT_ASSET;
use bevy::transform::systems::{
    mark_dirty_trees, propagate_parent_transforms, sync_simple_transforms,
};
use bevy::{
    a11y::AccessibilityPlugin,
    app::{HierarchyPropagatePlugin, PropagateSet, TaskPoolPlugin},
    asset::AssetPlugin,
    camera::{Camera, Camera2d, ComputedCameraValues, RenderTargetInfo, Viewport},
    ecs::schedule::{ApplyDeferred, Schedule, Schedules},
    image::TextureAtlasPlugin,
    input::InputPlugin,
    input_focus::{InputDispatchPlugin, InputFocusPlugin},
    prelude::*,
    scene::ScenePlugin,
    text::{
        detect_text_needs_rerender, load_font_assets_into_font_collection, Font, LineBreak,
        TextLayout, TextLayoutInfo, TextPlugin,
    },
    time::TimePlugin,
    transform::TransformPlugin,
    ui::{
        ui_layout_system,
        update::{propagate_ui_target_cameras, update_clipping_system},
        widget::{
            measure_text_system, scroll_editable_text, text_system,
            update_editable_text_content_size, update_editable_text_layout,
            update_editable_text_styles, update_image_content_size_system,
        },
        ComputedUiRenderTargetInfo, ComputedUiTargetCamera, UiPlugin, UiScale,
    },
    window::WindowPlugin,
};

#[derive(Resource, Default)]
struct LayoutFixtureStarted(bool);

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum LayoutFixtureSet {
    Prepare,
    Content,
    Layout,
    Transform,
    PostLayout,
}

pub(crate) fn app(width: u32, height: u32, scale: f32) -> App {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        TimePlugin,
        InputPlugin,
        InputFocusPlugin,
        InputDispatchPlugin,
        WindowPlugin {
            primary_window: None,
            ..default()
        },
        AccessibilityPlugin,
        AssetPlugin::default(),
        TextureAtlasPlugin,
        ScenePlugin,
        TransformPlugin,
        bevy::camera::CameraPlugin,
        TextPlugin,
        UiPlugin,
    ))
    .init_resource::<LayoutFixtureStarted>()
    .init_resource::<Assets<Image>>();
    // Asset polling is intentionally absent from this deterministic fixture, so
    // register the shipped UI font under the same path-backed handle scenes use.
    let inter_font = app
        .world()
        .resource::<AssetServer>()
        .load::<Font>(INTER_FONT_ASSET);
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(
            inter_font.id(),
            Font::from_bytes(include_bytes!("../../../../assets/fonts/InterVariable.ttf").to_vec()),
        )
        .unwrap();
    // Keep Bevy's real text/content/layout/clipping path while excluding
    // render-only visibility systems whose mesh resources do not exist here.
    // Do not reintroduce a layout-only schedule: zero-sized text made the
    // responsive acceptance matrix pass without exercising the shipped UI.
    app.world_mut()
        .resource_mut::<Schedules>()
        .remove(PostUpdate);
    app.add_schedule(Schedule::new(PostUpdate));
    HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(PostUpdate).build(&mut app);
    HierarchyPropagatePlugin::<ComputedUiRenderTargetInfo>::new(PostUpdate).build(&mut app);
    app.configure_sets(
        PostUpdate,
        (
            LayoutFixtureSet::Prepare,
            PropagateSet::<ComputedUiTargetCamera>::default(),
            PropagateSet::<ComputedUiRenderTargetInfo>::default(),
            LayoutFixtureSet::Content,
            LayoutFixtureSet::Layout,
            LayoutFixtureSet::Transform,
            LayoutFixtureSet::PostLayout,
        )
            .chain(),
    )
    .add_systems(
        PostUpdate,
        (propagate_ui_target_cameras, ApplyDeferred)
            .chain()
            .in_set(LayoutFixtureSet::Prepare),
    )
    .add_systems(
        PostUpdate,
        (
            load_font_assets_into_font_collection,
            detect_text_needs_rerender,
            update_editable_text_content_size,
            update_editable_text_styles,
            update_image_content_size_system,
            measure_text_system,
        )
            .chain()
            .in_set(LayoutFixtureSet::Content),
    )
    .add_systems(
        PostUpdate,
        ui_layout_system.in_set(LayoutFixtureSet::Layout),
    )
    .add_systems(
        PostUpdate,
        (
            mark_dirty_trees,
            sync_simple_transforms,
            propagate_parent_transforms,
        )
            .chain()
            .in_set(LayoutFixtureSet::Transform),
    )
    .add_systems(
        PostUpdate,
        (
            text_system,
            update_editable_text_layout,
            scroll_editable_text,
            update_clipping_system,
        )
            .chain()
            .in_set(LayoutFixtureSet::PostLayout),
    );
    app.world_mut().resource_mut::<UiScale>().0 = scale;
    app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: ComputedCameraValues {
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
        },
    ));
    app
}

pub(crate) fn settle(app: &mut App) {
    let started = app.world().resource::<LayoutFixtureStarted>().0;
    if !started {
        app.finish();
        app.cleanup();
        if app.world().resource::<Schedules>().contains(Startup) {
            app.world_mut().run_schedule(Startup);
        }
        app.world_mut().resource_mut::<LayoutFixtureStarted>().0 = true;
    }
    for _ in 0..8 {
        if app.world().resource::<Schedules>().contains(Update) {
            app.world_mut().run_schedule(Update);
        }
        app.world_mut().run_schedule(PostUpdate);
    }
}

pub(crate) fn required_viewports() -> impl Iterator<Item = (u32, u32, f32)> {
    [800_u32, 960].into_iter().flat_map(|width| {
        [0.75_f32, 1.0, 1.5, 2.0]
            .into_iter()
            .map(move |scale| (width, 600, scale))
    })
}

#[test]
fn fixture_measures_intrinsic_text_and_computes_clipping() {
    let mut app = app(320, 180, 1.0);
    let inter_font = app
        .world()
        .resource::<AssetServer>()
        .load::<Font>(INTER_FONT_ASSET);
    let clip = app
        .world_mut()
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: px(90),
            height: px(28),
            overflow: Overflow::clip(),
            ..default()
        })
        .id();
    let text = app
        .world_mut()
        .spawn((
            Text::new("THIS INTRINSIC TEXT IS WIDER THAN ITS CLIP"),
            TextLayout {
                linebreak: LineBreak::NoWrap,
                ..default()
            },
            TextFont {
                font: inter_font.into(),
                font_size: 18.0.into(),
                ..default()
            },
            ChildOf(clip),
        ))
        .id();

    settle(&mut app);

    let text_node = app.world().get::<ComputedNode>(text).unwrap();
    assert!(text_node.size().x > 90.0);
    let text_layout = app.world().get::<TextLayoutInfo>(text).unwrap();
    assert!(text_layout.size.x > 90.0 && text_layout.size.y > 0.0);
    assert!(
        app.world()
            .get::<ComputedNode>(clip)
            .unwrap()
            .content_size()
            .x
            > 90.0
    );
    let clip_rect = app.world().get::<CalculatedClip>(text).unwrap().clip;
    assert!(clip_rect.max.x - clip_rect.min.x <= 90.0 + 1.0);
}
