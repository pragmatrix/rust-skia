use crate::gpu::gl;
#[cfg(feature = "vulkan")]
use crate::gpu::vk;
use crate::prelude::*;
use skia_bindings as sb;
use skia_bindings::{GrBackendApi, GrBackendSemaphore};
use std::ptr;

pub type BackendSemaphore = Handle<GrBackendSemaphore>;

impl NativeDrop for GrBackendSemaphore {
    fn drop(&mut self) {}
}

impl NativeClone for GrBackendSemaphore {
    fn clone(&self) -> Self {
        *self
    }
}

impl Handle<GrBackendSemaphore> {
    pub fn new() -> Self {
        Self::from_native(GrBackendSemaphore {
            fBackend: GrBackendApi::kOpenGL,
            __bindgen_anon_1: sb::GrBackendSemaphore__bindgen_ty_1 {
                fGLSync: ptr::null_mut(),
            },
            fIsInitialized: false,
        })
    }

    pub fn init_gl(&mut self, sync: gl::Sync) {
        let n = self.native_mut();
        n.fBackend = GrBackendApi::kOpenGL;
        n.__bindgen_anon_1.fGLSync = sync;
        n.fIsInitialized = true;
    }

    pub fn init_vulkan(&mut self, semaphore: vk::Semaphore) {
        let n = self.native_mut();
        n.fBackend = GrBackendApi::kVulkan;
        n.__bindgen_anon_1.fVkSemaphore = semaphore;
        n.fIsInitialized = true;
    }

    pub fn is_initialized(&self) -> bool {
        self.native().fIsInitialized
    }

    pub fn gl_sync(&self) -> gl::Sync {
        let n = self.native();
        if !n.fIsInitialized || n.fBackend != GrBackendApi::kOpenGL {
            return ptr::null_mut();
        }
        unsafe { n.__bindgen_anon_1.fGLSync }
    }

    #[cfg(feature = "vulkan")]
    pub fn vk_semaphore(&self) -> vk::Semaphore {
        let n = self.native();
        if !n.fIsInitialized || n.fBackend != GrBackendApi::kVulkan {
            return ptr::null_mut();
        }
        unsafe { n.__bindgen_anon_1.fVkSemaphore }
    }
}
