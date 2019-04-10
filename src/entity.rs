// entity.rs
// Managing `entities`, which are just containers for `components`, individual bits of game functionality.
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

use log::debug;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::{error::Error, serial::Index};

/// An entity is just a bundle of components.
#[derive(Clone, Deserialize, Serialize)]
pub struct Entity {
    pub id: Index,
    pub entity_type: EntityType,
    pub components: Vec<Index>,
}

#[derive(Clone, PartialEq, Deserialize, Eq, Hash, Serialize)]
pub enum EntityType {
    Background = 0,
    Sprite = 1,
}

impl Entity {
    pub fn new(id: Index, entity_type: EntityType) -> Self {
        Entity {
            id,
            entity_type: entity_type,
            components: Vec::new(),
        }
    }
}

/// The owner for all entities.
/// Handles creation, access, and deletion.
#[derive(Deserialize)]
pub struct EntityManager {
    pub counter: Index,
    pub entities: Vec<Entity>,
}

impl EntityManager {
    pub fn load_entities() -> Result<EntityManager, Error> {
        debug!("Loading entities...");
        Ok(serde_yaml::from_str(&fs::read_to_string(
            "./data/entities.yaml",
        )?)?)
    }

    pub fn create_entity(&mut self, entity_type: EntityType) -> Index {
        let new_entity = Entity {
            id: self.counter,
            entity_type,
            components: Vec::new(),
        };

        // len() is the number of elements in the vec, not the allocated size
        // thus push() always places items into vec[vec.len() - 1]
        self.entities.push(new_entity);
        self.counter += 1;
        (self.entities.len() - 1) as Index
    }

    pub fn get_entities_of_type(&self, requested_type: EntityType) -> Vec<Index> {
        self.entities
            .iter()
            .filter(|entity| entity.entity_type == requested_type)
            .map(|entity| entity.id)
            .collect()
    }

    pub fn get_entity(&self, index: Index) -> &Entity {
        &self.entities[index as usize]
    }

    pub fn get_entity_mut(&mut self, index: Index) -> &mut Entity {
        &mut self.entities[index as usize]
    }
}
