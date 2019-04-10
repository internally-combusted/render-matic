// render.rs
// Rendering things so they appear on a screen.
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

//! A rendering system using the [`gfx_hal`](https://docs.rs/gfx-hal/0.1.0/gfx_hal/) crate.

use gfx_hal::image as gfx_image;
use gfx_hal::{
    adapter::{MemoryType, MemoryTypeId},
    buffer::{IndexBufferView, Usage},
    command::{ClearColor, ClearValue, MultiShot},
    format::{Aspects, ChannelType, Format, Swizzle},
    image::{Access, Layout, SubresourceRange, ViewKind},
    memory::{Properties, Requirements},
    pass::{
        Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp, SubpassDependency,
        SubpassDesc, SubpassRef,
    },
    pool::{CommandPool, CommandPoolCreateFlags},
    pso::{PipelineStage, Rect},
    queue::Submission,
    window::{CompositeAlpha, PresentMode, Surface},
    Backbuffer, Backend as GfxBackend, Device, FrameSync, Graphics, IndexType, Instance,
    PhysicalDevice, QueueGroup, SwapImageIndex, Swapchain, SwapchainConfig,
};
use std::ops::Range;
use winit::{dpi::PhysicalSize, Window};

use crate::{
    error::Error,
    geometry,
    pipeline::PipelineData,
    resource::ResourceManager,
    serial::{Color, Index, Position2D, Position3D},
    texture,
};

use self::backend::Backend;
use gfx_backend_metal as backend;

/// Represents a buffer and all of the data and logic surrounding its
/// creation and handling.
#[derive(Debug)]
pub struct BufferObject {
    pub buffer: <Backend as GfxBackend>::Buffer,
    pub requirements: Requirements,
    /// The [`Buffer`]'s offset into its associated allocated memory.
    ///
    /// [`Buffer`] struct.Buffer.html
    pub offset: usize,
}

impl BufferObject {
    pub unsafe fn new(device: &backend::Device, buffer_length: u64, usage: Usage) -> BufferObject {
        let buffer = match device.create_buffer(buffer_length, usage) {
            Ok(buffer) => buffer,
            Err(err) => panic!(err),
        };

        let requirements = device.get_buffer_requirements(&buffer);

        BufferObject {
            buffer,
            requirements,
            offset: 0,
        }
    }

    /// Finds a suitable [`MemoryType`] for the given set of `BufferObject`s,
    /// allocates a single chunk of memory to contain all of them, sets their
    /// buffer offsets, and then binds memory to them.
    pub unsafe fn allocate_buffers(
        device: &backend::Device,
        buffers: &mut [&mut Self],
        memory_types: &[MemoryType],
    ) -> Result<<Backend as GfxBackend>::Memory, Error> {
        // Find a type of memory that will work for all the given buffers.
        let total_memory: u64 = buffers
            .iter()
            .map(|buffer_object| buffer_object.requirements.size)
            .sum();

        let requirements = buffers
            .iter()
            .map(|buffer_object| device.get_buffer_requirements(&buffer_object.buffer))
            .collect::<Vec<Requirements>>();
        let memory_type =
            match select_memory_type(&requirements, memory_types, Properties::CPU_VISIBLE) {
                Some(memory_type) => memory_type,
                None => {
                    return Err(Error::NoSuitableMemory());
                }
            };

        // Allocate memory.
        let memory = device.allocate_memory(memory_type, total_memory)?;

        // Assign a buffer offset to each buffer.
        let mut sum: u64 = 0;
        for buffer_object in buffers {
            buffer_object.offset = sum as usize;
            device.bind_buffer_memory(&memory, sum, &mut buffer_object.buffer)?;
            sum += buffer_object.requirements.size;
        }
        assert_eq!(sum, total_memory);
        Ok(memory)
    }

    pub unsafe fn copy_data_to_buffer<T: Copy>(
        &self,
        device: &backend::Device,
        memory: &mut <Backend as GfxBackend>::Memory,
        data: &[T],
    ) {
        let mut writer = device
            .acquire_mapping_writer(
                &memory,
                self.offset as u64..self.offset as u64 + self.requirements.size,
            )
            .expect("Couldn't acquire writer for buffer!");
        writer[..data.len()].copy_from_slice(data);
        device
            .release_mapping_writer(writer)
            .expect("Couldn't release writer for buffer!");
    }
}

/// Given a set of memory requirements, finds and returns a type that matches all requirements,
/// or None.
pub unsafe fn select_memory_type(
    requirements: &[Requirements],
    memory_types: &[MemoryType],
    properties: Properties,
) -> Option<MemoryTypeId> {
    let mask = requirements
        .iter()
        .fold(u64::max_value(), |combined_mask, requirement| {
            combined_mask & requirement.type_mask
        });
    memory_types
        .iter()
        .enumerate()
        .find(|&(id, memory_type)| {
            mask & (1 << id) != 0 && memory_type.properties.contains(properties)
        })
        .map(|(id, _)| MemoryTypeId(id))
}

/// Contains data for a vertex containing only the elements that will be copied to a vertex buffer.
///
/// The texture_index in [`VertexData`] isn't used for drawing, so this omits that element.
#[derive(Clone, Copy, Debug)]
pub struct FormattedVertexData {
    pub position: Position3D,
    pub color: Color,
    pub uv: Position2D,
}

/// Receives graphical data and draws it to the screen.
pub struct Renderer {
    device: backend::Device,
    queue_group: QueueGroup<Backend, Graphics>,
    framebuffers: Vec<<Backend as GfxBackend>::Framebuffer>,
    frame_images: Vec<(
        <Backend as GfxBackend>::Image,
        <Backend as GfxBackend>::ImageView,
    )>,
    render_pass: <Backend as GfxBackend>::RenderPass,
    swapchain: <Backend as GfxBackend>::Swapchain,

    // Data to send to the shaders.
    vertex_buffer: BufferObject,
    index_buffer: BufferObject,
    texture_staging_buffer: BufferObject,
    buffer_memory: <Backend as GfxBackend>::Memory,
    image_memory: <Backend as GfxBackend>::Memory,

    pipeline_data: PipelineData,

    command_pool: CommandPool<Backend, Graphics>,

    // Synchronization.
    frame_semaphore: <Backend as GfxBackend>::Semaphore,
    present_semaphore: <Backend as GfxBackend>::Semaphore,

    view_rect: Rect,
    pub physical_size: PhysicalSize,
    pub color_format: Format,
}

impl Renderer {
    // Determines the size of the vertex and index buffers.
    const MAX_QUADS: u64 = 10;

    /// Creates a new renderer for the given window and sets up a pipeline.
    pub fn new(
        name: &str,
        window: &Window,
        resource_manager: &mut ResourceManager,
    ) -> Result<Self, Error> {
        let physical_size = match window.get_inner_size() {
            // The window's size in pixels.
            Some(size) => size.to_physical(1.0),
            None => panic!("Couldn't get window size; window no longer exists!"),
        };

        let instance = backend::Instance::create(name, 1);
        let mut surface = instance.create_surface(window);
        let adapter = instance.enumerate_adapters().remove(0);

        let memory_types = adapter.physical_device.memory_properties().memory_types;
        let limits = adapter.physical_device.limits();

        let (device, mut queue_group) = adapter
            .open_with::<_, Graphics>(1, |family| surface.supports_queue_family(family))
            .unwrap();

        let mut command_pool = unsafe {
            device.create_command_pool_typed(&queue_group, CommandPoolCreateFlags::empty())
        }
        .unwrap();

        // Create vertex buffers.
        let mut vertex_buffer = unsafe {
            BufferObject::new(
                &device,
                (Renderer::MAX_QUADS as usize
                    * geometry::QUAD_VERTICES.len()
                    * std::mem::size_of::<FormattedVertexData>()) as u64,
                Usage::VERTEX,
            )
        };

        // Create index buffers.
        let mut index_buffer = unsafe {
            BufferObject::new(
                &device,
                (Renderer::MAX_QUADS as usize
                    * geometry::QUAD_INDICES.len()
                    * std::mem::size_of::<u16>()) as u64,
                Usage::INDEX,
            )
        };

        let physical_device = &adapter.physical_device;
        let (caps, formats, _, _) = surface.compatibility(physical_device);

        // Choose a color format. TODO: Ensure this is consistent systemwide.
        let surface_color_format = {
            match formats {
                Some(choices) => choices
                    .into_iter()
                    .find(|format| format.base_format().1 == ChannelType::Srgb)
                    .unwrap(),
                None => Format::Rgba8Srgb,
            }
        };

        // Load textures from disk and create [`Image`]s.
        for texture in &mut resource_manager.textures {
            texture.initialize(&device, surface_color_format)?;
        }

        // Create texture staging buffer.
        let mut texture_staging_buffer = unsafe {
            BufferObject::new(
                &device,
                texture::total_texture_size(
                    &resource_manager
                        .textures
                        .iter()
                        .map(|texture| texture.index)
                        .collect::<Vec<Index>>(),
                    resource_manager,
                    limits,
                ),
                Usage::TRANSFER_SRC,
            )
        };

        // Create a single memory allocation for all buffers.
        let buffer_memory = unsafe {
            BufferObject::allocate_buffers(
                &device,
                &mut [
                    &mut vertex_buffer,
                    &mut index_buffer,
                    &mut texture_staging_buffer,
                ],
                &memory_types,
            )?
        };

        // Copy texture data to buffer.
        let mut buffer_memory_offset = texture_staging_buffer.offset;
        for texture in &mut resource_manager.textures {
            unsafe {
                texture.buffer_data(
                    &device,
                    &buffer_memory,
                    buffer_memory_offset as u64,
                    &limits,
                )?;

                buffer_memory_offset = texture.buffer_memory_range.end;
            }
        }

        // Find memory for the textures.
        let image_requirements = unsafe {
            resource_manager
                .textures
                .iter()
                .map(|texture| device.get_image_requirements(texture.get_image().unwrap()))
                .collect::<Vec<Requirements>>()
        };
        let image_memory_type = unsafe {
            match select_memory_type(&image_requirements, &memory_types, Properties::DEVICE_LOCAL) {
                Some(memory_type) => memory_type,
                None => {
                    return Err(Error::NoSuitableMemory());
                }
            }
        };

        // Allocate image memory.
        let required_image_memory = image_requirements
            .iter()
            .map(|requirement| requirement.size)
            .sum();
        let image_memory =
            unsafe { device.allocate_memory(image_memory_type, required_image_memory)? };

        // Load textures into GPU memory.
        let mut image_memory_offset = 0;
        unsafe {
            for texture in &mut resource_manager.textures {
                texture.copy_image_to_memory(
                    &device,
                    &image_memory,
                    image_memory_offset,
                    &mut command_pool,
                    &mut queue_group.queues[0],
                    &texture_staging_buffer,
                    &limits,
                )?;

                image_memory_offset += image_requirements[texture.index].size as u64;
            }
        }

        // Create a render pass.
        let render_pass = {
            let color_attachment = Attachment {
                format: Some(surface_color_format),
                samples: 1,
                ops: AttachmentOps::new(AttachmentLoadOp::Clear, AttachmentStoreOp::Store),
                stencil_ops: AttachmentOps::DONT_CARE,
                layouts: Layout::Undefined..Layout::Present,
            };

            // Only one subpass, so there's no data as input from/preserve for previous
            // or subsequent subpasses. No idea what resolves are...
            let subpass = SubpassDesc {
                colors: &[(0, Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            let dependency = SubpassDependency {
                passes: SubpassRef::External..SubpassRef::Pass(0),
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT
                    ..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                accesses: Access::empty()
                    ..(Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE),
            };

            unsafe { device.create_render_pass(&[color_attachment], &[subpass], &[dependency]) }
        }
        .unwrap();

        // Set up the view rectangle.
        let extent = caps.current_extent.unwrap();
        let view_rect = Rect {
            x: 0,
            y: 0,
            w: extent.width as i16,
            h: extent.height as i16,
        };

        // Build a [`PipelineData`] object for descriptor stuff.
        let pipeline_data = unsafe {
            PipelineData::new(
                &device,
                &caps,
                &resource_manager.textures,
                surface_color_format,
            )?
        };

        // Set up a swapchain.
        let swap_config = SwapchainConfig {
            present_mode: PresentMode::Fifo,
            composite_alpha: CompositeAlpha::Inherit,
            format: surface_color_format,
            extent,
            image_count: std::cmp::max(
                2,
                std::cmp::min(caps.image_count.start + 1, caps.image_count.end),
            ),
            image_layers: 1,
            image_usage: gfx_image::Usage::COLOR_ATTACHMENT,
        };

        let extent = swap_config.extent.to_extent();

        let (swapchain, backbuffer) = unsafe {
            device
                .create_swapchain(&mut surface, swap_config, None)
                .unwrap()
        };

        // Set up framebuffers.
        let (frame_images, framebuffers) = match backbuffer {
            Backbuffer::Images(images) => {
                let pairs = images
                    .into_iter()
                    .map(|image| unsafe {
                        let rtv = device
                            .create_image_view(
                                &image,
                                ViewKind::D2,
                                surface_color_format,
                                Swizzle::NO,
                                SubresourceRange {
                                    aspects: Aspects::COLOR,
                                    levels: 0..1,
                                    layers: 0..1,
                                },
                            )
                            .unwrap();
                        (image, rtv)
                    })
                    .collect::<Vec<_>>();
                let fbos = pairs
                    .iter()
                    .map(|&(_, ref rtv)| unsafe {
                        device
                            .create_framebuffer(&render_pass, Some(rtv), extent)
                            .unwrap()
                    })
                    .collect();
                (pairs, fbos)
            }
            Backbuffer::Framebuffer(fbo) => (Vec::new(), vec![fbo]),
        };

        // Create synchronization primitives.
        let frame_semaphore = device.create_semaphore().unwrap();
        let present_semaphore = device.create_semaphore().unwrap();

        // Finally, return a new Renderer.
        Ok(Renderer {
            color_format: surface_color_format,
            command_pool,
            device,
            frame_images,
            frame_semaphore,
            framebuffers,
            queue_group,
            present_semaphore,
            render_pass,
            swapchain,
            vertex_buffer,
            index_buffer,
            texture_staging_buffer,
            buffer_memory,
            image_memory,
            view_rect,
            pipeline_data,
            physical_size,
        })
    }

    ///  Renders a frame using the data provided by the given `Component`s.
    pub fn render_frame(
        &mut self,
        vertex_data: Vec<FormattedVertexData>,
        index_ranges: Vec<Range<u32>>,
    ) {
        unsafe {
            self.command_pool.reset();
        }

        let frame_index: SwapImageIndex = unsafe {
            match self
                .swapchain
                .acquire_image(!0, FrameSync::Semaphore(&self.frame_semaphore))
            {
                Ok(index) => index,
                Err(err) => panic!(err),
            }
        };

        unsafe {
            self.vertex_buffer.copy_data_to_buffer(
                &self.device,
                &mut self.buffer_memory,
                &vertex_data,
            );
        }

        // Calculate indices.
        let quads = vertex_data.len() / geometry::QUAD_VERTICES.len();
        let indices = quads * geometry::QUAD_INDICES.len();
        let mut index_data: Vec<u16> = vec![];
        index_data.resize(indices, 0);
        for quad in 0..quads {
            for index in 0..geometry::QUAD_INDICES.len() {
                index_data[quad * geometry::QUAD_INDICES.len() + index] =
                    geometry::QUAD_INDICES[index] + (geometry::QUAD_VERTICES.len() * quad) as u16;
            }
        }

        // Copy index data to buffer.
        unsafe {
            self.index_buffer.copy_data_to_buffer(
                &self.device,
                &mut self.buffer_memory,
                &index_data,
            );
        }

        // Start lining up instructions for the GPU.
        let finished_command_buffer = {
            let mut command_buffer = self.command_pool.acquire_command_buffer::<MultiShot>();

            unsafe {
                command_buffer.begin(false);
            }

            unsafe {
                command_buffer.bind_vertex_buffers(0, vec![(&self.vertex_buffer.buffer, 0)]);

                command_buffer.bind_index_buffer(IndexBufferView {
                    buffer: &self.index_buffer.buffer,
                    offset: 0,
                    index_type: IndexType::U16,
                });

                command_buffer.bind_graphics_pipeline(&self.pipeline_data.pipeline);
            }
            {
                // new scope to prevent compiler whining about borrowing lifetimes
                let mut encoder = unsafe {
                    command_buffer.begin_render_pass_inline(
                        &self.render_pass,
                        &self.framebuffers[frame_index as usize],
                        self.view_rect,
                        &[ClearValue::Color(ClearColor::Float([0.0, 0.0, 0.0, 0.0]))],
                    )
                };

                for (group, range) in index_ranges.iter().enumerate() {
                    unsafe {
                        encoder.bind_graphics_descriptor_sets(
                            &self.pipeline_data.pipeline_layout,
                            0,
                            vec![
                                &self.pipeline_data.sampler_set,
                                &self.pipeline_data.texture_sets[group],
                            ],
                            &[],
                        );

                        {
                            encoder.draw_indexed(range.start..range.end, 0, 0..1);
                        }
                    }
                }
            }
            unsafe {
                command_buffer.finish();
            }

            command_buffer
        };

        // Submit the command queue and present the next frame.
        let submission = Submission {
            wait_semaphores: Some((&self.frame_semaphore, PipelineStage::BOTTOM_OF_PIPE)),
            signal_semaphores: Some(&self.present_semaphore),
            command_buffers: Some(&finished_command_buffer),
        };

        unsafe {
            self.queue_group.queues[0].submit(submission, None);

            match self.swapchain.present(
                &mut self.queue_group.queues[0],
                frame_index,
                vec![&self.present_semaphore],
            ) {
                Ok(()) => (),
                Err(()) => (),
            }
        }
    }

    /// Waits for executing command buffers to idle, then releases
    /// all renderer resources.
    ///
    /// # Errors
    ///
    /// May return [`Error::HostExecution`] if one is generated by the underlying
    /// hardware abstraction layer. The conditions for this are unknown, as there
    /// doesn't seem to be good documentation on this in the `gfx_hal` crate.
    ///
    /// [`Error::HostExecution`]: ../error/struct.Error.html#HostExecution
    pub fn clean_up(self) -> Result<(), Error> {
        self.queue_group.queues[0].wait_idle()?;

        unsafe {
            // Free allocated memory.
            self.device.free_memory(self.buffer_memory);
            self.device.free_memory(self.image_memory);

            // Destroy created objects.
            self.pipeline_data.destroy(&self.device);
            self.device.destroy_buffer(self.vertex_buffer.buffer);
            self.device.destroy_buffer(self.index_buffer.buffer);
            self.device
                .destroy_buffer(self.texture_staging_buffer.buffer);

            self.device.destroy_semaphore(self.frame_semaphore);
            self.device.destroy_semaphore(self.present_semaphore);

            for framebuffer in self.framebuffers {
                self.device.destroy_framebuffer(framebuffer);
            }

            for (_, image_view) in self.frame_images {
                self.device.destroy_image_view(image_view);
            }

            self.device.destroy_swapchain(self.swapchain);

            self.device
                .destroy_command_pool(self.command_pool.into_raw());

            Ok(())
        }
    }
}
