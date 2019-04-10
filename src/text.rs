// text.rs
// Utilities for handling and rendering text.
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

use image::{Rgba, RgbaImage};
use rusttype::{Font, PositionedGlyph, Scale};
use serde::Deserialize;

use crate::{
    error::Error,
    serial::{Filename, Index},
};

// There are a few ways of getting text done.
//
// 1) Generate a static texture for each font. Then, drawing text is basically the
// same as using a texture atlas. The main issue here is deciding which glyphs to
// include in the static texture.
//
// It would be simplest to just have the texture contain a basic set like
// ASCII or the Basic Latin Unicode block, perhaps with some additional blocks
// like Latin Extended-A. This minimizes the size of each texture, but creates issues
// for localization, or if the user simply works in a language that requires more/different
// Unicode blocks.

// It would be utterly infeasible to pre-render every Unicode glyph to a texture, and
// even a single block of glyphs might be a problem if it's something like CJK Unified
// Ideographs.

// If the application's content is more or less static, it might be possible to compile
// a list of every glyph that will actually be used and to render only those to a texture.
// This could run into trouble if user input is unconstrained and people decide to do
// fun Unicode things like name their character ᏕᏋᎮᏂᎥᏒᎧᏖᏂ.
//
// 2) Render text to textures on the fly.
//
// A major problem here is the cost of spontaneously generating new textures. This
// may not be a problem if the application isn't too graphics/CPU heavy, and given
// that this is intended to be a 2D renderer, it might be okay.
//
// I've gone with this option for the sake of generality, though it may be necessary
// to reconsider down the road. As it is, the main problem to be resolved right now
// is the fact that this results in scenes potentially having a quite variable number
// of active textures for text at any given moment.

/// Renders the given static text component to a texture.
pub fn render_text(
    text: &str,
    font: &Font,
    color: [f32; 4],
    height: u32,
) -> Result<RgbaImage, Error> {
    let scale = Scale { x: 1.0, y: 1.0 };
    let v_metrics = font.v_metrics(scale);
    let offset = rusttype::point(0.0, v_metrics.ascent);

    // Lay out glyphs for this text.
    let glyphs = font
        .layout(&text, scale, offset)
        .collect::<Vec<PositionedGlyph>>();

    // Check that `layout` gave us at least one glyph.
    let end_glyph = match glyphs.last() {
        Some(glyph) => glyph,
        None => return Err(Error::None()),
    };

    // Width is last glyph's position + its width.
    let width = (end_glyph.position().x as f32 + end_glyph.unpositioned().h_metrics().advance_width)
        .ceil() as usize;

    // Use the given drawing color to paint the texture with each texel's alpha value
    // equal to the coverage value returned by the draw() call.
    let mut texture = RgbaImage::new(width as u32, height);
    let color_u32 = [
        (color[0] * 255.0) as u8,
        (color[1] * 255.0) as u8,
        (color[2] * 255.0) as u8,
        (color[3] * 255.0) as u8,
    ];
    for glyph in glyphs {
        glyph.draw(|x, y, a| {
            texture.put_pixel(
                x,
                y,
                Rgba {
                    data: [
                        color_u32[0],
                        color_u32[1],
                        color_u32[2],
                        (color[3] * a * 255.0) as u8,
                    ],
                },
            );
        })
    }
    Ok(texture)
}

#[derive(Deserialize)]
pub struct GameFont<'a> {
    pub index: Index,
    pub file: Filename,
    #[serde(skip)]
    pub data: Option<Font<'a>>,
}
