//! Headless offscreen Graphite/Vulkan render — FFI soundness verification.
//!
//! Vulkan counterpart of `graphite_offscreen.rs`. The Graphite `Context` /
//! `Recorder` types are backend-agnostic, so this also exercises the
//! `RefHandle` ownership fix (delete vs. unref) on the Vulkan path. With an
//! iteration count > 1, a leak shows up as growing RSS:
//!   /usr/bin/time -v ./graphite_offscreen_vulkan 500
//!
//! Run (Linux): `cargo run --example graphite_offscreen_vulkan --features vulkan,graphite -- [iters] [reuse]`

#[cfg(not(all(feature = "vulkan", feature = "graphite")))]
fn main() {
    eprintln!("graphite_offscreen_vulkan requires --features vulkan,graphite");
}

#[cfg(all(feature = "vulkan", feature = "graphite"))]
fn main() {
    use ash::vk::{self, Handle};

    use skia_safe::{
        gpu,
        graphite::{self, vk as gvk},
        AlphaType, Color4f, ColorType, ImageInfo, Paint, Rect,
    };

    let iters: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let reuse = std::env::args().nth(2).as_deref() == Some("reuse");
    let verbose = iters == 1;

    // --- Minimal Vulkan instance / device / queue (offscreen, no surface) ---
    let entry = unsafe { ash::Entry::load().expect("failed to load Vulkan loader") };
    // Request a high core version so the loader exposes 1.2/1.3/1.4 core device
    // functions (Graphite needs 1.3 extended-dynamic-state + 1.4 host-image-copy
    // entry points; with 1.1 they resolve to null and VulkanInterface validation
    // fails). Capped to whatever the loader supports.
    let loader_version = unsafe { entry.try_enumerate_instance_version() }
        .ok()
        .flatten()
        .unwrap_or(vk::API_VERSION_1_1);
    let app_info = vk::ApplicationInfo::default().api_version(loader_version);
    let instance_ci = vk::InstanceCreateInfo::default().application_info(&app_info);
    let instance = unsafe {
        entry
            .create_instance(&instance_ci, None)
            .expect("create_instance")
    };

    let pdevices = unsafe {
        instance
            .enumerate_physical_devices()
            .expect("enumerate_physical_devices")
    };
    let pdevice = *pdevices.first().expect("no Vulkan physical device");

    let qfams = unsafe { instance.get_physical_device_queue_family_properties(pdevice) };
    let family_index = qfams
        .iter()
        .position(|q| q.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        .expect("no graphics queue family") as u32;

    let priorities = [1.0f32];
    let queue_cis = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(family_index)
        .queue_priorities(&priorities)];
    let device_ci = vk::DeviceCreateInfo::default().queue_create_infos(&queue_cis);
    let device = unsafe {
        instance
            .create_device(pdevice, &device_ci, None)
            .expect("create_device")
    };
    let queue = unsafe { device.get_device_queue(family_index, 0) };

    if verbose {
        let props = unsafe { instance.get_physical_device_properties(pdevice) };
        let name = unsafe { std::ffi::CStr::from_ptr(props.device_name.as_ptr()) };
        println!(
            "[ok] Vulkan device: {} (queue family {family_index})",
            name.to_string_lossy()
        );
    }

    // GetProc resolver bridging Skia -> ash. Borrows `entry` + `instance`,
    // which outlive every BackendContext created below.
    let resolver = |of: gpu::vk::GetProcOf| -> *const std::os::raw::c_void {
        unsafe {
            match of {
                gpu::vk::GetProcOf::Instance(inst, name) => entry
                    .get_instance_proc_addr(vk::Instance::from_raw(inst as u64), name)
                    .map_or(std::ptr::null(), |fp| fp as *const _),
                gpu::vk::GetProcOf::Device(dev, name) => instance
                    .get_device_proc_addr(vk::Device::from_raw(dev as u64), name)
                    .map_or(std::ptr::null(), |fp| fp as *const _),
            }
        }
    };

    let make_backend = || unsafe {
        gpu::vk::BackendContext::new_with_extensions(
            instance.handle().as_raw() as gpu::vk::Instance,
            pdevice.as_raw() as gpu::vk::PhysicalDevice,
            device.handle().as_raw() as gpu::vk::Device,
            (queue.as_raw() as gpu::vk::Queue, family_index as usize),
            &resolver,
            &[],
            &[],
        )
    };

    // reuse mode: one Context + Recorder for the whole run (the BackendContext
    // is only needed to build the Context and is dropped immediately after).
    let mut shared = if reuse {
        let backend = make_backend();
        let context = gvk::make_context(&backend, None).expect("make_context returned None");
        let recorder = context.make_recorder(None).expect("make_recorder returned None");
        Some((context, recorder))
    } else {
        None
    };

    for i in 0..iters {
        let mut fresh = if reuse {
            None
        } else {
            let backend = make_backend();
            let context = gvk::make_context(&backend, None).expect("make_context returned None");
            let recorder = context.make_recorder(None).expect("make_recorder returned None");
            Some((context, recorder))
        };

        let (context, recorder) = match (&mut shared, &mut fresh) {
            (Some((c, r)), _) | (_, Some((c, r))) => (c, r),
            _ => unreachable!(),
        };

        let image_info = ImageInfo::new((64, 64), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut surface = graphite::surfaces::render_target(
            recorder,
            &image_info,
            graphite::Mipmapped::No,
            None,
            Some("graphite_offscreen_vulkan_verify"),
        )
        .expect("render_target returned None");

        let canvas = surface.canvas();
        canvas.clear(Color4f::new(1.0, 1.0, 1.0, 1.0));
        canvas.draw_rect(
            Rect::from_xywh(8.0, 8.0, 48.0, 48.0),
            &Paint::new(Color4f::new(0.0, 0.0, 1.0, 1.0), None),
        );

        let recording = recorder.snap().expect("recorder.snap() returned None");
        let info = graphite::InsertRecordingInfo::new(&recording);
        let status = context.insert_recording(&info);
        let submitted = context.submit_and_wait();
        if verbose {
            println!("[ok] insert_recording={status:?} submit_and_wait={submitted}");
        }

        drop(recording);
        drop(surface);
        drop(fresh);

        if !verbose && (i + 1) % 100 == 0 {
            println!("iter {}/{}", i + 1, iters);
        }
    }

    drop(shared);
    println!("[PASS] {iters} iteration(s) (reuse={reuse}) completed without crash");

    // Peak resident set size (Linux), so a leak shows up as growth with `iters`
    // without depending on an external `time` binary.
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        if let Some(kb) = status.lines().find_map(|l| {
            l.strip_prefix("VmHWM:")
                .and_then(|r| r.split_whitespace().next())
                .and_then(|n| n.parse::<u64>().ok())
        }) {
            println!("[rss] peak VmHWM = {:.1} MB ({kb} kB)", kb as f64 / 1024.0);
        }
    }
}
