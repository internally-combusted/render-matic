// texture.rs
// Creation and handling of images and textures.
// (c) 2019, Ryan McGowan <ryan@internally-combusted.net>

//! Loading and management of textures.

use gfx_backend_metal as backend;
use nalgebra_glm as glm;

use gfx_hal::{
    command::{BufferImageCopy, CommandBuffer, OneShot},
    format::{Aspects, Format},
    image::{Access, Extent, Layout, Offset, SubresourceLayers, SubresourceRange, Usage},
    memory::{Barrier, Dependencies},
    pso::PipelineStage,
    Backend as GfxBackend, Device, Graphics, Limits,
};
use image::{Rgba, RgbaImage};
use nalgebra_glm::Mat3;
use serde::Deserialize;
use std::{mem, ops::Range};

use self::backend::Backend;

use crate::{
    error::Error,
    render::BufferObject,
    resource::ResourceManager,
    serial::{Filename, Index, Size},
};

// Calculates the total memory size of all the textures given.
// TODO: I probably don't need this.
pub fn total_texture_size(
    textures: &[Index],
    resource_manager: &ResourceManager,
    limits: Limits,
) -> u64 {
    textures
        .iter()
        .map(|texture| {
            Texture::image_data_size(
                resource_manager.textures[*texture].get_data().unwrap(),
                &limits,
            )
        })
        .sum()
}

// Just to make `serde` stop crying when deserializing `Texture`s.
fn default_range() -> Range<usize> {
    0..0
}

/// Owns texture data and handles Vulkan-related constructs like
/// `Image`s and `ImageView`s.
#[derive(Debug, Deserialize)]
pub struct Texture {
    pub index: Index,

    /// The size in texels.
    pub size: Size,
    pub file: Filename,

    /// When this `Texture` is bound to buffer memory, this stores the range of bytes within
    /// the buffer that this `Texture` occupies.
    #[serde(default = "default_range", skip)]
    pub buffer_memory_range: Range<usize>,

    /// The [`ImageView`] for the pipeline to use.
    #[serde(skip)]
    pub image_view: Option<<Backend as GfxBackend>::ImageView>,

    /// The actual image data.
    #[serde(skip)]
    pub data: Option<RgbaImage>,

    /// This `Texture` as a Vulkan object.
    #[serde(skip)]
    pub image: Option<<Backend as GfxBackend>::Image>,

    /// A matrix precalculated based on the `Texture`'s size to scale
    /// all (u, v) coordinates to be in the [0.0, 1.0] range.
    #[serde(default = "glm::identity", skip)]
    pub normalization_matrix: Mat3,

    /// This `Texture`'s `DescriptorSet`.
    /// TODO: Will probably need to rework how the pipeline handles textures.
    #[serde(skip)]
    pub descriptor_set: Option<<Backend as GfxBackend>::DescriptorSet>,
}

impl Texture {
    /// Creates a new `Texture` and copies the texture data to buffer.
    pub unsafe fn new(
        index: Index,
        device: &backend::Device,
        limits: &Limits,
        texture_data: RgbaImage,
        buffer_memory: &mut <Backend as GfxBackend>::Memory,
        buffer_memory_offset: u64,
    ) -> Result<Texture, Error> {
        // Create Image.
        let image = device.create_image(
            gfx_hal::image::Kind::D2(texture_data.width(), texture_data.height(), 1, 1),
            1,
            Format::Rgba8Srgb,
            gfx_hal::image::Tiling::Optimal,
            Usage::TRANSFER_DST | Usage::SAMPLED,
            gfx_hal::image::ViewCapabilities::empty(),
        )?;

        // Copy texture data to the given buffer.
        let memory_requirement = device.get_image_requirements(&image);
        Texture::write_image_to_buffer(
            device,
            buffer_memory,
            buffer_memory_offset..memory_requirement.size + buffer_memory_offset,
            &texture_data,
            limits,
        )?;

        Ok(Texture {
            index,
            file: "".to_string(),
            size: Size {
                x: texture_data.width() as f32,
                y: texture_data.height() as f32,
            },
            normalization_matrix: glm::scaling2d(&glm::vec2(
                1.0 / texture_data.width() as f32,
                1.0 / texture_data.height() as f32,
            )),
            data: Some(texture_data),
            descriptor_set: None,
            image: Some(image),
            image_view: None,
            buffer_memory_range: buffer_memory_offset as usize
                ..(memory_requirement.size + buffer_memory_offset) as usize,
        })
    }

    /// Loads texture data from file and creates the `Texture`'s `Image`.
    pub fn initialize(
        &mut self,
        device: &<Backend as GfxBackend>::Device,
        color_format: Format,
    ) -> Result<(), Error> {
        let data = image::open(&self.file)?.to_rgba();
        let image = unsafe {
            device.create_image(
                gfx_hal::image::Kind::D2(data.width(), data.height(), 1, 1),
                1,
                color_format,
                gfx_hal::image::Tiling::Optimal,
                Usage::TRANSFER_DST | Usage::SAMPLED,
                gfx_hal::image::ViewCapabilities::empty(),
            )?
        };

        // Creates the uv normalization matrix for this texture.
        self.normalization_matrix = glm::scaling2d(&glm::vec2(
            1.0 / data.width() as f32,
            1.0 / data.height() as f32,
        ));
        self.image = Some(image);
        self.data = Some(data);
        Ok(())
    }

    /// Copies the `Texture` data to the given buffer memory.
    pub unsafe fn buffer_data(
        &mut self,
        device: &<Backend as GfxBackend>::Device,
        buffer_memory: &<Backend as GfxBackend>::Memory,
        buffer_memory_offset: u64,
        limits: &Limits,
    ) -> Result<(), Error> {
        let memory_requirement = device.get_image_requirements(self.get_image()?);
        self.buffer_memory_range = buffer_memory_offset as usize
            ..(memory_requirement.size + buffer_memory_offset) as usize;

        Self::write_image_to_buffer(
            device,
            buffer_memory,
            self.buffer_memory_range.start as u64..self.buffer_memory_range.end as u64,
            self.get_data()?,
            limits,
        )?;

        Ok(())
    }

    /// Finds the memory size needed for the given texture.
    pub fn image_data_size(texture: &RgbaImage, limits: &Limits) -> u64 {
        let pixel_size = mem::size_of::<Rgba<u8>>() as u32;
        let row_size = pixel_size * texture.width();

        // TODO: investigate the wizardry involved in the next two lines.
        let row_alignment_mask = limits.min_buffer_copy_pitch_alignment as u32 - 1;
        let row_pitch = (row_size + row_alignment_mask) & !row_alignment_mask;
        u64::from(row_pitch * texture.height())
    }

    /// Given the location of this Texture's image data in the image memory,
    /// binds the given memory and copies the data into the Image itself, then
    /// creates the ImageView.
    #[allow(clippy::too_many_arguments)] // CLIPPY HUSH
    pub unsafe fn copy_image_to_memory(
        &mut self,
        device: &<Backend as GfxBackend>::Device,
        image_memory: &<Backend as GfxBackend>::Memory,
        image_memory_offset: u64,
        command_pool: &mut gfx_hal::CommandPool<Backend, Graphics>,
        command_queue: &mut gfx_hal::CommandQueue<Backend, Graphics>,
        staging_buffer: &BufferObject,
        limits: &Limits,
    ) -> Result<(), Error> {
        device.bind_image_memory(
            &image_memory,
            image_memory_offset,
            &mut self.image.as_mut().unwrap(),
        )?;

        // Creating an Image is basically like drawing a regular frame except
        // the data gets rendered to memory instead of the screen, so we go
        // through the whole process of creating a command buffer, adding commands,
        // and submitting.
        let mut command_buffer = command_pool.acquire_command_buffer::<OneShot>();
        command_buffer.begin();

        // Set the Image to write mode.
        Texture::reformat_image(
            &mut command_buffer,
            (Access::empty(), Layout::Undefined),
            (Access::TRANSFER_WRITE, Layout::TransferDstOptimal),
            self.get_image()?,
            PipelineStage::TOP_OF_PIPE,
            PipelineStage::TRANSFER,
        );

        // Figure out the size of the texture data.
        let pixel_size = mem::size_of::<Rgba<u8>>() as u32;
        let row_size = pixel_size * self.size.x as u32;
        let row_alignment_mask = limits.min_buffer_copy_pitch_alignment as u32 - 1;
        let row_pitch = (row_size + row_alignment_mask) & !row_alignment_mask;

        // Copy the data.
        command_buffer.copy_buffer_to_image(
            &staging_buffer.buffer,
            self.get_image()?,
            Layout::TransferDstOptimal,
            &[BufferImageCopy {
                buffer_offset: (self.buffer_memory_range.start - staging_buffer.offset) as u64,
                buffer_width: (row_pitch / pixel_size) as u32,
                buffer_height: self.size.y as u32,
                image_layers: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                image_offset: Offset { x: 0, y: 0, z: 0 },
                image_extent: Extent {
                    width: self.size.x as u32,
                    height: self.size.y as u32,
                    depth: 1,
                },
            }],
        );

        // Set Image to read mode.
        Texture::reformat_image(
            &mut command_buffer,
            (Access::TRANSFER_WRITE, Layout::TransferDstOptimal),
            (Access::SHADER_READ, Layout::ShaderReadOnlyOptimal),
            self.get_image()?,
            PipelineStage::TRANSFER,
            PipelineStage::FRAGMENT_SHADER,
        );

        // Synchronize and then perform the rendering.
        command_buffer.finish();
        let upload_fence = device.create_fence(false)?;
        command_queue.submit_nosemaphores(Some(&command_buffer), Some(&upload_fence));
        device.wait_for_fence(&upload_fence, core::u64::MAX)?;
        device.destroy_fence(upload_fence);

        command_pool.free(Some(command_buffer));

        // Create the ImageView.
        self.image_view = Some(device.create_image_view(
            self.get_image()?,
            gfx_hal::image::ViewKind::D2,
            // Changing this to match the renderer's surface_color_format does funky things
            // TODO: Investigate why this happens
            Format::Rgba8Srgb,
            gfx_hal::format::Swizzle::NO,
            SubresourceRange {
                aspects: Aspects::COLOR,
                levels: 0..1,
                layers: 0..1,
            },
        )?);

        Ok(())
    }

    // Extracted from copy_image_to_memory to clean it up a bit.
    /// Switches an Image to the given state/format, handling the synchronization
    /// involved.
    fn reformat_image(
        command_buffer: &mut CommandBuffer<Backend, Graphics>,
        source_format: (Access, Layout),
        target_format: (Access, Layout),
        resource: &<Backend as GfxBackend>::Image,
        source_pipeline_stage: PipelineStage,
        target_pipeline_stage: PipelineStage,
    ) {
        let image_barrier = Barrier::Image {
            states: source_format..target_format,
            target: resource,
            families: None,
            range: SubresourceRange {
                aspects: Aspects::COLOR,
                levels: 0..1,
                layers: 0..1,
            },
        };

        unsafe {
            command_buffer.pipeline_barrier(
                source_pipeline_stage..target_pipeline_stage,
                Dependencies::empty(),
                &[image_barrier],
            )
        };
    }

    /// Copies an `RgbaImage` containing texture data to the specified buffer.
    unsafe fn write_image_to_buffer(
        device: &backend::Device,
        buffer_memory: &<Backend as GfxBackend>::Memory,
        data_range: Range<u64>,
        image: &RgbaImage,
        limits: &Limits,
    ) -> Result<(), Error> {
        let pixel_size = mem::size_of::<Rgba<u8>>() as u32;
        assert_eq!(pixel_size, 32 / 8);

        // Calculate image size.
        // TODO: Not sure why I have a function to do this but then write it out twice.
        let row_size = pixel_size * image.width();
        let row_alignment_mask = limits.min_buffer_copy_pitch_alignment as u32 - 1;
        let row_pitch = (row_size + row_alignment_mask) & !row_alignment_mask; // what wizardry is this

        let mut writer = device.acquire_mapping_writer::<u8>(buffer_memory, data_range)?;

        // Write the data row by row.
        for row in 0..image.height() {
            let image_offset = (row * row_size) as usize;
            let data = &(**image)[image_offset..(row_size as usize + image_offset)];
            let completed_row_size = (row * row_pitch) as usize;
            writer[completed_row_size..(data.len() + completed_row_size)].copy_from_slice(data);
        }
        device.release_mapping_writer(writer)?;

        Ok(())
    }

    /// A method for getting the `image` field because `unwrap()` unhelpfully moves
    /// instead of borrowing.
    pub fn get_image(&self) -> Result<&<Backend as GfxBackend>::Image, Error> {
        match &self.image {
            Some(image) => Ok(image),
            None => Err(Error::None()),
        }
    }

    /// A method for getting the `image_view` field because `unwrap()` unhelpfully moves
    /// instead of borrowing.
    pub fn get_image_view(&self) -> Result<&<Backend as GfxBackend>::ImageView, Error> {
        match &self.image_view {
            Some(image_view) => Ok(image_view),
            None => Err(Error::None()),
        }
    }

    /// A method for getting the `data` field because `unwrap()` unhelpfully moves instead
    /// of borrowing.
    pub fn get_data(&self) -> Result<&RgbaImage, Error> {
        match &self.data {
            Some(data) => Ok(data),
            None => Err(Error::None()),
        }
    }

    /// Releases resources held by this object.
    pub unsafe fn destroy(self, device: &backend::Device) {
        device.destroy_image(self.image.unwrap());
        device.destroy_image_view(self.image_view.unwrap());
    }
}
