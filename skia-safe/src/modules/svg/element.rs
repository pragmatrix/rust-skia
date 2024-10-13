use super::{DebugAttributes, HasBase, Length, PreserveAspectRatio};
use crate::{prelude::*, Rect, Size};
use skia_bindings as sb;

pub type Svg = RCHandle<sb::SkSVGSVG>;

impl NativeRefCountedBase for sb::SkSVGSVG {
    type Base = sb::SkRefCntBase;
}

impl HasBase for sb::SkSVGSVG {
    type Base = sb::SkSVGContainer;
}

impl DebugAttributes for Svg {
    const NAME: &'static str = "Svg";

    fn _dbg(&self, builder: &mut std::fmt::DebugStruct) {
        self.as_base()._dbg(
            builder
                .field("x", &self.x())
                .field("y", &self.y())
                .field("width", &self.width())
                .field("height", &self.height())
                .field("preserve_aspect_ratio", self.preserve_aspect_ratio())
                .field("view_box", &self.view_box()),
        );
    }
}

impl Svg {
    pub fn intrinsic_size(&self) -> Size {
        unsafe { Size::from_native_c(sb::C_SkSVGSVG_intrinsicSize(self.native())) }
    }

    skia_svg_macros::attrs! {
        SkSVGSVG => {
            x: Length [get(value) => Length::from_native_ref(value), set(value) => value.into_native()],
            y: Length [get(value) => Length::from_native_ref(value), set(value) => value.into_native()],
            width: Length [get(value) => Length::from_native_ref(value), set(value) => value.into_native()],
            height: Length [get(value) => Length::from_native_ref(value), set(value) => value.into_native()],
            preserve_aspect_ratio: PreserveAspectRatio [get(value) => PreserveAspectRatio::from_native_ref(value), set(value) => value.into_native()],
            view_box?: Rect [get(value) => value.map(Rect::from_native_ref), set(value) => value.into_native()]
        }
    }
}
