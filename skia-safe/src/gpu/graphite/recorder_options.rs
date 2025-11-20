use crate::prelude::*;
use skia_bindings as sb;

pub type RecorderOptions = Handle<sb::skgpu_graphite_RecorderOptions>;

impl NativeDrop for sb::skgpu_graphite_RecorderOptions {
    fn drop(&mut self) {
        unsafe { sb::C_RecorderOptions_Destruct(self) }
    }
}

impl Default for RecorderOptions {
    fn default() -> Self {
        Self::construct(|ro| unsafe { sb::C_RecorderOptions_Construct(ro) })
    }
}
