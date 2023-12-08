use noise::NoiseFn;

use bevy::{
    prelude::*,
    render::{
        mesh::Indices,
        render_resource::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat},
    },
};
use bevy_xpbd_3d::prelude::*;

use crate::GameCollisionLayer;

#[derive(Debug, Clone)]
#[derive(Component)]
pub struct TerrainChunk {
    // the quad size of each rendered chunk of the mesh
    pub quad_size: Vec2,
    // the number of vertices in each chunk
    pub chunk_size: (u16, u16),
    pub origin_vertex: (i32, i32),
    pub noise_seed: u32,
}

impl Default for TerrainChunk {
    fn default() -> Self {
        TerrainChunk::new((0, 0), (50, 50), Vec2::ONE * 2., 0)
    }
}

impl TerrainChunk {
    pub fn new(
        origin_vertex: (i32, i32),
        chunk_size: (u16, u16),
        quad_size: Vec2,
        noise_seed: u32,
    ) -> Self {
        Self {
            quad_size,
            chunk_size,
            noise_seed,
            origin_vertex,
        }
    }

    pub fn generate_mesh(&self, noise: &impl NoiseFn<f64, 2>) -> Mesh {
        let num_vertices = self.chunk_size.0 * self.chunk_size.1;
        let num_indices = (self.chunk_size.0 - 1) * (self.chunk_size.1 - 1) * 6;
        let mut positions: Vec<[f32; 3]> = Vec::with_capacity(num_vertices as usize);
        let mut normals: Vec<[f32; 3]> = Vec::with_capacity(num_vertices as usize);
        let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(num_vertices as usize);
        // Each row is (M - 1) X (N-1) quads
        let mut indices: Vec<u32> = Vec::with_capacity(num_indices as usize);

        let slope = Quat::from_rotation_x(std::f32::consts::FRAC_PI_4);

        // let total_z = self.origin_vertex.1 * self.chunk_size.1 as i32 + z;
        for z in 0..=self.chunk_size.1 as i32 {
            for x in 0..=self.chunk_size.0 as i32 {
                let tx = x as f32 / self.chunk_size.0 as f32 - 0.5;
                let x_position = tx * self.chunk_size.0 as f32 * self.quad_size.x;
                let z_position = z as f32 * self.quad_size.y;

                let sample_x = (x + self.origin_vertex.0 * self.chunk_size.0 as i32) as f64;
                let sample_z = (self.chunk_size.1 as i32 - z
                    + self.origin_vertex.1 * self.chunk_size.1 as i32)
                    as f64;
                let noise_sample = noise.get([sample_x, sample_z]) as f32;
                let sloped_noise = slope * Vec3::new(0., noise_sample, 0.);

                let sloped_position = Vec3::new(x_position, -z_position, z_position);
                let unsloped_position = Vec3::new(x_position, 0., z_position);
                let target_position = sloped_position + sloped_noise;

                if self.origin_vertex.1 > 0 {
                    positions.push(unsloped_position.to_array());
                    normals.push(Vec3::Y.to_array());
                } else if self.origin_vertex.1 == 0 {
                    // blend between 0 and the noise
                    let chunk_z_ratio =
                        (self.chunk_size.1 as f32 - z as f32) / self.chunk_size.1 as f32;

                    positions.push(
                        target_position
                            .lerp(unsloped_position, chunk_z_ratio)
                            .to_array(),
                    );

                    normals.push(Vec3::Y.to_array());
                } else {
                    positions.push(target_position.to_array());
                    normals.push(Vec3::Y.to_array());
                }
                // TODO: offsets for less repetitive uv?
                uvs.push([
                    tx,
                    (z % (self.chunk_size.1 + 1) as i32) as f32 / self.chunk_size.1 as f32,
                ]);
            }

            if z < self.chunk_size.1 as i32 {
                for x in 0..self.chunk_size.0 {
                    let row_offset = self.chunk_size.0 as u32 + 1;
                    let quad_index = row_offset * z as u32 + x as u32;
                    // right triangle
                    indices.push(quad_index + row_offset + 1);
                    indices.push(quad_index + 1);
                    indices.push(quad_index + row_offset);
                    // left triangle
                    indices.push(quad_index);
                    indices.push(quad_index + row_offset);
                    indices.push(quad_index + 1);
                }
            }
        }

        Mesh::new(PrimitiveTopology::TriangleList)
            .with_indices(Some(Indices::U32(indices)))
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    }

    pub fn to_bundle(
        self,
        noise: &impl NoiseFn<f64, 2>,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> impl Bundle {
        let mesh = self.generate_mesh(noise);
        let x = self.origin_vertex.0 as f32 * self.chunk_size.0 as f32 * self.quad_size.x;
        let y = (self.origin_vertex.1 as f32).clamp(std::f32::NEG_INFINITY, 0.)
            * self.chunk_size.1 as f32
            * self.quad_size.y;
        let z = -(self.origin_vertex.1 as f32 * self.chunk_size.1 as f32) * self.quad_size.y;
        (
            Name::new(format!(
                "Terrain Chunk {}x{}",
                self.origin_vertex.0, self.origin_vertex.1,
            )),
            RigidBody::Static,
            ColliderDensity(1e7),
            AsyncCollider(ComputedCollider::TriMesh),
            CollisionLayers::new([GameCollisionLayer::Bodies], [GameCollisionLayer::Bodies]),
            PbrBundle {
                mesh: meshes.add(mesh),
                material: materials.add(StandardMaterial {
                    base_color_texture: Some(images.add(uv_debug_texture())),
                    ..default()
                }),
                transform: Transform::from_xyz(x, y, z),
                ..Default::default()
            },
            self,
        )
    }
}

/// Creates a colorful test pattern
fn uv_debug_texture() -> Image {
    const TEXTURE_SIZE: usize = 8;

    let mut palette: [u8; 32] = [
        255, 102, 159, 255, 255, 159, 102, 255, 236, 255, 102, 255, 121, 255, 102, 255, 102, 255,
        198, 255, 102, 198, 255, 255, 121, 102, 255, 255, 236, 102, 255, 255,
    ];

    let mut texture_data = [0; TEXTURE_SIZE * TEXTURE_SIZE * 4];
    for y in 0..TEXTURE_SIZE {
        let offset = TEXTURE_SIZE * y * 4;
        texture_data[offset..(offset + TEXTURE_SIZE * 4)].copy_from_slice(&palette);
        palette.rotate_right(4);
    }

    Image::new_fill(
        Extent3d {
            width: TEXTURE_SIZE as u32,
            height: TEXTURE_SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &texture_data,
        TextureFormat::Rgba8UnormSrgb,
    )
}
