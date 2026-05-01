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
use core::mem::{transmute, MaybeUninit};
use core::ops::FnOnce;

/// A borrowed byte buffer which is incrementally filled and initialized.
///
/// This type is a sort of "double cursor". It tracks three regions in the
/// buffer: a region at the beginning of the buffer that has been logically
/// filled with data, a region that has been initialized at some point but not
/// yet logically filled, and a region at the end that is fully uninitialized.
/// The filled region is guaranteed to be a subset of the initialized region.
///
/// In summary, the contents of the buffer can be visualized as:
/// ```not_rust
/// [             capacity              ]
/// [ filled |         unfilled         ]
/// [    initialized    | uninitialized ]
/// ```
///
/// A `BorrowedBuf` is created around some existing data (or capacity for data)
/// via a unique reference (`&mut`). The `BorrowedBuf` can be configured (e.g.,
/// using `clear` or `set_init`), but cannot be directly written. To write into
/// the buffer, use `unfilled` to create a `BorrowedCursor`. The cursor
/// has write-only access to the unfilled portion of the buffer (you can think
/// of it as a write-only iterator).
///
/// The lifetime `'a` is a bound on the lifetime of the underlying data.
// Wrapper to prevent getting the nightly warning.
#[cfg_attr(not(feature = "strip"), derive(Debug))]
#[repr(transparent)]
pub struct BorrowedBuf<'a>(io::BorrowedBuf<'a>);
/// A writeable view of the unfilled portion of a [`BorrowedBuf`].
///
/// The unfilled portion consists of an initialized and an uninitialized part;
/// see [`BorrowedBuf`] for details.
///
/// Data can be written directly to the cursor by using
/// [`append`](BorrowedCursor::append) or indirectly by getting a
/// slice of part or all of the cursor and writing into the slice. In the
/// indirect case, the caller must call [`advance`](BorrowedCursor::advance)
/// after writing to inform the cursor how many bytes have been written.
///
/// Once data is written to the cursor, it becomes part of the filled portion of
/// the underlying `BorrowedBuf` and can no longer be accessed or re-written by
/// the cursor. I.e., the cursor tracks the unfilled part of the underlying
/// `BorrowedBuf`.
///
/// The lifetime `'a` is a bound on the lifetime of the underlying buffer (which
/// means it is a bound on the data in that buffer by transitivity).
// Wrapper to prevent getting the nightly warning.
#[cfg_attr(not(feature = "strip"), derive(Debug))]
#[repr(transparent)]
pub struct BorrowedCursor<'a>(io::BorrowedCursor<'a>);

impl<'a> BorrowedBuf<'a> {
    /// Returns the length of the filled part of the buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Returns a shared reference to the filled portion of the buffer.
    #[inline]
    pub fn filled(&self) -> &[u8] {
        self.0.filled()
    }
    /// Returns the total capacity of the buffer.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
    /// Returns the length of the initialized part of the buffer.
    #[inline]
    pub fn init_len(&self) -> usize {
        self.0.init_len()
    }
    /// Returns a shared reference to the filled portion of the buffer with its
    /// original lifetime.
    #[inline]
    pub fn into_filled(self) -> &'a [u8] {
        self.0.into_filled()
    }
    /// Returns a mutable reference to the filled portion of the buffer.
    #[inline]
    pub fn filled_mut(&mut self) -> &mut [u8] {
        self.0.filled_mut()
    }
    /// Returns a mutable reference to the filled portion of the buffer with its
    /// original lifetime.
    #[inline]
    pub fn into_filled_mut(self) -> &'a mut [u8] {
        self.0.into_filled_mut()
    }
    /// Clears the buffer, resetting the filled region to empty.
    #[inline]
    pub fn clear(&mut self) -> &mut BorrowedBuf<'a> {
        unsafe { transmute(self.0.clear()) }
    }
    /// Returns a cursor over the unfilled part of the buffer.
    #[inline]
    pub fn unfilled<'b>(&'b mut self) -> BorrowedCursor<'b> {
        BorrowedCursor(self.0.unfilled())
    }

    /// Asserts that the first `n` bytes of the buffer are initialized.
    #[inline]
    pub unsafe fn set_init(&mut self, n: usize) -> &mut BorrowedBuf<'a> {
        unsafe { transmute(self.0.set_init(n)) }
    }
}
impl<'a> BorrowedCursor<'a> {
    /// Returns the number of bytes written to this cursor since it was created
    /// from a `BorrowedBuf`.
    ///
    /// Note that if this cursor is a reborrowed clone of another, then the
    /// count returned is the count written via either cursor, not the count
    /// since the cursor was reborrowed.
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
    pub fn append(&mut self, b: &[u8]) {
        self.0.append(b)
    }
    /// Returns a mutable reference to the initialized portion of the cursor.
    #[inline]
    pub fn init_mut(&mut self) -> &mut [u8] {
        self.0.init_mut()
    }
    /// Reborrows this cursor by cloning it with a smaller lifetime.
    #[inline]
    pub fn reborrow<'b>(&'b mut self) -> BorrowedCursor<'b> {
        BorrowedCursor(self.0.reborrow())
    }
    /// Initializes all bytes in the cursor.
    #[inline]
    pub fn ensure_init(&mut self) -> &mut BorrowedCursor<'a> {
        unsafe { transmute(self.0.ensure_init()) }
    }
    /// Advances the cursor by asserting that `n` bytes have been filled.
    ///
    /// After advancing, the `n` bytes are no longer accessible via the cursor
    /// and can only be accessed via the underlying buffer. I.e., the
    /// buffer's filled portion grows by `n` elements and its unfilled
    /// portion (and the capacity of this cursor) shrinks by `n` elements.
    ///
    /// If less than `n` bytes initialized (by the cursor's point of view),
    /// `set_init` should be called first.
    ///
    /// # Panics
    ///
    /// Panics if there are less than `n` bytes initialized.
    #[inline]
    pub fn advance(&mut self, n: usize) -> &mut BorrowedCursor<'a> {
        unsafe { transmute(self.0.advance(n)) }
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
    pub fn with_unfilled_buf<T>(&mut self, f: impl FnOnce(&mut io::BorrowedBuf<'_>) -> T) -> T {
        self.0.with_unfilled_buf(f)
    }

    /// Returns a mutable reference to the whole cursor.
    ///
    /// # Safety
    ///
    /// The caller must not uninitialize any bytes in the initialized portion of
    /// the cursor.
    #[inline]
    pub unsafe fn as_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        // SAFETY: always in bounds
        unsafe { self.0.as_mut() }
    }
    /// Asserts that the first `n` unfilled bytes of the cursor are initialized.
    ///
    /// `BorrowedBuf` assumes that bytes are never de-initialized, so this
    /// method does nothing when called with fewer bytes than are already
    /// known to be initialized.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the first `n` bytes of the buffer have
    /// already been initialized.
    #[inline]
    pub unsafe fn set_init(&mut self, n: usize) -> &mut BorrowedCursor<'a> {
        unsafe { transmute(self.0.set_init(n)) }
    }
    /// Advances the cursor by asserting that `n` bytes have been filled.
    ///
    /// After advancing, the `n` bytes are no longer accessible via the cursor
    /// and can only be accessed via the underlying buffer. I.e., the
    /// buffer's filled portion grows by `n` elements and its unfilled
    /// portion (and the capacity of this cursor) shrinks by `n` elements.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the first `n` bytes of the cursor have been
    /// properly initialized.
    #[inline]
    pub unsafe fn advance_unchecked(&mut self, n: usize) -> &mut BorrowedCursor<'a> {
        unsafe { transmute(self.0.advance_unchecked(n)) }
    }
}

impl<'a> From<&'a mut [u8]> for BorrowedBuf<'a> {
    #[inline]
    fn from(v: &'a mut [u8]) -> BorrowedBuf<'a> {
        BorrowedBuf(io::BorrowedBuf::from(v))
    }
}
impl<'a> From<&'a mut [MaybeUninit<u8>]> for BorrowedBuf<'a> {
    #[inline]
    fn from(v: &'a mut [MaybeUninit<u8>]) -> BorrowedBuf<'a> {
        BorrowedBuf(io::BorrowedBuf::from(v))
    }
}

impl<'a> From<BorrowedCursor<'a>> for BorrowedBuf<'a> {
    #[inline]
    fn from(v: BorrowedCursor<'a>) -> BorrowedBuf<'a> {
        BorrowedBuf(io::BorrowedBuf::from(v.0))
    }
}
