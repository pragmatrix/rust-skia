#[cfg(feature = "graphite")]
#[test]
fn test_graphite_context() {
    // This test requires a backend (Metal, Vulkan, etc.) to run.
    // For now we just check if types are available.
    use skia_safe::gpu::graphite::{Context, Recorder, Recording};
    unsafe {
        let _ = Context::from_ptr(std::ptr::null_mut());
        let _ = Recorder::from_ptr(std::ptr::null_mut());
        let _ = Recording::from_ptr(std::ptr::null_mut());
    }
}
