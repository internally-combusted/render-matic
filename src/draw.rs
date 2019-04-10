// draw.rs
// Components and entities relating to things that get drawn on the screen.
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

use nalgebra_glm as glm;

use nalgebra_glm::{Mat4, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use winit::Window;

use crate::{
    component::{ComponentData, ComponentManager, ComponentType},
    data::DataManager,
    error::Error,
    geometry::{self, Transform2D, VertexData},
    render::{FormattedVertexData, Renderer},
    serial::{Color, Index, Position2D, Position3D, Size},
    time,
};

/// A trait required for any `Component` that needs to be drawn on the screen.
pub trait Draw2D {
    fn vertex_data(
        &self,
        projection_matrix: Mat4,
        data_manager: &DataManager,
    ) -> Result<Vec<VertexData>, Error>;
}

/// A partitioned texture (or contiguous region of an atlas) containing animations frames
/// for a sprite.
///
/// Currently, every frame in a spritesheet must be of uniform size. If an atlas contains multiple
/// spritesheets, individual spritesheets can have different frame sizes.
#[derive(Deserialize)]
pub struct Spritesheet {
    pub index: Index,

    /// The number of frames in each row.
    pub pitch: u16,

    pub position: Position2D,
    pub size: Size,
    pub frame_size: Size,
}

/// A static image to display beneath all other images.
#[derive(Deserialize)]
pub struct Background {
    pub index: Index,
    pub position: Position2D,
    pub size: Size,
}

/// Performs all drawing operations.
pub struct DrawingSystem<'a> {
    component_manager: &'a ComponentManager,
    data_manager: &'a DataManager<'a>,
    renderer: Renderer,
}

impl<'a> DrawingSystem<'a> {
    pub fn new(window: &'a Window, data_manager: &'a mut DataManager) -> Result<Self, Error> {
        let component_manager = &data_manager.component_manager;
        let renderer = Renderer::new(
            &data_manager.game_data.program.name,
            window,
            &mut data_manager.resource_manager,
        )?;
        Ok(Self {
            component_manager,
            renderer,
            data_manager,
        })
    }

    /// Collects all Components of the given type and returns a `Vec` of `VertexData`
    /// giving all the vertex information needed to render the components.
    fn get_vertex_data_for_type(&self, component_type: ComponentType) -> Vec<Vec<VertexData>> {
        self.component_manager
            .get_components_of_type(component_type)
            .iter()
            .map(|component| {
                self.component_manager
                    .get_component(*component)
                    .component_data
                    .vertex_data(
                        geometry::projection_matrix(glm::vec2(
                            self.renderer.physical_size.width as f32,
                            self.renderer.physical_size.height as f32,
                        )),
                        self.data_manager,
                    )
                    .unwrap()
            })
            .collect::<Vec<Vec<VertexData>>>()
    }

    /// Collects all drawable [`Component`]s and sends them to the [`Renderer`] to be drawn.
    pub fn draw_frame(&mut self) {
        let types = vec![ComponentType::Quad, ComponentType::Animation2D];
        let mut quad_vertices = vec![];
        let mut quad_counts = vec![];

        // Collect all vertices into a single Vec and count the quads of each type.
        for component_type in types {
            let new_vertices = self.get_vertex_data_for_type(component_type);
            quad_counts.push(new_vertices.len());
            quad_vertices.extend(new_vertices);
        }

        let vertex_data = quad_vertices
            .iter()
            .flat_map(|quad| {
                quad.iter().map(|vertex| FormattedVertexData {
                    position: vertex.position,
                    color: vertex.color,
                    uv: vertex.uv,
                })
            })
            .collect::<Vec<FormattedVertexData>>();

        let mut index_ranges = vec![];
        let mut sum: u32 = 0;
        for count in quad_counts {
            index_ranges.push(sum..sum + (count * geometry::QUAD_INDICES.len()) as u32);
            sum += index_ranges.last().unwrap().end;
        }

        // Draw everything.
        self.renderer.render_frame(vertex_data, index_ranges);
    }

    pub fn clean_up(self) -> Result<(), Error> {
        self.renderer.clean_up()
    }
}

/// A single animation sequence for a sprite.
///
/// Animations only specify the length of each frame, how to repeat, and which frame indices
/// of a spritesheet to use. They aren't tied to specific spritesheets.
///
/// Currently, all frames have to be of equal duration. Repeat an index to hold on a particular
/// frame for more than one "tick".
#[derive(Deserialize, Serialize)]
pub struct Animation {
    /// The sequence of frames to use from the spritesheet.
    pub frames: Vec<u16>,
    /// How the animation progresses through the frames.
    pub animation_type: AnimationType,
    /// The length of each frame in milliseconds.
    pub frame_length: u32,
}

/// Whether and how the animation repeats.
#[derive(Deserialize, PartialEq, Serialize)]
pub enum AnimationType {
    /// Repeat by going through the sequence over and over: 1 2 3 1 2 3
    Loop,
    /// Repeat by going back and forth through the sequence: 1 2 3 2 1 2 3
    Bounce,
    /// Play once, then stop.
    Once,
}

impl Draw2D for ComponentData {
    /// Collect vertex data according to the Component's type.
    fn vertex_data(
        &self,
        projection_matrix: Mat4,
        data_manager: &DataManager,
    ) -> Result<Vec<VertexData>, Error> {
        match self {
            ComponentData::Animation2D { texture_index, .. }
            | ComponentData::Quad { texture_index, .. } => {
                let vertices = self.vertices(projection_matrix)?;
                let uvs = self.uv_coordinates(data_manager)?;
                Ok(vertices
                    .iter()
                    .enumerate()
                    .map(|(index, _)| VertexData {
                        position: Position3D::from(vertices[index]),
                        uv: Position2D::from(uvs[index]),
                        color: Color::from(glm::vec4(1.0, 1.0, 1.0, 1.0)),
                        texture_index: *texture_index,
                    })
                    .collect::<Vec<VertexData>>())
            }
        }
    }
}

impl ComponentData {
    /// Calculates and returns the position data for the associated [`Component`]'s vertices.
    fn vertices(&self, projection_matrix: Mat4) -> Result<Vec<Vec3>, Error> {
        match self {
            ComponentData::Animation2D { .. } | ComponentData::Quad { .. } => {
                Ok(geometry::QUAD_VERTICES
                    .iter()
                    .map(|vertex| {
                        // TODO: Attempting to do any layer/depth stuff here causes weirdness.
                        let transformed =
                            self.transformation_matrix() * glm::vec3(vertex[0], vertex[1], 1.0);
                        (projection_matrix * glm::vec4(transformed.x, transformed.y, 1.0, 1.0))
                            .xyz()
                    })
                    .collect())
            }
        }
    }

    /// Calculates and returns the uv data for the associated [`Component`]'s vertices.
    fn uv_coordinates(&self, data_manager: &DataManager) -> Result<Vec<Vec2>, Error> {
        match self {
            ComponentData::Animation2D {
                texture_index,
                spritesheet_index,
                animations,
                current_animation,
                start_time,
                ..
            } => {
                let spritesheet = &data_manager.spritesheets[*spritesheet_index];
                let frame_size = spritesheet.frame_size;
                let animation = &animations[*current_animation];

                let current_frame = time::calculate_frame(
                    *start_time,
                    animation.frames.len(),
                    animation.frame_length,
                );
                let frame_uv = glm::vec2(
                    ((current_frame % spritesheet.pitch as usize) * frame_size.x as usize) as f32,
                    ((current_frame / spritesheet.pitch as usize) * frame_size.y as usize) as f32,
                );
                let uv_offset = glm::translation2d(&frame_uv)
                    * glm::translation2d(&glm::vec2(
                        spritesheet.position.x as f32,
                        spritesheet.position.y as f32,
                    ));
                Ok(geometry::QUAD_UVS
                    .iter()
                    .map(|uv| {
                        (data_manager.resource_manager.textures[*texture_index]
                            .normalization_matrix
                            * uv_offset
                            //* self.scaling_matrix() // DELETE?
                            * glm::scaling2d(&glm::vec2(frame_size.x, frame_size.y))
                            * glm::vec3(uv[0], uv[1], uv[2]))
                        .xy()
                    })
                    .collect())
            }
            ComponentData::Quad {
                texture_index,
                uv_offset,
                ..
            } => Ok(geometry::QUAD_UVS
                .iter()
                .map(|uv| {
                    (data_manager.resource_manager.textures[*texture_index].normalization_matrix
                        * glm::translation2d(&glm::vec2(uv_offset.x, uv_offset.y))
                        * self.scaling_matrix()
                        * glm::vec3(uv[0], uv[1], uv[2]))
                    .xy()
                })
                .collect()),
            // _ => Err(Error::WrongType(
            //     "uv_coordinates() expected ComponentData::Animation2D variant",
            // )),
        }
    }
}
