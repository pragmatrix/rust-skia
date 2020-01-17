use crate::prelude::*;
use crate::{scalar, Path, PathEffect};
use skia_bindings::SkPathEffect;

impl RCHandle<SkPathEffect> {
    pub fn path_1d(
        path: &Path,
        advance: scalar,
        phase: scalar,
        style: path_1d_path_effect::Style,
    ) -> Option<PathEffect> {
        path_1d_path_effect::new(path, advance, phase, style)
    }
}

pub mod path_1d_path_effect {
    use crate::prelude::*;
    use crate::{scalar, Path, PathEffect};
    use skia_bindings::C_SkPath1DPathEffect_Make;

    pub use skia_bindings::SkPath1DPathEffect_Style as Style;

    #[test]
    fn style_enum_naming() {
        let _n = Style::Translate;
    }

    pub fn new(path: &Path, advance: scalar, phase: scalar, style: Style) -> Option<PathEffect> {
        PathEffect::from_ptr(unsafe {
            C_SkPath1DPathEffect_Make(path.native(), advance, phase, style)
        })
    }
}
