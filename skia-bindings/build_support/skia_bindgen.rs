//! Full build support for the SkiaBindings library, and bindings.rs file.
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use bindgen::{CodegenConfig, EnumVariation};
use cc::Build;

use crate::build_support::{
    binaries_config,
    cargo::{self, Target},
    features,
    platform::{self, prelude::feature},
    type_bindings,
};

// 20: Since m143
const CPP_VERSION: &str = "20";

pub mod env {
    use crate::build_support::cargo;

    pub fn skia_lib_definitions() -> Option<String> {
        cargo::env_var("SKIA_BUILD_DEFINES")
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Configuration {
    /// The features active.
    pub features: features::Features,

    /// The binding source files to compile.
    pub binding_sources: Vec<PathBuf>,

    /// The Skia source directory.
    pub skia_source_dir: PathBuf,

    /// Further definitions needed for build consistency.
    pub definitions: Definitions,
}

impl Configuration {
    pub fn new(
        features: &features::Features,
        definitions: Definitions,
        skia_source_dir: &Path,
    ) -> Self {
        let binding_sources = {
            let mut sources: Vec<PathBuf> = vec!["src/bindings.cpp".into()];
            if features[feature::GL] {
                sources.push("src/gl.cpp".into());
            }
            if features[feature::EGL] {
                sources.push("src/egl.cpp".into());
            }
            if features[feature::VULKAN] {
                sources.push("src/vulkan.cpp".into());
            }
            if features[feature::METAL] {
                sources.push("src/metal.cpp".into());
            }
            if features[feature::D3D] {
                sources.push("src/d3d.cpp".into());
            }
            if features.has_gpu_engine() {
                sources.push("src/gpu.cpp".into());
            }
            if features.ganesh() {
                sources.push("src/ganesh.cpp".into());
            }
            if features.graphite() {
                sources.push("src/graphite.cpp".into());
            }
            if features[feature::TEXTLAYOUT] {
                sources.extend(vec!["src/shaper.cpp".into(), "src/paragraph.cpp".into()]);
            }
            if features[feature::SVG] {
                sources.push("src/svg.cpp".into());
            }
            if features[feature::SKOTTIE] {
                sources.push("src/skottie.cpp".into());
            }
            if features[feature::SVG] || features[feature::SKOTTIE] {
                sources.push("src/skresources.cpp".into());
            }
            if features[feature::WEBP_ENCODE] {
                sources.push("src/webp-encode.cpp".into());
            }
            sources
        };

        Self {
            features: features.clone(),
            skia_source_dir: skia_source_dir.into(),
            binding_sources,
            definitions,
        }
    }
}

pub fn generate_bindings(
    build: &Configuration,
    output_directory: &Path,
    target: Target,
    sysroot: Option<&str>,
) {
    let unclassified = Rc::new(RefCell::new(Vec::new()));
    let matched_entries = Rc::new(RefCell::new(vec![false; type_bindings::entry_count()]));
    let mut builder = bindgen::Builder::default()
        .generate_comments(false)
        .layout_tests(true)
        .default_enum_style(EnumVariation::Rust {
            non_exhaustive: false,
        })
        .size_t_is_usize(true)
        .parse_callbacks(Box::new(ParseCallbacks {
            unclassified: unclassified.clone(),
            matched_entries: matched_entries.clone(),
        }))
        .allowlist_function("C_.*")
        .allowlist_recursively(false)
        .constified_enum(".*Mask")
        .constified_enum(".*Flags")
        .constified_enum(".*Bits")
        .constified_enum("SkCanvas_SaveLayerFlagsSet")
        .constified_enum("GrVkAlloc_Flag")
        .constified_enum("GrGLBackendState")
        // SkPathRef_Editor member functions are not used; the type itself
        // and the remaining masks (private Gr* types, SkUnicode, std::
        // templates, the Vk* reexports) are governed by the Type Bindings
        // Table below (ADR 0001).
        .blocklist_function("SkPathRef_Editor_Editor")
        .blocklist_function("GrRecordingContext_priv.*")
        .blocklist_function("GrDirectContext_priv.*")
        .blocklist_function("SkContext_priv.*")
        .blocklist_function("SkDeferredDisplayList_priv.*")
        .blocklist_function("SkVertices_priv.*")
        .blocklist_function("std::bitset_flip.*")
        // m91: These functions are not actually implemented.
        .blocklist_function("SkCustomTypefaceBuilder_setGlyph[123].*")
        // misc
        .allowlist_var("SK_Color.*")
        .allowlist_var("kAll_GrBackendState")
        .use_core()
        .clang_arg(format!("-std=c++{CPP_VERSION}"))
        .clang_args(&["-x", "c++"])
        .clang_arg("-v");

    // Don't generate destructors for Windows targets:
    // <https://github.com/rust-skia/rust-skia/issues/318>
    if target.is_windows() {
        builder = builder.with_codegen_config({
            let mut config = CodegenConfig::default();
            config.remove(CodegenConfig::DESTRUCTORS);
            config
        });
    }

    for function in ALLOWLISTED_FUNCTIONS {
        builder = builder.allowlist_function(function)
    }

    // OPAQUE_TYPES/BLOCKLISTED_TYPES are superseded by the Type Bindings
    // Table above. Their entries were migrated into it (see ADR 0001).

    // `std::basic_string` specializations resolve to `std_string` via the
    // ITEM_RENAMES below. The type is only ever referenced by pointer from
    // Rust (`interop/string.rs` reads it via `C_string_ptr_size`), so an
    // opaque stand-in definition suffices; generating the full
    // `std::basic_string` template would drag in the libc++ internals that
    // trip bindgen's opaque/phantom-field asserts.
    builder = builder
        .raw_line("pub struct std_string(u8, ::core::marker::PhantomData<u8>);");

    // Type Bindings Table (ADR 0001): with `allowlist_recursively(false)`,
    // only explicitly allowlisted types are generated. `Include` and
    // `Opaque` entries drive type allowlisting; `Stub` and `Exclude`
    // entries blocklist (Stubs additionally emit a `raw_line` stand-in).
    //
    // SKIA_TABLE_FILTER diagnostic: comma-separated entries; `!<name>`
    // *skips* the named entry (any action) — used by
    // build_support/tools/bisect_opaque.py to find table entries whose
    // generation panics bindgen.
    let filter: Option<Vec<String>> = std::env::var("SKIA_TABLE_FILTER")
        .ok()
        .map(|v| v.split(',').map(str::to_string).collect());

    for entry in type_bindings::entries() {
        let mut skip = false;
        if let Some(filter) = &filter {
            for f in filter {
                let (negated, pat) = match f.strip_prefix('!') {
                    Some(rest) => (true, rest),
                    None => (false, f.as_str()),
                };
                let matched = if negated {
                    !entry.name.contains(pat)
                } else {
                    entry.name.contains(pat)
                };
                if !matched {
                    skip = true;
                    break;
                }
            }
        }
        if skip {
            continue;
        }
        match entry.action() {
            type_bindings::TypeAction::Include => builder = builder.allowlist_type(entry.name),
            type_bindings::TypeAction::Opaque => {
                builder = builder.allowlist_type(entry.name);
                builder = builder.opaque_type(entry.name);
            }
            type_bindings::TypeAction::Stub => {
                builder = builder.blocklist_type(entry.name);
                builder = builder.raw_line(format!(
                    "pub enum {} {{}}",
                    entry.name.replace("::", "_").replace(".*", "")
                ));
            }
            type_bindings::TypeAction::StubGeneric => {
                // Template type: never enters the IR (its argument phantoms
                // would break the opaque-with-fields assertion); instead a
                // generic stand-in definition is injected so generated
                // `extern "C"` signatures referencing it compile. The
                // definition is a bindgen-shaped generic struct provided
                // by the table entry.
                builder = builder.blocklist_type(entry.name);
                builder = builder.raw_line(entry.definition);
            }
            type_bindings::TypeAction::Exclude => builder = builder.blocklist_type(entry.name),
        }
        if let Some(pattern) = entry.function_blocklist() {
            // Scan(Fields) verdict: the type's members are accessed but no
            // method is called, so blocklist all generated method wrappers
            // `{Name}_.+`. The hand-written `C_{Name}_*` wrappers carry the
            // `C_` prefix and cannot match the start-anchored pattern.
            builder = builder.blocklist_function(pattern);
        }
    }

    let mut cc_build = Build::new();

    for source in &build.binding_sources {
        cc_build.file(source);
        let source = source.to_str().unwrap();
        cargo::rerun_if_file_changed(source);
        builder = builder.header(source);
    }

    let mut bindgen_args = Vec::new();
    let mut cc_defines = Vec::new();
    let mut cc_args = Vec::new();

    let include_path = &build.skia_source_dir;
    cargo::rerun_if_file_changed(include_path.join("include"));

    bindgen_args.push(format!("-I{}", include_path.display()));
    cc_build.include(include_path);

    for (name, value) in &build.definitions {
        match value {
            Some(value) => {
                cc_defines.push((name, value.as_str()));
                bindgen_args.push(format!("-D{name}={value}"));
            }
            None => {
                cc_defines.push((name, ""));
                bindgen_args.push(format!("-D{name}"));
            }
        }
    }

    cc_build.cpp(true).out_dir(output_directory);

    {
        let std_cpp_arg = if target.builds_with_msvc() {
            format!("/std:c++{CPP_VERSION}")
        } else {
            format!("-std=c++{CPP_VERSION}")
        };
        cc_args.push(std_cpp_arg);
    }

    // Disable RTTI on non-MSVC targets. On MSVC, RTTI must stay enabled (/GR)
    // because std::function in <functional> uses typeid, which fails with /GR-.
    if target.builds_with_msvc() {
        cc_args.push("/GR".into());
    } else {
        bindgen_args.push("-fno-rtti".into());
        cc_args.push("-fno-rtti".into());
    }

    // Set platform specific arguments and flags and target.
    {
        let args = platform::bindgen_and_cc_args(&target, sysroot);

        bindgen_args.extend(args.bindgen_only_args);
        bindgen_args.extend(args.shared_args.clone());
        cc_args.extend(args.shared_args);

        let mut target_str = &target.to_string();
        let mut override_target = false;
        if let Some(target) = &args.target_override {
            target_str = target;
            override_target = true;
        }

        // If we use the target() function for override targets, cc will override it based on the
        // environment, for example when targeting the ios simulator.
        if override_target {
            cc_args.push(format!("--target={target_str}"));
        }
        bindgen_args.push(format!("--target={target_str}"));
    }

    {
        println!("COMPILING BINDINGS: {:?}", build.binding_sources);
        println!(
            "  DEFINES: {}",
            cc_defines
                .iter()
                .map(|(n, v)| format!("{n}={v}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        println!("  ARGS: {}", cc_args.join(" "));

        for (var, val) in cc_defines {
            cc_build.define(var, val);
        }

        for arg in cc_args {
            cc_build.flag(&arg);
        }

        // we add skia-bindings later on.
        cc_build.cargo_metadata(false);
        cc_build.compile(binaries_config::lib::SKIA_BINDINGS);
    }

    {
        println!("GENERATING BINDINGS");
        println!("  ARGS: {}", bindgen_args.join(" "));

        builder = builder.clang_args(bindgen_args);

        let bindings = builder.generate().expect("Unable to generate bindings");
        report_unclassified_types(&unclassified);
        report_unused_entries(&matched_entries);
        bindings
            .write_to_file(output_directory.join("bindings.rs"))
            .expect("Couldn't write bindings!");
    }
}

const ALLOWLISTED_FUNCTIONS: &[&str] = &[
    "SkAnnotateRectWithURL",
    "SkAnnotateNamedDestination",
    "SkAnnotateLinkToDestination",
    "SkColorTypeBytesPerPixel",
    "SkColorTypeIsAlwaysOpaque",
    "SkColorTypeValidateAlphaType",
    "SkRGBToHSV",
    "SkHSVToColor",
    "SkPreMultiplyARGB",
    "SkPreMultiplyColor",
    "SkBlendMode_AsCoeff",
    "SkBlendMode_Name",
    "SkSwapRB",
    // pathops/
    "Op",
    "Simplify",
    "TightBounds",
    "AsWinding",
    "SkYUVColorSpaceIsLimitedRange",
];

// OPAQUE_TYPES/BLOCKLISTED_TYPES were migrated into the
// Type Bindings Table (build_support/type_bindings.rs, ADR 0001).

#[derive(Debug)]
struct ParseCallbacks {
    /// Types discovered by bindgen that the Type Bindings Table does not
    /// classify. Collected during generation and reported (and failed on)
    /// afterwards so a milestone update shows the complete diff instead of
    /// one panic per type. Shared with the caller via `Rc`.
    unclassified: Rc<RefCell<Vec<String>>>,
    /// Indices of table entries that matched at least one discovered type
    /// (for the unused-entry report). Shared with the caller via `Rc`.
    matched_entries: Rc<RefCell<Vec<bool>>>,
}

impl bindgen::callbacks::ParseCallbacks for ParseCallbacks {
    /// Allows to rename an enum variant, replacing `_original_variant_name`.
    fn enum_variant_name(
        &self,
        enum_name: Option<&str>,
        original_variant_name: &str,
        _variant_value: bindgen::callbacks::EnumVariantValue,
    ) -> Option<String> {
        enum_name.and_then(|enum_name| {
            ENUM_REWRITES
                .iter()
                .find(|n| n.0 == enum_name)
                .map(|(_, replacer)| replacer(enum_name, original_variant_name))
        })
    }

    fn item_name(&self, original_item_name: bindgen::callbacks::ItemInfo<'_>) -> Option<String> {
        ITEM_RENAMES
            .iter()
            .find(|(original, _)| *original == original_item_name.name)
            .map(|(_, replacement)| replacement.to_string())
    }

    /// Opt-in enforcement (ADR 0001): every discovered type must be
    /// classified in the Type Bindings Table. Types missing from the table fail the build
    /// here, at the point of discovery, instead of surfacing later as link
    /// errors or wrong layouts.
    fn new_item_found(
        &self,
        _id: bindgen::callbacks::DiscoveredItemId,
        item: bindgen::callbacks::DiscoveredItem,
        _location: Option<&bindgen::callbacks::SourceLocation>,
    ) {
        // The table matches C++ names; anonymous types have no original
        // name and cannot be classified, but also cannot be referenced.
        let name = match &item {
            bindgen::callbacks::DiscoveredItem::Struct { original_name, .. }
            | bindgen::callbacks::DiscoveredItem::Union { original_name, .. } => {
                original_name.as_deref()
            }
            _ => None,
        };
        if let Some(name) = name {
            if let Some(index) = type_bindings::matched_entry_index(name) {
                self.matched_entries.borrow_mut()[index] = true;
            } else {
                self.unclassified.borrow_mut().push(name.into());
            }
        }
    }
}

/// Reports all types bindgen discovered that are missing from the
/// Type Bindings Table, and fails the build if there are any (ADR 0001).
fn report_unclassified_types(unclassified: &Rc<RefCell<Vec<String>>>) {
    let mut unclassified = unclassified.borrow_mut();
    if unclassified.is_empty() {
        return;
    }
    unclassified.sort_unstable();
    unclassified.dedup();
    panic!(
        "{} type(s) were discovered by bindgen but are not classified in the \
         Type Bindings Table (build_support/type_bindings.rs). \
         See docs/adr/0001-opt-in-binding-generation.md: add an entry with the \
         appropriate TypeAction for each of them, or exclude them from the closure:\n\
         {}",
        unclassified.len(),
        unclassified.join("\n")
    );
}

/// Analytics (ADR 0001): reports table entries that matched no type
/// discovered by bindgen in this build — candidates for removal, since
/// their allowlist/blocklist/no-op did nothing. Prints a one-line summary
/// always, and the full list when `SKIA_TABLE_UNUSED_REPORT=1`.
fn report_unused_entries(matched_entries: &Rc<RefCell<Vec<bool>>>) {
    let matched = matched_entries.borrow();
    let mut unused = Vec::new();
    for index in 0..matched.len() {
        if !matched[index] {
            unused.push(type_bindings::entry_name(index));
        }
    }
    if unused.is_empty() {
        eprintln!(
            "Type Bindings Table: all {} entries matched a discovered type.",
            matched.len()
        );
        return;
    }
    if std::env::var("SKIA_TABLE_UNUSED_REPORT").as_deref() == Ok("1") {
        eprintln!(
            "Type Bindings Table: {} of {} entries matched no discovered \
             type (candidates for removal):\n{}",
            unused.len(),
            matched.len(),
            unused.join("\n")
        );
    } else {
        eprintln!(
            "Type Bindings Table: {} of {} entries matched no discovered \
             type (rerun with SKIA_TABLE_UNUSED_REPORT=1 for the list).",
            unused.len(),
            matched.len()
        );
    }
}

type EnumEntry = (&'static str, fn(&str, &str) -> String);

const ITEM_RENAMES: &[(&str, &str)] = &[
    ("std___1_string_view", "std_string_view"),
    ("std___2_string_view", "std_string_view"),
    ("std___1_string", "std_string"),
    ("std___2_string", "std_string"),
];

const ENUM_REWRITES: &[EnumEntry] = &[
    // codec/
    ("DocumentStructureType", rewrite::k_xxx),
    ("ZeroInitialized", rewrite::k_xxx_name),
    ("SelectionPolicy", rewrite::k_xxx),
    // core/ effects/
    ("SkApplyPerspectiveClip", rewrite::k_xxx),
    ("SkBlendMode", rewrite::k_xxx),
    ("SkBlendModeCoeff", rewrite::k_xxx),
    ("SkBlurStyle", rewrite::k_xxx_name),
    ("SkClipOp", rewrite::k_xxx),
    ("SkColorChannel", rewrite::k_xxx),
    ("SkCoverageMode", rewrite::k_xxx),
    ("SkEncodedImageFormat", rewrite::k_xxx),
    ("SkEncodedOrigin", rewrite::k_xxx_name),
    ("SkFilterQuality", rewrite::k_xxx_name),
    ("SkFontHinting", rewrite::k_xxx),
    ("SkAlphaType", rewrite::k_xxx_name),
    ("SkYUVColorSpace", rewrite::k_xxx_name),
    ("SkPathFillType", rewrite::k_xxx),
    ("SkPathConvexityType", rewrite::k_xxx),
    ("SkPathDirection", rewrite::k_xxx),
    ("SkPathVerb", rewrite::k_xxx),
    ("SkPathOp", rewrite::k_xxx_name),
    ("SkTileMode", rewrite::k_xxx),
    // svg/
    ("Unit", rewrite::k_xxx),
    ("Scale", rewrite::k_xxx),
    ("SkSVGLineCap", rewrite::k_xxx),
    ("SkSVGXmlSpace", rewrite::k_xxx),
    ("SkSVGColorspace", rewrite::k_xxx),
    ("SkSVGDisplay", rewrite::k_xxx),
    ("SkSVGAttribute", rewrite::k_xxx),
    ("SkSVGTag", rewrite::k_xxx),
    ("LengthType", rewrite::k_xxx),
    // SkPaint_Style
    // SkStrokeRec_Style
    // SkPath1DPathEffect_Style
    ("Style", rewrite::k_xxx_name_opt),
    // SkPaint_Cap
    ("Cap", rewrite::k_xxx_name),
    // SkPaint_Join
    ("Join", rewrite::k_xxx_name),
    // SkStrokeRec_InitStyle
    ("InitStyle", rewrite::k_xxx_name),
    // SkBlurImageFilter_TileMode
    // SkMatrixConvolutionImageFilter_TileMode
    ("TileMode", rewrite::k_xxx_name),
    // SkCanvas_*
    ("PointMode", rewrite::k_xxx_name),
    ("SrcRectConstraint", rewrite::k_xxx_name),
    // SkCanvas_Lattice_RectType
    ("RectType", rewrite::k_xxx),
    // SkDisplacementMapEffect_ChannelSelectorType
    ("ChannelSelectorType", rewrite::k_xxx_name),
    // SkDropShadowImageFilter_ShadowMode
    ("ShadowMode", rewrite::k_xxx_name),
    // SkFont_Edging
    ("Edging", rewrite::k_xxx),
    // SkFont_Slant
    ("Slant", rewrite::k_xxx_name),
    // SkHighContrastConfig_InvertStyle
    ("InvertStyle", rewrite::k_xxx),
    // SkImage_*
    ("BitDepth", rewrite::k_xxx),
    ("CachingHint", rewrite::k_xxx_name),
    ("SkTextureCompressionType", rewrite::k_xxx),
    // SkImageFilter_MapDirection
    ("MapDirection", rewrite::k_xxx_name),
    // SkCodec_Result
    // SkInterpolatorBase_Result
    ("Result", rewrite::k_xxx),
    // SkMatrix_ScaleToFit
    ("ScaleToFit", rewrite::k_xxx_name),
    // SkPathBuilder_*
    ("ArcSize", rewrite::k_xxx_name),
    ("AddPathMode", rewrite::k_xxx_name),
    // SkPathBuilder_*
    ("DumpFormat", rewrite::k_xxx),
    // SkRegion_Op
    // TODO: remove kLastOp?
    ("Op", rewrite::k_xxx_name_opt),
    // SkRRect_*
    // TODO: remove kLastType?
    // SkRuntimeEffect_Uniform_Type
    ("Type", rewrite::k_xxx_name_opt),
    ("Corner", rewrite::k_xxx_name),
    // SkShader_GradientType
    ("GradientType", rewrite::k_xxx_name),
    // SkSurface_*
    ("ContentChangeMode", rewrite::k_xxx_name),
    ("BackendHandleAccess", rewrite::k_xxx),
    // SkTextUtils_Align
    // We need name_opt to cover SkSVGPreserveAspectRatio_Align
    ("Align", rewrite::k_xxx_name_opt),
    // SkTrimPathEffect_Mode
    ("Mode", rewrite::k_xxx),
    // SkTypeface_SerializeBehavior
    ("SerializeBehavior", rewrite::k_xxx),
    // SkVertices_VertexMode
    ("VertexMode", rewrite::k_xxx_name),
    // SkYUVAIndex_Index
    ("Index", rewrite::k_xxx_name),
    // SkRuntimeEffect_Variable_Qualifier
    ("Qualifier", rewrite::k_xxx),
    // private type that leaks through SkRuntimeEffect_Variable
    ("GrSLType", rewrite::k_xxx_name),
    // gpu/
    ("Origin", rewrite::k_xxx),
    ("GrGLStandard", rewrite::k_xxx_name),
    ("GrGLFormat", rewrite::k_xxx),
    ("GrSurfaceOrigin", rewrite::k_xxx_name),
    ("GrBackendApi", rewrite::k_xxx),
    ("Mipmapped", rewrite::k_xxx),
    ("Renderable", rewrite::k_xxx),
    ("Protected", rewrite::k_xxx),
    // DartTypes.h
    ("Affinity", rewrite::k_xxx),
    ("TextAlign", rewrite::k_xxx),
    ("TextDirection", rewrite::k_xxx_uppercase),
    ("TextBaseline", rewrite::k_xxx),
    ("TextHeightBehavior", rewrite::k_xxx),
    ("DrawOptions", rewrite::k_xxx),
    // TextStyle.h
    ("TextDecorationStyle", rewrite::k_xxx),
    ("TextDecorationMode", rewrite::k_xxx),
    ("StyleType", rewrite::k_xxx),
    // Vk*
    ("VkChromaLocation", rewrite::vk),
    ("VkFilter", rewrite::vk),
    ("VkFormat", rewrite::vk),
    ("VkImageLayout", rewrite::vk),
    ("VkImageTiling", rewrite::vk),
    ("VkSamplerYcbcrModelConversion", rewrite::vk),
    ("VkSamplerYcbcrRange", rewrite::vk),
    ("VkStructureType", rewrite::vk),
    // m84: SkPath::Verb
    ("Verb", rewrite::k_xxx_name),
    // m84: SkVertices::Attribute::Usage
    ("Usage", rewrite::k_xxx),
    ("GrSemaphoresSubmitted", rewrite::k_xxx),
    ("BackendSurfaceAccess", rewrite::k_xxx),
    // m85
    ("VkSharingMode", rewrite::vk),
    // m86:
    ("SkFilterMode", rewrite::k_xxx),
    ("SkMipmapMode", rewrite::k_xxx),
    ("Enable", rewrite::k_xxx),
    ("ShaderCacheStrategy", rewrite::k_xxx),
    // m87:
    // SkYUVAInfo_PlanarConfig
    ("PlanarConfig", rewrite::k_xxx),
    ("Siting", rewrite::k_xxx),
    // SkYUVAPixmapInfo
    ("DataType", rewrite::k_xxx),
    // m88:
    // SkYUVAInfo_*
    ("PlaneConfig", rewrite::k_xxx),
    // m89, SkImageFilters::Dither
    ("Dither", rewrite::k_xxx),
    ("SkScanlineOrder", rewrite::k_xxx_name),
    // m94: SkRuntimeEffect::ChildType
    ("ChildType", rewrite::k_xxx_name_opt),
    // m108: SkGradientShader::Interpolation::InPremul
    ("InPremul", rewrite::k_xxx),
    // m108: skgpu::BackendApi
    ("BackendApi", rewrite::k_xxx),
    // m109: SkGradientShader::Interpolation::ColorSpace
    ("ColorSpace", rewrite::k_xxx),
    // m109: SkGradientShader::Interpolation::HueMethod
    ("HueMethod", rewrite::k_xxx),
    // SkCodecAnimation
    ("DisposalMethod", rewrite::k_xxx),
    ("Blend", rewrite::k_xxx),
    // SkJpegEncoder.h
    ("AlphaOption", rewrite::k_xxx),
    // SkWebpEncoder.h
    ("Compression", rewrite::k_xxx),
    // m118:
    ("GrPurgeResourceOptions", rewrite::k_xxx),
    ("GrSyncCpu", rewrite::k_xxx),
    // m129:
    ("Clamp", rewrite::k_xxx), // SkColorFilters
    // svg:
    ("SkSVGFeColorMatrixType", rewrite::k_xxx),
    ("SkSVGFeCompositeOperator", rewrite::k_xxx),
    ("SkSVGFeFuncType", rewrite::k_xxx),
    // SkSVGFeMorphology::Operator
    ("Operator", rewrite::k_xxx),
    // m131:
    ("GrMarkFrameBoundary", rewrite::k_xxx),
    // SkResources.h
    ("ImageDecodeStrategy", rewrite::k_xxx),
    // SkNamedPrimaries::CicpId, SkNamedTransferFn::CicpId
    ("CicpId", rewrite::k_xxx),
    // `SkCodec::IsAnimated`s
    ("IsAnimated", rewrite::k_xxx),
    // m142: PngRustEncoder::CompressionLevel
    ("CompressionLevel", rewrite::k_opt_xxx),
    // m148: SkShapers::CT::LineBreakMode
    ("LineBreakMode", rewrite::k_xxx),
    // graphite: skgpu::graphite::InsertStatus::V (the class-enum migration
    // shim for Context::insertRecording's status)
    ("V", rewrite::k_xxx),
    // graphite: skgpu::graphite::SyncToCpu
    ("SyncToCpu", rewrite::k_xxx),
];

pub(crate) mod rewrite {
    use heck::ToShoutySnakeCase;
    use regex::Regex;

    pub fn k_xxx_uppercase(name: &str, variant: &str) -> String {
        k_xxx(name, variant).to_uppercase()
    }

    pub fn k_xxx(name: &str, variant: &str) -> String {
        if let Some(stripped) = variant.strip_prefix('k') {
            stripped.into()
        } else {
            panic!(
                "Variant name '{variant}' of enum type '{name}' is expected to start with a 'k'"
            );
        }
    }

    // For enums that come with and without a `k` prefix.
    pub fn k_opt_xxx(_name: &str, variant: &str) -> String {
        if let Some(stripped) = variant.strip_prefix('k') {
            stripped.into()
        } else {
            variant.into()
        }
    }

    pub fn _k_xxx_enum(name: &str, variant: &str) -> String {
        capture(name, variant, &format!("k(.*)_{name}"))
    }

    pub fn k_xxx_name_opt(name: &str, variant: &str) -> String {
        let suffix = &format!("_{name}");
        let value = if variant.ends_with(suffix) {
            capture(name, variant, &format!("k(.*){suffix}"))
        } else {
            capture(name, variant, "k(.*)")
        };

        if value.parse::<usize>().is_ok() {
            // it's a FontWeight::Type
            format!("W{value}") // W(eight)
        } else {
            value
        }
    }

    pub fn k_xxx_name(name: &str, variant: &str) -> String {
        capture(name, variant, &format!("k(.*)_{name}"))
    }

    pub fn vk(name: &str, variant: &str) -> String {
        let prefix = name.to_shouty_snake_case();
        capture(name, variant, &format!("{prefix}_(.*)"))
    }

    fn capture(name: &str, variant: &str, pattern: &str) -> String {
        let re = Regex::new(pattern).unwrap();
        re.captures(variant).unwrap_or_else(|| {
            panic!("failed to match '{pattern}' on enum variant '{variant}' of enum '{name}'")
        })[1]
            .into()
    }
}

#[allow(unused)]
pub use definitions::{Definition, Definitions};

pub(crate) mod definitions {
    use std::{
        collections::HashSet,
        fs,
        io::Write,
        path::{Path, PathBuf},
    };

    use super::env;
    use crate::build_support::{
        cargo,
        features::{self, feature},
    };

    /// A preprocessor definition.
    pub type Definition = (String, Option<String>);
    /// A container for a number of preprocessor definitions.
    pub type Definitions = Vec<Definition>;

    pub fn from_env() -> Definitions {
        let env_string =
            env::skia_lib_definitions().expect("must include library definition environment");
        from_defines_str(&env_string)
    }

    pub fn save_definitions(
        definitions: &[Definition],
        output_directory: impl AsRef<Path>,
    ) -> std::io::Result<()> {
        fs::create_dir_all(&output_directory)?;
        let mut file = fs::File::create(output_directory.as_ref().join("skia-defines.txt"))?;
        for (name, value) in definitions.iter() {
            if let Some(value) = value {
                writeln!(file, "-D{name}={value}")?;
            } else {
                writeln!(file, "-D{name}")?;
            }
        }
        writeln!(file)
    }

    // Extracts definitions from ninja files that need to be parsed for build consistency.
    pub fn from_ninja_features(
        features: &features::Features,
        use_system_libraries: bool,
        output_directory: &Path,
    ) -> Definitions {
        let ninja_files = ninja_files_for_features(features, use_system_libraries);
        from_ninja_files(&ninja_files, output_directory)
    }

    fn from_ninja_files(ninja_files: &[PathBuf], output_directory: &Path) -> Definitions {
        let mut definitions = Vec::new();

        for ninja_file in ninja_files {
            let ninja_file = output_directory.join(ninja_file);
            let contents = fs::read_to_string(&ninja_file).unwrap_or_else(|err| {
                panic!(
                    "Failed to read ninja file: `{}`: {err}",
                    ninja_file.display()
                )
            });
            definitions = combine(definitions, from_ninja_file_content(&ninja_file, contents))
        }

        definitions
    }

    /// Parse a defines = line from a ninja build file.
    fn from_ninja_file_content(file_path: &Path, contents: impl AsRef<str>) -> Definitions {
        let defines = {
            let prefix = "defines = ";
            let defines = contents
                .as_ref()
                .lines()
                .find(|s| s.starts_with(prefix))
                .unwrap_or_else(|| {
                    panic!(
                        "Missing a line starting with `defines =` in: {}",
                        file_path.display()
                    )
                });
            &defines[prefix.len()..]
        };
        from_defines_str(defines)
    }

    fn ninja_files_for_features(
        features: &features::Features,
        use_system_libraries: bool,
    ) -> Vec<PathBuf> {
        let mut files = vec!["obj/skia.ninja".into()];
        if features.ganesh() {
            files.push("obj/gpu.ninja".into());
        }
        if features.graphite() {
            files.push("obj/graphite.ninja".into());
        }
        if features[feature::TEXTLAYOUT] {
            files.extend(vec![
                "obj/modules/skshaper/skshaper.ninja".into(),
                "obj/modules/skparagraph/skparagraph.ninja".into(),
                "obj/modules/skunicode/skunicode_core.ninja".into(),
                "obj/modules/skunicode/skunicode_icu.ninja".into(),
            ]);
            // shaper.cpp includes SkLoadICU.h — skip bundled ICU ninja when
            // system ICU is active (either via SKIA_USE_SYSTEM_LIBRARIES or
            // skia_use_system_icu=true in SKIA_GN_ARGS).
            let system_icu = use_system_libraries
                || cargo::env_var("SKIA_GN_ARGS")
                    .is_some_and(|args| args.contains("skia_use_system_icu=true"));
            if !system_icu {
                files.push("obj/third_party/icu/icu.ninja".into())
            }
        }
        if features[feature::SVG] {
            files.push("obj/modules/svg/svg.ninja".into());
        }
        if features[feature::SKOTTIE] {
            files.push("obj/modules/skottie/skottie.ninja".into());
        }
        files
    }

    fn combine(a: Definitions, b: Definitions) -> Definitions {
        remove_duplicates(a.into_iter().chain(b).collect())
    }

    fn remove_duplicates(mut definitions: Definitions) -> Definitions {
        let mut uniques = HashSet::new();
        definitions.retain(|e| uniques.insert(e.0.clone()));
        definitions
    }

    fn from_defines_str(defines: &str) -> Definitions {
        const PREFIX: &str = "-D";
        defines
            .split_whitespace()
            .map(|d| {
                if let Some(stripped) = d.strip_prefix(PREFIX) {
                    stripped
                } else {
                    panic!("Missing '{PREFIX}' prefix from a definition")
                }
            })
            .map(|d| {
                let items: Vec<&str> = d.splitn(2, '=').collect();
                match items.len() {
                    1 => (items[0].to_string(), None),
                    2 => (items[0].to_string(), Some(unescape_ninja(items[1]))),
                    _ => panic!("Internal error"),
                }
            })
            .collect()
    }

    fn unescape_ninja(input: &str) -> String {
        unescape(&unescape(input, '$'), '\\')
    }

    fn unescape(input: &str, escape_character: char) -> String {
        let mut result = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == escape_character {
                if let Some(&next_ch) = chars.peek() {
                    chars.next();
                    result.push(next_ch);
                }
            } else {
                result.push(ch);
            }
        }

        result
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn properly_unescape_trivial_abi() {
            // This happens if SKIA_DEBUG=1
            let str = r#"\[\[clang$:$:trivial_abi\]\]"#;
            assert_eq!(super::unescape_ninja(str), "[[clang::trivial_abi]]");
        }
    }
}
