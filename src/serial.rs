// serial.rs
// Common structs and other things for use with (de-)serialization.
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

//! Types and traits to assist with (de-)serialization.
//!
//! External crates may provide opaque types that don't implement
//! the [`serde`] crate's [`Serialize`] or [`Deserialize`] traits.

use nalgebra_glm as glm;

use nalgebra_glm::{Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

/// A unified representation of a 3D position.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Position3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Into<Vec3> for Position3D {
    /// (Hopefully) automatic conversion to simplify use with `glm` functions.
    fn into(self) -> Vec3 {
        glm::vec3(self.x, self.y, self.z)
    }
}

impl From<Vec3> for Position3D {
    /// (Hopefully) automatic conversion to simplify use with `glm` functions.
    fn from(vec: Vec3) -> Position3D {
        Position3D {
            x: vec.x,
            y: vec.y,
            z: vec.z,
        }
    }
}

/// A unified representation of a 2D size.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Size {
    pub x: f32,
    pub y: f32,
}

impl Into<Vec2> for Size {
    /// (Hopefully) automatic conversion to simplify use with `glm` functions.
    fn into(self) -> Vec2 {
        glm::vec2(self.x, self.y)
    }
}

impl From<Vec2> for Size {
    /// (Hopefully) automatic conversion to simplify use with `glm` functions.
    fn from(vec: Vec2) -> Size {
        Size { x: vec.x, y: vec.y }
    }
}

/// Allow code to be clear that a position is involved instead of confusingly
/// passing around sizes to functions that handle positions.
pub type Position2D = Size;

/// A unified representation of rgba color.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Into<Vec4> for Color {
    /// (Hopefully) automatic conversion to simplify use with `glm` functions.
    fn into(self) -> Vec4 {
        glm::vec4(self.r, self.g, self.b, self.a)
    }
}

impl From<Vec4> for Color {
    /// (Hopefully) automatic conversion to simplify use with `glm` functions.
    fn from(vec: Vec4) -> Color {
        Color {
            r: vec.x,
            g: vec.y,
            b: vec.z,
            a: vec.w,
        }
    }
}

// TODO: It may actually be worse to have these aliases since the appearance of an
// unknown type may mislead people into thinking that they're actual new types.
pub type Filename = String;
pub type Index = usize;
