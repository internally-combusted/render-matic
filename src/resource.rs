// resource.rs
// Managing art, sound, and music resources.
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

//! Handling of media including fonts, music, and textures.

use gfx_hal::Backend as GfxBackend;
use log::debug;
use serde::Deserialize;
use std::fs;

use self::backend::Backend;
use gfx_backend_metal as backend;

use crate::{error::Error, text::GameFont, texture::Texture};

/// Central repository (but not direct owner) of media resources.
#[derive(Deserialize)]
pub struct ResourceManager<'a> {
    pub fonts: Vec<GameFont<'a>>,
    pub textures: Vec<Texture>,
}

impl<'a> ResourceManager<'a> {
    /// Acquires all resources specified in `./data/resources.yaml`.
    pub fn load_resources() -> Result<ResourceManager<'a>, Error> {
        debug!("Loading resources...");
        Ok(serde_yaml::from_str(&fs::read_to_string(
            "./data/resources.yaml",
        )?)?)
    }

    /// Releases all resources held by this object.
    pub unsafe fn clean_up(self, device: &mut <Backend as GfxBackend>::Device) {
        for texture in self.textures {
            texture.destroy(device);
        }
    }
}
