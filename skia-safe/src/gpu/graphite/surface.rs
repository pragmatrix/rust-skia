use crate::{prelude::*, Surface};
use skia_bindings as sb;

pub fn make(
    recorder: &mut crate::gpu::graphite::Recorder,
    image_info: &crate::ImageInfo,
    mipmapped: Option<crate::gpu::Mipmapped>,
    surface_props: Option<&crate::SurfaceProps>,
) -> Option<Surface> {
    Surface::from_ptr(unsafe {
        sb::C_Surface_Make(
            recorder.native_mut(),
            image_info.native(),
            mipmapped.unwrap_or(crate::gpu::Mipmapped::No),
            surface_props.native_ptr_or_null(),
        )
    })
}

pub fn wrap_backend_texture(
    recorder: &mut crate::gpu::graphite::Recorder,
    backend_texture: &crate::gpu::graphite::BackendTexture,
    color_type: crate::ColorType,
    color_space: Option<&crate::ColorSpace>,
    surface_props: Option<&crate::SurfaceProps>,
) -> Option<Surface> {
    Surface::from_ptr(unsafe {
        sb::C_Surface_MakeGraphiteWrapped(
            recorder.native_mut(),
            backend_texture.native(),
            color_type.into_native(),
            color_space.native_ptr_or_null(),
            surface_props.native_ptr_or_null(),
        )
    })
}
