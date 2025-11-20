use crate::prelude::*;
use skia_bindings as sb;
use std::fmt;

pub type Handle = sb::GrMTLHandle;

pub type BackendContext = crate::prelude::Handle<sb::skgpu_graphite_MtlBackendContext>;

impl NativeDrop for sb::skgpu_graphite_MtlBackendContext {
    fn drop(&mut self) {
        unsafe { sb::C_MtlBackendContext_Destruct(self) }
    }
}

impl fmt::Debug for BackendContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackendContext").finish()
    }
}

impl BackendContext {
    pub unsafe fn new(device: Handle, queue: Handle) -> Self {
        Self::construct(|bc| sb::C_MtlBackendContext_Construct(bc, device, queue))
    }
}
