//! WP0 — application shell (ARCHITECTURE §8): window, orbit-camera stub,
//! dev-only diagnostics overlay, and a `--smoke` mode for CI launch checks.

#[cfg(debug_assertions)]
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

/// Orbit rig stub (full input-intent layer + SimCommand routing is WP5 —
/// do NOT grow direct-input mutation habits here; this stub is throwaway
/// in exactly that respect).
#[derive(Resource)]
struct OrbitRig {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Default for OrbitRig {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.35,
            distance: 12.0,
        }
    }
}

/// `--smoke N`: exit(0) after N rendered frames — used by CI to prove the
/// app launches and renders, not just links.
#[derive(Resource)]
struct SmokeFrames(Option<u32>);

fn main() {
    let smoke = std::env::args()
        .skip_while(|a| a != "--smoke")
        .nth(1)
        .and_then(|n| n.parse::<u32>().ok())
        .or(std::env::args().any(|a| a == "--smoke").then_some(60));

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "solar-sim (WP0 shell)".into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(OrbitRig::default())
    .insert_resource(SmokeFrames(smoke))
    .add_systems(Startup, setup)
    .add_systems(Update, (orbit_rig_stub, smoke_exit));

    #[cfg(debug_assertions)]
    {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .add_systems(Startup, spawn_diag_overlay)
            .add_systems(Update, update_diag_overlay);
    }

    app.run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Placeholder sun so there is something to look at until WP4.
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.3),
            emissive: LinearRgba::rgb(4.0, 3.2, 0.8),
            ..default()
        })),
    ));
    commands.spawn((Camera3d::default(), Transform::default()));
}

/// Right-drag orbits, scroll dollies. Raw input handling is acceptable
/// ONLY inside WP0's stub; WP5 replaces this with the SimCommand path.
fn orbit_rig_stub(
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut rig: ResMut<OrbitRig>,
    mut cam: Query<&mut Transform, With<Camera3d>>,
) {
    if buttons.pressed(MouseButton::Right) {
        for m in motion.read() {
            rig.yaw -= m.delta.x * 0.005;
            rig.pitch = (rig.pitch + m.delta.y * 0.005).clamp(-1.5, 1.5);
        }
    } else {
        motion.clear();
    }
    for w in wheel.read() {
        rig.distance = (rig.distance * (1.0 - w.y * 0.1)).clamp(2.0, 200.0);
    }
    if let Ok(mut t) = cam.single_mut() {
        let (sy, cy) = rig.yaw.sin_cos();
        let (sp, cp) = rig.pitch.sin_cos();
        t.translation = Vec3::new(cy * cp, sp, sy * cp) * rig.distance;
        t.look_at(Vec3::ZERO, Vec3::Y);
    }
}

fn smoke_exit(mut frames: Local<u32>, smoke: Res<SmokeFrames>, mut exit: MessageWriter<AppExit>) {
    if let Some(n) = smoke.0 {
        *frames += 1;
        if *frames >= n {
            exit.write(AppExit::Success);
        }
    }
}

#[cfg(debug_assertions)]
#[derive(Component)]
struct DiagText;

#[cfg(debug_assertions)]
fn spawn_diag_overlay(mut commands: Commands) {
    commands.spawn((Text::new("fps: --"), DiagText));
}

#[cfg(debug_assertions)]
fn update_diag_overlay(diags: Res<DiagnosticsStore>, mut q: Query<&mut Text, With<DiagText>>) {
    if let Some(fps) = diags
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
    {
        for mut t in &mut q {
            **t = format!("fps: {fps:.0}");
        }
    }
}
