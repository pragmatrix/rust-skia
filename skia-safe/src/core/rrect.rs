//! Describes a rounded rectangle with a bounds and a pair of radii for each corner. The bounds
//! and radii can be set so that [`RRect`] describes: a rectangle with sharp corners; a circle; an
//! oval; or a rectangle with one or more rounded corners.
//!
//! [`RRect`] allows implementing CSS properties that describe rounded corners. [`RRect`] may have
//! up to eight different radii, one for each axis on each of its four corners.
//!
//! [`RRect`] may modify the provided parameters when initializing bounds and radii. If either axis
//! radii is zero or less: radii are stored as zero; corner is square. If corner curves overlap,
//! radii are proportionally reduced to fit within bounds.

use crate::{Matrix, Point, Rect, Vector, interop, prelude::*, scalar};
use skia_bindings::{self as sb, SkRRect};
use std::{fmt, mem, ptr};

/// Describes possible specializations of [`RRect`]. Each type is exclusive; an [`RRect`] may only
/// have one type.
///
/// Type members become progressively less restrictive; larger values of type have more degrees of
/// freedom than smaller values.
pub use skia_bindings::SkRRect_Type as Type;
variant_name!(Type::Complex);

/// The radii are stored: top-left, top-right, bottom-right, bottom-left.
pub use skia_bindings::SkRRect_Corner as Corner;
variant_name!(Corner::LowerLeft);

#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct RRect(SkRRect);

native_transmutable!(SkRRect, RRect);

impl PartialEq for RRect {
    fn eq(&self, rhs: &Self) -> bool {
        unsafe { sb::C_SkRRect_Equals(self.native(), rhs.native()) }
    }
}

impl Default for RRect {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for RRect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RRect")
            .field("rect", &self.rect())
            .field(
                "radii",
                &[
                    self.radii(Corner::UpperLeft),
                    self.radii(Corner::UpperRight),
                    self.radii(Corner::LowerRight),
                    self.radii(Corner::LowerLeft),
                ],
            )
            .field("type", &self.get_type())
            .finish()
    }
}

impl AsRef<RRect> for RRect {
    fn as_ref(&self) -> &RRect {
        self
    }
}

impl RRect {
    /// Initializes bounds at (0, 0), the origin, with zero width and height. Initializes corner
    /// radii to (0, 0), and sets the type to [`Type::Empty`].
    pub fn new() -> Self {
        RRect::construct(|rr| unsafe { sb::C_SkRRect_Construct(rr) })
    }

    pub fn get_type(&self) -> Type {
        unsafe { sb::C_SkRRect_getType(self.native()) }
    }

    pub fn is_empty(&self) -> bool {
        self.get_type() == Type::Empty
    }

    pub fn is_rect(&self) -> bool {
        self.get_type() == Type::Rect
    }

    pub fn is_oval(&self) -> bool {
        self.get_type() == Type::Oval
    }

    pub fn is_simple(&self) -> bool {
        self.get_type() == Type::Simple
    }

    pub fn is_nine_patch(&self) -> bool {
        self.get_type() == Type::NinePatch
    }

    pub fn is_complex(&self) -> bool {
        self.get_type() == Type::Complex
    }

    /// Returns the span on the x-axis. This does not check if the result fits in a 32-bit float;
    /// the result may be infinity.
    pub fn width(&self) -> scalar {
        self.rect().width()
    }

    /// Returns the span on the y-axis. This does not check if the result fits in a 32-bit float;
    /// the result may be infinity.
    pub fn height(&self) -> scalar {
        self.rect().height()
    }

    /// Returns the top-left corner radii. If the type is [`Type::Empty`], [`Type::Rect`],
    /// [`Type::Oval`], or [`Type::Simple`], returns a value representative of all corner radii. If
    /// the type is [`Type::NinePatch`] or [`Type::Complex`], at least one of the remaining three
    /// corners has a different value.
    pub fn simple_radii(&self) -> Vector {
        self.radii(Corner::UpperLeft)
    }

    /// Sets bounds to zero width and height at (0, 0), the origin. Sets corner radii to zero and
    /// sets the type to [`Type::Empty`].
    pub fn set_empty(&mut self) {
        *self = Self::new()
    }

    /// Sets bounds to sorted `rect`, and sets corner radii to zero. If the set bounds has width and
    /// height, sets the type to [`Type::Rect`]; otherwise, sets the type to [`Type::Empty`].
    ///
    /// - `rect` bounds to set
    pub fn set_rect(&mut self, rect: impl AsRef<Rect>) {
        unsafe { sb::C_SkRRect_setRect(self.native_mut(), rect.as_ref().native()) }
    }

    /// Initializes bounds at (0, 0), the origin, with zero width and height. Initializes corner
    /// radii to (0, 0), and sets the type to [`Type::Empty`].
    pub fn new_empty() -> Self {
        Self::new()
    }

    // TODO: consider to rename all the following new_* function to from_* functions?
    //       is it possible to find a proper convention here (new_ vs from_?)?

    /// Initializes to a copy of `rect` bounds and zeroes corner radii.
    ///
    /// - `rect` bounds to copy
    pub fn new_rect(rect: impl AsRef<Rect>) -> Self {
        let mut rr = Self::default();
        rr.set_rect(rect);
        rr
    }

    /// Initializes to an oval, x-axis radii to half `oval.width()`, and all y-axis radii to half
    /// `oval.height()`. If the oval bounds is empty, sets the type to [`Type::Empty`]. Otherwise,
    /// sets the type to [`Type::Oval`].
    ///
    /// - `oval` bounds of oval
    pub fn new_oval(oval: impl AsRef<Rect>) -> Self {
        let mut rr = Self::default();
        rr.set_oval(oval);
        rr
    }

    /// Initializes to a rounded rectangle with the same radii for all four corners. If `rect` is
    /// empty, sets the type to [`Type::Empty`]. Otherwise, if `x_rad` and `y_rad` are zero, sets
    /// the type to [`Type::Rect`]. Otherwise, if `x_rad` is at least half `rect.width()` and
    /// `y_rad` is at least half `rect.height()`, sets the type to [`Type::Oval`]. Otherwise, sets
    /// the type to [`Type::Simple`].
    ///
    /// - `rect` bounds of rounded rectangle
    /// - `x_rad` x-axis radius of corners
    /// - `y_rad` y-axis radius of corners
    pub fn new_rect_xy(rect: impl AsRef<Rect>, x_rad: scalar, y_rad: scalar) -> Self {
        let mut rr = Self::default();
        rr.set_rect_xy(rect.as_ref(), x_rad, y_rad);
        rr
    }

    /// Initializes to a rounded rectangle with a radii array for individual control of all four
    /// corners.
    ///
    /// If `rect` is empty, sets the type to [`Type::Empty`]. Otherwise, if one of each corner radii
    /// are zero, sets the type to [`Type::Rect`]. Otherwise, if all x-axis radii are equal and at
    /// least half `rect.width()`, and all y-axis radii are equal at least half `rect.height()`,
    /// sets the type to [`Type::Oval`]. Otherwise, if all x-axis radii are equal, and all y-axis
    /// radii are equal, sets the type to [`Type::Simple`]. Otherwise, sets the type to
    /// [`Type::NinePatch`].
    ///
    /// - `rect` bounds of rounded rectangle
    /// - `radii` corner x-axis and y-axis radii
    pub fn new_rect_radii(rect: impl AsRef<Rect>, radii: &[Vector; 4]) -> Self {
        let mut rr = Self::default();
        rr.set_rect_radii(rect, radii);
        rr
    }

    /// Initializes bounds to `rect`. Sets radii to (`left_rad`, `top_rad`), (`right_rad`,
    /// `top_rad`), (`right_rad`, `bottom_rad`), (`left_rad`, `bottom_rad`).
    ///
    /// If `rect` is empty, sets the type to [`Type::Empty`]. Otherwise, if `left_rad` and
    /// `right_rad` are zero, sets the type to [`Type::Rect`]. Otherwise, if `top_rad` and
    /// `bottom_rad` are zero, sets the type to [`Type::Rect`]. Otherwise, if `left_rad` and
    /// `right_rad` are equal and at least half `rect.width()`, and `top_rad` and `bottom_rad` are
    /// equal at least half `rect.height()`, sets the type to [`Type::Oval`]. Otherwise, if
    /// `left_rad` and `right_rad` are equal, and `top_rad` and `bottom_rad` are equal, sets the
    /// type to [`Type::Simple`]. Otherwise, sets the type to [`Type::NinePatch`].
    ///
    /// Nine patch refers to the nine parts defined by the radii: one center rectangle, four edge
    /// patches, and four corner patches.
    ///
    /// - `rect` bounds of rounded rectangle
    /// - `left_rad` left-top and left-bottom x-axis radius
    /// - `top_rad` left-top and right-top y-axis radius
    /// - `right_rad` right-top and right-bottom x-axis radius
    /// - `bottom_rad` left-bottom and right-bottom y-axis radius
    pub fn new_nine_patch(
        rect: impl AsRef<Rect>,
        left_rad: scalar,
        top_rad: scalar,
        right_rad: scalar,
        bottom_rad: scalar,
    ) -> Self {
        let mut r = Self::default();
        r.set_nine_patch(rect, left_rad, top_rad, right_rad, bottom_rad);
        r
    }

    /// Sets bounds to `oval`, x-axis radii to half `oval.width()`, and all y-axis radii to half
    /// `oval.height()`. If the oval bounds is empty, sets the type to [`Type::Empty`]. Otherwise,
    /// sets the type to [`Type::Oval`].
    ///
    /// - `oval` bounds of oval
    pub fn set_oval(&mut self, oval: impl AsRef<Rect>) {
        unsafe { self.native_mut().setOval(oval.as_ref().native()) }
    }

    /// Sets to a rounded rectangle with the same radii for all four corners. If `rect` is empty,
    /// sets the type to [`Type::Empty`]. Otherwise, if `x_rad` or `y_rad` is zero, sets the type to
    /// [`Type::Rect`]. Otherwise, if `x_rad` is at least half `rect.width()` and `y_rad` is at
    /// least half `rect.height()`, sets the type to [`Type::Oval`]. Otherwise, sets the type to
    /// [`Type::Simple`].
    ///
    /// - `rect` bounds of rounded rectangle
    /// - `x_rad` x-axis radius of corners
    /// - `y_rad` y-axis radius of corners
    pub fn set_rect_xy(&mut self, rect: impl AsRef<Rect>, x_rad: scalar, y_rad: scalar) {
        unsafe {
            self.native_mut()
                .setRectXY(rect.as_ref().native(), x_rad, y_rad)
        }
    }

    /// Sets bounds to `rect`. Sets radii to (`left_rad`, `top_rad`), (`right_rad`, `top_rad`),
    /// (`right_rad`, `bottom_rad`), (`left_rad`, `bottom_rad`).
    ///
    /// If `rect` is empty, sets the type to [`Type::Empty`]. Otherwise, if `left_rad` and
    /// `right_rad` are zero, sets the type to [`Type::Rect`]. Otherwise, if `top_rad` and
    /// `bottom_rad` are zero, sets the type to [`Type::Rect`]. Otherwise, if `left_rad` and
    /// `right_rad` are equal and at least half `rect.width()`, and `top_rad` and `bottom_rad` are
    /// equal at least half `rect.height()`, sets the type to [`Type::Oval`]. Otherwise, if
    /// `left_rad` and `right_rad` are equal, and `top_rad` and `bottom_rad` are equal, sets the
    /// type to [`Type::Simple`]. Otherwise, sets the type to [`Type::NinePatch`].
    ///
    /// Nine patch refers to the nine parts defined by the radii: one center rectangle, four edge
    /// patches, and four corner patches.
    ///
    /// - `rect` bounds of rounded rectangle
    /// - `left_rad` left-top and left-bottom x-axis radius
    /// - `top_rad` left-top and right-top y-axis radius
    /// - `right_rad` right-top and right-bottom x-axis radius
    /// - `bottom_rad` left-bottom and right-bottom y-axis radius
    pub fn set_nine_patch(
        &mut self,
        rect: impl AsRef<Rect>,
        left_rad: scalar,
        top_rad: scalar,
        right_rad: scalar,
        bottom_rad: scalar,
    ) {
        unsafe {
            self.native_mut().setNinePatch(
                rect.as_ref().native(),
                left_rad,
                top_rad,
                right_rad,
                bottom_rad,
            )
        }
    }

    /// Sets bounds to `rect`. Sets the radii array for individual control of all four corners.
    ///
    /// If `rect` is empty, sets the type to [`Type::Empty`]. Otherwise, if one of each corner radii
    /// are zero, sets the type to [`Type::Rect`]. Otherwise, if all x-axis radii are equal and at
    /// least half `rect.width()`, and all y-axis radii are equal at least half `rect.height()`,
    /// sets the type to [`Type::Oval`]. Otherwise, if all x-axis radii are equal, and all y-axis
    /// radii are equal, sets the type to [`Type::Simple`]. Otherwise, sets the type to
    /// [`Type::NinePatch`].
    ///
    /// - `rect` bounds of rounded rectangle
    /// - `radii` corner x-axis and y-axis radii
    pub fn set_rect_radii(&mut self, rect: impl AsRef<Rect>, radii: &[Vector; 4]) {
        unsafe {
            self.native_mut()
                .setRectRadii(rect.as_ref().native(), radii.native().as_ptr())
        }
    }

    pub fn rect(&self) -> &Rect {
        Rect::from_native_ref(&self.native().fRect)
    }

    pub fn radii(&self, corner: Corner) -> Vector {
        Vector::from_native_c(self.native().fRadii[corner as usize])
    }

    pub fn radii_ref(&self) -> &[Vector; 4] {
        Vector::from_native_array_ref(&self.native().fRadii)
    }

    pub fn bounds(&self) -> &Rect {
        self.rect()
    }

    pub fn inset(&mut self, delta: impl Into<Vector>) {
        *self = self.with_inset(delta)
    }

    #[must_use]
    pub fn with_inset(&self, delta: impl Into<Vector>) -> Self {
        let delta = delta.into();
        let mut r = Self::default();
        unsafe { self.native().inset(delta.x, delta.y, r.native_mut()) };
        r
    }

    pub fn outset(&mut self, delta: impl Into<Vector>) {
        *self = self.with_outset(delta)
    }

    #[must_use]
    pub fn with_outset(&self, delta: impl Into<Vector>) -> Self {
        self.with_inset(-delta.into())
    }

    pub fn offset(&mut self, delta: impl Into<Vector>) {
        Rect::from_native_ref_mut(&mut self.native_mut().fRect).offset(delta)
    }

    #[must_use]
    pub fn with_offset(&self, delta: impl Into<Vector>) -> Self {
        let mut copied = *self;
        copied.offset(delta);
        copied
    }

    /// Returns true if `point` is inside the bounds and corner radii, and this rounded rectangle is
    /// not empty.
    pub fn contains_point(&self, point: impl Into<Point>) -> bool {
        let point = point.into();
        unsafe { sb::C_SkRRect_containsPoint(self.native(), point.native()) }
    }

    pub fn contains(&self, rect: impl AsRef<Rect>) -> bool {
        unsafe { sb::C_SkRRect_containsRect(self.native(), rect.as_ref().native()) }
    }

    pub fn is_valid(&self) -> bool {
        unsafe { self.native().isValid() }
    }

    pub const SIZE_IN_MEMORY: usize = mem::size_of::<Self>();

    pub fn write_to_memory(&self, buffer: &mut Vec<u8>) {
        unsafe {
            let size = self.native().writeToMemory(ptr::null_mut());
            buffer.resize(size, 0);
            let written = self.native().writeToMemory(buffer.as_mut_ptr() as _);
            debug_assert_eq!(written, size);
        }
    }

    pub fn read_from_memory(&mut self, buffer: &[u8]) -> usize {
        unsafe {
            self.native_mut()
                .readFromMemory(buffer.as_ptr() as _, buffer.len())
        }
    }

    #[must_use]
    pub fn transform(&self, matrix: &Matrix) -> Option<Self> {
        let mut r = Self::default();
        unsafe { self.native().transform1(matrix.native(), r.native_mut()) }.then_some(r)
    }

    pub fn dump(&self, as_hex: impl Into<Option<bool>>) {
        unsafe { self.native().dump(as_hex.into().unwrap_or_default()) }
    }

    pub fn dump_to_string(&self, as_hex: bool) -> String {
        let mut str = interop::String::default();
        unsafe { sb::C_SkRRect_dumpToString(self.native(), as_hex, str.native_mut()) }
        str.to_string()
    }

    pub fn dump_hex(&self) {
        self.dump(true)
    }
}
