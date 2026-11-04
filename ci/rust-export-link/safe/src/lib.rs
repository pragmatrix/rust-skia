pub fn run_backend() -> u64 {
    let device = shim::Device::new();
    unsafe { bindings::C_Backend_Run(device.raw()) }
}
pub fn core() -> i32 {
    unsafe { bindings::C_Core() }
}
