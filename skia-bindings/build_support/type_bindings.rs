//! The Type Bindings Table for opt-in binding generation.
//!
//! See `docs/adr/0001-opt-in-binding-generation.md`. Every type in the
//! transitive closure of the `C_*` wrapper functions must appear here with a
//! `TypeAction`; bindgen's `new_item_found` callback hard-errors on any
//! discovered type missing from this table.
//!
//! Entries are migrated 1:1 from the former `OPAQUE_TYPES`,
//! `BLOCKLISTED_TYPES`, and inline blocklists in `skia_bindgen.rs`.
//!
//! Generation decisions come in two flavors (`Source`):
//! - `Manual`: an explicit `TypeAction` (legacy opaque/stub/exclude
//!   entries, std:: templates, and types outside the touch-point scan).
//! - `Scanned`: usage flags harvested by
//!   `tools/scan_type_usage.py` from the Rust-side touch-point corpus,
//!   or-combined as `ScanFlags::FIELDS | ScanFlags::FUNCTIONS`; the
//!   *action is derived* from them at generation time:
//!
//!   | fields needed | functions needed | derived action             |
//!   |---------------|------------------|----------------------------|
//!   | yes           | yes              | `Include`                   |
//!   | yes           | no               | `Include` + blocklist all   |
//!   |               |                  | methods (`{Name}_.+`, never |
//!   |               |                  | the `C_*` wrappers)         |
//!   | no            | yes              | `Opaque`                    |
//!   | no            | no               | blocklist (excluded)        |

/// How a type in the closure is generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeAction {
    /// Generate in full (members visible; needed for member access or by
    /// value in `extern "C"` signatures).
    Include,
    /// Generate as opaque (size/alignment only) via `opaque_type`.
    Opaque,
    /// Not generated; bindgen blocklist + `raw_line` stub
    /// (`pub enum X {}`) so referencing Rust code still compiles.
    Stub,
    /// Blocklisted + `raw_line`-injected generic stand-in definition for a
    /// C++ template type that appears in generated function signatures but
    /// cannot enter bindgen's IR in `allowlist_recursively(false)` mode
    /// (its template argument phantoms trip the opaque-with-fields
    /// assertion, bindgen issue 2437 family). The injected definition is
    /// generic over the template's parameters and sized via its Rust-side
    /// usage in `extern "C"` signatures.
    StubGeneric,
    /// Excluded entirely with no stub (must not be referenced from Rust).
    Exclude,
}

/// Usage flags from the Rust-side touch-point scan
/// (`tools/scan_type_usage.py`), or-combined at call sites:
/// `ScanFlags::FIELDS | ScanFlags::FUNCTIONS`. Drives the derivation of
/// the entry's action at generation time (see the module-level decision
/// matrix).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanFlags(u8);

impl ScanFlags {
    /// The type's members are read or written from Rust (field access,
    /// struct literal, transmute) — the type must be generated in full.
    pub const FIELDS: Self = Self(1 << 0);
    /// The type's methods are called from Rust (via `C_*` wrappers) — the
    /// type must stay non-blocklisted.
    pub const FUNCTIONS: Self = Self(1 << 1);

    /// Empty flag set: the type is untouched from Rust and the builder
    /// blocklists it entirely.
    pub const NONE: Self = Self(0);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// `FIELDS_FUNCTIONS` as a single const (const `|` on the wrapper
    /// type is not const-evaluable; the raw bits are).
    pub const FIELDS_FUNCTIONS: Self = Self((1 << 0) | (1 << 1));
}

impl std::ops::BitOr for ScanFlags {
    type Output = Self;
    #[allow(clippy::op_ref)]
    fn bitor(self, other: Self) -> Self {
        self.union(other)
    }
}

/// Unqualified aliases so table entries read `FIELDS_FUNCTIONS`.
#[allow(non_upper_case_globals)]
const FIELDS: ScanFlags = ScanFlags::FIELDS;
const FUNCTIONS: ScanFlags = ScanFlags::FUNCTIONS;
const FIELDS_FUNCTIONS: ScanFlags = ScanFlags::FIELDS_FUNCTIONS;
const NONE: ScanFlags = ScanFlags::NONE;

/// Whether an entry's action is written by hand or derived from scan
/// flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// An explicitly chosen action (opaque/stub/exclude entries, std::
    /// templates, and types outside the touch-point scan).
    Manual(TypeAction),
    /// The action is derived from the usage flags by the builder (see the
    /// module-level decision matrix). `ScanFlags::FIELDS | ScanFlags::
    /// FUNCTIONS` means members are accessed *and* methods are called;
    /// `ScanFlags::FUNCTIONS` only means methods are called but members
    /// are never touched (derives `Opaque`). `ScanFlags::NONE` means
    /// untouched. The flags are rewritten wholesale by
    /// `tools/scan_type_usage.py --update-table`.
    Scanned(ScanFlags),
}

impl Source {
    /// The action the builder will derive for this source. `Manual` returns
    /// the stored action; `Scanned` resolves the decision matrix.
    pub fn action(self) -> TypeAction {
        match self {
            Source::Manual(action) => action,
            Source::Scanned(flags) => {
                if flags.contains(ScanFlags::FIELDS) {
                    TypeAction::Include
                } else if flags.contains(ScanFlags::FUNCTIONS) {
                    TypeAction::Opaque
                } else {
                    TypeAction::Exclude
                }
            }
        }
    }
}

/// A single Type Bindings Table row.
///
/// `features` lists the cargo features under which the type can appear in
/// the closure; an empty slice means it is feature-independent.
#[derive(Debug, Clone, Copy)]
pub struct TypeEntry {
    pub name: &'static str,
    /// How the action is decided: written by hand or derived from scan
    /// flags (see the module-level decision matrix).
    pub source: Source,
    pub features: &'static [&'static str],
    /// Generic stand-in definition injected via `raw_line` for
    /// `StubGeneric` entries.
    pub definition: &'static str,
}

impl TypeEntry {
    /// The action the builder will apply to this entry.
    pub fn action(&self) -> TypeAction {
        self.source.action()
    }

    /// For a scan-managed entry with `ScanFlags::FIELDS` set but no
    /// `ScanFlags::FUNCTIONS`, the blanket bindgen regex that blocks all of
    /// the type's generated method wrappers. The hand-written `C_*`
    /// wrappers are unaffected (their names carry the `C_` prefix, which
    /// the start-anchored pattern cannot match).
    pub fn function_blocklist(&self) -> Option<String> {
        if let Source::Scanned(flags) = self.source {
            if flags.contains(ScanFlags::FIELDS)
                && !flags.contains(ScanFlags::FUNCTIONS)
                && !self.name.ends_with(".*")
            {
                return Some(format!("{}_.+", self.name));
            }
        }
        None
    }
}

const fn opaque(name: &'static str) -> TypeEntry {
    TypeEntry {
        name,
        source: Source::Manual(TypeAction::Opaque),
        features: &[],
        definition: "",
    }
}

const fn opaque_f(name: &'static str, features: &'static [&'static str]) -> TypeEntry {
    TypeEntry {
        name,
        source: Source::Manual(TypeAction::Opaque),
        features,
        definition: "",
    }
}

const fn stub(name: &'static str) -> TypeEntry {
    TypeEntry {
        name,
        source: Source::Manual(TypeAction::Stub),
        features: &[],
        definition: "",
    }
}

const fn excluded(name: &'static str) -> TypeEntry {
    TypeEntry {
        name,
        source: Source::Manual(TypeAction::Exclude),
        features: &[],
        definition: "",
    }
}

const fn excluded_f(name: &'static str, features: &'static [&'static str]) -> TypeEntry {
    TypeEntry {
        name,
        source: Source::Manual(TypeAction::Exclude),
        features,
        definition: "",
    }
}

const fn included_f(name: &'static str, features: &'static [&'static str]) -> TypeEntry {
    TypeEntry {
        name,
        source: Source::Manual(TypeAction::Include),
        features,
        definition: "",
    }
}

/// Scan-managed entry (ADR 0001): the action and any method blocklists are
/// derived from `flags` at generation time; rewritten wholesale by
/// `tools/scan_type_usage.py --update-table`.
const fn scanned_f(
    name: &'static str,
    flags: ScanFlags,
    features: &'static [&'static str],
) -> TypeEntry {
    TypeEntry {
        name,
        source: Source::Scanned(flags),
        features,
        definition: "",
    }
}

/// Blocklisted template whose generated signature usage is covered by an
/// injected generic definition (see `StubGeneric`).
const fn stub_generic(name: &'static str, definition: &'static str) -> TypeEntry {
    TypeEntry {
        name,
        source: Source::Manual(TypeAction::StubGeneric),
        features: &[],
        definition,
    }
}

/// Feature sets that pull in SkShaper.
const SHAPER_FEATURES: &[&str] = &["skia", "textlayout", "svg", "skottie"];

/// The table in generation order: types whose action must not be shadowed by
/// a later, more general regex come first. Order otherwise follows the
/// former opt-out lists (inline blocklist, then `OPAQUE_TYPES`, then
/// `BLOCKLISTED_TYPES`) to keep the milestone-history comments traceable.
// ---- SCAN BEGIN (generated by tools/scan_type_usage.py) ----
// SCAN: SkSVGNode = fields
// SCAN: skresources::ResourceProvider = none
// SCAN: GrGLenum = fields
// SCAN: GrGLFormat = fields
// SCAN: VkComponentMapping = fields
// SCAN: skia::textlayout::TextAlign = none
// SCAN: skia::textlayout::PositionWithAffinity = none
// SCAN: skia::textlayout::TextBaseline = none
// SCAN: skia::textlayout::TextDecorationStyle = functions
// SCAN: skia::textlayout::TextDecorationMode = functions
// SCAN: SkFlattenable::Type = none
// SCAN: SkAlphaType = fields
// SCAN: SkArc_Type = fields
// SCAN: SkBlendMode = fields
// SCAN: SkBlurStyle = fields
// SCAN: SkCanvas_Lattice_RectType = fields
// SCAN: SkClipOp = fields
// SCAN: SkPaint_Cap = fields
// SCAN: SkPaint_Join = fields
// SCAN: SkParsePath_PathEncoding = fields
// SCAN: SkPath_Verb = fields+functions
// SCAN: SkPathDirection = fields
// SCAN: SkPathFillType = fields
// SCAN: SkPathVerb = fields
// SCAN: SkTileMode = fields
// SCAN: SkYUVColorSpace = fields
// SCAN: GrBackendApi = fields
// SCAN: GrBackendDrawableInfo = fields
// SCAN: GrBackendFormat = fields+functions
// SCAN: GrBackendRenderTarget = fields
// SCAN: GrBackendSemaphore = fields
// SCAN: GrBackendTexture = fields
// SCAN: GrContextOptions = fields
// SCAN: GrDirectContext_DirectContextID = fields
// SCAN: GrFlushInfo = fields
// SCAN: GrGLExtensions = fields+functions
// SCAN: GrGLFramebufferInfo = fields
// SCAN: GrGLSurfaceInfo = fields
// SCAN: GrGLTextureInfo = fields
// SCAN: GrMtlBackendContext = fields
// SCAN: GrMtlSurfaceInfo = fields
// SCAN: GrMtlTextureInfo = fields
// SCAN: GrVkDrawableInfo = fields
// SCAN: GrVkImageInfo = fields
// SCAN: GrVkSurfaceInfo = fields
// SCAN: GrYUVABackendTextureInfo = fields+functions
// SCAN: sk_sp = fields
// SCAN: Sk3DView = fields
// SCAN: SkArc = fields
// SCAN: SkAutoCanvasRestore = fields
// SCAN: SkBlender = fields
// SCAN: SkCanvas = fields
// SCAN: SkCanvas_SaveLayerRec = fields
// SCAN: SkCodec = fields
// SCAN: SkCodec_FrameInfo = fields
// SCAN: SkCodec_Options = fields
// SCAN: SkCodecs::Decoder = functions
// SCAN: SkColor = fields+functions
// SCAN: SkColor4f = fields
// SCAN: SkColorInfo = fields
// SCAN: SkColorMatrix = fields
// SCAN: SkColorSpace = fields
// SCAN: SkColorSpacePrimaries = fields
// SCAN: SkColorTable = fields
// SCAN: SkColorType = fields
// SCAN: SkContext = fields
// SCAN: SkContextOptions = fields
// SCAN: SkContourMeasureIter = fields+functions
// SCAN: SkCubicMap = fields+functions
// SCAN: SkCustomTypefaceBuilder = fields+functions
// SCAN: SkData = fields
// SCAN: SkDynamicMemoryWStream = fields
// SCAN: SkEncodedOrigin = fields
// SCAN: SkFont = fields+functions
// SCAN: SkFontArguments = fields
// SCAN: SkFontArguments_Palette = fields
// SCAN: SkFontArguments_VariationPosition = fields
// SCAN: SkFontArguments_VariationPosition_Coordinate = fields
// SCAN: SkFontParameters_Variation_Axis = fields
// SCAN: SkFontStyle = fields
// SCAN: SkFourByteTag = fields
// SCAN: SkGradient_Interpolation = fields
// SCAN: SkHighContrastConfig = fields
// SCAN: SkImage = fields
// SCAN: SkImage_RequiredProperties = fields
// SCAN: SkImageFilter = fields
// SCAN: SkImageGenerator = fields
// SCAN: SkImageInfo = fields
// SCAN: SkIPoint = fields
// SCAN: SkIRect = fields
// SCAN: SkISize = fields
// SCAN: SkJpegEncoder::Downsample = none
// SCAN: SkM44 = fields
// SCAN: SkMatrix = fields
// SCAN: SkMemoryStream = fields
// SCAN: SkOpBuilder = fields
// SCAN: SkOrderedFontMgr = fields
// SCAN: SkPaint = fields+functions
// SCAN: SkPath = fields+functions
// SCAN: SkPath_Iter = fields+functions
// SCAN: SkPath_RawIter = fields
// SCAN: SkPathBuilder = fields
// SCAN: SkPathContourIter = fields
// SCAN: SkPathIter = fields
// SCAN: SkPathMeasure = fields+functions
// SCAN: SkPDF::Metadata = none
// SCAN: SkPictureRecorder = fields
// SCAN: SkPixmap = fields
// SCAN: SkPngEncoder::FilterFlag = fields
// SCAN: SkPoint = fields
// SCAN: SkPoint3 = fields
// SCAN: SkRecorder = fields
// SCAN: SkRect = fields
// SCAN: SkRefCnt = fields
// SCAN: SkRefCntBase = fields+functions
// SCAN: SkRegion = fields+functions
// SCAN: SkRegion_Cliperator = fields+functions
// SCAN: SkRegion_Iterator = fields+functions
// SCAN: SkRegion_Spanerator = fields+functions
// SCAN: SkRRect = fields
// SCAN: SkRSXform = fields
// SCAN: SkRuntimeShaderBuilder = fields
// SCAN: SkSamplingOptions = fields
// SCAN: SkScalar = fields
// SCAN: SkSize = fields
// SCAN: SkSL::Version = none
// SCAN: SkStrikeRef = fields
// SCAN: SkString = fields+functions
// SCAN: SkStrings = fields
// SCAN: SkStrokeRec = fields+functions
// SCAN: SkSurfaceProps = fields+functions
// SCAN: SkSVGCircle = fields
// SCAN: SkSVGClipPath = fields
// SCAN: SkSVGColor = fields
// SCAN: SkSVGContainer = fields
// SCAN: SkSVGDefs = fields
// SCAN: SkSVGDOM = fields+functions
// SCAN: SkSVGEllipse = fields
// SCAN: SkSVGFe = fields
// SCAN: SkSVGFeBlend = fields
// SCAN: SkSVGFeColorMatrix = fields
// SCAN: SkSVGFeComponentTransfer = fields
// SCAN: SkSVGFeComposite = fields
// SCAN: SkSVGFeDiffuseLighting = fields
// SCAN: SkSVGFeDisplacementMap = fields
// SCAN: SkSVGFeDistantLight = fields
// SCAN: SkSVGFeFlood = fields
// SCAN: SkSVGFeFunc = fields
// SCAN: SkSVGFeGaussianBlur = fields
// SCAN: SkSVGFeGaussianBlur_StdDeviation = fields
// SCAN: SkSVGFeImage = fields
// SCAN: SkSVGFeInputType = fields
// SCAN: SkSVGFeLighting = fields
// SCAN: SkSVGFeLighting_KernelUnitLength = fields
// SCAN: SkSVGFeMerge = fields
// SCAN: SkSVGFeMergeNode = fields
// SCAN: SkSVGFeMorphology = fields
// SCAN: SkSVGFeMorphology_Radius = fields
// SCAN: SkSVGFeOffset = fields
// SCAN: SkSVGFePointLight = fields
// SCAN: SkSVGFeSpecularLighting = fields
// SCAN: SkSVGFeSpotLight = fields
// SCAN: SkSVGFeTurbulence = fields
// SCAN: SkSVGFeTurbulenceBaseFrequency = fields
// SCAN: SkSVGFeTurbulenceType = fields
// SCAN: SkSVGFillRule = fields
// SCAN: SkSVGFilter = fields
// SCAN: SkSVGFontFamily = fields
// SCAN: SkSVGFontSize = fields
// SCAN: SkSVGFontStyle = fields
// SCAN: SkSVGFontWeight = fields
// SCAN: SkSVGFuncIRI = fields
// SCAN: SkSVGG = fields
// SCAN: SkSVGGradient = fields
// SCAN: SkSVGImage = fields
// SCAN: SkSVGIRI = fields
// SCAN: SkSVGLength = fields
// SCAN: SkSVGLine = fields
// SCAN: SkSVGLinearGradient = fields
// SCAN: SkSVGLineJoin = fields
// SCAN: SkSVGMask = fields
// SCAN: SkSVGObjectBoundingBoxUnits = fields
// SCAN: SkSVGPaint = fields
// SCAN: SkSVGPath = fields
// SCAN: SkSVGPattern = fields
// SCAN: SkSVGPoly = fields
// SCAN: SkSVGPreserveAspectRatio = fields
// SCAN: SkSVGRadialGradient = fields
// SCAN: SkSVGRect = fields
// SCAN: SkSVGSpreadMethod = fields
// SCAN: SkSVGStop = fields
// SCAN: SkSVGSVG = fields
// SCAN: SkSVGText = fields
// SCAN: SkSVGTextAnchor = fields
// SCAN: SkSVGTextContainer = fields
// SCAN: SkSVGTextLiteral = fields
// SCAN: SkSVGTextPath = fields
// SCAN: SkSVGTransformableNode = fields
// SCAN: SkSVGTSpan = fields
// SCAN: SkSVGUse = fields
// SCAN: SkSVGVisibility = fields
// SCAN: SkTextBlob = fields
// SCAN: SkTextBlob_Iter = fields+functions
// SCAN: SkTextBlobBuilder = fields+functions
// SCAN: SkTextBlobBuilderRunHandler = fields
// SCAN: SkTextEncoding = fields
// SCAN: SkTypeface = fields
// SCAN: SkTypeface_LocalizedStrings = fields
// SCAN: SkV2 = fields
// SCAN: SkV3 = fields
// SCAN: SkV4 = fields
// SCAN: SkVertices = fields
// SCAN: SkVertices_Builder = fields+functions
// SCAN: SkYUVAInfo = fields+functions
// SCAN: SkYUVAInfo_Subsampling = fields
// SCAN: SkYUVAPixmapInfo = fields+functions
// SCAN: SkYUVAPixmapInfo_SupportedDataTypes = fields
// SCAN: SkYUVAPixmaps = fields
// SCAN: VkImageLayout = fields
// SCAN: GrDirectContext = fields
// SCAN: IndexedStyleMetrics = fields
// SCAN: RustResourceProvider = fields
// SCAN: RustResourceProvider_Param = fields
// SCAN: RustRunHandler = fields
// SCAN: RustRunHandler_Param = fields
// SCAN: RustStream = fields+functions
// SCAN: RustWStream = fields+functions
// SCAN: Sink = fields+functions
// SCAN: SkBitmap = fields+functions
// SCAN: SkCamera3D = fields+functions
// SCAN: skcms_TransferFunction = fields
// SCAN: SkCodecAnimation::Blend = functions
// SCAN: skcpu::Recorder = functions
// SCAN: SkCubicResampler = fields
// SCAN: SkFontMetrics = fields
// SCAN: skgpu::BackendApi = none
// SCAN: skgpu::Budgeted = fields
// SCAN: skgpu::GpuStatsFlags = functions
// SCAN: skgpu::graphite::BackendTexture = functions
// SCAN: skgpu::graphite::Buffer = none
// SCAN: skgpu::graphite::Context = functions
// SCAN: skgpu::graphite::ContextOptions = fields+functions
// SCAN: skgpu::graphite::Device = none
// SCAN: skgpu::graphite::Recorder = functions
// SCAN: skgpu::graphite::RecorderOptions = functions
// SCAN: skgpu::graphite::ResourceProvider = none
// SCAN: skgpu::graphite::SubmitInfo = fields+functions
// SCAN: skgpu::graphite::TextureInfo = fields+functions
// SCAN: skgpu::Mipmapped = none
// SCAN: skia::textlayout::Block = fields
// SCAN: skia::textlayout::FontArguments = fields+functions
// SCAN: skia::textlayout::FontFeature = functions
// SCAN: skia::textlayout::LineMetrics = functions
// SCAN: skia::textlayout::Paragraph = functions
// SCAN: skia::textlayout::ParagraphBuilder = functions
// SCAN: skia::textlayout::ParagraphStyle = functions
// SCAN: skia::textlayout::Placeholder = fields
// SCAN: skia::textlayout::PlaceholderStyle = fields
// SCAN: skia::textlayout::RectHeightStyle = fields
// SCAN: skia::textlayout::RectWidthStyle = fields
// SCAN: skia::textlayout::StrutStyle = functions
// SCAN: skia::textlayout::TextShadow = fields+functions
// SCAN: skia::textlayout::TextStyle = functions
// SCAN: skia::textlayout::TypefaceFontProvider = functions
// SCAN: skia::textlayout::TypefaceFontStyleSet = functions
// SCAN: skottie::Animation = functions
// SCAN: skottie::ResourceProvider = none
// SCAN: SkPixelGeometry = fields
// SCAN: skresources::ImageAsset = functions
// SCAN: SkRuntimeEffectBuilder = fields
// SCAN: sksg::ImageFilter = functions
// SCAN: sksg::Matrix = fields+functions
// SCAN: sksg::Node = functions
// SCAN: sksg::Shader = functions
// SCAN: SkSVGShape = fields
// SCAN: VecSink = fields+functions
// SCAN: GrSubmitInfo = fields
// SCAN: skgpu::GpuStats = fields
// SCAN: skgpu::graphite::BackendSemaphore = functions
// SCAN: skgpu::graphite::InsertStatus = none
// SCAN: skhdr::Metadata = none
// SCAN: skia::textlayout::Decoration = fields+functions
// SCAN: skia::textlayout::PlaceholderAlignment = fields
// SCAN: skia::textlayout::StyleMetrics = fields+functions
// SCAN: skia::textlayout::TextBox = fields
// SCAN: skia::textlayout::TextDirection = none
// SCAN: skia::textlayout::TextHeightBehavior = none
// SCAN: skia::textlayout::TextRange = none
// SCAN: SkNVRefCnt = fields
// SCAN: SkPatch3D = fields+functions
// SCAN: TraitObject = fields
// SCAN: skia::textlayout::TextDecoration = functions
// SCAN: std::function = fields
// ---- SCAN END ----

pub(crate) const TYPES: &[TypeEntry] = &[
    // ------------------------------------------------------------------
    // From the inline blocklists in skia_bindgen.rs.
    // ------------------------------------------------------------------
    // not used:
    stub("SkPathRef_Editor"),
    // private types that pull in inline functions that cannot be linked:
    // <https://github.com/rust-skia/rust-skia/issues/318>
    stub("GrContext_Base"),
    stub("GrImageContext"),
    stub("GrImageContextPriv"),
    stub("GrContextThreadSafeProxy"),
    stub("GrContextThreadSafeProxyPriv"),
    stub("GrRecordingContextPriv"),
    stub("GrContextPriv"),
    stub("SkVerticesPriv"),
    // m113: `SkUnicode` pulls in an impl block that forwards static
    // functions that may not be linked into the final executable.
    stub("SkUnicode"),
    // ------------------------------------------------------------------
    // From OPAQUE_TYPES.
    // ------------------------------------------------------------------
    // Types for which the binding generator pulls in stuff that can not be
    // compiled.
    opaque("SkDeferredDisplayList"),
    opaque("SkDeferredDisplayList_PendingPathsMap"),
    // Types for which a bindgen layout is wrong causing types that contain
    // fields of them to fail their layout test.
    // Windows:
    // NOTE: std:: template entries were Opaque under recursive
    // allowlisting. With `allowlist_recursively(false)` bindgen cannot
    // generate opaque definitions for template instantiations (it hits
    // "assertion failed: fields.is_empty()" or the anonymous
    // "type-parameter-0-0" name). Parents that would carry them as
    // members are made opaque, with member access reimplemented via
    // `C_*` wrapper functions in skia-bindings/src/*.cpp. They are
    // therefore Exclude.
    // unique_ptr<T, D>: pointer + phantom deleter (8 bytes, ABI: ptr).
    // std::default_delete<T>: empty struct.
    opaque("SkTHashMap"),
    // Ubuntu 18 LLVM 6: all types derived from SkWeakRefCnt
    opaque("SkWeakRefCnt"),
    opaque("GrContext"),
    opaque("GrGLInterface"),
    opaque("GrSurfaceProxy"),
    opaque("Sk2DPathEffect"),
    opaque("SkCornerPathEffect"),
    opaque("SkDataTable"),
    opaque("SkDiscretePathEffect"),
    opaque("SkDrawable"),
    opaque("SkLine2DPathEffect"),
    opaque("SkPath2DPathEffect"),
    opaque("SkPathRef::GenIDChangeListener"),
    opaque("SkPicture"),
    opaque("SkPixelRef"),
    opaque("SkSurface"),
    // Types not needed (for now): excluded entirely, no stub.
    excluded("GrGLInterface_Functions"),
    // SkShaper (m77) Trivial*Iterator classes create two vtable pointers.
    opaque_f("SkShaper::TrivialBiDiRunIterator", SHAPER_FEATURES),
    opaque_f("SkShaper::TrivialFontRunIterator", SHAPER_FEATURES),
    opaque_f("SkShaper::TrivialLanguageRunIterator", SHAPER_FEATURES),
    opaque_f("SkShaper::TrivialScriptRunIterator", SHAPER_FEATURES),
    // skparagraph
    // std::vector<T>: 3 pointers (begin/end/cap), 24 bytes.
    opaque_f("std::u16string", &[]),
    // skparagraph (m78), (layout fails on macOS and Linux, not sure why,
    // looks like an obscure alignment problem)
    opaque_f("skia::textlayout::FontCollection", &["textlayout"]),
    // skparagraph (m79), std::map is used in LineMetrics
    // (map is included_f now, with the vendored bindgen resolving templates)
    // Vulkan reexports with the wrong field naming conventions.
    opaque_f("VkPhysicalDeviceFeatures", &["vulkan"]),
    opaque_f("VkPhysicalDeviceFeatures2", &["vulkan"]),
    // Since Rust 1.39 beta (TODO: investigate why, and re-test when 1.39
    // goes stable).
    opaque("GrContextOptions_PersistentCache"),
    opaque("GrContextOptions_ShaderErrorHandler"),
    opaque("Sk1DPathEffect"),
    opaque("SkBBoxHierarchy"), // vtable
    opaque("SkBBHFactory"),
    opaque("SkBitmap::Allocator"),
    opaque("SkBitmap::HeapAllocator"),
    opaque("SkColorFilter"),
    opaque("SkDeque_F2BIter"),
    opaque("SkDrawable::GpuDrawHandler"),
    opaque("SkFlattenable"),
    opaque("SkFontMgr"),
    opaque("SkFontStyleSet"),
    opaque("SkMaskFilter"),
    opaque("SkPathEffect"),
    opaque("SkPicture::AbortCallback"),
    opaque("SkPixelRef::GenIDChangeListener"),
    opaque("SkRasterHandleAllocator"),
    // m114: `SkRefCnt` must be kept (commented out upstream, retained here
    // as documentation of that decision).
    opaque("SkShader"),
    opaque("SkStream"),
    opaque("SkStreamAsset"),
    opaque("SkStreamMemory"),
    opaque("SkStreamRewindable"),
    opaque("SkStreamSeekable"),
    opaque("SkTypeface::LocalizedStrings"),
    opaque("SkWStream"),
    opaque_f("SkShaper::LanguageRunIterator", SHAPER_FEATURES),
    included_f("SkShaper::RunHandler", SHAPER_FEATURES),
    opaque_f("SkShaper::RunIterator", SHAPER_FEATURES),
    opaque_f("SkShaper::ScriptRunIterator", SHAPER_FEATURES),
    opaque("SkContourMeasure"),
    opaque("SkDocument"),
    // m81: tuples:
    opaque("SkRuntimeEffect_EffectResult"),
    opaque("SkRuntimeEffect_ByteCodeResult"),
    opaque("SkRuntimeEffect_SpecializeResult"),
    // m81: derives from std::string
    opaque("SkSL::String"),
    // m81: wrong size on macOS and Linux
    opaque("SkRuntimeEffect"),
    opaque("GrShaderCaps"),
    // more stuff we don't need that was tracked down fixing:
    // <https://github.com/rust-skia/rust-skia/issues/318>
    // referred from SkPath, but not used:
    excluded("SkPathRef"),
    // m82: private
    // m86:
    opaque("GrRecordingContext"),
    // m87:
    opaque_f("GrD3DAlloc", &["d3d"]),
    opaque_f("GrD3DMemoryAllocator", &["d3d"]),
    // m87, yuva_pixmaps
    // std:: template instantiations never enter bindgen's IR in
    // `allowlist_recursively(false)` mode: their template arguments arrive
    // as anonymous `type-parameter-N-M` spellings which would panic bindgen
    // (issue 2437). They are excluded; any parent carrying them as members
    // must be opaque with the member access reimplemented via `C_*`
    // wrapper functions in skia-bindings/src/*.cpp.
    // std::tuple<...>: opaque 8-byte blob with a phantom for T.
    // Homebrew macOS LLVM 13
    excluded("std::tuple_.*"),
    // Since 3.1.57 of the emsdk:
    // <https://github.com/rust-skia/rust-skia/issues/975>
    opaque_f("std::__1::basic_string_view<char>", &[]),
    opaque_f("std::__1::basic_string<char>", &[]),
    excluded("std::__2::.*"),
    // clang 18 / XCode 16

    // m93: private, exposed by Paint::asBlendMode(), fails layout tests.
    opaque("skstd::optional"),
    // m100
    // std::optional<T>: 8-byte value + 4-byte inline state on this ABI
    // (bindgen recursive output: opaque array over the payload).
    // Feature `svg`:
    excluded_f("SkSVGProperty", &["svg"]),
    included_f("SkSVGNode", &["svg"]),
    // Same bindgen limitation as the std:: templates above: these are
    excluded_f("SkTCopyOnFirstWrite", &["svg"]),
    scanned_f("skresources::ResourceProvider", NONE, &["svg"]), // m107 (layout failure)
    opaque_f("skgpu::VulkanMemoryAllocator", &["vulkan"]),
    // m109 (ParagraphPainter::SkPaintOrID)
    excluded("std::variant"),
    // m111 used in SkTextBlobBuilder
    // Pulled in by `SkData`.
    excluded("FILE"),
    // m114: results in wrongly sized template specializations.
    // Template type (K, V phantoms break opaque generation, bindgen 2437 family).
    excluded("skia_private::THashMap"),
    // m121:
    opaque_f("skgpu::MutableTextureState", &["vulkan", "graphite"]),
    // emscripten: uses SkLRUCache (which is excluded)
    opaque_f("skia::textlayout::ParagraphCache", &["textlayout"]),
    // Fix bindgen 0.70 layout failures
    opaque_f("skgpu::VulkanBackendContext", &["vulkan"]),
    opaque_f("GrYUVABackendTextures", &["vulkan"]),
    // LLVM21
    // std::basic_string<C>: SSO string, 24 bytes (bindgen recursive shape:
    // opaque array 8 x u8 = 24 in libstdc++; libc++ uses 24 too).
    excluded("std::basic_string.*"),
    excluded("std::__tree.*"),
    // libstdc++ 10 on Linux (since m143, c++20)
    excluded("std::strong_ordering"),
    // skottie internal types with layout issues
    opaque_f("skottie::internal::TextAnimator", &["skottie"]),
    opaque_f(
        "skottie::internal::TextAnimator_AnimatedProps",
        &["skottie"],
    ),
    opaque_f("skottie::internal::TextAdapter", &["skottie"]),
    opaque_f("skottie::VectorValue", &["skottie"]),
    opaque_f("skottie::ColorValue", &["skottie"]),
    opaque_f("sksg::PaintNode", &["skottie"]),
    opaque_f("sksg::Color", &["skottie"]),
    opaque_f("sksg::BlurImageFilter", &["skottie"]),
    // m147
    excluded("std::unordered_map.*"),
    // Graphite types that expose std::unordered_set in public fields
    opaque_f("skgpu::graphite::Recording", &["graphite"]),
    // ------------------------------------------------------------------
    // From BLOCKLISTED_TYPES.
    // ------------------------------------------------------------------
    // modules/skparagraph
    //   pulls in a std::map<>, which we treat as opaque, but bindgen creates
    //   wrong bindings for std::_Tree* types
    excluded("std::_Tree.*"),
    //   debug builds:
    excluded("SkLRUCache"),
    excluded("SkLRUCache_Entry"),
    // too much template magic:
    excluded("SkRuntimeEffect_ConstIterable.*"),
    // Linux LLVM9 c++17
    excluded("std::_Rb_tree.*"),
    // archlinux
    excluded("std::__rb_tree.*"),
    // Linux LLVM9 c++17 with SKIA_DEBUG=1
    excluded("std::__cxx.*"),
    excluded("std::array.*"),
    // m115 unused Linux
    excluded("std::__uset_hashtable.*"),
    excluded("std::unordered_set.*"),
    // m115 unused Windows
    excluded("std::_List_unchecked.*"),
    excluded("std::_Hash.*"),
    excluded("std::_List_const.*"),
    excluded("std::list.*"),
    excluded("std::list__Unchecked.*"),
    excluded("std::_List_iterator.*"),
    // <https://github.com/rust-skia/rust-skia/issues/1009> (feature vulkan)
    excluded_f("PFN_vkVoidFunction", &["vulkan"]),
    // Inline-only masks migrated from the builder chain (ADR 0001):
    opaque_f("VkCommandBuffer", &["vulkan"]),
    opaque_f("VkImage", &["vulkan"]),
    opaque_f("VkImageTiling", &["vulkan"]),
    opaque_f("VkImageUsageFlags", &["vulkan"]),
    opaque_f("VkPhysicalDevice", &["vulkan"]),
    opaque_f("VkQueue", &["vulkan"]),
    opaque_f("VkRenderPass", &["vulkan"]),
    opaque_f("VkSemaphore", &["vulkan"]),
    opaque_f("VkSharingMode", &["vulkan"]),
    // Vulkan struct reexports explicitly allowlisted in skia_bindgen.rs
    // (not reachable from an extern "C" function signature).
    opaque_f("VkExtent2D", &["vulkan"]),
    opaque_f("VkOffset2D", &["vulkan"]),
    opaque_f("VkRect2D", &["vulkan"]),
    // ------------------------------------------------------------------
    // Types referenced by generated-code consumers (defaults.rs,
    // impls.rs, skia-bindings Rust sources) that arrived implicitly under
    // recursive allowlisting and must now be explicit Includes.
    // ------------------------------------------------------------------
    scanned_f("GrGLenum", FIELDS, &["ganesh", "gl"]), scanned_f("GrGLFormat", FIELDS, &["ganesh", "gl"]), scanned_f("VkComponentMapping", FIELDS, &["vulkan"]), // textlayout enums (DartTypes.h / TextStyle.h) exposed at crate root.
    // Bindgen allowlist patterns are unanchored regex matches against the
    // qualified C++ name, so the short (nested) names suffice.
    opaque_f("skia::textlayout::Affinity", &["textlayout"]),
    scanned_f("skia::textlayout::TextAlign", NONE, &["textlayout"]), scanned_f("skia::textlayout::PositionWithAffinity", NONE, &["textlayout"]), scanned_f("skia::textlayout::TextBaseline", NONE, &["textlayout"]), scanned_f("skia::textlayout::TextDecorationStyle", FUNCTIONS, &["textlayout"]), scanned_f("skia::textlayout::TextDecorationMode", FUNCTIONS, &["textlayout"]), scanned_f("SkFlattenable::Type", NONE, &[]), opaque_f("skia::textlayout::StyleType", &["textlayout"]),
    // Enums referenced from skia-bindings' Rust sources (defaults.rs,
    // impls.rs). bindgen does not report enums via `new_item_found`, and
    // without recursive allowlisting they are only generated when
    // explicitly allowlisted, so each gets an Include entry.
    scanned_f("SkAlphaType", FIELDS, &[]), scanned_f("SkArc_Type", FIELDS, &[]), scanned_f("SkBlendMode", FIELDS, &[]), opaque_f("SkBlendModeCoeff", &[]),
    scanned_f("SkBlurStyle", FIELDS, &[]), scanned_f("SkCanvas_Lattice_RectType", FIELDS, &[]), scanned_f("SkClipOp", FIELDS, &[]), scanned_f("SkPaint_Cap", FIELDS, &[]), scanned_f("SkPaint_Join", FIELDS, &[]), scanned_f("SkParsePath_PathEncoding", FIELDS, &[]), scanned_f("SkPath_Verb", FIELDS_FUNCTIONS, &[]), scanned_f("SkPathDirection", FIELDS, &[]), scanned_f("SkPathFillType", FIELDS, &[]), scanned_f("SkPathVerb", FIELDS, &[]), opaque_f("SkPDF::Metadata::CompressionLevel", &["pdf"]),
    scanned_f("SkTileMode", FIELDS, &[]), scanned_f("SkYUVColorSpace", FIELDS, &[]), // ------------------------------------------------------------------
    // Core types implicitly present under recursive allowlisting that
    // skia-bindings' Rust sources (lib.rs, defaults.rs, impls.rs) and
    // the C_* wrapper signatures reference. Harvested from compile
    // errors in non-recursive mode; classified Include (member access
    // or by-value use).
    // ------------------------------------------------------------------
    scanned_f("GrBackendApi", FIELDS, &[]), scanned_f("GrBackendDrawableInfo", FIELDS, &[]), scanned_f("GrBackendFormat", FIELDS_FUNCTIONS, &[]), scanned_f("GrBackendRenderTarget", FIELDS, &[]), scanned_f("GrBackendSemaphore", FIELDS, &[]), scanned_f("GrBackendTexture", FIELDS, &[]), scanned_f("GrContextOptions", FIELDS, &[]), scanned_f("GrDirectContext_DirectContextID", FIELDS, &[]), opaque_f("GrDrawingManager", &[]),
    scanned_f("GrFlushInfo", FIELDS, &[]), opaque_f("GrGLBackendState", &[]),
    scanned_f("GrGLExtensions", FIELDS_FUNCTIONS, &[]), scanned_f("GrGLFramebufferInfo", FIELDS, &[]), scanned_f("GrGLSurfaceInfo", FIELDS, &[]), scanned_f("GrGLTextureInfo", FIELDS, &[]), scanned_f("GrMtlBackendContext", FIELDS, &[]), opaque_f("GrMTLHandle", &[]),
    opaque_f("GrMTLPixelFormat", &[]),
    scanned_f("GrMtlSurfaceInfo", FIELDS, &[]), scanned_f("GrMtlTextureInfo", FIELDS, &[]), opaque_f("GrMTLTextureUsage", &[]),
    opaque_f("GrOnFlushCallbackObject", &[]),
    opaque_f("GrProgramInfo", &[]),
    opaque_f("GrProtected", &[]),
    opaque_f("GrPurgeResourceOptions", &[]),
    opaque_f("GrRenderable", &[]),
    opaque_f("GrSemaphoresSubmitted", &[]),
    opaque_f("GrSurfaceCharacterization", &[]),
    opaque_f("GrSurfaceOrigin", &[]),
    opaque_f("GrThreadSafeCache", &[]),
    scanned_f("GrVkDrawableInfo", FIELDS, &[]), scanned_f("GrVkImageInfo", FIELDS, &[]), scanned_f("GrVkSurfaceInfo", FIELDS, &[]), scanned_f("GrYUVABackendTextureInfo", FIELDS_FUNCTIONS, &[]), scanned_f("sk_sp", FIELDS, &[]), scanned_f("Sk3DView", FIELDS, &[]), scanned_f("SkArc", FIELDS, &[]), opaque_f("SkArenaAlloc", &[]),
    scanned_f("SkAutoCanvasRestore", FIELDS, &[]), scanned_f("SkBlender", FIELDS, &[]), scanned_f("SkCanvas", FIELDS, &[]), opaque_f("SkCanvas_PointMode", &[]),
    scanned_f("SkCanvas_SaveLayerRec", FIELDS, &[]), opaque_f("SkCanvas_SrcRectConstraint", &[]),
    scanned_f("SkCodec", FIELDS, &[]), scanned_f("SkCodec_FrameInfo", FIELDS, &[]), opaque_f("SkCodec_IsAnimated", &[]),
    scanned_f("SkCodec_Options", FIELDS, &[]), opaque_f("SkCodec_Result", &[]),
    opaque_f("SkCodec_SelectionPolicy", &[]),
    opaque_f("SkCodec_SkScanlineOrder", &[]),
    opaque_f("SkCodecs::DecodeContext", &[]),
    scanned_f("SkCodecs::Decoder", FUNCTIONS, &[]), scanned_f("SkColor", FIELDS_FUNCTIONS, &[]), scanned_f("SkColor4f", FIELDS, &[]), opaque_f("SkColorChannel", &[]),
    opaque_f("SkColorChannelFlag", &[]),
    opaque_f("SkColorFilters_Clamp", &[]),
    scanned_f("SkColorInfo", FIELDS, &[]), scanned_f("SkColorMatrix", FIELDS, &[]), scanned_f("SkColorSpace", FIELDS, &[]), scanned_f("SkColorSpacePrimaries", FIELDS, &[]), scanned_f("SkColorTable", FIELDS, &[]), scanned_f("SkColorType", FIELDS, &[]), scanned_f("SkContext", FIELDS, &[]), scanned_f("SkContextOptions", FIELDS, &[]), scanned_f("SkContourMeasureIter", FIELDS_FUNCTIONS, &[]), opaque_f("SkCoverageMode", &[]),
    scanned_f("SkCubicMap", FIELDS_FUNCTIONS, &[]), scanned_f("SkCustomTypefaceBuilder", FIELDS_FUNCTIONS, &[]), scanned_f("SkData", FIELDS, &[]), opaque_f("SkDeserialProcs", &[]),
    scanned_f("SkDynamicMemoryWStream", FIELDS, &[]), opaque_f("SkEncodedImageFormat", &[]),
    scanned_f("SkEncodedOrigin", FIELDS, &[]), opaque_f("SkFilterMode", &[]),
    scanned_f("SkFont", FIELDS_FUNCTIONS, &[]), opaque_f("SkFont_Edging", &[]),
    scanned_f("SkFontArguments", FIELDS, &[]), scanned_f("SkFontArguments_Palette", FIELDS, &[]), scanned_f("SkFontArguments_VariationPosition", FIELDS, &[]), scanned_f("SkFontArguments_VariationPosition_Coordinate", FIELDS, &[]), opaque_f("SkFontHinting", &[]),
    scanned_f("SkFontParameters_Variation_Axis", FIELDS, &[]), scanned_f("SkFontStyle", FIELDS, &[]), opaque_f("SkFontStyle_Slant", &[]),
    opaque_f("SkFontTableTag", &[]),
    scanned_f("SkFourByteTag", FIELDS, &[]), opaque_f("SkGlyphID", &[]),
    scanned_f("SkGradient_Interpolation", FIELDS, &[]), opaque_f("SkGraphics", &[]),
    scanned_f("SkHighContrastConfig", FIELDS, &[]), scanned_f("SkImage", FIELDS, &[]), opaque_f("SkImage_AsyncReadResult", &[]),
    opaque_f("SkImage_CachingHint", &[]),
    scanned_f("SkImage_RequiredProperties", FIELDS, &[]), scanned_f("SkImageFilter", FIELDS, &[]), opaque_f("SkImageFilter_MapDirection", &[]),
    opaque_f("SkImageFilters_Dither", &[]),
    scanned_f("SkImageGenerator", FIELDS, &[]), scanned_f("SkImageInfo", FIELDS, &[]), opaque_f("SkImages::BitDepth", &[]),
    scanned_f("SkIPoint", FIELDS, &[]), scanned_f("SkIRect", FIELDS, &[]), scanned_f("SkISize", FIELDS, &[]), opaque_f("SkJpegEncoder::AlphaOption", &[]),
    opaque_f("SkJpegEncoder::Downsample", &[]), opaque_f("SkJSONWriter", &[]),
    scanned_f("SkM44", FIELDS, &[]), scanned_f("SkMatrix", FIELDS, &[]), opaque_f("SkMatrix_ScaleToFit", &[]),
    opaque_f("SkMatrix_TypeMask", &[]),
    scanned_f("SkMemoryStream", FIELDS, &[]), opaque_f("SkNamedPrimaries::CicpId", &[]),
    opaque_f("SkNamedTransferFn::CicpId", &[]),
    scanned_f("SkOpBuilder", FIELDS, &[]), scanned_f("SkOrderedFontMgr", FIELDS, &[]), scanned_f("SkPaint", FIELDS_FUNCTIONS, &[]), opaque_f("SkPaint_Style", &[]),
    opaque_f("SkParsePath", &[]),
    scanned_f("SkPath", FIELDS_FUNCTIONS, &[]), scanned_f("SkPath_Iter", FIELDS_FUNCTIONS, &[]), scanned_f("SkPath_RawIter", FIELDS, &[]), opaque_f("SkPath1DPathEffect_Style", &[]),
    scanned_f("SkPathBuilder", FIELDS, &[]), opaque_f("SkPathBuilder_DumpFormat", &[]),
    scanned_f("SkPathContourIter", FIELDS, &[]), opaque_f("SkPathContourIter_Rec", &[]),
    scanned_f("SkPathIter", FIELDS, &[]), opaque_f("SkPathIter_Rec", &[]),
    scanned_f("SkPathMeasure", FIELDS_FUNCTIONS, &[]), opaque_f("SkPathOp", &[]),
    opaque_f("SkPathSegmentMask", &[]),
    opaque_f("SkPDF::AttributeList", &[]),
    included_f("SkPDF::Metadata", &["pdf"]), opaque_f("SkPDF::StructureElementNode", &[]),
    scanned_f("SkPictureRecorder", FIELDS, &[]), scanned_f("SkPixmap", FIELDS, &[]), opaque_f("SkPMColor", &[]),
    scanned_f("SkPngEncoder::FilterFlag", FIELDS, &[]), scanned_f("SkPoint", FIELDS, &[]), scanned_f("SkPoint3", FIELDS, &[]), opaque_f("SkReadBuffer", &[]),
    scanned_f("SkRecorder", FIELDS, &[]), opaque_f("SkRecorder_Type", &[]),
    scanned_f("SkRect", FIELDS, &[]), scanned_f("SkRefCnt", FIELDS, &[]), scanned_f("SkRefCntBase", FIELDS_FUNCTIONS, &[]), scanned_f("SkRegion", FIELDS_FUNCTIONS, &[]), scanned_f("SkRegion_Cliperator", FIELDS_FUNCTIONS, &[]), scanned_f("SkRegion_Iterator", FIELDS_FUNCTIONS, &[]), scanned_f("SkRegion_Spanerator", FIELDS_FUNCTIONS, &[]), scanned_f("SkRRect", FIELDS, &[]), opaque_f("SkRRect_Type", &[]),
    scanned_f("SkRSXform", FIELDS, &[]), scanned_f("SkRuntimeShaderBuilder", FIELDS, &[]), scanned_f("SkSamplingOptions", FIELDS, &[]), scanned_f("SkScalar", FIELDS, &[]), opaque_f("SkSerialProcs", &[]),
    opaque_f("SkShadowFlags", &[]),
    opaque_f("SkShadowUtils", &[]),
    opaque_f("SkShapers::CT_LineBreakMode", &[]),
    scanned_f("SkSize", FIELDS, &[]), included_f("SkSL::Version", &[]), // SkSpan<T> is a template; its phantom field breaks opaque generation
    // (bindgen 2437 family). The skia-safe side uses SkSpan as a type
    // alias over slice pointers, so exclude it.
    // SkSpan<T>: ptr + size = 16 bytes (bindgen recursive shape confirmed).
    included_f("SkSpan", &[]),
    scanned_f("SkStrikeRef", FIELDS, &[]), scanned_f("SkString", FIELDS_FUNCTIONS, &[]), scanned_f("SkStrings", FIELDS, &[]), scanned_f("SkStrokeRec", FIELDS_FUNCTIONS, &[]), scanned_f("SkSurfaceProps", FIELDS_FUNCTIONS, &[]), opaque_f("SkSurfaces::BackendSurfaceAccess", &[]),
    opaque_f("SkSVGCanvas", &[]),
    scanned_f("SkSVGCircle", FIELDS, &[]), scanned_f("SkSVGClipPath", FIELDS, &[]), scanned_f("SkSVGColor", FIELDS, &[]), opaque_f("SkSVGColorspace", &[]),
    opaque_f("SkSVGColorType", &[]),
    scanned_f("SkSVGContainer", FIELDS, &[]), scanned_f("SkSVGDefs", FIELDS, &[]), opaque_f("SkSVGDisplay", &[]),
    scanned_f("SkSVGDOM", FIELDS_FUNCTIONS, &[]), scanned_f("SkSVGEllipse", FIELDS, &[]), scanned_f("SkSVGFe", FIELDS, &[]), scanned_f("SkSVGFeBlend", FIELDS, &[]), opaque_f("SkSVGFeBlend_Mode", &[]),
    scanned_f("SkSVGFeColorMatrix", FIELDS, &[]), opaque_f("SkSVGFeColorMatrixType", &[]),
    scanned_f("SkSVGFeComponentTransfer", FIELDS, &[]), scanned_f("SkSVGFeComposite", FIELDS, &[]), opaque_f("SkSVGFeCompositeOperator", &[]),
    scanned_f("SkSVGFeDiffuseLighting", FIELDS, &[]), scanned_f("SkSVGFeDisplacementMap", FIELDS, &[]), opaque_f("SkSVGFeDisplacementMap_ChannelSelector", &[]),
    scanned_f("SkSVGFeDistantLight", FIELDS, &[]), scanned_f("SkSVGFeFlood", FIELDS, &[]), scanned_f("SkSVGFeFunc", FIELDS, &[]), opaque_f("SkSVGFeFuncType", &[]),
    scanned_f("SkSVGFeGaussianBlur", FIELDS, &[]), scanned_f("SkSVGFeGaussianBlur_StdDeviation", FIELDS, &[]), scanned_f("SkSVGFeImage", FIELDS, &[]), scanned_f("SkSVGFeInputType", FIELDS, &[]), scanned_f("SkSVGFeLighting", FIELDS, &[]), scanned_f("SkSVGFeLighting_KernelUnitLength", FIELDS, &[]), opaque_f("SkSVGFeLightSource", &[]),
    scanned_f("SkSVGFeMerge", FIELDS, &[]), scanned_f("SkSVGFeMergeNode", FIELDS, &[]), scanned_f("SkSVGFeMorphology", FIELDS, &[]), opaque_f("SkSVGFeMorphology_Operator", &[]),
    scanned_f("SkSVGFeMorphology_Radius", FIELDS, &[]), scanned_f("SkSVGFeOffset", FIELDS, &[]), scanned_f("SkSVGFePointLight", FIELDS, &[]), scanned_f("SkSVGFeSpecularLighting", FIELDS, &[]), scanned_f("SkSVGFeSpotLight", FIELDS, &[]), scanned_f("SkSVGFeTurbulence", FIELDS, &[]), scanned_f("SkSVGFeTurbulenceBaseFrequency", FIELDS, &[]), scanned_f("SkSVGFeTurbulenceType", FIELDS, &[]), scanned_f("SkSVGFillRule", FIELDS, &[]), scanned_f("SkSVGFilter", FIELDS, &[]), scanned_f("SkSVGFontFamily", FIELDS, &[]), scanned_f("SkSVGFontSize", FIELDS, &[]), scanned_f("SkSVGFontStyle", FIELDS, &[]), scanned_f("SkSVGFontWeight", FIELDS, &[]), scanned_f("SkSVGFuncIRI", FIELDS, &[]), scanned_f("SkSVGG", FIELDS, &[]), scanned_f("SkSVGGradient", FIELDS, &[]), opaque_f("SkSVGHiddenContainer", &[]),
    scanned_f("SkSVGImage", FIELDS, &[]), opaque_f("SkSVGIntegerType", &[]),
    scanned_f("SkSVGIRI", FIELDS, &[]), opaque_f("SkSVGIRI_Type", &[]),
    scanned_f("SkSVGLength", FIELDS, &[]), scanned_f("SkSVGLine", FIELDS, &[]), scanned_f("SkSVGLinearGradient", FIELDS, &[]), opaque_f("SkSVGLineCap", &[]),
    scanned_f("SkSVGLineJoin", FIELDS, &[]), scanned_f("SkSVGMask", FIELDS, &[]), opaque_f("SkSVGNumberType", &[]),
    scanned_f("SkSVGObjectBoundingBoxUnits", FIELDS, &[]), scanned_f("SkSVGPaint", FIELDS, &[]), scanned_f("SkSVGPath", FIELDS, &[]), scanned_f("SkSVGPattern", FIELDS, &[]), scanned_f("SkSVGPoly", FIELDS, &[]), scanned_f("SkSVGPreserveAspectRatio", FIELDS, &[]), scanned_f("SkSVGRadialGradient", FIELDS, &[]), scanned_f("SkSVGRect", FIELDS, &[]), scanned_f("SkSVGSpreadMethod", FIELDS, &[]), scanned_f("SkSVGStop", FIELDS, &[]), opaque_f("SkSVGStringType", &[]),
    scanned_f("SkSVGSVG", FIELDS, &[]), opaque_f("SkSVGSVG_Type", &[]),
    opaque_f("SkSVGTag", &[]),
    scanned_f("SkSVGText", FIELDS, &[]), scanned_f("SkSVGTextAnchor", FIELDS, &[]), scanned_f("SkSVGTextContainer", FIELDS, &[]), scanned_f("SkSVGTextLiteral", FIELDS, &[]), scanned_f("SkSVGTextPath", FIELDS, &[]), scanned_f("SkSVGTransformableNode", FIELDS, &[]), opaque_f("SkSVGTransformType", &[]),
    scanned_f("SkSVGTSpan", FIELDS, &[]), scanned_f("SkSVGUse", FIELDS, &[]), opaque_f("SkSVGValue", &[]),
    opaque_f("SkSVGViewBoxType", &[]),
    scanned_f("SkSVGVisibility", FIELDS, &[]), opaque_f("SkSVGXmlSpace", &[]),
    opaque_f("SkTableMaskFilter", &[]),
    scanned_f("SkTextBlob", FIELDS, &[]), scanned_f("SkTextBlob_Iter", FIELDS_FUNCTIONS, &[]), scanned_f("SkTextBlobBuilder", FIELDS_FUNCTIONS, &[]), scanned_f("SkTextBlobBuilderRunHandler", FIELDS, &[]), scanned_f("SkTextEncoding", FIELDS, &[]), opaque_f("SkTextureCompressionType", &[]),
    opaque_f("SkTextUtils", &[]),
    opaque_f("SkTrimPathEffect_Mode", &[]),
    scanned_f("SkTypeface", FIELDS, &[]), scanned_f("SkTypeface_LocalizedStrings", FIELDS, &[]), opaque_f("SkTypeface_SerializeBehavior", &[]),
    opaque_f("SkUnichar", &[]),
    scanned_f("SkV2", FIELDS, &[]), scanned_f("SkV3", FIELDS, &[]), scanned_f("SkV4", FIELDS, &[]), opaque_f("SkVector", &[]),
    scanned_f("SkVertices", FIELDS, &[]), scanned_f("SkVertices_Builder", FIELDS_FUNCTIONS, &[]), opaque_f("SkVertices_VertexMode", &[]),
    opaque_f("SkWebpEncoder::Compression", &[]),
    scanned_f("SkYUVAInfo", FIELDS_FUNCTIONS, &[]), opaque_f("SkYUVAInfo_PlaneConfig", &[]),
    scanned_f("SkYUVAInfo_Subsampling", FIELDS, &[]), opaque_f("SkYUVAInfo_YUVALocations", &[]),
    scanned_f("SkYUVAPixmapInfo", FIELDS_FUNCTIONS, &[]), opaque_f("SkYUVAPixmapInfo_DataType", &[]),
    opaque_f("SkYUVAPixmapInfo_PlaneConfig", &[]),
    scanned_f("SkYUVAPixmapInfo_SupportedDataTypes", FIELDS, &[]), scanned_f("SkYUVAPixmaps", FIELDS, &[]), opaque_f("VkBool32", &[]),
    opaque_f("VkBuffer", &[]),
    opaque_f("VkChromaLocation", &[]),
    opaque_f("VkCommandBuffer_T", &[]),
    opaque_f("VkComponentSwizzle", &[]),
    opaque_f("VkFilter", &[]),
    opaque_f("VkFlags", &[]),
    opaque_f("VkFormat", &[]),
    opaque_f("VkFormatFeatureFlags", &[]),
    opaque_f("VkImage_T", &[]),
    scanned_f("VkImageLayout", FIELDS, &[]), opaque_f("VkPhysicalDevice_T", &[]),
    opaque_f("VkQueue_T", &[]),
    opaque_f("VkRenderPass_T", &[]),
    opaque_f("VkSamplerYcbcrModelConversion", &[]),
    opaque_f("VkSamplerYcbcrRange", &[]),
    opaque_f("VkSemaphore_T", &[]),

    opaque("AsyncReadResult"),
    opaque("AutoUpdateQRBounds"),
    opaque("Axis"),
    opaque("BackImage"),
    opaque("Builder"),
    opaque("Cliperator"),
    opaque("Coordinate"),
    opaque("Desc"),
    opaque("DirectContextID"),
    opaque("ExperimentalRun"),
    opaque("FrameInfo"),
    opaque("GlyphRec"),
    opaque("ImageInfo"),
    opaque("ImageSetEntry"),
    opaque("Impl"),
    opaque("Interpolation"),
    opaque("IterRec"),
    opaque("Iterator"),
    opaque("KernelUnitLength"),
    opaque("Lattice"),
    opaque("Layer"),
    opaque("LocalizedString"),
    opaque("MCRec"),
    opaque("Override"),
    opaque("Palette"),
    opaque("PatternAttributes"),
    opaque("Radius"),
    opaque("RangeIter"),
    opaque("RawIter"),
    opaque("RefCntVars"),
    opaque("RequiredProperties"),
    opaque("Run"),
    opaque("RunBuffer"),
    opaque("RunHead"),
    opaque("RunRecord"),
    opaque("SaveLayerRec"),
    opaque("Sizes"),
    opaque("Spanerator"),
    opaque("StdDeviation"),
    opaque("SupportedDataTypes"),
    opaque("VariationPosition"),
    opaque("YUVALocation"),

    // ------------------------------------------------------------------
    // Wave 5: namespace-mangled module types (skia::textlayout, skottie,
    // sksg, skresources, skgpu, SkShapers, SkCodecs, ...) and remaining
    // core types, harvested from compile errors in non-recursive mode.
    // ------------------------------------------------------------------
    opaque_f("GetProcFnVoidPtr", &[]),
    opaque_f("GLGetProcFnVoidPtr", &[]),
    opaque_f("GrColorFormatDesc", &[]),
    scanned_f("GrDirectContext", FIELDS, &[]), opaque_f("GrDirectContextDestroyedContext", &[]),
    opaque_f("GrDirectContextDestroyedProc", &[]),
    opaque_f("GrDriverBugWorkarounds", &[]),
    opaque_f("GrEGLDisplay", &[]),
    opaque_f("GrGLStandard", &[]),
    opaque_f("GrGLuint", &[]),
    opaque_f("GrGpuFinishedContext", &[]),
    opaque_f("GrGpuFinishedProc", &[]),
    opaque_f("GrGpuFinishedWithStatsProc", &[]),
    opaque_f("GrGpuSubmittedContext", &[]),
    opaque_f("GrGpuSubmittedProc", &[]),
    opaque_f("GrMTLStorageMode", &[]),
    opaque_f("GrTextureType", &[]),
    scanned_f("IndexedStyleMetrics", FIELDS, &[]), opaque_f("OptionalU64", &[]),
    scanned_f("RustResourceProvider", FIELDS, &[]), scanned_f("RustResourceProvider_Param", FIELDS, &[]), scanned_f("RustRunHandler", FIELDS, &[]), scanned_f("RustRunHandler_Param", FIELDS, &[]), scanned_f("RustStream", FIELDS_FUNCTIONS, &[]), scanned_f("RustWStream", FIELDS_FUNCTIONS, &[]), opaque_f("ShaderBuilderUniformResult", &[]),
    scanned_f("Sink", FIELDS_FUNCTIONS, &[]), // Template type (T phantom breaks opaque generation, bindgen 2437 family).
    excluded("sk_cfp"),
    scanned_f("SkBitmap", FIELDS_FUNCTIONS, &[]), scanned_f("SkCamera3D", FIELDS_FUNCTIONS, &[]), opaque_f("SkCapabilities", &[]),
    opaque_f("skcms_AlphaFormat", &[]),
    opaque_f("skcms_ICCProfile", &[]),
    opaque_f("skcms_Matrix3x3", &[]),
    included_f("skcms_TransferFunction", &[]), scanned_f("SkCodecAnimation::Blend", FUNCTIONS, &[]), opaque_f("SkCodecAnimation::DisposalMethod", &[]),
    scanned_f("skcpu::Recorder", FUNCTIONS, &[]), scanned_f("SkCubicResampler", FIELDS, &[]), opaque_f("SkDeque", &[]),
    opaque_f("SkDeque_Iter", &[]),
    opaque_f("SkDescriptor", &[]),
    opaque_f("SkDevice", &[]),
    opaque_f("SkDrawShadowRec", &[]),
    opaque_f("SkEncodedInfo", &[]),
    opaque_f("SkExecutor", &[]),
    scanned_f("SkFontMetrics", FIELDS, &[]), scanned_f("skgpu::BackendApi", NONE, &[]), scanned_f("skgpu::Budgeted", FIELDS, &[]), scanned_f("skgpu::GpuStatsFlags", FUNCTIONS, &[]), scanned_f("skgpu::graphite::BackendTexture", FUNCTIONS, &[]), scanned_f("skgpu::graphite::Buffer", NONE, &[]), opaque_f("skgpu::graphite::Caps", &["graphite"]),
    scanned_f("skgpu::graphite::Context", FUNCTIONS, &[]), scanned_f("skgpu::graphite::ContextOptions", FIELDS_FUNCTIONS, &[]), scanned_f("skgpu::graphite::Device", NONE, &[]), opaque_f("skgpu::graphite::ImageProvider", &["graphite"]),
    opaque_f("skgpu::graphite::InsertRecordingInfo", &["graphite"]),
    opaque_f("skgpu::graphite::InsertStatus_V", &["graphite"]),
    opaque_f("skgpu::graphite::MtlBackendContext", &["graphite"]),
    scanned_f("skgpu::graphite::Recorder", FUNCTIONS, &[]), scanned_f("skgpu::graphite::RecorderOptions", FUNCTIONS, &[]), opaque_f("skgpu::graphite::RecordingPriv", &["graphite"]),
    scanned_f("skgpu::graphite::ResourceProvider", NONE, &[]), opaque_f("skgpu::graphite::RuntimeEffectDictionary", &["graphite"]),
    opaque_f("skgpu::graphite::SharedContext", &["graphite"]),
    scanned_f("skgpu::graphite::SubmitInfo", FIELDS_FUNCTIONS, &[]), opaque_f("skgpu::graphite::Texture", &["graphite"]),
    scanned_f("skgpu::graphite::TextureInfo", FIELDS_FUNCTIONS, &[]), opaque_f("skgpu::graphite::TextureProxy", &["graphite"]),
    scanned_f("skgpu::Mipmapped", NONE, &[]), opaque_f("skgpu::Origin", &["vulkan", "graphite"]),
    opaque_f("skgpu::Protected", &["vulkan", "graphite"]),
    opaque_f("skgpu::RefCntedCallback", &["vulkan", "graphite"]),
    opaque_f("skgpu::VulkanAlloc", &["vulkan", "graphite"]),
    opaque_f("skgpu::VulkanYcbcrConversionInfo", &["vulkan", "graphite"]),
    scanned_f("skia::textlayout::Block", FIELDS, &[]), scanned_f("skia::textlayout::FontArguments", FIELDS_FUNCTIONS, &[]), scanned_f("skia::textlayout::FontFeature", FUNCTIONS, &[]), scanned_f("skia::textlayout::LineMetrics", FUNCTIONS, &[]), scanned_f("skia::textlayout::Paragraph", FUNCTIONS, &[]), opaque_f("skia::textlayout::Paragraph_ExtendedVisitorInfo", &["textlayout"]),
    opaque_f("skia::textlayout::Paragraph_GlyphInfo", &["textlayout"]),
    opaque_f("skia::textlayout::Paragraph_VisitorInfo", &["textlayout"]),
    scanned_f("skia::textlayout::ParagraphBuilder", FUNCTIONS, &[]), opaque_f("skia::textlayout::ParagraphImpl", &["textlayout"]),
    scanned_f("skia::textlayout::ParagraphStyle", FUNCTIONS, &[]), scanned_f("skia::textlayout::Placeholder", FIELDS, &[]), scanned_f("skia::textlayout::PlaceholderStyle", FIELDS, &[]), scanned_f("skia::textlayout::RectHeightStyle", FIELDS, &[]), scanned_f("skia::textlayout::RectWidthStyle", FIELDS, &[]), scanned_f("skia::textlayout::StrutStyle", FUNCTIONS, &[]), opaque_f("skia::textlayout::TextIndex", &["textlayout"]),
    scanned_f("skia::textlayout::TextShadow", FIELDS_FUNCTIONS, &[]), scanned_f("skia::textlayout::TextStyle", FUNCTIONS, &[]), scanned_f("skia::textlayout::TypefaceFontProvider", FUNCTIONS, &[]), scanned_f("skia::textlayout::TypefaceFontStyleSet", FUNCTIONS, &[]), opaque_f("SkIDChangeListener", &[]),
    opaque_f("skjson::ObjectValue", &[]),
    opaque_f("SkMask", &[]),
    opaque_f("SkMesh", &[]),
    opaque_f("SkMipmapMode", &[]),
    opaque_f("SkNoncopyable", &[]),
    opaque_f("SkOnce", &[]),
    scanned_f("skottie::Animation", FUNCTIONS, &["skottie"]), opaque_f("skottie::Animation_Builder", &["skottie"]),
    opaque_f("skottie::ExpressionManager", &["skottie"]),
    opaque_f("skottie::GlyphDecorator", &["skottie"]),
    opaque_f("skottie::GlyphDecorator_GlyphInfo", &["skottie"]),
    opaque_f("skottie::internal::AnimatablePropertyContainer", &["skottie"]),
    opaque_f("skottie::internal::AnimationBuilder", &["skottie"]),
    opaque_f("skottie::internal::CustomFont::GlyphCompMapper", &["skottie"]),
    opaque_f("skottie::internal::SceneGraphRevalidator", &["skottie"]),
    opaque_f("skottie::LayerInfo", &["skottie"]),
    opaque_f("skottie::Logger", &["skottie"]),
    opaque_f("skottie::MarkerObserver", &["skottie"]),
    opaque_f("skottie::PrecompInterceptor", &["skottie"]),
    opaque_f("skottie::PropertyObserver", &["skottie"]),
    scanned_f("skottie::ResourceProvider", NONE, &["skottie"]), opaque_f("skottie::Shaper_ShapedGlyphs", &["skottie"]),
    opaque_f("skottie::SlotManager", &["skottie"]),
    opaque_f("skottie::SlotManager_ImageAssetProxy", &["skottie"]),
    opaque_f("skottie::TextValue", &["skottie"]),
    opaque_f("SkPathConvexity", &[]),
    opaque_f("SkPathData", &[]),
    opaque_f("SkPathIsAData", &[]),
    opaque_f("SkPathIsAType", &[]),
    opaque_f("SkPathRaw", &[]),
    scanned_f("SkPixelGeometry", FIELDS, &[]), opaque_f("SkPngChunkReader", &[]),
    opaque_f("SkRecord", &[]),
    opaque_f("skresources::CachingResourceProvider", &["svg"]),
    opaque_f("skresources::ExternalTrackAsset", &["svg"]),
    scanned_f("skresources::ImageAsset", FUNCTIONS, &["svg"]), opaque_f("skresources::ImageDecodeStrategy", &["svg"]),
    opaque_f("skresources::MultiFrameImageAsset", &["svg"]),
    scanned_f("SkRuntimeEffectBuilder", FIELDS, &[]), opaque_f("SkScalerContextEffects", &[]),
    opaque_f("sksg::ExternalImageFilter", &["skottie"]),
    opaque_f("sksg::Group", &["skottie"]),
    scanned_f("sksg::ImageFilter", FUNCTIONS, &["skottie"]), scanned_f("sksg::Matrix", FIELDS_FUNCTIONS, &["skottie"]), scanned_f("sksg::Node", FUNCTIONS, &["skottie"]), opaque_f("sksg::RenderNode", &["skottie"]),
    scanned_f("sksg::Shader", FUNCTIONS, &["skottie"]), opaque_f("sksg::Transform", &["skottie"]),
    opaque_f("sksg::TransformEffect", &["skottie"]),
    opaque_f("SkShapers::Factory", &[]),
    opaque_f("SkSharedContext", &[]),
    opaque_f("SkSL::DebugTrace", &[]),
    opaque_f("SkSpecialImage", &[]),
    opaque_f("SkStrike", &[]),
    opaque_f("SkSurface_Base", &[]),
    opaque_f("SkSVGAttribute", &[]),
    opaque_f("SkSVGFeColorMatrixValues", &[]),
    opaque_f("SkSVGFilterContext", &[]),
    opaque_f("SkSVGIDMapper", &[]),
    opaque_f("SkSVGLengthContext", &[]),
    opaque_f("SkSVGPointsType", &[]),
    opaque_f("SkSVGPresentationContext", &[]),
    opaque_f("SkSVGRenderContext", &[]),
    scanned_f("SkSVGShape", FIELDS, &[]), opaque_f("SkSVGTextFragment", &[]),
    // Template type (T phantom breaks opaque generation, bindgen 2437 family).
    opaque_f("sktext::gpu::SubRunAllocator", &[]),
    opaque_f("sktext::gpu::TextBlobRedrawCoordinator", &[]),
    opaque_f("SkTraceMemoryDump", &[]),
    opaque_f("SkTypefaceID", &[]),
    opaque_f("SkWriteBuffer", &[]),
    opaque_f("std::byte", &[]),
    opaque_f("std::basic_string<char>", &[]),
    opaque_f("std::basic_string<wchar_t>", &[]),
    opaque_f("std::basic_string<char16_t>", &[]),
    opaque_f("std::basic_string_view<char>", &[]),
    opaque_f("std::true_type", &[]),
    opaque_f("T", &[]),
    opaque_f("U8CPU", &[]),
    opaque_f("va_list", &[]),
    scanned_f("VecSink", FIELDS_FUNCTIONS, &[]), opaque_f("VkBuffer_T", &[]),
    opaque("AsyncParams"),
    opaque("CropRect"),
    opaque("BorrowedNode"),
    opaque("BuilderChild"),
    opaque("BuilderUniform"),
    opaque("ContextID"),
    opaque("Data"),
    opaque("DeleteCallbackHelper"),
    opaque("ExtendedVisitorInfo"),
    opaque("FontInfo"),
    opaque("FrameData"),
    opaque("GlyphClusterInfo"),
    opaque("GlyphInfo"),
    opaque("ImageAssetProxy"),
    opaque("List"),
    opaque("OBBScope"),
    opaque("OBBTransform"),
    opaque("Param"),
    opaque("PixelTransferResult"),
    opaque("RenderContext"),
    opaque("ScopedFlag"),
    opaque("ScopedRenderContext"),
    opaque("ShapedGlyphs"),
    opaque("SlotInfo"),
    opaque("TextInfo"),
    opaque("ValuePair"),
    opaque("VisitorInfo"),

    opaque_f("GrColorTypeEncoding", &[]),
    opaque_f("GrMockOptions", &[]),
    scanned_f("GrSubmitInfo", FIELDS, &[]), opaque_f("GrSyncCpu", &[]),
    opaque_f("SkCapture", &[]),
    opaque_f("skcms_A2B", &[]),
    opaque_f("skcms_B2A", &[]),
    opaque_f("skcms_CICP", &[]),
    opaque_f("skcms_Curve", &[]),
    opaque_f("SkCodecs::IsFormatCallback", &[]),
    opaque_f("SkCodecs::MakeFromStreamCallback", &[]),
    opaque_f("skgpu::ganesh::SmallPathAtlasMgr", &[]),
    scanned_f("skgpu::GpuStats", FIELDS, &[]), scanned_f("skgpu::graphite::BackendSemaphore", FUNCTIONS, &[]), opaque_f("skgpu::graphite::ContextOptionsPriv", &[]),
    opaque_f("skgpu::graphite::ContextPriv", &[]),
    opaque_f("skgpu::graphite::GpuFinishedContext", &[]),
    opaque_f("skgpu::graphite::GpuFinishedProc", &[]),
    opaque_f("skgpu::graphite::GpuFinishedWithStatsProc", &[]),
    opaque_f("skgpu::graphite::InsertFinishInfo", &[]),
    scanned_f("skgpu::graphite::InsertStatus", NONE, &[]), opaque_f("skgpu::graphite::MarkFrameBoundary", &[]),
    opaque_f("skgpu::graphite::PersistentPipelineStorage", &[]),
    opaque_f("skgpu::graphite::RecorderOptionsPriv", &[]),
    opaque_f("skgpu::graphite::RecorderPriv", &[]),
    opaque_f("skgpu::graphite::SampleCount", &[]),
    opaque_f("skgpu::graphite::SyncToCpu", &[]),
    opaque_f("skgpu::graphite::TextureFormat", &[]),
    opaque_f("skgpu::ShaderErrorHandler", &[]),
    opaque_f("skgpu::SingleOwner", &[]),
    opaque_f("skgpu::VulkanBackendMemory", &[]),
    opaque_f("skhdr::Metadata", &[]), opaque_f("skia::textlayout::BlockRange", &[]),
    scanned_f("skia::textlayout::Decoration", FIELDS_FUNCTIONS, &["textlayout"]), opaque_f("skia::textlayout::ParagraphPainter_SkPaintOrID", &[]),
    scanned_f("skia::textlayout::PlaceholderAlignment", FIELDS, &["textlayout"]), scanned_f("skia::textlayout::StyleMetrics", FIELDS_FUNCTIONS, &["textlayout"]), scanned_f("skia::textlayout::TextBox", FIELDS, &["textlayout"]), scanned_f("skia::textlayout::TextDirection", NONE, &["textlayout"]), scanned_f("skia::textlayout::TextHeightBehavior", NONE, &["textlayout"]), scanned_f("skia::textlayout::TextRange", NONE, &["textlayout"]), opaque_f("SkImageFilters_CropRect", &[]),
    opaque_f("SkIVector", &[]),
    opaque_f("SkMutex", &[]),
    scanned_f("SkNVRefCnt", FIELDS, &[]), opaque_f("skottie::TextPropertyValue", &[]),
    scanned_f("SkPatch3D", FIELDS_FUNCTIONS, &[]), opaque_f("skresources::ResourceProviderProxyBase", &[]),
    opaque_f("sksg::EffectNode", &[]),
    opaque_f("sksg::InvalidationController", &[]),
    opaque_f("SkSVGPresentationAttributes", &[]),
    opaque_f("SkSVGTextContext", &[]),
    opaque_f("SkTDStorage", &[]),
    opaque_f("std::chrono::milliseconds", &[]),
    scanned_f("TraitObject", FIELDS, &[]), opaque_f("VkDevice", &[]),
    opaque_f("VkDeviceMemory", &[]),
    opaque_f("VkDeviceSize", &[]),
    opaque_f("VkInstance", &[]),
    opaque_f("VkSamplerYcbcrConversionCreateInfo", &[]),

    opaque_f("GrMarkFrameBoundary", &[]),
    opaque_f("VkDevice_T", &[]),
    opaque_f("VkDeviceMemory_T", &[]),
    opaque_f("VkInstance_T", &[]),
    opaque_f("VkStructureType", &[]),

    opaque_f("CFTypeRef", &[]),
    opaque_f("skcms_Matrix3x4", &[]),
    opaque_f("skgpu::CallbackResult", &[]),
    opaque_f("skhdr::AdaptiveGlobalToneMap", &[]),
    opaque_f("skhdr::ContentLightLevelInformation", &[]),
    opaque_f("skhdr::MasteringDisplayColorVolume", &[]),
    // Template type (T phantom breaks opaque generation, bindgen 2437 family).
    excluded("skia::textlayout::SkRange"),
    scanned_f("skia::textlayout::TextDecoration", FUNCTIONS, &[]), opaque_f("skottie::internal::CustomFont_GlyphCompMapper", &[]),
    opaque_f("skottie::Shaper_Capitalization", &[]),
    opaque_f("skottie::Shaper_Direction", &[]),
    opaque_f("skottie::Shaper_LinebreakPolicy", &[]),
    opaque_f("skottie::Shaper_ResizePolicy", &[]),
    opaque_f("skottie::Shaper_VAlign", &[]),
    opaque_f("skottie::TextPaintOrder", &[]),
    opaque_f("SkShapers::CT::LineBreakMode", &[]),
    // std::chrono::duration is a template; generating it opaque trips the
    // bindgen "opaque with fields" assertion (its fields are the template
    // parameter phantoms, bindgen issue 2437 family). Exclude, per the
    // opaque-parent + C_ wrapper approach.
    excluded("std::chrono::duration"),
    excluded("std::chrono::duration<_Rep"),
    opaque_f("std::milli", &[]),
    opaque_f("std::nano", &[]),
    opaque_f("std::micro", &[]),
    opaque_f("std::ratio", &[]),
    opaque_f("std::__1::ratio", &[]),
    opaque_f("__1::chrono::duration_rep", &[]),
    opaque_f("__1::chrono::duration_period", &[]),
    opaque_f("std::chrono::duration::_Period", &[]),
    opaque_f("std::chrono::duration::_Rep", &[]),

    opaque("AlternateImage"),
    opaque("ColorGainFunction"),
    opaque("ComponentMixingFunction"),
    opaque("ControlPoint"),
    opaque("GainCurve"),
    opaque("HeadroomAdaptiveToneMap"),

    opaque_f("__builtin_va_list", &[]),
    opaque_f("std::make_signed_t", &[]),

    excluded("__no_overflow"),
    // Sized definitions for std::-templates whose *layout* is needed by
    // transmuted parents (native_transmutable\!), but whose full generation
    // trips the bindgen opaque/phantom-field assert. Sized via their
    // instantiation shapes on this ABI (ptr = 8 bytes):
    // - std::unique_ptr<T, D>: a raw pointer, 8 bytes.
    // - std::default_delete<T>: empty, 1 byte padded to 1.
    // - std::optional<T>: ptr-sized payload + 1 byte state, 16 w/ align 8.
    // - std::vector<T, A>: 3 pointers, 24.
    // - std::allocator<T>: empty, 1.
    // - std::atomic<T>: T-sized, aligned like T.
    // - std::tuple<T...>: opaque blob, sized by usage (SkPDF etc.) - 16.
    stub_generic("std::unique_ptr", "#[repr(C, align(8))] pub struct std_unique_ptr<T, D>(u8, ::core::marker::PhantomData<(T, D)>);"),
    stub_generic("std::default_delete", "#[repr(C, align(8))] pub struct std_default_delete<T>(u8, ::core::marker::PhantomData<T>);"),
    stub_generic("std::optional", "#[repr(C, align(8))] pub struct std_optional<T>(u8, ::core::marker::PhantomData<T>);"),
    stub_generic("std::vector", "#[repr(C, align(8))] pub struct std_vector<T, A>(u8, ::core::marker::PhantomData<(T, A)>);"),
    stub_generic("std::allocator", "#[repr(C, align(8))] pub struct std_allocator<T>(u8, ::core::marker::PhantomData<T>);"),
    stub_generic("std::atomic", "#[repr(C, align(8))] pub struct std_atomic<T>(T);"),
    stub_generic("std::tuple", "#[repr(C, align(8))] pub struct std_tuple<T>(u8, ::core::marker::PhantomData<T>);"),
    // skia_private::AutoTMalloc<T>: holds one pointer, 8 bytes.
    stub_generic("skia_private::AutoTMalloc", "#[repr(C, align(8))] pub struct skia_private_AutoTMalloc<T, D>(u8, ::core::marker::PhantomData<(T, D)>);"),
    // SkTDArray<T>: pointer + count, 16 bytes, align 8.
    stub_generic("SkTDArray", "#[repr(C, align(8))] pub struct SkTDArray<T>(u8, ::core::marker::PhantomData<T>);"),
    // Function-pointer typedefs in SkPDF (fields of the metadata struct):
    stub_generic("SkPDF::EncodeJpegCallback", "pub type SkPDF_EncodeJpegCallback = u8;"),
    stub_generic("SkPDF::DecodeJpegCallback", "pub type SkPDF_DecodeJpegCallback = u8;"),
    // Skia-internal types pulled into the layout-assert closures of
    // std_unique_ptr/std_default_delete template specializations
    // (SkCodecPriv.h SkCodecs::ColorProfile, SkSL programs, codec streams,
    // pdf types...). Untouched from Rust (scan: no touch points); Opaque
    // keeps the layout asserts computable without generating members.
    opaque_f("SkCodecs::ColorProfile", &[]),
    opaque_f("SkSL::Program", &[]),
    opaque_f("SkSL::RP::Program", &[]),
    opaque_f("SkSL::SampleUsage", &[]),
    opaque_f("sktext::GlyphRunBuilder", &[]),
    opaque_f("SkPDFArray", &[]),
    opaque_f("SkFILEStream", &[]),
    opaque_f("SkRecordCanvas", &[]),
    opaque_f("SkPDF::DateTime", &[]),
    opaque_f("SkOpenTypeSVGDecoder", &[]),
    // Our own FFI request struct in bindings.h: only ever passed as a
    // pointer in C_SkFontMgr_* wrappers.
    opaque_f("C_SkFontMgr_Request", &[]),
    // libc++ internals dragged in as nested aliases of generated template
    // wrappers; opaque keeps the member aliases out of the output. The
    // remaining references (std___conditional_t etc.) are resolved via
    // TEMPLATE_STANDINS in skia_bindgen.rs.
    // Clang AST helper types that leak into the closure when template
    // types are allowlisted (guards from clang's -fdiagnostics / vector
    // destruction internals).
    excluded("_CheckOptionalArgsConstructor"),
    excluded("_CheckOptionalLikeConstructor"),
    excluded("_ConstructTransaction"),
    excluded("__destroy_vector"),
    // libc++ internal helpers/sso members that leak into the closure once
    // basic_string / basic_stringstream templates are allowlisted.
    excluded("__annotate_new_size"),
    excluded("__annotation_guard"),
    excluded("__assume_valid"),
    excluded("__long"),
    excluded("__rep"),
    excluded("__short"),
    excluded("std::__1::basic_stringbuf"),
    excluded("std::__2::basic_stringbuf"),
    excluded("basic_stringbuf"),
    excluded("std::__1::basic_stringstream"),
    excluded("std::__2::basic_stringstream"),
    excluded("basic_stringstream"),

    opaque("Iter"),
    opaque("Pair"),
    opaque("Rec"),

    // ------------------------------------------------------------------
    // Post-StubGeneric wave: types referenced by generated signatures
    // whose parents include std::-carrying members or template typedefs.
    // ------------------------------------------------------------------
    // std:: templates used inside the generated signatures:
    // Template-parameter structs for the basic_string family (bindgen
    // emits `std_char_traits<_CharT>` as a generic parameter type of the
    // generated `std_basic_string` definitions; the parameters themselves
    // must exist in the generated code).
    excluded("std::char_traits"),
    excluded("std::reverse_iterator"),
    excluded("std::less"),
    scanned_f("std::function", FIELDS, &[]), opaque_f("std::string_view", &[]),
    excluded("std::__1::basic_string_view"),
    excluded("std::__1::basic_string"),
    // chrono duration specializations (milli/micro seconds are instantiations
    // of the excluded duration template; sized 8).
    // SkShaper & friends (function-signature types; opaque is fine since
    // skia-safe accesses everything via C_ wrappers).
    opaque_f("SkShaper", SHAPER_FEATURES),
    opaque_f("SkShaper::BiDiRunIterator", SHAPER_FEATURES),
    opaque_f("SkShaper::FontRunIterator", SHAPER_FEATURES),
    opaque_f("std::chrono::microseconds", &[]),
    opaque_f("std::map", &[]),

    // ------------------------------------------------------------------
    // Nested implementation types reported by `new_item_found` under their
    // short name. They are members or private machinery of types that are
    // themselves Opaque/Stub/Exclude, are never referenced from Rust, and
    // exist in the closure only as discovered children of those parents.
    // They are classified Opaque so their size/alignment stays available
    // where bindgen needs it.
    // ------------------------------------------------------------------
    opaque("AnimatedProps"),
    opaque("AnimatedPropsModulator"),
    opaque("Arenas"),
    opaque("Buffer"),
    opaque("CMapEntry"),
    opaque("Cache"),
    opaque("Child"),
    opaque("ChildPtr"),
    opaque("Dir"),
    opaque("DomainMaps"),
    opaque("DomainSpan"),
    opaque("Entry"),
    opaque("F2BIter"),
    opaque("FaceCache"),
    opaque("Feature"),
    opaque("ForwardVerbIterator"),
    opaque("FragmentRec"),
    opaque("Functions"),
    opaque("GlyphDecoratorNode"),
    opaque("LazyProxyData"),
    opaque("Options"),
    opaque("OwnedArenas"),
    opaque("PathInfo"),
    opaque("PersistentCache"),
    opaque("PrivateInitializer"),
    opaque("ProgramData"),
    opaque("ProxyHash"),
    opaque("Range"),
    opaque("Request"),
    opaque("ResolvedProps"),
    opaque("Result"),
    opaque("RunInfo"),
    opaque("Segment"),
    opaque("Stats"),
    opaque("TextValueTracker"),
    opaque("TracedShader"),
    opaque("TrivialRunIterator"),
    opaque("Uniform"),
    opaque("VariationCache"),
    opaque("VerbMeasure"),
];

/// Look up the action for a C++ type name. Specific entries are placed
/// before more general ones in the table and win.
///
/// Kept for the Rust-side touch-point scan (ADR 0001) and tests; the
/// generation path uses `matched_entry_index` instead.
#[allow(dead_code)]
pub(crate) fn action_for(type_name: &str) -> Option<TypeAction> {
    TYPES
        .iter()
        .find(|entry| pattern_matches(entry.name, type_name))
        .map(|entry| entry.action())
}

/// All table entries, regardless of feature set. Called by the bindgen
/// builder setup (feature-specific pruning happens later, when the entries
/// carry per-feature closures that differ per build configuration).
pub(crate) fn entries() -> impl Iterator<Item = &'static TypeEntry> {
    TYPES.iter()
}

/// Index of the table entry that matched `type_name` (not the action),
/// so callers can track which entries actually saw activity. The first
/// matching entry wins, mirroring `action_for`.
///
/// O(1): consults hash maps built once over the table (exact name and
/// last name component), falling back to the linear scan only for the
/// handful of suffix-`.*` prefix patterns and ambiguous last components.
pub(crate) fn matched_entry_index(type_name: &str) -> Option<usize> {
    let maps = lookup_maps();
    if let Some(index) = maps.exact.get(type_name) {
        return Some(*index);
    }
    if let Some(index) = maps.last_component.get(type_name) {
        return Some(*index);
    }
    // Linear fallback for `.*` prefix patterns and ambiguous last
    // components (both rare).
    TYPES
        .iter()
        .position(|entry| pattern_matches(entry.name, type_name))
}

/// The entry names of the table, indexed like `matched_entry_index`.
pub(crate) fn entry_name(index: usize) -> &'static str {
    TYPES[index].name
}

/// The total number of entries (upper bound for the used-marker vector).
pub(crate) fn entry_count() -> usize {
    TYPES.len()
}

/// Lazily built lookup maps: exact C++ name → entry index, and last name
/// component → entry index. Multi-valued last components (two entries like
/// `SkNamedPrimaries::CicpId` and `SkNamedTransferFn::CicpId`) are absent
/// from the map so those lookups take the linear fallback, which resolves
/// first-match-wins exactly like the scan.
fn lookup_maps() -> &'static LookupMaps {
    static MAPS: std::sync::OnceLock<LookupMaps> = std::sync::OnceLock::new();
    MAPS.get_or_init(build_lookup_maps)
}

struct LookupMaps {
    exact: std::collections::HashMap<&'static str, usize>,
    last_component: std::collections::HashMap<&'static str, usize>,
}

fn build_lookup_maps() -> LookupMaps {
    let mut exact = std::collections::HashMap::new();
    let mut last_component = std::collections::HashMap::new();
    for (index, entry) in TYPES.iter().enumerate() {
        if entry.name.ends_with(".*") {
            continue;
        }
        exact.entry(entry.name).or_insert(index);
        let last = entry.name.rsplit("::").next().unwrap_or(entry.name);
        if last != entry.name {
            last_component.entry(last).or_insert(index);
        }
    }
    LookupMaps { exact, last_component }
}

/// Minimal matcher for the bindgen-style patterns used in the table:
/// literal text plus an optional trailing `.*`. Anything else is rejected
/// by the `patterns_are_literal_or_prefix` test rather than mis-matched.
///
/// A qualified entry (`skia::textlayout::FontCollection`) also matches the
/// type's last name component (`FontCollection`), because bindgen reports
/// nested types via their unqualified `original_name` in `new_item_found`.
fn pattern_matches(pattern: &str, name: &str) -> bool {
    let literal = pattern.strip_suffix(".*").unwrap_or(pattern);
    if name.starts_with(literal) && pattern.ends_with(".*") {
        return true;
    }
    if pattern == name {
        return true;
    }
    if !pattern.contains("::") {
        return false;
    }
    pattern.rsplit("::").next() == Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_have_unique_names() {
        let mut names: Vec<_> = TYPES.iter().map(|e| e.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TYPES.len());
    }

    #[test]
    fn patterns_are_literal_or_prefix() {
        // `pattern_matches` only supports literal names and trailing `.*`;
        // any other regex syntax would silently mis-match.
        for entry in TYPES {
            let bare = entry.name.strip_suffix(".*").unwrap_or(entry.name);
            assert!(
                !bare.contains(['*', '.', '?', '+', '[', ']', '(', ')']),
                "unsupported regex syntax in pattern '{}'",
                entry.name
            );
        }
    }

    #[test]
    fn lookup() {
        assert_eq!(action_for("SkSurface"), Some(TypeAction::Opaque));
        assert_eq!(action_for("SkUnicode"), Some(TypeAction::Stub));
        assert_eq!(
            action_for("std::unordered_map<int>"),
            Some(TypeAction::Exclude)
        );
        assert_eq!(action_for("PFN_vkVoidFunction"), Some(TypeAction::Exclude));
        assert_eq!(action_for("SkBitmap"), Some(TypeAction::Include));
    }
}
