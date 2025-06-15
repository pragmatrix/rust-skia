use ash::vk::Handle;
use std::{ptr, sync::Arc};
use vulkano::{
    device::Queue,
    image::{view::ImageView, ImageLayout, ImageUsage},
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass},
    swapchain::{
        acquire_next_image, PresentMode, Surface, Swapchain, SwapchainAcquireFuture,
        SwapchainCreateInfo, SwapchainPresentInfo,
    },
    sync::{self, GpuFuture},
    Validated, VulkanError, VulkanObject,
};

use skia_safe::{
    gpu::{self, backend_render_targets, direct_contexts, surfaces, vk, FlushInfo},
    ColorType,
};

use winit::{dpi::LogicalSize, dpi::PhysicalSize, window::Window};

pub struct VulkanRenderer {
    pub window: Arc<Window>,
    queue: Arc<Queue>,
    swapchain: Arc<Swapchain>,
    framebuffers: Vec<Arc<Framebuffer>>,
    render_pass: Arc<RenderPass>,
    last_render: Option<Box<dyn GpuFuture>>,
    skia_ctx: gpu::DirectContext,
    swapchain_is_valid: bool,
    pending_resize: bool,
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        // prevent in-flight commands from trying to draw to the window after it's gone
        self.skia_ctx.abandon();
    }
}

impl VulkanRenderer {
    pub fn new(window: Arc<Window>, queue: Arc<Queue>) -> Self {
        // Extract references to key structs from the queue
        let library = queue.device().instance().library();
        let instance = queue.device().instance();
        let device = queue.device();
        let queue = queue.clone();

        // Before we can render to a window, we must first create a `vulkano::swapchain::Surface`
        // object from it, which represents the drawable surface of a window. For that we must wrap
        // the `winit::window::Window` in an `Arc`.
        let surface = Surface::from_window(instance.clone(), window.clone()).unwrap();
        let window_size = window.inner_size();

        // Before we can draw on the surface, we have to create what is called a swapchain.
        // Creating a swapchain allocates the color buffers that will contain the image that will
        // ultimately be visible on the screen. These images are returned alongside the swapchain.
        let (swapchain, _images) = {
            // Querying the capabilities of the surface. When we create the swapchain we can only
            // pass values that are allowed by the capabilities.
            let surface_capabilities = device
                .physical_device()
                .surface_capabilities(&surface, Default::default())
                .unwrap();

            // Choosing the internal format that the images will have.
            let (image_format, _) = device
                .physical_device()
                .surface_formats(&surface, Default::default())
                .unwrap()[0];

            // Check supported present modes for smoother rendering
            let supported_modes = device
                .physical_device()
                .surface_present_modes(&surface, Default::default())
                .unwrap();

            let present_mode = if supported_modes.contains(&PresentMode::Mailbox) {
                PresentMode::Mailbox
            } else {
                PresentMode::Fifo
            };

            // Please take a look at the docs for the meaning of the parameters we didn't mention.
            Swapchain::new(
                device.clone(),
                surface,
                SwapchainCreateInfo {
                    // Some drivers report an `min_image_count` of 1, but fullscreen mode requires
                    // at least 2. Therefore we must ensure the count is at least 2, otherwise the
                    // program would crash when entering fullscreen mode on those drivers.
                    min_image_count: surface_capabilities.min_image_count.max(2),

                    // The size of the window, only used to initially setup the swapchain.
                    //
                    // NOTE:
                    // On some drivers the swapchain extent is specified by
                    // `surface_capabilities.current_extent` and the swapchain size must use this
                    // extent. This extent is always the same as the window size.
                    //
                    // However, other drivers don't specify a value, i.e.
                    // `surface_capabilities.current_extent` is `None`. These drivers will allow
                    // anything, but the only sensible value is the window size.
                    //
                    // Both of these cases need the swapchain to use the window size, so we just
                    // use that.
                    image_extent: window_size.into(),

                    image_usage: ImageUsage::COLOR_ATTACHMENT,

                    image_format,

                    // The present_mode affects what is commonly known as "vertical sync" or "vsync" for short.
                    // The `Immediate` mode is equivalent to disabling vertical sync, while the others enable
                    // vertical sync in various forms. An important aspect of the present modes is their potential
                    // *latency*: the time between when an image is presented, and when it actually appears on
                    // the display.
                    //
                    // Only `Fifo` is guaranteed to be supported on every device. For the others, you must call
                    // [`surface_present_modes`] to see if they are supported.
                    present_mode,

                    // The alpha mode indicates how the alpha value of the final image will behave.
                    // For example, you can choose whether the window will be
                    // opaque or transparent.
                    composite_alpha: surface_capabilities
                        .supported_composite_alpha
                        .into_iter()
                        .next()
                        .unwrap(),

                    ..Default::default()
                },
            )
            .unwrap()
        };

        // The next step is to create a *render pass*, which is an object that describes where the
        // output of the graphics pipeline will go. It describes the layout of the images where the
        // colors (and in other use-cases depth and/or stencil information) will be written.
        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                // `color` is a custom name we give to the first and only attachment.
                color: {
                    // `format: <ty>` indicates the type of the format of the image. This has to be
                    // one of the types of the `vulkano::format` module (or alternatively one of
                    // your structs that implements the `FormatDesc` trait). Here we use the same
                    // format as the swapchain.
                    format: swapchain.image_format(),
                    // `samples: 1` means that we ask the GPU to use one sample to determine the
                    // value of each pixel in the color attachment. We could use a larger value
                    // (multisampling) for antialiasing. An example of this can be found in
                    // msaa-renderpass.rs.
                    samples: 1,
                    // `load_op: DontCare` means that the initial contents of the attachment haven't been
                    // 'cleared' ahead of time (i.e., the pixels haven't all been set to a single color).
                    // This is fine since we'll be filling the entire framebuffer with skia's output
                    load_op: DontCare,
                    // `store_op: Store` means that we ask the GPU to store the output of the draw
                    // in the actual image. We could also ask it to discard the result.
                    store_op: Store,
                    // Set proper initial and final layouts for swapchain images
                    initial_layout: ImageLayout::Undefined,
                    final_layout: ImageLayout::PresentSrc,
                },
            },
            pass: {
                // We use the attachment named `color` as the one and only color attachment.
                color: [color],
                // No depth-stencil attachment is indicated with empty brackets.
                depth_stencil: {},
            },
        )
        .unwrap();

        // The render pass we created above only describes the layout of our framebuffers. Before
        // we can draw we also need to create the actual framebuffers.
        //
        // Since we need to draw to multiple images, we are going to create a different framebuffer
        // for each image. We'll wait until the first `prepare_swapchain` call to actually allocate them.
        let framebuffers = vec![];

        // In some situations, the swapchain will become invalid by itself. This includes for
        // example when the window is resized (as the images of the swapchain will no longer match
        // the window's) or, on Android, when the application went to the background and goes back
        // to the foreground.
        //
        // In this situation, acquiring a swapchain image or presenting it will return an error.
        // Rendering to an image of that swapchain will not produce any error, but may or may not
        // work. To continue rendering, we need to recreate the swapchain by creating a new
        // swapchain. Here, we remember that we need to do this for the next loop iteration.
        //
        // Since we haven't allocated framebuffers yet, we'll start in an invalid state to flag that
        // they need to be recreated before we render.
        let swapchain_is_valid = false;

        // In the `draw_and_present` method below we are going to submit commands to the GPU.
        // Submitting a command produces an object that implements the `GpuFuture` trait, which
        // holds the resources for as long as they are in use by the GPU.
        //
        // Destroying the `GpuFuture` blocks until the GPU is finished executing it. In order to
        // avoid that, we store the submission of the previous frame here.
        let last_render = Some(sync::now(device.clone()).boxed());

        // Next we need to connect Skia's gpu backend to the device & queue we've set up.
        let skia_ctx = unsafe {
            // In order to access the vulkan api, we need to give skia some lookup routines
            // to find the expected function pointers for our configured instance & device.
            let get_proc = |gpo| {
                let get_device_proc_addr = instance.fns().v1_0.get_device_proc_addr;

                match gpo {
                    vk::GetProcOf::Instance(instance, name) => {
                        let vk_instance = ash::vk::Instance::from_raw(instance as _);
                        library.get_instance_proc_addr(vk_instance, name)
                    }
                    vk::GetProcOf::Device(device, name) => {
                        let vk_device = ash::vk::Device::from_raw(device as _);
                        get_device_proc_addr(vk_device, name)
                    }
                }
                .map(|f| f as _)
                .unwrap_or_else(|| {
                    println!("Vulkan: failed to resolve {}", gpo.name().to_str().unwrap());
                    ptr::null()
                })
            };

            // We then pass skia_safe references to the whole shebang, resulting in a DirectContext
            // from which we'll be able to get a canvas reference that draws directly to framebuffers
            // on the swapchain.
            let direct_context = direct_contexts::make_vulkan(
                &vk::BackendContext::new(
                    instance.handle().as_raw() as _,
                    device.physical_device().handle().as_raw() as _,
                    device.handle().as_raw() as _,
                    (
                        queue.handle().as_raw() as _,
                        queue.queue_family_index() as usize,
                    ),
                    &get_proc,
                ),
                None,
            )
            .unwrap();

            direct_context
        };

        VulkanRenderer {
            skia_ctx,
            queue,
            window,
            swapchain,
            swapchain_is_valid,
            render_pass,
            framebuffers,
            last_render,
            pending_resize: false,
        }
    }

    pub fn invalidate_swapchain(&mut self) {
        // Mark both swapchain as invalid and indicate a resize is pending
        self.swapchain_is_valid = false;
        self.pending_resize = true;
    }

    fn ensure_gpu_idle(&mut self) {
        // Ensure all GPU operations are complete before swapchain recreation
        if let Some(last_render) = self.last_render.as_mut() {
            last_render.cleanup_finished();
        }

        // Submit any pending Skia operations and wait for completion
        self.skia_ctx.submit(Some(gpu::SyncCpu::Yes)); // Sync/wait for completion

        // For critical operations like swapchain recreation, ensure device is fully idle
        unsafe {
            self.queue.device().wait_idle().ok();
        }
    }

    pub fn prepare_swapchain(&mut self) {
        // Early exit if swapchain is already valid and no resize is pending
        if self.swapchain_is_valid && !self.pending_resize {
            // Still do regular cleanup
            if let Some(last_render) = self.last_render.as_mut() {
                last_render.cleanup_finished();
            }
            return;
        }

        // Get current window size
        let window_size: PhysicalSize<u32> = self.window.inner_size();
        if window_size.width == 0 || window_size.height == 0 {
            // Window is minimized or has zero size, can't recreate swapchain
            return;
        }

        // Ensure complete GPU synchronization before recreating swapchain
        self.ensure_gpu_idle();

        // Recreate the swapchain
        let (new_swapchain, new_images) = self
            .swapchain
            .recreate(SwapchainCreateInfo {
                image_extent: window_size.into(),
                ..self.swapchain.create_info()
            })
            .expect("failed to recreate swapchain");

        self.swapchain = new_swapchain;

        // Recreate framebuffers with the new swapchain images
        self.framebuffers = new_images
            .iter()
            .map(|image| {
                let view = ImageView::new_default(image.clone()).unwrap();

                Framebuffer::new(
                    self.render_pass.clone(),
                    FramebufferCreateInfo {
                        attachments: vec![view],
                        ..Default::default()
                    },
                )
                .unwrap()
            })
            .collect::<Vec<_>>();

        // Create a fresh future for the new swapchain
        self.last_render = Some(sync::now(self.queue.device().clone()).boxed());

        // Mark swapchain as valid and clear pending resize flag
        self.swapchain_is_valid = true;
        self.pending_resize = false;
    }

    fn get_next_frame(&mut self) -> Option<(u32, SwapchainAcquireFuture)> {
        // Only try to acquire if the swapchain is currently valid
        if !self.swapchain_is_valid || self.pending_resize {
            return None;
        }

        // Try to acquire with a retry mechanism in case of semaphore issues
        for attempt in 0..3 {
            // Prepare to render by identifying the next framebuffer to draw to and acquiring the
            // GpuFuture that we'll be replacing `last_render` with once we submit the frame
            let result =
                acquire_next_image(self.swapchain.clone(), None).map_err(Validated::unwrap);

            match result {
                Ok((image_index, suboptimal, acquire_future)) => {
                    // `acquire_next_image` can be successful, but suboptimal. This means that the
                    // swapchain image will still work, but it may not display correctly. With some
                    // drivers this can be when the window resizes, but it may not cause the swapchain
                    // to become out of date.
                    if suboptimal {
                        self.swapchain_is_valid = false;
                        self.pending_resize = true;
                    }
                    return Some((image_index, acquire_future));
                }
                Err(VulkanError::OutOfDate) => {
                    self.swapchain_is_valid = false;
                    self.pending_resize = true;
                    return None;
                }
                Err(e) => {
                    eprintln!(
                        "Failed to acquire next image (attempt {}): {e}",
                        attempt + 1
                    );

                    // If this is a validation error related to semaphores and we have retries left,
                    // ensure GPU synchronization and try again
                    if attempt < 2 {
                        // Clean up any pending operations
                        if let Some(last_render) = self.last_render.as_mut() {
                            last_render.cleanup_finished();
                        }

                        // For persistent errors, ensure complete GPU synchronization
                        if attempt == 1 {
                            self.skia_ctx.submit(Some(gpu::SyncCpu::Yes)); // Sync submit
                        }

                        // Brief pause to allow GPU operations to settle
                        std::thread::sleep(std::time::Duration::from_millis(2));
                        continue;
                    }

                    // After all retries failed, mark for recreation
                    self.swapchain_is_valid = false;
                    self.pending_resize = true;
                    return None;
                }
            }
        }

        None
    }

    pub fn draw_and_present<F>(&mut self, f: F)
    where
        F: FnOnce(&skia_safe::Canvas, LogicalSize<f32>),
    {
        // Ensure swapchain is valid before trying to acquire
        self.prepare_swapchain();

        // Find the next framebuffer to render into and acquire a new GpuFuture to block on
        if let Some((image_index, acquire_future)) = self.get_next_frame() {
            // Pull the appropriate framebuffer from the swapchain and attach a skia Surface to it
            let framebuffer = self.framebuffers[image_index as usize].clone();
            let mut surface = surface_for_framebuffer(&mut self.skia_ctx, framebuffer.clone());
            let canvas = surface.canvas();

            // Use the display's DPI to convert the window size to logical coords and pre-scale the
            // canvas's matrix to match
            let extent: PhysicalSize<u32> = self.window.inner_size();
            let size: LogicalSize<f32> = extent.to_logical(self.window.scale_factor());

            let scale = (
                (f64::from(extent.width) / size.width as f64) as f32,
                (f64::from(extent.height) / size.height as f64) as f32,
            );
            canvas.reset_matrix();
            canvas.scale(scale);

            // Pass the surface's canvas and canvas size to the user-provided callback
            f(canvas, size);

            // Create the target layout state for presentation
            let present_state = vk::mutable_texture_states::new_vulkan(
                vk::ImageLayout::PRESENT_SRC_KHR,
                self.queue.queue_family_index(),
            );

            // Flush the canvas's contents to the framebuffer with proper layout transition
            let flush_info = FlushInfo::default();
            self.skia_ctx.flush_surface_with_texture_state(
                &mut surface,
                &flush_info,
                Some(&present_state),
            );
            
            // Submit all pending GPU operations
            self.skia_ctx.submit(None);

            // Get the current last_render future, creating a fresh one if None
            let last_render = self
                .last_render
                .take()
                .unwrap_or_else(|| sync::now(self.queue.device().clone()).boxed());

            // Send the framebuffer to the GPU and display it on screen
            let joined_future = last_render.join(acquire_future);
            let present_future = joined_future.then_swapchain_present(
                self.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(self.swapchain.clone(), image_index),
            );

            // Attempt to create a fence for this future with better error handling
            match present_future.then_signal_fence_and_flush() {
                Ok(fence_future) => {
                    self.last_render = Some(Box::new(fence_future) as Box<dyn GpuFuture>);
                }
                Err(vulkano::Validated::Error(vulkano::VulkanError::OutOfDate)) => {
                    // Swapchain is out of date, mark it for recreation
                    self.swapchain_is_valid = false;
                    self.pending_resize = true;
                    self.last_render = Some(sync::now(self.queue.device().clone()).boxed());
                }
                Err(e) => {
                    eprintln!("Failed to create fence for present future: {e}");
                    // If fence creation failed for other reasons, create a fresh future
                    self.last_render = Some(sync::now(self.queue.device().clone()).boxed());
                    // Also mark for potential swapchain recreation if this keeps happening
                    self.swapchain_is_valid = false;
                    self.pending_resize = true;
                }
            }
        } else {
            // Failed to acquire frame, ensure we have a valid future
            if self.last_render.is_none() {
                self.last_render = Some(sync::now(self.queue.device().clone()).boxed());
            }
        }
    }
}

// Create a skia `Surface` (and its associated `.canvas()`) whose render target is the specified `Framebuffer`.
fn surface_for_framebuffer(
    skia_ctx: &mut gpu::DirectContext,
    framebuffer: Arc<Framebuffer>,
) -> skia_safe::Surface {
    let [width, height] = framebuffer.extent();
    let image_access = &framebuffer.attachments()[0];
    let image_object = image_access.image().handle().as_raw();

    let format = image_access.format();

    let (vk_format, color_type) = match format {
        vulkano::format::Format::B8G8R8A8_UNORM => (
            skia_safe::gpu::vk::Format::B8G8R8A8_UNORM,
            ColorType::BGRA8888,
        ),
        _ => panic!("Unsupported color format {format:?}"),
    };

    let alloc = vk::Alloc::default();
    let image_info = &unsafe {
        vk::ImageInfo::new(
            image_object as _,
            alloc,
            vk::ImageTiling::OPTIMAL,
            // Use COLOR_ATTACHMENT_OPTIMAL for rendering
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk_format,
            1,
            None,
            None,
            None,
            None,
        )
    };

    let render_target = &backend_render_targets::make_vk(
        (width.try_into().unwrap(), height.try_into().unwrap()),
        image_info,
    );

    surfaces::wrap_backend_render_target(
        skia_ctx,
        render_target,
        gpu::SurfaceOrigin::TopLeft,
        color_type,
        None,
        None,
    )
    .unwrap()
}
