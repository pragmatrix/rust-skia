use super::{generic, prelude::*};

pub struct Wasm32Wasip1Threads;

impl PlatformDetails for Wasm32Wasip1Threads {
    fn uses_freetype(&self, _config: &BuildConfiguration) -> bool {
        true
    }

    fn gn_args(&self, config: &BuildConfiguration, builder: &mut GnArgsBuilder) {
        let features = &config.features;

        generic::gn_args(config, builder);

        builder
            .arg("cc", quote(&format!("{}/bin/clang", wasi_sdk_base_dir())))
            .arg(
                "cxx",
                quote(&format!("{}/bin/clang++", wasi_sdk_base_dir())),
            )
            .arg("ar", quote(&format!("{}/bin/llvm-ar", wasi_sdk_base_dir())))
            .arg("skia_gl_standard", quote("webgl"))
            .arg("skia_use_webgl", yes_if(features.gpu()))
            .arg("target_cpu", quote("wasm"))
            .arg("skia_enable_fontmgr_custom_embedded", no())
            .arg("skia_enable_fontmgr_custom_empty", yes())
            .cflags(flags());
    }

    fn bindgen_args(&self, _target: &Target, builder: &mut BindgenArgsBuilder) {
        builder.args(flags());
    }

    fn link_libraries(&self, _features: &Features) -> Vec<String> {
        [
            "c++",
            "c++abi",
            "c++experimental",
            "c-printscan-long-double",
            "c-printscan-no-floating-point",
            "c",
            "crypt",
            "dl",
            "m",
            "pthread",
            "resolv",
            "rt",
            "setjmp",
            "util",
            "wasi-emulated-getpid",
            "wasi-emulated-mman",
            "wasi-emulated-process-clocks",
            "wasi-emulated-signal",
            "xnet",
            "clang_rt.builtins-wasm32",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }
}

fn flags() -> Vec<String> {
    let wasi_sdk_base_dir = wasi_sdk_base_dir();
    let emsdk_system_include = emsdk_system_include();
    [
        "-DSK_BUILD_FOR_UNIX",
        "-D__wasm32__",
        "-D__EMSCRIPTEN__",
        "-D_WASI_EMULATED_GETPID",
        "-mllvm",
        "-wasm-enable-sjlj",
        "-mtail-call",
        "-D_WASI_EMULATED_MMAN",
        "-pthread",
        "-fvisibility=default",
        "-Xclang -target-feature -Xclang +atomics",
        "-Xclang -target-feature -Xclang +bulk-memory",
        "-Xclang -target-feature -Xclang +mutable-globals",
        &format!("--sysroot=/{wasi_sdk_base_dir}/share/wasi-sysroot"),
        &format!("-I/{wasi_sdk_base_dir}/lib/clang/18/include"),
        &format!("-I{emsdk_system_include}"),
    ]
    .iter()
    .flat_map(|s| s.split_whitespace().map(|s| s.to_string()))
    .collect()
}

fn emsdk_system_include() -> String {
    match std::env::var("EMSDK_SYSTEM_INCLUDE") {
        Ok(val) => val,
        Err(_e) => panic!(
            "please set the EMSDK_SYSTEM_INCLUDE environment variable to the {{emsdk}}/system/include directory"
        ),
    }
}

fn wasi_sdk_base_dir() -> String {
    match std::env::var("WASI_SDK") {
        Ok(val) => val,
        Err(_e) => {
            panic!("please set the WASI_SDK environment variable to the root of your wasi-sdk")
        }
    }
}
