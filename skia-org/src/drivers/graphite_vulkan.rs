use std::{path::Path, ptr};

use ash::vk::Handle;
use skia_safe::{
    gpu::{
        self,
        graphite::{self, Context, ContextOptions, Recorder},
        Mipmapped,
    },
    Canvas,
};

use crate::{artifact, drivers::DrawingDriver, Driver};

// Re-use AshGraphics from vulkan.rs if possible, or duplicate it.
// Since vulkan.rs is a module, we can't easily access AshGraphics if it's not public.
// Let's assume we need to copy it or make it public. For now, I'll copy the necessary parts or try to import if I can make it public.
// Checking vulkan.rs again... it's not public. I'll copy the AshGraphics struct and implementation for now to avoid modifying existing vulkan.rs too much,
// or better, I'll modify vulkan.rs to export AshGraphics.

#[path = "vulkan.rs"]
pub mod vulkan_driver;
use vulkan_driver::AshGraphics;

#[allow(dead_code)]
pub struct GraphiteVulkan {
    // ordered for drop order
    recorder: Recorder,
    context: Context,
    ash_graphics: AshGraphics,
}

impl DrawingDriver for GraphiteVulkan {
    const DRIVER: Driver = Driver::GraphiteVulkan;

    fn new() -> Self {
        let ash_graphics = unsafe { AshGraphics::new("skia-org") };
        let mut context = {
            let get_proc = |of| unsafe {
                match ash_graphics.get_proc(of) {
                    Some(f) => f as _,
                    None => {
                        println!("resolve of {} failed", of.name().to_str().unwrap());
                        ptr::null()
                    }
                }
            };

            let backend_context = unsafe {
                gpu::vk::BackendContext::new(
                    ash_graphics.instance.handle().as_raw() as _,
                    ash_graphics.physical_device.as_raw() as _,
                    ash_graphics.device.handle().as_raw() as _,
                    (
                        ash_graphics.queue_and_index.0.as_raw() as _,
                        ash_graphics.queue_and_index.1 as usize,
                    ),
                    &get_proc,
                )
            };

            let options = ContextOptions::default();
            unsafe { Context::make_vulkan(&backend_context, &options) }.unwrap()
        };

        let recorder = context.make_recorder(None).unwrap();

        Self {
            recorder,
            context,
            ash_graphics,
        }
    }

    fn draw_image(
        &mut self,
        (width, height): (i32, i32),
        path: &Path,
        name: &str,
        func: impl Fn(&Canvas),
    ) {
        let width = width * 2;
        let height = height * 2;

        // 1. Create Vulkan Image (OPTIMAL)
        let create_info = ash::vk::ImageCreateInfo::default()
            .image_type(ash::vk::ImageType::TYPE_2D)
            .format(ash::vk::Format::B8G8R8A8_UNORM)
            .extent(ash::vk::Extent3D {
                width: width as u32,
                height: height as u32,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(ash::vk::SampleCountFlags::TYPE_1)
            .tiling(ash::vk::ImageTiling::OPTIMAL)
            .usage(ash::vk::ImageUsageFlags::COLOR_ATTACHMENT | ash::vk::ImageUsageFlags::TRANSFER_SRC | ash::vk::ImageUsageFlags::SAMPLED | ash::vk::ImageUsageFlags::TRANSFER_DST | ash::vk::ImageUsageFlags::INPUT_ATTACHMENT)
            .sharing_mode(ash::vk::SharingMode::EXCLUSIVE)
            .initial_layout(ash::vk::ImageLayout::UNDEFINED);

        let image = unsafe { self.ash_graphics.device.create_image(&create_info, None).unwrap() };

        let mem_requirements = unsafe { self.ash_graphics.device.get_image_memory_requirements(image) };
        let memory_type_index = self
            .find_memory_type(
                mem_requirements.memory_type_bits,
                ash::vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
            .expect("Failed to find suitable memory type");

        let alloc_info = ash::vk::MemoryAllocateInfo::default()
            .allocation_size(mem_requirements.size)
            .memory_type_index(memory_type_index);

        let memory = unsafe { self.ash_graphics.device.allocate_memory(&alloc_info, None).unwrap() };
        unsafe { self.ash_graphics.device.bind_image_memory(image, memory, 0).unwrap() };

        // 2. Create BackendTexture
        let texture_info = graphite::vk::TextureInfo::new(
            1,
            Mipmapped::No,
            unsafe { std::mem::transmute(ash::vk::ImageCreateFlags::empty().as_raw()) },
            unsafe { std::mem::transmute(ash::vk::Format::B8G8R8A8_UNORM.as_raw()) },
            unsafe { std::mem::transmute(ash::vk::ImageTiling::OPTIMAL.as_raw()) },
            ash::vk::ImageUsageFlags::COLOR_ATTACHMENT.as_raw() | ash::vk::ImageUsageFlags::TRANSFER_SRC.as_raw() | ash::vk::ImageUsageFlags::SAMPLED.as_raw() | ash::vk::ImageUsageFlags::TRANSFER_DST.as_raw() | ash::vk::ImageUsageFlags::INPUT_ATTACHMENT.as_raw(),
            unsafe { std::mem::transmute(ash::vk::SharingMode::EXCLUSIVE.as_raw()) },
            ash::vk::ImageAspectFlags::COLOR.as_raw(),
            &gpu::vk::YcbcrConversionInfo::default(),
        );

        let mut alloc = gpu::vk::Alloc::default();
        alloc.memory = memory.as_raw() as _;
        alloc.offset = 0;
        alloc.size = mem_requirements.size as _;
        alloc.flags = gpu::vk::AllocFlag::empty();

        let backend_texture = unsafe {
            graphite::BackendTexture::new_vulkan(
                (width, height),
                &texture_info,
                std::mem::transmute(ash::vk::ImageLayout::UNDEFINED.as_raw()),
                ash::vk::QUEUE_FAMILY_IGNORED,
                image.as_raw() as _,
                alloc,
            )
        };

        // 3. Wrap in Surface
        let mut surface = graphite::surface::wrap_backend_texture(
            &mut self.recorder,
            &backend_texture,
            skia_safe::ColorType::BGRA8888,
            Some(&skia_safe::ColorSpace::new_srgb()),
            None,
        )
        .unwrap();

        // 4. Draw
        let canvas = surface.canvas();
        canvas.scale((2.0, 2.0));
        func(canvas);

        // 5. Snap and Submit
        let recording = self.recorder.snap().expect("Failed to snap recording");
        self.context.insert_recording(recording);
        self.context.submit(Some(graphite::SyncToCpu::Yes));

        // 6. Read pixels (Copy to Buffer)
        // Create Buffer
        let buffer_create_info = ash::vk::BufferCreateInfo::default()
            .size(mem_requirements.size)
            .usage(ash::vk::BufferUsageFlags::TRANSFER_DST)
            .sharing_mode(ash::vk::SharingMode::EXCLUSIVE);
        let buffer = unsafe { self.ash_graphics.device.create_buffer(&buffer_create_info, None).unwrap() };
        let buffer_mem_reqs = unsafe { self.ash_graphics.device.get_buffer_memory_requirements(buffer) };
        let buffer_mem_type = self.find_memory_type(buffer_mem_reqs.memory_type_bits, ash::vk::MemoryPropertyFlags::HOST_VISIBLE | ash::vk::MemoryPropertyFlags::HOST_COHERENT).unwrap();
        let buffer_memory = unsafe { self.ash_graphics.device.allocate_memory(&ash::vk::MemoryAllocateInfo::default().allocation_size(buffer_mem_reqs.size).memory_type_index(buffer_mem_type), None).unwrap() };
        unsafe { self.ash_graphics.device.bind_buffer_memory(buffer, buffer_memory, 0).unwrap() };

        // Create Command Pool & Buffer
        let pool_create_info = ash::vk::CommandPoolCreateInfo::default()
            .queue_family_index(self.ash_graphics.queue_and_index.1 as u32)
            .flags(ash::vk::CommandPoolCreateFlags::TRANSIENT);
        let command_pool = unsafe { self.ash_graphics.device.create_command_pool(&pool_create_info, None).unwrap() };
        let cmd_buf_alloc_info = ash::vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(ash::vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let command_buffer = unsafe { self.ash_graphics.device.allocate_command_buffers(&cmd_buf_alloc_info).unwrap()[0] };

        // Record Copy
        let begin_info = ash::vk::CommandBufferBeginInfo::default()
            .flags(ash::vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.ash_graphics.device.begin_command_buffer(command_buffer, &begin_info).unwrap();
            
            let barrier = ash::vk::ImageMemoryBarrier::default()
                .old_layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .new_layout(ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .src_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
                .image(image)
                .subresource_range(ash::vk::ImageSubresourceRange {
                    aspect_mask: ash::vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .src_access_mask(ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(ash::vk::AccessFlags::TRANSFER_READ);
                
            self.ash_graphics.device.cmd_pipeline_barrier(
                command_buffer,
                ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                ash::vk::PipelineStageFlags::TRANSFER,
                ash::vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );

            let copy_region = ash::vk::BufferImageCopy::default()
                .buffer_offset(0)
                .buffer_row_length(width as u32)
                .buffer_image_height(height as u32)
                .image_subresource(ash::vk::ImageSubresourceLayers {
                    aspect_mask: ash::vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image_offset(ash::vk::Offset3D { x: 0, y: 0, z: 0 })
                .image_extent(ash::vk::Extent3D { width: width as u32, height: height as u32, depth: 1 });

            self.ash_graphics.device.cmd_copy_image_to_buffer(
                command_buffer,
                image,
                ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                buffer,
                &[copy_region],
            );
            
            self.ash_graphics.device.end_command_buffer(command_buffer).unwrap();
        }
        
        // Submit Copy
        let command_buffers = [command_buffer];
        let submit_info = ash::vk::SubmitInfo::default()
            .command_buffers(&command_buffers);
        unsafe {
            self.ash_graphics.device.queue_submit(self.ash_graphics.queue_and_index.0, &[submit_info], ash::vk::Fence::null()).unwrap();
            self.ash_graphics.device.queue_wait_idle(self.ash_graphics.queue_and_index.0).unwrap();
        }
        
        // Map Buffer
        let data_ptr = unsafe { self.ash_graphics.device.map_memory(buffer_memory, 0, buffer_mem_reqs.size, ash::vk::MemoryMapFlags::empty()).unwrap() };
        
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        unsafe {
            std::ptr::copy_nonoverlapping(data_ptr as *const u8, pixels.as_mut_ptr(), (width * height * 4) as usize);
            self.ash_graphics.device.unmap_memory(buffer_memory);
        }

        artifact::write_png(
            path,
            name,
            (width, height),
            &mut pixels,
            (width * 4) as usize,
            skia_safe::ColorType::BGRA8888,
        );

        unsafe {
            self.ash_graphics.device.destroy_command_pool(command_pool, None);
            self.ash_graphics.device.destroy_buffer(buffer, None);
            self.ash_graphics.device.free_memory(buffer_memory, None);
            self.ash_graphics.device.destroy_image(image, None);
            self.ash_graphics.device.free_memory(memory, None);
        }
    }
}

impl GraphiteVulkan {
    fn find_memory_type(
        &self,
        type_filter: u32,
        properties: ash::vk::MemoryPropertyFlags,
    ) -> Option<u32> {
        let mem_properties = unsafe {
            self.ash_graphics
                .instance
                .get_physical_device_memory_properties(self.ash_graphics.physical_device)
        };
        for i in 0..mem_properties.memory_type_count {
            if (type_filter & (1 << i)) != 0
                && (mem_properties.memory_types[i as usize].property_flags & properties)
                    == properties
            {
                return Some(i);
            }
        }
        None
    }
}
