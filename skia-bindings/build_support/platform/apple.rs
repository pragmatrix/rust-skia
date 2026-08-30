use std::{
    path::{Path, PathBuf},
    process::Command,
};

use super::prelude::{BindgenArgsBuilder, Target, cargo};

pub fn use_sdk_libcxx(builder: &mut BindgenArgsBuilder, sdk: &Path) {
    // Bindgen may load a non-Apple libclang, whose default C++ headers take precedence over the
    // Apple SDK selected by -isysroot. Mixing those headers with the SDK can fail when Xcode and
    // libclang ship incompatible libc++ versions, so make Bindgen use the SDK's libc++ explicitly.
    builder.bindgen_only_arg("-nostdinc++");
    builder.bindgen_only_arg(format!("-isystem{}/usr/include/c++/v1", sdk.display()));
}

pub fn add_compiler_runtime(target: &Target) {
    let library = match target.as_strs() {
        (_, "apple", "darwin", _) | (_, "apple", "ios", Some("macabi")) => "clang_rt.osx",
        (_, "apple", "ios", Some("sim")) | ("x86_64", "apple", "ios", _) => "clang_rt.iossim",
        (_, "apple", "ios", _) => "clang_rt.ios",
        (_, "apple", "visionos", Some("sim")) => "clang_rt.xrossim",
        (_, "apple", "visionos", _) => "clang_rt.xros",
        _ => return,
    };

    let output = Command::new("xcrun")
        .args(["clang", "--print-resource-dir"])
        .output()
        .unwrap_or_else(|error| panic!("failed to locate the Clang runtime: {error}"));
    assert!(
        output.status.success(),
        "failed to locate the Clang runtime: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let path: PathBuf = [
        String::from_utf8(output.stdout)
            .expect("Clang runtime path is not valid UTF-8")
            .trim(),
        "lib",
        "darwin",
    ]
    .into_iter()
    .collect();
    assert!(
        path.is_dir(),
        "Clang runtime directory not found at {}",
        path.display()
    );
    cargo::add_native_link_search(path.to_str().unwrap());
    cargo::add_static_link_lib(target, library);
}
