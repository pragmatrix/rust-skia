use crate::prelude::*;
use crate::{gpu, Canvas, IRect, ImageInfo, Matrix, NativeFlattenable, Point, Rect};
use skia_bindings as sb;
use skia_bindings::{SkDrawable, SkDrawable_GpuDrawHandler, SkFlattenable, SkRefCntBase};

pub type Drawable = RCHandle<SkDrawable>;

impl NativeRefCountedBase for SkDrawable {
    type Base = SkRefCntBase;
}

impl NativeFlattenable for SkDrawable {
    fn native_flattenable(&self) -> &SkFlattenable {
        unsafe { &*(self as *const SkDrawable as *const SkFlattenable) }
    }

    fn native_deserialize(data: &[u8]) -> *mut Self {
        unsafe { sb::C_SkDrawable_Deserialize(data.as_ptr() as _, data.len()) }
    }
}

impl RCHandle<SkDrawable> {
    pub fn draw(&mut self, canvas: &mut Canvas, matrix: Option<&Matrix>) {
        unsafe {
            self.native_mut()
                .draw(canvas.native_mut(), matrix.native_ptr_or_null())
        }
    }

    pub fn draw_at(&mut self, canvas: &mut Canvas, point: impl Into<Point>) {
        let point = point.into();
        unsafe {
            self.native_mut()
                .draw1(canvas.native_mut(), point.x, point.y)
        }
    }

    pub fn snap_gpu_draw_handler(
        &mut self,
        api: gpu::BackendAPI,
        matrix: &Matrix,
        clip_bounds: impl Into<IRect>,
        buffer_info: &ImageInfo,
    ) -> Option<GPUDrawHandler> {
        GPUDrawHandler::from_ptr(unsafe {
            sb::C_SkDrawable_snapGpuDrawHandler(
                self.native_mut(),
                api,
                matrix.native(),
                clip_bounds.into().native(),
                buffer_info.native(),
            )
        })
    }

    // TODO: clarify ref-counter situation here, return value is SkPicture*
    /*
    pub fn new_picture_snapshot(&mut self) -> Option<Picture> {
        unimplemented!()
    }
    */

    pub fn generation_id(&mut self) -> u32 {
        unsafe { self.native_mut().getGenerationID() }
    }

    pub fn bounds(&mut self) -> Rect {
        Rect::from_native(unsafe { self.native_mut().getBounds() })
    }

    pub fn notify_drawing_changed(&mut self) {
        unsafe { self.native_mut().notifyDrawingChanged() }
    }
}

pub type GPUDrawHandler = RefHandle<SkDrawable_GpuDrawHandler>;

impl NativeDrop for SkDrawable_GpuDrawHandler {
    fn drop(&mut self) {
        unsafe { sb::C_SkDrawable_GpuDrawHandler_delete(self) }
    }
}

impl RefHandle<SkDrawable_GpuDrawHandler> {
    pub fn draw(&mut self, info: &gpu::BackendDrawableInfo) {
        unsafe {
            sb::C_SkDrawable_GpuDrawHandler_draw(self.native_mut(), info.native());
        }
    }
}
