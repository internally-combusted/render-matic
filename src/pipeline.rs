// pipeline.rs
// Structs for creating and handling graphics pipelines.
// (c) 2019 Ryan McGowan <ryan@internally-combusted.net>

//! Creating and handling graphics pipelines.

use gfx_hal::{
    format::Format,
    image::{Access, Filter, Layout, SamplerInfo, WrapMode},
    pass::{
        Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp, Subpass, SubpassDependency,
        SubpassDesc, SubpassRef,
    },
    pso::{
        AttributeDesc, BakedStates, BlendState, ColorBlendDesc, ColorMask, Descriptor,
        DescriptorRangeDesc, DescriptorSetLayoutBinding, DescriptorSetWrite, DescriptorType,
        ElemStride, Element, EntryPoint, GraphicsPipelineDesc, GraphicsShaderSet, PipelineStage,
        Rasterizer, Rect, ShaderStageFlags, Specialization, VertexBufferDesc, Viewport,
    },
    window::SurfaceCapabilities,
    Backend as GfxBackend, DescriptorPool, Device, Primitive,
};

use crate::{error::Error, render::FormattedVertexData, texture::Texture};

use self::backend::Backend;
use gfx_backend_metal as backend;

#[derive(Debug)]
/// Holds all the data needed to create and use a pipeline.
pub struct PipelineData {
    pub pipeline_layout: <Backend as GfxBackend>::PipelineLayout,
    pub pipeline: <Backend as GfxBackend>::GraphicsPipeline,

    // It seems like one sampler can handle any number of textures as long as
    // only one sampler configuration is needed.
    pub sampler: <Backend as GfxBackend>::Sampler,

    // Pipeline is currently fixed to permit only sampler and texture `DescriptorSet`s.
    pub sampler_set: <Backend as GfxBackend>::DescriptorSet,
    pub texture_sets: Vec<<Backend as GfxBackend>::DescriptorSet>,
    sampler_layout: <Backend as GfxBackend>::DescriptorSetLayout,
    texture_layout: <Backend as GfxBackend>::DescriptorSetLayout,

    // Only one each of vertex and fragment shader is currently allowed.
    vertex_shader_module: <Backend as GfxBackend>::ShaderModule,
    fragment_shader_module: <Backend as GfxBackend>::ShaderModule,

    render_pass: <Backend as GfxBackend>::RenderPass,
    descriptor_pool: <Backend as GfxBackend>::DescriptorPool,
}

impl PipelineData {
    /// Creates and returns a new `PipelineData` object.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] will be returned if `gfx_hal` says there isn't enough memory.
    pub unsafe fn new(
        device: &backend::Device,
        capabilities: &SurfaceCapabilities,
        textures: &[Texture],
        surface_color_format: Format,
    ) -> Result<PipelineData, Error> {
        // Create the set layouts.
        let sampler_layout = device.create_descriptor_set_layout(
            &[DescriptorSetLayoutBinding {
                binding: 0,
                ty: DescriptorType::Sampler,
                count: 1,
                stage_flags: ShaderStageFlags::FRAGMENT,
                immutable_samplers: false,
            }],
            &[],
        )?;

        let texture_layout = device.create_descriptor_set_layout(
            &[DescriptorSetLayoutBinding {
                binding: 1,
                ty: DescriptorType::SampledImage,
                count: 1,
                stage_flags: ShaderStageFlags::FRAGMENT,
                immutable_samplers: false,
            }],
            &[],
        )?;

        // Create the descriptor pool.
        let mut pool: <Backend as GfxBackend>::DescriptorPool = device.create_descriptor_pool(
            textures.len() + 1, // number of textures + 1 for the sampler
            &[
                DescriptorRangeDesc {
                    ty: DescriptorType::Sampler,
                    count: 1,
                },
                DescriptorRangeDesc {
                    ty: DescriptorType::SampledImage,
                    count: textures.len(),
                },
            ],
        )?;

        // Set up the sampler and descriptor sets.
        let sampler = device.create_sampler(SamplerInfo::new(Filter::Nearest, WrapMode::Tile))?;
        let sampler_set = pool.allocate_set(&sampler_layout)?;
        let texture_sets = textures
            .iter()
            .map(|_| pool.allocate_set(&texture_layout).unwrap())
            .collect::<Vec<<Backend as GfxBackend>::DescriptorSet>>();

        // Write descriptor sets for resources to be made available to the shaders.
        let mut sets: Vec<DescriptorSetWrite<Backend, Vec<Descriptor<Backend>>>> =
            vec![DescriptorSetWrite {
                set: &sampler_set,
                binding: 0,
                array_offset: 0,
                descriptors: vec![Descriptor::Sampler(&sampler)],
            }];
        sets.extend(
            textures
                .iter()
                .enumerate()
                .map(|(index, texture)| DescriptorSetWrite {
                    set: &texture_sets[index],
                    binding: 1,
                    array_offset: 0,
                    descriptors: vec![Descriptor::Image(
                        texture.get_image_view().unwrap(),
                        Layout::ShaderReadOnlyOptimal,
                    )],
                })
                .collect::<Vec<DescriptorSetWrite<Backend, Vec<Descriptor<Backend>>>>>(),
        );

        device.write_descriptor_sets(sets);

        // Create the pipeline layout.
        let pipeline_layout =
            device.create_pipeline_layout(vec![&sampler_layout, &texture_layout], &[])?;

        // Set up shaders.
        let vertex_shader_module = {
            let spirv = include_bytes!("shaders/gen/shader.vert.spv");
            device.create_shader_module(spirv)?
        };

        let fragment_shader_module = {
            let spirv = include_bytes!("shaders/gen/shader.frag.spv");
            device.create_shader_module(spirv)?
        };

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
            let subpass_desc = SubpassDesc {
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

            device.create_render_pass(&[color_attachment], &[subpass_desc], &[dependency])
        }?;

        let pipeline = {
            let vs_entry = EntryPoint {
                entry: "main",
                module: &vertex_shader_module,
                specialization: Specialization::default(),
            };

            let fs_entry = EntryPoint {
                entry: "main",
                module: &fragment_shader_module,
                specialization: Specialization::default(),
            };

            let shader_entries = GraphicsShaderSet {
                vertex: vs_entry,
                hull: None,
                domain: None,
                geometry: None,
                fragment: Some(fs_entry),
            };

            let subpass = Subpass {
                index: 0,
                main_pass: &render_pass,
            };

            // Set up the pipeline.
            let mut pipeline_desc = GraphicsPipelineDesc::new(
                shader_entries,
                Primitive::TriangleList,
                Rasterizer::FILL,
                &pipeline_layout,
                subpass,
            );

            pipeline_desc
                .blender
                .targets
                .push(ColorBlendDesc(ColorMask::ALL, BlendState::ALPHA));

            // Add vertex buffer to the pipeline.
            pipeline_desc.vertex_buffers.push(VertexBufferDesc {
                binding: 0,
                stride: std::mem::size_of::<FormattedVertexData>() as ElemStride,
                rate: 0,
            });

            // Attribute storing xyz data for each vertex.
            pipeline_desc.attributes.push(AttributeDesc {
                location: 0,
                binding: 0,
                element: Element {
                    format: Format::Rgb32Float,
                    offset: 0,
                },
            });

            // Attribute storing rgba data for each vertex.
            pipeline_desc.attributes.push(AttributeDesc {
                location: 1,
                binding: 0,
                element: Element {
                    format: Format::Rgba32Float,
                    offset: 12,
                },
            });

            // Attribute for storing uv information for each vertex.
            pipeline_desc.attributes.push(AttributeDesc {
                location: 2,
                binding: 0,
                element: Element {
                    format: Format::Rg32Float,
                    offset: 28,
                },
            });

            let extent = capabilities.current_extent.unwrap();
            let view_rect = Rect {
                x: 0,
                y: 0,
                w: extent.width as i16,
                h: extent.height as i16,
            };

            // Baked states that don't need to be reset every draw call.
            // Currently just the view rectangle and scissor rectangle.
            pipeline_desc.baked_states = BakedStates {
                viewport: Some(Viewport {
                    rect: view_rect,
                    depth: 0.0..1.0,
                }),
                scissor: Some(view_rect),
                blend_color: None,
                depth_bounds: None,
            };

            match device.create_graphics_pipeline(&pipeline_desc, None) {
                Ok(pipeline) => pipeline,
                Err(err) => panic!("Couldn't create graphics pipeline: {}", err),
            }
        };

        Ok(PipelineData {
            descriptor_pool: pool,
            sampler_layout,
            texture_layout,
            sampler_set,
            texture_sets,
            sampler,
            vertex_shader_module,
            fragment_shader_module,
            pipeline_layout,
            pipeline,
            render_pass,
        })
    }

    /// Destroys all resources created by the PipelineData, consuming it in the process.
    pub unsafe fn destroy(mut self, device: &backend::Device) {
        let mut sets = vec![self.sampler_set];
        sets.extend(self.texture_sets);
        self.descriptor_pool.free_sets(sets);
        device.destroy_descriptor_pool(self.descriptor_pool);
        device.destroy_descriptor_set_layout(self.sampler_layout);
        device.destroy_descriptor_set_layout(self.texture_layout);

        device.destroy_render_pass(self.render_pass);
        device.destroy_graphics_pipeline(self.pipeline);

        device.destroy_shader_module(self.fragment_shader_module);
        device.destroy_shader_module(self.vertex_shader_module);

        device.destroy_pipeline_layout(self.pipeline_layout);

        device.destroy_sampler(self.sampler);
    }
}
