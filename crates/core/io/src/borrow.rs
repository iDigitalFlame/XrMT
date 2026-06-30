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
#![cfg(not(feature = "std"))]

extern crate core;

use core::convert::From;
use core::io;
use core::marker::Copy;
use core::mem::{transmute, MaybeUninit};
use core::ops::FnOnce;

/// A borrowed buffer of initially uninitialized elements, which is
/// incrementally filled.
///
/// This type makes it safer to work with `MaybeUninit` buffers, such as to read
/// into a buffer without having to initialize it first. It tracks the region of
/// elements that have been filled and whether the unfilled region was
/// initialized.
///
/// In summary, the contents of the buffer can be visualized as:
/// ```not_rust
/// [                capacity                ]
/// [ filled | unfilled (may be initialized) ]
/// ```
///
/// A `BorrowedBuf` is created around some existing elements (or capacity for
/// elements) via a unique reference (`&mut`). The `BorrowedBuf` can be
/// configured (e.g., using `clear` or `set_init`), but cannot be directly
/// written. To write into the buffer, use `unfilled` to create a
/// `BorrowedCursor`. The cursor has write-only access to the unfilled portion
/// of the buffer (you can think of it as a write-only iterator).
///
/// The lifetime `'a` is a bound on the lifetime of the underlying elements.
///
/// The type is most commonly used to manage bytes, but can manage any type of
/// elements.
#[cfg_attr(not(feature = "strip"), derive(Debug))]
#[repr(transparent)]
pub struct BorrowedBuf<'a, T>(io::BorrowedBuf<'a, T>);
/// A writeable view of the unfilled portion of a [`BorrowedBuf`].
///
/// The unfilled portion may be uninitialized; see [`BorrowedBuf`] for details.
///
/// Data can be written directly to the cursor by using
/// [`append`](BorrowedCursor::append) or indirectly by getting a slice of part
/// or all of the cursor and writing into the slice. In the indirect case, the
/// caller must call [`advance`](BorrowedCursor::advance) after writing to
/// inform the cursor how many elements have been written.
///
/// Once elements are written to the cursor, they become part of the filled
/// portion of the underlying `BorrowedBuf` and can no longer be accessed or
/// re-written by the cursor. In other words, the cursor tracks the unfilled
/// part of the underlying `BorrowedBuf`.
///
/// The lifetime `'a` is a bound on the lifetime of the underlying buffer (which
/// means it is a bound on the elements in that buffer by transitivity).
#[cfg_attr(not(feature = "strip"), derive(Debug))]
#[repr(transparent)]
pub struct BorrowedCursor<'a, T>(io::BorrowedCursor<'a, T>);

impl<'a, T> BorrowedBuf<'a, T> {
    /// Returns the length of the filled part of the buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Returns `true` if the buffer is initialized.
    #[inline]
    pub fn is_init(&self) -> bool {
        self.0.is_init()
    }
    /// Returns the total capacity of the buffer.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}
impl<'a> BorrowedCursor<'a, u8> {
    /// Initializes all bytes in the cursor and returns them.
    #[inline]
    pub fn ensure_init(&mut self) -> &mut [u8] {
        self.0.ensure_init()
    }
}
impl<'a, T: Copy> BorrowedBuf<'a, T> {
    /// Returns a shared reference to the filled portion of the buffer.
    #[inline]
    pub fn filled(&self) -> &[T] {
        self.0.filled()
    }
    /// Returns a shared reference to the filled portion of the buffer with its
    /// original lifetime.
    #[inline]
    pub fn into_filled(self) -> &'a [T] {
        self.0.into_filled()
    }
    /// Returns a mutable reference to the filled portion of the buffer.
    #[inline]
    pub fn filled_mut(&mut self) -> &mut [T] {
        self.0.filled_mut()
    }
    /// Returns a mutable reference to the filled portion of the buffer with its
    /// original lifetime.
    #[inline]
    pub fn into_filled_mut(self) -> &'a mut [T] {
        self.0.into_filled_mut()
    }
    /// Clears the buffer, resetting the filled region to empty.
    #[inline]
    pub fn clear(&mut self) -> &mut BorrowedBuf<'a, T> {
        unsafe { transmute(self.0.clear()) }
    }
    /// Returns a cursor over the unfilled part of the buffer.
    #[inline]
    pub fn unfilled<'b>(&'b mut self) -> BorrowedCursor<'b, T> {
        BorrowedCursor(self.0.unfilled())
    }

    /// Asserts that the first `n` bytes of the buffer are initialized.
    #[inline]
    pub unsafe fn set_init(&mut self) -> &mut BorrowedBuf<'a, T> {
        unsafe { transmute(self.0.set_init()) }
    }
}
impl<'a, T: Copy> BorrowedCursor<'a, T> {
    /// Returns `true` if the buffer is initialized.
    #[inline]
    pub fn is_init(&self) -> bool {
        self.0.is_init()
    }
    /// Returns the number of elements written to the `BorrowedBuf` this cursor
    /// was created from.
    ///
    /// In particular, the count returned is shared by all reborrows of the
    /// cursor.
    #[inline]
    pub fn written(&self) -> usize {
        self.0.written()
    }
    /// Returns the available space in the cursor.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
    /// Appends data to the cursor, advancing position within its buffer.
    #[inline]
    pub fn append(&mut self, v: &[T]) {
        self.0.append(v)
    }
    /// Reborrows this cursor by cloning it with a smaller lifetime.
    #[inline]
    pub fn reborrow<'b>(&'b mut self) -> BorrowedCursor<'b, T> {
        BorrowedCursor(self.0.reborrow())
    }
    /// Advances the cursor by asserting that `n` elements have been filled.
    ///
    /// After advancing, the `n` elements are no longer accessible via the
    /// cursor and can only be accessed via the underlying buffer. I.e., the
    /// buffer's filled portion grows by `n` elements and its unfilled
    /// portion (and the capacity of this cursor) shrinks by `n` elements.
    ///
    /// If less than `n` elements initialized (by the cursor's point of view),
    /// `set_init` should be called first.
    ///
    /// # Panics
    ///
    /// Panics if there are less than `n` elements initialized.
    #[inline]
    pub fn advance_checked(&mut self, n: usize) -> &mut BorrowedCursor<'a, T> {
        unsafe { transmute(self.0.advance_checked(n)) }
    }
    /// Runs the given closure with a `BorrowedBuf` containing the unfilled part
    /// of the cursor.
    ///
    /// This enables inspecting what was written to the cursor.
    ///
    /// # Panics
    ///
    /// Panics if the `BorrowedBuf` given to the closure is replaced by another
    /// one.
    #[inline]
    pub fn with_unfilled_buf<V>(&mut self, f: impl FnOnce(&mut io::BorrowedBuf<'_, T>) -> V) -> V {
        self.0.with_unfilled_buf(f)
    }

    /// Set the buffer as fully initialized.
    ///
    /// # Safety
    ///
    /// All the elements of the cursor must be initialized.
    #[inline]
    pub unsafe fn set_init(&mut self) {
        unsafe { self.0.set_init() }
    }
    /// Returns a mutable reference to the whole cursor.
    ///
    /// # Safety
    ///
    /// The caller must not uninitialize any bytes in the initialized portion of
    /// the cursor.
    #[inline]
    pub unsafe fn as_mut(&mut self) -> &mut [MaybeUninit<T>] {
        // SAFETY: always in bounds
        unsafe { self.0.as_mut() }
    }
    /// Advances the cursor by asserting that `n` elements have been filled.
    ///
    /// After advancing, the `n` elements are no longer accessible via the
    /// cursor and can only be accessed via the underlying buffer. I.e., the
    /// buffer's filled portion grows by `n` elements and its unfilled
    /// portion (and the capacity of this cursor) shrinks by `n` elements.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the first `n` elements of the cursor have
    /// been initialized.
    #[inline]
    pub unsafe fn advance(&mut self, n: usize) -> &mut BorrowedCursor<'a, T> {
        unsafe { transmute(self.0.advance(n)) }
    }
}

impl<'a, T: Copy> From<&'a mut [T]> for BorrowedBuf<'a, T> {
    #[inline]
    fn from(v: &'a mut [T]) -> BorrowedBuf<'a, T> {
        BorrowedBuf(io::BorrowedBuf::from(v))
    }
}
impl<'a, T: Copy> From<&'a mut [MaybeUninit<T>]> for BorrowedBuf<'a, T> {
    #[inline]
    fn from(v: &'a mut [MaybeUninit<T>]) -> BorrowedBuf<'a, T> {
        BorrowedBuf(io::BorrowedBuf::from(v))
    }
}

impl<'a, T: Copy> From<BorrowedCursor<'a, T>> for BorrowedBuf<'a, T> {
    #[inline]
    fn from(v: BorrowedCursor<'a, T>) -> BorrowedBuf<'a, T> {
        BorrowedBuf(io::BorrowedBuf::from(v.0))
    }
}
