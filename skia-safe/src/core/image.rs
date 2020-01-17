use crate::prelude::*;
use crate::{gpu, FilterQuality, ImageFilter, ImageGenerator, Pixmap};
use crate::{
    AlphaType, Bitmap, ColorSpace, ColorType, Data, EncodedImageFormat, IPoint, IRect, ISize,
    ImageInfo, Matrix, Paint, Picture, Shader, TileMode, YUVAIndex, YUVColorSpace,
};
use skia_bindings as sb;
use skia_bindings::{SkImage, SkRefCntBase};
use std::mem;

pub use skia_bindings::SkImage_BitDepth as BitDepth;
#[test]
fn test_bit_depth_naming() {
    let _ = BitDepth::F16;
}

pub use skia_bindings::SkImage_CachingHint as CachingHint;
#[test]
fn test_caching_hint_naming() {
    let _ = CachingHint::Allow;
}

pub use skia_bindings::SkImage_CompressionType as CompressionType;
#[test]
fn test_compression_type_naming() {
    let _ = CompressionType::ETC1;
}

pub type Image = RCHandle<SkImage>;

impl NativeBase<SkRefCntBase> for SkImage {}

impl NativeRefCountedBase for SkImage {
    type Base = SkRefCntBase;
}

impl RCHandle<SkImage> {
    // TODO: MakeRasterCopy()

    pub fn from_raster_data(info: &ImageInfo, pixels: Data, row_bytes: usize) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_MakeRasterData(info.native(), pixels.into_ptr(), row_bytes)
        })
    }

    // TODO: MakeFromRaster()

    pub fn from_bitmap(bitmap: &Bitmap) -> Option<Image> {
        Image::from_ptr(unsafe { sb::C_SkImage_MakeFromBitmap(bitmap.native()) })
    }

    pub fn from_generator(
        mut image_generator: ImageGenerator,
        subset: Option<&IRect>,
    ) -> Option<Image> {
        let image = Image::from_ptr(unsafe {
            sb::C_SkImage_MakeFromGenerator(
                image_generator.native_mut(),
                subset.native_ptr_or_null(),
            )
        });
        mem::forget(image_generator);
        image
    }

    pub fn from_encoded(data: Data, subset: Option<IRect>) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_MakeFromEncoded(data.into_ptr(), subset.native().as_ptr_or_null())
        })
    }

    pub fn decode_to_raster(encoded: &[u8], subset: impl Into<Option<IRect>>) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_DecodeToRaster(
                encoded.as_ptr() as _,
                encoded.len(),
                subset.into().into_native().as_ptr_or_null(),
            )
        })
    }

    pub fn decode_to_texture(
        context: &mut gpu::Context,
        encoded: &[u8],
        subset: impl Into<Option<IRect>>,
    ) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_DecodeToTexture(
                context.native_mut(),
                encoded.as_ptr() as _,
                encoded.len(),
                subset.into().into_native().as_ptr_or_null(),
            )
        })
    }

    // TODO: this is experimental, should probably be removed.
    pub fn from_compressed(
        context: &mut gpu::Context,
        data: Data,
        size: impl Into<ISize>,
        c_type: CompressionType,
    ) -> Option<Image> {
        let size = size.into();
        Image::from_ptr(unsafe {
            sb::C_SkImage_MakeFromCompressed(
                context.native_mut(),
                data.into_ptr(),
                size.width,
                size.height,
                c_type,
            )
        })
    }

    // TODO: add variant with TextureReleaseProc

    pub fn from_texture(
        context: &mut gpu::Context,
        backend_texture: &gpu::BackendTexture,
        origin: gpu::SurfaceOrigin,
        color_type: ColorType,
        alpha_type: AlphaType,
        color_space: impl Into<Option<ColorSpace>>,
    ) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_MakeFromTexture(
                context.native_mut(),
                backend_texture.native(),
                origin,
                color_type.into_native(),
                alpha_type,
                color_space.into().into_ptr_or_null(),
            )
        })
    }

    pub fn from_pixmap_cross_context(
        context: &mut gpu::Context,
        pixmap: &Pixmap,
        build_mips: bool,
        limit_to_max_texture_size: impl Into<Option<bool>>,
    ) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_MakeCrossContextFromPixmap(
                context.native_mut(),
                pixmap.native(),
                build_mips,
                limit_to_max_texture_size.into().unwrap_or(false),
            )
        })
    }

    pub fn from_adopted_texture(
        context: &mut gpu::Context,
        backend_texture: &gpu::BackendTexture,
        origin: gpu::SurfaceOrigin,
        color_type: ColorType,
        alpha_type: AlphaType,
        color_space: impl Into<Option<ColorSpace>>,
    ) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_MakeFromAdoptedTexture(
                context.native_mut(),
                backend_texture.native(),
                origin,
                color_type.into_native(),
                alpha_type,
                color_space.into().into_ptr_or_null(),
            )
        })
    }

    // TODO: rename to clone_from_yuva_textures() ?
    pub fn from_yuva_textures_copy(
        context: &mut gpu::Context,
        yuv_color_space: YUVColorSpace,
        yuva_textures: &[gpu::BackendTexture],
        yuva_indices: &[YUVAIndex; 4],
        image_size: impl Into<ISize>,
        image_origin: gpu::SurfaceOrigin,
        image_color_space: impl Into<Option<ColorSpace>>,
    ) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_MakeFromYUVATexturesCopy(
                context.native_mut(),
                yuv_color_space,
                yuva_textures.native().as_ptr(),
                yuva_indices.native().as_ptr(),
                image_size.into().into_native(),
                image_origin,
                image_color_space.into().into_ptr_or_null(),
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_yuva_textures_copy_with_external_backend(
        context: &mut gpu::Context,
        yuv_color_space: YUVColorSpace,
        yuva_textures: &[gpu::BackendTexture],
        yuva_indices: &[YUVAIndex; 4],
        image_size: impl Into<ISize>,
        image_origin: gpu::SurfaceOrigin,
        backend_texture: &gpu::BackendTexture,
        image_color_space: impl Into<Option<ColorSpace>>,
        // TODO: m78 introduced textureReleaseProc and releaseContext here.
    ) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_MakeFromYUVATexturesCopyWithExternalBackend(
                context.native_mut(),
                yuv_color_space,
                yuva_textures.native().as_ptr(),
                yuva_indices.native().as_ptr(),
                image_size.into().into_native(),
                image_origin,
                backend_texture.native(),
                image_color_space.into().into_ptr_or_null(),
            )
        })
    }

    pub fn from_yuva_textures(
        context: &mut gpu::Context,
        yuv_color_space: YUVColorSpace,
        yuva_textures: &[gpu::BackendTexture],
        yuva_indices: &[YUVAIndex; 4],
        image_size: impl Into<ISize>,
        image_origin: gpu::SurfaceOrigin,
        image_color_space: impl Into<Option<ColorSpace>>,
    ) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_MakeFromYUVATextures(
                context.native_mut(),
                yuv_color_space,
                yuva_textures.native().as_ptr(),
                yuva_indices.native().as_ptr(),
                image_size.into().into_native(),
                image_origin,
                image_color_space.into().into_ptr_or_null(),
            )
        })
    }

    // TODO: MakeFromYUVAPixmaps()

    pub fn from_nv12_textures_copy(
        context: &mut gpu::Context,
        yuv_color_space: YUVColorSpace,
        nv12_textures: &[gpu::BackendTexture; 2],
        image_origin: gpu::SurfaceOrigin,
        image_color_space: impl Into<Option<ColorSpace>>,
    ) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_MakeFromNV12TexturesCopy(
                context.native_mut(),
                yuv_color_space,
                nv12_textures.native().as_ptr(),
                image_origin,
                image_color_space.into().into_ptr_or_null(),
            )
        })
    }

    pub fn from_nv12_textures_copy_with_external_backend(
        context: &mut gpu::Context,
        yuv_color_space: YUVColorSpace,
        nv12_textures: &[gpu::BackendTexture; 2],
        image_origin: gpu::SurfaceOrigin,
        backend_texture: &gpu::BackendTexture,
        image_color_space: impl Into<Option<ColorSpace>>,
        // TODO: m78 introduced textureReleaseProc and releaseContext here.
    ) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_MakeFromNV12TexturesCopyWithExternalBackend(
                context.native_mut(),
                yuv_color_space,
                nv12_textures.native().as_ptr(),
                image_origin,
                backend_texture.native(),
                image_color_space.into().into_ptr_or_null(),
            )
        })
    }

    pub fn from_picture(
        picture: Picture,
        dimensions: impl Into<ISize>,
        matrix: Option<&Matrix>,
        paint: Option<&Paint>,
        bit_depth: BitDepth,
        color_space: impl Into<Option<ColorSpace>>,
    ) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_MakeFromPicture(
                picture.into_ptr(),
                dimensions.into().native(),
                matrix.native_ptr_or_null(),
                paint.native_ptr_or_null(),
                bit_depth,
                color_space.into().into_ptr_or_null(),
            )
        })
    }

    pub fn image_info(&self) -> &ImageInfo {
        ImageInfo::from_native_ref(&self.native().fInfo)
    }

    pub fn width(&self) -> i32 {
        self.image_info().width()
    }

    pub fn height(&self) -> i32 {
        self.image_info().height()
    }

    pub fn dimensions(&self) -> ISize {
        self.image_info().dimensions()
    }

    pub fn bounds(&self) -> IRect {
        self.image_info().bounds()
    }

    pub fn unique_id(&self) -> u32 {
        self.native().fUniqueID
    }

    pub fn alpha_type(&self) -> AlphaType {
        unsafe { self.native().alphaType() }
    }

    pub fn color_type(&self) -> ColorType {
        ColorType::from_native(unsafe { self.native().colorType() })
    }

    pub fn color_space(&self) -> ColorSpace {
        ColorSpace::from_unshared_ptr(unsafe { self.native().colorSpace() }).unwrap()
    }

    pub fn is_alpha_only(&self) -> bool {
        unsafe { self.native().isAlphaOnly() }
    }

    pub fn is_opaque(&self) -> bool {
        self.alpha_type().is_opaque()
    }

    pub fn to_shader<'a>(
        &self,
        tile_modes: impl Into<Option<(TileMode, TileMode)>>,
        local_matrix: impl Into<Option<&'a Matrix>>,
    ) -> Shader {
        let tile_modes = tile_modes.into();
        let tm1 = tile_modes.map(|m| m.0).unwrap_or_default();
        let tm2 = tile_modes.map(|m| m.1).unwrap_or_default();

        Shader::from_ptr(unsafe {
            sb::C_SkImage_makeShader(
                self.native(),
                tm1,
                tm2,
                local_matrix.into().native_ptr_or_null(),
            )
        })
        .unwrap()
    }

    pub fn peek_pixels(&self) -> Option<Borrows<Pixmap>> {
        let mut pixmap = Pixmap::default();
        unsafe { self.native().peekPixels(pixmap.native_mut()) }
            .if_false_then_some(|| pixmap.borrows(self))
    }

    pub fn is_texture_backed(&self) -> bool {
        unsafe { self.native().isTextureBacked() }
    }

    pub fn is_valid(&self, context: &mut gpu::Context) -> bool {
        unsafe { self.native().isValid(context.native_mut()) }
    }

    // TODO: flush(GrContext*, GrFlushInfo&).

    pub fn flush(&mut self, context: &mut gpu::Context) {
        unsafe { self.native_mut().flush1(context.native_mut()) }
    }

    pub fn backend_texture(
        &self,
        flush_pending_gr_context_io: bool,
    ) -> (gpu::BackendTexture, gpu::SurfaceOrigin) {
        let mut origin = gpu::SurfaceOrigin::TopLeft;
        let texture = gpu::BackendTexture::from_native(unsafe {
            self.native()
                .getBackendTexture(flush_pending_gr_context_io, &mut origin)
        });
        (texture, origin)
    }

    pub fn read_pixels<P>(
        &self,
        dst_info: &ImageInfo,
        pixels: &mut [P],
        dst_row_bytes: usize,
        src: impl Into<IPoint>,
        caching_hint: CachingHint,
    ) -> bool {
        if pixels.elements_size_of()
            != (usize::try_from(dst_info.height()).unwrap() * dst_row_bytes)
        {
            return false;
        }

        let src = src.into();

        unsafe {
            self.native().readPixels(
                dst_info.native(),
                pixels.as_mut_ptr() as _,
                dst_row_bytes,
                src.x,
                src.y,
                caching_hint,
            )
        }
    }

    #[must_use]
    pub fn scale_pixels(
        &self,
        dst: &Pixmap,
        filter_quality: FilterQuality,
        caching_hint: impl Into<Option<CachingHint>>,
    ) -> bool {
        unsafe {
            self.native().scalePixels(
                dst.native(),
                filter_quality,
                caching_hint.into().unwrap_or(CachingHint::Allow),
            )
        }
    }

    // TODO: support quality!
    pub fn encode_to_data(&self, image_format: EncodedImageFormat) -> Option<Data> {
        Data::from_ptr(unsafe { sb::C_SkImage_encodeToData(self.native(), image_format) })
    }

    pub fn encoded_data(&self) -> Option<Data> {
        Data::from_ptr(unsafe { sb::C_SkImage_refEncodedData(self.native()) })
    }

    pub fn new_subset(&self, rect: impl AsRef<IRect>) -> Option<Image> {
        Image::from_ptr(unsafe { sb::C_SkImage_makeSubset(self.native(), rect.as_ref().native()) })
    }

    pub fn new_texture_image(
        &self,
        context: &mut gpu::Context,
        mip_mapped: gpu::MipMapped,
    ) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_makeTextureImage(self.native(), context.native_mut(), mip_mapped)
        })
    }

    pub fn new_non_texture_image(&self) -> Option<Image> {
        Image::from_ptr(unsafe { sb::C_SkImage_makeNonTextureImage(self.native()) })
    }

    pub fn new_raster_image(&self) -> Option<Image> {
        Image::from_ptr(unsafe { sb::C_SkImage_makeRasterImage(self.native()) })
    }

    // TODO: rename to with_filter()?
    pub fn new_with_filter(
        &self,
        mut context: Option<&mut gpu::Context>,
        filter: &ImageFilter,
        clip_bounds: impl Into<IRect>,
        subset: impl Into<IRect>,
    ) -> Option<(Image, IRect, IPoint)> {
        let mut out_subset = IRect::default();
        let mut offset = IPoint::default();

        Image::from_ptr(unsafe {
            sb::C_SkImage_makeWithFilter(
                self.native(),
                context.native_ptr_or_null_mut(),
                filter.native(),
                subset.into().native(),
                clip_bounds.into().native(),
                out_subset.native_mut(),
                offset.native_mut(),
            )
        })
        .map(|image| (image, out_subset, offset))
    }

    // TODO: MakeBackendTextureFromSkImage()

    pub fn is_lazy_generated(&self) -> bool {
        unsafe { self.native().isLazyGenerated() }
    }

    pub fn new_color_space(&self, color_space: impl Into<Option<ColorSpace>>) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_makeColorSpace(self.native(), color_space.into().into_ptr_or_null())
        })
    }

    pub fn reinterpret_color_space(&self, new_color_space: ColorSpace) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkImage_reinterpretColorSpace(self.native(), new_color_space.into_ptr())
        })
    }
}
