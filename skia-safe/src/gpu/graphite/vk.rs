use crate::prelude::*;
use skia_bindings as sb;
use crate::gpu;

pub struct TextureInfo(*mut sb::skgpu_graphite_VulkanTextureInfo);

impl Drop for TextureInfo {
    fn drop(&mut self) {
        unsafe { sb::C_VulkanTextureInfo_Destruct(self.0) }
    }
}

impl NativeAccess for TextureInfo {
    type Native = sb::skgpu_graphite_VulkanTextureInfo;

    fn native(&self) -> &Self::Native {
        unsafe { &*self.0 }
    }

    fn native_mut(&mut self) -> &mut Self::Native {
        unsafe { &mut *self.0 }
    }
}

impl TextureInfo {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sample_count: u32,
        mipmapped: gpu::Mipmapped,
        flags: gpu::vk::ImageCreateFlags,
        format: gpu::vk::Format,
        image_tiling: gpu::vk::ImageTiling,
        image_usage_flags: gpu::vk::ImageUsageFlags,
        sharing_mode: gpu::vk::SharingMode,
        aspect_mask: gpu::vk::ImageAspectFlags,
        ycbcr_conversion_info: &gpu::vk::YcbcrConversionInfo,
    ) -> Self {
        Self(unsafe {
            sb::C_VulkanTextureInfo_Make(
                sample_count,
                mipmapped,
                flags,
                format,
                image_tiling,
                image_usage_flags,
                sharing_mode,
                aspect_mask,
                ycbcr_conversion_info.native(),
            )
        })
    }
}
