use crate::prelude::*;
use skia_bindings as sb;
use std::fmt;

pub struct Recorder(*mut sb::skgpu_graphite_Recorder);

impl NativeAccess for Recorder {
    type Native = sb::skgpu_graphite_Recorder;

    fn native(&self) -> &Self::Native {
        unsafe { &*self.0 }
    }

    fn native_mut(&mut self) -> &mut Self::Native {
        unsafe { &mut *self.0 }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        unsafe { sb::C_Recorder_Destruct(self.0) }
    }
}

impl fmt::Debug for Recorder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Recorder").finish()
    }
}

impl Recorder {
    pub unsafe fn from_ptr(ptr: *mut sb::skgpu_graphite_Recorder) -> Option<Recorder> {
        if ptr.is_null() {
            None
        } else {
            Some(Recorder(ptr))
        }
    }

    pub fn snap(&mut self) -> Option<crate::gpu::graphite::Recording> {
        unsafe {
            crate::gpu::graphite::Recording::from_ptr(sb::C_Recorder_snap(self.native_mut()))
        }
    }
}
