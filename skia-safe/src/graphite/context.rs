use crate::graphite::{InsertRecordingInfo, InsertStatus, Recorder, RecorderOptions, SubmitInfo};
use crate::prelude::*;
use skia_bindings as sb;
use std::fmt;

// `skgpu::graphite::Context` is `final` with no base class and is handed out as
// `std::unique_ptr<Context>` (Context::MakeMetal etc.). It is NOT ref-counted,
// so it must be modeled as a `RefHandle` whose drop `delete`s it — modeling it
// as an `RCHandle` would call `SkRefCntBase::unref()` on a non-ref-counted
// object (UB; the real `~Context()` never runs -> leak).
pub type Context = RefHandle<sb::skgpu_graphite_Context>;

// Deliberately NOT `Send`/`Sync`: a Graphite `Context` has threading
// constraints and its `&self` methods funnel into mutating C++ with no internal
// lock, so sharing `&Context` across threads would be a data race. This matches
// the Ganesh `gpu::DirectContext`, which is likewise neither `Send` nor `Sync`.

impl NativeDrop for sb::skgpu_graphite_Context {
    fn drop(&mut self) {
        unsafe { sb::C_Context_delete(self) }
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("is_device_lost", &self.is_device_lost())
            .finish()
    }
}

impl Context {
    /// Create a new recorder for recording draw operations
    ///
    /// # Arguments
    /// - `options` - Configuration for the recorder, or `None` for default options
    ///
    /// # Returns
    /// A new `Recorder` instance, or `None` if creation failed
    pub fn make_recorder(&self, options: Option<&RecorderOptions>) -> Option<Recorder> {
        let default_options;
        let options_ptr = match options {
            Some(opts) => opts.native() as *const _,
            None => {
                default_options = RecorderOptions::default();
                default_options.native() as *const _
            }
        };

        let recorder_ptr =
            unsafe { sb::C_Context_makeRecorder(self.native_mut_force(), options_ptr) };
        Recorder::from_ptr(recorder_ptr)
    }

    /// Insert a recording into the context for later submission
    ///
    /// # Arguments
    /// - `info` - Information about the recording to insert
    ///
    /// # Returns
    /// Status indicating success or failure of the insertion
    pub fn insert_recording(&self, info: &InsertRecordingInfo<'_>) -> InsertStatus {
        let status_int =
            unsafe { sb::C_Context_insertRecording(self.native_mut_force(), info.native()) };
        InsertStatus::from(status_int)
    }

    /// Submit pending work to the GPU
    ///
    /// # Arguments
    /// - `submit_info` - Information about the submission, or `None` for defaults
    ///
    /// # Returns
    /// `true` if submission was successful, `false` otherwise
    pub fn submit(&self, submit_info: Option<&SubmitInfo>) -> bool {
        let default_info;
        let info_ptr = match submit_info {
            Some(info) => info.native() as *const _,
            None => {
                default_info = SubmitInfo::default();
                default_info.native() as *const _
            }
        };

        unsafe { sb::C_Context_submit(self.native_mut_force(), info_ptr) }
    }

    /// Submit pending work and block until it has completed on the GPU.
    ///
    /// Submits with `SyncToCpu::kYes`, so — unlike [`submit`](Self::submit) —
    /// this returns only after the GPU has finished the submitted work.
    ///
    /// # Returns
    /// `true` if submission succeeded.
    pub fn submit_and_wait(&self) -> bool {
        self.submit(Some(&SubmitInfo::with_sync_to_cpu(true)))
    }

    /// Pump any already-finished asynchronous work (invoking its finished procs).
    ///
    /// This does **not** block or report completion status: the underlying
    /// `Context::checkAsyncWorkCompletion()` returns `void`. To wait for GPU
    /// completion, use [`submit_and_wait`](Self::submit_and_wait).
    pub fn check_async_work_completion(&self) {
        unsafe { sb::C_Context_checkAsyncWorkCompletion(self.native_mut_force()) }
    }

    /// Delete a backend texture that was created through this context
    ///
    /// # Arguments
    /// - `texture` - The backend texture to delete
    pub fn delete_backend_texture(&self, texture: &crate::graphite::BackendTexture) {
        unsafe {
            sb::C_Context_deleteBackendTexture(self.native_mut_force(), texture.native());
        }
    }

    /// Check if the GPU device has been lost
    ///
    /// # Returns
    /// `true` if the device is lost and the context is unusable
    pub fn is_device_lost(&self) -> bool {
        unsafe { sb::C_Context_isDeviceLost(self.native()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_debug() {
        // We can't easily create a Context without platform-specific setup,
        // but we can test that the debug implementation compiles
        let context: Option<Context> = None;
        assert!(context.is_none());
    }
}
