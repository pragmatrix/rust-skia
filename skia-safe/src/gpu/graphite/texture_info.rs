use crate::prelude::*;
use skia_bindings as sb;
use std::fmt;

pub type TextureInfo = Handle<sb::skgpu_graphite_TextureInfo>;
unsafe impl Send for TextureInfo {}
unsafe impl Sync for TextureInfo {}

impl NativeDrop for sb::skgpu_graphite_TextureInfo {
    fn drop(&mut self) {
        unsafe { sb::C_TextureInfo_Destruct(self) }
    }
}

impl Default for TextureInfo {
    fn default() -> Self {
        Self::construct(|ti| unsafe { sb::C_TextureInfo_Construct(ti) })
    }
}

impl fmt::Debug for TextureInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextureInfo").finish()
    }
}

impl TextureInfo {
    pub fn is_valid(&self) -> bool {
        unsafe { sb::C_TextureInfo_isValid(self.native()) }
    }
    #[cfg(feature = "metal")]
    pub unsafe fn new_metal(texture: crate::gpu::mtl::Handle) -> Self {
        Self::construct(|ti| sb::C_TextureInfo_MakeMetal(ti, texture))
    }
}
