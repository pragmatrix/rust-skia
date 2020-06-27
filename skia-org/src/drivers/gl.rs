use crate::artifact;
use crate::drivers::DrawingDriver;
use skia_safe::{gpu, Budgeted, Canvas, ImageInfo, Surface};
use std::path::Path;
use surfman::{Device, ContextAttributes};

pub struct OpenGL {
    device: Device,
    surfman_context: surfman::Context,
    context: gpu::Context,
}

impl Drop for OpenGL {
    fn drop(&mut self) {
        self.device.destroy_context(&mut self.surfman_context).unwrap();
    }
}

impl DrawingDriver for OpenGL {
    const NAME: &'static str = "opengl";

    fn new() -> Self {
        let connection = surfman::Connection::new().unwrap();
        let adapter = connection.create_hardware_adapter().unwrap();
        let mut device = connection.create_device(&adapter).unwrap();
        let context_attributes = ContextAttributes {
            version: surfman::GLVersion::new(3, 3),
            flags: surfman::ContextAttributeFlags::empty(),
        };
        let context_descriptor = device
            .create_context_descriptor(&context_attributes)
            .unwrap();
        let context = device.create_context(&context_descriptor, None).unwrap();
        device.make_context_current(&context).unwrap();

        Self {
            device,
            surfman_context: context,
            context: gpu::Context::new_gl(None).unwrap(),
        }
    }

    fn draw_image(
        &mut self,
        (width, height): (i32, i32),
        path: &Path,
        name: &str,
        func: impl Fn(&mut Canvas),
    ) {
        let image_info = ImageInfo::new_n32_premul((width * 2, height * 2), None);
        let mut surface = Surface::new_render_target(
            &mut self.context,
            Budgeted::Yes,
            &image_info,
            None,
            gpu::SurfaceOrigin::BottomLeft,
            None,
            false,
        )
        .unwrap();

        artifact::draw_image_on_surface(&mut surface, path, name, func);
    }
}
