use crate::{
    prelude::*,
    scalar,
    svg::{DebugAttributes, HasBase},
};
use skia_bindings as sb;

pub type DistantLight = RCHandle<sb::SkSVGFeDistantLight>;

impl NativeRefCountedBase for sb::SkSVGFeDistantLight {
    type Base = sb::SkRefCntBase;
}

impl HasBase for sb::SkSVGFeDistantLight {
    type Base = sb::SkSVGContainer;
}

impl DebugAttributes for DistantLight {
    const NAME: &'static str = "FeDistantLight";

    fn _dbg(&self, builder: &mut std::fmt::DebugStruct) {
        self.as_base()._dbg(
            builder
                .field("azimuth", &self.azimuth())
                .field("elevation", &self.elevation()),
        );
    }
}

impl DistantLight {
    skia_svg_macros::attrs! {
        SkSVGFeDistantLight => {
            *azimuth: scalar [get(value) => value, set(value) => value],
            *elevation: scalar [get(value) => value, set(value) => value]
        }
    }
}

pub type PointLight = RCHandle<sb::SkSVGFePointLight>;

impl NativeRefCountedBase for sb::SkSVGFePointLight {
    type Base = sb::SkRefCntBase;
}

impl HasBase for sb::SkSVGFePointLight {
    type Base = sb::SkSVGContainer;
}

impl DebugAttributes for PointLight {
    const NAME: &'static str = "FePointLight";

    fn _dbg(&self, builder: &mut std::fmt::DebugStruct) {
        self.as_base()._dbg(
            builder
                .field("x", &self.x())
                .field("y", &self.y())
                .field("z", &self.z()),
        );
    }
}

impl PointLight {
    skia_svg_macros::attrs! {
        SkSVGFePointLight => {
            *x: scalar [get(value) => value, set(value) => value],
            *y: scalar [get(value) => value, set(value) => value],
            *z: scalar [get(value) => value, set(value) => value]
        }
    }
}

pub type SpotLight = RCHandle<sb::SkSVGFeSpotLight>;

impl NativeRefCountedBase for sb::SkSVGFeSpotLight {
    type Base = sb::SkRefCntBase;
}

impl HasBase for sb::SkSVGFeSpotLight {
    type Base = sb::SkSVGContainer;
}

impl DebugAttributes for SpotLight {
    const NAME: &'static str = "FeSpotLight";

    fn _dbg(&self, builder: &mut std::fmt::DebugStruct) {
        self.as_base()._dbg(
            builder
                .field("x", &self.x())
                .field("y", &self.y())
                .field("z", &self.z())
                .field("points_at_x", &self.points_at_x())
                .field("points_at_y", &self.points_at_y())
                .field("points_at_z", &self.points_at_z())
                .field("specular_exponent", &self.specular_exponent())
                .field("limiting_cone_angle", &self.limiting_cone_angle()),
        );
    }
}

impl SpotLight {
    skia_svg_macros::attrs! {
        SkSVGFeSpotLight => {
            *x: scalar [get(value) => value, set(value) => value],
            *y: scalar [get(value) => value, set(value) => value],
            *z: scalar [get(value) => value, set(value) => value],
            *points_at_x: scalar [get(value) => value, set(value) => value],
            *points_at_y: scalar [get(value) => value, set(value) => value],
            *points_at_z: scalar [get(value) => value, set(value) => value],
            *specular_exponent: scalar [get(value) => value, set(value) => value],
            *limiting_cone_angle?: scalar [get(value) => value, set(value) => value]
        }
    }
}
