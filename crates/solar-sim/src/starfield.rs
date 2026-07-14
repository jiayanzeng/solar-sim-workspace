//! WP13 retained Bright Star Catalogue point-sprite mesh — Rev C §10.5.
//!
//! The committed asset is produced offline by `xtask bake-starfield`. Each
//! baked celestial point expands into a camera-centered tangent quad inside
//! one retained mesh so the default Bevy material pipeline can display the
//! baked magnitude-scaled size (WebGPU point primitives have fixed size).

use crate::SimulationSet;
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use std::fmt;
use std::path::{Path, PathBuf};

pub const DEFAULT_STARFIELD_PATH: &str = "assets/starfield.bsc";
pub const STARFIELD_RADIUS_UNITS: f32 = 8.0e8;
pub const EXPECTED_STARFIELD_POINTS: usize = 5_000;
const STAR_POINT_HALF_SIZE_UNITS: f32 = 4.0e5;
const BAKED_MAGIC: &[u8; 8] = b"SSBSC1\0\0";
const BAKED_HEADER_BYTES: usize = 12;
const BAKED_RECORD_BYTES: usize = 22;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StarfieldPoint {
    pub hr: u16,
    pub position_ecliptic: [f32; 3],
    pub magnitude: f32,
    pub point_size: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StarfieldAssetError {
    Read { path: PathBuf, message: String },
    BadMagic,
    Truncated,
    TooManyRecords { count: usize },
    LengthMismatch { expected: usize, actual: usize },
    InvalidRecord { index: usize },
}

impl fmt::Display for StarfieldAssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, message } => {
                write!(f, "could not read '{}': {message}", path.display())
            }
            Self::BadMagic => write!(f, "not a solar-sim BSC starfield"),
            Self::Truncated => write!(f, "starfield header is truncated"),
            Self::TooManyRecords { count } => {
                write!(
                    f,
                    "starfield record count {count} exceeds its binary format"
                )
            }
            Self::LengthMismatch { expected, actual } => {
                write!(f, "starfield is {actual} bytes; expected {expected}")
            }
            Self::InvalidRecord { index } => {
                write!(
                    f,
                    "starfield record {index} is not finite or on the unit sphere"
                )
            }
        }
    }
}

impl std::error::Error for StarfieldAssetError {}

#[derive(Resource, Debug, Clone)]
pub struct StarfieldSource(pub PathBuf);

impl Default for StarfieldSource {
    fn default() -> Self {
        Self(PathBuf::from(DEFAULT_STARFIELD_PATH))
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct StarfieldRoot {
    pub point_count: usize,
}

pub struct StarfieldPlugin;

impl Plugin for StarfieldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StarfieldSource>()
            .add_systems(Startup, spawn_starfield)
            .add_systems(
                Update,
                center_starfield_on_camera.in_set(SimulationSet::Render),
            );
    }
}

pub fn load_starfield(path: &Path) -> Result<Vec<StarfieldPoint>, StarfieldAssetError> {
    let bytes = std::fs::read(path).map_err(|error| StarfieldAssetError::Read {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    decode_starfield(&bytes)
}

pub fn decode_starfield(bytes: &[u8]) -> Result<Vec<StarfieldPoint>, StarfieldAssetError> {
    if bytes.len() < BAKED_HEADER_BYTES {
        return Err(StarfieldAssetError::Truncated);
    }
    if &bytes[..8] != BAKED_MAGIC {
        return Err(StarfieldAssetError::BadMagic);
    }
    let count = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let expected = count
        .checked_mul(BAKED_RECORD_BYTES)
        .and_then(|record_bytes| record_bytes.checked_add(BAKED_HEADER_BYTES))
        .ok_or(StarfieldAssetError::TooManyRecords { count })?;
    if bytes.len() != expected {
        return Err(StarfieldAssetError::LengthMismatch {
            expected,
            actual: bytes.len(),
        });
    }

    let mut points = Vec::with_capacity(count);
    for index in 0..count {
        let offset = BAKED_HEADER_BYTES + index * BAKED_RECORD_BYTES;
        let hr = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        let mut cursor = offset + 2;
        let mut read_f32 = || {
            let value = f32::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]);
            cursor += 4;
            value
        };
        let position_ecliptic = [read_f32(), read_f32(), read_f32()];
        let magnitude = read_f32();
        let point_size = read_f32();
        let norm_squared: f32 = position_ecliptic.iter().map(|value| value * value).sum();
        if !position_ecliptic.iter().all(|value| value.is_finite())
            || !magnitude.is_finite()
            || !point_size.is_finite()
            || point_size <= 0.0
            || (norm_squared - 1.0).abs() > 2.0e-5
        {
            return Err(StarfieldAssetError::InvalidRecord { index });
        }
        points.push(StarfieldPoint {
            hr,
            position_ecliptic,
            magnitude,
            point_size,
        });
    }
    Ok(points)
}

fn spawn_starfield(
    mut commands: Commands,
    source: Res<StarfieldSource>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let points = match load_starfield(&source.0) {
        Ok(points) => points,
        Err(error) => {
            warn!("BSC starfield unavailable: {error}");
            return;
        }
    };
    let point_count = points.len();
    commands.spawn((
        Name::new("Yale BSC starfield"),
        StarfieldRoot { point_count },
        Mesh3d(meshes.add(build_starfield_mesh(&points))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
    ));
}

fn center_starfield_on_camera(
    camera: Query<&Transform, (With<Camera3d>, Without<StarfieldRoot>)>,
    mut starfields: Query<&mut Transform, With<StarfieldRoot>>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    for mut transform in &mut starfields {
        transform.translation = camera.translation;
    }
}

fn build_starfield_mesh(points: &[StarfieldPoint]) -> Mesh {
    let mut positions = Vec::with_capacity(points.len() * 4);
    let mut normals = Vec::with_capacity(points.len() * 4);
    let mut colors = Vec::with_capacity(points.len() * 4);
    let mut indices = Vec::with_capacity(points.len() * 6);

    for (index, point) in points.iter().enumerate() {
        // Ecliptic x-y is Bevy x-z; ecliptic z is Bevy y, matching body rebase.
        let direction = Vec3::new(
            point.position_ecliptic[0],
            point.position_ecliptic[2],
            point.position_ecliptic[1],
        )
        .normalize();
        let helper = if direction.y.abs() < 0.9 {
            Vec3::Y
        } else {
            Vec3::X
        };
        let tangent = direction.cross(helper).normalize();
        let bitangent = direction.cross(tangent).normalize();
        let center = direction * STARFIELD_RADIUS_UNITS;
        let half_size = STAR_POINT_HALF_SIZE_UNITS * point.point_size;
        let tangent = tangent * half_size;
        let bitangent = bitangent * half_size;
        for vertex in [
            center - tangent - bitangent,
            center + tangent - bitangent,
            center + tangent + bitangent,
            center - tangent + bitangent,
        ] {
            positions.push(vertex.to_array());
            normals.push((-direction).to_array());
        }
        let brightness = (0.35 + 0.65 * ((6.5 - point.magnitude) / 8.0)).clamp(0.25, 1.0);
        colors.extend([[brightness, brightness, brightness, 1.0]; 4]);
        let base = (index * 4) as u32;
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn encoded(points: &[StarfieldPoint]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(BAKED_MAGIC);
        bytes.extend_from_slice(&(points.len() as u32).to_le_bytes());
        for point in points {
            bytes.extend_from_slice(&point.hr.to_le_bytes());
            for coordinate in point.position_ecliptic {
                bytes.extend_from_slice(&coordinate.to_le_bytes());
            }
            bytes.extend_from_slice(&point.magnitude.to_le_bytes());
            bytes.extend_from_slice(&point.point_size.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn baked_asset_rejects_corrupt_or_non_unit_records() {
        assert_eq!(
            decode_starfield(b"short").unwrap_err(),
            StarfieldAssetError::Truncated
        );
        let invalid = StarfieldPoint {
            hr: 1,
            position_ecliptic: [2.0, 0.0, 0.0],
            magnitude: 1.0,
            point_size: 1.0,
        };
        assert_eq!(
            decode_starfield(&encoded(&[invalid])).unwrap_err(),
            StarfieldAssetError::InvalidRecord { index: 0 }
        );
    }

    #[test]
    fn retained_mesh_expands_each_magnitude_scaled_point_once() {
        let points = [
            StarfieldPoint {
                hr: 1,
                position_ecliptic: [1.0, 0.0, 0.0],
                magnitude: -1.0,
                point_size: 4.0,
            },
            StarfieldPoint {
                hr: 2,
                position_ecliptic: [0.0, 1.0, 0.0],
                magnitude: 6.0,
                point_size: 1.0,
            },
        ];
        let decoded = decode_starfield(&encoded(&points)).unwrap();
        let mesh = build_starfield_mesh(&decoded);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("starfield positions must be float3");
        };
        assert_eq!(positions.len(), 8);
        assert_eq!(mesh.indices().unwrap().len(), 12);

        let bright_span = Vec3::from_array(positions[0]).distance(Vec3::from_array(positions[1]));
        let faint_span = Vec3::from_array(positions[4]).distance(Vec3::from_array(positions[5]));
        assert!((bright_span / faint_span - 4.0).abs() < 1.0e-5);
    }

    #[test]
    fn committed_nasa_bake_has_five_thousand_unit_points_and_correct_polaris_tilt() {
        const BAKED: &[u8] = include_bytes!("../../../assets/starfield.bsc");
        const PROVENANCE: &str = include_str!("../../../assets/starfield-SOURCE.md");
        let points = decode_starfield(BAKED).unwrap();
        assert_eq!(points.len(), EXPECTED_STARFIELD_POINTS);
        let polaris = points.iter().find(|point| point.hr == 424).unwrap();
        let separation_deg = polaris.position_ecliptic[2]
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees();
        assert!((separation_deg - 23.4).abs() < 1.0, "{separation_deg}");
        assert!(PROVENANCE.contains("NASA Open Data"));
        assert!(PROVENANCE.contains("government-works"));
        assert!(
            PROVENANCE.contains("312d6b4a94f0fd62e4877f7c63d36ba8af7ac084537f05d07faead3ef6fd628b")
        );
    }

    #[test]
    fn committed_bake_spawns_as_one_retained_mesh() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/starfield.bsc");
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(StarfieldSource(source))
            .add_systems(Startup, spawn_starfield);
        app.update();

        let mut query = app.world_mut().query::<(&StarfieldRoot, &Mesh3d)>();
        let roots: Vec<_> = query
            .iter(app.world())
            .map(|(root, mesh)| (*root, mesh.0.clone()))
            .collect();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].0.point_count, EXPECTED_STARFIELD_POINTS);
        let meshes = app.world().resource::<Assets<Mesh>>();
        let mesh = meshes.get(&roots[0].1).unwrap();
        assert_eq!(mesh.count_vertices(), EXPECTED_STARFIELD_POINTS * 4);
        assert_eq!(mesh.indices().unwrap().len(), EXPECTED_STARFIELD_POINTS * 6);
    }
}
