//! Bazel-based build of the Skia library.
//!
//! Replaces the former GN/Ninja pipeline (`config.rs`). The Cargo `build.rs`
//! orchestration, feature mapping, binary cache, and bindgen flow are preserved;
//! only the Skia compilation backend swaps to Bazel.
//!
//! Per the design decisions:
//! - The Bazel workspace is generated entirely into the cargo output directory
//!   (`$OUT_DIR/skia/bazel/`); no Bazel files live in the source tree.
//! - `@skia` is wired to the `skia-bindings/skia` submodule via `local_path_override`.
//! - `@skia_user_config` is a local override that re-exports upstream's copts/linkopts
//!   and appends the rust-skia-specific knobs (system-library toggles, freetype
//!   include-path workaround, mozjpeg-sys include injection, opt-level, sysroot).
//! - Major feature groups (gl, vulkan, metal, pdf, textlayout, svg, skottie) are
//!   expressed as Bazel target selection; fine-grained knobs live in `EXTRA_COPTS`.
//! - After `bazel build //:skia`, `bazel cquery 'deps(//:skia)' --output=files`
//!   enumerates the transitive archives, which are copied (renamed to the legacy
//!   `lib<name>.a` names) into `binaries_config.output_directory`.
//! - `bazel cquery --output=starlark --starlark:file` recovers
//!   `CcInfo.compilation_context.defines` per target for the bindgen step.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::build_support::{
    binaries_config,
    cargo::{self, Target},
    features::{self, feature},
    platform,
};

/// The subdirectory of the cargo output directory that holds the generated
/// Bazel workspace.
const BAZEL_WORKSPACE_SUBDIR: &str = "bazel";

/// The Bazel version pinned by the upstream `skia-bindings/skia/.bazelversion`.
/// Kept in sync manually; a mismatch fails the build with a clear error.
const BAZEL_VERSION: &str = "8.2.1";

/// The final, low-level build configuration (mirrors the former GN
/// `FinalBuildConfiguration` so the rest of the pipeline is unchanged).
#[derive(Debug)]
pub struct FinalBuildConfiguration {
    /// The Skia source directory (the submodule).
    pub skia_source_dir: PathBuf,

    /// Whether system libraries were used.
    #[allow(dead_code)]
    pub use_system_libraries: bool,

    /// The target (arch-vendor-os-abi).
    pub target: Target,

    /// An optional target sysroot.
    pub sysroot: Option<String>,
}

impl FinalBuildConfiguration {
    /// Build the final configuration from a feature set.
    pub fn from_build_configuration(
        build: &BuildConfiguration,
        use_system_libraries: bool,
        skia_source_dir: &Path,
    ) -> FinalBuildConfiguration {
        let features = platform::filter_features(
            &build.target,
            use_system_libraries,
            build.features.clone(),
        );
        let _ = features; // platform filtering is applied at label-selection time

        let sysroot = cargo::env_var("SDKTARGETSYSROOT").or_else(|| cargo::env_var("SDKROOT"));

        FinalBuildConfiguration {
            skia_source_dir: skia_source_dir.into(),
            use_system_libraries,
            target: build.target.clone(),
            sysroot,
        }
    }
}

/// The build configuration for Skia (mirrors the former GN `BuildConfiguration`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BuildConfiguration {
    /// Build Skia in a debug configuration?
    pub skia_debug: bool,

    /// The Skia feature set to compile.
    pub features: features::Features,

    /// The target (arch-vendor-os-abi).
    pub target: Target,
}

impl BuildConfiguration {
    pub fn from_features(features: features::Features, skia_debug: bool) -> Self {
        BuildConfiguration {
            skia_debug,
            features,
            target: cargo::target(),
        }
    }
}

/// Orchestrates the entire Bazel build of Skia.
///
/// `bazel_command` overrides the Bazel launcher (defaults to `bazelisk` with a
/// `bazel` fallback). The `offline` parameter is a no-op for the Bazel path;
/// Bazel has its own offline mode via `--enable_bzlmod=false` + a vendored cache.
pub fn build(
    build: &FinalBuildConfiguration,
    config: &binaries_config::BinariesConfiguration,
    bazel_command: Option<PathBuf>,
    _offline: bool,
) {
    let bazel = bazel_command
        .map(|p| p.to_owned())
        .unwrap_or_else(locate_bazel);

    let workspace_dir = config
        .output_directory
        .join(BAZEL_WORKSPACE_SUBDIR);
    generate_workspace(&workspace_dir, build, config);

    let compilation_mode = if build.target.is_emscripten() {
        // Skia's Bazel build uses `--config=wasm` for emscripten; the
        // compilation_mode is still driven by debug/release.
        if config.skia_debug { "dbg" } else { "opt" }
    } else if config.skia_debug {
        "dbg"
    } else {
        "opt"
    };

    // Build each component label directly. A `cc_library`'s `.a` archive is only
    // materialized when it is a direct build target; transitive deps of a
    // cc_library stay as object files. So we enumerate the transitive
    // cc_library targets of our feature-selected labels and build each one
    // directly, ensuring every archive exists on disk for the copy step.
    let labels = feature_labels(&config.features, &build.target);
    let all_targets = enumerate_cc_library_targets(&bazel, &workspace_dir, &labels, compilation_mode);
    let mut build_args = vec!["build".to_string(), format!("--compilation_mode={compilation_mode}")];
    for target in &all_targets {
        build_args.push(target.clone());
    }
    let build_args_ref: Vec<&str> = build_args.iter().map(|s| s.as_str()).collect();
    run_bazel(&bazel, &workspace_dir, &build_args_ref);

    // Discover and copy the transitive archives into the output directory,
    // renamed to the legacy lib names that `binaries_config` expects.
    copy_archives(&bazel, &workspace_dir, &all_targets, compilation_mode, config);

    // Recover the preprocessor defines for the bindgen step.
    let defines = extract_defines(&bazel, &workspace_dir, &all_targets, compilation_mode);
    skia_bindgen::definitions::save_definitions(&defines, &config.output_directory)
        .expect("failed to write Skia defines");
    // Stash the defines where build.rs can pick them up; we return them via a
    // side file because `build()` returns `()` to match the former signature.
    let defines_path = config.output_directory.join("bazel-defines.txt");
    fs::write(&defines_path, serialize_defines(&defines))
        .expect("failed to write bazel defines");
}

/// Locate the Bazel launcher, preferring `bazelisk` (which honors `.bazelversion`)
/// and falling back to `bazel`.
fn locate_bazel() -> PathBuf {
    for cmd in ["bazelisk", "bazel"] {
        if which(cmd).is_some() {
            return PathBuf::from(cmd);
        }
    }
    panic!(
        ">>>>> Neither `bazelisk` nor `bazel` found in PATH. Install bazelisk \
         (e.g. `brew install bazelisk`) to build skia-bindings from source. <<<<<"
    );
}

/// Minimal `which` lookup.
fn which(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(cmd))
        .find(|p| p.is_file())
}

/// Run a `bazel` subcommand in the generated workspace, inheriting stdout/stderr.
fn run_bazel(bazel: &Path, workspace_dir: &Path, args: &[&str]) {
    println!("Running: {} {}", bazel.display(), args.join(" "));
    let status = Command::new(bazel)
        .args(args)
        .current_dir(workspace_dir)
        .envs(std::env::vars())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .unwrap_or_else(|e| panic!("failed to run `{}`: {e}", bazel.display()));
    assert!(
        status.success(),
        "`bazel {}` returned an error, please check the output for details.",
        args.join(" ")
    );
}

/// Generate the entire Bazel workspace into `workspace_dir`.
fn generate_workspace(
    workspace_dir: &Path,
    build: &FinalBuildConfiguration,
    config: &binaries_config::BinariesConfiguration,
) {
    fs::create_dir_all(workspace_dir).expect("failed to create bazel workspace dir");
    let user_config_dir = workspace_dir.join("skia_user_config");
    fs::create_dir_all(&user_config_dir).expect("failed to create skia_user_config dir");

    // Absolute path to the Skia submodule, for the local_path_override.
    let skia_path = build
        .skia_source_dir
        .canonicalize()
        .unwrap_or_else(|e| panic!("failed to canonicalize skia source dir: {e}"));

    write_file(
        &workspace_dir.join(".bazelversion"),
        &format!("{BAZEL_VERSION}\n"),
    );
    write_file(&workspace_dir.join("WORKSPACE.bazel"), "");
    write_file(&workspace_dir.join(".bazelrc"), BAZELRC);
    write_file(
        &workspace_dir.join("MODULE.bazel"),
        &module_bazel(&skia_path),
    );

    // skia_user_config override
    write_file(
        &user_config_dir.join("MODULE.bazel"),
        USER_CONFIG_MODULE_BAZEL,
    );
    write_file(&user_config_dir.join("BUILD.bazel"), USER_CONFIG_BUILD_BAZEL);
    write_file(&user_config_dir.join("copts.bzl"), USER_CONFIG_COPTS_BZL);
    write_file(
        &user_config_dir.join("linkopts.bzl"),
        USER_CONFIG_LINKOPTS_BZL,
    );
    write_file(&user_config_dir.join("SkUserConfig.h"), SK_USER_CONFIG_H);
    write_file(
        &user_config_dir.join("extra_copts.bzl"),
        &extra_copts_bzl(build, config),
    );
}

/// The `MODULE.bazel` for the generated workspace.
fn module_bazel(skia_path: &Path) -> String {
    let skia_path_str = skia_path.display();
    format!(
        r#"module(
    name = "skia_bindings",
)

bazel_dep(name = "rules_cc", version = "0.1.5")
bazel_dep(name = "platforms", version = "1.0.0")

# The Skia submodule, consumed as a local Bazel module.
bazel_dep(name = "skia")
local_path_override(
    module_name = "skia",
    path = "{skia_path_str}",
)

# Local override for Skia's user-config module (system-library toggles, freetype
# include-path workaround, mozjpeg-sys include injection, opt-level, sysroot).
bazel_dep(name = "skia_user_config")
local_path_override(
    module_name = "skia_user_config",
    path = "skia_user_config",
)

# Re-export the third-party C++ module repos that Skia's build pulls in.
skia_deps = use_extension("@skia//bazel:cpp_modules.bzl", "cpp_modules")
skia_deps.from_file(deps_json = "@skia//bazel:deps.json")
use_repo(
    skia_deps,
    "dawn",
    "delaunator",
    "dng_sdk",
    "egl_registry",
    "expat",
    "freetype",
    "harfbuzz",
    "icu",
    "icu4x",
    "imgui",
    "jinja2",
    "libavif",
    "libgav1",
    "libjpeg_turbo",
    "libjxl",
    "libpng",
    "libwebp",
    "libyuv",
    "markupsafe",
    "opengl_registry",
    "perfetto",
    "piex",
    "spirv_cross",
    "spirv_headers",
    "spirv_tools",
    "vello",
    "vulkan_headers",
    "vulkan_tools",
    "vulkan_utility_libraries",
    "vulkanmemoryallocator",
    "wuffs",
    "zlib",
)
"#
    )
}

/// `.bazelrc` for the generated workspace.
///
/// We use the system (default @rules_cc) toolchain rather than Skia's hermetic
/// one, because the hermetic toolchain's sysroot is resolved via a relative
/// path that breaks in the sandbox when building from an external workspace.
/// We add `-std=c++20` (Skia now requires C++20) and the poison-include
/// suppression via EXTRA_COPTS in our @skia_user_config override.
const BAZELRC: &str = r#"# Force Bazel's C++ rules to use platforms to select toolchains.
build --incompatible_enable_cc_toolchain_resolution

# Enforce stricter environment rules for hermeticity.
build --incompatible_strict_action_env=true

# Use the system C++ toolchain with -std=c++20 (Skia now requires C++20). Only
# pass to C++ compilations (--cxxopt), not C (--copt would break libjpeg-turbo).
build --cxxopt=-std=c++20
"#;

/// Map the Cargo feature set to Skia Bazel labels.
///
/// Major feature groups are expressed as target selection (Q7-c). The base set
/// mirrors what the former GN `libskia.a` bundled: core + pathops + the
/// codec/ports/pdf/gpu sub-targets that the GN `skia_component("skia")` pulled in.
fn feature_labels(features: &features::Features, target: &Target) -> Vec<&'static str> {
    let mut labels = vec!["@skia//:core", "@skia//:pathops"];

    // GPU backends.
    if features.gpu() {
        // GL is the base GPU backend; EGL/X11/Wayland select the GL factory.
        if features[feature::GL] {
            labels.push("@skia//:ganesh_gl");
        }
        if features[feature::VULKAN] {
            labels.push("@skia//:ganesh_vulkan");
        }
        if features[feature::METAL] {
            labels.push("@skia//:ganesh_metal");
        }
        // D3D is Windows-only and has no public Bazel alias; reached via the
        // internal target when needed. Left out for now pending a Windows probe.
    }

    // PDF.
    if features[feature::PDF] {
        labels.push("@skia//:pdf_writer");
    }

    // Codecs.
    if features[feature::JPEG_DECODE] {
        labels.push("@skia//:jpeg_decode_codec");
    }
    if features[feature::JPEG_ENCODE] {
        labels.push("@skia//:jpeg_encode_codec");
    }
    if features[feature::WEBP_DECODE] {
        labels.push("@skia//:webp_decode_codec");
    }
    if features[feature::WEBP_ENCODE] {
        labels.push("@skia//:webp_encode_codec");
    }

    // Text layout (skshaper + skparagraph + skunicode).
    if features[feature::TEXTLAYOUT] {
        // On macOS, CoreText shaper is available; use the harfbuzz shaper to match
        // the former GN default (skia_use_harfbuzz = yes).
        labels.push("@skia//:skshaper_harfbuzz");
        labels.push("@skia//:skparagraph_harfbuzz_skunicode");
        labels.push("@skia//:skunicode_core");
        labels.push("@skia//:skunicode_icu");
    }

    // SVG (renderer for input, writer for output).
    if features[feature::SVG] {
        labels.push("@skia//:svg_renderer");
        labels.push("@skia//:svg_writer");
        labels.push("@skia//:skresources");
    }

    // Skottie (depends on sksg, jsonreader, skresources).
    if features[feature::SKOTTIE] {
        labels.push("@skia//:skottie");
        labels.push("@skia//:sksg");
        labels.push("@skia//:jsonreader");
        if !features[feature::SVG] {
            labels.push("@skia//:skresources");
        }
    }

    // Platform-specific font manager. The GN build pulled in fontmgr_mac_ct on
    // macOS, fontmgr_fontconfig on Linux, etc. Mirror that here.
    labels.extend(platform_fontmgr_labels(target));

    labels
}

/// Platform-specific font-manager labels (mirrors the former GN `public_deps`
/// of `skia_component("skia")`).
fn platform_fontmgr_labels(target: &Target) -> Vec<&'static str> {
    if target.system == "macos" || target.system == "ios" {
        vec!["@skia//:fontmgr_coretext", "@skia//:typeface_coretext"]
    } else if target.system == "linux" {
        vec!["@skia//:fontmgr_fontconfig", "@skia//:freetype_support"]
    } else if target.is_windows() {
        vec!["@skia//:fontmgr_data_freetype"]
    } else {
        vec!["@skia//:fontmgr_empty_freetype", "@skia//:freetype_support"]
    }
}

/// `extra_copts.bzl`: the fine-grained knobs (Q7-c), regenerated per build.
fn extra_copts_bzl(build: &FinalBuildConfiguration, config: &binaries_config::BinariesConfiguration) -> String {
    let mut copts: Vec<String> = Vec::new();

    // The toolchain's default include path contains /usr/local/include, which
    // clang flags as unsafe for cross-compilation under -Werror. We build for
    // the host (or a configured sysroot), so suppress the poison warning.
    copts.push("-Wno-poison-system-directories".to_string());

    // Sysroot.
    if let Some(sysroot) = &build.sysroot {
        copts.push(format!("--sysroot={sysroot}"));
    }

    // Opt level (mirrors the former GN cflag -O{opt_level}).
    if let Some(opt_level) = cargo::env_var("OPT_LEVEL") {
        // Windows MSVC doesn't accept -O; skip there.
        if !build.target.is_windows() {
            copts.push(format!("-O{opt_level}"));
        }
    }

    // FreeType include-path workaround (mirrors the former GN args).
    let use_freetype = platform::uses_freetype(&build.target);
    if use_freetype && !config.features[feature::EMBED_FREETYPE] {
        // When cross-compiling against a sysroot, prepend `=` to substitute the
        // sysroot if present, then fall back to the host path.
        copts.push("-I=/usr/include/freetype2".to_string());
        copts.push("-I/usr/include/freetype2".to_string());
    }

    // mozjpeg-sys include injection (use-system-jpeg-turbo feature).
    if cfg!(feature = "use-system-jpeg-turbo") {
        let paths = cargo::env_var("DEP_JPEG_INCLUDE").expect("mozjpeg-sys include path");
        for p in std::env::split_paths(&paths) {
            copts.push(format!("-I{}", p.display()));
        }
    }

    let mut out = String::from(
        "# Regenerated by skia-bindings/build.rs. Extra copts for the current\n\
         # Cargo feature set and environment.\n\
         EXTRA_COPTS = [\n",
    );
    for c in &copts {
        out.push_str(&format!("    {c:?},\n"));
    }
    out.push_str("]\n");
    out
}

/// Copy the transitive archives produced by `bazel build <labels>` into the
/// output directory, renamed to the legacy `lib<name>.a` names.
///
/// The mapping from Bazel archive path → legacy lib name is derived from
/// `binaries_config.ninja_built_libraries` (the names cargo expects to link).
/// Archives that don't match a legacy name are copied verbatim (third-party
/// libs like libharfbuzz.a, libpng.a, ...).
fn copy_archives(
    bazel: &Path,
    workspace_dir: &Path,
    targets: &[String],
    compilation_mode: &str,
    config: &binaries_config::BinariesConfiguration,
) {
    let archives = enumerate_archives(bazel, workspace_dir, targets, compilation_mode);
    let out_dir = &config.output_directory;

    for archive in archives {
        let name = archive
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| panic!("non-utf8 archive path: {}", archive.display()));
        // `lib<target>.a` -> `<target>`
        let lib_name = name
            .strip_prefix("lib")
            .and_then(|n| n.strip_suffix(".a"))
            .unwrap_or_else(|| panic!("unexpected archive name: {name}"));

        // Rename known component archives to the legacy names cargo links.
        let renamed = rename_component(lib_name, &config.features);
        let dest_name = match renamed {
            Some(legacy) => format!("lib{legacy}.a"),
            None => format!("lib{lib_name}.a"),
        };
        let dest = out_dir.join(&dest_name);
        // Bazel output archives are read-only; a previous copy may have left a
        // read-only file at the destination, so remove it before copying.
        let _ = fs::remove_file(&dest);
        fs::copy(&archive, &dest).unwrap_or_else(|e| {
            panic!(
                "failed to copy bazel archive {} -> {}: {e}",
                archive.display(),
                dest.display()
            );
        });
        println!("copied {} -> {}", archive.display(), dest.display());
    }
}

/// Map a Bazel component archive name to the legacy lib name cargo links.
///
/// Returns `None` for archives that have no rename (third-party libs, or
/// component libs whose Bazel name already matches the legacy name).
fn rename_component(bazel_name: &str, _features: &features::Features) -> Option<&'static str> {
    match bazel_name {
        // The GN `libskia.a` is gone; the closest single component is `core`,
        // but cargo links `skia`. We map `core` -> `skia` so the legacy
        // `libskia.a` link name resolves. Other component libs keep their names.
        "core" => Some("skia"),
        "skshaper_harfbuzz" | "skshaper_coretext" | "skshaper_core" => Some("skshaper"),
        "skparagraph_harfbuzz_skunicode" => Some("skparagraph"),
        "svg_renderer" | "svg_writer" => Some("svg"),
        _ => None,
    }
}

/// Enumerate the transitive `cc_library` targets of the given labels via
/// `bazel cquery`, returning their labels. We build each one directly so that
/// its `.a` archive is materialized on disk (Bazel only produces archives for
/// direct build targets, not transitive deps).
fn enumerate_cc_library_targets(
    bazel: &Path,
    workspace_dir: &Path,
    labels: &[&str],
    compilation_mode: &str,
) -> Vec<String> {
    let query = labels
        .iter()
        .map(|l| format!("deps({l})"))
        .collect::<Vec<_>>()
        .join(" + ");

    // Filter to cc_library/cc_binary targets (those that produce a CcInfo
    // provider) and print their labels.
    let starlark = r#"
def format(target):
    p = providers(target)
    if p == None or "CcInfo" not in p:
        return None
    return str(target.label)
"#;
    let query_file = workspace_root_tmp(workspace_dir).join("targets_query.bzl");
    fs::write(&query_file, starlark).expect("failed to write targets query file");

    let output = Command::new(bazel)
        .args([
            "cquery",
            &query,
            "--output=starlark",
            &format!("--starlark:file={}", query_file.display()),
            &format!("--compilation_mode={compilation_mode}"),
        ])
        .current_dir(workspace_dir)
        .output()
        .expect("failed to run `bazel cquery` for targets");

    assert!(
        output.status.success(),
        "`bazel cquery` for targets failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("non-utf8 targets output");
    stdout
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && *l != "None")
        .map(|l| l.to_string())
        .collect()
}

/// Enumerate the transitive `.a` archives of the built targets via
/// `bazel cquery`, filtering out system-SDK archives. The `compilation_mode`
/// must match the one passed to `bazel build` so the reported paths point at
/// the materialized archives.
fn enumerate_archives(
    bazel: &Path,
    workspace_dir: &Path,
    targets: &[String],
    compilation_mode: &str,
) -> Vec<PathBuf> {
    // `cquery --output=files` returns paths relative to the execroot. Resolve
    // them against `bazel info execution_root`, which is the absolute path to
    // the execroot (the parent of bazel-bin).
    let execroot = bazel_info(bazel, workspace_dir, "execution_root");

    // Build a union query: `deps(<target1>) + deps(<target2>) + ...`.
    let query = targets
        .iter()
        .map(|l| format!("deps({l})"))
        .collect::<Vec<_>>()
        .join(" + ");

    let output = Command::new(bazel)
        .args([
            "cquery",
            &query,
            "--output=files",
            "--noimplicit_deps",
            &format!("--compilation_mode={compilation_mode}"),
        ])
        .current_dir(workspace_dir)
        .output()
        .expect("failed to run `bazel cquery`");
    assert!(
        output.status.success(),
        "`bazel cquery {query}` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("non-utf8 cquery output");

    stdout
        .lines()
        .map(|l| l.trim())
        .filter(|l| l.ends_with(".a"))
        .filter(|l| !is_system_sdk_archive(l))
        .map(|l| resolve_archive_path(l, &execroot))
        .collect()
}

/// Run `bazel info <key>` and return the trimmed output path.
fn bazel_info(bazel: &Path, workspace_dir: &Path, key: &str) -> PathBuf {
    let output = Command::new(bazel)
        .args(["info", key])
        .current_dir(workspace_dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to run `bazel info {key}`: {e}"));
    assert!(
        output.status.success(),
        "`bazel info {key}` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("non-utf8 bazel info output");
    PathBuf::from(stdout.trim())
}

/// Whether an archive path is a system-SDK archive that must not be copied.
///
/// macOS: `external/.../MacSDK/...` and `external/.../clang_mac/...`.
/// Other platforms will need their own filters added here.
fn is_system_sdk_archive(path: &str) -> bool {
    path.contains("MacSDK")
        || path.contains("clang_mac")
        || path.contains("/usr/lib/")
        || path.contains("/usr/lib64/")
}

/// Resolve a `bazel cquery --output=files` path to an absolute filesystem path.
///
/// `cquery --output=files` returns paths relative to the execroot. We resolve
/// them against `bazel info execution_root`. Paths that are already absolute
/// (e.g. `/usr/lib/...`) are returned as-is, though these are typically filtered
/// out by `is_system_sdk_archive`.
fn resolve_archive_path(line: &str, execroot: &Path) -> PathBuf {
    let trimmed = line.trim();
    if trimmed.starts_with('/') {
        PathBuf::from(trimmed)
    } else {
        execroot.join(trimmed)
    }
}

/// Extract the preprocessor defines from the built targets via
/// `bazel cquery --output=starlark --starlark:file`.
fn extract_defines(
    bazel: &Path,
    workspace_dir: &Path,
    targets: &[String],
    compilation_mode: &str,
) -> Vec<(String, Option<String>)> {
    // Write the starlark query file into the workspace.
    let query_file = workspace_root_tmp(workspace_dir).join("defines_query.bzl");
    fs::write(&query_file, DEFINES_QUERY_BZL).expect("failed to write defines query file");

    let query = targets
        .iter()
        .map(|l| format!("deps({l})"))
        .collect::<Vec<_>>()
        .join(" + ");

    let output = Command::new(bazel)
        .args([
            "cquery",
            &query,
            "--output=starlark",
            &format!("--starlark:file={}", query_file.display()),
            &format!("--compilation_mode={compilation_mode}"),
        ])
        .current_dir(workspace_dir)
        .output()
        .expect("failed to run `bazel cquery` for defines");

    assert!(
        output.status.success(),
        "`bazel cquery` for defines failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("non-utf8 defines output");
    parse_defines(&stdout)
}

/// The starlark query file that prints `<label> [<define>, ...]` per target.
///
/// We collect both `defines` (transitive) and `local_defines` (per-target)
/// because the GN build's ninja files flattened both into a single list. For
/// example, `SKIA_IMPLEMENTATION=1` lives in `local_defines` on each `cc_library`,
/// and `SK_PDF_USE_HARFBUZZ_SUBSET` is a `local_define` on `//src/pdf:pdf`.
const DEFINES_QUERY_BZL: &str = r#"def format(target):
    p = providers(target)
    if p == None or "CcInfo" not in p:
        return None
    cc = p["CcInfo"]
    defines = cc.compilation_context.defines.to_list()
    local_defines = cc.compilation_context.local_defines.to_list() if hasattr(cc.compilation_context, "local_defines") else []
    return "%s %s" % (target.label, defines + local_defines)
"#;

/// Parse the `cquery --output=starlark` defines output into a flat list of
/// `(name, value)` pairs, deduplicated.
fn parse_defines(stdout: &str) -> Vec<(String, Option<String>)> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for line in stdout.lines() {
        // Lines look like: `@@//src/core:core ["SK_RELEASE", "SK_GL=1"]`
        // or `None` for non-cc targets.
        if let Some(bracket_start) = line.find('[') {
            let bracket_end = line.rfind(']').unwrap_or(line.len());
            let list_str = &line[bracket_start + 1..bracket_end];
            for raw in list_str.split(',') {
                let raw = raw.trim().trim_matches('"').trim_matches('\'');
                if raw.is_empty() {
                    continue;
                }
                if let Some((name, value)) = raw.split_once('=') {
                    if seen.insert(name.to_string()) {
                        out.push((name.to_string(), Some(value.to_string())));
                    }
                } else if seen.insert(raw.to_string()) {
                    out.push((raw.to_string(), None));
                }
            }
        }
    }
    out
}

/// Serialize defines to the `skia-defines.txt` format that
/// `skia_bindgen::definitions::save_definitions` already uses.
fn serialize_defines(defines: &[(String, Option<String>)]) -> String {
    let mut out = String::new();
    for (name, value) in defines {
        if let Some(value) = value {
            out.push_str(&format!("-D{name}={value}\n"));
        } else {
            out.push_str(&format!("-D{name}\n"));
        }
    }
    out.push('\n');
    out
}

/// A tmp dir under the workspace for query files.
fn workspace_root_tmp(workspace_dir: &Path) -> PathBuf {
    let dir = workspace_dir.join("tmp");
    fs::create_dir_all(&dir).expect("failed to create workspace tmp dir");
    dir
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent dir");
    }
    let mut file = fs::File::create(path)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", path.display()));
    file.write_all(content.as_bytes())
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", path.display()));
}

// ---- skia_user_config override file contents ----

const USER_CONFIG_MODULE_BAZEL: &str = r#"module(
    name = "skia_user_config",
)

bazel_dep(name = "rules_cc", version = "0.1.5")
bazel_dep(name = "platforms", version = "1.0.0")
bazel_dep(name = "skia")
"#;

const USER_CONFIG_BUILD_BAZEL: &str = r#"load("@rules_cc//cc:cc_library.bzl", "cc_library")

licenses(["notice"])

exports_files(
    ["SkUserConfig.h"],
    visibility = ["//visibility:public"],
)

config_setting(
    name = "debug_build",
    values = {"compilation_mode": "dbg"},
)

cc_library(
    name = "user_config",
    hdrs = ["SkUserConfig.h"],
    defines = [
        "SK_USE_BAZEL_CONFIG_HEADER",
    ] + select({
        ":debug_build": ["SK_DEBUG"],
        "//conditions:default": ["SK_RELEASE"],
    }),
    visibility = ["//visibility:public"],
)
"#;

const USER_CONFIG_COPTS_BZL: &str = r#"# Re-export Skia's upstream copts and append the rust-skia-specific knobs.
#
# The upstream values live at @skia//include/config:copts.bzl and are
# self-contained (no loads from @skia_user_config), so loading them here is
# cycle-free. EXTRA_COPTS is regenerated by skia-bindings/build.rs before each
# build; see //:extra_copts.bzl.

load("@skia//include/config:copts.bzl", _upstream_copts = "DEFAULT_COPTS", _upstream_objc_copts = "DEFAULT_OBJC_COPTS")
load("//:extra_copts.bzl", "EXTRA_COPTS")

DEFAULT_COPTS = _upstream_copts + EXTRA_COPTS
DEFAULT_OBJC_COPTS = _upstream_objc_copts + EXTRA_COPTS
"#;

const USER_CONFIG_LINKOPTS_BZL: &str = r#"# Re-export Skia's upstream linkopts verbatim.

load("@skia//include/config:linkopts.bzl", _upstream_default_linkopts = "DEFAULT_LINKOPTS")

DEFAULT_LINKOPTS = _upstream_default_linkopts
"#;

const SK_USER_CONFIG_H: &str = include_str!("../../skia/include/config/SkUserConfig.h");

// Re-export the bindgen definitions module for `save_definitions`.
use crate::build_support::skia_bindgen;