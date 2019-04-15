// main.rs
// Render-matic, aspiring to be an engine for 2D JRPG-style games when it grows up.
// Version 0.0.000000001
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

pub mod component;
pub mod config;
pub mod data;
pub mod draw;
pub mod entity;
pub mod error;
pub mod geometry;
pub mod pipeline;
pub mod render;
pub mod resource;
pub mod serial;
pub mod text;
pub mod texture;
pub mod time;

use winit::{
    dpi::LogicalSize, Event, EventsLoop, KeyboardInput, VirtualKeyCode, WindowBuilder, WindowEvent,
};

use crate::{
    component::ReceiveInput, config::Configuration, data::DataManager, draw::DrawingSystem,
    error::Error,
};

/*
    CREDITS

    THANKS TO:
        Lokathor, for the Rust specific tutorials:
            https://github.com/Lokathor/learn-gfx-hal
        Joey de Vries, for an expansive walkthrough of modern graphics programming:
            https://learnopengl.com
        The gfx-hal team, of course:
            https://github.com/gfx-rs/gfx
        Pawel L., creator of API without Secrets: Introduction to Vulkan
            https://software.intel.com/en-us/articles/api-without-secrets-introduction-to-vulkan-part-1
        Sascha Willems, who has a ton of Vulkan examples on his GitHub
            https://github.com/SaschaWillems/Vulkan/tree/master/examples
*/

/// Where it all begins.
fn main() -> Result<(), Error> {
    // Lets us actually read all the error messages `gfx_hal` throws out.
    simple_logger::init_with_level(log::Level::Debug).unwrap();

    // Load things from YAML files.
    let configuration = Configuration::new()?;
    let mut data = DataManager::new()?;

    let window_name = data.game_data.program.name.clone();

    // create window
    let mut event_loop = EventsLoop::new();

    let width: f64 = configuration.graphics.window.size.x.into();
    let height: f64 = configuration.graphics.window.size.y.into();

    let game_window = WindowBuilder::new()
        .with_title(window_name)
        .with_dimensions(LogicalSize::new(width, height))
        .build(&event_loop)?;

    let mut drawing_system = DrawingSystem::new(
        &game_window,
        &data.game_data.program.name,
        &mut data.resource_manager,
    )?;

    // Game loop.
    loop {
        let mut quitting = false;

        // If the window is closed, or Escape is pressed, quit
        event_loop.poll_events(|event| {
            if let Event::WindowEvent { event, .. } = event {
                match event {
                    WindowEvent::CloseRequested => quitting = true,
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => quitting = true,
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Right),
                                ..
                            },
                        ..
                    } => {
                        data.component_manager
                            .keyboard_response(VirtualKeyCode::Right)
                            .unwrap();
                    }
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Left),
                                ..
                            },
                        ..
                    } => {
                        data.component_manager
                            .keyboard_response(VirtualKeyCode::Left)
                            .unwrap();
                    }
                    _ => {}
                }
            }
        });

        if quitting {
            break;
        }

        drawing_system.draw_frame(&data);
    }

    drawing_system.clean_up()?;
    Ok(())
}
