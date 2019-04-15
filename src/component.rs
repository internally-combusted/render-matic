// component.rs
// Manages components, discrete units of game functionality.
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

//! The "component" part of the entity-component model.
//!
//! *Components* are collections of data providing all the information a *system* needs
//! to execute a desired behavior. For example, in order to draw a tree to the screen,
//! the [`DrawingSystem`] needs to know how big the tree should be, where it should be
//! drawn, what texture or part of a texture contains the picture we want to use, and
//! so on. All of this information is bundled together into a [`Component`] with the
//! and placed in the [`ComponentManager`]. When it comes time to
//! draw a frame to the screen, the [`DrawingSystem`] will ask the [`ComponentManager`]
//! for every [`Component`] of each drawable type and use that information to
//! draw all of the currently existing drawable components to the screen.
//!
//! [`Component`]: struct.Component.html
//! [`ComponentManager`]: struct.ComponentManager.html
//! [`DrawingSystem`]: ../draw/struct.DrawingSystem.html

use nalgebra_glm as glm;
use nalgebra_glm::Mat3;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::{fs, time::Instant};
use winit::VirtualKeyCode;

use crate::{
    draw::Animation,
    entity::EntityManager,
    error::Error,
    geometry::{Movement2D, Transform2D, TransformData},
    serial::{Index, Position2D},
};

/// An `enum` for the different types of [`Component`]s.
/// Rust provides `std::mem::discriminant` for looking at which variant an `enum` object
/// belongs to, and it works for `enum` variants that have attached data. However, it's a little
/// clunky and it's simpler to use a plain `enum` for variant identification and comparison.
///
/// [`Component`]: struct.Component.html
#[derive(Deserialize, PartialEq, Serialize)]
pub enum ComponentType {
    /// A simple textured quad.
    Quad = 0,
    /// An animated quad.
    Animation2D = 1,
}

/// Stores a [`Component`]'s data. Use `match` or `if let` constructs to access members of a
/// particular `ComponentData` variant.
///
/// [`Component`]: struct.Component.html
#[derive(Deserialize, Serialize)]
pub enum ComponentData {
    /// An animated quad.
    Animation2D {
        texture_index: Index,
        /// Which [`Spritesheet`] this [`Component`] uses.
        ///
        /// [`Spritesheet`]: ../draw/struct.Spritesheet.html
        /// [`Component`]: struct.Component.html
        spritesheet_index: Index,
        /// The [`Component`]'s z-distance. Lower layers have higher values (1 is further from the camera than 0, etc.).
        ///
        /// [`Component`]: struct.Component.html
        layer: u16,
        /// A list of this [`Component`]'s [`Animation`]s.
        ///
        /// [`Animation`]: ../draw/struct.Animation.html
        /// [`Component`]: struct.Component.html
        animations: Vec<Animation>,
        /// An [`Index`] into the `animations` `Vec` representing the currently active animation.
        ///
        /// [`Index`]: ../serial/type.Index.html
        current_animation: Index,
        /// The `Component`'s position, scaling, and rotation values.
        ///
        /// [`Component`]: struct.Component.html
        transform_data: TransformData,

        #[serde(default = "std::time::Instant::now", skip)]
        /// When the current animation began, used to calculate which frame in the current animation to use.
        start_time: Instant,
        movement: Movement2D,
    },
    /// A plain textured quad.
    Quad {
        texture_index: Index,
        /// The [`Component`]'s position, scaling, and rotation values.
        /// [`Component`]: struct.Component.html
        transform_data: TransformData,
        /// The (u, v) coordinates of the point on the `Quad`'s texture that should be attached to
        /// the `Quad`'s top-left corner.
        uv_offset: Position2D,
        layer: u16,
    },
}

impl Transform2D for ComponentData {
    fn rotation_matrix(&self) -> Mat3 {
        match self {
            ComponentData::Animation2D { transform_data, .. }
            | ComponentData::Quad { transform_data, .. } => {
                glm::rotation2d(transform_data.rotation)
            }
        }
    }

    fn scaling_matrix(&self) -> Mat3 {
        match self {
            ComponentData::Animation2D { transform_data, .. }
            | ComponentData::Quad { transform_data, .. } => glm::scaling2d(&glm::vec2(
                transform_data.scaling[0],
                transform_data.scaling[1],
            )),
        }
    }

    fn translation_matrix(&self) -> Mat3 {
        match self {
            ComponentData::Animation2D { transform_data, .. }
            | ComponentData::Quad { transform_data, .. } => glm::translation2d(&glm::vec2(
                transform_data.translation[0],
                transform_data.translation[1],
            )),
        }
    }
}

/// A single data object with no associated behavior.
#[derive(Deserialize, Serialize)]
pub struct Component {
    /// This `Component`'s unique index in the list of all `Component`s.
    pub id: Index,
    /// The type of the `Component`.
    pub component_type: ComponentType,
    /// The data the `Component` carries.
    pub component_data: ComponentData,
}

/// The owner for all [`Component`]s. Controls creation, access, and deletion.
///
/// [`Component`]: struct.Component.html
// Components are mostly handled via their indices so the borrow checker
// doesn't get mad about a bunch of references being thrown around.
#[derive(Deserialize)]
pub struct ComponentManager {
    /// The number of [`Component`]s that have been created. Used to assign new id numbers.
    ///
    /// [`Component`]: struct.Component.html
    pub counter: Index,
    /// A list of all extant game [`Component`]s.
    ///
    /// [`Component`]: struct.Component.html
    pub components: Vec<Component>,
}

impl ComponentManager {
    /// Loads all [`Component`] data from disk.
    ///
    /// # Errors
    ///
    /// The [`Component`] data is expected to be found in `./data/components.yaml` and an [`Error::Io`]
    /// will be returned if that file doesn't exist or can't be read for some reason.
    ///
    /// An [`Error::SerdeYaml`] will be returned if the YAML is malformed or if its data can't be matched
    /// to a valid [`Component`] structure.
    ///
    /// [`Component`]: struct.Component.html
    /// [`Error::Io`]: ../error/enum.Error.html#variant.Io
    /// [`Error::SerdeYaml`]: ../error/enum.Error.html#variant.SerdeYaml
    pub fn load_components() -> Result<ComponentManager, Error> {
        use log::debug;
        debug!("Loading components...");
        Ok(serde_yaml::from_str(&fs::read_to_string(
            "./data/components.yaml",
        )?)?)
    }

    /// Creates a [`Component`] with the given type and data, then returns the new [`Component`]'s `id`.
    ///
    /// [`Component`]: struct.Component.html
    pub fn create_component(
        &mut self,
        component_type: ComponentType,
        data: ComponentData,
    ) -> Index {
        let new_component = Component {
            id: self.counter,
            component_type: component_type,
            component_data: data,
        };
        self.components.push(new_component);
        self.counter += 1;
        (self.components.len() - 1) as Index
    }

    /// Adds a [`Component`] to the specified [`Entity`].
    ///
    /// [`Component`]: struct.Component.html
    /// [`Entity`]: ../entity/struct.Entity.html
    pub fn add_entity_component(
        &mut self,
        entity: Index,
        entity_manager: &mut EntityManager,
        index: Index,
    ) {
        entity_manager.get_entity_mut(entity).components.push(index);
    }

    /// Retrieves a reference to the [`Component`] with the given `id`.
    ///
    /// [`Component`]: struct.Component.html
    pub fn get_component(&self, id: Index) -> &Component {
        &self.components[id as usize]
    }

    /// Retrieves a mutable reference to the `Component` with the given `id`.
    ///
    /// [`Component`]: struct.Component.html
    pub fn get_component_mut(&mut self, index: Index) -> &mut Component {
        &mut self.components[index as usize]
    }

    /// Get `id`s for all [`Component`]s belonging to the given [`Entity`] that are of the specified type.
    ///
    /// [`Component`]: struct.Component.html
    /// [`Entity`]: ../entity/struct.Entity.html
    pub fn get_entity_components_of_type(
        &self,
        entity_index: Index,
        entity_manager: &EntityManager,
        component_type: &ComponentType,
    ) -> Vec<Index> {
        let entity = entity_manager.get_entity(entity_index);
        entity
            .components
            .iter()
            .filter(|index| self.components[**index as usize].component_type == *component_type)
            .cloned()
            .collect()
    }

    /// Get `id`s for all existing [`Component`]s of the given type.
    ///
    /// [`Component`]: struct.Component.html
    pub fn get_components_of_type(&self, component_type: ComponentType) -> Vec<Index> {
        self.components
            .iter()
            .filter(|component| component.component_type == component_type)
            .map(|component| component.id)
            .collect()
    }

    /// Removes a [`Component`] from an [`Entity`].
    ///
    /// [`Component`]: struct.Component.html
    /// [`Entity`]: ../entity/struct.Entity.html
    pub fn remove_entity_component(
        &self,
        entity_manager: &mut EntityManager,
        entity_index: Index,
        component_index: Index,
    ) -> Result<(), Error> {
        let mut index = 0;
        let components: &mut Vec<Index> =
            &mut entity_manager.get_entity_mut(entity_index).components;
        while index < components.len() {
            if components[index] == component_index {
                components.remove(index);
                return Ok(());
            }
            index += 1;
        }
        Err(Error::Index())
    }
}

pub trait ReceiveInput {
    fn keyboard_response(&mut self, keycode: VirtualKeyCode) -> Result<(), Error>;
}

impl ReceiveInput for Component {
    fn keyboard_response(&mut self, keycode: VirtualKeyCode) -> Result<(), Error> {
        if let ComponentData::Animation2D {
            movement,
            transform_data,
            ..
        } = &mut self.component_data
        {
            match keycode {
                VirtualKeyCode::Right => {
                    transform_data.translation[0] += movement.delta_translate[0];
                    transform_data.translation[1] += movement.delta_translate[1];
                    transform_data.rotation += movement.delta_rotation;
                    transform_data.scaling[0] += movement.delta_scale[0];
                    transform_data.scaling[1] += movement.delta_scale[1];
                    return Ok(());
                }
                VirtualKeyCode::Left => {
                    transform_data.translation[0] -= movement.delta_translate[0];
                    transform_data.translation[1] -= movement.delta_translate[1];
                    transform_data.rotation -= movement.delta_rotation;
                    transform_data.scaling[0] -= movement.delta_scale[0];
                    transform_data.scaling[1] -= movement.delta_scale[1];
                    return Ok(());
                }
                _ => {
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}

impl ReceiveInput for ComponentManager {
    fn keyboard_response(&mut self, keycode: VirtualKeyCode) -> Result<(), Error> {
        for component in &mut self.components {
            component.keyboard_response(keycode).unwrap();
        }
        Ok(())
    }
}
