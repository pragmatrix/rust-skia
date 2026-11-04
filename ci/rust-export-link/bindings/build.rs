//! Builds the C++ library `bindings` and links it the way `skia-bindings` links its library.

use std::env;

fn main() {
    cc::Build::new()
        .cpp(true)
        .files(["cpp/core.cpp", "cpp/backend.cpp"])
        .cargo_metadata(false)
        .compile("bindings");

    println!("cargo:rustc-link-search=native={}", env::var("OUT_DIR").unwrap());
    // `skia-bindings` links its library with `static=` on Unix, which bundles it into the rlib,
    // and without it on Windows (`build_support/cargo.rs`). `BUNDLE=0` selects the Windows way.
    if env::var("BUNDLE").as_deref() == Ok("0") {
        println!("cargo:rustc-link-lib=bindings");
    } else {
        println!("cargo:rustc-link-lib=static=bindings");
    }
    // MSVC links its C++ runtime by default. The other toolchains need it named.
    match (
        env::var("CARGO_CFG_TARGET_ENV").unwrap().as_str(),
        env::var("CARGO_CFG_TARGET_OS").unwrap().as_str(),
    ) {
        ("msvc", _) => {}
        (_, "macos") => println!("cargo:rustc-link-lib=c++"),
        _ => println!("cargo:rustc-link-lib=stdc++"),
    }
    println!("cargo:rerun-if-env-changed=BUNDLE");
    println!("cargo:rerun-if-changed=cpp");
}
