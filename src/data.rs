// data.rs
// Manages all game data.
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

//! Management of all game data, primarily data relating to game logic.
//!
//! Resources like art and fonts are sub-managed by [`ResourceManager`].
//!
//! [`ResourceManager`]: struct.ResourceManager.html

use log::debug;
use serde::Deserialize;
use std::fs;

use crate::{
    component::ComponentManager,
    draw::{Background, Spritesheet},
    entity::EntityManager,
    error::Error,
    resource::ResourceManager,
    serial::{Index, Size},
};

// YAML is used for (de-)serialization because the `serde-toml` crate seemed to have
// issues and YAML looks better than JSON imo. A human-readable format is good for
// debugging, but we can switch to something else later if we need to for some reason.

#[derive(Deserialize)]
/// Data for a single author.
pub struct Author {
    /// Author's name.
    pub name: String,
    /// (Optional) Author's email address.
    pub email: Option<String>,
}

#[derive(Deserialize)]
pub struct Program {
    /// The game's name.
    pub name: String,
    /// (Optional) The game version.
    pub version: Option<String>,
}

#[derive(Deserialize)]
/// A map containing tile and event data.
pub struct GameMap {
    pub index: Index,
    pub name: String,
    pub size: Size,
    pub entities: Vec<Index>,
}

#[derive(Deserialize)]
/// Metadata related to the game itself as a program.
pub struct GameData {
    /// Data for the author(s) of the game.
    pub authors: Vec<Author>,
    /// Data about the game.
    pub program: Program,
}

impl GameData {
    /// Creates the `GameData` object by loading from a configuration file.
    ///
    /// # Errors
    ///
    /// The data is expected to be contained in the file `./data/game_data.yaml`.
    /// If this file is absent or its data is malformed, [`Error::SerdeYaml`] will be
    /// returned.
    ///
    /// [`Error::SerdeYaml`]: ../error/enum.Error.html#variant.SerdeYaml
    pub fn new() -> Result<Self, Error> {
        debug!("Loading game data...");
        Ok(serde_yaml::from_str(&fs::read_to_string(
            "./data/game_data.yaml",
        )?)?)
    }
}

/// Owner for all game data.
pub struct DataManager<'a> {
    pub game_data: GameData,
    pub resource_manager: ResourceManager<'a>,
    pub component_manager: ComponentManager,
    pub entity_manager: EntityManager,
    pub backgrounds: Vec<Background>,
    pub maps: Vec<GameMap>,
    pub spritesheets: Vec<Spritesheet>,
}

impl<'a> DataManager<'a> {
    /// Creates a new `DataManager` by loading all relevant YAML files.
    ///
    /// Data is loaded from the following files:
    ///
    /// + [`Background`]s: `./data/backgrounds.yaml`
    /// + [`GameMap`]s: `./data/maps.yaml`
    /// + [`Spritesheet`]s: `./data/spritesheets.yaml`
    ///
    /// See the individual structs to see what fields they have and whether they're optional.
    ///
    /// Note that the YAML files don't contain resources like music or art directly, but give
    /// paths to the files, which are later loaded by the [`ResourceManager`].
    ///
    /// # Errors
    ///
    /// If any of the expected files are absent or malformed, [`Error::SerdeYaml`] will be returned.
    ///
    /// [`Background`]: ../draw/struct.Background.html
    /// [`Error::SerdeYaml`]: ../error/enum.Error.html#variant.SerdeYaml
    /// [`GameMap`]: struct.GameMap.html
    /// [`Spritesheet`]: ../draw/struct.Spritesheet.html
    pub fn new() -> Result<DataManager<'a>, Error> {
        debug!("Loading backgrounds, maps, and spritesheets...");
        let new_data = DataManager {
            game_data: GameData::new()?,
            resource_manager: ResourceManager::load_resources()?,
            component_manager: ComponentManager::load_components()?,
            entity_manager: EntityManager::load_entities()?,
            backgrounds: serde_yaml::from_str(&fs::read_to_string("./data/backgrounds.yaml")?)?,
            maps: serde_yaml::from_str(&fs::read_to_string("./data/maps.yaml")?)?,
            spritesheets: serde_yaml::from_str(&fs::read_to_string("./data/spritesheets.yaml")?)?,
        };

        Ok(new_data)
    }
}
