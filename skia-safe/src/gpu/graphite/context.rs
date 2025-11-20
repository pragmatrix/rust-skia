use crate::prelude::*;
use skia_bindings as sb;
use std::fmt;

pub struct Context(*mut sb::skgpu_graphite_Context);

impl NativeAccess for Context {
    type Native = sb::skgpu_graphite_Context;

    fn native(&self) -> &Self::Native {
        unsafe { &*self.0 }
    }

    fn native_mut(&mut self) -> &mut Self::Native {
        unsafe { &mut *self.0 }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { sb::C_Context_Destruct(self.0) }
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context").finish()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum SyncToCpu {
    No = sb::skgpu_graphite_SyncToCpu::kNo as _,
    Yes = sb::skgpu_graphite_SyncToCpu::kYes as _,
}

impl NativeTransmutable<sb::skgpu_graphite_SyncToCpu> for SyncToCpu {}

impl Context {
    pub unsafe fn from_ptr(ptr: *mut sb::skgpu_graphite_Context) -> Option<Context> {
        if ptr.is_null() {
            None
        } else {
            Some(Context(ptr))
        }
    }

    #[cfg(feature = "metal")]
    pub unsafe fn make_metal(
        backend_context: &crate::gpu::graphite::mtl::BackendContext,
        options: &crate::gpu::graphite::ContextOptions,
    ) -> Option<Context> {
        Context::from_ptr(sb::C_Context_MakeMetal(
            backend_context.native(),
            options.native(),
        ))
    }

    #[cfg(feature = "vulkan")]
    pub unsafe fn make_vulkan(
        backend_context: &crate::gpu::vk::BackendContext,
        options: &crate::gpu::graphite::ContextOptions,
    ) -> Option<Context> {
        let _resolver = backend_context.begin_resolving();
        Context::from_ptr(sb::C_Context_MakeVulkan(
            backend_context.native.as_ptr() as _,
            options.native(),
        ))
    }

    pub fn make_recorder(
        &mut self,
        options: Option<&crate::gpu::graphite::RecorderOptions>,
    ) -> Option<crate::gpu::graphite::Recorder> {
        unsafe {
            crate::gpu::graphite::Recorder::from_ptr(sb::C_Context_makeRecorder(
                self.native_mut(),
                options.native_ptr_or_null(),
            ))
        }
    }

    pub fn insert_recording(&mut self, recording: crate::gpu::graphite::Recording) -> bool {
        unsafe { sb::C_Context_insertRecording(self.native_mut(), recording.into_native()) }
    }

    pub fn submit(&mut self, sync_to_cpu: Option<SyncToCpu>) {
        unsafe {
            sb::C_Context_submit(
                self.native_mut(),
                sync_to_cpu.unwrap_or(SyncToCpu::No).into_native(),
            )
        }
    }
}
