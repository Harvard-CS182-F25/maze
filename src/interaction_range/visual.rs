use std::f32::consts::TAU;

use bevy::{
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

const RING_SEGMENTS: usize = 96; // how many dashes
const RING_ON_RATIO: f32 = 0.75; // % of each segment that is "on"
const RING_THICKNESS_FRAC: f32 = 0.07; // thickness as a fraction of radius (unit mesh)

#[derive(Resource)]
pub struct RingAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

impl FromWorld for RingAssets {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let mesh = meshes.add(make_unit_dotted_ring(
            RING_SEGMENTS,
            RING_ON_RATIO,
            RING_THICKNESS_FRAC,
        ));

        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::WHITE,
            cull_mode: None,
            ..Default::default()
        });

        RingAssets { mesh, material }
    }
}

fn make_unit_dotted_ring(segments: usize, on_ratio: f32, thickness_frac: f32) -> Mesh {
    let outer_radius = 1.0;
    let inner_radius = (1.0 - thickness_frac).max(0.0);

    let mut positions = Vec::with_capacity(segments * 4);
    let mut normals = Vec::with_capacity(segments * 4);
    let mut uvs = Vec::with_capacity(segments * 4);
    let mut indices = Vec::with_capacity(segments * 6);

    for i in 0..segments {
        let segment_start_point = (i as f32) / (segments as f32) * TAU;
        let segment_end_point = (i + 1) as f32 / (segments as f32) * TAU;

        let midpoint = (segment_start_point + segment_end_point) / 2.0;
        let dash_length = (segment_end_point - segment_start_point) * on_ratio;
        let dash_start = midpoint - dash_length / 2.0;
        let dash_end = midpoint + dash_length / 2.0;

        if dash_end <= dash_start {
            continue;
        }

        // Approximate the annulus with two triangles
        let (start_cos, start_sin) = (dash_start.cos(), dash_start.sin());
        let (end_cos, end_sin) = (dash_end.cos(), dash_end.sin());

        let v0 = [outer_radius * start_cos, 0.0, outer_radius * start_sin];
        let v1 = [outer_radius * end_cos, 0.0, outer_radius * end_sin];
        let v2 = [inner_radius * end_cos, 0.0, inner_radius * end_sin];
        let v3 = [inner_radius * start_cos, 0.0, inner_radius * start_sin];

        let base = positions.len() as u32;
        positions.extend_from_slice(&[v0, v1, v2, v3]);
        uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
        normals.extend(std::iter::repeat_n([0.0, 1.0, 0.0], 4));

        // Two triangles: v0, v1, v2 and v0, v2, v3 with normals pointing up
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
