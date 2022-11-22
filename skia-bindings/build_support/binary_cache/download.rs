use super::{binaries, env, git, utils, SRC_BINDINGS_RS};
use crate::build_support::{binaries_config, cargo, platform};
use flate2::read::GzDecoder;
use std::{
    ffi::OsStr,
    fs,
    io::{self, Cursor},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

/// Resolve the `skia/` and `depot_tools/` subdirectory contents, either by checking out the
/// submodules, or when `build.rs` was invoked outside of the git repository by downloading and
/// unpacking them from GitHub.
pub fn resolve_dependencies() {
    if cargo::is_crate() {
        // In a crate.
        download_dependencies();
        return;
    }

    // Not in a crate, assuming a git repo. Update all submodules.
    assert!(
        Command::new("git")
            .args(["submodule", "update", "--init", "--depth", "1"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .unwrap()
            .success(),
        "`git submodule update` failed"
    );
}

/// Downloads the `skia` and `depot_tools` from their repositories.
///
/// The hashes are taken from the `Cargo.toml` section `[package.metadata]`.
fn download_dependencies() {
    let metadata = cargo::get_metadata();

    for dep in DEPENDENCIES {
        let repo_url = dep.url;
        let repo_name = dep.repo;

        let dir = PathBuf::from(repo_name);

        // Directory exists => assume that the download of the archive was successful.
        if dir.exists() {
            continue;
        }

        // hash available?
        let (_, short_hash) = metadata
            .iter()
            .find(|(n, _)| n == repo_name)
            .expect("metadata entry not found");

        // Remove unpacking directory, GitHub will format it to repo_name-hash
        let unpack_dir = &PathBuf::from(format!("{}-{}", repo_name, short_hash));
        if unpack_dir.is_dir() {
            fs::remove_dir_all(unpack_dir).unwrap();
        }

        // Download
        let archive_url = &format!("{}/{}", repo_url, short_hash);
        println!("DOWNLOADING: {}", archive_url);
        let archive = utils::download(archive_url)
            .unwrap_or_else(|err| panic!("Failed to download {} ({})", archive_url, err));

        // Unpack
        {
            let tar = GzDecoder::new(Cursor::new(archive));
            let mut archive = tar::Archive::new(tar);
            let dir = std::env::current_dir().unwrap();
            for entry in archive.entries().expect("failed to iterate over archive") {
                let mut entry = entry.unwrap();
                let path = entry.path().unwrap();
                let mut components = path.components();
                let root = components.next().unwrap();
                // skip pax headers.
                if root.as_os_str() == unpack_dir.as_os_str()
                    && (dep.path_filter)(components.as_path())
                {
                    entry.unpack_in(&dir).unwrap();
                }
            }
        }

        // Move unpack directory to the target repository directory
        fs::rename(unpack_dir, repo_name).expect("failed to move directory");
    }
}

// Specifies where to download Skia and Depot Tools archives from.
//
// Using `codeload.github.com`, otherwise the short hash will be expanded to a full hash as the root
// directory inside the `tar.gz`, and we run into filesystem path length restrictions with Skia.
struct Dependency {
    pub repo: &'static str,
    pub url: &'static str,
    pub path_filter: fn(&Path) -> bool,
}

const DEPENDENCIES: [Dependency; 2] = [
    Dependency {
        repo: "skia",
        url: "https://codeload.github.com/rust-skia/skia/tar.gz",
        path_filter: filter_skia,
    },
    Dependency {
        repo: "depot_tools",
        url: "https://codeload.github.com/rust-skia/depot_tools/tar.gz",
        path_filter: filter_depot_tools,
    },
];

// `infra/` contains very long filenames which may hit the max path restriction on Windows.
// <https://github.com/rust-skia/rust-skia/issues/169>
fn filter_skia(p: &Path) -> bool {
    !matches!(p.components().next(),
            Some(Component::Normal(name)) if name == OsStr::new("infra"))
}

// Need only `ninja` from `depot_tools/`.
// <https://github.com/rust-skia/rust-skia/pull/165>
fn filter_depot_tools(p: &Path) -> bool {
    p.to_str().unwrap().starts_with("ninja")
}

impl binaries_config::BinariesConfiguration {
    pub fn key(&self, repository_short_hash: &str) -> String {
        binaries::key(repository_short_hash, &self.feature_ids, self.skia_debug)
    }
}

/// Returns whether the prepared download needs to be built.
pub fn try_prepare_download(binaries_config: &binaries_config::BinariesConfiguration) -> bool {
    env::force_skia_build() || {
        let force_download = env::force_skia_binaries_download();
        if let Some((tag, key)) = should_try_download_binaries(binaries_config, force_download) {
            println!(
                "TRYING TO DOWNLOAD AND INSTALL SKIA BINARIES: {}/{}",
                tag, key
            );
            let url = binaries::download_url(
                env::skia_binaries_url().unwrap_or_else(env::skia_binaries_url_default),
                tag,
                key,
            );
            println!("  FROM: {}", url);
            if let Err(e) = download_and_install(url, &binaries_config.output_directory) {
                println!("DOWNLOAD AND INSTALL FAILED: {}", e);
                if force_download {
                    panic!("Downloading of binaries was forced but failed.")
                }
            
                if cargo::env_var("SKIA_EXP_FEATURE_UPGRADE").is_some() {
                    let target = cargo::target();
                    if let Some(upgraded) = platform::upgrade_features(&target, binaries_config.feature_ids) {
                        if let Some(features_available) = binaries_config.upgrade_features() {
                            println!("FEATURE UPGRADE:")
                            println!("  REQUESTED: {:?}", binaries_config.feature_ids);
                            println!("  UPGRADED: {:?}", upgraded);
                            
                            
                        }
    
                    } else {

                    }
                    
    
    

                }


                true
            } else {
                println!("DOWNLOAD AND INSTALL SUCCEEDED");
                false
            }
        } else {
            true
        }
    }
}

/// If the binaries should be downloaded, return the tag and key.
fn should_try_download_binaries(
    config: &binaries_config::BinariesConfiguration,
    force: bool,
) -> Option<(String, String)> {
    let tag = cargo::package_version();

    // For testing:
    if force {
        // Retrieve the hash from the repository above.
        let half_hash = git::half_hash()?;
        return Some((tag, config.key(&half_hash)));
    }

    // Building inside a crate?
    if let Ok(ref full_hash) = cargo::crate_repository_hash() {
        let half_hash = git::trim_hash(full_hash);
        return Some((tag, config.key(&half_hash)));
    }

    None
}

fn download_and_install(url: impl AsRef<str>, output_directory: &Path) -> io::Result<()> {
    let archive = utils::download(url)?;
    println!(
        "UNPACKING ARCHIVE INTO: {}",
        output_directory.to_str().unwrap()
    );
    binaries::unpack(Cursor::new(archive), output_directory)?;
    // TODO: Verify key?
    println!("INSTALLING BINDINGS");
    fs::copy(output_directory.join("bindings.rs"), SRC_BINDINGS_RS)?;

    Ok(())
}
