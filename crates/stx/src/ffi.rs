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

//
// Module assistance with help from the Rust Team std/io code!
//

//! Utilities related to FFI bindings.
//!
//! This module provides utilities to handle data across non-Rust
//! interfaces, like other programming languages and the underlying
//! operating system. It is mainly of use for FFI (Foreign Function
//! Interface) bindings and code that needs to exchange C-like strings
//! with other languages.
//!
//! # Overview
//!
//! Rust represents owned strings with the [`String`] type, and
//! borrowed slices of strings with the [`str`] primitive. Both are
//! always in UTF-8 encoding, and may contain nul bytes in the middle,
//! i.e., if you look at the bytes that make up the string, there may
//! be a `\0` among them. Both `String` and `str` store their length
//! explicitly; there are no nul terminators at the end of strings
//! like in C.
//!
//! C strings are different from Rust strings:
//!
//! * **Encodings** - Rust strings are UTF-8, but C strings may use
//! other encodings. If you are using a string from C, you should
//! check its encoding explicitly, rather than just assuming that it
//! is UTF-8 like you can do in Rust.
//!
//! * **Character size** - C strings may use `char` or `wchar_t`-sized
//! characters; please **note** that C's `char` is different from Rust's.
//! The C standard leaves the actual sizes of those types open to
//! interpretation, but defines different APIs for strings made up of
//! each character type. Rust strings are always UTF-8, so different
//! Unicode characters will be encoded in a variable number of bytes
//! each. The Rust type [`char`] represents a '[Unicode scalar
//! value]', which is similar to, but not the same as, a '[Unicode
//! code point]'.
//!
//! * **Nul terminators and implicit string lengths** - Often, C
//! strings are nul-terminated, i.e., they have a `\0` character at the
//! end. The length of a string buffer is not stored, but has to be
//! calculated; to compute the length of a string, C code must
//! manually call a function like `strlen()` for `char`-based strings,
//! or `wcslen()` for `wchar_t`-based ones. Those functions return
//! the number of characters in the string excluding the nul
//! terminator, so the buffer length is really `len+1` characters.
//! Rust strings don't have a nul terminator; their length is always
//! stored and does not need to be calculated. While in Rust
//! accessing a string's length is an *O*(1) operation (because the
//! length is stored); in C it is an *O*(*n*) operation because the
//! length needs to be computed by scanning the string for the nul
//! terminator.
//!
//! * **Internal nul characters** - When C strings have a nul
//! terminator character, this usually means that they cannot have nul
//! characters in the middle — a nul character would essentially
//! truncate the string. Rust strings *can* have nul characters in
//! the middle, because nul does not have to mark the end of the
//! string in Rust.
//!
//! # Representations of non-Rust strings
//!
//! [`CString`] and [`CStr`] are useful when you need to transfer
//! UTF-8 strings to and from languages with a C ABI, like Python.
//!
//! * **From Rust to C:** [`CString`] represents an owned, C-friendly
//! string: it is nul-terminated, and has no internal nul characters.
//! Rust code can create a [`CString`] out of a normal string (provided
//! that the string doesn't have nul characters in the middle), and
//! then use a variety of methods to obtain a raw <code>\*mut [u8]</code> that
//! can then be passed as an argument to functions which use the C
//! conventions for strings.
//!
//! * **From C to Rust:** [`CStr`] represents a borrowed C string; it
//! is what you would use to wrap a raw <code>\*const [u8]</code> that you got
//! from a C function. A [`CStr`] is guaranteed to be a nul-terminated array
//! of bytes. Once you have a [`CStr`], you can convert it to a Rust
//! <code>&[str]</code> if it's valid UTF-8, or lossily convert it by adding
//! replacement characters.
//!
//! [`OsString`] and [`OsStr`] are useful when you need to transfer
//! strings to and from the operating system itself, or when capturing
//! the output of external commands. Conversions between [`OsString`],
//! [`OsStr`] and Rust strings work similarly to those for [`CString`]
//! and [`CStr`].
//!
//! * [`OsString`] losslessly represents an owned platform string. However, this
//! representation is not necessarily in a form native to the platform.
//! In the Rust standard library, various APIs that transfer strings to/from the
//! operating system use [`OsString`] instead of plain strings. For example,
//! [`env::var_os()`] is used to query environment variables; it
//! returns an <code>[Option]<[OsString]></code>. If the environment variable
//! exists you will get a <code>[Some]\(os_string)</code>, which you can
//! *then* try to convert to a Rust string. This yields a [`Result`], so that
//! your code can detect errors in case the environment variable did
//! not in fact contain valid Unicode data.
//!
//! * [`OsStr`] losslessly represents a borrowed reference to a platform string.
//! However, this representation is not necessarily in a form native to the
//! platform. It can be converted into a UTF-8 Rust string slice in a similar
//! way to [`OsString`].
//!
//! # Conversions
//!
//! ## On Unix
//!
//! On Unix, [`OsStr`] implements the
//! <code>xrmt_stx::os::unix::ffi::OsStrExt</code> trait, which
//! augments it with two methods, `from_bytes` and `as_bytes`.
//! These do inexpensive conversions from and to byte slices.
//!
//! Additionally, on Unix [`OsString`] implements the
//! <code>xrmt_stx::os::unix::ffi::OsStringExt</code> trait,
//! which provides `from_vec` and `into_vec` methods that consume
//! their arguments, and take or produce vectors of [`u8`].
//!
//! ## On Windows
//!
//! An [`OsStr`] can be losslessly converted to a native Windows string. And
//! a native Windows string can be losslessly converted to an [`OsString`].
//!
//! On Windows, [`OsStr`] implements the
//! <code>xrmt_stx::os::windows::ffi::[OsStrExt][windows.OsStrExt]</code> trait,
//! which provides an [`encode_wide`] method. This provides an
//! iterator that can be [`collect`]ed into a vector of [`u16`]. After a nul
//! characters is appended, this is the same as a native Windows string.
//!
//! Additionally, on Windows [`OsString`] implements the
//! <code>xrmt_stx::os::windows:ffi::[OsStringExt][windows.OsStringExt]</code>
//! trait, which provides a [`from_wide`] method to convert a native Windows
//! string (without the terminating nul character) to an [`OsString`].
//!
//! ## Other platforms
//!
//! Many other platforms provide their own extension traits in a
//! `xrmt_stx::os::*::ffi` module.
//!
//! ## On all platforms
//!
//! On all platforms, [`OsStr`] consists of a sequence of bytes that is encoded
//! as a superset of UTF-8; see [`OsString`] for more details on its encoding on
//! different platforms.
//!
//! For limited, inexpensive conversions from and to bytes, see
//! [`OsStr::as_encoded_bytes`] and [`OsStr::from_encoded_bytes_unchecked`].
//!
//! For basic string processing, see [`OsStr::slice_encoded_bytes`].
//!
//! [Unicode scalar value]: https://www.unicode.org/glossary/#unicode_scalar_value
//! [Unicode code point]: https://www.unicode.org/glossary/#code_point
//! [`env::set_var()`]: crate::env::set_var "env::set_var"
//! [`env::var_os()`]: crate::env::var_os "env::var_os"
//! [windows.OsStrExt]: crate::os::windows::ffi::OsStrExt "os::windows::ffi::OsStrExt"
//! [`encode_wide`]: crate::os::windows::ffi::OsStrExt::encode_wide "os::windows::ffi::OsStrExt::encode_wide"
//! [`collect`]: core::iter::Iterator::collect "iter::Iterator::collect"
//! [windows.OsStringExt]: crate::os::windows::ffi::OsStringExt "os::windows::ffi::OsStringExt"
//! [`from_wide`]: crate::os::windows::ffi::OsStringExt::from_wide "os::windows::ffi::OsStringExt::from_wide"

#![no_implicit_prelude]
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate alloc;
extern crate core;

extern crate xrmt_data;
extern crate xrmt_winapi;

use alloc::borrow::{Borrow, Cow, ToOwned};
use alloc::boxed::Box;
use alloc::collections::TryReserveError;
use alloc::rc::Rc;
use alloc::slice::Join;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::clone::Clone;
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::{AsRef, From, Infallible, Into, TryFrom};
use core::default::Default;
use core::fmt::Write;
use core::hash::{Hash, Hasher};
use core::iter::{Extend, FromIterator, IntoIterator, Iterator};
use core::marker::Sized;
use core::mem::transmute;
use core::ops::{Deref, DerefMut, FnOnce, Index, IndexMut, RangeBounds, RangeFull};
use core::option::Option::{self, None, Some};
use core::result::Result::{self, Ok};
use core::slice::range;
use core::str::{from_utf8, FromStr, Utf8Error};

use xrmt_data::Fiber;
use xrmt_winapi::structs::{Char, CharLike, CharPtr, CharSlice, StringLikeU16, StringLikeU8, WChar, WCharLike, WCharPtr, WCharSlice};

use crate::io::FmtResult;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use core::ffi::*;
pub use alloc::ffi::{CString, FromVecWithNulError, IntoStringError, NulError};

/// Borrowed reference to an OS string (see [`OsString`]).
///
/// This type represents a borrowed reference to a string in the operating
/// system's preferred representation.
///
/// `&OsStr` is to [`OsString`] as <code>&[str]</code> is to [`String`]: the
/// former in each pair are borrowed references; the latter are owned strings.
///
/// See the [module's toplevel documentation about conversions][conversions] for
/// a discussion on the traits which `OsStr` implements for [conversions]
/// from/to native representations.
///
/// [conversions]: crate::ffi#conversions
#[repr(transparent)]
pub struct OsStr([u8]);
/// A type that can represent owned, mutable platform-native strings, but is
/// cheaply inter-convertible with Rust strings.
///
/// The need for this type arises from the fact that:
///
/// * On Unix systems, strings are often arbitrary sequences of non-zero bytes,
///   in many cases interpreted as UTF-8.
///
/// * On Windows, strings are often arbitrary sequences of non-zero 16-bit
///   values, interpreted as UTF-16 when it is valid to do so.
///
/// * In Rust, strings are always valid UTF-8, which may contain zeros.
///
/// `OsString` and [`OsStr`] bridge this gap by simultaneously representing Rust
/// and platform-native string values, and in particular allowing a Rust string
/// to be converted into an "OS" string with no cost if possible. A consequence
/// of this is that `OsString` instances are *not* `NUL` terminated; in order
/// to pass to e.g., Unix system call, you should create a [`CStr`].
///
/// `OsString` is to <code>&[OsStr]</code> as [`String`] is to
/// <code>&[str]</code>: the former in each pair are owned strings; the latter
/// are borrowed references.
///
/// Note, `OsString` and [`OsStr`] internally do not necessarily hold strings in
/// the form native to the platform; While on Unix, strings are stored as a
/// sequence of 8-bit values, on Windows, where strings are 16-bit value based
/// as just discussed, strings are also actually stored as a sequence of 8-bit
/// values, encoded in a less-strict variant of UTF-8. This is useful to
/// understand when handling capacity and length values.
///
/// # Capacity of `OsString`
///
/// Capacity uses units of UTF-8 bytes for OS strings which were created from
/// valid unicode, and uses units of bytes in an unspecified encoding for other
/// contents. On a given target, all `OsString` and `OsStr` values use the same
/// units for capacity, so the following will work:
/// ```
/// use xrmt_stx::ffi::{OsStr, OsString};
///
/// fn concat_os_strings(a: &OsStr, b: &OsStr) -> OsString {
///     let mut ret = OsString::with_capacity(a.len() + b.len()); // This will
/// allocate     ret.push(a); // This will not allocate further
///     ret.push(b); // This will not allocate further
///     ret
/// }
/// ```
///
/// # Creating an `OsString`
///
/// **From a Rust string**: `OsString` implements
/// <code>[From]<[String]></code>, so you can use
/// <code>my_string.[into]\()</code> to create an `OsString` from a normal Rust
/// string.
///
/// **From slices:** Just like you can start with an empty Rust
/// [`String`] and then [`String::push_str`] some <code>&[str]</code>
/// sub-string slices into it, you can create an empty `OsString` with
/// the [`OsString::new`] method and then push string slices into it with the
/// [`OsString::push`] method.
///
/// # Extracting a borrowed reference to the whole OS string
///
/// You can use the [`OsString::as_os_str`] method to get an
/// <code>&[OsStr]</code> from an `OsString`; this is effectively a borrowed
/// reference to the whole string.
///
/// # Conversions
///
/// See the [module's toplevel documentation about conversions][conversions] for
/// a discussion on the traits which `OsString` implements for [conversions]
/// from/to native representations.
///
/// [`CStr`]: crate::ffi::CStr
/// [conversions]: crate::ffi#conversions
/// [into]: core::convert::Into::into
pub struct OsString(Vec<u8>);

impl OsStr {
    /// Coerces into an `OsStr` slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("foo");
    /// ```
    #[inline]
    pub fn new<T: ?Sized + AsRef<OsStr>>(s: &T) -> &OsStr {
        s.as_ref()
    }

    /// Converts a slice of bytes to an OS string slice without checking that
    /// the string contains valid `OsStr`-encoded data.
    ///
    /// The byte encoding is an unspecified, platform-specific,
    /// self-synchronizing superset of UTF-8. By being a self-synchronizing
    /// superset of UTF-8, this encoding is also a superset of 7-bit ASCII.
    ///
    /// See the [module's toplevel documentation about conversions][conversions]
    /// for safe, cross-platform [conversions] from/to native
    /// representations.
    ///
    /// # Safety
    ///
    /// As the encoding is unspecified, callers must pass in bytes that
    /// originated as a mixture of validated UTF-8 and bytes from
    /// [`OsStr::as_encoded_bytes`] from within the same Rust version
    /// built for the same target platform.  For example, reconstructing an
    /// `OsStr` from bytes sent over the network or stored in a file will
    /// likely violate these safety rules.
    ///
    /// Due to the encoding being self-synchronizing, the bytes from
    /// [`OsStr::as_encoded_bytes`] can be split either immediately before
    /// or immediately after any valid non-empty UTF-8 substring.
    ///
    /// # Example
    ///
    /// ```
    /// use xrmt_stx::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("Mary had a little lamb");
    /// let bytes = os_str.as_encoded_bytes();
    /// let words = bytes.split(|b| *b == b' ');
    /// let words: Vec<&OsStr> = words.map(|word| {
    ///     // SAFETY:
    ///     // - Each `word` only contains content that originated from `OsStr::as_encoded_bytes`
    ///     // - Only split with ASCII whitespace which is a non-empty UTF-8 substring
    ///     unsafe { OsStr::from_encoded_bytes_unchecked(word) }
    /// }).collect();
    /// ```
    ///
    /// [conversions]: crate::ffi#conversions
    #[inline]
    pub unsafe fn from_encoded_bytes_unchecked(bytes: &[u8]) -> &OsStr {
        unsafe { transmute(bytes) }
    }

    /// Returns the length of this `OsStr`.
    ///
    /// Note that this does **not** return the number of bytes in the string in
    /// OS string form.
    ///
    /// The length returned is that of the underlying storage used by `OsStr`.
    /// As discussed in the [`OsString`] introduction, [`OsString`] and `OsStr`
    /// store strings in a form best suited for cheap inter-conversion between
    /// native-platform and Rust string forms, which may differ significantly
    /// from both of them, including in storage size and encoding.
    ///
    /// This number is simply useful for passing to other methods, like
    /// [`OsString::with_capacity`] to avoid reallocations.
    ///
    /// See the main `OsString` documentation information about encoding and
    /// capacity units.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("");
    /// assert_eq!(os_str.len(), 0);
    ///
    /// let os_str = OsStr::new("foo");
    /// assert_eq!(os_str.len(), 3);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Checks if all characters in this string are within the ASCII range.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// let ascii = OsString::from("hello!\n");
    /// let non_ascii = OsString::from("Grüße, Jürgen ❤");
    ///
    /// assert!(ascii.is_ascii());
    /// assert!(!non_ascii.is_ascii());
    /// ```
    #[inline]
    pub fn is_ascii(&self) -> bool {
        self.0.is_ascii()
    }
    /// Checks whether the `OsStr` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("");
    /// assert!(os_str.is_empty());
    ///
    /// let os_str = OsStr::new("foo");
    /// assert!(!os_str.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Yields a <code>&[str]</code> slice if the `OsStr` is valid Unicode.
    ///
    /// This conversion may entail doing a check for UTF-8 validity.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("foo");
    /// assert_eq!(os_str.to_str(), Some("foo"));
    /// ```
    #[inline]
    pub fn to_str(&self) -> Option<&str> {
        from_utf8(&self.0).ok()
    }
    /// Converts this string to its ASCII lower case equivalent in-place.
    ///
    /// ASCII letters 'A' to 'Z' are mapped to 'a' to 'z',
    /// but non-ASCII letters are unchanged.
    ///
    /// To return a new lowercased value without modifying the existing one, use
    /// [`OsStr::to_ascii_lowercase`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// let mut s = OsString::from("GRÜßE, JÜRGEN ❤");
    ///
    /// s.make_ascii_lowercase();
    ///
    /// assert_eq!("grÜße, jÜrgen ❤", s);
    /// ```
    #[inline]
    pub fn make_ascii_lowercase(&mut self) {
        self.0.make_ascii_lowercase()
    }
    /// Copies the slice into an owned [`OsString`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::{OsStr, OsString};
    ///
    /// let os_str = OsStr::new("foo");
    /// let os_string = os_str.to_os_string();
    /// assert_eq!(os_string, OsString::from("foo"));
    /// ```
    #[inline]
    pub fn to_os_string(&self) -> OsString {
        OsString(self.0.to_vec())
    }
    /// Converts this string to its ASCII upper case equivalent in-place.
    ///
    /// ASCII letters 'a' to 'z' are mapped to 'A' to 'Z',
    /// but non-ASCII letters are unchanged.
    ///
    /// To return a new uppercased value without modifying the existing one, use
    /// [`OsStr::to_ascii_uppercase`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// let mut s = OsString::from("Grüße, Jürgen ❤");
    ///
    /// s.make_ascii_uppercase();
    ///
    /// assert_eq!("GRüßE, JüRGEN ❤", s);
    /// ```
    #[inline]
    pub fn make_ascii_uppercase(&mut self) {
        self.0.make_ascii_uppercase()
    }
    /// Converts an OS string slice to a byte slice.  To convert the byte slice
    /// back into an OS string slice, use the
    /// [`OsStr::from_encoded_bytes_unchecked`] function.
    ///
    /// The byte encoding is an unspecified, platform-specific,
    /// self-synchronizing superset of UTF-8. By being a self-synchronizing
    /// superset of UTF-8, this encoding is also a superset of 7-bit ASCII.
    ///
    /// Note: As the encoding is unspecified, any sub-slice of bytes that is not
    /// valid UTF-8 should be treated as opaque and only comparable within
    /// the same Rust version built for the same target platform.  For
    /// example, sending the slice over the network or storing it in a file
    /// will likely result in incompatible byte slices.  See [`OsString`] for
    /// more encoding details and [`xrmt_stx::ffi`] for platform-specific,
    /// specified conversions.
    ///
    /// [`xrmt_stx::ffi`]: crate::ffi
    #[inline]
    pub fn as_encoded_bytes(&self) -> &[u8] {
        &self.0
    }
    /// Returns a copy of this string where each character is mapped to its
    /// ASCII lower case equivalent.
    ///
    /// ASCII letters 'A' to 'Z' are mapped to 'a' to 'z',
    /// but non-ASCII letters are unchanged.
    ///
    /// To lowercase the value in-place, use [`OsStr::make_ascii_lowercase`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    /// let s = OsString::from("Grüße, Jürgen ❤");
    ///
    /// assert_eq!("grüße, jürgen ❤", s.to_ascii_lowercase());
    /// ```
    #[inline]
    pub fn to_ascii_lowercase(&self) -> OsString {
        OsString(self.0.to_ascii_lowercase())
    }
    /// Returns a copy of this string where each character is mapped to its
    /// ASCII upper case equivalent.
    ///
    /// ASCII letters 'a' to 'z' are mapped to 'A' to 'Z',
    /// but non-ASCII letters are unchanged.
    ///
    /// To uppercase the value in-place, use [`OsStr::make_ascii_uppercase`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    /// let s = OsString::from("Grüße, Jürgen ❤");
    ///
    /// assert_eq!("GRüßE, JüRGEN ❤", s.to_ascii_uppercase());
    /// ```
    #[inline]
    pub fn to_ascii_uppercase(&self) -> OsString {
        OsString(self.0.to_ascii_uppercase())
    }
    /// Converts an `OsStr` to a <code>[Cow]<[str]></code>.
    ///
    /// Any non-UTF-8 sequences are replaced with
    /// [`U+FFFD REPLACEMENT CHARACTER`][U+FFFD].
    ///
    /// [U+FFFD]: core::char::REPLACEMENT_CHARACTER
    ///
    /// # Examples
    ///
    /// Calling `to_string_lossy` on an `OsStr` with invalid unicode:
    ///
    /// ```
    /// // Note, due to differences in how Unix and Windows represent strings,
    /// // we are forced to complicate this example, setting up example `OsStr`s
    /// // with different source data and via different platform extensions.
    /// // Understand that in reality you could end up with such example invalid
    /// // sequences simply through collecting user command line arguments, for
    /// // example.
    ///
    /// #[cfg(unix)] {
    ///     use xrmt_stx::ffi::OsStr;
    ///     use xrmt_stx::os::unix::ffi::OsStrExt;
    ///
    ///     // Here, the values 0x66 and 0x6f correspond to 'f' and 'o'
    ///     // respectively. The value 0x80 is a lone continuation byte, invalid
    ///     // in a UTF-8 sequence.
    ///     let source = [0x66, 0x6f, 0x80, 0x6f];
    ///     let os_str = OsStr::from_bytes(&source[..]);
    ///
    ///     assert_eq!(os_str.to_string_lossy(), "fo�o");
    /// }
    /// #[cfg(windows)] {
    ///     use xrmt_stx::ffi::OsString;
    ///     use xrmt_stx::os::windows::prelude::*;
    ///
    ///     // Here the values 0x0066 and 0x006f correspond to 'f' and 'o'
    ///     // respectively. The value 0xD800 is a lone surrogate half, invalid
    ///     // in a UTF-16 sequence.
    ///     let source = [0x0066, 0x006f, 0xD800, 0x006f];
    ///     let os_string = OsString::from_wide(&source[..]);
    ///     let os_str = os_string.as_os_str();
    ///
    ///     assert_eq!(os_str.to_string_lossy(), "fo�o");
    /// }
    /// ```
    #[inline]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.0)
    }
    /// Checks that two strings are an ASCII case-insensitive match.
    ///
    /// Same as `to_ascii_lowercase(a) == to_ascii_lowercase(b)`,
    /// but without allocating and copying temporaries.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// assert!(OsString::from("Ferris").eq_ignore_ascii_case("FERRIS"));
    /// assert!(OsString::from("Ferrös").eq_ignore_ascii_case("FERRöS"));
    /// assert!(!OsString::from("Ferrös").eq_ignore_ascii_case("FERRÖS"));
    /// ```
    #[inline]
    pub fn eq_ignore_ascii_case<T: AsRef<OsStr>>(&self, other: T) -> bool {
        self.0.eq_ignore_ascii_case(&other.as_ref().0)
    }
    /// Takes a substring based on a range that corresponds to the return value
    /// of [`OsStr::as_encoded_bytes`].
    ///
    /// The range's start and end must lie on valid `OsStr` boundaries.
    /// A valid `OsStr` boundary is one of:
    /// - The start of the string
    /// - The end of the string
    /// - Immediately before a valid non-empty UTF-8 substring
    /// - Immediately after a valid non-empty UTF-8 substring
    ///
    /// # Panics
    ///
    /// Panics if `range` does not lie on valid `OsStr` boundaries or if it
    /// exceeds the end of the string.
    ///
    /// # Example
    ///
    /// ```
    /// use xrmt_stx::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("foo=bar");
    /// let bytes = os_str.as_encoded_bytes();
    /// if let Some(index) = bytes.iter().position(|b| *b == b'=') {
    ///     let key = os_str.slice_encoded_bytes(..index);
    ///     let value = os_str.slice_encoded_bytes(index + 1..);
    ///     assert_eq!(key, "foo");
    ///     assert_eq!(value, "bar");
    /// }
    /// ```
    #[inline]
    pub fn slice_encoded_bytes<T: RangeBounds<usize>>(&self, r: T) -> &OsStr {
        unsafe { transmute(&self.0[range(r, ..self.0.len())]) }
    }

    /// Converts a <code>[Box]<[OsStr]></code> into an [`OsString`] without
    /// copying or allocating.
    #[inline]
    pub fn into_os_string(v: Box<OsStr>) -> OsString {
        unsafe { OsString(Box::from_raw(Box::into_raw(v) as *mut OsStr as *mut [u8]).into_vec()) }
    }

    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }
    #[inline]
    pub(crate) fn to_rc(&self) -> Rc<OsStr> {
        unsafe { Rc::from_raw(Rc::into_raw(Rc::from_raw(&self.0)) as *const OsStr) }
    }
    #[inline]
    pub(crate) fn to_arc(&self) -> Arc<OsStr> {
        unsafe { Arc::from_raw(Arc::into_raw(Arc::from_raw(&self.0)) as *const OsStr) }
    }
    #[inline]
    pub(crate) fn to_box(&self) -> Box<OsStr> {
        unsafe { transmute(self.0.to_vec().into_boxed_slice()) }
    }
    #[inline]
    pub(crate) fn from_slice(v: &[u8]) -> &OsStr {
        unsafe { transmute(v) }
    }
    #[inline]
    pub(crate) fn last(&self, func: impl FnOnce(&u8) -> bool) -> bool {
        self.0.last().map_or(false, func)
    }
    #[inline]
    pub(crate) fn first(&self, func: impl FnOnce(&u8) -> bool) -> bool {
        self.0.first().map_or(false, func)
    }

    #[inline]
    fn from_inner_mut(v: &mut [u8]) -> &mut OsStr {
        unsafe { &mut *(v as *mut [u8] as *mut OsStr) }
    }
}
impl OsString {
    /// Constructs a new empty `OsString`.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// let os_string = OsString::new();
    /// ```
    #[inline]
    pub fn new() -> OsString {
        OsString(Vec::new())
    }
    /// Creates a new `OsString` with at least the given capacity.
    ///
    /// The string will be able to hold at least `capacity` length units of
    /// other OS strings without reallocating. This method is allowed to
    /// allocate for more units than `capacity`. If `capacity` is 0, the
    /// string will not allocate.
    ///
    /// See the main `OsString` documentation information about encoding and
    /// capacity units.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// let mut os_string = OsString::with_capacity(10);
    /// let capacity = os_string.capacity();
    ///
    /// // This push is done without reallocating
    /// os_string.push("foo");
    ///
    /// assert_eq!(capacity, os_string.capacity());
    /// ```
    #[inline]
    pub fn with_capacity(len: usize) -> OsString {
        OsString(Vec::with_capacity(len))
    }

    /// Converts bytes to an `OsString` without checking that the bytes contains
    /// valid [`OsStr`]-encoded data.
    ///
    /// The byte encoding is an unspecified, platform-specific,
    /// self-synchronizing superset of UTF-8. By being a self-synchronizing
    /// superset of UTF-8, this encoding is also a superset of 7-bit ASCII.
    ///
    /// See the [module's toplevel documentation about conversions][conversions]
    /// for safe, cross-platform [conversions] from/to native
    /// representations.
    ///
    /// # Safety
    ///
    /// As the encoding is unspecified, callers must pass in bytes that
    /// originated as a mixture of validated UTF-8 and bytes from
    /// [`OsStr::as_encoded_bytes`] from within the same Rust version
    /// built for the same target platform.  For example, reconstructing an
    /// `OsString` from bytes sent over the network or stored in a file will
    /// likely violate these safety rules.
    ///
    /// Due to the encoding being self-synchronizing, the bytes from
    /// [`OsStr::as_encoded_bytes`] can be split either immediately before
    /// or immediately after any valid non-empty UTF-8 substring.
    ///
    /// # Example
    ///
    /// ```
    /// use xrmt_stx::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("Mary had a little lamb");
    /// let bytes = os_str.as_encoded_bytes();
    /// let words = bytes.split(|b| *b == b' ');
    /// let words: Vec<&OsStr> = words.map(|word| {
    ///     // SAFETY:
    ///     // - Each `word` only contains content that originated from `OsStr::as_encoded_bytes`
    ///     // - Only split with ASCII whitespace which is a non-empty UTF-8 substring
    ///     unsafe { OsStr::from_encoded_bytes_unchecked(word) }
    /// }).collect();
    /// ```
    ///
    /// [conversions]: crate::ffi#conversions
    #[inline]
    pub unsafe fn from_encoded_bytes_unchecked(bytes: Vec<u8>) -> OsString {
        OsString(bytes)
    }

    /// Truncates the `OsString` to zero length.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// let mut os_string = OsString::from("foo");
    /// assert_eq!(&os_string, "foo");
    ///
    /// os_string.clear();
    /// assert_eq!(&os_string, "");
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }
    /// Returns the capacity this `OsString` can hold without reallocating.
    ///
    /// See the main `OsString` documentation information about encoding and
    /// capacity units.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// let os_string = OsString::with_capacity(10);
    /// assert!(os_string.capacity() >= 10);
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
    /// Shrinks the capacity of the `OsString` to match its length.
    ///
    /// See the main `OsString` documentation information about encoding and
    /// capacity units.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// let mut s = OsString::from("foo");
    ///
    /// s.reserve(100);
    /// assert!(s.capacity() >= 100);
    ///
    /// s.shrink_to_fit();
    /// assert_eq!(3, s.capacity());
    /// ```
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit()
    }
    /// Converts to an [`OsStr`] slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::{OsString, OsStr};
    ///
    /// let os_string = OsString::from("foo");
    /// let os_str = OsStr::new("foo");
    /// assert_eq!(os_string.as_os_str(), os_str);
    /// ```
    #[inline]
    pub fn as_os_str(&self) -> &OsStr {
        self
    }
    /// Reserves capacity for at least `additional` more capacity to be inserted
    /// in the given `OsString`. Does nothing if the capacity is
    /// already sufficient.
    ///
    /// The collection may reserve more space to speculatively avoid frequent
    /// reallocations.
    ///
    /// See the main `OsString` documentation information about encoding and
    /// capacity units.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// let mut s = OsString::new();
    /// s.reserve(10);
    /// assert!(s.capacity() >= 10);
    /// ```
    #[inline]
    pub fn reserve(&mut self, len: usize) {
        self.0.reserve(len)
    }
    /// Truncate the `OsString` to the specified length.
    ///
    /// # Panics
    /// Panics if `len` does not lie on a valid `OsStr` boundary
    /// (as described in [`OsStr::slice_encoded_bytes`]).
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        self.0.truncate(len);
    }
    /// Consumes and leaks the `OsString`, returning a mutable reference to the
    /// contents, `&'a mut OsStr`.
    ///
    /// The caller has free choice over the returned lifetime, including
    /// 'static. Indeed, this function is ideally used for data that lives
    /// for the remainder of the program’s life, as dropping the returned
    /// reference will cause a memory leak.
    ///
    /// It does not reallocate or shrink the `OsString`, so the leaked
    /// allocation may include unused capacity that is not part of the
    /// returned slice. If you want to discard excess capacity, call
    /// [`into_boxed_os_str`], and then [`Box::leak`] instead. However, keep
    /// in mind that trimming the capacity may result in a reallocation and
    /// copy.
    ///
    /// [`into_boxed_os_str`]: Self::into_boxed_os_str
    #[inline]
    pub fn leak<'a>(self) -> &'a mut OsStr {
        OsStr::from_inner_mut(self.0.leak())
    }
    /// Shrinks the capacity of the `OsString` with a lower bound.
    ///
    /// The capacity will remain at least as large as both the length
    /// and the supplied value.
    ///
    /// If the current capacity is less than the lower limit, this is a no-op.
    ///
    /// See the main `OsString` documentation information about encoding and
    /// capacity units.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// let mut s = OsString::from("foo");
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
        self.0.shrink_to(len)
    }
    /// Converts the `OsString` into a byte vector.  To convert the byte vector
    /// back into an `OsString`, use the
    /// [`OsString::from_encoded_bytes_unchecked`] function.
    ///
    /// The byte encoding is an unspecified, platform-specific,
    /// self-synchronizing superset of UTF-8. By being a self-synchronizing
    /// superset of UTF-8, this encoding is also a superset of 7-bit ASCII.
    ///
    /// Note: As the encoding is unspecified, any sub-slice of bytes that is not
    /// valid UTF-8 should be treated as opaque and only comparable within
    /// the same Rust version built for the same target platform.  For
    /// example, sending the bytes over the network or storing it in a file
    /// will likely result in incompatible data.  See [`OsString`] for more
    /// encoding details and [`xrmt_stx::ffi`] for platform-specific, specified
    /// conversions.
    ///
    /// [`xrmt_stx::ffi`]: crate::ffi
    #[inline]
    pub fn into_encoded_bytes(self) -> Vec<u8> {
        self.0
    }
    /// Reserves the minimum capacity for at least `additional` more capacity to
    /// be inserted in the given `OsString`. Does nothing if the capacity is
    /// already sufficient.
    ///
    /// Note that the allocator may give the collection more space than it
    /// requests. Therefore, capacity can not be relied upon to be precisely
    /// minimal. Prefer [`reserve`] if future insertions are expected.
    ///
    /// [`reserve`]: OsString::reserve
    ///
    /// See the main `OsString` documentation information about encoding and
    /// capacity units.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// let mut s = OsString::new();
    /// s.reserve_exact(10);
    /// assert!(s.capacity() >= 10);
    /// ```
    #[inline]
    pub fn reserve_exact(&mut self, len: usize) {
        self.0.reserve_exact(len)
    }
    /// Converts this `OsString` into a boxed [`OsStr`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::{OsString, OsStr};
    ///
    /// let s = OsString::from("hello");
    ///
    /// let b: Box<OsStr> = s.into_boxed_os_str();
    /// ```
    #[inline]
    pub fn into_boxed_os_str(self) -> Box<OsStr> {
        unsafe { Box::from_raw(Box::into_raw(self.0.into_boxed_slice()) as *mut OsStr) }
    }
    /// Extends the string with the given <code>&[OsStr]</code> slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// let mut os_string = OsString::from("foo");
    /// os_string.push("bar");
    /// assert_eq!(&os_string, "foobar");
    /// ```
    #[inline]
    pub fn push<T: AsRef<OsStr>>(&mut self, s: T) {
        self.0.extend_from_slice(&s.as_ref().0)
    }
    /// Converts the `OsString` into a [`String`] if it contains valid Unicode
    /// data.
    ///
    /// On failure, ownership of the original `OsString` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    ///
    /// let os_string = OsString::from("foo");
    /// let string = os_string.into_string();
    /// assert_eq!(string, Ok(String::from("foo")));
    /// ```
    #[inline]
    pub fn into_string(self) -> Result<String, OsString> {
        String::from_utf8(self.0).map_err(|e| OsString(e.into_bytes()))
    }
    /// Tries to reserve capacity for at least `additional` more length units
    /// in the given `OsString`. The string may reserve more space to
    /// speculatively avoid frequent reallocations. After calling
    /// `try_reserve`, capacity will be greater than or equal to `self.len()
    /// + additional` if it returns `Ok(())`. Does nothing if capacity is
    /// already sufficient. This method preserves the contents even if an
    /// error occurs.
    ///
    /// See the main `OsString` documentation information about encoding and
    /// capacity units.
    ///
    /// # Errors
    ///
    /// If the capacity overflows, or the allocator reports a failure, then an
    /// error is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::{OsStr, OsString};
    /// use xrmt_stx::collections::TryReserveError;
    ///
    /// fn process_data(data: &str) -> Result<OsString, TryReserveError> {
    ///     let mut s = OsString::new();
    ///
    ///     // Pre-reserve the memory, exiting if we can't
    ///     s.try_reserve(OsStr::new(data).len())?;
    ///
    ///     // Now we know this can't OOM in the middle of our complex work
    ///     s.push(data);
    ///
    ///     Ok(s)
    /// }
    /// # process_data("123").expect("why is the test harness OOMing on 3 bytes?");
    /// ```
    #[inline]
    pub fn try_reserve(&mut self, len: usize) -> Result<(), TryReserveError> {
        self.0.try_reserve(len)
    }
    /// Tries to reserve the minimum capacity for at least `additional`
    /// more length units in the given `OsString`. After calling
    /// `try_reserve_exact`, capacity will be greater than or equal to
    /// `self.len() + additional` if it returns `Ok(())`.
    /// Does nothing if the capacity is already sufficient.
    ///
    /// Note that the allocator may give the `OsString` more space than it
    /// requests. Therefore, capacity can not be relied upon to be precisely
    /// minimal. Prefer [`try_reserve`] if future insertions are expected.
    ///
    /// [`try_reserve`]: OsString::try_reserve
    ///
    /// See the main `OsString` documentation information about encoding and
    /// capacity units.
    ///
    /// # Errors
    ///
    /// If the capacity overflows, or the allocator reports a failure, then an
    /// error is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::{OsStr, OsString};
    /// use xrmt_stx::collections::TryReserveError;
    ///
    /// fn process_data(data: &str) -> Result<OsString, TryReserveError> {
    ///     let mut s = OsString::new();
    ///
    ///     // Pre-reserve the memory, exiting if we can't
    ///     s.try_reserve_exact(OsStr::new(data).len())?;
    ///
    ///     // Now we know this can't OOM in the middle of our complex work
    ///     s.push(data);
    ///
    ///     Ok(s)
    /// }
    /// # process_data("123").expect("why is the test harness OOMing on 3 bytes?");
    /// ```
    #[inline]
    pub fn try_reserve_exact(&mut self, len: usize) -> Result<(), TryReserveError> {
        self.0.try_reserve_exact(len)
    }

    #[inline]
    pub(crate) fn into_rc(self) -> Rc<OsStr> {
        unsafe { Rc::from_raw(Rc::into_raw(Rc::from_raw(self.0.as_slice())) as *const OsStr) }
    }
    #[inline]
    pub(crate) fn into_arc(self) -> Arc<OsStr> {
        unsafe { Arc::from_raw(Arc::into_raw(Arc::from_raw(self.0.as_slice())) as *const OsStr) }
    }
    #[inline]
    pub(crate) fn as_mut_vec(&mut self) -> &mut Vec<u8> {
        &mut self.0
    }
}

impl Eq for OsStr {}
impl Ord for OsStr {
    #[inline]
    fn cmp(&self, other: &OsStr) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl Hash for OsStr {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.hash(h)
    }
}
impl ToOwned for OsStr {
    type Owned = OsString;

    #[inline]
    fn to_owned(&self) -> OsString {
        self.to_os_string()
    }
    #[inline]
    fn clone_into(&self, v: &mut OsString) {
        self.0.clone_into(&mut v.0)
    }
}
impl PartialEq for OsStr {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        self.0.eq(&other.0)
    }
}
impl PartialOrd for OsStr {
    #[inline]
    fn lt(&self, other: &OsStr) -> bool {
        self.0.lt(&other.0)
    }
    #[inline]
    fn le(&self, other: &OsStr) -> bool {
        self.0.le(&other.0)
    }
    #[inline]
    fn gt(&self, other: &OsStr) -> bool {
        self.0.gt(&other.0)
    }
    #[inline]
    fn ge(&self, other: &OsStr) -> bool {
        self.0.ge(&other.0)
    }
    #[inline]
    fn partial_cmp(&self, other: &OsStr) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl AsRef<OsStr> for OsStr {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self
    }
}
impl PartialEq<str> for OsStr {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        *self == *OsStr::new(other)
    }
}
impl PartialOrd<str> for OsStr {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<Ordering> {
        self.partial_cmp(OsStr::new(other))
    }
}
impl<'a> Default for &'a OsStr {
    #[inline]
    fn default() -> &'a OsStr {
        OsStr::new("")
    }
}

impl<'a, 'b> PartialEq<OsString> for OsStr {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a, 'b> PartialOrd<OsString> for OsStr {
    #[inline]
    fn partial_cmp(&self, other: &OsString) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<'a, 'b> PartialEq<OsString> for &'a OsStr {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a, 'b> PartialOrd<OsString> for &'a OsStr {
    #[inline]
    fn partial_cmp(&self, other: &OsString) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<'a, 'b> PartialEq<Cow<'a, OsStr>> for OsStr {
    #[inline]
    fn eq(&self, other: &Cow<'a, OsStr>) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a, 'b> PartialOrd<Cow<'a, OsStr>> for OsStr {
    #[inline]
    fn partial_cmp(&self, other: &Cow<'a, OsStr>) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<'a, 'b> PartialEq<Cow<'a, OsStr>> for &'b OsStr {
    #[inline]
    fn eq(&self, other: &Cow<'a, OsStr>) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a, 'b> PartialOrd<Cow<'a, OsStr>> for &'b OsStr {
    #[inline]
    fn partial_cmp(&self, other: &Cow<'a, OsStr>) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<'a, 'b> PartialEq<OsStr> for Cow<'a, OsStr> {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a, 'b> PartialOrd<OsStr> for Cow<'a, OsStr> {
    #[inline]
    fn partial_cmp(&self, other: &OsStr) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<'a, 'b> PartialEq<OsString> for Cow<'a, OsStr> {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a, 'b> PartialOrd<OsString> for Cow<'a, OsStr> {
    #[inline]
    fn partial_cmp(&self, other: &OsString) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<'a, 'b> PartialEq<&'b OsStr> for Cow<'a, OsStr> {
    #[inline]
    fn eq(&self, other: &&'b OsStr) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a, 'b> PartialOrd<&'b OsStr> for Cow<'a, OsStr> {
    #[inline]
    fn partial_cmp(&self, other: &&'b OsStr) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Eq for OsString {}
impl Ord for OsString {
    #[inline]
    fn cmp(&self, other: &OsString) -> Ordering {
        (&**self).cmp(&**other)
    }
}
impl Hash for OsString {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        (&**self).hash(h)
    }
}
impl Write for OsString {
    #[inline]
    fn write_str(&mut self, s: &str) -> FmtResult {
        self.push(s);
        Ok(())
    }
}
impl Clone for OsString {
    #[inline]
    fn clone(&self) -> Self {
        OsString(self.0.clone())
    }
    #[inline]
    fn clone_from(&mut self, v: &OsString) {
        self.0.clone_from(&v.0)
    }
}
impl Deref for OsString {
    type Target = OsStr;

    #[inline]
    fn deref(&self) -> &OsStr {
        &self[..]
    }
}
impl FromStr for OsString {
    type Err = Infallible;

    #[inline]
    fn from_str(v: &str) -> Result<OsString, Infallible> {
        Ok(OsString::from(v))
    }
}
impl Default for OsString {
    #[inline]
    fn default() -> OsString {
        OsString::new()
    }
}
impl DerefMut for OsString {
    #[inline]
    fn deref_mut(&mut self) -> &mut OsStr {
        &mut self[..]
    }
}
impl PartialEq for OsString {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        (&**self).eq(&**other)
    }
}
impl PartialOrd for OsString {
    #[inline]
    fn lt(&self, other: &OsString) -> bool {
        (&**self).lt(&**other)
    }
    #[inline]
    fn le(&self, other: &OsString) -> bool {
        (&**self).le(&**other)
    }
    #[inline]
    fn gt(&self, other: &OsString) -> bool {
        (&**self).gt(&**other)
    }
    #[inline]
    fn ge(&self, other: &OsString) -> bool {
        (&**self).ge(&**other)
    }
    #[inline]
    fn partial_cmp(&self, other: &OsString) -> Option<Ordering> {
        (&**self).partial_cmp(&**other)
    }
}
impl AsRef<OsStr> for OsString {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self
    }
}
impl From<String> for OsString {
    #[inline]
    fn from(v: String) -> OsString {
        OsString(v.into_bytes())
    }
}
impl Borrow<OsStr> for OsString {
    #[inline]
    fn borrow(&self) -> &OsStr {
        &self[..]
    }
}
impl PartialEq<str> for OsString {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        (&**self).eq(other)
    }
}
impl PartialOrd<str> for OsString {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<Ordering> {
        (&**self).partial_cmp(other)
    }
}
impl PartialEq<&str> for OsString {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        (**self).eq(&**other)
    }
}
impl Index<RangeFull> for OsString {
    type Output = OsStr;

    #[inline]
    fn index(&self, _i: RangeFull) -> &OsStr {
        OsStr::from_slice(self.0.as_slice())
    }
}
impl From<Box<OsStr>> for OsString {
    #[inline]
    fn from(v: Box<OsStr>) -> OsString {
        OsStr::into_os_string(v)
    }
}
impl Extend<OsString> for OsString {
    #[inline]
    fn extend<T: IntoIterator<Item = OsString>>(&mut self, i: T) {
        let x = i.into_iter();
        match x.size_hint() {
            (_, Some(v)) => self.reserve(v),
            (v, _) => self.reserve(v),
        }
        for v in x {
            self.push(&v);
        }
    }
}
impl IndexMut<RangeFull> for OsString {
    #[inline]
    fn index_mut(&mut self, _i: RangeFull) -> &mut OsStr {
        OsStr::from_inner_mut(self.0.as_mut_slice())
    }
}
impl<'a> Extend<&'a OsStr> for OsString {
    #[inline]
    fn extend<T: IntoIterator<Item = &'a OsStr>>(&mut self, i: T) {
        let x = i.into_iter();
        match x.size_hint() {
            (_, Some(v)) => self.reserve(v),
            (v, _) => self.reserve(v),
        }
        for v in x {
            self.push(v);
        }
    }
}
impl FromIterator<OsString> for OsString {
    #[inline]
    fn from_iter<I: IntoIterator<Item = OsString>>(i: I) -> OsString {
        let mut x = i.into_iter();
        match x.next() {
            None => OsString::new(),
            Some(mut b) => {
                b.extend(x);
                b
            },
        }
    }
}
impl<'a> From<Cow<'a, OsStr>> for OsString {
    #[inline]
    fn from(v: Cow<'a, OsStr>) -> Self {
        v.into_owned()
    }
}
impl<'a> Extend<Cow<'a, OsStr>> for OsString {
    #[inline]
    fn extend<T: IntoIterator<Item = Cow<'a, OsStr>>>(&mut self, i: T) {
        let x = i.into_iter();
        match x.size_hint() {
            (_, Some(v)) => self.reserve(v),
            (v, _) => self.reserve(v),
        }
        for v in x {
            self.push(v);
        }
    }
}
impl<'a> FromIterator<&'a OsStr> for OsString {
    #[inline]
    fn from_iter<I: IntoIterator<Item = &'a OsStr>>(i: I) -> OsString {
        let x = i.into_iter();
        let mut b = Self::new();
        match x.size_hint() {
            (_, Some(v)) => b.reserve(v),
            (v, _) => b.reserve(v),
        }
        for v in x {
            b.push(v);
        }
        b
    }
}
impl<'a> FromIterator<Cow<'a, OsStr>> for OsString {
    #[inline]
    fn from_iter<I: IntoIterator<Item = Cow<'a, OsStr>>>(i: I) -> OsString {
        let mut x = i.into_iter();
        match x.next() {
            None => OsString::new(),
            Some(Cow::Owned(mut b)) => {
                b.extend(x);
                b
            },
            Some(Cow::Borrowed(b)) => {
                let mut v = OsString::from(b);
                v.extend(x);
                v
            },
        }
    }
}
impl<T: ?Sized + AsRef<OsStr>> From<&T> for OsString {
    #[inline]
    fn from(v: &T) -> OsString {
        v.as_ref().to_os_string()
    }
}

impl<'a, 'b> PartialEq<OsStr> for OsString {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a, 'b> PartialOrd<OsStr> for OsString {
    #[inline]
    fn partial_cmp(&self, other: &OsStr) -> Option<Ordering> {
        self.0.as_slice().partial_cmp(&other.0)
    }
}
impl<'a, 'b> PartialEq<&'a OsStr> for OsString {
    #[inline]
    fn eq(&self, other: &&'a OsStr) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a, 'b> PartialOrd<&'a OsStr> for OsString {
    #[inline]
    fn partial_cmp(&self, other: &&'a OsStr) -> Option<Ordering> {
        self.0.as_slice().partial_cmp(&other.0)
    }
}
impl<'a, 'b> PartialEq<Cow<'a, OsStr>> for OsString {
    #[inline]
    fn eq(&self, other: &Cow<'a, OsStr>) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a, 'b> PartialOrd<Cow<'a, OsStr>> for OsString {
    #[inline]
    fn partial_cmp(&self, other: &Cow<'a, OsStr>) -> Option<Ordering> {
        self.0.as_slice().partial_cmp(&other.0)
    }
}

impl AsRef<OsStr> for str {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        unsafe { transmute(self) }
    }
}
impl PartialEq<OsStr> for str {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        (*other).eq(OsStr::new(self))
    }
}
impl PartialEq<OsString> for str {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        (&**other).eq(self)
    }
}
impl<'a> TryFrom<&'a OsStr> for &'a str {
    type Error = Utf8Error;

    #[inline]
    fn try_from(v: &'a OsStr) -> Result<&'a str, Utf8Error> {
        from_utf8(&v.0)
    }
}
impl<'a> PartialEq<OsString> for &'a str {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        (**other).eq(&**self)
    }
}

impl AsRef<OsStr> for String {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        (&**self).as_ref()
    }
}

impl Clone for Box<OsStr> {
    #[inline]
    fn clone(&self) -> Box<OsStr> {
        self.to_os_string().into_boxed_os_str()
    }
}
impl Default for Box<OsStr> {
    #[inline]
    fn default() -> Box<OsStr> {
        let v: Box<[u8]> = Default::default();
        unsafe { Box::from_raw(Box::into_raw(v) as *mut OsStr) }
    }
}
impl From<&OsStr> for Box<OsStr> {
    #[inline]
    fn from(v: &OsStr) -> Box<OsStr> {
        v.to_box()
    }
}
impl From<OsString> for Box<OsStr> {
    #[inline]
    fn from(v: OsString) -> Box<OsStr> {
        v.into_boxed_os_str()
    }
}
impl From<&mut OsStr> for Box<OsStr> {
    #[inline]
    fn from(v: &mut OsStr) -> Box<OsStr> {
        unsafe { transmute(v.0.to_vec().into_boxed_slice()) }
    }
}
impl From<Cow<'_, OsStr>> for Box<OsStr> {
    #[inline]
    fn from(v: Cow<'_, OsStr>) -> Box<OsStr> {
        match v {
            Cow::Owned(s) => Box::from(s),
            Cow::Borrowed(s) => Box::from(s),
        }
    }
}

impl From<&OsStr> for Rc<OsStr> {
    #[inline]
    fn from(v: &OsStr) -> Rc<OsStr> {
        v.to_rc()
    }
}
impl From<OsString> for Rc<OsStr> {
    #[inline]
    fn from(v: OsString) -> Rc<OsStr> {
        v.into_rc()
    }
}
impl From<&mut OsStr> for Rc<OsStr> {
    #[inline]
    fn from(v: &mut OsStr) -> Rc<OsStr> {
        Rc::from(&*v)
    }
}

impl From<&OsStr> for Arc<OsStr> {
    #[inline]
    fn from(v: &OsStr) -> Arc<OsStr> {
        v.to_arc()
    }
}
impl From<OsString> for Arc<OsStr> {
    #[inline]
    fn from(v: OsString) -> Arc<OsStr> {
        v.into_arc()
    }
}
impl From<&mut OsStr> for Arc<OsStr> {
    #[inline]
    fn from(v: &mut OsStr) -> Arc<OsStr> {
        Arc::from(&*v)
    }
}

impl<'a> From<OsString> for Cow<'a, OsStr> {
    #[inline]
    fn from(v: OsString) -> Cow<'a, OsStr> {
        Cow::Owned(v)
    }
}
impl<'a> From<&'a OsStr> for Cow<'a, OsStr> {
    #[inline]
    fn from(v: &'a OsStr) -> Cow<'a, OsStr> {
        Cow::Borrowed(v)
    }
}
impl<'a> From<&'a OsString> for Cow<'a, OsStr> {
    #[inline]
    fn from(v: &'a OsString) -> Cow<'a, OsStr> {
        Cow::Borrowed(v.as_os_str())
    }
}

impl<T: Borrow<OsStr>> Join<&OsStr> for [T] {
    type Output = OsString;

    fn join(slice: &Self, sep: &OsStr) -> OsString {
        let Some((f, s)) = slice.split_first() else {
            return OsString::new();
        };
        let r = f.borrow().to_owned();
        s.iter().fold(r, |mut a, b| {
            a.push(sep);
            a.push(b.borrow());
            a
        })
    }
}

impl From<Char> for OsString {
    #[inline]
    fn from(v: Char) -> OsString {
        v.into_string().into()
    }
}
impl<'a> From<CharPtr<'a>> for OsString {
    #[inline]
    fn from(v: CharPtr<'a>) -> OsString {
        v.into_string().into()
    }
}
impl<'a> From<CharLike<'a>> for OsString {
    #[inline]
    fn from(v: CharLike<'a>) -> OsString {
        v.into_string().into()
    }
}
impl<'a> From<CharSlice<'a>> for OsString {
    #[inline]
    fn from(v: CharSlice<'a>) -> OsString {
        v.into_string().into()
    }
}

impl From<WChar> for OsString {
    #[inline]
    fn from(v: WChar) -> OsString {
        v.into_string().into()
    }
}
impl<'a> From<WCharPtr<'a>> for OsString {
    #[inline]
    fn from(v: WCharPtr<'a>) -> OsString {
        v.into_string().into()
    }
}
impl<'a> From<WCharLike<'a>> for OsString {
    #[inline]
    fn from(v: WCharLike<'a>) -> OsString {
        v.into_string().into()
    }
}
impl<'a> From<WCharSlice<'a>> for OsString {
    #[inline]
    fn from(v: WCharSlice<'a>) -> OsString {
        v.into_string().into()
    }
}

impl From<&OsStr> for Char {
    #[inline]
    fn from(v: &OsStr) -> Char {
        Char::from(v.as_bytes())
    }
}
impl From<OsString> for Char {
    #[inline]
    fn from(v: OsString) -> Char {
        Char::from(v.as_bytes())
    }
}
impl From<&OsString> for Char {
    #[inline]
    fn from(v: &OsString) -> Char {
        Char::from(v.as_bytes())
    }
}

impl<'a> From<&'a OsStr> for CharPtr<'a> {
    #[inline]
    fn from(v: &'a OsStr) -> CharPtr<'a> {
        CharPtr::from(v.as_bytes())
    }
}
impl<'a> From<&'a OsString> for CharPtr<'a> {
    #[inline]
    fn from(v: &'a OsString) -> CharPtr<'a> {
        CharPtr::from(v.as_bytes())
    }
}

impl<'a> From<OsString> for CharLike<'a> {
    #[inline]
    fn from(v: OsString) -> CharLike<'a> {
        CharLike::Owned(Char::from(v))
    }
}
impl<'a> From<&'a OsStr> for CharLike<'a> {
    #[inline]
    fn from(v: &'a OsStr) -> CharLike<'a> {
        CharLike::Slice(CharSlice::from(v))
    }
}
impl<'a> From<&'a OsString> for CharLike<'a> {
    #[inline]
    fn from(v: &'a OsString) -> CharLike<'a> {
        CharLike::Slice(CharSlice::from(v))
    }
}

impl<'a> From<&'a OsStr> for CharSlice<'a> {
    #[inline]
    fn from(v: &'a OsStr) -> CharSlice<'a> {
        CharSlice::from(v.as_bytes())
    }
}
impl<'a> From<&'a OsString> for CharSlice<'a> {
    #[inline]
    fn from(v: &'a OsString) -> CharSlice<'a> {
        CharSlice::from(v.as_bytes())
    }
}

impl From<&OsStr> for WChar {
    #[inline]
    fn from(v: &OsStr) -> WChar {
        WChar::from(v.as_bytes())
    }
}
impl From<OsString> for WChar {
    #[inline]
    fn from(v: OsString) -> WChar {
        WChar::from(v.as_bytes())
    }
}
impl From<&OsString> for WChar {
    #[inline]
    fn from(v: &OsString) -> WChar {
        WChar::from(v.as_bytes())
    }
}

impl<'a> From<&OsStr> for WCharLike<'a> {
    #[inline]
    fn from(v: &OsStr) -> WCharLike<'a> {
        WCharLike::Owned(WChar::from(v))
    }
}
impl<'a> From<OsString> for WCharLike<'a> {
    #[inline]
    fn from(v: OsString) -> WCharLike<'a> {
        WCharLike::Owned(WChar::from(v))
    }
}
impl<'a> From<&OsString> for WCharLike<'a> {
    #[inline]
    fn from(v: &OsString) -> WCharLike<'a> {
        WCharLike::Owned(WChar::from(v))
    }
}

impl From<Fiber> for OsString {
    #[inline]
    fn from(v: Fiber) -> OsString {
        OsString(v.into_bytes())
    }
}

impl<A: Allocator> AsRef<OsStr> for Fiber<A> {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        OsStr::new(self)
    }
}

pub mod c_str {
    extern crate alloc;
    extern crate core;

    pub use alloc::ffi::c_str::*;
    pub use core::ffi::c_str::*;
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Formatter, Result};
    use core::str::from_utf8_unchecked;

    use crate::ffi::{OsStr, OsString};

    impl Debug for OsStr {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            f.write_str(unsafe { from_utf8_unchecked(&self.0) })
        }
    }
    impl Debug for OsString {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            f.write_str(unsafe { from_utf8_unchecked(&self.0) })
        }
    }
}
