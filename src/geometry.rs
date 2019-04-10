// geometry.rs
// Helpful things for handling rendering geometry.
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

use nalgebra_glm as glm;
use nalgebra_glm::{Mat3, Mat4, Vec2};
use serde::{Deserialize, Serialize};

use crate::serial::{Color, Index, Position2D, Position3D};

/// The UV coordinates to make a texture fit a quad precisely.
pub const QUAD_UVS: [[f32; 3]; 4] = [
    [0.0, 0.0, 1.0], // top-left
    [0.0, 1.0, 1.0], // bottom-left
    [1.0, 1.0, 1.0], // bottom-right
    [1.0, 0.0, 1.0], // top-right
];

/// The vertices of a unit square centered on the origin.
// This greatly simplifies the math for rotating quads.
pub const QUAD_VERTICES: [[f32; 2]; 4] = [
    [-0.5, 0.5],  //top-left
    [-0.5, -0.5], //bottom-left
    [0.5, -0.5],  // bottom-right
    [0.5, 0.5],   // top-right
];

// A quad is two triangles with three vertices each, but two of the vertices are the same.
/// The base vertex indices to form a quad.
pub const QUAD_INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];

// Assuming normal pixel density, should give 1 pixel per logical unit?
/// The orthographic projection matrix (since we're doing a static 2D sort of thing.)
pub fn projection_matrix(physical_size: Vec2) -> Mat4 {
    glm::ortho_lh_zo(
        -physical_size.x / 2.0,
        physical_size.x / 2.0,
        -physical_size.y / 2.0,
        physical_size.y / 2.0,
        0.0,
        1.0,
    )
}

/// Contains the z-coordinates for each layer of quads.
///
/// Larger values are further away from the camera. Because we're using
/// orthographic projection, the z-distance doesn't affect size; it just
/// makes sure that sprites are drawn on top of backgrounds instead of under
/// them, etc.
pub enum LayerDepth {
    Sprite = 0,
    Background = 1,
}

/// Contains all of the data needed for a vertex.
#[derive(Copy, Clone, Debug)]
pub struct VertexData {
    pub position: Position3D,
    pub uv: Position2D,
    pub color: Color,
    pub texture_index: Index,
}

// It'd be lovely if I could use Vec2 here instead of Vec<f32>
// but I don't feel like figuring out how to implement Deserialize
// for a type from an external library.
/// A representation of objects' positions, orientations, etc.
#[derive(Deserialize, Serialize)]
pub struct TransformData {
    pub translation: Vec<f32>,
    pub scaling: Vec<f32>,
    pub rotation: f32,
}

impl TransformData {
    pub fn new(translation: Vec2, scaling: Vec2, rotation: f32) -> TransformData {
        TransformData {
            translation: vec![translation.x, translation.y],
            scaling: vec![scaling.x, scaling.y],
            rotation,
        }
    }
}

/// This trait is to ensure that every component that can be drawn
/// can provide its own transformation data.
///
/// By default, all transform matrices are the identity matrix.
pub trait Transform2D {
    fn translation_matrix(&self) -> Mat3 {
        glm::identity()
    }
    fn rotation_matrix(&self) -> Mat3 {
        glm::identity()
    }
    fn scaling_matrix(&self) -> Mat3 {
        glm::identity()
    }

    /// This should probably never need to be overridden.
    fn transformation_matrix(&self) -> Mat3 {
        self.translation_matrix() * self.rotation_matrix() * self.scaling_matrix()
    }
}
