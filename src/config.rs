// config.rs
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

//! Stuff for (de-)serializing global game configuration values.

use serde::{Deserialize, Serialize};
use std::fs;

use crate::error::Error;
use crate::serial::Size;

#[derive(Deserialize, Serialize)]
/// Global game configuration.
pub struct Configuration {
    pub graphics: Graphics,
}

#[derive(Deserialize, Serialize)]
/// Graphics-related configuration.
pub struct Graphics {
    pub window: Window,
}

#[derive(Deserialize, Serialize)]
pub struct Window {
    /// Window size in pixels.
    pub size: Size,
}

impl Configuration {
    /// Reads game configuration data from `./config.yaml`.
    pub fn new() -> Result<Self, Error> {
        Ok(serde_yaml::from_str(&fs::read_to_string("./config.yaml")?)?)
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            graphics: Graphics {
                window: Window {
                    size: Size {
                        x: 1024.0,
                        y: 768.0,
                    },
                },
            },
        }
    }
}
