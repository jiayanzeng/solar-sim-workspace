//! WP15 — catalog-driven surface textures and Saturn's ring disc.
//!
//! Texture handles are optional polish. A missing assignment or asset server
//! keeps the catalog color as the material truth, while assignments that came
//! through the curated manifest use the same body mesh and physics radius.

use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use sim_core::catalog::BodyRecord;

use crate::{KM_PER_RENDER_UNIT, SUN_LIGHT_INTENSITY_LUMENS, SUN_LIGHT_RANGE_UNITS};

pub(crate) const SATURN_RING_TEXTURE_PATH: &str = "textures/saturn-rings.ktx2";
pub(crate) const SATURN_RING_INNER_RADIUS: f32 = 1.11;
pub(crate) const SATURN_RING_OUTER_RADIUS: f32 = 2.32;
const SATURN_RING_SEGMENTS: usize = 256;

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct SaturnRing;

pub(crate) fn catalog_color(body: &BodyRecord) -> Color {
    let (r, g, b) = body.color_srgb;
    Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

pub(crate) fn body_material(
    body: &BodyRecord,
    asset_server: Option<&AssetServer>,
) -> StandardMaterial {
    let color = catalog_color(body);
    let base_color_texture = body
        .texture
        .as_ref()
        .and_then(|path| asset_server.map(|assets| assets.load(path.clone())));
    let night_side_texture = base_color_texture.clone();
    let is_star = body.category == sim_core::catalog::Category::Star;
    let emissive = if is_star {
        let (r, g, b) = body.color_srgb;
        LinearRgba::rgb(
            r as f32 / 255.0 * 80.0,
            g as f32 / 255.0 * 80.0,
            b as f32 / 255.0 * 80.0,
        )
    } else if night_side_texture.is_some() {
        // A very low texture-matched floor keeps the night side legible at
        // astronomical light distances without turning planets into unlit
        // sprites; direct Sun light still supplies the day/night contrast.
        LinearRgba::rgb(0.06, 0.06, 0.065)
    } else {
        LinearRgba::BLACK
    };
    StandardMaterial {
        // White preserves the source texels. Catalog color is the complete
        // fallback when either the assignment or AssetServer is absent.
        base_color: if base_color_texture.is_some() {
            Color::WHITE
        } else {
            color
        },
        base_color_texture,
        emissive,
        emissive_texture: if is_star { None } else { night_side_texture },
        unlit: is_star,
        ..default()
    }
}

pub(crate) fn spawn_saturn_ring(
    commands: &mut Commands,
    parent: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: Option<&AssetServer>,
) {
    let texture = asset_server.map(|assets| assets.load(SATURN_RING_TEXTURE_PATH));
    commands.spawn((
        Name::new("Saturn rings"),
        SaturnRing,
        ChildOf(parent),
        Mesh3d(meshes.add(build_saturn_ring_mesh())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.92, 0.86, 0.72, 0.9),
            base_color_texture: texture,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            perceptual_roughness: 0.9,
            ..default()
        })),
        Transform::default(),
    ));
}

pub(crate) fn build_saturn_ring_mesh() -> Mesh {
    let mut positions = Vec::with_capacity((SATURN_RING_SEGMENTS + 1) * 2);
    let mut normals = Vec::with_capacity((SATURN_RING_SEGMENTS + 1) * 2);
    let mut uvs = Vec::with_capacity((SATURN_RING_SEGMENTS + 1) * 2);
    let mut indices = Vec::with_capacity(SATURN_RING_SEGMENTS * 6);
    for segment in 0..=SATURN_RING_SEGMENTS {
        let turn = segment as f32 / SATURN_RING_SEGMENTS as f32;
        let (sin, cos) = if segment == SATURN_RING_SEGMENTS {
            // Close the retained mesh bit-exactly; TAU.sin() is not zero in
            // f32 and otherwise leaves a hairline seam at extreme zoom.
            (0.0, 1.0)
        } else {
            (std::f32::consts::TAU * turn).sin_cos()
        };
        for (radius, radial_uv) in [
            (SATURN_RING_INNER_RADIUS, 0.0),
            (SATURN_RING_OUTER_RADIUS, 1.0),
        ] {
            positions.push([radius * cos, 0.0, radius * sin]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([radial_uv, turn]);
        }
    }
    for segment in 0..SATURN_RING_SEGMENTS as u32 {
        let inner = segment * 2;
        let outer = inner + 1;
        let next_inner = inner + 2;
        let next_outer = inner + 3;
        indices.extend([inner, outer, next_outer, inner, next_outer, next_inner]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub(crate) fn sun_light(body: &BodyRecord) -> PointLight {
    let scale = (body.radius_km / KM_PER_RENDER_UNIT) as f32;
    PointLight {
        color: catalog_color(body),
        intensity: SUN_LIGHT_INTENSITY_LUMENS,
        range: SUN_LIGHT_RANGE_UNITS,
        radius: scale,
        shadow_maps_enabled: false,
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn saturn_ring_mesh_is_an_open_annulus_with_a_closed_uv_seam() {
        let mesh = build_saturn_ring_mesh();
        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() {
            VertexAttributeValues::Float32x3(values) => values,
            other => panic!("unexpected ring position format: {other:?}"),
        };
        let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap() {
            VertexAttributeValues::Float32x2(values) => values,
            other => panic!("unexpected ring UV format: {other:?}"),
        };
        assert_eq!(positions.len(), (SATURN_RING_SEGMENTS + 1) * 2);
        assert_eq!(positions[0], positions[positions.len() - 2]);
        assert_eq!(positions[1], positions[positions.len() - 1]);
        assert_eq!(uvs[0], [0.0, 0.0]);
        assert_eq!(uvs[uvs.len() - 2], [0.0, 1.0]);
        assert!((Vec3::from(positions[0]).length() - SATURN_RING_INNER_RADIUS).abs() < 1.0e-6);
        assert!((Vec3::from(positions[1]).length() - SATURN_RING_OUTER_RADIUS).abs() < 1.0e-6);
    }
}
