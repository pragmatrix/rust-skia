//! Decoders for the image formats that Skia supports natively.
//!
//! Each module provides a [`Decoder`] for the format that can be passed to
//! [`crate::Codec::from_stream`] or [`crate::Codec::from_data_with_decoders`], and a convenience
//! [`decode_stream()`] function that decodes a stream directly.

pub mod bmp_decoder {
    use std::{io, result};

    use crate::{Codec, codec::Result, codec::codecs::Decoder};

    /// Attempts to decode the given bytes as a BMP.
    ///
    /// If the bytes are not a BMP, returns an error [`Result`] explaining the reason for the
    /// failure.
    pub fn decode_stream(stream: &mut impl io::Read) -> result::Result<Codec, Result> {
        decoder().from_stream(stream)
    }

    /// Returns the decoder for BMP images, identifying data with the id `"bmp"`.
    pub fn decoder() -> Decoder {
        Decoder::construct(|decoder| unsafe { skia_bindings::C_SkBmpDecoder_Decoder(decoder) })
    }
}

pub mod gif_decoder {
    use std::{io, result};

    use crate::{Codec, codec::Result, codec::codecs::Decoder};

    /// Attempts to decode the given bytes as a GIF.
    ///
    /// If the bytes are not a GIF, returns an error [`Result`] explaining the reason for the
    /// failure.
    pub fn decode_stream(stream: &mut impl io::Read) -> result::Result<Codec, Result> {
        decoder().from_stream(stream)
    }

    /// Returns the decoder for GIF images, identifying data with the id `"gif"`.
    pub fn decoder() -> Decoder {
        Decoder::construct(|decoder| unsafe { skia_bindings::C_SkGifDecoder_Decoder(decoder) })
    }
}

pub mod ico_decoder {
    use std::{io, result};

    use crate::{Codec, codec::Result, codec::codecs::Decoder};

    /// Attempts to decode the given bytes as a ICO.
    ///
    /// If the bytes are not a ICO, returns an error [`Result`] explaining the reason for the
    /// failure.
    pub fn decode_stream(stream: &mut impl io::Read) -> result::Result<Codec, Result> {
        decoder().from_stream(stream)
    }

    /// Returns the decoder for ICO images, identifying data with the id `"ico"`.
    pub fn decoder() -> Decoder {
        Decoder::construct(|decoder| unsafe { skia_bindings::C_SkIcoDecoder_Decoder(decoder) })
    }
}

#[cfg(feature = "jpeg")]
pub mod jpeg_decoder {
    use std::{io, result};

    use crate::{Codec, codec::Result, codec::codecs::Decoder};

    /// Attempts to decode the given bytes as a JPEG.
    ///
    /// If the bytes are not a JPEG, returns an error [`Result`] explaining the reason for the
    /// failure.
    pub fn decode_stream(stream: &mut impl io::Read) -> result::Result<Codec, Result> {
        decoder().from_stream(stream)
    }

    /// Returns the decoder for JPEG images, identifying data with the id `"jpeg"`.
    pub fn decoder() -> Decoder {
        Decoder::construct(|decoder| unsafe { skia_bindings::C_SkJpegDecoder_Decoder(decoder) })
    }
}

pub mod png_decoder {
    use std::{io, result};

    use crate::{Codec, codec::Result, codec::codecs::Decoder};

    /// Attempts to decode the given bytes as a PNG.
    ///
    /// If the bytes are not a PNG, returns an error [`Result`] explaining the reason for the
    /// failure.
    pub fn decode_stream(stream: &mut impl io::Read) -> result::Result<Codec, Result> {
        decoder().from_stream(stream)
    }

    /// Returns the decoder for PNG images, identifying data with the id `"png"`.
    pub fn decoder() -> Decoder {
        Decoder::construct(|decoder| unsafe { skia_bindings::C_SkPngDecoder_Decoder(decoder) })
    }
}

#[cfg(any())]
pub mod png_rust_decoder {
    use std::{io, result};

    use crate::{Codec, codec::Result, codec::codecs::Decoder};

    /// Attempts to decode the given bytes as a PNG.
    ///
    /// If the bytes are not a PNG, returns an error [`Result`] explaining the reason for the
    /// failure.
    pub fn decode_stream(stream: &mut impl io::Read) -> result::Result<Codec, Result> {
        decoder().from_stream(stream)
    }

    /// Returns the decoder for PNG images, identifying data with the id `"png"`.
    pub fn decoder() -> Decoder {
        Decoder::construct(|decoder| unsafe { skia_bindings::C_SkPngRustDecoder_Decoder(decoder) })
    }
}

pub mod wbmp_decoder {
    use std::{io, result};

    use crate::{Codec, codec::Result, codec::codecs::Decoder};

    /// Attempts to decode the given bytes as a WBMP.
    ///
    /// If the bytes are not a WBMP, returns an error [`Result`] explaining the reason for the
    /// failure.
    pub fn decode_stream(stream: &mut impl io::Read) -> result::Result<Codec, Result> {
        decoder().from_stream(stream)
    }

    /// Returns the decoder for WBMP images, identifying data with the id `"wbmp"`.
    pub fn decoder() -> Decoder {
        Decoder::construct(|decoder| unsafe { skia_bindings::C_SkWbmpDecoder_Decoder(decoder) })
    }
}

#[cfg(feature = "webp-decode")]
pub mod webp_decoder {
    use std::{io, result};

    use crate::{Codec, codec::Result, codec::codecs::Decoder};

    /// Attempts to decode the given bytes as a WEBP.
    ///
    /// If the bytes are not a WEBP, returns an error [`Result`] explaining the reason for the
    /// failure.
    pub fn decode_stream(stream: &mut impl io::Read) -> result::Result<Codec, Result> {
        decoder().from_stream(stream)
    }

    /// Returns the decoder for WEBP images, identifying data with the id `"webp"`.
    pub fn decoder() -> Decoder {
        Decoder::construct(|decoder| unsafe { skia_bindings::C_SkWebpDecoder_Decoder(decoder) })
    }
}
