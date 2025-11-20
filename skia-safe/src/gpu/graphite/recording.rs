use crate::prelude::*;
use skia_bindings as sb;
use std::fmt;

pub struct Recording(*mut sb::skgpu_graphite_Recording);

impl NativeAccess for Recording {
    type Native = sb::skgpu_graphite_Recording;

    fn native(&self) -> &Self::Native {
        unsafe { &*self.0 }
    }

    fn native_mut(&mut self) -> &mut Self::Native {
        unsafe { &mut *self.0 }
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        unsafe { sb::C_Recording_Destruct(self.0) }
    }
}

impl fmt::Debug for Recording {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Recording").finish()
    }
}

impl Recording {
    pub unsafe fn from_ptr(ptr: *mut sb::skgpu_graphite_Recording) -> Option<Recording> {
        if ptr.is_null() {
            None
        } else {
            Some(Recording(ptr))
        }
    }

    pub fn into_native(self) -> *mut sb::skgpu_graphite_Recording {
        let ptr = self.0;
        std::mem::forget(self);
        ptr
    }
}
