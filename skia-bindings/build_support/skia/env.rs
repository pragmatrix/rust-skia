/// Environment variables used for configuring the Skia build.
use crate::build_support::cargo;
use std::path::PathBuf;

/// A boolean specifying whether to build Skia's dependencies or not. If not, the system's
/// provided libraries are used.
pub fn use_system_libraries() -> bool {
    cargo::env_var("SKIA_USE_SYSTEM_LIBRARIES").is_some()
}

/// The full path of the Bazel launcher to run. Defaults to `bazelisk` (which
/// honors the `.bazelversion` pinned by the Skia submodule) with a `bazel`
/// fallback. Override with `SKIA_BAZEL_COMMAND`.
pub fn bazel_command() -> Option<PathBuf> {
    cargo::env_var("SKIA_BAZEL_COMMAND").map(PathBuf::from)
}
