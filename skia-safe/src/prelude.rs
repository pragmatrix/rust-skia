use skia_bindings::{
    C_SkRefCntBase_ref, C_SkRefCntBase_unique, C_SkRefCntBase_unref, SkNVRefCnt, SkRefCnt,
    SkRefCntBase,
};
use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::{mem, ptr, slice};
// Re-export TryFrom / TryInto to make them available in all modules that use prelude::*.
pub use std::convert::{TryFrom, TryInto};
use std::marker::PhantomData;

/// Swiss army knife to convert any reference into any other.
pub unsafe fn transmute_ref<FromT, ToT>(from: &FromT) -> &ToT {
    // TODO: can we do this statically for all instantiations of transmute_ref?
    debug_assert_eq!(mem::size_of::<FromT>(), mem::size_of::<ToT>());
    &*(from as *const FromT as *const ToT)
}

pub unsafe fn transmute_ref_mut<FromT, ToT>(from: &mut FromT) -> &mut ToT {
    // TODO: can we do this statically for all instantiations of transmute_ref_mut?
    debug_assert_eq!(mem::size_of::<FromT>(), mem::size_of::<ToT>());
    &mut *(from as *mut FromT as *mut ToT)
}

pub(crate) trait IntoOption {
    type Target;
    fn into_option(self) -> Option<Self::Target>;
}

impl<T> IntoOption for *const T {
    type Target = *const T;

    fn into_option(self) -> Option<Self::Target> {
        if !self.is_null() {
            Some(self)
        } else {
            None
        }
    }
}

impl<T> IntoOption for *mut T {
    type Target = *mut T;

    fn into_option(self) -> Option<Self::Target> {
        if !self.is_null() {
            Some(self)
        } else {
            None
        }
    }
}

impl IntoOption for bool {
    type Target = ();

    fn into_option(self) -> Option<Self::Target> {
        if self {
            Some(())
        } else {
            None
        }
    }
}

pub(crate) trait IfBoolSome {
    fn if_true_some<V>(self, v: V) -> Option<V>;
    fn if_false_some<V>(self, v: V) -> Option<V>;
    fn if_true_then_some<V>(self, f: impl FnOnce() -> V) -> Option<V>;
    fn if_false_then_some<V>(self, f: impl FnOnce() -> V) -> Option<V>;
}

impl IfBoolSome for bool {
    fn if_true_some<V>(self, v: V) -> Option<V> {
        self.into_option().and(Some(v))
    }

    fn if_false_some<V>(self, v: V) -> Option<V> {
        (!self).if_true_some(v)
    }

    fn if_true_then_some<V>(self, f: impl FnOnce() -> V) -> Option<V> {
        self.into_option().map(|()| f())
    }

    fn if_false_then_some<V>(self, f: impl FnOnce() -> V) -> Option<V> {
        (!self).into_option().map(|()| f())
    }
}

pub(crate) trait RefCount {
    fn ref_cnt(&self) -> usize;
}

impl RefCount for SkRefCntBase {
    // the problem here is that the binding generator represents std::atomic as an u8 (we
    // are lucky that the C alignment rules make space for an i32), so to get the ref
    // counter, we need to get the u8 pointer to fRefCnt and interpret it as an i32 pointer.
    #[allow(clippy::cast_ptr_alignment)]
    fn ref_cnt(&self) -> usize {
        unsafe {
            let ptr: *const i32 = &self.fRefCnt as *const _ as *const i32;
            (*ptr).try_into().unwrap()
        }
    }
}

impl RefCount for SkRefCnt {
    fn ref_cnt(&self) -> usize {
        self._base.ref_cnt()
    }
}

impl RefCount for SkNVRefCnt {
    #[allow(clippy::cast_ptr_alignment)]
    fn ref_cnt(&self) -> usize {
        unsafe {
            let ptr: *const i32 = &self.fRefCnt as *const _ as *const i32;
            (*ptr).try_into().unwrap()
        }
    }
}

pub trait NativeRefCounted: Sized {
    fn _ref(&self);
    fn _unref(&self);
    fn unique(&self) -> bool;
    fn _ref_cnt(&self) -> usize {
        unimplemented!();
    }
}

impl NativeRefCounted for SkRefCntBase {
    fn _ref(&self) {
        unsafe { C_SkRefCntBase_ref(self) }
    }

    fn _unref(&self) {
        unsafe { C_SkRefCntBase_unref(self) }
    }

    fn unique(&self) -> bool {
        unsafe { C_SkRefCntBase_unique(self) }
    }

    #[allow(clippy::cast_ptr_alignment)]
    fn _ref_cnt(&self) -> usize {
        unsafe {
            let ptr: *const i32 = &self.fRefCnt as *const _ as *const i32;
            (*ptr).try_into().unwrap()
        }
    }
}

/// Implements NativeRefCounted by just providing a reference to the base class
/// that implements a RefCount.
pub trait NativeRefCountedBase {
    type Base: NativeRefCounted;

    /// Returns the ref counter base class of the ref counted type.
    ///
    /// Default implementation assumes that the base class ptr is the same as the
    /// ptr to self.
    fn ref_counted_base(&self) -> &Self::Base {
        unsafe { &*(self as *const _ as *const Self::Base) }
    }
}

impl<Native, Base: NativeRefCounted> NativeRefCounted for Native
where
    Native: NativeRefCountedBase<Base = Base>,
{
    fn _ref(&self) {
        self.ref_counted_base()._ref();
    }

    fn _unref(&self) {
        self.ref_counted_base()._unref();
    }

    fn unique(&self) -> bool {
        self.ref_counted_base().unique()
    }

    fn _ref_cnt(&self) -> usize {
        self.ref_counted_base()._ref_cnt()
    }
}

/// Trait that enables access to a native representation by reference.
pub(crate) trait NativeAccess<N> {
    fn native(&self) -> &N;
    fn native_mut(&mut self) -> &mut N;
    // Returns a ptr to the native mutable value.
    unsafe fn native_mut_force(&self) -> *mut N {
        self.native() as *const N as *mut N
    }
}

/// Implements Drop for native types we can not implement Drop for.
pub trait NativeDrop {
    fn drop(&mut self);
}

/// Clone for bindings types we can not implement Clone for.
pub trait NativeClone {
    fn clone(&self) -> Self;
}

/// Even though some types may have value semantics, equality
/// comparison may need to be customized.
pub trait NativePartialEq {
    fn eq(&self, rhs: &Self) -> bool;
}

/// Implements Hash for the native type so that the wrapper type
/// can derive it from.
pub trait NativeHash {
    fn hash<H: Hasher>(&self, state: &mut H);
}

/// Wraps a native type that can be represented and used in Rust memory.
///
/// This type requires the trait `NativeDrop` to be implemented.
#[repr(transparent)]
pub struct Handle<N: NativeDrop>(N);

impl<N: NativeDrop> AsRef<Handle<N>> for Handle<N> {
    fn as_ref(&self) -> &Self {
        &self
    }
}

impl<N: NativeDrop> Handle<N> {
    /// Wrap a native instance into a handle.
    /// TODO: rename to wrap_native() and mark as unsafe.
    pub(crate) fn from_native(n: N) -> Self {
        Handle(n)
    }

    /// Create a reference to the Rust wrapper from a reference to the native type.
    pub(crate) fn from_native_ref(n: &N) -> &Self {
        unsafe { transmute_ref(n) }
    }

    /// Create a mutable reference to the Rust wrapper from a reference to the native type.
    #[allow(dead_code)]
    pub(crate) fn from_native_ref_mut(n: &mut N) -> &mut Self {
        unsafe { transmute_ref_mut(n) }
    }

    /// Constructs a C++ object in place by calling a
    /// function that expects a pointer that points to
    /// uninitialized memory of the native type.
    pub(crate) fn construct(construct: impl FnOnce(*mut N)) -> Self {
        Self::from_native(self::construct(construct))
    }

    /// Replaces the native instance with the one from this Handle, and
    /// returns the replaced one wrapped in  a Rust Handle without
    /// deinitializing either one.
    pub(crate) fn replace_native(mut self, native: &mut N) -> Self {
        mem::swap(&mut self.0, native);
        self
    }
}

pub(crate) trait ReplaceWith<Other> {
    fn replace_with(&mut self, other: Other) -> Other;
}

impl<N: NativeDrop> ReplaceWith<Handle<N>> for N {
    fn replace_with(&mut self, other: Handle<N>) -> Handle<N> {
        other.replace_native(self)
    }
}

/// Constructs a C++ object in place by calling a lambda that is meant to initialize
/// the pointer to the Rust memory provided as a pointer.
pub(crate) fn construct<N>(construct: impl FnOnce(*mut N)) -> N {
    let mut instance = MaybeUninit::uninit();
    construct(instance.as_mut_ptr());
    unsafe { instance.assume_init() }
}

impl<N: NativeDrop> Drop for Handle<N> {
    fn drop(&mut self) {
        self.0.drop()
    }
}

impl<N: NativeDrop> NativeAccess<N> for Handle<N> {
    fn native(&self) -> &N {
        &self.0
    }

    fn native_mut(&mut self) -> &mut N {
        &mut self.0
    }
}

impl<N: NativeDrop + NativeClone> Clone for Handle<N> {
    fn clone(&self) -> Self {
        Self::from_native(self.0.clone())
    }
}

impl<N: NativeDrop + NativePartialEq> PartialEq for Handle<N> {
    fn eq(&self, rhs: &Self) -> bool {
        self.native().eq(rhs.native())
    }
}

impl<N: NativeDrop + NativeHash> Hash for Handle<N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.native().hash(state);
    }
}

pub(crate) trait NativeSliceAccess<N: NativeDrop> {
    fn native(&self) -> &[N];
}

impl<N: NativeDrop> NativeSliceAccess<N> for [Handle<N>] {
    fn native(&self) -> &[N] {
        let ptr = self
            .first()
            .map(|f| f.native() as *const _)
            .unwrap_or(ptr::null());
        unsafe { slice::from_raw_parts(ptr, self.len()) }
    }
}

/// A trait that supports retrieving a pointer from an Option<Handle<Native>>.
/// Returns a null pointer if the Option is None.
pub(crate) trait NativePointerOrNull<N> {
    fn native_ptr_or_null(&self) -> *const N;
    unsafe fn native_ptr_or_null_mut_force(&self) -> *mut N;
}

pub(crate) trait NativePointerOrNullMut<N> {
    fn native_ptr_or_null_mut(&mut self) -> *mut N;
}

impl<H, N> NativePointerOrNull<N> for Option<&H>
where
    H: NativeAccess<N>,
{
    fn native_ptr_or_null(&self) -> *const N {
        match self {
            Some(handle) => handle.native(),
            None => ptr::null(),
        }
    }

    unsafe fn native_ptr_or_null_mut_force(&self) -> *mut N {
        match self {
            Some(handle) => handle.native_mut_force(),
            None => ptr::null_mut(),
        }
    }
}

impl<H, N> NativePointerOrNullMut<N> for Option<&mut H>
where
    H: NativeAccess<N>,
{
    fn native_ptr_or_null_mut(&mut self) -> *mut N {
        match self {
            Some(handle) => handle.native_mut(),
            None => ptr::null_mut(),
        }
    }
}

pub(crate) trait NativePointerOrNullMut2<N> {
    fn native_ptr_or_null_mut(&mut self) -> *mut N;
}

pub(crate) trait NativePointerOrNull2<N> {
    fn native_ptr_or_null(&self) -> *const N;
}

impl<H, N> NativePointerOrNull2<N> for Option<&H>
where
    H: NativeTransmutable<N>,
{
    fn native_ptr_or_null(&self) -> *const N {
        match self {
            Some(handle) => handle.native(),
            None => ptr::null(),
        }
    }
}

impl<H, N> NativePointerOrNullMut2<N> for Option<&mut H>
where
    H: NativeTransmutable<N>,
{
    fn native_ptr_or_null_mut(&mut self) -> *mut N {
        match self {
            Some(handle) => handle.native_mut(),
            None => ptr::null_mut(),
        }
    }
}

/// A wrapper type that represents a native type with a pointer to
/// the native object.
#[repr(transparent)]
pub struct RefHandle<N: NativeDrop>(*mut N);

impl<N: NativeDrop> Drop for RefHandle<N> {
    fn drop(&mut self) {
        self.native_mut().drop()
    }
}

impl<N: NativeDrop> NativeAccess<N> for RefHandle<N> {
    fn native(&self) -> &N {
        unsafe { &*self.0 }
    }
    fn native_mut(&mut self) -> &mut N {
        unsafe { &mut *self.0 }
    }
}

impl<N: NativeDrop> RefHandle<N> {
    /// Creates a RefHandle from a native pointer.
    ///
    /// From this time on the RefHandle ownes the object that the pointer points
    /// to and will call it's NativeDrop implementation it it goes out of scope.
    pub(crate) fn from_ptr(ptr: *mut N) -> Option<Self> {
        ptr.into_option().map(Self)
    }
}

/// A wrapper type represented by a reference counted pointer
/// to the native type.
#[repr(transparent)]
pub struct RCHandle<Native: NativeRefCounted>(*mut Native);

/// A reference counted handle is cheap to clone, so we do support a conversion
/// from a reference to a ref counter to an owned handle.
impl<N: NativeRefCounted> From<&RCHandle<N>> for RCHandle<N> {
    fn from(rch: &RCHandle<N>) -> Self {
        rch.clone()
    }
}

impl<N: NativeRefCounted> AsRef<RCHandle<N>> for RCHandle<N> {
    fn as_ref(&self) -> &Self {
        &self
    }
}

impl<N: NativeRefCounted> RCHandle<N> {
    /// Creates an RCHandle from a pointer.
    /// Returns None if the pointer is null.
    /// Does not increase the reference count.
    #[inline]
    pub(crate) fn from_ptr(ptr: *mut N) -> Option<Self> {
        if !ptr.is_null() {
            Some(RCHandle(ptr))
        } else {
            None
        }
    }

    /// Creates an RCHandle from a pointer.
    /// Returns None if the pointer is null.
    /// Increases the reference count.
    #[inline]
    pub(crate) fn from_unshared_ptr(ptr: *mut N) -> Option<Self> {
        ptr.into_option().map(|ptr| {
            unsafe { (*ptr)._ref() };
            Self(ptr)
        })
    }
}

impl<N: NativeRefCounted> NativeAccess<N> for RCHandle<N> {
    /// Returns a reference to the native representation.
    fn native(&self) -> &N {
        unsafe { &*self.0 }
    }

    /// Returns a mutable reference to the native representation.
    fn native_mut(&mut self) -> &mut N {
        unsafe { &mut *self.0 }
    }
}

impl<N: NativeRefCounted> Clone for RCHandle<N> {
    fn clone(&self) -> Self {
        // Support shared mutability when a ref-counted handle is cloned.
        let ptr = self.0;
        unsafe { (&*ptr)._ref() };
        Self(ptr)
    }
}

impl<N: NativeRefCounted> Drop for RCHandle<N> {
    #[inline]
    fn drop(&mut self) {
        unsafe { &*self.0 }._unref();
    }
}

impl<N: NativeRefCounted + NativePartialEq> PartialEq for RCHandle<N> {
    fn eq(&self, rhs: &Self) -> bool {
        self.native().eq(rhs.native())
    }
}

/// A trait that consumes self and converts it to a ptr to the native type.
pub(crate) trait IntoPtr<N> {
    fn into_ptr(self) -> *mut N;
}

impl<N: NativeRefCounted> IntoPtr<N> for RCHandle<N> {
    fn into_ptr(self) -> *mut N {
        let ptr = self.0;
        mem::forget(self);
        ptr
    }
}

/// A trait that consumes self and converts it to a ptr to the native type or null.
pub(crate) trait IntoPtrOrNull<N> {
    fn into_ptr_or_null(self) -> *mut N;
}

impl<N: NativeRefCounted> IntoPtrOrNull<N> for Option<RCHandle<N>> {
    fn into_ptr_or_null(self) -> *mut N {
        self.map(|rc| rc.into_ptr()).unwrap_or(ptr::null_mut())
    }
}

/// Trait to compute how many bytes the elements of this type occupy in memory.
pub(crate) trait ElementsSizeOf {
    fn elements_size_of(&self) -> usize;
}

impl<N: Sized> ElementsSizeOf for [N] {
    fn elements_size_of(&self) -> usize {
        mem::size_of::<N>() * self.len()
    }
}

/// Tag the type to automatically implement get() functions for
/// all Index implementations.
pub trait IndexGet {}

/// Tag the type to automatically implement get() and set() functions
/// for all Index & IndexMut implementation for that type.
pub trait IndexSet {}

pub trait IndexGetter<I, O: Copy> {
    fn get(&self, index: I) -> O;
}

impl<T, I, O: Copy> IndexGetter<I, O> for T
where
    T: Index<I, Output = O> + IndexGet,
{
    fn get(&self, index: I) -> O {
        self[index]
    }
}

pub trait IndexSetter<I, O: Copy> {
    fn set(&mut self, index: I, value: O) -> &mut Self;
}

impl<T, I, O: Copy> IndexSetter<I, O> for T
where
    T: IndexMut<I, Output = O> + IndexSet,
{
    fn set(&mut self, index: I, value: O) -> &mut Self {
        self[index] = value;
        self
    }
}

/// Trait to use native types that as a rust type
/// _inplace_ with the same size and field layout.
pub(crate) trait NativeTransmutable<NT: Sized>: Sized {
    /// Provides access to the native value through a
    /// transmuted reference to the Rust value.
    fn native(&self) -> &NT {
        unsafe { transmute_ref(self) }
    }

    /// Provides mutable access to the native value through a
    /// transmuted reference to the Rust value.
    fn native_mut(&mut self) -> &mut NT {
        unsafe { transmute_ref_mut(self) }
    }

    /// Copies the native value to an equivalent Rust value.
    fn from_native(nt: NT) -> Self {
        unsafe { mem::transmute_copy::<NT, Self>(&nt) }
    }

    /// Copies the rust type to an equivalent instance of the native type.
    fn into_native(self) -> NT {
        unsafe { mem::transmute_copy::<Self, NT>(&self) }
    }

    /// Provides access to the Rust value through a
    /// transmuted reference to the native value.
    fn from_native_ref(nt: &NT) -> &Self {
        unsafe { transmute_ref(nt) }
    }

    /// Provides access to the Rust value through a
    /// transmuted reference to the native mutable value.
    fn from_native_ref_mut(nt: &mut NT) -> &mut Self {
        unsafe { transmute_ref_mut(nt) }
    }

    /// Runs a test that proves that the native and the rust
    /// type are of the same size.
    fn test_layout() {
        assert_eq!(mem::size_of::<Self>(), mem::size_of::<NT>());
    }

    fn construct(construct: impl FnOnce(*mut NT)) -> Self {
        Self::from_native(self::construct(construct))
    }
}

pub(crate) trait NativeTransmutableSliceAccess<NT: Sized> {
    fn native(&self) -> &[NT];
    fn native_mut(&mut self) -> &mut [NT];
}

impl<NT, ElementT> NativeTransmutableSliceAccess<NT> for [ElementT]
where
    ElementT: NativeTransmutable<NT>,
{
    fn native(&self) -> &[NT] {
        unsafe { &*(self as *const [ElementT] as *const [NT]) }
    }

    fn native_mut(&mut self) -> &mut [NT] {
        unsafe { &mut *(self as *mut [ElementT] as *mut [NT]) }
    }
}

impl<NT, RustT> NativeTransmutable<Option<NT>> for Option<RustT> where RustT: NativeTransmutable<NT> {}

impl<NT, RustT> NativeTransmutable<Option<&[NT]>> for Option<&[RustT]> where
    RustT: NativeTransmutable<NT>
{
}

pub(crate) trait NativeTransmutableOptionSliceAccessMut<NT: Sized> {
    fn native_mut(&mut self) -> &mut Option<&mut [NT]>;
}

impl<NT, RustT> NativeTransmutableOptionSliceAccessMut<NT> for Option<&mut [RustT]>
where
    RustT: NativeTransmutable<NT>,
{
    fn native_mut(&mut self) -> &mut Option<&mut [NT]> {
        unsafe { transmute_ref_mut(self) }
    }
}

//
// Convenience functions to access Option<&[]> as optional ptr (opt_ptr)
// that may be null.
//

pub(crate) trait AsPointerOrNull<PointerT> {
    fn as_ptr_or_null(&self) -> *const PointerT;
}

pub(crate) trait AsPointerOrNullMut<PointerT> {
    fn as_ptr_or_null(&self) -> *const PointerT;
    fn as_ptr_or_null_mut(&mut self) -> *mut PointerT;
}

impl<E> AsPointerOrNull<E> for Option<E> {
    fn as_ptr_or_null(&self) -> *const E {
        match self {
            Some(e) => e,
            None => ptr::null(),
        }
    }
}

impl<E> AsPointerOrNull<E> for Option<&[E]> {
    fn as_ptr_or_null(&self) -> *const E {
        match self {
            Some(slice) => slice.as_ptr(),
            None => ptr::null(),
        }
    }
}

impl<E> AsPointerOrNullMut<E> for Option<&mut [E]> {
    fn as_ptr_or_null(&self) -> *const E {
        match self {
            Some(slice) => slice.as_ptr(),
            None => ptr::null(),
        }
    }

    fn as_ptr_or_null_mut(&mut self) -> *mut E {
        match self {
            Some(slice) => slice.as_mut_ptr(),
            None => ptr::null_mut(),
        }
    }
}

impl<E> AsPointerOrNull<E> for Option<&Vec<E>> {
    fn as_ptr_or_null(&self) -> *const E {
        match self {
            Some(v) => v.as_ptr(),
            None => ptr::null(),
        }
    }
}

impl<E> AsPointerOrNullMut<E> for Option<Vec<E>> {
    fn as_ptr_or_null(&self) -> *const E {
        match self {
            Some(v) => v.as_ptr(),
            None => ptr::null(),
        }
    }

    fn as_ptr_or_null_mut(&mut self) -> *mut E {
        match self {
            Some(v) => v.as_mut_ptr(),
            None => ptr::null_mut(),
        }
    }
}

// Wraps a handle so that the Rust's borrow checker assumes it represents
// something that borrows something else.
#[repr(C)]
pub struct Borrows<'a, H>(H, PhantomData<&'a ()>);

impl<'a, H> Deref for Borrows<'a, H> {
    type Target = H;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, H> DerefMut for Borrows<'a, H> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, H> Borrows<'a, H> {
    /// Release the borrowed dependency and return the handle.
    pub unsafe fn release(self) -> H {
        self.0
    }
}

pub(crate) trait BorrowsFrom: Sized {
    fn borrows<D: ?Sized>(self, _dep: &D) -> Borrows<Self>;
}

impl<T: Sized> BorrowsFrom for T {
    fn borrows<D: ?Sized>(self, _dep: &D) -> Borrows<Self> {
        Borrows(self, PhantomData)
    }
}
