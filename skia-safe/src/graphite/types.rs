use crate::prelude::*;
use skia_bindings as sb;

// Re-export backend types from skia_bindings
pub use sb::skgpu_BackendApi as BackendApi;
pub use sb::skgpu_Budgeted as Budgeted;
pub use sb::skgpu_Mipmapped as Mipmapped;

/// Status of recording insertion
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum InsertStatus {
    /// Recording was successfully inserted
    Success = 0,
    /// Recording failed to insert
    Failure = 1,
}

impl From<i32> for InsertStatus {
    fn from(value: i32) -> Self {
        match value {
            0 => InsertStatus::Success,
            _ => InsertStatus::Failure,
        }
    }
}

/// Configuration for recorder creation
#[derive(Debug)]
pub struct RecorderOptions {
    inner: sb::skgpu_graphite_RecorderOptions,
}

impl Default for RecorderOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for RecorderOptions {
    fn drop(&mut self) {
        unsafe { sb::C_RecorderOptions_destruct(&mut self.inner) }
    }
}

impl RecorderOptions {
    /// Create new recorder options with the C++ defaults (e.g. a 256 MiB GPU
    /// budget). Placement-constructed rather than zero-initialized, because
    /// `RecorderOptions` has a non-trivial constructor and members (an `sk_sp`,
    /// a `std::optional`, and a non-zero default budget).
    pub fn new() -> Self {
        let inner = unsafe {
            let mut inner = std::mem::MaybeUninit::uninit();
            sb::C_RecorderOptions_Construct(inner.as_mut_ptr());
            inner.assume_init()
        };
        Self { inner }
    }

    pub(crate) fn native(&self) -> &sb::skgpu_graphite_RecorderOptions {
        &self.inner
    }

    #[allow(dead_code)]
    pub(crate) fn native_mut(&mut self) -> &mut sb::skgpu_graphite_RecorderOptions {
        &mut self.inner
    }
}

/// Information for inserting a recording into the context.
///
/// Borrows the [`Recording`](crate::graphite::Recording) it references (it stores
/// a raw `fRecording` pointer), so the borrow checker keeps the `Recording` alive
/// for as long as this info — and any `insert_recording` call using it — is in use.
#[derive(Debug)]
pub struct InsertRecordingInfo<'a> {
    inner: sb::skgpu_graphite_InsertRecordingInfo,
    _recording: std::marker::PhantomData<&'a crate::graphite::Recording>,
}

impl<'a> InsertRecordingInfo<'a> {
    /// Create insert recording info for a recording.
    pub fn new(recording: &'a crate::graphite::Recording) -> Self {
        // All `InsertRecordingInfo` fields are plain pointers / POD / `kSuccess`
        // (== 0) defaults, so zero-init is a valid default; we then point
        // `fRecording` at the borrowed recording.
        let mut inner: sb::skgpu_graphite_InsertRecordingInfo =
            unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
        inner.fRecording = recording.native() as *const _ as *mut _;
        Self {
            inner,
            _recording: std::marker::PhantomData,
        }
    }

    pub(crate) fn native(&self) -> &sb::skgpu_graphite_InsertRecordingInfo {
        &self.inner
    }
}

/// Information for submitting work to the GPU
#[derive(Debug)]
pub struct SubmitInfo {
    inner: sb::skgpu_graphite_SubmitInfo,
}

impl Default for SubmitInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl SubmitInfo {
    /// Create new submit info with default settings (no CPU sync).
    ///
    /// Every `SubmitInfo` field's zero value equals its C++ default
    /// (`SyncToCpu::kNo`, `MarkFrameBoundary::kNo`, `0`, null procs), so
    /// zero-init is a valid default here.
    pub fn new() -> Self {
        // Every field's zero value equals its C++ default, so a zeroed struct is
        // a valid default `SubmitInfo`.
        let inner = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
        Self { inner }
    }

    /// Submit info whose `fSync` is set so that `Context::submit` blocks until
    /// the submitted GPU work has completed (`SyncToCpu::kYes` when `sync`).
    pub fn with_sync_to_cpu(sync: bool) -> Self {
        let mut info = Self::new();
        info.inner.fSync = if sync {
            sb::skgpu_graphite_SyncToCpu::kYes
        } else {
            sb::skgpu_graphite_SyncToCpu::kNo
        };
        info
    }

    pub(crate) fn native(&self) -> &sb::skgpu_graphite_SubmitInfo {
        &self.inner
    }
}

/// Synchronization mode for GPU operations
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SyncToCpu {
    /// Don't wait for GPU completion
    No = 0,
    /// Wait for GPU operations to complete
    Yes = 1,
}

impl From<bool> for SyncToCpu {
    fn from(sync: bool) -> Self {
        if sync {
            SyncToCpu::Yes
        } else {
            SyncToCpu::No
        }
    }
}

impl From<SyncToCpu> for bool {
    fn from(sync: SyncToCpu) -> bool {
        match sync {
            SyncToCpu::Yes => true,
            SyncToCpu::No => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_status_conversion() {
        assert_eq!(InsertStatus::from(0), InsertStatus::Success);
        assert_eq!(InsertStatus::from(1), InsertStatus::Failure);
        assert_eq!(InsertStatus::from(999), InsertStatus::Failure);
    }

    #[test]
    fn test_sync_to_cpu_conversion() {
        assert_eq!(SyncToCpu::from(true), SyncToCpu::Yes);
        assert_eq!(SyncToCpu::from(false), SyncToCpu::No);
        assert!(bool::from(SyncToCpu::Yes));
        assert!(!bool::from(SyncToCpu::No));
    }

    #[test]
    fn test_recorder_options_creation() {
        let options = RecorderOptions::new();
        let _default_options = RecorderOptions::default();
        // Should not panic and should create valid options
        let _ = format!("{:?}", options);
    }

    #[test]
    fn test_submit_info_creation() {
        let info = SubmitInfo::new();
        let _default_info = SubmitInfo::default();
        // Should not panic and should create valid info
        let _ = format!("{:?}", info);
    }
}
