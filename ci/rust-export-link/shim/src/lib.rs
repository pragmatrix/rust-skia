//! Stands for `skia-wgpu`: defines the `wgpu*` functions that the C++ library of `bindings` calls.

use bindings::*;

/// The exports live in their own module, so that incremental builds put them into a codegen unit
/// apart from the entry point.
mod exports {
    use bindings::*;

    #[no_mangle]
    pub unsafe extern "C" fn wgpuDeviceCreateBuffer(
        _device: WGPUDevice,
        descriptor: *const WGPUBufferDescriptor,
    ) -> WGPUBuffer {
        Box::into_raw(Box::new(unsafe { (*descriptor).size })).cast()
    }

    #[cfg(not(feature = "omit-get-size"))]
    #[no_mangle]
    pub unsafe extern "C" fn wgpuBufferGetSize(buffer: WGPUBuffer) -> u64 {
        unsafe { *buffer.cast::<u64>() }
    }

    #[no_mangle]
    pub unsafe extern "C" fn wgpuBufferRelease(buffer: WGPUBuffer) {
        drop(unsafe { Box::from_raw(buffer.cast::<u64>()) });
    }

    // Each export is checked against its function pointer type from `webgpu.h`.
    const _: WGPUProcDeviceCreateBuffer = Some(wgpuDeviceCreateBuffer);
    #[cfg(not(feature = "omit-get-size"))]
    const _: WGPUProcBufferGetSize = Some(wgpuBufferGetSize);
    const _: WGPUProcBufferRelease = Some(wgpuBufferRelease);

    /// References every export, so that the linker must load them.
    #[cfg(feature = "keep")]
    pub fn keep_alive() {
        std::hint::black_box(wgpuDeviceCreateBuffer as *const ());
        #[cfg(not(feature = "omit-get-size"))]
        std::hint::black_box(wgpuBufferGetSize as *const ());
        std::hint::black_box(wgpuBufferRelease as *const ());
    }
}

pub struct Device(WGPUDevice);

impl Device {
    /// Stands for the entry point that `skia-safe` calls before Skia makes a WebGPU call.
    pub fn new() -> Self {
        #[cfg(feature = "keep")]
        exports::keep_alive();
        Device(std::ptr::dangling_mut::<u8>().cast())
    }

    pub fn raw(&self) -> WGPUDevice {
        self.0
    }
}
