mod clip_path;
mod container;
mod defs;
pub mod fe;
mod filter;
mod g;
mod gradient;
mod image;
mod mask;
mod node;
mod node_hierarchy;
mod pattern;
mod shape;
mod stop;
mod svg_;
mod text;
mod transformable_node;
mod types;
mod r#use;

pub use self::{
    clip_path::ClipPath,
    container::Container,
    defs::Defs,
    filter::Filter,
    g::G,
    gradient::*,
    image::Image,
    mask::Mask,
    node::*,
    node_hierarchy::*,
    r#use::Use,
    shape::*,
    stop::Stop,
    svg_::*,
    text::{TSpan, Text, TextLiteral, TextPath},
    transformable_node::TransformableNode,
    types::*,
};

use std::{
    error::Error,
    ffi::CStr,
    fmt,
    io::{self, Read},
    os::raw,
};

use crate::{
    interop::{MemoryStream, NativeStreamBase, RustStream},
    prelude::*,
    Canvas, Data, FontMgr, FontStyle as SkFontStyle, Size,
};

use skia_bindings as sb;
use skia_bindings::{SkData, SkTypeface};

pub type Dom = RCHandle<sb::SkSVGDOM>;

require_base_type!(sb::SkSVGDOM, sb::SkRefCnt);
unsafe_send_sync!(Dom);

impl NativeRefCounted for sb::SkSVGDOM {
    fn _ref(&self) {
        unsafe { sb::C_SkSVGDOM_ref(self) }
    }

    fn _unref(&self) {
        unsafe { sb::C_SkSVGDOM_unref(self) }
    }

    fn unique(&self) -> bool {
        unsafe { sb::C_SkSVGDOM_unique(self) }
    }
}

/// Error when something goes wrong when loading an SVG file. Sadly, Skia doesn't give further
/// details so we can't return a more expressive error type, but we still use this instead of
/// `Option` to express the intent and allow for `Try`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoadError;

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to load svg (reason unknown)")
    }
}

impl Error for LoadError {
    fn description(&self) -> &str {
        "Failed to load svg (reason unknown)"
    }
}

impl From<LoadError> for io::Error {
    fn from(other: LoadError) -> Self {
        io::Error::new(io::ErrorKind::Other, other)
    }
}

#[derive(Debug)]
#[repr(C)]
struct LoadContext {
    font_mgr: FontMgr,
}

impl LoadContext {
    fn new(font_mgr: FontMgr) -> Self {
        Self { font_mgr }
    }

    fn native(&mut self) -> *mut raw::c_void {
        self as *mut _ as _
    }
}

extern "C" fn handle_load_type_face(
    resource_path: *const raw::c_char,
    resource_name: *const raw::c_char,
    load_context: *mut raw::c_void,
) -> *mut SkTypeface {
    let data = Data::from_ptr(handle_load(resource_path, resource_name, load_context));
    let load_context: &mut LoadContext = unsafe { &mut *(load_context as *mut LoadContext) };
    if let Some(data) = data {
        if let Some(typeface) = load_context.font_mgr.new_from_data(&data, None) {
            return typeface.into_ptr();
        }
    }

    load_context
        .font_mgr
        .legacy_make_typeface(None, SkFontStyle::default())
        .unwrap()
        .into_ptr()
}

extern "C" fn handle_load(
    resource_path: *const raw::c_char,
    resource_name: *const raw::c_char,
    _load_context: *mut raw::c_void,
) -> *mut SkData {
    unsafe {
        let mut is_base64 = false;
        if resource_path.is_null() {
            is_base64 = true;
        }

        let resource_path = CStr::from_ptr(resource_path);
        let resource_name = CStr::from_ptr(resource_name);

        if resource_path.to_string_lossy().is_empty() {
            is_base64 = true;
        }

        if cfg!(windows) && !resource_name.to_string_lossy().starts_with("data:") {
            is_base64 = false;
        }

        if is_base64 {
            let data = Dom::handle_load_base64(resource_name.to_string_lossy().as_ref());
            data.into_ptr()
        } else {
            // url returned in the resource_name on windows
            // https://github.com/rust-skia/rust-skia/pull/569#issuecomment-978034696
            let path = if cfg!(windows) {
                resource_name.to_string_lossy().to_string()
            } else {
                format!(
                    "{}/{}",
                    resource_path.to_string_lossy(),
                    resource_name.to_string_lossy()
                )
            };

            match ureq::get(&path).call() {
                Ok(response) => {
                    let mut reader = response.into_reader();
                    let mut data = Vec::new();
                    if reader.read_to_end(&mut data).is_err() {
                        data.clear();
                    };
                    let data = Data::new_copy(&data);
                    data.into_ptr()
                }
                Err(_) => {
                    let data = Data::new_empty();
                    data.into_ptr()
                }
            }
        }
    }
}

impl fmt::Debug for Dom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dom").finish()
    }
}

impl Dom {
    pub fn read<R: io::Read>(
        mut reader: R,
        font_mgr: impl Into<FontMgr>,
    ) -> Result<Self, LoadError> {
        let mut reader = RustStream::new(&mut reader);
        let stream = reader.stream_mut();
        let font_mgr = font_mgr.into();
        let mut load_context = LoadContext::new(font_mgr.clone());

        let out = unsafe {
            sb::C_SkSVGDOM_MakeFromStream(
                stream,
                font_mgr.into_ptr(),
                Some(handle_load),
                Some(handle_load_type_face),
                load_context.native(),
            )
        };

        Self::from_ptr(out).ok_or(LoadError)
    }

    pub fn from_str(svg: impl AsRef<str>, font_mgr: impl Into<FontMgr>) -> Result<Self, LoadError> {
        Self::from_bytes(svg.as_ref().as_bytes(), font_mgr)
    }

    pub fn from_bytes(svg: &[u8], font_mgr: impl Into<FontMgr>) -> Result<Self, LoadError> {
        let mut ms = MemoryStream::from_bytes(svg);
        let font_mgr = font_mgr.into();
        let mut load_context = LoadContext::new(font_mgr.clone());

        let out = unsafe {
            sb::C_SkSVGDOM_MakeFromStream(
                ms.native_mut().as_stream_mut(),
                font_mgr.into_ptr(),
                Some(handle_load),
                Some(handle_load_type_face),
                load_context.native(),
            )
        };
        Self::from_ptr(out).ok_or(LoadError)
    }

    pub fn render(&self, canvas: &Canvas) {
        // TODO: may be we should init ICU whenever we expose a Canvas?
        #[cfg(all(feature = "embed-icudtl", feature = "textlayout"))]
        crate::icu::init();

        unsafe { sb::SkSVGDOM::render(self.native() as &_, canvas.native_mut()) }
    }

    pub fn set_container_size(&mut self, size: impl Into<Size>) {
        let size = size.into();
        unsafe { sb::C_SkSVGDOM_setContainerSize(self.native_mut(), size.native()) }
    }

    fn handle_load_base64(data: &str) -> Data {
        let data: Vec<_> = data.split(',').collect();
        if data.len() > 1 {
            let result = decode_base64(data[1]);
            return Data::new_copy(result.as_slice());
        }
        Data::new_empty()
    }

    pub fn root(&self) -> Svg {
        unsafe {
            Svg::from_unshared_ptr(sb::C_SkSVGDOM_getRoot(self.native()) as *mut _)
                .unwrap_unchecked()
        }
    }
}

type StaticCharVec = &'static [char];

const HTML_SPACE_CHARACTERS: StaticCharVec =
    &['\u{0020}', '\u{0009}', '\u{000a}', '\u{000c}', '\u{000d}'];

// https://github.com/servo/servo/blob/1610bd2bc83cea8ff0831cf999c4fba297788f64/components/script/dom/window.rs#L575
fn decode_base64(value: &str) -> Vec<u8> {
    fn is_html_space(c: char) -> bool {
        HTML_SPACE_CHARACTERS.iter().any(|&m| m == c)
    }
    let without_spaces = value
        .chars()
        .filter(|&c| !is_html_space(c))
        .collect::<String>();

    base64::decode(&without_spaces).unwrap_or_default()
}

mod base64 {
    use base64::{
        alphabet,
        engine::{self, GeneralPurposeConfig},
        Engine,
    };

    pub fn decode(input: &str) -> Result<Vec<u8>, base64::DecodeError> {
        ENGINE.decode(input)
    }

    const ENGINE: engine::GeneralPurpose = engine::GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new().with_decode_allow_trailing_bits(true),
    );
}

// TODO: Prelude candidate.
#[macro_export]
macro_rules! impl_default_make {
    ($type:ty, $make_fn:path) => {
        impl Default for $type {
            fn default() -> Self {
                Self::from_ptr(unsafe { $make_fn() }).unwrap()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write, path::Path};

    use super::Dom;
    use crate::{
        modules::svg::decode_base64,
        surfaces,
        svg::{Length, LengthUnit},
        EncodedImageFormat, FontMgr, Surface,
    };

    #[test]
    fn render_simple_svg() {
        // https://dev.w3.org/SVG/tools/svgweb/samples/svg-files/410.svg
        // Note: height and width must be set
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" height = "256" width = "256">
            <path d="M30,1h40l29,29v40l-29,29h-40l-29-29v-40z" stroke="#;000" fill="none"/>
            <path d="M31,3h38l28,28v38l-28,28h-38l-28-28v-38z" fill="#a23"/>
            <text x="50" y="68" font-size="48" fill="#FFF" text-anchor="middle"><![CDATA[410]]></text>
            </svg>"##;
        let mut surface = surfaces::raster_n32_premul((256, 256)).unwrap();
        let canvas = surface.canvas();
        let font_mgr = FontMgr::new();
        let dom = Dom::from_str(svg, font_mgr).unwrap();
        dom.render(canvas);
        // save_surface_to_tmp(&mut surface);
    }

    #[test]
    fn test_svg_attributes() {
        let data = r#"
            <svg width="100" height="200" viewBox="0 0 1200 800" version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" xml:space="preserve" xmlns:serif="http://www.serif.com/" style="fill-rule:evenodd;clip-rule:evenodd;stroke-linejoin:round;stroke-miterlimit:1.41421;">
                <g id="Layer-1" serif:id="Layer 1">
                    <g transform="matrix(1,0,0,1,597.344,637.02)">
                        <path d="M0,-279.559C-121.238,-279.559 -231.39,-264.983 -312.939,-241.23L-312.939,-38.329C-231.39,-14.575 -121.238,0 0,0C138.76,0 262.987,-19.092 346.431,-49.186L346.431,-230.37C262.987,-260.465 138.76,-279.559 0,-279.559" style="fill:rgb(165,43,0);fill-rule:nonzero;"/>
                    </g>
                    <g transform="matrix(1,0,0,1,1068.75,575.642)">
                        <path d="M0,-53.32L-14.211,-82.761C-14.138,-83.879 -14.08,-84.998 -14.08,-86.121C-14.08,-119.496 -48.786,-150.256 -107.177,-174.883L-107.177,2.643C-79.932,-8.849 -57.829,-21.674 -42.021,-35.482C-46.673,-16.775 -62.585,21.071 -75.271,47.686C-96.121,85.752 -103.671,118.889 -102.703,120.53C-102.086,121.563 -94.973,110.59 -84.484,92.809C-60.074,58.028 -13.82,-8.373 -4.575,-25.287C5.897,-44.461 0,-53.32 0,-53.32" style="fill:rgb(165,43,0);fill-rule:nonzero;"/>
                    </g>
                    <g transform="matrix(1,0,0,1,149.064,591.421)">
                        <path d="M0,-99.954C0,-93.526 1.293,-87.194 3.788,-80.985L-4.723,-65.835C-4.723,-65.835 -11.541,-56.989 0.465,-38.327C11.055,-21.872 64.1,42.54 92.097,76.271C104.123,93.564 112.276,104.216 112.99,103.187C114.114,101.554 105.514,69.087 81.631,32.046C70.487,12.151 57.177,-14.206 49.189,-33.675C71.492,-19.559 100.672,-6.755 135.341,4.265L135.341,-204.17C51.797,-177.622 0,-140.737 0,-99.954" style="fill:rgb(165,43,0);fill-rule:nonzero;"/>
                    </g>
                    <g transform="matrix(-65.8097,-752.207,-752.207,65.8097,621.707,796.312)">
                        <path d="M0.991,-0.034L0.933,0.008C0.933,0.014 0.933,0.02 0.933,0.026L0.99,0.069C0.996,0.073 0.999,0.08 0.998,0.087C0.997,0.094 0.992,0.1 0.986,0.103L0.92,0.133C0.919,0.139 0.918,0.145 0.916,0.15L0.964,0.203C0.968,0.208 0.97,0.216 0.968,0.222C0.965,0.229 0.96,0.234 0.953,0.236L0.882,0.254C0.88,0.259 0.877,0.264 0.875,0.27L0.91,0.33C0.914,0.336 0.914,0.344 0.91,0.35C0.907,0.356 0.9,0.36 0.893,0.361L0.82,0.365C0.817,0.369 0.813,0.374 0.81,0.379L0.832,0.445C0.835,0.452 0.833,0.459 0.828,0.465C0.824,0.47 0.816,0.473 0.809,0.472L0.737,0.462C0.733,0.466 0.729,0.47 0.724,0.474L0.733,0.544C0.734,0.551 0.731,0.558 0.725,0.562C0.719,0.566 0.711,0.568 0.704,0.565L0.636,0.542C0.631,0.546 0.626,0.549 0.621,0.552L0.615,0.621C0.615,0.629 0.61,0.635 0.604,0.638C0.597,0.641 0.589,0.641 0.583,0.638L0.521,0.602C0.52,0.603 0.519,0.603 0.518,0.603L0.406,0.729C0.406,0.729 0.394,0.747 0.359,0.725C0.329,0.705 0.206,0.599 0.141,0.543C0.109,0.52 0.089,0.504 0.09,0.502C0.093,0.499 0.149,0.509 0.217,0.554C0.278,0.588 0.371,0.631 0.38,0.619C0.38,0.619 0.396,0.604 0.406,0.575C0.406,0.575 0.406,0.575 0.406,0.575C0.407,0.576 0.407,0.576 0.406,0.575C0.406,0.575 0.091,0.024 0.305,-0.531C0.311,-0.593 0.275,-0.627 0.275,-0.627C0.266,-0.639 0.178,-0.598 0.12,-0.566C0.055,-0.523 0.002,-0.513 0,-0.516C-0.001,-0.518 0.018,-0.533 0.049,-0.555C0.11,-0.608 0.227,-0.707 0.256,-0.726C0.289,-0.748 0.301,-0.73 0.301,-0.73L0.402,-0.615C0.406,-0.614 0.41,-0.613 0.415,-0.613L0.47,-0.658C0.475,-0.663 0.483,-0.664 0.49,-0.662C0.497,-0.66 0.502,-0.655 0.504,-0.648L0.522,-0.58C0.527,-0.578 0.533,-0.576 0.538,-0.574L0.602,-0.608C0.608,-0.612 0.616,-0.612 0.623,-0.608C0.629,-0.605 0.633,-0.599 0.633,-0.592L0.637,-0.522C0.642,-0.519 0.647,-0.515 0.652,-0.512L0.721,-0.534C0.728,-0.536 0.736,-0.535 0.741,-0.531C0.747,-0.526 0.75,-0.519 0.749,-0.512L0.738,-0.443C0.742,-0.439 0.746,-0.435 0.751,-0.431L0.823,-0.439C0.83,-0.44 0.837,-0.437 0.842,-0.432C0.847,-0.426 0.848,-0.419 0.845,-0.412L0.821,-0.347C0.824,-0.342 0.828,-0.337 0.831,-0.332L0.903,-0.327C0.911,-0.327 0.917,-0.322 0.92,-0.316C0.924,-0.31 0.924,-0.302 0.92,-0.296L0.883,-0.236C0.885,-0.231 0.887,-0.226 0.889,-0.22L0.959,-0.202C0.966,-0.2 0.972,-0.195 0.974,-0.188C0.976,-0.181 0.974,-0.174 0.969,-0.168L0.92,-0.116C0.921,-0.111 0.923,-0.105 0.924,-0.099L0.988,-0.068C0.995,-0.065 0.999,-0.059 1,-0.052C1.001,-0.045 0.997,-0.038 0.991,-0.034ZM0.406,0.575C0.406,0.575 0.406,0.575 0.406,0.575C0.406,0.575 0.406,0.575 0.406,0.575Z" style="fill:url(#_Linear1);fill-rule:nonzero;"/>
                    </g>
                    <g transform="matrix(1,0,0,1,450.328,483.629)">
                        <path d="M0,167.33C-1.664,165.91 -2.536,165.068 -2.536,165.068L140.006,153.391C23.733,0 -69.418,122.193 -79.333,135.855L-79.333,167.33L0,167.33Z" style="fill-rule:nonzero;"/>
                    </g>
                    <g transform="matrix(1,0,0,1,747.12,477.333)">
                        <path d="M0,171.974C1.663,170.554 2.536,169.71 2.536,169.71L-134.448,159.687C-18.12,0 69.421,126.835 79.335,140.497L79.335,171.974L0,171.974Z" style="fill-rule:nonzero;"/>
                    </g>
                    <g transform="matrix(-1.53e-05,-267.211,-267.211,1.53e-05,809.465,764.23)">
                        <path d="M1,-0.586C1,-0.586 0.768,-0.528 0.524,-0.165L0.5,-0.064C0.5,-0.064 1.1,0.265 0.424,0.731C0.424,0.731 0.508,0.586 0.405,0.197C0.405,0.197 0.131,0.376 0.14,0.736C0.14,0.736 -0.275,0.391 0.324,-0.135C0.324,-0.135 0.539,-0.691 1,-0.736L1,-0.586Z" style="fill:url(#_Linear2);fill-rule:nonzero;"/>
                    </g>
                    <g transform="matrix(1,0,0,1,677.392,509.61)">
                        <path d="M0,-92.063C0,-92.063 43.486,-139.678 86.974,-92.063C86.974,-92.063 121.144,-28.571 86.974,3.171C86.974,3.171 31.062,47.615 0,3.171C0,3.171 -37.275,-31.75 0,-92.063" style="fill-rule:nonzero;"/>
                    </g>
                    <g transform="matrix(1,0,0,1,727.738,435.209)">
                        <path d="M0,0.002C0,18.543 -10.93,33.574 -24.408,33.574C-37.885,33.574 -48.814,18.543 -48.814,0.002C-48.814,-18.539 -37.885,-33.572 -24.408,-33.572C-10.93,-33.572 0,-18.539 0,0.002" style="fill:white;fill-rule:nonzero;"/>
                    </g>
                    <g transform="matrix(1,0,0,1,483.3,502.984)">
                        <path d="M0,-98.439C0,-98.439 74.596,-131.467 94.956,-57.748C94.956,-57.748 116.283,28.178 33.697,33.028C33.697,33.028 -71.613,12.745 0,-98.439" style="fill-rule:nonzero;"/>
                    </g>
                    <g transform="matrix(1,0,0,1,520.766,436.428)">
                        <path d="M0,0C0,19.119 -11.27,34.627 -25.173,34.627C-39.071,34.627 -50.344,19.119 -50.344,0C-50.344,-19.124 -39.071,-34.627 -25.173,-34.627C-11.27,-34.627 0,-19.124 0,0" style="fill:white;fill-rule:nonzero;"/>
                    </g>
                    <g transform="matrix(-1.53e-05,-239.021,-239.021,1.53e-05,402.161,775.388)">
                        <path d="M0.367,0.129C-0.364,-0.441 0.223,-0.711 0.223,-0.711C0.259,-0.391 0.472,-0.164 0.472,-0.164C0.521,-0.548 0.525,-0.77 0.525,-0.77C1.203,-0.256 0.589,0.161 0.589,0.161C0.627,0.265 0.772,0.372 0.906,0.451L1,0.77C0.376,0.403 0.367,0.129 0.367,0.129Z" style="fill:url(#_Linear3);fill-rule:nonzero;"/>
                    </g>
                </g>
                <defs>
                    <linearGradient id="_Linear1" x1="0" y1="0" x2="1" y2="0" gradientUnits="userSpaceOnUse" gradientTransform="matrix(1,0,1.38778e-17,-1,0,-0.000650515)"><stop offset="0" style="stop-color:rgb(247,76,0);stop-opacity:1"/><stop offset="0.33" style="stop-color:rgb(247,76,0);stop-opacity:1"/><stop offset="1" style="stop-color:rgb(244,150,0);stop-opacity:1"/></linearGradient>
                    <linearGradient id="_Linear2" x1="0" y1="0" x2="1" y2="0" gradientUnits="userSpaceOnUse" gradientTransform="matrix(1,0,0,-1,0,1.23438e-06)"><stop offset="0" style="stop-color:rgb(204,58,0);stop-opacity:1"/><stop offset="0.15" style="stop-color:rgb(204,58,0);stop-opacity:1"/><stop offset="0.74" style="stop-color:rgb(247,76,0);stop-opacity:1"/><stop offset="1" style="stop-color:rgb(247,76,0);stop-opacity:1"/></linearGradient>
                    <linearGradient id="_Linear3" x1="0" y1="0" x2="1" y2="0" gradientUnits="userSpaceOnUse" gradientTransform="matrix(1,1.32349e-23,1.32349e-23,-1,0,-9.1568e-07)"><stop offset="0" style="stop-color:rgb(204,58,0);stop-opacity:1"/><stop offset="0.15" style="stop-color:rgb(204,58,0);stop-opacity:1"/><stop offset="0.74" style="stop-color:rgb(247,76,0);stop-opacity:1"/><stop offset="1" style="stop-color:rgb(247,76,0);stop-opacity:1"/></linearGradient>
                </defs>
            </svg>"#;

        let mgr = FontMgr::default();
        let dom = Dom::from_bytes(data.as_bytes(), mgr).unwrap();
        let mut root = dom.root();

        println!("{:#?}", root.transform());
        println!("{:#?}", root.intrinsic_size());

        root.set_width(Length::new(50., LengthUnit::PX));
        root.set_height(Length::new(600., LengthUnit::CM));

        println!("{:#?}", root.intrinsic_size());
        println!("{:#?}", root.children_typed());
    }

    #[allow(unused)]
    fn save_surface_to_tmp(surface: &mut Surface) {
        let image = surface.image_snapshot();
        let data = image.encode(None, EncodedImageFormat::PNG, None).unwrap();
        write_file(data.as_bytes(), Path::new("/tmp/test.png"));

        pub fn write_file(bytes: &[u8], path: &Path) {
            let mut file = File::create(path).expect("failed to create file");
            file.write_all(bytes).expect("failed to write to file");
        }
    }

    #[test]
    fn decoding_base64() {
        use std::str::from_utf8;

        // padding length of 0-2 should be supported
        assert_eq!("Hello", from_utf8(&decode_base64("SGVsbG8=")).unwrap());
        assert_eq!("Hello!", from_utf8(&decode_base64("SGVsbG8h")).unwrap());
        assert_eq!(
            "Hello!!",
            from_utf8(&decode_base64("SGVsbG8hIQ==")).unwrap()
        );

        // padding length of 3 is invalid
        assert_eq!(0, decode_base64("SGVsbG8hIQ===").len());

        // if input length divided by 4 gives a remainder of 1 after padding removal, it's invalid
        assert_eq!(0, decode_base64("SGVsbG8hh").len());
        assert_eq!(0, decode_base64("SGVsbG8hh=").len());
        assert_eq!(0, decode_base64("SGVsbG8hh==").len());

        // invalid characters in the input
        assert_eq!(0, decode_base64("$GVsbG8h").len());
    }
}
