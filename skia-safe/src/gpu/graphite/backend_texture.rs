use crate::gpu::graphite::TextureInfo;
use crate::prelude::*;
use skia_bindings as sb;
use std::fmt;

pub type BackendTexture = Handle<sb::skgpu_graphite_BackendTexture>;
unsafe impl Send for BackendTexture {}
unsafe impl Sync for BackendTexture {}

impl NativeDrop for sb::skgpu_graphite_BackendTexture {
    fn drop(&mut self) {
        unsafe { sb::C_BackendTexture_Destruct(self) }
    }
}

impl Default for BackendTexture {
    fn default() -> Self {
        Self::construct(|bt| unsafe { sb::C_BackendTexture_Construct(bt) })
    }
}

impl fmt::Debug for BackendTexture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackendTexture")
            .field("dimensions", &self.dimensions())
            .field("info", &self.info())
            .finish()
    }
}

impl BackendTexture {
    pub fn dimensions(&self) -> crate::ISize {
        crate::ISize::from_native_c(unsafe { sb::C_BackendTexture_dimensions(self.native()) })
    }

    pub fn info(&self) -> TextureInfo {
        TextureInfo::construct(|ti| unsafe { sb::C_BackendTexture_info(self.native(), ti) })
    }

    pub fn is_valid(&self) -> bool {
        unsafe { sb::C_BackendTexture_isValid(self.native()) }
    }

    #[cfg(feature = "metal")]
    pub unsafe fn new_metal(
        dimensions: impl Into<crate::ISize>,
        texture: crate::gpu::mtl::Handle,
    ) -> Self {
        Self::construct(|bt| {
            sb::C_BackendTexture_MakeMetal(bt, dimensions.into().native(), texture)
        })
    }

    #[cfg(feature = "vulkan")]
    pub unsafe fn new_vulkan(
        dimensions: impl Into<crate::ISize>,
        texture_info: &crate::gpu::graphite::vk::TextureInfo,
        layout: crate::gpu::vk::ImageLayout,
        queue_family_index: u32,
        image: crate::gpu::vk::Image,
        alloc: crate::gpu::vk::Alloc,
    ) -> Self {
        Self::construct(|bt| {
            sb::C_BackendTexture_MakeVulkan(
                bt,
                dimensions.into().native(),
                texture_info.native(),
                layout,
                queue_family_index,
                image,
                alloc.native(),
            )
        })
    }
}
