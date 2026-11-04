#[repr(C)]
pub struct WGPUBufferDescriptor {
    pub size: u64,
}
pub type WGPUDevice = *mut std::ffi::c_void;
pub type WGPUBuffer = *mut std::ffi::c_void;
// What bindgen generates from webgpu.h's WGPUProc* typedefs.
pub type WGPUProcDeviceCreateBuffer =
    Option<unsafe extern "C" fn(WGPUDevice, *const WGPUBufferDescriptor) -> WGPUBuffer>;
pub type WGPUProcBufferGetSize = Option<unsafe extern "C" fn(WGPUBuffer) -> u64>;
pub type WGPUProcBufferRelease = Option<unsafe extern "C" fn(WGPUBuffer)>;

extern "C" {
    pub fn C_Core() -> i32;
    pub fn C_Backend_Run(device: WGPUDevice) -> u64;
}
