// Copyright (C) 2023 - 2025 iDigitalFlame
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//

#![no_implicit_prelude]

extern crate alloc;
extern crate core;

extern crate xrmt_io;

use alloc::alloc::Global;
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::bstr::ByteString;
use alloc::collections::TryReserveError;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::{self, Vec};
use core::alloc::Allocator;
use core::ascii::Char;
use core::borrow::{Borrow, BorrowMut};
use core::bstr::ByteStr;
use core::clone::Clone;
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::{AsMut, AsRef, From, Infallible, Into};
use core::default::Default;
use core::error::Error;
use core::fmt::{Debug, Display, Formatter, Write};
use core::hash::{Hash, Hasher};
use core::iter::{DoubleEndedIterator, Extend, FromIterator, FusedIterator, IntoIterator, Iterator};
use core::marker::{Send, Sync};
use core::mem::{drop, swap, transmute};
use core::ops::{Add, AddAssign, Deref, DerefMut, FnMut, Index, IndexMut, RangeBounds};
use core::option::Option::{self, None, Some};
use core::ptr::{copy, copy_nonoverlapping};
use core::result::Result::{self, Err, Ok};
use core::slice::SliceIndex;
use core::str::{from_utf8, from_utf8_unchecked, from_utf8_unchecked_mut, FromStr, Utf8Error};

use xrmt_io::FmtResult;

use crate::text::{utf16_to_fiber_in, utf8_to_lossy_insert, utf8_to_lossy_rewrite, CharSize, U16DecodeError, U16Decoder};
use crate::{AllocFrom, AllocInto};

pub struct Fiber<A: Allocator = Global> {
    vec: Vec<u8, A>,
}
pub struct FromUtf16Error(U16DecodeError);
pub struct FromUtf8Error<A: Allocator = Global> {
    bytes: Vec<u8, A>,
    error: Utf8Error,
}
pub struct Drain<'a, A: Allocator = Global>(vec::Drain<'a, u8, A>);

pub trait MaybeString {
    fn as_maybe(&self) -> Option<&str>;
}
pub trait ToFiber<A: Allocator = Global> {
    fn to_fiber(&self) -> Fiber<A>;
}

impl Fiber {
    /// Creates a new empty `Fiber`.
    ///
    /// Given that the `Fiber` is empty, this will not allocate any initial
    /// buffer. While that means that this initial operation is very
    /// inexpensive, it may cause excessive allocation later when you add
    /// data. If you have an idea of how much data the `Fiber` will hold,
    /// consider the [`with_capacity`] method to prevent excessive
    /// re-allocation.
    ///
    /// [`with_capacity`]: Fiber::with_capacity
    ///
    /// # Examples
    ///
    /// ```
    /// let s = Fiber::new();
    /// ```
    #[inline]
    pub const fn new() -> Fiber {
        Fiber { vec: Vec::new() }
    }

    #[inline]
    pub fn from_str(v: &str) -> Fiber {
        Fiber { vec: v.as_bytes().to_vec() }
    }
    /// Converts a slice of bytes to a string, including invalid characters.
    ///
    /// Fiber are made of bytes ([`u8`]), and a slice of bytes
    /// ([`&[u8]`][byteslice]) is made of bytes, so this function converts
    /// between the two. Not all byte slices are valid strings, however: strings
    /// are required to be valid UTF-8. During this conversion,
    /// `from_utf8_lossy()` will replace any invalid UTF-8 sequences with
    /// [`U+FFFD REPLACEMENT CHARACTER`][U+FFFD], which looks like this: �
    ///
    /// [byteslice]: prim@slice
    /// [U+FFFD]: core::char::REPLACEMENT_CHARACTER
    ///
    /// If you are sure that the byte slice is valid UTF-8, and you don't want
    /// to incur the overhead of the conversion, there is an unsafe version
    /// of this function, [`from_utf8_unchecked`], which has the same behavior
    /// but skips the checks.
    ///
    /// [`from_utf8_unchecked`]: Fiber::from_utf8_unchecked
    ///
    /// This function returns a [`Cow<'a, str>`]. If our byte slice is invalid
    /// UTF-8, then we need to insert the replacement characters, which will
    /// change the size of the string, and hence, require a `Fiber`. But if
    /// it's already valid UTF-8, we don't need a new allocation. This return
    /// type allows us to handle both cases.
    ///
    /// [`Cow<'a, str>`]: Cow "borrow::Cow"
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// // some bytes, in a vector
    /// let sparkle_heart = vec![240, 159, 146, 150];
    ///
    /// let sparkle_heart = Fiber::from_utf8_lossy(&sparkle_heart);
    ///
    /// assert_eq!("💖", sparkle_heart);
    /// ```
    ///
    /// Incorrect bytes:
    ///
    /// ```
    /// // some invalid bytes
    /// let input = b"Hello \xF0\x90\x80World";
    /// let output = Fiber::from_utf8_lossy(input);
    ///
    /// assert_eq!("Hello �World", output);
    /// ```
    #[inline]
    pub fn from_utf8_lossy(v: &[u8]) -> Fiber {
        Fiber::from_utf8_lossy_in(v, Global)
    }
    /// Creates a new empty `Fiber` with at least the specified capacity.
    ///
    /// `Fiber`s have an internal buffer to hold their data. The capacity is
    /// the length of that buffer, and can be queried with the [`capacity`]
    /// method. This method creates an empty `Fiber`, but one with an initial
    /// buffer that can hold at least `capacity` bytes. This is useful when you
    /// may be appending a bunch of data to the `Fiber`, reducing the number of
    /// reallocations it needs to do.
    ///
    /// [`capacity`]: Fiber::capacity
    ///
    /// If the given capacity is `0`, no allocation will occur, and this method
    /// is identical to the [`new`] method.
    ///
    /// [`new`]: Fiber::new
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = Fiber::with_capacity(10);
    ///
    /// // The Fiber contains no chars, even though it has capacity for more
    /// assert_eq!(s.len(), 0);
    ///
    /// // These are all done without reallocating...
    /// let cap = s.capacity();
    /// for _ in 0..10 {
    ///     s.push('a');
    /// }
    ///
    /// assert_eq!(s.capacity(), cap);
    ///
    /// // ...but this may make the string reallocate
    /// s.push('a');
    /// ```
    #[inline]
    pub fn with_capacity(len: usize) -> Fiber {
        Fiber { vec: Vec::with_capacity(len) }
    }
    /// Decode a native endian UTF-16–encoded slice `v` into a `Fiber`,
    /// replacing invalid data with [the replacement character
    /// (`U+FFFD`)][U+FFFD].
    ///
    /// Unlike [`from_utf8_lossy`] which returns a [`Cow<'a, str>`],
    /// `from_utf16_lossy` returns a `Fiber` since the UTF-16 to UTF-8
    /// conversion requires a memory allocation.
    ///
    /// [`from_utf8_lossy`]: Fiber::from_utf8_lossy
    /// [`Cow<'a, str>`]: alloc::borrow::Cow "borrow::Cow"
    /// [U+FFFD]: core::char::REPLACEMENT_CHARACTER
    ///
    /// # Examples
    ///
    /// ```
    /// // 𝄞mus<invalid>ic<invalid>
    /// let v = &[0xD834, 0xDD1E, 0x006d, 0x0075,
    ///           0x0073, 0xDD1E, 0x0069, 0x0063,
    ///           0xD834];
    ///
    /// assert_eq!(Fiber::from("𝄞mus\u{FFFD}ic\u{FFFD}"),
    ///            Fiber::from_utf16_lossy(v));
    /// ```
    #[inline]
    pub fn from_utf16_lossy(v: &[u16]) -> Fiber {
        Fiber::from_utf16_lossy_in(v, Global)
    }
    /// Converts a [`Vec<u8>`] to a `Fiber`, substituting invalid UTF-8
    /// sequences with replacement characters.
    ///
    /// See [`from_utf8_lossy`] for more details.
    ///
    /// [`from_utf8_lossy`]: Fiber::from_utf8_lossy
    ///
    /// Note that this function does not guarantee reuse of the original `Vec`
    /// allocation.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// // some bytes, in a vector
    /// let sparkle_heart = vec![240, 159, 146, 150];
    ///
    /// let sparkle_heart = Fiber::from_utf8_lossy_owned(sparkle_heart);
    ///
    /// assert_eq!(Fiber::from("💖"), sparkle_heart);
    /// ```
    ///
    /// Incorrect bytes:
    ///
    /// ```
    /// // some invalid bytes
    /// let input: Vec<u8> = b"Hello \xF0\x90\x80World".into();
    /// let output = Fiber::from_utf8_lossy_owned(input);
    ///
    /// assert_eq!(String::from("Hello �World"), output);
    /// ```
    #[inline]
    pub fn from_utf8_lossy_owned(v: Vec<u8>) -> Fiber {
        Fiber { vec: v }
    }
    /// Decode a native endian UTF-16–encoded vector `v` into a `Fiber`,
    /// returning [`Err`] if `v` contains any invalid data.
    ///
    /// # Examples
    ///
    /// ```
    /// // 𝄞music
    /// let v = &[0xD834, 0xDD1E, 0x006d, 0x0075,
    ///           0x0073, 0x0069, 0x0063];
    /// assert_eq!(Fiber::from("𝄞music"),
    ///            Fiber::from_utf16(v).unwrap());
    ///
    /// // 𝄞mu<invalid>ic
    /// let v = &[0xD834, 0xDD1E, 0x006d, 0x0075,
    ///           0xD800, 0x0069, 0x0063];
    /// assert!(Fiber::from_utf16(v).is_err());
    /// ```
    #[inline]
    pub fn from_utf16(v: &[u16]) -> Result<Fiber, FromUtf16Error> {
        Fiber::from_utf16_in(v, Global)
    }

    /// Converts a slice of bytes to a string slice without checking
    /// that the string contains valid UTF-8.
    ///
    /// See the safe version, [`from_utf8`], for more information.
    ///
    /// # Safety
    ///
    /// The bytes passed in must be valid UTF-8.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use xrmt_stx::str;
    ///
    /// // some bytes, in a vector
    /// let sparkle_heart = vec![240, 159, 146, 150];
    ///
    /// let sparkle_heart = unsafe {
    ///     Fiber::from_utf8_unchecked(&sparkle_heart)
    /// };
    ///
    /// assert_eq!("💖", sparkle_heart);
    /// ```
    #[inline]
    pub unsafe fn from_utf8_slice_unchecked(v: &[u8]) -> Fiber {
        Fiber { vec: v.to_vec() }
    }
    /// Creates a new `Fiber` from a pointer, a length and a capacity.
    ///
    /// # Safety
    ///
    /// This is highly unsafe, due to the number of invariants that aren't
    /// checked:
    ///
    /// * all safety requirements for [`Vec::<u8>::from_raw_parts`].
    /// * all safety requirements for [`String::from_utf8_unchecked`].
    ///
    /// Violating these may cause problems like corrupting the allocator's
    /// internal data structures. For example, it is normally **not** safe to
    /// build a `Fiber` from a pointer to a C `char` array containing UTF-8
    /// _unless_ you are certain that array was originally allocated by the
    /// Rust standard library's allocator.
    ///
    /// The ownership of `buf` is effectively transferred to the
    /// `Fiber` which may then deallocate, reallocate or change the
    /// contents of memory pointed to by the pointer at will. Ensure
    /// that nothing else uses the pointer after calling this
    /// function.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::mem;
    ///
    /// unsafe {
    ///     let s = String::from("hello");
    /// }
    #[inline]
    pub unsafe fn from_raw_parts(buf: *mut u8, length: usize, capacity: usize) -> Fiber {
        unsafe {
            Fiber {
                vec: Vec::from_raw_parts(buf, length, capacity),
            }
        }
    }

    /// Decomposes a `Fiber` into its raw components: `(pointer, length,
    /// capacity)`.
    ///
    /// Returns the raw pointer to the underlying data, the length of
    /// the string (in bytes), and the allocated capacity of the data
    /// (in bytes). These are the same arguments in the same order as
    /// the arguments to [`from_raw_parts`].
    ///
    /// After calling this function, the caller is responsible for the
    /// memory previously managed by the `Fiber`. The only way to do
    /// this is to convert the raw pointer, length, and capacity back
    /// into a `Fiber` with the [`from_raw_parts`] function, allowing
    /// the destructor to perform the cleanup.
    ///
    /// [`from_raw_parts`]: Fiber::from_raw_parts
    ///
    /// # Examples
    ///
    /// ```
    /// let s = Fiber::from("hello");
    ///
    /// let (ptr, len, cap) = s.into_raw_parts();
    ///
    /// let rebuilt = unsafe { Fiber::from_raw_parts(ptr, len, cap) };
    /// assert_eq!(rebuilt, "hello");
    /// ```
    #[inline]
    pub fn into_raw_parts(self) -> (*mut u8, usize, usize) {
        self.vec.into_raw_parts()
    }

    #[inline]
    pub fn to_fiber_vec(vec: Vec<impl AsRef<str>>) -> Vec<Fiber> {
        if vec.is_empty() {
            return Vec::new();
        }
        let mut r = Vec::with_capacity(vec.len());
        for i in vec {
            r.push(Fiber::from_str(i.as_ref()));
        }
        r
    }
}
impl<A: Allocator> Fiber<A> {
    /// Constructs a new, empty `Fiber<A>`.
    ///
    /// The Fiber will not allocate until elements are pushed onto it.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::alloc::System;
    ///
    /// # #[allow(unused_mut)]
    /// let mut vec: Fiber<_> = Fiber::new_in(System);
    /// ```
    #[inline]
    pub const fn new_in(alloc: A) -> Fiber<A> {
        Fiber { vec: Vec::new_in(alloc) }
    }

    #[inline]
    pub fn from_str_in(v: &str, alloc: A) -> Fiber<A> {
        Fiber {
            vec: v.as_bytes().to_vec_in(alloc),
        }
    }
    #[inline]
    pub fn from_utf8_lossy_in(v: &[u8], alloc: A) -> Fiber<A> {
        let mut f = Fiber::new_in(alloc);
        utf8_to_lossy_insert(&mut f.vec, v);
        f
    }
    /// Decode a native endian UTF-16–encoded slice `v` into a `Fiber`,
    /// replacing invalid data with [the replacement character
    /// (`U+FFFD`)][U+FFFD].
    ///
    /// Unlike [`from_utf8_lossy`] which returns a [`Cow<'a, str>`],
    /// `from_utf16_lossy` returns a `Fiber` since the UTF-16 to UTF-8
    /// conversion requires a memory allocation.
    ///
    /// [`from_utf8_lossy`]: Fiber::from_utf8_lossy
    /// [`Cow<'a, str>`]: alloc::borrow::Cow "borrow::Cow"
    /// [U+FFFD]: core::char::REPLACEMENT_CHARACTER
    ///
    /// # Examples
    ///
    /// ```
    /// // 𝄞mus<invalid>ic<invalid>
    /// let v = &[0xD834, 0xDD1E, 0x006d, 0x0075,
    ///           0x0073, 0xDD1E, 0x0069, 0x0063,
    ///           0xD834];
    ///
    /// assert_eq!(Fiber::from("𝄞mus\u{FFFD}ic\u{FFFD}"),
    ///            Fiber::from_utf16_lossy(v));
    /// ```
    #[inline]
    pub fn from_utf16_lossy_in(v: &[u16], alloc: A) -> Fiber<A> {
        utf16_to_fiber_in(v, alloc)
    }
    /// Constructs a new, empty `Fiber` with at least the specified capacity
    /// with the provided allocator.
    ///
    /// The vector will be able to hold at least `capacity` elements without
    /// reallocating. This method is allowed to allocate for more elements than
    /// `capacity`. If `capacity` is zero, the vector will not allocate.
    ///
    /// It is important to note that although the returned vector has the
    /// minimum *capacity* specified, the vector will have a zero *length*. For
    /// an explanation of the difference between length and capacity, see
    /// *[Capacity and reallocation]*.
    ///
    /// If it is important to know the exact allocated capacity of a `Fiber`,
    /// always use the [`capacity`] method after construction.
    ///
    /// [Capacity and reallocation]: #capacity-and-reallocation
    /// [`capacity`]: Fiber::capacity
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` _bytes_.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::alloc::System;
    ///
    /// let mut vec = Fiber::with_capacity_in(10, System);
    ///
    /// // The vector contains no items, even though it has capacity for more
    /// assert_eq!(vec.len(), 0);
    /// assert!(vec.capacity() >= 10);
    ///
    /// // A vector of a zero-sized type will always over-allocate, since no
    /// // allocation is necessary
    /// let vec_units = Fiber::<System>::with_capacity_in(10, System);
    /// assert_eq!(vec_units.capacity(), usize::MAX);
    /// ```
    #[inline]
    pub fn with_capacity_in(capacity: usize, alloc: A) -> Fiber<A> {
        Fiber {
            vec: Vec::with_capacity_in(capacity, alloc),
        }
    }
    /// Converts a vector of bytes to a `Fiber`.
    ///
    /// A string ([`Fiber`]) is made of bytes ([`u8`]), and a vector of bytes
    /// ([`Fiber`]) is made of bytes, so this function converts between the
    /// two. Not all byte slices are valid `Fiber`s, however: `Fiber`
    /// requires that it is valid UTF-8. `from_utf8()` checks to ensure that
    /// the bytes are valid UTF-8, and then does the conversion.
    ///
    /// If you are sure that the byte slice is valid UTF-8, and you don't want
    /// to incur the overhead of the validity check, there is an unsafe version
    /// of this function, [`from_utf8_unchecked`], which has the same behavior
    /// but skips the check.
    ///
    /// This method will take care to not copy the vector, for efficiency's
    /// sake.
    ///
    /// If you need a [`&str`] instead of a `Fiber`, consider
    /// [`str::from_utf8`].
    ///
    /// The inverse of this method is [`into_bytes`].
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the slice is not UTF-8 with a description as to why
    /// the provided bytes are not UTF-8. The vector you moved in is also
    /// included.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// // some bytes, in a vector
    /// let sparkle_heart = vec![240, 159, 146, 150];
    ///
    /// // We know these bytes are valid, so we'll use `unwrap()`.
    /// let sparkle_heart = Fiber::from_utf8(sparkle_heart).unwrap();
    ///
    /// assert_eq!("💖", sparkle_heart);
    /// ```
    ///
    /// Incorrect bytes:
    ///
    /// ```
    /// // some invalid bytes, in a vector
    /// let sparkle_heart = vec![0, 159, 146, 150];
    ///
    /// assert!(Fiber::from_utf8(sparkle_heart).is_err());
    /// ```
    ///
    /// See the docs for [`FromUtf8Error`] for more details on what you can do
    /// with this error.
    ///
    /// [`from_utf8_unchecked`]: Fiber::from_utf8_unchecked
    /// [`Vec<u8>`]: alloc::vec::Vec "Vec"
    /// [`&str`]: prim@str "&str"
    /// [`into_bytes`]: Fiber::into_bytes
    #[inline]
    pub fn from_utf8(vec: Vec<u8, A>) -> Result<Fiber<A>, FromUtf8Error<A>> {
        match from_utf8(&vec) {
            Ok(..) => Ok(Fiber { vec }),
            Err(e) => Err(FromUtf8Error { bytes: vec, error: e }),
        }
    }
    /// Decode a native endian UTF-16–encoded vector `v` into a `Fiber`,
    /// returning [`Err`] if `v` contains any invalid data.
    ///
    /// # Examples
    ///
    /// ```
    /// // 𝄞music
    /// let v = &[0xD834, 0xDD1E, 0x006d, 0x0075,
    ///           0x0073, 0x0069, 0x0063];
    /// assert_eq!(Fiber::from("𝄞music"),
    ///            Fiber::from_utf16(v).unwrap());
    ///
    /// // 𝄞mu<invalid>ic
    /// let v = &[0xD834, 0xDD1E, 0x006d, 0x0075,
    ///           0xD800, 0x0069, 0x0063];
    /// assert!(Fiber::from_utf16(v).is_err());
    /// ```
    #[inline]
    pub fn from_utf16_in(v: &[u16], alloc: A) -> Result<Fiber<A>, FromUtf16Error> {
        let mut b = Fiber::new_in(alloc);
        for i in U16Decoder::new(v) {
            b.vec
                .extend_from_slice(CharSize::new_u32(i.map_err(FromUtf16Error)?).as_slice());
        }
        Ok(b)
    }

    #[inline]
    pub unsafe fn swap(dst: &mut Fiber<A>, src: &mut Fiber<A>) {
        swap(&mut src.vec, &mut dst.vec);
    }
    /// Converts a vector of bytes to a `Fiber` without checking that the
    /// string contains valid UTF-8.
    ///
    /// See the safe version, [`from_utf8`], for more details.
    ///
    /// [`from_utf8`]: Fiber::from_utf8
    ///
    /// # Safety
    ///
    /// This function is unsafe because it does not check that the bytes passed
    /// to it are valid UTF-8. If this constraint is violated, it may cause
    /// memory unsafety issues with future users of the `Fiber`, as the rest of
    /// the standard library assumes that `Fiber`s are valid UTF-8.
    ///
    /// # Examples
    ///
    /// ```
    /// // some bytes, in a vector
    /// let sparkle_heart = vec![240, 159, 146, 150];
    ///
    /// let sparkle_heart = unsafe {
    ///     Fiber::from_utf8_unchecked(sparkle_heart)
    /// };
    ///
    /// assert_eq!("💖", sparkle_heart);
    /// ```
    #[inline]
    pub unsafe fn from_utf8_unchecked(bytes: Vec<u8, A>) -> Fiber<A> {
        Fiber { vec: bytes }
    }
    /// Creates a `Fiber` directly from a pointer, a length, a capacity,
    /// and an allocator.
    ///
    /// # Safety
    ///
    /// This is highly unsafe, due to the number of invariants that aren't
    /// checked:
    ///
    /// * `ptr` must be [*currently allocated*] via the given allocator `alloc`.
    /// * `T` needs to have the same alignment as what `ptr` was allocated with.
    ///   (`T` having a less strict alignment is not sufficient, the alignment
    ///   really needs to be equal to satisfy the [`dealloc`] requirement that
    ///   memory must be allocated and deallocated with the same layout.)
    /// * The size of `T` times the `capacity` (ie. the allocated size in bytes)
    ///   needs to be the same size as the pointer was allocated with. (Because
    ///   similar to alignment, [`dealloc`] must be called with the same layout
    ///   `size`.)
    /// * `length` needs to be less than or equal to `capacity`.
    /// * The first `length` values must be properly initialized values of type
    ///   `T`.
    /// * `capacity` needs to [*fit*] the layout size that the pointer was
    ///   allocated with.
    /// * The allocated size in bytes must be no larger than `isize::MAX`. See
    ///   the safety documentation of `pointer::offset`.
    ///
    /// These requirements are always upheld by any `ptr` that has been
    /// allocated via `Fiber`. Other allocation sources are allowed if the
    /// invariants are upheld.
    ///
    /// Violating these may cause problems like corrupting the allocator's
    /// internal data structures. For example it is **not** safe
    /// to build a `Fiber` from a pointer to a C `char` array with length
    /// `size_t`.
    ///
    /// The ownership of `ptr` is effectively transferred to the
    /// `Fiber` which may then deallocate, reallocate or change the
    /// contents of memory pointed to by the pointer at will. Ensure
    /// that nothing else uses the pointer after calling this
    /// function.
    ///
    /// [`dealloc`]: alloc::alloc::GlobalAlloc::dealloc
    /// [*currently allocated*]: alloc::alloc::Allocator#currently-allocated-memory
    /// [*fit*]: alloc::alloc::Allocator#memory-fitting
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::alloc::System;
    ///
    /// use xrmt_stx::ptr;
    /// use xrmt_stx::mem;
    ///
    /// let mut v = Fiber::with_capacity_in(3, System);
    /// v.push("1");
    /// v.push("2");
    /// v.push("3");
    ///
    /// // Prevent running `v`'s destructor so we are in complete control
    /// // of the allocation.
    /// let mut v = mem::ManuallyDrop::new(v);
    ///
    /// // Pull out the various important pieces of information about `v`
    /// let p = v.as_mut_ptr();
    /// let len = v.len();
    /// let cap = v.capacity();
    /// let alloc = v.allocator();
    ///
    /// unsafe {
    ///     ptr::write(p, "testing");
    ///
    ///     // Put everything back together into a Fiber
    ///     let rebuilt = Fiber::from_raw_parts_in(p, len, cap, alloc.clone());
    /// }
    /// ```
    ///
    /// Using memory that was allocated elsewhere:
    ///
    /// ```rust
    /// use xrmt_stx::alloc::{AllocError, Allocator, Global, Layout};
    ///
    /// fn main() {
    ///     let layout = Layout::array::<u32>(16).expect("overflow cannot happen");
    ///
    ///     let vec = unsafe {
    ///         let mem = match Global.allocate(layout) {
    ///             Ok(mem) => mem.cast::<u32>().as_ptr(),
    ///             Err(AllocError) => return,
    ///         };
    ///
    ///         Fiber::from_raw_parts_in(mem, 1, 16, Global)
    ///     };
    ///
    ///     assert_eq!(vec.capacity(), 16);
    /// }
    /// ```
    #[inline]
    pub unsafe fn from_raw_parts_in(buf: *mut u8, length: usize, capacity: usize, alloc: A) -> Fiber<A> {
        unsafe {
            Fiber {
                vec: Vec::from_raw_parts_in(buf, length, capacity, alloc),
            }
        }
    }

    /// Truncates this `Fiber`, removing all contents.
    ///
    /// While this means the `Fiber` will have a length of zero, it does not
    /// touch its capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = Fiber::from("foo");
    ///
    /// s.clear();
    ///
    /// assert!(s.is_empty());
    /// assert_eq!(0, s.len());
    /// assert_eq!(3, s.capacity());
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.vec.clear()
    }
    /// Returns the length of this `Fiber`, in bytes, not [`char`]s or
    /// graphemes. In other words, it might not be what a human considers the
    /// length of the string.
    ///
    /// # Examples
    ///
    /// ```
    /// let a = Fiber::from("foo");
    /// assert_eq!(a.len(), 3);
    ///
    /// let fancy_f = Fiber::from("ƒoo");
    /// assert_eq!(fancy_f.len(), 4);
    /// assert_eq!(fancy_f.chars().count(), 3);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.vec.len()
    }
    /// Extracts a string slice containing the entire `Fiber`.
    ///
    /// # Examples
    ///
    /// ```
    /// let s = Fiber::from("foo");
    ///
    /// assert_eq!("foo", s.as_str());
    /// ```
    #[inline]
    pub fn as_str(&self) -> &str {
        self
    }
    /// Returns a reference to the underlying allocator.
    #[inline]
    pub fn allocator(&self) -> &A {
        self.vec.allocator()
    }
    /// Returns `true` if this `Fiber` has a length of zero, and `false`
    /// otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut v = Fiber::new();
    /// assert!(v.is_empty());
    ///
    /// v.push('a');
    /// assert!(!v.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }
    /// Returns a slice of [`u8`]s bytes that were attempted to convert to a
    /// `Fiber`.
    ///
    /// # Examples
    ///
    /// ```
    /// // some invalid bytes, in a vector
    /// let bytes = vec![0, 159];
    ///
    /// let value = Fiber::from_utf8(bytes);
    ///
    /// assert_eq!(&[0, 159], value.unwrap_err().as_bytes());
    /// ```
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.vec
    }
    /// Shrinks the capacity of this `Fiber` to match its length.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = Fiber::from("foo");
    ///
    /// s.reserve(100);
    /// assert!(s.capacity() >= 100);
    ///
    /// s.shrink_to_fit();
    /// assert_eq!(3, s.capacity());
    /// ```
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.vec.shrink_to_fit()
    }
    /// Returns this `Fiber`'s capacity, in bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// let s = Fiber::with_capacity(10);
    ///
    /// assert!(s.capacity() >= 10);
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.vec.capacity()
    }
    /// Appends the given [`char`] to the end of this `Fiber`.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = Fiber::from("abc");
    ///
    /// s.push('1');
    /// s.push('2');
    /// s.push('3');
    ///
    /// assert_eq!("abc123", s);
    /// ```
    #[inline]
    pub fn push(&mut self, ch: char) {
        self.vec.extend_from_slice(CharSize::new(ch).as_slice());
    }
    /// Appends a given string slice onto the end of this `Fiber`.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = Fiber::from("foo");
    ///
    /// s.push_str("bar");
    ///
    /// assert_eq!("foobar", s);
    /// ```
    #[inline]
    pub fn push_str(&mut self, v: &str) {
        self.vec.extend_from_slice(v.as_bytes())
    }
    /// Reserves capacity for at least `additional` bytes more than the
    /// current length. The allocator may reserve more space to speculatively
    /// avoid frequent allocations. After calling `reserve`,
    /// capacity will be greater than or equal to `self.len() + additional`.
    /// Does nothing if capacity is already sufficient.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows [`usize`].
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// let mut s = Fiber::new();
    ///
    /// s.reserve(10);
    ///
    /// assert!(s.capacity() >= 10);
    /// ```
    ///
    /// This might not actually increase the capacity:
    ///
    /// ```
    /// let mut s = Fiber::with_capacity(10);
    /// s.push('a');
    /// s.push('b');
    ///
    /// // s now has a length of 2 and a capacity of at least 10
    /// let capacity = s.capacity();
    /// assert_eq!(2, s.len());
    /// assert!(capacity >= 10);
    ///
    /// // Since we already have at least an extra 8 capacity, calling this...
    /// s.reserve(8);
    ///
    /// // ... doesn't actually increase.
    /// assert_eq!(capacity, s.capacity());
    /// ```
    #[inline]
    pub fn reserve(&mut self, len: usize) {
        self.vec.reserve(len)
    }
    /// Converts a `Fiber` into a byte vector.
    ///
    /// This consumes the `Fiber`, so we do not need to copy its contents.
    ///
    /// # Examples
    ///
    /// ```
    /// let s = String::from("hello");
    /// let bytes = s.into_bytes();
    ///
    /// assert_eq!(&[104, 101, 108, 108, 111][..], &bytes[..]);
    /// ```
    #[inline]
    pub fn into_bytes(self) -> Vec<u8, A> {
        self.vec
    }
    /// Removes the last character from the string buffer and returns it.
    ///
    /// Returns [`None`] if this `Fiber` is empty.
    ///
    /// [`None`]: core::option::Option
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = String::from("abč");
    ///
    /// assert_eq!(s.pop(), Some('č'));
    /// assert_eq!(s.pop(), Some('b'));
    /// assert_eq!(s.pop(), Some('a'));
    ///
    /// assert_eq!(s.pop(), None);
    /// ```
    #[inline]
    pub fn pop(&mut self) -> Option<char> {
        let c = self.chars().rev().next()?;
        let n = self.len() - c.len_utf8();
        unsafe { self.vec.set_len(n) };
        Some(c)
    }
    /// Shortens this `Fiber` to the specified length.
    ///
    /// If `new_len` is greater than or equal to the string's current length,
    /// this has no effect.
    ///
    /// Note that this method has no effect on the allocated capacity
    /// of the string
    ///
    /// # Panics
    ///
    /// Panics if `new_len` does not lie on a [`char`] boundary.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = String::from("hello");
    ///
    /// s.truncate(2);
    ///
    /// assert_eq!("he", s);
    /// ```
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        self.vec.truncate(len)
    }
    /// Shrinks the capacity of this `Fiber` with a lower bound.
    ///
    /// The capacity will remain at least as large as both the length
    /// and the supplied value.
    ///
    /// If the current capacity is less than the lower limit, this is a no-op.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = String::from("foo");
    ///
    /// s.reserve(100);
    /// assert!(s.capacity() >= 100);
    ///
    /// s.shrink_to(10);
    /// assert!(s.capacity() >= 10);
    /// s.shrink_to(0);
    /// assert!(s.capacity() >= 3);
    /// ```
    #[inline]
    pub fn shrink_to(&mut self, len: usize) {
        self.vec.shrink_to(len)
    }
    /// Converts a `Fiber` into a mutable string slice.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = String::from("foobar");
    /// let s_mut_str = s.as_mut_str();
    ///
    /// s_mut_str.make_ascii_uppercase();
    ///
    /// assert_eq!("FOOBAR", s_mut_str);
    /// ```
    #[inline]
    pub fn as_mut_str(&mut self) -> &mut str {
        self
    }
    /// Converts this `Fiber` into a <code>[Box]<[str]></code>.
    ///
    /// Before doing the conversion, this method discards excess capacity like
    /// [`shrink_to_fit`]. Note that this call may reallocate and copy the
    /// bytes of the string.
    ///
    /// [`shrink_to_fit`]: Fiber::shrink_to_fit
    /// [str]: prim@str "str"
    ///
    /// # Examples
    ///
    /// ```
    /// let s = Fiber::from("hello");
    ///
    /// let b = s.into_boxed_str();
    /// ```
    #[inline]
    pub fn into_boxed_str(self) -> Box<str, A> {
        let (v, a) = Box::into_raw_with_allocator(self.vec.into_boxed_slice());
        unsafe { Box::from_raw_in(v as *mut str, a) }
    }
    // Reserves the minimum capacity for at least `additional` bytes more than
    /// the current length. Unlike [`reserve`], this will not
    /// deliberately over-allocate to speculatively avoid frequent allocations.
    /// After calling `reserve_exact`, capacity will be greater than or equal to
    /// `self.len() + additional`. Does nothing if the capacity is already
    /// sufficient.
    ///
    /// [`reserve`]: Fiber::reserve
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows [`usize`].
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// let mut s = String::new();
    ///
    /// s.reserve_exact(10);
    ///
    /// assert!(s.capacity() >= 10);
    /// ```
    ///
    /// This might not actually increase the capacity:
    ///
    /// ```
    /// let mut s = String::with_capacity(10);
    /// s.push('a');
    /// s.push('b');
    ///
    /// // s now has a length of 2 and a capacity of at least 10
    /// let capacity = s.capacity();
    /// assert_eq!(2, s.len());
    /// assert!(capacity >= 10);
    ///
    /// // Since we already have at least an extra 8 capacity, calling this...
    /// s.reserve_exact(8);
    ///
    /// // ... doesn't actually increase.
    /// assert_eq!(capacity, s.capacity());
    /// ```
    #[inline]
    pub fn reserve_exact(&mut self, len: usize) {
        self.vec.reserve_exact(len)
    }
    /// Inserts a character into this `Fiber` at a byte position.
    ///
    /// This is an *O*(*n*) operation as it requires copying every element in
    /// the buffer.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is larger than the `Fiber`'s length, or if it does not
    /// lie on a [`char`] boundary.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = String::with_capacity(3);
    ///
    /// s.insert(0, 'f');
    /// s.insert(1, 'o');
    /// s.insert(2, 'o');
    ///
    /// assert_eq!("foo", s);
    /// ```
    #[inline]
    pub fn insert(&mut self, idx: usize, ch: char) {
        self.insert_bytes(idx, CharSize::new(ch).as_slice())
    }
    /// Removes a [`char`] from this `Fiber` at a byte position and returns it.
    ///
    /// This is an *O*(*n*) operation, as it requires copying every element in
    /// the buffer.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is larger than or equal to the `Fiber`'s length,
    /// or if it does not lie on a [`char`] boundary.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = Fiber::from("abç");
    ///
    /// assert_eq!(s.remove(0), 'a');
    /// assert_eq!(s.remove(1), 'ç');
    /// assert_eq!(s.remove(0), 'b');
    /// ```
    #[inline]
    pub fn remove(&mut self, idx: usize) -> char {
        self.vec.remove(idx) as char
    }
    /// Inserts a string slice into this `Fiber` at a byte position.
    ///
    /// This is an *O*(*n*) operation as it requires copying every element in
    /// the buffer.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is larger than the `Fiber`'s length, or if it does not
    /// lie on a [`char`] boundary.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = String::from("bar");
    ///
    /// s.insert_str(0, "foo");
    ///
    /// assert_eq!("foobar", s);
    /// ```
    #[inline]
    pub fn insert_str(&mut self, idx: usize, string: &str) {
        self.insert_bytes(idx, string.as_bytes())
    }
    /// Retains only the characters specified by the predicate.
    ///
    /// In other words, remove all characters `c` such that `f(c)` returns
    /// `false`. This method operates in place, visiting each character
    /// exactly once in the original order, and preserves the order of the
    /// retained characters.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = Fiber::from("f_o_ob_ar");
    ///
    /// s.retain(|c| c != '_');
    ///
    /// assert_eq!(s, "foobar");
    /// ```
    ///
    /// Because the elements are visited exactly once in the original order,
    /// external state may be used to decide which elements to keep.
    ///
    /// ```
    /// let mut s = Fiber::from("abcde");
    /// let keep = [false, true, true, false, true];
    /// let mut iter = keep.iter();
    /// s.retain(|_| *iter.next().unwrap());
    /// assert_eq!(s, "bce");
    /// ```
    #[inline]
    pub fn retain(&mut self, mut f: impl FnMut(char) -> bool) {
        self.vec.retain(|v| f(*v as char));
    }
    /// Copies elements from `src` range to the end of the string.
    ///
    /// # Panics
    ///
    /// Panics if the starting point or end point do not lie on a [`char`]
    /// boundary, or if they're out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut string = String::from("abcde");
    ///
    /// string.extend_from_within(2..);
    /// assert_eq!(string, "abcdecde");
    ///
    /// string.extend_from_within(..2);
    /// assert_eq!(string, "abcdecdeab");
    ///
    /// string.extend_from_within(4..8);
    /// assert_eq!(string, "abcdecdeabecde");
    /// ```
    #[inline]
    pub fn extend_from_within(&mut self, src: impl RangeBounds<usize>) {
        self.vec.extend_from_within(src);
    }
    /// Decomposes a `Fiber` into its raw components: `(pointer, length,
    /// capacity, allocator)`.
    ///
    /// Returns the raw pointer to the underlying data, the length of the vector
    /// (in elements), the allocated capacity of the data (in elements), and
    /// the allocator. These are the same arguments in the same order as the
    /// arguments to [`from_raw_parts_in`].
    ///
    /// After calling this function, the caller is responsible for the
    /// memory previously managed by the `Vec`. The only way to do
    /// this is to convert the raw pointer, length, and capacity back
    /// into a `Fiber` with the [`from_raw_parts_in`] function, allowing
    /// the destructor to perform the cleanup.
    ///
    /// [`from_raw_parts_in`]: Fiber::from_raw_parts_in
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::alloc::System;
    ///
    /// let mut v: Fiber<System> = Fiber::new_in(System);
    /// v.push_str("-1");
    /// v.push_str("0");
    /// v.push_str("1");
    ///
    /// let (ptr, len, cap, alloc) = v.into_raw_parts_with_alloc();
    ///
    /// let rebuilt = unsafe {
    ///     // We can now make changes to the components, such as
    ///     // transmuting the raw pointer to a compatible type.
    ///     let ptr = ptr as *mut u32;
    ///
    ///     Fiber::from_raw_parts_in(ptr, len, cap, alloc)
    /// };
    /// ```
    #[inline]
    pub fn into_raw_parts_with_alloc(self) -> (*mut u8, usize, usize, A) {
        self.vec.into_raw_parts_with_alloc()
    }
    /// Removes the specified range from the string in bulk, returning all
    /// removed characters as an iterator.
    ///
    /// The returned iterator keeps a mutable borrow on the string to optimize
    /// its implementation.
    ///
    /// # Panics
    ///
    /// Panics if the starting point or end point do not lie on a [`char`]
    /// boundary, or if they're out of bounds.
    ///
    /// # Leaking
    ///
    /// If the returned iterator goes out of scope without being dropped (due to
    /// [`forget`], for example), the string may still contain a copy
    /// of any drained characters, or may have lost characters arbitrarily,
    /// including characters outside the range.
    ///
    /// [`forget`]: core::mem::forget
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = Fiber::from("α is alpha, β is beta");
    /// let beta_offset = s.find('β').unwrap_or(s.len());
    ///
    /// // Remove the range up until the β from the string
    /// let t: Fiber = s.drain(..beta_offset).collect();
    /// assert_eq!(t, "α is alpha, ");
    /// assert_eq!(s, "β is beta");
    ///
    /// // A full range clears the string, like `clear()` does
    /// s.drain(..);
    /// assert_eq!(s, "");
    /// ```
    #[inline]
    pub fn drain(&mut self, range: impl RangeBounds<usize>) -> Drain<'_, A> {
        Drain(self.vec.drain(range))
    }
    /// Tries to reserve capacity for at least `additional` bytes more than the
    /// current length. The allocator may reserve more space to speculatively
    /// avoid frequent allocations. After calling `try_reserve`, capacity will
    /// be greater than or equal to `self.len() + additional` if it returns
    /// `Ok(())`. Does nothing if capacity is already sufficient. This method
    /// preserves the contents even if an error occurs.
    ///
    /// # Errors
    ///
    /// If the capacity overflows, or the allocator reports a failure, then an
    /// error is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::collections::TryReserveError;
    ///
    /// fn process_data(data: &str) -> Result<String, TryReserveError> {
    ///     let mut output = String::new();
    ///
    ///     // Pre-reserve the memory, exiting if we can't
    ///     output.try_reserve(data.len())?;
    ///
    ///     // Now we know this can't OOM in the middle of our complex work
    ///     output.push_str(data);
    ///
    ///     Ok(output)
    /// }
    /// ```
    #[inline]
    pub fn try_reserve(&mut self, len: usize) -> Result<(), TryReserveError> {
        self.vec.try_reserve(len)
    }
    /// Tries to reserve the minimum capacity for at least `additional` bytes
    /// more than the current length. Unlike [`try_reserve`], this will not
    /// deliberately over-allocate to speculatively avoid frequent allocations.
    /// After calling `try_reserve_exact`, capacity will be greater than or
    /// equal to `self.len() + additional` if it returns `Ok(())`.
    /// Does nothing if the capacity is already sufficient.
    ///
    /// Note that the allocator may give the collection more space than it
    /// requests. Therefore, capacity can not be relied upon to be precisely
    /// minimal. Prefer [`try_reserve`] if future insertions are expected.
    ///
    /// [`try_reserve`]: Fiber::try_reserve
    ///
    /// # Errors
    ///
    /// If the capacity overflows, or the allocator reports a failure, then an
    /// error is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::collections::TryReserveError;
    ///
    /// fn process_data(data: &str) -> Result<String, TryReserveError> {
    ///     let mut output = String::new();
    ///
    ///     // Pre-reserve the memory, exiting if we can't
    ///     output.try_reserve_exact(data.len())?;
    ///
    ///     // Now we know this can't OOM in the middle of our complex work
    ///     output.push_str(data);
    ///
    ///     Ok(output)
    /// }
    /// ```
    #[inline]
    pub fn try_reserve_exact(&mut self, len: usize) -> Result<(), TryReserveError> {
        self.vec.try_reserve_exact(len)
    }
    /// Removes the specified range in the string,
    /// and replaces it with the given string.
    /// The given string doesn't need to be the same length as the range.
    ///
    /// # Panics
    ///
    /// Panics if the starting point or end point do not lie on a [`char`]
    /// boundary, or if they're out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = Fiber::from("α is alpha, β is beta");
    /// let beta_offset = s.find('β').unwrap_or(s.len());
    ///
    /// // Replace the range up until the β from the string
    /// s.replace_range(..beta_offset, "Α is capital alpha; ");
    /// assert_eq!(s, "Α is capital alpha; β is beta");
    /// ```
    #[inline]
    pub fn replace_range(&mut self, range: impl RangeBounds<usize>, replace_with: &str) {
        drop(self.vec.splice(range, replace_with.bytes()));
    }

    #[inline]
    pub unsafe fn into_string(self) -> String {
        // Zero allocation, just transfer the vec.
        // This might fail horribly...
        //  let (a, b, c, _) = self.vec.into_raw_parts_with_alloc();
        //  unsafe { String::from_raw_parts(a, b, c) }
        let mut v = String::new();
        unsafe { v.as_mut_vec().extend_from_slice(&self.vec) };
        v
    }
    /// Returns a mutable reference to the contents of this `Fiber`.
    ///
    /// # Safety
    ///
    /// This function is unsafe because the returned `&mut Vec` allows writing
    /// bytes which are not valid UTF-8. If this constraint is violated, using
    /// the original `Fiber` after dropping the `&mut Vec` may violate memory
    /// safety, as the rest of the standard library assumes that `Fiber`s are
    /// valid UTF-8.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = String::from("hello");
    ///
    /// unsafe {
    ///     let vec = s.as_mut_vec();
    ///     assert_eq!(&[104, 101, 108, 108, 111][..], &vec[..]);
    ///
    ///     vec.reverse();
    /// }
    /// assert_eq!(s, "olleh");
    /// ```
    #[inline]
    pub unsafe fn as_mut_vec(&mut self) -> &mut Vec<u8, A> {
        &mut self.vec
    }

    fn insert_bytes(&mut self, idx: usize, bytes: &[u8]) {
        let (n, c) = (self.len(), bytes.len());
        self.vec.reserve(c);
        unsafe {
            copy(
                self.vec.as_ptr().add(idx),
                self.vec.as_mut_ptr().add(idx + c),
                n - idx,
            );
            copy_nonoverlapping(bytes.as_ptr(), self.vec.as_mut_ptr().add(idx), c);
            self.vec.set_len(n + c);
        }
    }
}
impl<A: Allocator + Clone> Fiber<A> {
    /// Returns a clone of the underlying allocator.
    #[inline]
    pub fn allocator_clone(&self) -> A {
        self.vec.allocator().clone()
    }
    /// Splits the string into two at the given byte index.
    ///
    /// Returns a newly allocated `Fiber`. `self` contains bytes `[0, at)`, and
    /// the returned `Fiber` contains bytes `[at, len)`. `at` must be on the
    /// boundary of a UTF-8 code point.
    ///
    /// Note that the capacity of `self` does not change.
    ///
    /// # Panics
    ///
    /// Panics if `at` is not on a `UTF-8` code point boundary, or if it is
    /// beyond the last code point of the string.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() {
    /// let mut hello = Fiber::from("Hello, World!");
    /// let world = hello.split_off(7);
    /// assert_eq!(hello, "Hello, ");
    /// assert_eq!(world, "World!");
    /// # }
    /// ```
    #[inline]
    pub fn split_off(&mut self, at: usize) -> Fiber<A> {
        unsafe { Fiber::from_utf8_unchecked(self.vec.split_off(at)) }
    }

    #[inline]
    pub fn to_fiber_vec_in<B: Allocator>(vec: Vec<impl AsRef<str>, B>, alloc: A) -> Vec<Fiber<A>, A> {
        if vec.is_empty() {
            return Vec::new_in(alloc);
        }
        let mut r = Vec::with_capacity_in(vec.len(), alloc.clone());
        for i in vec {
            r.push(i.as_ref().into_alloc(alloc.clone()))
        }
        r
    }
}
impl<A: Allocator> FromUtf8Error<A> {
    /// Returns a slice of [`u8`]s bytes that were attempted to convert to a
    /// `Fiber`.
    ///
    /// # Examples
    ///
    /// ```
    /// // some invalid bytes, in a vector
    /// let bytes = vec![0, 159];
    ///
    /// let value = Fiber::from_utf8(bytes);
    ///
    /// assert_eq!(&[0, 159], value.unwrap_err().as_bytes());
    /// ```
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
    /// Returns the bytes that were attempted to convert to a `Fiber`.
    ///
    /// This method is carefully constructed to avoid allocation. It will
    /// consume the error, moving out the bytes, so that a copy of the bytes
    /// does not need to be made.
    ///
    /// # Examples
    ///
    /// ```
    /// // some invalid bytes, in a vector
    /// let bytes = vec![0, 159];
    ///
    /// let value = Fiber::from_utf8(bytes);
    ///
    /// assert_eq!(vec![0, 159], value.unwrap_err().into_bytes());
    /// ```
    #[inline]
    pub fn into_bytes(self) -> Vec<u8, A> {
        self.bytes
    }
    /// Fetch a `Utf8Error` to get more details about the conversion failure.
    ///
    /// The [`Utf8Error`] type provided by [`core::str`] represents an error
    /// that may occur when converting a slice of [`u8`]s to a [`&str`]. In
    /// this sense, it's an analogue to `FromUtf8Error`. See its
    /// documentation for more details on using it.
    ///
    /// [`core::str`]: core::str "core::str"
    /// [`&str`]: prim@str "&str"
    ///
    /// # Examples
    ///
    /// ```
    /// // some invalid bytes, in a vector
    /// let bytes = vec![0, 159];
    ///
    /// let error = Fiber::from_utf8(bytes).unwrap_err().utf8_error();
    ///
    /// // the first byte is invalid here
    /// assert_eq!(1, error.valid_up_to());
    /// ```
    #[inline]
    pub fn utf8_error(&self) -> Utf8Error {
        self.error
    }
    /// Converts the bytes into a `Fiber` lossily, substituting invalid UTF-8
    /// sequences with replacement characters.
    ///
    /// See [`Fiber::from_utf8_lossy`] for more details on replacement of
    /// invalid sequences, and [`Fiber::from_utf8_lossy_owned`] for the
    /// `Fiber` function which corresponds to this function.
    ///
    /// # Examples
    ///
    /// ```
    /// // some invalid bytes
    /// let input: Vec<u8> = b"Hello \xF0\x90\x80World".into();
    /// let output = String::from_utf8(input).unwrap_or_else(|e| e.into_utf8_lossy());
    ///
    /// assert_eq!(String::from("Hello �World"), output);
    /// ```
    #[inline]
    pub fn into_utf8_lossy(self) -> Fiber<A> {
        let mut b = self.bytes;
        utf8_to_lossy_rewrite(&mut b);
        Fiber { vec: b }
    }
}
impl<'a, A: Allocator> Drain<'a, A> {
    /// Returns the remaining (sub)string of this iterator as a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = String::from("abc");
    /// let mut drain = s.drain(..);
    /// assert_eq!(drain.as_str(), "abc");
    /// let _ = drain.next().unwrap();
    /// assert_eq!(drain.as_str(), "bc");
    /// ```
    #[inline]
    pub fn as_str(&self) -> &str {
        unsafe { from_utf8_unchecked(self.0.as_slice()) }
    }
    /// Returns a reference to the underlying allocator.
    #[inline]
    pub fn allocator(&self) -> &A {
        self.0.allocator()
    }
}
impl<'a, A: Allocator + 'a> Fiber<A> {
    /// Consumes and leaks the `Fiber`, returning a mutable reference to the
    /// contents, `&'a mut str`.
    ///
    /// The caller has free choice over the returned lifetime, including
    /// `'static`. Indeed, this function is ideally used for data that lives
    /// for the remainder of the program's life, as dropping the returned
    /// reference will cause a memory leak.
    ///
    /// It does not reallocate or shrink the `Fiber`, so the leaked allocation
    /// may include unused capacity that is not part of the returned slice.
    /// If you want to discard excess capacity, call [`into_boxed_str`], and
    /// then [`Box::leak`] instead. However, keep in mind that trimming the
    /// capacity may result in a reallocation and copy.
    ///
    /// [`into_boxed_str`]: Fiber::into_boxed_str
    ///
    /// # Examples
    ///
    /// ```
    /// let x = Fiber::from("bucket");
    /// let static_ref: &'static mut str = x.leak();
    /// assert_eq!(static_ref, "bucket");
    /// ```
    #[inline]
    pub fn leak(self) -> &'a mut str {
        unsafe { from_utf8_unchecked_mut(self.vec.leak()) }
    }
}
impl<'a, A: Allocator + Clone> Drain<'a, A> {
    /// Returns a clone of the underlying allocator.
    #[inline]
    pub fn allocator_clone(&self) -> A {
        self.0.allocator().clone()
    }
}

impl Default for Fiber {
    #[inline]
    fn default() -> Fiber {
        Fiber::new()
    }
}
impl FromStr for Fiber {
    type Err = Infallible;

    #[inline]
    fn from_str(v: &str) -> Result<Fiber, Infallible> {
        Ok(Fiber::from_str(v))
    }
}

impl<T: AsRef<[u8]>> ToFiber for T {
    #[inline]
    fn to_fiber(&self) -> Fiber {
        Fiber::from_utf8_lossy(self.as_ref())
    }
}

impl MaybeString for &str {
    #[inline]
    fn as_maybe(&self) -> Option<&str> {
        Some(self)
    }
}
impl MaybeString for String {
    #[inline]
    fn as_maybe(&self) -> Option<&str> {
        Some(self)
    }
}
impl MaybeString for &String {
    #[inline]
    fn as_maybe(&self) -> Option<&str> {
        Some(self)
    }
}
impl MaybeString for Option<&str> {
    #[inline]
    fn as_maybe(&self) -> Option<&str> {
        *self
    }
}
impl MaybeString for Cow<'_, str> {
    #[inline]
    fn as_maybe(&self) -> Option<&str> {
        Some(self)
    }
}

impl<A: Allocator> MaybeString for Fiber<A> {
    #[inline]
    fn as_maybe(&self) -> Option<&str> {
        Some(self)
    }
}
impl<A: Allocator> MaybeString for &Fiber<A> {
    #[inline]
    fn as_maybe(&self) -> Option<&str> {
        Some(self)
    }
}
impl<A: Allocator> MaybeString for Option<Fiber<A>> {
    #[inline]
    fn as_maybe(&self) -> Option<&str> {
        self.as_deref()
    }
}

impl<A: Allocator> Eq for Fiber<A> {}
impl<A: Allocator> Ord for Fiber<A> {
    #[inline]
    fn cmp(&self, other: &Fiber<A>) -> Ordering {
        self.vec.cmp(&other.vec)
    }
}
impl<A: Allocator> Hash for Fiber<A> {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.vec.hash(h);
    }
}
impl<A: Allocator> Deref for Fiber<A> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        unsafe { transmute(self.vec.as_slice()) }
    }
}
impl<A: Allocator> Write for Fiber<A> {
    #[inline]
    fn write_str(&mut self, v: &str) -> FmtResult {
        self.push_str(v);
        Ok(())
    }
    #[inline]
    fn write_char(&mut self, c: char) -> FmtResult {
        self.push(c);
        Ok(())
    }
}
impl<A: Allocator> Debug for Fiber<A> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(self.as_str())
    }
}
impl<A: Allocator> Display for Fiber<A> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self.as_str(), f)
    }
}
impl<A: Allocator> DerefMut for Fiber<A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut str {
        unsafe { from_utf8_unchecked_mut(&mut self.vec) }
    }
}
impl<A: Allocator> PartialOrd for Fiber<A> {
    #[inline]
    fn partial_cmp(&self, other: &Fiber<A>) -> Option<Ordering> {
        self.vec.partial_cmp(&other.vec)
    }
}
impl<A: Allocator + Clone> Clone for Fiber<A> {
    #[inline]
    fn clone(&self) -> Fiber<A> {
        Fiber { vec: self.vec.clone() }
    }
    #[inline]
    fn clone_from(&mut self, v: &Fiber<A>) {
        self.vec.clone_from(&v.vec);
    }
}

impl<A: Allocator> Add<&str> for Fiber<A> {
    type Output = Fiber<A>;

    #[inline]
    fn add(mut self, other: &str) -> Fiber<A> {
        self.push_str(other);
        self
    }
}
impl<A: Allocator> AddAssign<&str> for Fiber<A> {
    #[inline]
    fn add_assign(&mut self, other: &str) {
        self.push_str(other);
    }
}
impl<A: Allocator, T: Allocator> Add<&Fiber<T>> for Fiber<A> {
    type Output = Fiber<A>;

    #[inline]
    fn add(mut self, other: &Fiber<T>) -> Fiber<A> {
        self.push_str(other);
        self
    }
}

impl<A: Allocator> AsMut<str> for Fiber<A> {
    #[inline]
    fn as_mut(&mut self) -> &mut str {
        self
    }
}
impl<A: Allocator> AsRef<str> for Fiber<A> {
    #[inline]
    fn as_ref(&self) -> &str {
        self
    }
}
impl<A: Allocator> AsRef<[u8]> for Fiber<A> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}
impl<A: Allocator> Borrow<str> for Fiber<A> {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
impl<A: Allocator> BorrowMut<str> for Fiber<A> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut str {
        self.as_mut_str()
    }
}

impl<A: Allocator> Extend<u8> for Fiber<A> {
    #[inline]
    fn extend<I: IntoIterator<Item = u8>>(&mut self, i: I) {
        let x = i.into_iter();
        match x.size_hint() {
            (_, Some(v)) => self.reserve(v),
            (v, _) => self.reserve(v),
        }
        for v in x {
            self.vec.push(v)
        }
    }
}
impl<A: Allocator> Extend<char> for Fiber<A> {
    #[inline]
    fn extend<I: IntoIterator<Item = char>>(&mut self, i: I) {
        let x = i.into_iter();
        match x.size_hint() {
            (_, Some(v)) => self.reserve(v),
            (v, _) => self.reserve(v),
        }
        for v in x {
            self.push(v)
        }
    }
}
impl<A: Allocator> Extend<Char> for Fiber<A> {
    #[inline]
    fn extend<I: IntoIterator<Item = Char>>(&mut self, i: I) {
        let x = i.into_iter();
        match x.size_hint() {
            (_, Some(v)) => self.reserve(v),
            (v, _) => self.reserve(v),
        }
        for i in x {
            self.vec.push(i as u8);
        }
    }
}
impl<A: Allocator> Extend<String> for Fiber<A> {
    #[inline]
    fn extend<I: IntoIterator<Item = String>>(&mut self, i: I) {
        let x = i.into_iter();
        match x.size_hint() {
            (_, Some(v)) => self.reserve(v),
            (v, _) => self.reserve(v),
        }
        for v in x {
            self.push_str(&v)
        }
    }
}
impl<'a, A: Allocator> Extend<&'a str> for Fiber<A> {
    #[inline]
    fn extend<I: IntoIterator<Item = &'a str>>(&mut self, i: I) {
        let x = i.into_iter();
        match x.size_hint() {
            (_, Some(v)) => self.reserve(v),
            (v, _) => self.reserve(v),
        }
        for i in x {
            self.push_str(i);
        }
    }
}
impl<'a, A: Allocator> Extend<&'a char> for Fiber<A> {
    #[inline]
    fn extend<I: IntoIterator<Item = &'a char>>(&mut self, i: I) {
        let x = i.into_iter();
        match x.size_hint() {
            (_, Some(v)) => self.reserve(v),
            (v, _) => self.reserve(v),
        }
        for i in x {
            self.push(*i);
        }
    }
}
impl<'a, A: Allocator> Extend<&'a Char> for Fiber<A> {
    #[inline]
    fn extend<I: IntoIterator<Item = &'a Char>>(&mut self, i: I) {
        let x = i.into_iter();
        match x.size_hint() {
            (_, Some(v)) => self.reserve(v),
            (v, _) => self.reserve(v),
        }
        for i in x {
            self.vec.push(*i as u8);
        }
    }
}
impl<'a, A: Allocator> Extend<Cow<'a, str>> for Fiber<A> {
    #[inline]
    fn extend<I: IntoIterator<Item = Cow<'a, str>>>(&mut self, i: I) {
        let x = i.into_iter();
        match x.size_hint() {
            (_, Some(v)) => self.reserve(v),
            (v, _) => self.reserve(v),
        }
        for i in x {
            self.push_str(&i);
        }
    }
}
impl<A: Allocator, T: Allocator> Extend<Fiber<T>> for Fiber<A> {
    #[inline]
    fn extend<I: IntoIterator<Item = Fiber<T>>>(&mut self, i: I) {
        let x = i.into_iter();
        match x.size_hint() {
            (_, Some(v)) => self.reserve(v),
            (v, _) => self.reserve(v),
        }
        for i in x {
            self.push_str(i.as_str());
        }
    }
}
impl<A: Allocator, T: Allocator> Extend<Box<str, T>> for Fiber<A> {
    #[inline]
    fn extend<I: IntoIterator<Item = Box<str, T>>>(&mut self, i: I) {
        let x = i.into_iter();
        match x.size_hint() {
            (_, Some(v)) => self.reserve(v),
            (v, _) => self.reserve(v),
        }
        for i in x {
            self.push_str(&i);
        }
    }
}

impl From<&str> for Fiber {
    #[inline]
    fn from(v: &str) -> Fiber {
        Fiber::from_str(v)
    }
}
impl From<char> for Fiber {
    #[inline]
    fn from(v: char) -> Fiber {
        let mut r = Fiber::new();
        r.push(v);
        r
    }
}
impl From<String> for Fiber {
    #[inline]
    fn from(v: String) -> Fiber {
        Fiber { vec: v.into_bytes() }
    }
}
impl From<&String> for Fiber {
    #[inline]
    fn from(v: &String) -> Fiber {
        Fiber::from_str(v)
    }
}
impl From<&mut str> for Fiber {
    #[inline]
    fn from(v: &mut str) -> Fiber {
        Fiber::from_str(v)
    }
}
impl From<&ByteStr> for Fiber {
    #[inline]
    fn from(v: &ByteStr) -> Fiber {
        Fiber { vec: v.to_vec() }
    }
}
impl From<ByteString> for Fiber {
    #[inline]
    fn from(v: ByteString) -> Fiber {
        Fiber { vec: v.to_vec() }
    }
}
impl<'a> From<Cow<'a, str>> for Fiber {
    #[inline]
    fn from(v: Cow<'a, str>) -> Fiber {
        match v {
            Cow::Owned(i) => Fiber { vec: i.into_bytes() },
            Cow::Borrowed(i) => Fiber::from_str(i),
        }
    }
}

impl<A: Allocator> From<Vec<u8, A>> for Fiber<A> {
    #[inline]
    fn from(v: Vec<u8, A>) -> Fiber<A> {
        Fiber { vec: v }
    }
}
impl<A: Allocator> From<Box<str, A>> for Fiber<A> {
    #[inline]
    fn from(v: Box<str, A>) -> Fiber<A> {
        Fiber {
            vec: Box::<[u8], A>::from(v).into_vec(),
        }
    }
}
impl<A: Allocator + Clone> From<&Fiber<A>> for Fiber<A> {
    #[inline]
    fn from(v: &Fiber<A>) -> Fiber<A> {
        v.clone()
    }
}

impl From<Fiber> for String {
    #[inline]
    fn from(v: Fiber) -> String {
        unsafe { String::from_utf8_unchecked(v.vec) }
    }
}
impl From<Fiber> for Box<str> {
    #[inline]
    fn from(v: Fiber) -> Box<str> {
        v.into_boxed_str()
    }
}
impl From<Fiber> for Box<[u8]> {
    #[inline]
    fn from(v: Fiber) -> Box<[u8]> {
        v.vec.into_boxed_slice()
    }
}

impl<A: Allocator> From<Fiber<A>> for Vec<u8, A> {
    #[inline]
    fn from(v: Fiber<A>) -> Vec<u8, A> {
        v.into_bytes()
    }
}
impl<A: Allocator> From<Fiber<A>> for Arc<str, A> {
    #[inline]
    fn from(v: Fiber<A>) -> Arc<str, A> {
        unsafe {
            let (d, a) = Box::into_raw_with_allocator(v.vec.into_boxed_slice());
            Box::from_raw_in(d as *mut str, a)
        }
        .into()
    }
}
impl<'a, A: Allocator> From<&'a Fiber<A>> for Cow<'a, str> {
    #[inline]
    fn from(v: &'a Fiber<A>) -> Cow<'a, str> {
        Cow::Borrowed(v.as_str())
    }
}

impl FromIterator<char> for Fiber {
    #[inline]
    fn from_iter<I: IntoIterator<Item = char>>(i: I) -> Fiber {
        let (mut b, x) = (Fiber::new(), i.into_iter());
        match x.size_hint() {
            (_, Some(v)) => b.reserve(v),
            (v, _) => b.reserve(v),
        }
        for i in x {
            b.push(i);
        }
        b
    }
}
impl FromIterator<String> for Fiber {
    #[inline]
    fn from_iter<I: IntoIterator<Item = String>>(i: I) -> Fiber {
        let (mut b, x) = (Fiber::new(), i.into_iter());
        match x.size_hint() {
            (_, Some(v)) => b.reserve(v),
            (v, _) => b.reserve(v),
        }
        for i in x {
            b.push_str(&i);
        }
        b
    }
}
impl<'a> FromIterator<&'a str> for Fiber {
    #[inline]
    fn from_iter<I: IntoIterator<Item = &'a str>>(i: I) -> Fiber {
        let (mut b, x) = (Fiber::new(), i.into_iter());
        match x.size_hint() {
            (_, Some(v)) => b.reserve(v),
            (v, _) => b.reserve(v),
        }
        for i in x {
            b.push_str(&i);
        }
        b
    }
}
impl<'a> FromIterator<&'a char> for Fiber {
    #[inline]
    fn from_iter<I: IntoIterator<Item = &'a char>>(i: I) -> Fiber {
        let (mut b, x) = (Fiber::new(), i.into_iter());
        match x.size_hint() {
            (_, Some(v)) => b.reserve(v),
            (v, _) => b.reserve(v),
        }
        for i in x {
            b.push(*i);
        }
        b
    }
}
impl<'a> FromIterator<Cow<'a, str>> for Fiber {
    #[inline]
    fn from_iter<I: IntoIterator<Item = Cow<'a, str>>>(i: I) -> Fiber {
        let (mut b, x) = (Fiber::new(), i.into_iter());
        match x.size_hint() {
            (_, Some(v)) => b.reserve(v),
            (v, _) => b.reserve(v),
        }
        for i in x {
            b.push_str(&i);
        }
        b
    }
}
impl<A: Allocator> FromIterator<Box<str, A>> for Fiber {
    #[inline]
    fn from_iter<I: IntoIterator<Item = Box<str, A>>>(i: I) -> Fiber {
        let (mut b, x) = (Fiber::new(), i.into_iter());
        match x.size_hint() {
            (_, Some(v)) => b.reserve(v),
            (v, _) => b.reserve(v),
        }
        for i in x {
            b.push_str(&i);
        }
        b
    }
}

impl<A: Allocator> PartialEq<str> for Fiber<A> {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str().eq(other)
    }
}
impl<A: Allocator> PartialEq<String> for Fiber<A> {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_str().eq(other.as_str())
    }
}
impl<A: Allocator> PartialEq<ByteStr> for Fiber<A> {
    #[inline]
    fn eq(&self, other: &ByteStr) -> bool {
        self.vec.eq(&other.0)
    }
}
impl<A: Allocator> PartialEq<ByteString> for Fiber<A> {
    #[inline]
    fn eq(&self, other: &ByteString) -> bool {
        self.vec.eq(&other.0)
    }
}
impl<'a, A: Allocator> PartialEq<&'a str> for Fiber<A> {
    #[inline]
    fn eq(&self, other: &&'a str) -> bool {
        self.as_str().eq(*other)
    }
}
impl<'a, A: Allocator> PartialEq<Cow<'a, str>> for Fiber<A> {
    #[inline]
    fn eq(&self, other: &Cow<'a, str>) -> bool {
        self.as_str().eq(other)
    }
}
impl<A: Allocator, B: Allocator> PartialEq<Fiber<B>> for Fiber<A> {
    #[inline]
    fn eq(&self, other: &Fiber<B>) -> bool {
        self.vec.eq(&other.vec)
    }
}

impl<A: Allocator, I: SliceIndex<str>> Index<I> for Fiber<A> {
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &I::Output {
        index.index(self.as_str())
    }
}
impl<A: Allocator, I: SliceIndex<str>> IndexMut<I> for Fiber<A> {
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut I::Output {
        index.index_mut(self.as_mut_str())
    }
}

impl<A: Allocator> Eq for FromUtf8Error<A> {}
impl<A: Allocator> Debug for FromUtf8Error<A> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.error, f)
    }
}
impl<A: Allocator> Error for FromUtf8Error<A> {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
impl<A: Allocator> Display for FromUtf8Error<A> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.error, f)
    }
}
impl<A: Allocator> PartialEq for FromUtf8Error<A> {
    #[inline]
    fn eq(&self, other: &FromUtf8Error<A>) -> bool {
        self.bytes.eq(&other.bytes) && self.error.eq(&other.error)
    }
}
impl<A: Allocator + Clone> Clone for FromUtf8Error<A> {
    #[inline]
    fn clone(&self) -> FromUtf8Error<A> {
        FromUtf8Error {
            bytes: self.bytes.clone(),
            error: self.error,
        }
    }
}

impl Debug for FromUtf16Error {
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.0 .0, f)
    }
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("FromUtf16Error:")?;
        Display::fmt(&self.0 .0, f)
    }
}
impl Deref for FromUtf16Error {
    type Target = u16;

    #[inline]
    fn deref(&self) -> &u16 {
        &self.0 .0
    }
}
impl Error for FromUtf16Error {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Display for FromUtf16Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
    }
}

impl<A: Allocator> AllocFrom<u8, A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: u8, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(1, alloc);
        r.vec.push(v);
        r
    }
}
impl<A: Allocator> AllocFrom<&str, A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: &str, alloc: A) -> Fiber<A> {
        Fiber::from_str_in(v, alloc)
    }
}
impl<A: Allocator> AllocFrom<char, A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: char, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(v.len_utf8(), alloc);
        r.push(v);
        r
    }
}
impl<A: Allocator> AllocFrom<Char, A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: Char, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(1, alloc);
        r.vec.push(v as u8);
        r
    }
}
impl<A: Allocator> AllocFrom<&char, A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: &char, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(v.len_utf8(), alloc);
        r.push(*v);
        r
    }
}
impl<A: Allocator> AllocFrom<&Char, A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: &Char, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(1, alloc);
        r.vec.push(*v as u8);
        r
    }
}
impl<A: Allocator> AllocFrom<&[u8], A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: &[u8], alloc: A) -> Fiber<A> {
        Fiber::from_utf8_lossy_in(v, alloc)
    }
}
impl<A: Allocator> AllocFrom<String, A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: String, alloc: A) -> Fiber<A> {
        Fiber::from_str_in(&v, alloc)
    }
}
impl<A: Allocator> AllocFrom<&String, A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: &String, alloc: A) -> Fiber<A> {
        Fiber::from_str_in(&v, alloc)
    }
}

impl<A: Allocator> AllocInto<Fiber<A>, A> for u8 {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(1, alloc);
        r.vec.push(self);
        r
    }
}
impl<A: Allocator> AllocInto<Fiber<A>, A> for &str {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        Fiber::from_str_in(self, alloc)
    }
}
impl<A: Allocator> AllocInto<Fiber<A>, A> for char {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(self.len_utf8(), alloc);
        r.push(self);
        r
    }
}
impl<A: Allocator> AllocInto<Fiber<A>, A> for Char {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(1, alloc);
        r.vec.push(self as u8);
        r
    }
}
impl<A: Allocator> AllocInto<Fiber<A>, A> for &char {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(self.len_utf8(), alloc);
        r.push(*self);
        r
    }
}
impl<A: Allocator> AllocInto<Fiber<A>, A> for &Char {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(1, alloc);
        r.vec.push(*self as u8);
        r
    }
}
impl<A: Allocator> AllocInto<Fiber<A>, A> for &[u8] {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        Fiber::from_utf8_lossy_in(self, alloc)
    }
}
impl<A: Allocator> AllocInto<Fiber<A>, A> for String {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        Fiber::from_str_in(&self, alloc)
    }
}
impl<A: Allocator> AllocInto<Fiber<A>, A> for &String {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        Fiber::from_str_in(self, alloc)
    }
}

impl<A: Allocator> Iterator for Drain<'_, A> {
    type Item = char;

    #[inline]
    fn last(mut self) -> Option<char> {
        self.next_back()
    }
    #[inline]
    fn next(&mut self) -> Option<char> {
        self.0.next().map(|v| v as char)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<A: Allocator> FusedIterator for Drain<'_, A> {}
impl<'a, A: Allocator> AsRef<str> for Drain<'a, A> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl<'a, A: Allocator> AsRef<[u8]> for Drain<'a, A> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_str().as_bytes()
    }
}
impl<A: Allocator> DoubleEndedIterator for Drain<'_, A> {
    #[inline]
    fn next_back(&mut self) -> Option<char> {
        self.0.next_back().map(|v| v as char)
    }
}

unsafe impl Sync for Drain<'_> {}
unsafe impl Send for Drain<'_> {}
