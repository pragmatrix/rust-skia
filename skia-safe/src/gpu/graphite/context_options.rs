use crate::prelude::*;
use skia_bindings as sb;

pub type ContextOptions = Handle<sb::skgpu_graphite_ContextOptions>;

impl NativeDrop for sb::skgpu_graphite_ContextOptions {
    fn drop(&mut self) {
        unsafe { sb::C_ContextOptions_Destruct(self) }
    }
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self::construct(|co| unsafe { sb::C_ContextOptions_Construct(co) })
    }
}

impl ContextOptions {
    // TODO: Add fields as needed
}
