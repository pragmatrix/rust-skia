use crate::build_support::{cargo, clang, skia::BuildConfiguration};

use super::{
    prelude::{Features, Target},
    BindgenArgsBuilder, GnArgsBuilder, PlatformDetails,
};

pub struct Catalyst;

const MIN_VER: &str = "14.0";

impl PlatformDetails for Catalyst {
    fn gn_args(&self, config: &BuildConfiguration, builder: &mut GnArgsBuilder) {
        builder.target(None);
    }

    fn bindgen_args(&self, target: &Target, builder: &mut BindgenArgsBuilder) {
        let target_str = Target {
            architecture: clang::target_arch(&target.architecture).to_string(),
            system: format!("ios{MIN_VER}"),
            ..target.clone()
        }
        .to_string();

        // cargo::add_link_arg(format!("-target {target_str}"));

        builder.target(target_str);
    }

    fn link_libraries(&self, features: &Features) -> Vec<String> {
        Vec::new()
    }
}
