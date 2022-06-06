use crate::interop::AsStr;
use std::{fmt, ops::Index};

mod dart_types;
mod font_arguments;
mod font_collection;
mod metrics;
#[allow(clippy::module_inception)]
mod paragraph;
mod paragraph_builder;
mod paragraph_cache;
mod paragraph_style;
mod text_shadow;
mod text_style;
mod typeface_font_provider;

pub use dart_types::*;
pub use font_arguments::*;
pub use font_collection::*;
pub use metrics::*;
pub use paragraph::*;
pub use paragraph_builder::*;
pub use paragraph_cache::*;
pub use paragraph_style::*;
pub use text_shadow::*;
pub use text_style::*;
pub use typeface_font_provider::*;

/// Efficient reference type to a C++ vector of font family SkStrings.
///
/// Use indexer or .iter() to access the Rust str references.
pub struct FontFamilies<'a>(&'a [skia_bindings::SkString]);

impl fmt::Debug for FontFamilies<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let families: Vec<_> = self.iter().collect();
        f.debug_tuple("FontFamilies").field(&families).finish()
    }
}

impl Index<usize> for FontFamilies<'_> {
    type Output = str;
    fn index(&self, index: usize) -> &Self::Output {
        self.0[index].as_str()
    }
}

impl FontFamilies<'_> {
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|str| str.as_str())
    }
}
