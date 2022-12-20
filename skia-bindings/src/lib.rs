// 32-bit windows needs `thiscall` support.
// https://github.com/rust-skia/rust-skia/issues/540
#![cfg_attr(all(target_os = "windows", target_arch = "x86"), feature(abi_thiscall))]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

mod bindings;
pub use bindings::*;

mod defaults;
pub use defaults::*;

mod impls;
pub use impls::*;

#[cfg(feature = "textlayout")]
pub mod icu;

#[allow(unused_imports)]
#[doc(hidden)]
#[cfg(feature = "use-system-jpeg-turbo")]
use mozjpeg_sys;

use cpp::cpp;

cpp! {{
    #include "include/codec/SkCodec.h"
    #include "bindings.h"
}}

pub fn make_from_data(data: *mut SkData) -> *mut SkCodec {
    unsafe {
        cpp!([data as "SkData*"] -> *mut SkCodec as "SkCodec*" {
            return SkCodec::MakeFromData(sp(data)).release();
        })
    }
}
