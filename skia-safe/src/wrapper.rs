use crate::prelude::*;
///! FFI interoperability for skia-safe's wrapper types.
///!
///! This module is only meant to be used by external code. Internal code should continue to use the
///! traits in the `prelude` module.
use std::{
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
};

/// This trait supports the conversion of a wrapper into it's wrapped C/C++ pointer and back.
///
/// The wrapped value can be accessed through the functions `inner` and `inner_mut`.
///
/// # Safety
///
/// The native value `N` _should_ be treated as opaque, because its definition may change without
/// adhering to semantic versioning and depends on what the tool bindgen is able to generate.
///
/// Converting from a Rust wrapper to the wrapped value loses the automatic ability to free
/// associated resources.
pub unsafe trait PointerWrapper<N>
where
    Self: Sized,
{
    /// Wraps a native pointer into a wrapper type.
    /// Returns `None` if the pointer is `null`.
    fn wrap(ptr: *mut N) -> Option<Self>;
    /// Unwraps the wrapper type into the native pointer.
    fn unwrap(self) -> *mut N;
    /// Access the wrapped pointer.
    fn inner(&self) -> &N;
    /// Access the wrapped pointer.
    fn inner_mut(&mut self) -> &mut N;
}

/// A trait that supports the conversion from a C/C++ value into its Rust wrapper and back.
///
/// The wrapped value can be accessed through the functions `inner` and `inner_mut`.
///
/// This trait is implemented for all wrapper types that manage C++/C values in Rust without an
/// pointer indirection.
///
/// # Safety
///
/// The native type `N` _should_ be treated as opaque because its definition may change without
/// adhering to semantic versioning and depends on what the tool bindgen is able to generate.
///
/// Converting from a Rust wrapper to a wrapped value may lose the automatic ability to free
/// associated memory.
pub unsafe trait ValueWrapper<N> {
    fn wrap(native: N) -> Self;
    fn unwrap(self) -> N;
    fn inner(&self) -> &N;
    fn inner_mut(&mut self) -> &mut N;
}

/// A trait that supports the conversion from a C/C++ value into its Rust wrapper and back.
///
/// The wrapped value can be accessed through the functions `inner` and `inner_mut`.
///
/// This trait is implemented for for all types that implement `NativeTransmutable<N>`.
///
/// # Safety
///
/// The native type `N` _should_ be treated as opaque, because its definition may change
/// without adhering to semantic versioning and depends on what the tool bindgen is able to generate.
///
/// Converting from a Rust wrapper to a wrapped value may lose the automatic ability to free associated memory.
pub unsafe trait NativeTransmutableWrapper<N> {
    fn wrap(native: N) -> Self;
    fn unwrap(self) -> N;
    fn inner(&self) -> &N;
    fn inner_mut(&mut self) -> &mut N;
}

/// A trait that supports the conversion from a C/C++ reference into its Rust wrapper and back.
///
/// The wrapped value can be accessed through the functions `inner` and `inner_mut`.
///
/// This trait is implemented for all wrapper types that wrap C/C++ references.
///
/// # Safety
///
/// The native type `N` _should_ be treated as opaque, because its definition may change
/// without adhering to semantic versioning and depends on what the tool bindgen is able to generate.
///
/// Converting from a Rust wrapper to a wrapped value may lose the automatic ability to free associated memory.
pub unsafe trait RefWrapper<N> {
    fn wrap_ref(native: &N) -> &Self;
    fn wrap_mut(native: &mut N) -> &mut Self;
    fn inner(&self) -> &N;
    fn inner_mut(&mut self) -> &mut N;
}

/// Trait that enables a reference counted type to be compared by pointer equality.
pub trait PtrEq {
    fn ptr_eq(this: &Self, other: &Self) -> bool;
    fn ptr_hash<H: Hasher>(this: &Self, h: &mut H);
}

/// A wrapper to wrap values that implement [`PtrEq`] so that [`PartialEq`] and [`Eq`] are
/// implemented by pointer equality.
///
/// This way, reference counted Skia types can be used as hash map keys, for example.
#[derive(Clone, Eq, Debug)]
pub struct EqByPointer<T: PtrEq>(T);

impl<T: PtrEq> EqByPointer<T> {
    /// Wrap a value that implements [`PtrEq`].
    pub fn new(t: T) -> Self {
        Self(t)
    }
}

impl<T: PtrEq> Deref for EqByPointer<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: PtrEq> DerefMut for EqByPointer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: PtrEq> PartialEq for EqByPointer<T> {
    fn eq(&self, other: &Self) -> bool {
        <T as PtrEq>::ptr_eq(self, other)
    }
}

impl<T: PtrEq> Hash for EqByPointer<T> {
    fn hash<H>(&self, h: &mut H)
    where
        H: Hasher,
    {
        <T as PtrEq>::ptr_hash(self, h)
    }
}

//
// Handle<N>
//

unsafe impl<N> ValueWrapper<N> for Handle<N>
where
    N: NativeDrop,
{
    fn wrap(native: N) -> Self
    where
        N: NativeDrop,
    {
        Self::from_native_c(native)
    }

    fn unwrap(self) -> N {
        self.into_native()
    }

    fn inner(&self) -> &N {
        self.native()
    }

    fn inner_mut(&mut self) -> &mut N {
        self.native_mut()
    }
}

unsafe impl<N> RefWrapper<N> for Handle<N>
where
    N: NativeDrop,
{
    fn wrap_ref(native: &N) -> &Self {
        Self::from_native_ref(native)
    }

    fn wrap_mut(native: &mut N) -> &mut Self {
        Self::from_native_ref_mut(native)
    }

    fn inner(&self) -> &N {
        self.native()
    }

    fn inner_mut(&mut self) -> &mut N {
        self.native_mut()
    }
}

//
// RefHandle<N>
//

unsafe impl<N> PointerWrapper<N> for RefHandle<N>
where
    N: NativeDrop,
{
    fn wrap(ptr: *mut N) -> Option<Self> {
        Self::from_ptr(ptr)
    }

    fn unwrap(self) -> *mut N {
        self.into_ptr()
    }

    fn inner(&self) -> &N {
        self.native()
    }

    fn inner_mut(&mut self) -> &mut N {
        self.native_mut()
    }
}

//
// RCHandle<N>
//

unsafe impl<N> PointerWrapper<N> for RCHandle<N>
where
    N: NativeRefCounted,
{
    fn wrap(ptr: *mut N) -> Option<Self> {
        Self::from_ptr(ptr)
    }

    fn unwrap(self) -> *mut N {
        self.into_ptr()
    }

    fn inner(&self) -> &N {
        self.native()
    }

    fn inner_mut(&mut self) -> &mut N {
        self.native_mut()
    }
}

impl<N> PtrEq for RCHandle<N>
where
    N: NativeRefCounted,
{
    fn ptr_eq(this: &Self, other: &Self) -> bool {
        this.as_ptr() == other.as_ptr()
    }

    fn ptr_hash<H: Hasher>(this: &Self, h: &mut H) {
        std::ptr::hash(this.as_ptr(), h)
    }
}

//
// NativeTransmutable<N>
//

unsafe impl<N, T> NativeTransmutableWrapper<N> for T
where
    N: Sized,
    T: Sized,
    T: NativeTransmutable<N>,
{
    fn wrap(native: N) -> Self {
        Self::from_native_c(native)
    }

    fn unwrap(self) -> N {
        Self::into_native(self)
    }

    fn inner(&self) -> &N {
        self.native()
    }

    fn inner_mut(&mut self) -> &mut N {
        self.native_mut()
    }
}
