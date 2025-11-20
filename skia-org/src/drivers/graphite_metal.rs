use std::path::Path;

use foreign_types_shared::ForeignType;
use metal::{CommandQueue, Device};
use objc2::rc::{autoreleasepool, Retained};
use objc2_foundation::NSAutoreleasePool;

use crate::{artifact, drivers::DrawingDriver, Driver};
use skia_safe::{
    gpu::{
        graphite::{self, mtl, BackendTexture, Context, ContextOptions, Recorder, SyncToCpu, TextureInfo},
        Mipmapped,
    },
    Canvas, ImageInfo, Surface,
};

#[allow(dead_code)]
pub struct GraphiteMetal {
    // note: ordered for drop order
    recorder: Recorder,
    context: Context,
    queue: CommandQueue,
    device: Device,
    pool: Retained<NSAutoreleasePool>,
}

impl DrawingDriver for GraphiteMetal {
    const DRIVER: Driver = Driver::GraphiteMetal;

    fn new() -> Self {
        let pool = unsafe { NSAutoreleasePool::new() };

        let device = Device::system_default().expect("no Metal device");
        let queue = device.new_command_queue();

        let backend = unsafe {
            mtl::BackendContext::new(
                device.as_ptr() as mtl::Handle,
                queue.as_ptr() as mtl::Handle,
            )
        };

        let options = ContextOptions::default();
        let mut context = unsafe { Context::make_metal(&backend, &options) }.unwrap();
        let recorder = context.make_recorder(None).unwrap();

        Self {
            recorder,
            context,
            queue,
            device,
            pool,
        }
    }

    fn draw_image(
        &mut self,
        (width, height): (i32, i32),
        path: &Path,
        name: &str,
        func: impl Fn(&Canvas),
    ) {
        autoreleasepool(|_| {
            let mut context = unsafe {
                let backend = mtl::BackendContext::new(
                    self.device.as_ptr() as mtl::Handle,
                    self.queue.as_ptr() as mtl::Handle,
                );
                let options = ContextOptions::default();
                Context::make_metal(&backend, &options)
            }
            .unwrap();

            let mut recorder = context.make_recorder(None).unwrap();

            let texture_desc = metal::TextureDescriptor::new();
            texture_desc.set_width((width * 2) as u64);
            texture_desc.set_height((height * 2) as u64);
            texture_desc.set_pixel_format(metal::MTLPixelFormat::BGRA8Unorm);
            texture_desc.set_usage(metal::MTLTextureUsage::RenderTarget | metal::MTLTextureUsage::ShaderRead);
            texture_desc.set_storage_mode(metal::MTLStorageMode::Managed);

            let texture = self.device.new_texture(&texture_desc);

            let backend_texture = unsafe {
                BackendTexture::new_metal(
                    (width * 2, height * 2),
                    texture.as_ptr() as mtl::Handle,
                )
            };

            let mut surface = graphite::surface::wrap_backend_texture(
                &mut recorder,
                &backend_texture,
                skia_safe::ColorType::BGRA8888,
                Some(&skia_safe::ColorSpace::new_srgb()),
                None,
            )
            .unwrap();

            let canvas = surface.canvas();
            canvas.scale((2.0, 2.0));
            func(canvas);

            let recording = recorder.snap().unwrap();
            context.insert_recording(recording);
            context.submit(Some(SyncToCpu::Yes));

            let row_bytes = (width * 2 * 4) as usize;
            let mut pixels = vec![0u8; row_bytes * (height * 2) as usize];

            texture.get_bytes(
                pixels.as_mut_ptr() as *mut std::ffi::c_void,
                row_bytes as u64,
                metal::MTLRegion::new_2d(0, 0, (width * 2) as u64, (height * 2) as u64),
                0,
            );

            artifact::write_png(
                path,
                name,
                (width * 2, height * 2),
                &mut pixels,
                row_bytes,
                skia_safe::ColorType::BGRA8888,
            );
        })
    }
}
