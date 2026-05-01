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

//! Cross-platform path manipulation.
//!
//! This module provides two types, [`PathBuf`] and [`Path`] (akin to [`String`]
//! and [`str`]), for working with paths abstractly. These types are thin
//! wrappers around [`OsString`] and [`OsStr`] respectively, meaning that they
//! work directly on strings according to the local platform's path syntax.
//!
//! Paths can be parsed into [`Component`]s by iterating over the structure
//! returned by the [`components`] method on [`Path`]. [`Component`]s roughly
//! correspond to the substrings between path separators (`/` or `\`). You can
//! reconstruct an equivalent path from components with the [`push`] method on
//! [`PathBuf`]; note that the paths may differ syntactically by the
//! normalization described in the documentation for the [`components`] method.
//!
//! ## Case sensitivity
//!
//! Unless otherwise indicated path methods that do not access the filesystem,
//! such as [`Path::starts_with`] and [`Path::ends_with`], are case sensitive no
//! matter the platform or filesystem. An exception to this is made for Windows
//! drive letters.
//!
//! ## Simple usage
//!
//! Path manipulation includes both parsing components from slices and building
//! new owned paths.
//!
//! To parse a path, you can create a [`Path`] slice from a [`str`]
//! slice and start asking questions:
//!
//! ```
//! use xrmt_stx::path::Path;
//! use xrmt_stx::ffi::OsStr;
//!
//! let path = Path::new("/tmp/foo/bar.txt");
//!
//! let parent = path.parent();
//! assert_eq!(parent, Some(Path::new("/tmp/foo")));
//!
//! let file_stem = path.file_stem();
//! assert_eq!(file_stem, Some(OsStr::new("bar")));
//!
//! let extension = path.extension();
//! assert_eq!(extension, Some(OsStr::new("txt")));
//! ```
//!
//! To build or modify paths, use [`PathBuf`]:
//!
//! ```
//! use xrmt_stx::path::PathBuf;
//!
//! // This way works...
//! let mut path = PathBuf::from("c:\\");
//!
//! path.push("windows");
//! path.push("system32");
//!
//! path.set_extension("dll");
//!
//! // ... but push is best used if you don't know everything up
//! // front. If you do, this way is better:
//! let path: PathBuf = ["c:\\", "windows", "system32.dll"].iter().collect();
//! ```
//!
//! [`components`]: Path::components
//! [`push`]: PathBuf::push

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
use alloc::string::String;
use alloc::sync::Arc;
use core::alloc::Allocator;
use core::clone::Clone;
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::{AsRef, From, Infallible, Into};
use core::default::Default;
use core::error::Error;
use core::fmt::{Debug, Display, Formatter};
use core::hash::{Hash, Hasher};
use core::iter::{DoubleEndedIterator, Extend, FromIterator, FusedIterator, IntoIterator, Iterator};
use core::marker::{Copy, Sized};
use core::mem::discriminant;
use core::ops::{Deref, DerefMut, FnOnce};
use core::option::Option::{self, None, Some};
use core::ptr::replace;
use core::result::Result::{self, Err, Ok};
use core::str::FromStr;

use xrmt_data::Fiber;
use xrmt_winapi::path_normalize;
use xrmt_winapi::structs::{Char, CharLike, CharPtr, CharSlice, StringLikeU16, StringLikeU8, WChar, WCharLike, WCharPtr, WCharSlice};

use crate::ffi::{OsStr, OsString};
use crate::fs::{canonicalize, exists, metadata, read_dir, read_link, symlink_metadata, Metadata, ReadDir};
use crate::io::{FmtResult, IoResult};

/// The primary separator of path components for the current platform.
///
/// For example, `/` on Unix and `\` on Windows.
pub const MAIN_SEPARATOR: char = '\\';
/// The primary separator of path components for the current platform.
///
/// For example, `/` on Unix and `\` on Windows.
#[cfg(target_family = "windows")]
pub const MAIN_SEPARATOR_STR: &str = "\\";

const EXT: u8 = b'.';
const EXT_DOT: &str = ".";
const EXT_DOT_DOT: &str = "..";

const SEP_SLASH: u8 = b'\\';
const SEP_BACKSLASH: u8 = b'/';

const STATE_ROOT_SHOWN: u8 = 0x40u8;
const STATE_ROOT_PRESENT: u8 = 0x20u8;
const STATE_PREFIX_SHOWN: u8 = 0x80u8;

const SEP: u8 = SEP_SLASH;

/// Windows path prefixes, e.g., `C:` or `\\server\share`.
///
/// Windows uses a variety of path prefix styles, including references to drive
/// volumes (like `C:`), network shared folders (like `\\server\share`), and
/// others. In addition, some path prefixes are "verbatim" (i.e., prefixed with
/// `\\?\`), in which case `/` is *not* treated as a separator and essentially
/// no normalization is performed.
///
/// # Examples
///
/// ```
/// use xrmt_stx::path::{Component, Path, Prefix};
/// use xrmt_stx::path::Prefix::*;
/// use xrmt_stx::ffi::OsStr;
///
/// fn get_path_prefix(s: &str) -> Prefix<'_> {
///     let path = Path::new(s);
///     match path.components().next().unwrap() {
///         Component::Prefix(prefix_component) => prefix_component.kind(),
///         _ => panic!(),
///     }
/// }
///
/// # if cfg!(windows) {
/// assert_eq!(Verbatim(OsStr::new("pictures")),
///            get_path_prefix(r"\\?\pictures\kittens"));
/// assert_eq!(VerbatimUNC(OsStr::new("server"), OsStr::new("share")),
///            get_path_prefix(r"\\?\UNC\server\share"));
/// assert_eq!(VerbatimDisk(b'C'), get_path_prefix(r"\\?\c:\"));
/// assert_eq!(DeviceNS(OsStr::new("BrainInterface")),
///            get_path_prefix(r"\\.\BrainInterface"));
/// assert_eq!(UNC(OsStr::new("server"), OsStr::new("share")),
///            get_path_prefix(r"\\server\share"));
/// assert_eq!(Disk(b'C'), get_path_prefix(r"C:\Users\Rust\Pictures\Ferris"));
/// # }
/// ```
pub enum Prefix<'a> {
    /// Prefix `C:` for the given disk drive.
    Disk(u8),
    /// Verbatim disk prefix, e.g., `\\?\C:`.
    ///
    /// Verbatim disk prefixes consist of `\\?\` immediately followed by the
    /// drive letter and `:`.
    VerbatimDisk(u8),
    /// Verbatim prefix, e.g., `\\?\cat_pics`.
    ///
    /// Verbatim prefixes consist of `\\?\` immediately followed by the given
    /// component.
    Verbatim(&'a OsStr),
    /// Device namespace prefix, e.g., `\\.\COM42`.
    ///
    /// Device namespace prefixes consist of `\\.\` (possibly using `/`
    /// instead of `\`), immediately followed by the device name.
    DeviceNS(&'a OsStr),
    /// Prefix using Windows' _**U**niform **N**aming **C**onvention_, e.g.
    /// `\\server\share`.
    ///
    /// UNC prefixes consist of the server's hostname and a share name.
    UNC(&'a OsStr, &'a OsStr),
    /// Verbatim prefix using Windows' _**U**niform **N**aming **C**onvention_,
    /// e.g., `\\?\UNC\server\share`.
    ///
    /// Verbatim UNC prefixes consist of `\\?\UNC\` immediately followed by the
    /// server's hostname and a share name.
    VerbatimUNC(&'a OsStr, &'a OsStr),
}
/// A single component of a path.
///
/// A `Component` roughly corresponds to a substring between path separators
/// (`/` or `\`).
///
/// This `enum` is created by iterating over [`Components`], which in turn is
/// created by the [`components`](Path::components) method on [`Path`].
///
/// # Examples
///
/// ```rust
/// use xrmt_stx::path::{Component, Path};
///
/// let path = Path::new("/tmp/foo/bar.txt");
/// let components = path.components().collect::<Vec<_>>();
/// assert_eq!(&components, &[
///     Component::RootDir,
///     Component::Normal("tmp".as_ref()),
///     Component::Normal("foo".as_ref()),
///     Component::Normal("bar.txt".as_ref()),
/// ]);
/// ```
pub enum Component<'a> {
    /// A reference to the current directory, i.e., `.`.
    CurDir,
    /// The root directory component, appears after any prefix and before
    /// anything else.
    ///
    /// It represents a separator that designates that a path starts from root.
    RootDir,
    /// A reference to the parent directory, i.e., `..`.
    ParentDir,
    /// A normal component, e.g., `a` and `b` in `a/b`.
    ///
    /// This variant is the most common one, it represents references to files
    /// or directories.
    Normal(&'a OsStr),
    /// A Windows path prefix, e.g., `C:` or `\\server\share`.
    ///
    /// There is a large variety of prefix types, see [`Prefix`]'s documentation
    /// for more.
    ///
    /// Does not occur on Unix.
    Prefix(PrefixComponent<'a>),
}

/// A slice of a path (akin to [`str`]).
///
/// This type supports a number of operations for inspecting a path, including
/// breaking the path into its components (separated by `/` on Unix and by
/// either `/` or `\` on Windows), extracting the file name, determining whether
/// the path is absolute, and so on.
///
/// This is an *unsized* type, meaning that it must always be used behind a
/// pointer like `&` or [`Box`]. For an owned version of this type,
/// see [`PathBuf`].
///
/// More details about the overall approach can be found in
/// the [module documentation](self).
///
/// # Examples
///
/// ```
/// use xrmt_stx::path::Path;
/// use xrmt_stx::ffi::OsStr;
///
/// // Note: this example does work on Windows
/// let path = Path::new("./foo/bar.txt");
///
/// let parent = path.parent();
/// assert_eq!(parent, Some(Path::new("./foo")));
///
/// let file_stem = path.file_stem();
/// assert_eq!(file_stem, Some(OsStr::new("bar")));
///
/// let extension = path.extension();
/// assert_eq!(extension, Some(OsStr::new("txt")));
/// ```
// `Path::new` and `impl CloneToUninit for Path` current implementation relies
// on `Path` being layout-compatible with `OsStr`.
// However, `Path` layout is considered an implementation detail and must not be relied upon.
#[repr(transparent)]
pub struct Path(OsStr);
/// An iterator over the [`Component`]s of a [`Path`].
///
/// This `struct` is created by the [`components`] method on [`Path`].
/// See its documentation for more.
///
/// # Examples
///
/// ```
/// use xrmt_stx::path::Path;
///
/// let path = Path::new("/tmp/foo/bar.txt");
///
/// for component in path.components() {
///     println!("{component:?}");
/// }
/// ```
///
/// [`components`]: Path::components
pub struct Components<'a> {
    i:      DirSplit<'a>,
    s:      u8,
    path:   &'a [u8],
    prefix: Option<Prefix<'a>>,
}
/// An owned, mutable path (akin to [`String`]).
///
/// This type provides methods like [`push`] and [`set_extension`] that mutate
/// the path in place. It also implements [`Deref`] to [`Path`], meaning that
/// all methods on [`Path`] slices are available on `PathBuf` values as well.
///
/// [`push`]: PathBuf::push
/// [`set_extension`]: PathBuf::set_extension
///
/// More details about the overall approach can be found in
/// the [module documentation](self).
///
/// # Examples
///
/// You can use [`push`] to build up a `PathBuf` from
/// components:
///
/// ```
/// use xrmt_stx::path::PathBuf;
///
/// let mut path = PathBuf::new();
///
/// path.push(r"C:\");
/// path.push("windows");
/// path.push("system32");
///
/// path.set_extension("dll");
/// ```
///
/// However, [`push`] is best used for dynamic situations. This is a better way
/// to do this when you know all of the components ahead of time:
///
/// ```
/// use xrmt_stx::path::PathBuf;
///
/// let path: PathBuf = [r"C:\", "windows", "system32.dll"].iter().collect();
/// ```
///
/// We can still do better than this! Since these are all strings, we can use
/// `From::from`:
///
/// ```
/// use xrmt_stx::path::PathBuf;
///
/// let path = PathBuf::from(r"C:\windows\system32.dll");
/// ```
///
/// Which method works best depends on what kind of situation you're in.
///
/// Note that `PathBuf` does not always sanitize arguments, for example
/// [`push`] allows paths built from strings which include separators:
///
/// ```
/// use xrmt_stx::path::PathBuf;
///
/// let mut path = PathBuf::new();
///
/// path.push(r"C:\");
/// path.push("windows");
/// path.push(r"..\otherdir");
/// path.push("system32");
/// ```
///
/// The behavior of `PathBuf` may be changed to a panic on such inputs
/// in the future. [`Extend::extend`] should be used to add multi-part paths.
pub struct PathBuf(OsString);
/// A structure wrapping a Windows path prefix as well as its unparsed string
/// representation.
///
/// In addition to the parsed [`Prefix`] information returned by [`kind`],
/// `PrefixComponent` also holds the raw and unparsed [`OsStr`] slice,
/// returned by [`as_os_str`].
///
/// Instances of this `struct` can be obtained by matching against the
/// [`Prefix` variant] on [`Component`].
///
/// Does not occur on Unix.
///
/// # Examples
///
/// ```
/// # if cfg!(windows) {
/// use xrmt_stx::path::{Component, Path, Prefix};
/// use xrmt_stx::ffi::OsStr;
///
/// let path = Path::new(r"c:\you\later\");
/// match path.components().next().unwrap() {
///     Component::Prefix(prefix_component) => {
///         assert_eq!(Prefix::Disk(b'C'), prefix_component.kind());
///         assert_eq!(OsStr::new("c:"), prefix_component.as_os_str());
///     }
///     _ => unreachable!(),
/// }
/// # }
/// ```
///
/// [`as_os_str`]: PrefixComponent::as_os_str
/// [`kind`]: PrefixComponent::kind
/// [`Prefix` variant]: Component::Prefix
pub struct PrefixComponent<'a> {
    raw:    &'a OsStr,
    parsed: Prefix<'a>,
}
/// An error returned from [`Path::strip_prefix`] if the prefix was not found.
///
/// This `struct` is created by the [`strip_prefix`] method on [`Path`].
/// See its documentation for more.
///
/// [`strip_prefix`]: Path::strip_prefix
pub struct StripPrefixError(());
/// An iterator over the [`Component`]s of a [`Path`], as [`OsStr`] slices.
///
/// This `struct` is created by the [`iter`] method on [`Path`].
/// See its documentation for more.
///
/// [`iter`]: Path::iter
pub struct Iter<'a>(Components<'a>);
/// An iterator over [`Path`] and its ancestors.
///
/// This `struct` is created by the [`ancestors`] method on [`Path`].
/// See its documentation for more.
///
/// # Examples
///
/// ```
/// use xrmt_stx::path::Path;
///
/// let path = Path::new("/foo/bar");
///
/// for ancestor in path.ancestors() {
///     println!("{}", ancestor.display());
/// }
/// ```
///
/// [`ancestors`]: Path::ancestors
pub struct Ancestors<'a>(Option<&'a Path>);

struct DirSplit<'a>(&'a [u8]);

impl Path {
    /// Directly wraps a string slice as a `Path` slice.
    ///
    /// This is a cost-free conversion.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// Path::new("foo.txt");
    /// ```
    ///
    /// You can create `Path`s from `String`s, or even other `Path`s:
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// let string = String::from("foo.txt");
    /// let from_string = Path::new(&string);
    /// let from_path = Path::new(&from_string);
    /// assert_eq!(from_string, from_path);
    /// ```
    #[inline]
    pub fn new<T: ?Sized + AsRef<OsStr>>(s: &T) -> &Path {
        unsafe { &*(s.as_ref() as *const OsStr as *const Path) }
    }

    /// Returns `true` if the path points at an existing entity.
    ///
    /// Warning: this method may be error-prone, consider using [`try_exists()`]
    /// instead! It also has a risk of introducing time-of-check to
    /// time-of-use (TOCTOU) bugs.
    ///
    /// This function will traverse symbolic links to query information about
    /// the destination file.
    ///
    /// If you cannot access the metadata of the file, e.g. because of a
    /// permission error or broken symbolic links, this will return `false`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::path::Path;
    /// assert!(!Path::new("does_not_exist.txt").exists());
    /// ```
    ///
    /// # See Also
    ///
    /// This is a convenience function that coerces errors to false. If you want
    /// to check errors, call [`Path::try_exists`].
    ///
    /// [`try_exists()`]: Path::try_exists
    #[inline]
    pub fn exists(&self) -> bool {
        self.try_exists().unwrap_or(false)
    }
    /// Returns `true` if the path exists on disk and is pointing at a
    /// directory.
    ///
    /// This function will traverse symbolic links to query information about
    /// the destination file.
    ///
    /// If you cannot access the metadata of the file, e.g. because of a
    /// permission error or broken symbolic links, this will return `false`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::path::Path;
    /// assert_eq!(Path::new("./is_a_directory/").is_dir(), true);
    /// assert_eq!(Path::new("a_file.txt").is_dir(), false);
    /// ```
    ///
    /// # See Also
    ///
    /// This is a convenience function that coerces errors to false. If you want
    /// to check errors, call [`metadata`] and handle its [`Result`].
    /// Then call [`Metadata::is_dir`] if it was [`Ok`].
    ///
    /// [`metadata`]: crate::fs::metadata
    /// [`Metadata::is_dir`]: crate::fs::Metadata
    #[inline]
    pub fn is_dir(&self) -> bool {
        self.metadata().map_or(false, |v| v.is_dir())
    }
    /// Returns `true` if the path exists on disk and is pointing at a regular
    /// file.
    ///
    /// This function will traverse symbolic links to query information about
    /// the destination file.
    ///
    /// If you cannot access the metadata of the file, e.g. because of a
    /// permission error or broken symbolic links, this will return `false`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::path::Path;
    /// assert_eq!(Path::new("./is_a_directory/").is_file(), false);
    /// assert_eq!(Path::new("a_file.txt").is_file(), true);
    /// ```
    ///
    /// # See Also
    ///
    /// This is a convenience function that coerces errors to false. If you want
    /// to check errors, call [`metadata`] and handle its [`Result`].
    /// Then call [`Metadata::is_file`] if it was [`Ok`].
    ///
    /// When the goal is simply to read from (or write to) the source, the most
    /// reliable way to test the source can be read (or written to) is to open
    /// it. Only using `is_file` can break workflows like `diff <( prog_a )` on
    /// a Unix-like system for example. See [`File::open`] or
    /// [`OpenOptions::open`] for more information.
    ///
    /// [`File::open`]: crate::fs::File
    /// [`OpenOptions::open`]: crate::fs::OpenOptions
    #[inline]
    pub fn is_file(&self) -> bool {
        self.metadata().map_or(false, |v| v.is_file())
    }
    /// Produces an iterator over the path's components viewed as [`OsStr`]
    /// slices.
    ///
    /// For more information about the particulars of how the path is separated
    /// into components, see [`components`].
    ///
    /// [`components`]: Path::components
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{self, Path};
    /// use xrmt_stx::ffi::OsStr;
    ///
    /// let mut it = Path::new("/tmp/foo.txt").iter();
    /// assert_eq!(it.next(), Some(OsStr::new(&path::MAIN_SEPARATOR.to_string())));
    /// assert_eq!(it.next(), Some(OsStr::new("tmp")));
    /// assert_eq!(it.next(), Some(OsStr::new("foo.txt")));
    /// assert_eq!(it.next(), None)
    /// ```
    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        Iter(self.components())
    }
    /// Returns `true` if the `Path` has a root.
    ///
    /// * On Unix, a path has a root if it begins with `/`.
    ///
    /// * On Windows, a path has a root if it:
    ///     * has no prefix and begins with a separator, e.g., `\windows`
    ///     * has a prefix followed by a separator, e.g., `c:\windows` but not
    ///       `c:windows`
    ///     * has any non-disk prefix, e.g., `\\server\share`
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// assert!(Path::new("/etc/passwd").has_root());
    /// ```
    #[inline]
    pub fn has_root(&self) -> bool {
        self.components().is_root()
    }
    /// Returns `true` if the path exists on disk and is pointing at a symbolic
    /// link.
    ///
    /// This function will not traverse symbolic links.
    /// In case of a broken symbolic link this will also return true.
    ///
    /// If you cannot access the directory containing the file, e.g., because of
    /// a permission error, this will return false.
    ///
    /// # Examples
    /// ```no_run
    /// use xrmt_stx::path::Path;
    /// use xrmt_stx::os::unix::fs::symlink;
    ///
    /// let link_path = Path::new("link");
    /// symlink("/origin_does_not_exist/", link_path).unwrap();
    /// assert_eq!(link_path.is_symlink(), true);
    /// assert_eq!(link_path.exists(), false);
    /// ```
    ///
    /// # See Also
    ///
    /// This is a convenience function that coerces errors to false. If you want
    /// to check errors, call [`symlink_metadata`] and handle its
    /// [`Result`]. Then call [`Metadata::is_symlink`] if it was [`Ok`].
    #[inline]
    pub fn is_symlink(&self) -> bool {
        self.metadata().map_or(false, |v| v.is_symlink())
    }
    /// Returns `true` if the `Path` is absolute, i.e., if it is independent of
    /// the current directory.
    ///
    /// * On Unix, a path is absolute if it starts with the root, so
    /// `is_absolute` and [`has_root`] are equivalent.
    ///
    /// * On Windows, a path is absolute if it has a prefix and starts with the
    /// root: `c:\windows` is absolute, while `c:temp` and `\temp` are not.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// assert!(!Path::new("foo.txt").is_absolute());
    /// ```
    ///
    /// [`has_root`]: Path::has_root
    #[inline]
    pub fn is_absolute(&self) -> bool {
        self.components().is_root()
    }
    /// Returns `true` if the `Path` is relative, i.e., not absolute.
    ///
    /// See [`is_absolute`]'s documentation for more details.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// assert!(Path::new("foo.txt").is_relative());
    /// ```
    ///
    /// [`is_absolute`]: Path::is_absolute
    #[inline]
    pub fn is_relative(&self) -> bool {
        !self.is_absolute()
    }
    /// Yields the underlying [`OsStr`] slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// let os_str = Path::new("foo.txt").as_os_str();
    /// assert_eq!(os_str, xrmt_stx::ffi::OsStr::new("foo.txt"));
    /// ```
    #[inline]
    pub fn as_os_str(&self) -> &OsStr {
        &self.0
    }
    /// Yields a [`&str`] slice if the `Path` is valid unicode.
    ///
    /// This conversion may entail doing a check for UTF-8 validity.
    /// Note that validation is performed because non-UTF-8 strings are
    /// perfectly valid for some OS.
    ///
    /// [`&str`]: str
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// let path = Path::new("foo.txt");
    /// assert_eq!(path.to_str(), Some("foo.txt"));
    /// ```
    #[inline]
    pub fn to_str(&self) -> Option<&str> {
        self.0.to_str()
    }
    /// Converts a `Path` to an owned [`PathBuf`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{Path, PathBuf};
    ///
    /// let path_buf = Path::new("foo.txt").to_path_buf();
    /// assert_eq!(path_buf, PathBuf::from("foo.txt"));
    /// ```
    #[inline]
    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf(self.0.to_os_string())
    }
    /// Returns the `Path` without its final component, if there is one.
    ///
    /// This means it returns `Some("")` for relative paths with one component.
    ///
    /// Returns [`None`] if the path terminates in a root or prefix, or if it's
    /// the empty string.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// let path = Path::new("/foo/bar");
    /// let parent = path.parent().unwrap();
    /// assert_eq!(parent, Path::new("/foo"));
    ///
    /// let grand_parent = parent.parent().unwrap();
    /// assert_eq!(grand_parent, Path::new("/"));
    /// assert_eq!(grand_parent.parent(), None);
    ///
    /// let relative_path = Path::new("foo/bar");
    /// let parent = relative_path.parent();
    /// assert_eq!(parent, Some(Path::new("foo")));
    /// let grand_parent = parent.and_then(Path::parent);
    /// assert_eq!(grand_parent, Some(Path::new("")));
    /// let great_grand_parent = grand_parent.and_then(Path::parent);
    /// assert_eq!(great_grand_parent, None);
    /// ```
    #[inline]
    pub fn parent(&self) -> Option<&Path> {
        let mut x = self.components();
        x.next_back().and_then(|p| match p {
            Component::Normal(_) | Component::CurDir | Component::ParentDir => Some(x.as_path()),
            _ => None,
        })
    }
    /// Produces an iterator over `Path` and its ancestors.
    ///
    /// The iterator will yield the `Path` that is returned if the [`parent`]
    /// method is used zero or more times. If the [`parent`] method returns
    /// [`None`], the iterator will do likewise. The iterator will always
    /// yield at least one value, namely `Some(&self)`. Next it will yield
    /// `&self.parent()`, `&self.parent().and_then(Path::parent)` and so on.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// let mut ancestors = Path::new("/foo/bar").ancestors();
    /// assert_eq!(ancestors.next(), Some(Path::new("/foo/bar")));
    /// assert_eq!(ancestors.next(), Some(Path::new("/foo")));
    /// assert_eq!(ancestors.next(), Some(Path::new("/")));
    /// assert_eq!(ancestors.next(), None);
    ///
    /// let mut ancestors = Path::new("../foo/bar").ancestors();
    /// assert_eq!(ancestors.next(), Some(Path::new("../foo/bar")));
    /// assert_eq!(ancestors.next(), Some(Path::new("../foo")));
    /// assert_eq!(ancestors.next(), Some(Path::new("..")));
    /// assert_eq!(ancestors.next(), Some(Path::new("")));
    /// assert_eq!(ancestors.next(), None);
    /// ```
    ///
    /// [`parent`]: Path::parent
    #[inline]
    pub fn ancestors(&self) -> Ancestors<'_> {
        Ancestors(Some(&self))
    }
    /// Returns the final component of the `Path`, if there is one.
    ///
    /// If the path is a normal file, this is the file name. If it's the path of
    /// a directory, this is the directory name.
    ///
    /// Returns [`None`] if the path terminates in `..`.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    /// use xrmt_stx::ffi::OsStr;
    ///
    /// assert_eq!(Some(OsStr::new("bin")), Path::new("/usr/bin/").file_name());
    /// assert_eq!(Some(OsStr::new("foo.txt")), Path::new("tmp/foo.txt").file_name());
    /// assert_eq!(Some(OsStr::new("foo.txt")), Path::new("foo.txt/.").file_name());
    /// assert_eq!(Some(OsStr::new("foo.txt")), Path::new("foo.txt/.//").file_name());
    /// assert_eq!(None, Path::new("foo.txt/..").file_name());
    /// assert_eq!(None, Path::new("/").file_name());
    /// ```
    #[inline]
    pub fn file_name(&self) -> Option<&OsStr> {
        self.components().next_back().and_then(|p| match p {
            Component::Normal(p) => Some(p),
            _ => None,
        })
    }
    /// Extracts the stem (non-extension) portion of [`self.file_name`].
    ///
    /// [`self.file_name`]: Path::file_name
    ///
    /// The stem is:
    ///
    /// * [`None`], if there is no file name;
    /// * The entire file name if there is no embedded `.`;
    /// * The entire file name if the file name begins with `.` and has no other
    ///   `.`s within;
    /// * Otherwise, the portion of the file name before the final `.`
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// assert_eq!("foo", Path::new("foo.rs").file_stem().unwrap());
    /// assert_eq!("foo.tar", Path::new("foo.tar.gz").file_stem().unwrap());
    /// ```
    ///
    /// # See Also
    /// This method is similar to [`Path::file_prefix`], which extracts the
    /// portion of the file name before the *first* `.`
    ///
    /// [`Path::file_prefix`]: Path::file_prefix
    #[inline]
    pub fn file_stem(&self) -> Option<&OsStr> {
        let v = self.file_name()?;
        let b = v.as_bytes();
        match b.iter().rposition(|v| *v == EXT) {
            Some(i) if i > 0 => Some(OsStr::from_slice(unsafe { b.get_unchecked(0..i) })),
            _ => Some(v),
        }
    }
    /// Extracts the extension (without the leading dot) of [`self.file_name`],
    /// if possible.
    ///
    /// The extension is:
    ///
    /// * [`None`], if there is no file name;
    /// * [`None`], if there is no embedded `.`;
    /// * [`None`], if the file name begins with `.` and has no other `.`s
    ///   within;
    /// * Otherwise, the portion of the file name after the final `.`
    ///
    /// [`self.file_name`]: Path::file_name
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// assert_eq!("rs", Path::new("foo.rs").extension().unwrap());
    /// assert_eq!("gz", Path::new("foo.tar.gz").extension().unwrap());
    /// ```
    #[inline]
    pub fn extension(&self) -> Option<&OsStr> {
        let v = self.file_name()?.as_bytes();
        match v.iter().rposition(|v| *v == EXT) {
            Some(i) if i > 0 => Some(OsStr::from_slice(&v[i + 1..])),
            _ => None,
        }
    }
    /// Returns `Ok(true)` if the path points at an existing entity.
    ///
    /// This function will traverse symbolic links to query information about
    /// the destination file. In case of broken symbolic links this will
    /// return `Ok(false)`.
    ///
    /// [`Path::exists()`] only checks whether or not a path was both found and
    /// readable. By contrast, `try_exists` will return `Ok(true)` or
    /// `Ok(false)`, respectively, if the path was _verified_ to exist or
    /// not exist. If its existence can neither be confirmed nor denied, it
    /// will propagate an `Err(_)` instead. This can be the case if e.g. listing
    /// permission is denied on one of the parent directories.
    ///
    /// Note that while this avoids some pitfalls of the `exists()` method, it
    /// still can not prevent time-of-check to time-of-use (TOCTOU) bugs.
    /// You should only use it in scenarios where those bugs are not an
    /// issue.
    ///
    /// This is an alias for [`xrmt_stx::fs::exists`](crate::fs::exists).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::path::Path;
    /// assert!(!Path::new("does_not_exist.txt").try_exists().expect("Can't check existence of file does_not_exist.txt"));
    /// assert!(Path::new("/root/secret_file.txt").try_exists().is_err());
    /// ```
    ///
    /// [`exists()`]: Path::exists
    #[inline]
    pub fn try_exists(&self) -> IoResult<bool> {
        exists(self)
    }
    /// Produces an iterator over the [`Component`]s of the path.
    ///
    /// When parsing the path, there is a small amount of normalization:
    ///
    /// * Repeated separators are ignored, so `a/b` and `a//b` both have `a` and
    ///   `b` as components.
    ///
    /// * Occurrences of `.` are normalized away, except if they are at the
    ///   beginning of the path. For example, `a/./b`, `a/b/`, `a/b/.` and `a/b`
    ///   all have `a` and `b` as components, but `./a/b` starts with an
    ///   additional [`CurDir`] component.
    ///
    /// * A trailing slash is normalized away, `/a/b` and `/a/b/` are
    ///   equivalent.
    ///
    /// Note that no other normalization takes place; in particular, `a/c`
    /// and `a/b/../c` are distinct, to account for the possibility that `b`
    /// is a symbolic link (so its parent isn't `a`).
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{Path, Component};
    /// use xrmt_stx::ffi::OsStr;
    ///
    /// let mut components = Path::new("/tmp/foo.txt").components();
    ///
    /// assert_eq!(components.next(), Some(Component::RootDir));
    /// assert_eq!(components.next(), Some(Component::Normal(OsStr::new("tmp"))));
    /// assert_eq!(components.next(), Some(Component::Normal(OsStr::new("foo.txt"))));
    /// assert_eq!(components.next(), None)
    /// ```
    ///
    /// [`CurDir`]: Component::CurDir
    #[inline]
    pub fn components(&self) -> Components<'_> {
        Components::new(&self.0)
    }
    /// Extracts the prefix of [`self.file_name`].
    ///
    /// The prefix is:
    ///
    /// * [`None`], if there is no file name;
    /// * The entire file name if there is no embedded `.`;
    /// * The portion of the file name before the first non-beginning `.`;
    /// * The entire file name if the file name begins with `.` and has no other
    ///   `.`s within;
    /// * The portion of the file name before the second `.` if the file name
    ///   begins with `.`
    ///
    /// [`self.file_name`]: Path::file_name
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(path_file_prefix)]
    /// use xrmt_stx::path::Path;
    ///
    /// assert_eq!("foo", Path::new("foo.rs").file_prefix().unwrap());
    /// assert_eq!("foo", Path::new("foo.tar.gz").file_prefix().unwrap());
    /// ```
    ///
    /// # See Also
    /// This method is similar to [`Path::file_stem`], which extracts the
    /// portion of the file name before the *last* `.`
    ///
    /// [`Path::file_stem`]: Path::file_stem
    #[inline]
    pub fn file_prefix(&self) -> Option<&OsStr> {
        let v = self.file_name()?;
        let b = v.as_bytes();
        match b.iter().position(|v| *v == EXT) {
            Some(i) if i > 0 => Some(OsStr::from_slice(unsafe { b.get_unchecked(0..i) })),
            _ => Some(v),
        }
    }
    /// Returns an iterator over the entries within a directory.
    ///
    /// The iterator will yield instances of
    /// <code>[Result]<[DirEntry]></code>. New errors may
    /// be encountered after an iterator is initially constructed.
    ///
    /// This is an alias to [`read_dir`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::path::Path;
    ///
    /// let path = Path::new("/laputa");
    /// for entry in path.read_dir().expect("read_dir call failed") {
    ///     if let Ok(entry) = entry {
    ///         println!("{:?}", entry.path());
    ///     }
    /// }
    /// ```
    ///
    /// [DirEntry]: crate::fs::DirEntry
    #[inline]
    pub fn read_dir(&self) -> IoResult<ReadDir> {
        read_dir(self)
    }
    /// Queries the file system to get information about a file, directory, etc.
    ///
    /// This function will traverse symbolic links to query information about
    /// the destination file.
    ///
    /// This is an alias to [`metadata`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::path::Path;
    ///
    /// let path = Path::new("/Minas/tirith");
    /// let metadata = path.metadata().expect("metadata call failed");
    /// println!("{:?}", metadata.file_type());
    /// ```
    #[inline]
    pub fn metadata(&self) -> IoResult<Metadata> {
        metadata(self)
    }
    /// Reads a symbolic link, returning the file that the link points to.
    ///
    /// This is an alias to [`read_link`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::path::Path;
    ///
    /// let path = Path::new("/laputa/sky_castle.rs");
    /// let path_link = path.read_link().expect("read_link call failed");
    /// ```
    #[inline]
    pub fn read_link(&self) -> IoResult<PathBuf> {
        read_link(self)
    }
    /// Converts a `Path` to a [`Cow<str>`].
    ///
    /// Any non-UTF-8 sequences are replaced with
    /// [`U+FFFD REPLACEMENT CHARACTER`][U+FFFD].
    ///
    /// [U+FFFD]: core::char::REPLACEMENT_CHARACTER
    ///
    /// # Examples
    ///
    /// Calling `to_string_lossy` on a `Path` with valid unicode:
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// let path = Path::new("foo.txt");
    /// assert_eq!(path.to_string_lossy(), "foo.txt");
    /// ```
    ///
    /// Had `path` contained invalid unicode, the `to_string_lossy` call might
    /// have returned `"fo�.txt"`.
    #[inline]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        self.0.to_string_lossy()
    }
    /// Yields a mutable reference to the underlying [`OsStr`] slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{Path, PathBuf};
    ///
    /// let mut path = PathBuf::from("Foo.TXT");
    ///
    /// assert_ne!(path, Path::new("foo.txt"));
    ///
    /// path.as_mut_os_str().make_ascii_lowercase();
    /// assert_eq!(path, Path::new("foo.txt"));
    /// ```
    #[inline]
    pub fn as_mut_os_str(&mut self) -> &mut OsStr {
        &mut self.0
    }
    /// Returns the canonical, absolute form of the path with all intermediate
    /// components normalized and symbolic links resolved.
    ///
    /// This is an alias to [`canonicalize`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::path::{Path, PathBuf};
    ///
    /// let path = Path::new("/foo/test/../test/bar.rs");
    /// assert_eq!(path.canonicalize().unwrap(), PathBuf::from("/foo/test/bar.rs"));
    /// ```
    #[inline]
    pub fn canonicalize(&self) -> IoResult<PathBuf> {
        canonicalize(self)
    }
    /// Queries the metadata about a file without following symlinks.
    ///
    /// This is an alias to [`symlink_metadata`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::path::Path;
    ///
    /// let path = Path::new("/Minas/tirith");
    /// let metadata = path.symlink_metadata().expect("symlink_metadata call failed");
    /// println!("{:?}", metadata.file_type());
    /// ```
    #[inline]
    pub fn symlink_metadata(&self) -> IoResult<Metadata> {
        symlink_metadata(self)
    }
    /// Creates an owned [`PathBuf`] with `path` adjoined to `self`.
    ///
    /// If `path` is absolute, it replaces the current path.
    ///
    /// See [`PathBuf::push`] for more details on what it means to adjoin a
    /// path.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{Path, PathBuf};
    ///
    /// assert_eq!(Path::new("/etc").join("passwd"), PathBuf::from("/etc/passwd"));
    /// assert_eq!(Path::new("/etc").join("/bin/sh"), PathBuf::from("/bin/sh"));
    /// ```
    pub fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        let v = path.as_ref();
        if v.is_absolute() {
            return v.to_path_buf();
        }
        let mut p = self.to_path_buf();
        if !p.last(sep) && !v.first(sep) {
            p.0.as_mut_vec().push(SEP);
        }
        p.0.push(v);
        p
    }
    /// Determines whether `child` is a suffix of `self`.
    ///
    /// Only considers whole path components to match.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// let path = Path::new("/etc/resolv.conf");
    ///
    /// assert!(path.ends_with("resolv.conf"));
    /// assert!(path.ends_with("etc/resolv.conf"));
    /// assert!(path.ends_with("/etc/resolv.conf"));
    ///
    /// assert!(!path.ends_with("/resolv.conf"));
    /// assert!(!path.ends_with("conf")); // use .extension() instead
    /// ```
    pub fn ends_with(&self, child: impl AsRef<Path>) -> bool {
        let mut v = child.as_ref().as_bytes().split(sep).rev();
        for i in self.as_bytes().split(sep).rev() {
            match v.next() {
                Some(e) if *e == *i => (),
                Some(_) => return false,
                None => return true,
            }
        }
        true
    }
    /// Determines whether `base` is a prefix of `self`.
    ///
    /// Only considers whole path components to match.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// let path = Path::new("/etc/passwd");
    ///
    /// assert!(path.starts_with("/etc"));
    /// assert!(path.starts_with("/etc/"));
    /// assert!(path.starts_with("/etc/passwd"));
    /// assert!(path.starts_with("/etc/passwd/")); // extra slash is okay
    /// assert!(path.starts_with("/etc/passwd///")); // multiple extra slashes are okay
    ///
    /// assert!(!path.starts_with("/e"));
    /// assert!(!path.starts_with("/etc/passwd.txt"));
    ///
    /// assert!(!Path::new("/etc/foo.rs").starts_with("/etc/foo"));
    /// ```
    pub fn starts_with(&self, base: impl AsRef<Path>) -> bool {
        let (v, mut n) = (base.as_ref().as_bytes(), 0);
        for (i, b) in self.as_bytes().iter().enumerate() {
            if i >= v.len() {
                return true;
            }
            if v[i] != *b {
                return false;
            }
            n += 1;
        }
        if n < v.len() {
            for i in &v[n..] {
                if !sep(i) {
                    return false;
                }
            }
        }
        true
    }
    /// Creates an owned [`PathBuf`] like `self` but with the given file name.
    ///
    /// See [`PathBuf::set_file_name`] for more details.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{Path, PathBuf};
    ///
    /// let path = Path::new("/tmp/foo.png");
    /// assert_eq!(path.with_file_name("bar"), PathBuf::from("/tmp/bar"));
    /// assert_eq!(path.with_file_name("bar.txt"), PathBuf::from("/tmp/bar.txt"));
    ///
    /// let path = Path::new("/tmp");
    /// assert_eq!(path.with_file_name("var"), PathBuf::from("/var"));
    /// ```
    #[inline]
    pub fn with_file_name(&self, file_name: impl AsRef<OsStr>) -> PathBuf {
        let mut v = self.to_path_buf();
        v.set_file_name(file_name.as_ref());
        v
    }
    /// Creates an owned [`PathBuf`] like `self` but with the given extension.
    ///
    /// See [`PathBuf::set_extension`] for more details.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{Path, PathBuf};
    ///
    /// let path = Path::new("foo.rs");
    /// assert_eq!(path.with_extension("txt"), PathBuf::from("foo.txt"));
    ///
    /// let path = Path::new("foo.tar.gz");
    /// assert_eq!(path.with_extension(""), PathBuf::from("foo.tar"));
    /// assert_eq!(path.with_extension("xz"), PathBuf::from("foo.tar.xz"));
    /// assert_eq!(path.with_extension("").with_extension("txt"), PathBuf::from("foo.txt"));
    /// ```
    pub fn with_extension(&self, extension: impl AsRef<OsStr>) -> PathBuf {
        let mut b = self.to_path_buf();
        let v = extension.as_ref();
        if v.as_bytes().iter().position(sep).is_some() {
            return b;
        }
        let n = match b.file_stem() {
            Some(x) => x.len(),
            None => return b,
        };
        b.0.truncate(n);
        if !v.is_empty() {
            if v.last(|i| *i != EXT) {
                b.0.as_mut_vec().push(b'.');
            }
            b.0.push(v);
        }
        b
    }
    /// Creates an owned [`PathBuf`] like `self` but with the extension added.
    ///
    /// See [`PathBuf::add_extension`] for more details.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{Path, PathBuf};
    ///
    /// let path = Path::new("foo.rs");
    /// assert_eq!(path.with_added_extension("txt"), PathBuf::from("foo.rs.txt"));
    ///
    /// let path = Path::new("foo.tar.gz");
    /// assert_eq!(path.with_added_extension(""), PathBuf::from("foo.tar.gz"));
    /// assert_eq!(path.with_added_extension("xz"), PathBuf::from("foo.tar.gz.xz"));
    /// assert_eq!(path.with_added_extension("").with_added_extension("txt"), PathBuf::from("foo.tar.gz.txt"));
    /// ```
    #[inline]
    pub fn with_added_extension(&self, extension: impl AsRef<OsStr>) -> PathBuf {
        let mut v = self.to_path_buf();
        v.add_extension(extension);
        v
    }
    /// Returns a path that, when joined onto `base`, yields `self`.
    ///
    /// # Errors
    ///
    /// If `base` is not a prefix of `self` (i.e., [`starts_with`]
    /// returns `false`), returns [`Err`].
    ///
    /// [`starts_with`]: Path::starts_with
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{Path, PathBuf};
    ///
    /// let path = Path::new("/test/haha/foo.txt");
    ///
    /// assert_eq!(path.strip_prefix("/"), Ok(Path::new("test/haha/foo.txt")));
    /// assert_eq!(path.strip_prefix("/test"), Ok(Path::new("haha/foo.txt")));
    /// assert_eq!(path.strip_prefix("/test/"), Ok(Path::new("haha/foo.txt")));
    /// assert_eq!(path.strip_prefix("/test/haha/foo.txt"), Ok(Path::new("")));
    /// assert_eq!(path.strip_prefix("/test/haha/foo.txt/"), Ok(Path::new("")));
    ///
    /// assert!(path.strip_prefix("test").is_err());
    /// assert!(path.strip_prefix("/te").is_err());
    /// assert!(path.strip_prefix("/haha").is_err());
    ///
    /// let prefix = PathBuf::from("/test/");
    /// assert_eq!(path.strip_prefix(prefix), Ok(Path::new("haha/foo.txt")));
    /// ```
    pub fn strip_prefix(&self, base: impl AsRef<Path>) -> Result<&Path, StripPrefixError> {
        let v = base.as_ref();
        // Check first byte.
        match (self.as_bytes().first(), v.as_bytes().first()) {
            (Some(a), Some(b)) if *a == *b => (),
            _ => return Err(StripPrefixError(())),
        }
        let mut y = DirSplit(v.as_bytes());
        let mut x = DirSplit(self.as_bytes());
        loop {
            let k = match y.next() {
                Some(b) => b,
                None => break,
            };
            let j = match x.next() {
                Some(b) => b,
                None => break,
            };
            if !j.eq(k) {
                return Err(StripPrefixError(()));
            }
        }
        Ok(Path::from_slice(x.0))
    }

    /// Converts a [`Box<Path>`](Box) into a [`PathBuf`] without copying or
    /// allocating.
    #[inline]
    pub fn into_path_buf(self: Box<Path>) -> PathBuf {
        unsafe {
            PathBuf(OsString::from(Box::from_raw(
                Box::into_raw(self) as *mut OsStr
            )))
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
    #[inline]
    fn from_slice(s: &[u8]) -> &Path {
        Path::new(OsStr::from_slice(s))
    }
    #[inline]
    fn from_mut(v: &mut OsStr) -> &mut Path {
        unsafe { &mut *(v as *mut OsStr as *mut Path) }
    }
    #[inline]
    fn last(&self, func: impl FnOnce(&u8) -> bool) -> bool {
        self.0.last(func)
    }
    #[inline]
    fn first(&self, func: impl FnOnce(&u8) -> bool) -> bool {
        self.0.first(func)
    }
}
impl PathBuf {
    /// Allocates an empty `PathBuf`.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::PathBuf;
    ///
    /// let path = PathBuf::new();
    /// ```
    #[inline]
    pub fn new() -> PathBuf {
        PathBuf(OsString::new())
    }
    /// Creates a new `PathBuf` with a given capacity used to create the
    /// internal [`OsString`]. See [`with_capacity`] defined on [`OsString`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::PathBuf;
    ///
    /// let mut path = PathBuf::with_capacity(10);
    /// let capacity = path.capacity();
    ///
    /// // This push is done without reallocating
    /// path.push(r"C:\");
    ///
    /// assert_eq!(capacity, path.capacity());
    /// ```
    ///
    /// [`with_capacity`]: OsString::with_capacity
    #[inline]
    pub fn with_capacity(len: usize) -> PathBuf {
        PathBuf(OsString::with_capacity(len))
    }

    /// Invokes [`clear`] on the underlying instance of [`OsString`].
    ///
    /// [`clear`]: OsString::clear
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }
    /// Truncates `self` to [`self.parent`].
    ///
    /// Returns `false` and does nothing if [`self.parent`] is [`None`].
    /// Otherwise, returns `true`.
    ///
    /// [`self.parent`]: Path::parent
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{Path, PathBuf};
    ///
    /// let mut p = PathBuf::from("/spirited/away.rs");
    ///
    /// p.pop();
    /// assert_eq!(Path::new("/spirited"), p);
    /// p.pop();
    /// assert_eq!(Path::new("/"), p);
    /// ```
    #[inline]
    pub fn pop(&mut self) -> bool {
        if let Some(v) = self.parent().map(|p| p.len()) {
            self.0.truncate(v);
            return true;
        }
        false
    }
    /// Coerces to a [`Path`] slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{Path, PathBuf};
    ///
    /// let p = PathBuf::from("/test");
    /// assert_eq!(Path::new("/test"), p.as_path());
    /// ```
    #[inline]
    pub fn as_path(&self) -> &Path {
        self
    }
    /// Invokes [`capacity`] on the underlying instance of [`OsString`].
    ///
    /// [`capacity`]: OsString::capacity
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
    /// Invokes [`shrink_to_fit`] on the underlying instance of [`OsString`].
    ///
    /// [`shrink_to_fit`]: OsString::shrink_to_fit
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit()
    }
    /// Invokes [`reserve`] on the underlying instance of [`OsString`].
    ///
    /// [`reserve`]: OsString::reserve
    #[inline]
    pub fn reserve(&mut self, len: usize) {
        self.0.reserve(len)
    }
    /// Consumes and leaks the `PathBuf`, returning a mutable reference to the
    /// contents, `&'a mut Path`.
    ///
    /// The caller has free choice over the returned lifetime, including
    /// 'static. Indeed, this function is ideally used for data that lives
    /// for the remainder of the program’s life, as dropping the returned
    /// reference will cause a memory leak.
    ///
    /// It does not reallocate or shrink the `PathBuf`, so the leaked allocation
    /// may include unused capacity that is not part of the returned slice.
    /// If you want to discard excess capacity, call [`into_boxed_path`],
    /// and then [`Box::leak`] instead. However, keep in mind that trimming
    /// the capacity may result in a reallocation and copy.
    ///
    /// [`into_boxed_path`]: PathBuf::into_boxed_path
    #[inline]
    pub fn leak<'a>(self) -> &'a mut Path {
        Path::from_mut(self.0.leak())
    }
    /// Consumes the `PathBuf`, yielding its internal [`OsString`] storage.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::PathBuf;
    ///
    /// let p = PathBuf::from("/the/head");
    /// let os_str = p.into_os_string();
    /// ```
    #[inline]
    pub fn into_os_string(self) -> OsString {
        self.0
    }
    /// Invokes [`shrink_to`] on the underlying instance of [`OsString`].
    ///
    /// [`shrink_to`]: OsString::shrink_to
    #[inline]
    pub fn shrink_to(&mut self, len: usize) {
        self.0.shrink_to(len)
    }
    /// Converts this `PathBuf` into a [boxed](Box) [`Path`].
    #[inline]
    pub fn into_boxed_path(self) -> Box<Path> {
        unsafe { Box::from_raw(Box::into_raw(self.0.into_boxed_os_str()) as *mut Path) }
    }
    /// Invokes [`reserve_exact`] on the underlying instance of [`OsString`].
    ///
    /// [`reserve_exact`]: OsString::reserve_exact
    #[inline]
    pub fn reserve_exact(&mut self, len: usize) {
        self.0.reserve_exact(len)
    }
    /// Extends `self` with `path`.
    ///
    /// If `path` is absolute, it replaces the current path.
    ///
    /// On Windows:
    ///
    /// * if `path` has a root but no prefix (e.g., `\windows`), it replaces
    ///   everything except for the prefix (if any) of `self`.
    /// * if `path` has a prefix but no root, it replaces `self`.
    /// * if `self` has a verbatim prefix (e.g. `\\?\C:\windows`) and `path` is
    ///   not empty, the new path is normalized: all references to `.` and `..`
    ///   are removed.
    ///
    /// Consider using [`Path::join`] if you need a new `PathBuf` instead of
    /// using this function on a cloned `PathBuf`.
    ///
    /// # Examples
    ///
    /// Pushing a relative path extends the existing path:
    ///
    /// ```
    /// use xrmt_stx::path::PathBuf;
    ///
    /// let mut path = PathBuf::from("/tmp");
    /// path.push("file.bk");
    /// assert_eq!(path, PathBuf::from("/tmp/file.bk"));
    /// ```
    ///
    /// Pushing an absolute path replaces the existing path:
    ///
    /// ```
    /// use xrmt_stx::path::PathBuf;
    ///
    /// let mut path = PathBuf::from("/tmp");
    /// path.push("/etc");
    /// assert_eq!(path, PathBuf::from("/etc"));
    /// ```
    pub fn push(&mut self, path: impl AsRef<Path>) {
        if self.0.is_empty() {
            self.0.push(path.as_ref());
            return;
        }
        let v = path.as_ref();
        if v.has_root() {
            self.0.clear();
        } else if !self.last(sep) && !v.first(sep) {
            self.0.as_mut_vec().push(SEP);
        }
        self.0.push(v);
    }
    /// Yields a mutable reference to the underlying [`OsString`] instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{Path, PathBuf};
    ///
    /// let mut path = PathBuf::from("/foo");
    ///
    /// path.push("bar");
    /// assert_eq!(path, Path::new("/foo/bar"));
    ///
    /// // OsString's `push` does not add a separator.
    /// path.as_mut_os_string().push("baz");
    /// assert_eq!(path, Path::new("/foo/barbaz"));
    /// ```
    #[inline]
    pub fn as_mut_os_string(&mut self) -> &mut OsString {
        &mut self.0
    }
    /// Updates [`self.file_name`] to `file_name`.
    ///
    /// If [`self.file_name`] was [`None`], this is equivalent to pushing
    /// `file_name`.
    ///
    /// Otherwise it is equivalent to calling [`pop`] and then pushing
    /// `file_name`. The new path will be a sibling of the original path.
    /// (That is, it will have the same parent.)
    ///
    /// The argument is not sanitized, so can include separators. This
    /// behavior may be changed to a panic in the future.
    ///
    /// [`self.file_name`]: Path::file_name
    /// [`pop`]: PathBuf::pop
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::PathBuf;
    ///
    /// let mut buf = PathBuf::from("/");
    /// assert!(buf.file_name() == None);
    ///
    /// buf.set_file_name("foo.txt");
    /// assert!(buf == PathBuf::from("/foo.txt"));
    /// assert!(buf.file_name().is_some());
    ///
    /// buf.set_file_name("bar.txt");
    /// assert!(buf == PathBuf::from("/bar.txt"));
    ///
    /// buf.set_file_name("baz");
    /// assert!(buf == PathBuf::from("/baz"));
    ///
    /// buf.set_file_name("../b/c.txt");
    /// assert!(buf == PathBuf::from("/../b/c.txt"));
    ///
    /// buf.set_file_name("baz");
    /// assert!(buf == PathBuf::from("/../b/baz"));
    /// ```
    #[inline]
    pub fn set_file_name(&mut self, file_name: impl AsRef<OsStr>) {
        if self.file_name().is_some() {
            self.pop();
        }
        let v = file_name.as_ref();
        if !self.last(sep) && !v.first(sep) {
            self.0.as_mut_vec().push(SEP);
        }
        self.0.push(v);
    }
    /// Updates [`self.extension`] to `Some(extension)` or to `None` if
    /// `extension` is empty.
    ///
    /// Returns `false` and does nothing if [`self.file_name`] is [`None`],
    /// returns `true` and updates the extension otherwise.
    ///
    /// If [`self.extension`] is [`None`], the extension is added; otherwise
    /// it is replaced.
    ///
    /// If `extension` is the empty string, [`self.extension`] will be [`None`]
    /// afterwards, not `Some("")`.
    ///
    /// # Panics
    ///
    /// Panics if the passed extension contains a path separator (see
    /// [`is_separator`]).
    ///
    /// # Caveats
    ///
    /// The new `extension` may contain dots and will be used in its entirety,
    /// but only the part after the final dot will be reflected in
    /// [`self.extension`].
    ///
    /// If the file stem contains internal dots and `extension` is empty, part
    /// of the old file stem will be considered the new [`self.extension`].
    ///
    /// See the examples below.
    ///
    /// [`self.file_name`]: Path::file_name
    /// [`self.extension`]: Path::extension
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{Path, PathBuf};
    ///
    /// let mut p = PathBuf::from("/feel/the");
    ///
    /// p.set_extension("force");
    /// assert_eq!(Path::new("/feel/the.force"), p.as_path());
    ///
    /// p.set_extension("dark.side");
    /// assert_eq!(Path::new("/feel/the.dark.side"), p.as_path());
    ///
    /// p.set_extension("cookie");
    /// assert_eq!(Path::new("/feel/the.dark.cookie"), p.as_path());
    ///
    /// p.set_extension("");
    /// assert_eq!(Path::new("/feel/the.dark"), p.as_path());
    ///
    /// p.set_extension("");
    /// assert_eq!(Path::new("/feel/the"), p.as_path());
    ///
    /// p.set_extension("");
    /// assert_eq!(Path::new("/feel/the"), p.as_path());
    /// ```
    pub fn set_extension(&mut self, extension: impl AsRef<OsStr>) -> bool {
        let v = extension.as_ref();
        if v.as_bytes().iter().position(sep).is_some() {
            return false;
        }
        if let Some(n) = self.file_name() {
            if let Some(i) = n.as_bytes().iter().rposition(|i| *i == EXT) {
                self.0.truncate(self.0.len() - (n.len() - i));
            }
        }
        if !v.is_empty() {
            if v.last(|i| *i != EXT) {
                self.0.as_mut_vec().push(EXT);
            }
            self.0.push(v);
        }
        true
    }
    /// Append [`self.extension`] with `extension`.
    ///
    /// Returns `false` and does nothing if [`self.file_name`] is [`None`],
    /// returns `true` and updates the extension otherwise.
    ///
    /// # Caveats
    ///
    /// The appended `extension` may contain dots and will be used in its
    /// entirety, but only the part after the final dot will be reflected in
    /// [`self.extension`].
    ///
    /// See the examples below.
    ///
    /// [`self.file_name`]: Path::file_name
    /// [`self.extension`]: Path::extension
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::{Path, PathBuf};
    ///
    /// let mut p = PathBuf::from("/feel/the");
    ///
    /// p.add_extension("formatted");
    /// assert_eq!(Path::new("/feel/the.formatted"), p.as_path());
    ///
    /// p.add_extension("dark.side");
    /// assert_eq!(Path::new("/feel/the.formatted.dark.side"), p.as_path());
    ///
    /// p.set_extension("cookie");
    /// assert_eq!(Path::new("/feel/the.formatted.dark.cookie"), p.as_path());
    ///
    /// p.set_extension("");
    /// assert_eq!(Path::new("/feel/the.formatted.dark"), p.as_path());
    ///
    /// p.add_extension("");
    /// assert_eq!(Path::new("/feel/the.formatted.dark"), p.as_path());
    /// ```
    pub fn add_extension<S: AsRef<OsStr>>(&mut self, extension: S) -> bool {
        let v = extension.as_ref();
        if v.as_bytes().iter().position(sep).is_some() {
            return false;
        }
        match self.file_stem() {
            None => return false,
            Some(_) => (),
        };
        if !v.is_empty() {
            if v.last(|i| *i != EXT) {
                self.0.as_mut_vec().push(EXT);
            }
            self.0.push(v);
        }
        true
    }
    /// Invokes [`try_reserve`] on the underlying instance of [`OsString`].
    ///
    /// [`try_reserve`]: OsString::try_reserve
    #[inline]
    pub fn try_reserve(&mut self, len: usize) -> Result<(), TryReserveError> {
        self.0.try_reserve(len)
    }
    /// Invokes [`try_reserve_exact`] on the underlying instance of
    /// [`OsString`].
    ///
    /// [`try_reserve_exact`]: OsString::try_reserve_exact
    #[inline]
    pub fn try_reserve_exact(&mut self, len: usize) -> Result<(), TryReserveError> {
        self.0.try_reserve_exact(len)
    }
}
impl<'a> Iter<'a> {
    /// Extracts a slice corresponding to the portion of the path remaining for
    /// iteration.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// let mut iter = Path::new("/tmp/foo/bar.txt").iter();
    /// iter.next();
    /// iter.next();
    ///
    /// assert_eq!(Path::new("foo/bar.txt"), iter.as_path());
    /// ```
    #[inline]
    pub fn as_path(&self) -> &'a Path {
        self.0.as_path()
    }
}
impl<'a> Prefix<'a> {
    /// Determines if the prefix is verbatim, i.e., begins with `\\?\`.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Prefix::*;
    /// use xrmt_stx::ffi::OsStr;
    ///
    /// assert!(Verbatim(OsStr::new("pictures")).is_verbatim());
    /// assert!(VerbatimUNC(OsStr::new("server"), OsStr::new("share")).is_verbatim());
    /// assert!(VerbatimDisk(b'C').is_verbatim());
    /// assert!(!DeviceNS(OsStr::new("BrainInterface")).is_verbatim());
    /// assert!(!UNC(OsStr::new("server"), OsStr::new("share")).is_verbatim());
    /// assert!(!Disk(b'C').is_verbatim());
    /// ```
    #[inline]
    pub fn is_verbatim(&self) -> bool {
        match self {
            Prefix::Verbatim(_) => true,
            Prefix::VerbatimUNC(..) => true,
            Prefix::VerbatimDisk(_) => true,
            _ => false,
        }
    }

    fn new(v: &'a OsStr) -> Option<Prefix<'a>> {
        let b = v.as_bytes();
        if v.len() > 4 && b[0] == b'\\' && b[1] == b'\\' && b[2] == b'?' && b[3] == b'\\' {
            return match (v.len(), b[5]) {
                (6, b'a'..=b'z' | b'A'..=b'Z') => Some(Prefix::VerbatimDisk(b[5])),
                (8.., b'u' | b'U') if (b[6] == b'n' || b[6] == b'N') && (b[7] == b'c' || b[7] == b'C') && b[8] == b'\\' => {
                    let p = &mut b[9..].splitn(2, |i| *i == b'\\');
                    Some(Prefix::VerbatimUNC(
                        p.next().map(OsStr::from_slice).unwrap_or_default(),
                        p.next().map(OsStr::from_slice).unwrap_or_default(),
                    ))
                },
                (..) => Some(Prefix::Verbatim(v)),
            };
        }
        match v.len() {
            4.. if b[0] == b'\\' && b[1] == b'\\' && b[2] == b'.' && b[3] == b'\\' => Some(Prefix::DeviceNS(v)),
            3.. if b[0] == b'\\' && b[1] == b'\\' => {
                let p = &mut b[3..].splitn(2, |i| *i == b'\\');
                Some(Prefix::UNC(
                    p.next().map(OsStr::from_slice).unwrap_or_default(),
                    p.next().map(OsStr::from_slice).unwrap_or_default(),
                ))
            },
            _ => None,
        }
    }

    #[inline]
    fn len(&self) -> usize {
        match self {
            Prefix::Disk(_) => 2,
            Prefix::VerbatimDisk(_) => 6,
            Prefix::Verbatim(v) => 4 + v.len(),
            Prefix::DeviceNS(v) => 4 + v.len(),
            Prefix::UNC(x, y) => 2 + x.len() + if !y.is_empty() { 1 + y.len() } else { 0 },
            Prefix::VerbatimUNC(x, y) => 8 + x.len() + if !y.is_empty() { 1 + y.len() } else { 0 },
        }
    }
    #[inline]
    fn component(&self, raw: &'a [u8]) -> Component<'a> {
        Component::Prefix(PrefixComponent {
            raw:    OsStr::from_slice(&raw[0..self.len()]),
            parsed: *self,
        })
    }
}
impl<'a> Component<'a> {
    /// Extracts the underlying [`OsStr`] slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// let path = Path::new("./tmp/foo/bar.txt");
    /// let components: Vec<_> = path.components().map(|comp| comp.as_os_str()).collect();
    /// assert_eq!(&components, &[".", "tmp", "foo", "bar.txt"]);
    /// ```
    #[inline]
    pub fn as_os_str(self) -> &'a OsStr {
        match self {
            Component::CurDir => OsStr::new(EXT_DOT),
            Component::RootDir => OsStr::new(MAIN_SEPARATOR_STR),
            Component::ParentDir => OsStr::new(EXT_DOT_DOT),
            Component::Normal(v) => v,
            Component::Prefix(p) => p.as_os_str(),
        }
    }
}
impl<'a> Components<'a> {
    /// Extracts a slice corresponding to the portion of the path remaining for
    /// iteration.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    ///
    /// let mut components = Path::new("/tmp/foo/bar.txt").components();
    /// components.next();
    /// components.next();
    ///
    /// assert_eq!(Path::new("foo/bar.txt"), components.as_path());
    /// ```
    #[inline]
    pub fn as_path(&self) -> &'a Path {
        Path::from_slice(self.i.0)
    }

    #[inline]
    fn new(v: &'a OsStr) -> Components<'a> {
        let b = v.as_bytes();
        Components {
            i:      DirSplit(b),
            s:      if b.first().map_or(false, sep) {
                STATE_ROOT_PRESENT
            } else {
                0
            },
            path:   b,
            prefix: Prefix::new(v),
        }
    }

    #[inline]
    fn is_root(&self) -> bool {
        if is_sep(self.path[0]) {
            return true;
        }
        match &self.prefix {
            Some(Prefix::Disk(_) | Prefix::VerbatimDisk(_) | Prefix::Verbatim(_)) => true,
            _ => false,
        }
    }
    fn next(&mut self) -> Option<Component<'a>> {
        if self.s & STATE_PREFIX_SHOWN == 0 {
            self.s |= STATE_PREFIX_SHOWN;
            if let Some(v) = self.prefix {
                return Some(v.component(self.path));
            }
        }
        let v = self.i.next()?;
        match v.len() {
            0 if self.s & STATE_ROOT_SHOWN == 0 => {
                self.s |= STATE_ROOT_SHOWN;
                Some(Component::RootDir)
            },
            1 if v[0] == b'.' => Some(Component::CurDir),
            2 if v[0] == b'.' && v[1] == b'.' => Some(Component::ParentDir),
            _ => Some(Component::Normal(OsStr::from_slice(v))),
        }
    }
    fn prev(&mut self) -> Option<Component<'a>> {
        if let Some(v) = self.i.next_back() {
            return match v.len() {
                0 if self.i.0.len() == 1 && self.s & STATE_PREFIX_SHOWN == 0 => {
                    self.s |= STATE_ROOT_SHOWN;
                    Some(Component::RootDir)
                },
                0 => self.prev(),
                1 if v[0] == b'.' => self.prev(),
                2 if v[0] == b'.' && v[1] == b'.' => Some(Component::ParentDir),
                _ => Some(Component::Normal(OsStr::from_slice(v))),
            };
        }
        if self.s & STATE_PREFIX_SHOWN == 0 {
            self.s |= STATE_PREFIX_SHOWN;
            if let Some(v) = self.prefix {
                return Some(v.component(self.path));
            }
        }
        None
    }
}
impl<'a> PrefixComponent<'a> {
    /// Returns the parsed prefix data.
    ///
    /// See [`Prefix`]'s documentation for more information on the different
    /// kinds of prefixes.
    #[inline]
    pub fn kind(&self) -> Prefix<'a> {
        self.parsed
    }
    /// Returns the raw [`OsStr`] slice for this prefix.
    #[inline]
    pub fn as_os_str(&self) -> &'a OsStr {
        self.raw
    }
}

impl Eq for Prefix<'_> {}
impl Ord for Prefix<'_> {
    #[inline]
    fn cmp(&self, other: &Prefix<'_>) -> Ordering {
        match (self, other) {
            (Prefix::Disk(x), Prefix::Disk(y)) => x.cmp(y),
            (Prefix::UNC(x, _), Prefix::UNC(y, _)) => x.cmp(y),
            (Prefix::DeviceNS(x), Prefix::DeviceNS(y)) => x.cmp(y),
            (Prefix::Verbatim(x), Prefix::Verbatim(y)) => x.cmp(y),
            (Prefix::VerbatimDisk(x), Prefix::VerbatimDisk(y)) => x.cmp(y),
            (Prefix::VerbatimUNC(x, _), Prefix::VerbatimUNC(y, _)) => x.cmp(y),
            _ => Ordering::Less,
        }
    }
}
impl Hash for Prefix<'_> {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        discriminant(self).hash(h);
    }
}
impl Copy for Prefix<'_> {}
impl<'a> Clone for Prefix<'a> {
    #[inline]
    fn clone(&self) -> Prefix<'a> {
        match self {
            Prefix::Disk(v) => Prefix::Disk(v.clone()),
            Prefix::DeviceNS(v) => Prefix::DeviceNS(*v),
            Prefix::Verbatim(v) => Prefix::Verbatim(*v),
            Prefix::UNC(v, x) => Prefix::UNC(*v, *x),
            Prefix::VerbatimDisk(v) => Prefix::VerbatimDisk(v.clone()),
            Prefix::VerbatimUNC(v, x) => Prefix::VerbatimUNC(*v, *x),
        }
    }
}
impl PartialOrd for Prefix<'_> {
    #[inline]
    fn partial_cmp(&self, other: &Prefix<'_>) -> Option<Ordering> {
        match (self, other) {
            (Prefix::Disk(x), Prefix::Disk(v)) => x.partial_cmp(v),
            (Prefix::UNC(x, _), Prefix::UNC(v, _)) => x.partial_cmp(v),
            (Prefix::DeviceNS(x), Prefix::DeviceNS(v)) => x.partial_cmp(v),
            (Prefix::Verbatim(x), Prefix::Verbatim(v)) => x.partial_cmp(v),
            (Prefix::VerbatimDisk(x), Prefix::VerbatimDisk(v)) => x.partial_cmp(v),
            (Prefix::VerbatimUNC(x, _), Prefix::VerbatimUNC(v, _)) => x.partial_cmp(v),
            _ => None,
        }
    }
}
impl<'a> PartialEq for Prefix<'a> {
    #[inline]
    fn eq(&self, other: &Prefix<'a>) -> bool {
        match (self, other) {
            (Prefix::Disk(x), Prefix::Disk(v)) => x.eq(v),
            (Prefix::DeviceNS(x), Prefix::DeviceNS(v)) => x.eq(v),
            (Prefix::Verbatim(x), Prefix::Verbatim(v)) => x.eq(v),
            (Prefix::VerbatimDisk(x), Prefix::VerbatimDisk(v)) => x.eq(v),
            (Prefix::UNC(x, v), Prefix::UNC(a, b)) => x.eq(a) && v.eq(b),
            (Prefix::VerbatimUNC(x, v), Prefix::VerbatimUNC(a, b)) => x.eq(a) && v.eq(b),
            _ => false,
        }
    }
}

impl Eq for PrefixComponent<'_> {}
impl Ord for PrefixComponent<'_> {
    #[inline]
    fn cmp(&self, other: &PrefixComponent<'_>) -> Ordering {
        self.parsed.cmp(&other.parsed)
    }
}
impl Hash for PrefixComponent<'_> {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.parsed.hash(h);
    }
}
impl Copy for PrefixComponent<'_> {}
impl PartialEq for PrefixComponent<'_> {
    #[inline]
    fn eq(&self, other: &PrefixComponent<'_>) -> bool {
        self.parsed.eq(&other.parsed)
    }
}
impl<'a> Clone for PrefixComponent<'a> {
    #[inline]
    fn clone(&self) -> PrefixComponent<'a> {
        PrefixComponent {
            raw:    self.raw,
            parsed: self.parsed.clone(),
        }
    }
}
impl PartialOrd for PrefixComponent<'_> {
    #[inline]
    fn partial_cmp(&self, other: &PrefixComponent<'_>) -> Option<Ordering> {
        self.raw.partial_cmp(other.raw)
    }
}

impl Eq for Component<'_> {}
impl Ord for Component<'_> {
    #[inline]
    fn cmp(&self, other: &Component<'_>) -> Ordering {
        match (self, other) {
            (Component::CurDir, Component::CurDir) => Ordering::Equal,
            (Component::RootDir, Component::RootDir) => Ordering::Equal,
            (Component::ParentDir, Component::ParentDir) => Ordering::Equal,
            (Component::Normal(x), Component::Normal(v)) => x.cmp(v),
            (Component::Prefix(x), Component::Prefix(v)) => x.cmp(v),
            _ => Ordering::Less,
        }
    }
}
impl Hash for Component<'_> {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        discriminant(self).hash(h);
    }
}
impl Copy for Component<'_> {}
impl<'a> Clone for Component<'a> {
    #[inline]
    fn clone(&self) -> Component<'a> {
        match self {
            Component::CurDir => Component::CurDir,
            Component::RootDir => Component::RootDir,
            Component::ParentDir => Component::ParentDir,
            Component::Normal(v) => Component::Normal(*v),
            Component::Prefix(v) => Component::Prefix(*v),
        }
    }
}
impl PartialOrd for Component<'_> {
    #[inline]
    fn partial_cmp(&self, other: &Component<'_>) -> Option<Ordering> {
        match (self, other) {
            (Component::CurDir, Component::CurDir) => Some(Ordering::Equal),
            (Component::RootDir, Component::RootDir) => Some(Ordering::Equal),
            (Component::ParentDir, Component::ParentDir) => Some(Ordering::Equal),
            (Component::Normal(x), Component::Normal(v)) => Some(x.cmp(v)),
            (Component::Prefix(x), Component::Prefix(v)) => Some(x.cmp(v)),
            _ => None,
        }
    }
}
impl AsRef<Path> for Component<'_> {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.as_os_str().as_ref()
    }
}
impl AsRef<OsStr> for Component<'_> {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self.as_os_str()
    }
}
impl<'a> PartialEq for Component<'a> {
    #[inline]
    fn eq(&self, other: &Component<'a>) -> bool {
        match (self, other) {
            (Component::CurDir, Component::CurDir) => true,
            (Component::RootDir, Component::RootDir) => true,
            (Component::ParentDir, Component::ParentDir) => true,
            (Component::Normal(x), Component::Normal(v)) => x.eq(v),
            (Component::Prefix(x), Component::Prefix(v)) => x.eq(v),
            _ => false,
        }
    }
}

impl<'a> Clone for DirSplit<'a> {
    #[inline]
    fn clone(&self) -> DirSplit<'a> {
        DirSplit(self.0)
    }
}
impl<'a> Clone for Components<'a> {
    #[inline]
    fn clone(&self) -> Components<'a> {
        Components {
            i:      self.i.clone(),
            s:      self.s,
            path:   self.path,
            prefix: self.prefix,
        }
    }
}
impl<'a> Iterator for DirSplit<'a> {
    type Item = &'a [u8];

    #[inline]
    fn next(&mut self) -> Option<&'a [u8]> {
        if self.0.is_empty() {
            return None;
        }
        match self.0.iter().position(sep) {
            Some(i) => Some(unsafe { replace(&mut self.0, self.0.get_unchecked(i + 1..)).get_unchecked(0..i) }),
            None => Some(unsafe { replace(&mut self.0, &[]) }),
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.0.is_empty() {
            (0, Some(0))
        } else {
            (1, Some(self.0.len() + 1))
        }
    }
}
impl FusedIterator for DirSplit<'_> {}
impl<'a> DoubleEndedIterator for DirSplit<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a [u8]> {
        if self.0.is_empty() {
            return None;
        }
        match self.0.iter().rposition(sep) {
            Some(i) => Some(unsafe {
                replace(
                    &mut self.0,
                    self.0.get_unchecked(0..i + if i == 0 { 1 } else { 0 }),
                )
                .get_unchecked(i + 1..)
            }),
            None => Some(unsafe { replace(&mut self.0, &[]) }),
        }
    }
}

impl AsRef<Path> for Iter<'_> {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}
impl AsRef<OsStr> for Iter<'_> {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self.as_path().as_os_str()
    }
}
impl<'a> Iterator for Iter<'a> {
    type Item = &'a OsStr;

    #[inline]
    fn next(&mut self) -> Option<&'a OsStr> {
        self.0.next().map(|v| v.as_os_str())
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl FusedIterator for Iter<'_> {}
impl<'a> DoubleEndedIterator for Iter<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a OsStr> {
        self.0.next_back().map(|v| v.as_os_str())
    }
}

impl Eq for Components<'_> {}
impl Ord for Components<'_> {
    #[inline]
    fn cmp(&self, other: &Components<'_>) -> Ordering {
        self.path.cmp(other.path)
    }
}
impl AsRef<Path> for Components<'_> {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}
impl AsRef<OsStr> for Components<'_> {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self.as_path().as_os_str()
    }
}
impl<'a> Iterator for Components<'a> {
    type Item = Component<'a>;

    #[inline]
    fn next(&mut self) -> Option<Component<'a>> {
        self.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.i.size_hint()
    }
}
impl<'a> PartialEq for Components<'a> {
    #[inline]
    fn eq(&self, other: &Components<'a>) -> bool {
        self.path.eq(other.path)
    }
}
impl FusedIterator for Components<'_> {}
impl<'a> PartialOrd for Components<'a> {
    #[inline]
    fn partial_cmp(&self, other: &Components<'a>) -> Option<Ordering> {
        self.path.partial_cmp(other.path)
    }
}
impl<'a> DoubleEndedIterator for Components<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<Component<'a>> {
        self.prev()
    }
}

impl Copy for Ancestors<'_> {}
impl<'a> Clone for Ancestors<'a> {
    #[inline]
    fn clone(&self) -> Ancestors<'a> {
        Ancestors(self.0.clone())
    }
}
impl<'a> Iterator for Ancestors<'a> {
    type Item = &'a Path;

    #[inline]
    fn next(&mut self) -> Option<&'a Path> {
        unsafe { replace(&mut self.0, self.0?.parent()) }
    }
}
impl FusedIterator for Ancestors<'_> {}

impl Eq for Path {}
impl Ord for Path {
    #[inline]
    fn cmp(&self, other: &Path) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl Hash for Path {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.hash(h);
    }
}
impl ToOwned for Path {
    type Owned = PathBuf;

    #[inline]
    fn to_owned(&self) -> PathBuf {
        self.to_path_buf()
    }
    #[inline]
    fn clone_into(&self, v: &mut PathBuf) {
        self.0.clone_into(&mut v.0);
    }
}
impl PartialEq for Path {
    #[inline]
    fn eq(&self, other: &Path) -> bool {
        self.0.eq(&other.0)
    }
}
impl PartialOrd for Path {
    #[inline]
    fn partial_cmp(&self, other: &Path) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl AsRef<Path> for Path {
    #[inline]
    fn as_ref(&self) -> &Path {
        self
    }
}
impl AsRef<OsStr> for Path {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        &self.0
    }
}
impl PartialEq<OsStr> for Path {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        self.0.eq(other)
    }
}
impl PartialOrd<OsStr> for Path {
    #[inline]
    fn partial_cmp(&self, other: &OsStr) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}
impl PartialEq<PathBuf> for Path {
    #[inline]
    fn eq(&self, other: &PathBuf) -> bool {
        self.0.eq(&other.0)
    }
}
impl PartialOrd<PathBuf> for Path {
    #[inline]
    fn partial_cmp(&self, other: &PathBuf) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl PartialEq<OsString> for Path {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        self.0.eq(other)
    }
}
impl PartialOrd<OsString> for Path {
    #[inline]
    fn partial_cmp(&self, other: &OsString) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}
impl<'a> IntoIterator for &'a Path {
    type Item = &'a OsStr;
    type IntoIter = Iter<'a>;

    #[inline]
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}
impl<'a> PartialEq<OsStr> for &'a Path {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        self.0.eq(other)
    }
}
impl<'a> PartialEq<&'a OsStr> for Path {
    #[inline]
    fn eq(&self, other: &&'a OsStr) -> bool {
        self.0.eq(*other)
    }
}
impl<'a> PartialOrd<OsStr> for &'a Path {
    #[inline]
    fn partial_cmp(&self, other: &OsStr) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}
impl<'a> PartialOrd<&'a OsStr> for Path {
    #[inline]
    fn partial_cmp(&self, other: &&'a OsStr) -> Option<Ordering> {
        self.0.partial_cmp(*other)
    }
}
impl<'a> PartialEq<PathBuf> for &'a Path {
    #[inline]
    fn eq(&self, other: &PathBuf) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a> PartialEq<OsString> for &'a Path {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        self.0.eq(other)
    }
}
impl<'a> PartialOrd<PathBuf> for &'a Path {
    #[inline]
    fn partial_cmp(&self, other: &PathBuf) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl<'a> PartialOrd<OsString> for &'a Path {
    #[inline]
    fn partial_cmp(&self, other: &OsString) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}
impl<'a> PartialEq<Cow<'a, Path>> for Path {
    #[inline]
    fn eq(&self, other: &Cow<'a, Path>) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a> PartialOrd<Cow<'a, Path>> for Path {
    #[inline]
    fn partial_cmp(&self, other: &Cow<'a, Path>) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl<'a> PartialEq<Cow<'a, OsStr>> for Path {
    #[inline]
    fn eq(&self, other: &Cow<'a, OsStr>) -> bool {
        self.0.eq(other)
    }
}
impl<'a> PartialOrd<Cow<'a, OsStr>> for Path {
    #[inline]
    fn partial_cmp(&self, other: &Cow<'a, OsStr>) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}
impl<'a, 'b> PartialEq<Cow<'a, Path>> for &'b Path {
    #[inline]
    fn eq(&self, other: &Cow<'a, Path>) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a, 'b> PartialEq<Cow<'b, OsStr>> for &'a Path {
    #[inline]
    fn eq(&self, other: &Cow<'b, OsStr>) -> bool {
        self.0.eq(other)
    }
}
impl<'a, 'b> PartialOrd<Cow<'a, Path>> for &'b Path {
    #[inline]
    fn partial_cmp(&self, other: &Cow<'a, Path>) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl<'a, 'b> PartialOrd<Cow<'b, OsStr>> for &'a Path {
    #[inline]
    fn partial_cmp(&self, other: &Cow<'b, OsStr>) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl Eq for PathBuf {}
impl Ord for PathBuf {
    #[inline]
    fn cmp(&self, other: &PathBuf) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl Hash for PathBuf {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.as_path().hash(h)
    }
}
impl Clone for PathBuf {
    #[inline]
    fn clone(&self) -> PathBuf {
        PathBuf(self.0.clone())
    }
    #[inline]
    fn clone_from(&mut self, v: &PathBuf) {
        self.0.clone_from(&v.0)
    }
}
impl Deref for PathBuf {
    type Target = Path;

    #[inline]
    fn deref(&self) -> &Path {
        Path::new(&self.0)
    }
}
impl Default for PathBuf {
    #[inline]
    fn default() -> PathBuf {
        PathBuf::new()
    }
}
impl FromStr for PathBuf {
    type Err = Infallible;

    #[inline]
    fn from_str(v: &str) -> Result<PathBuf, Infallible> {
        Ok(PathBuf::from(v))
    }
}
impl DerefMut for PathBuf {
    #[inline]
    fn deref_mut(&mut self) -> &mut Path {
        Path::from_mut(&mut self.0)
    }
}
impl PartialEq for PathBuf {
    #[inline]
    fn eq(&self, other: &PathBuf) -> bool {
        self.0.eq(&other.0)
    }
}
impl PartialOrd for PathBuf {
    #[inline]
    fn partial_cmp(&self, other: &PathBuf) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl AsRef<Path> for PathBuf {
    #[inline]
    fn as_ref(&self) -> &Path {
        self
    }
}
impl Borrow<Path> for PathBuf {
    #[inline]
    fn borrow(&self) -> &Path {
        self.deref()
    }
}
impl AsRef<OsStr> for PathBuf {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        &self.0[..]
    }
}
impl From<String> for PathBuf {
    #[inline]
    fn from(v: String) -> PathBuf {
        PathBuf::from(OsString::from(v))
    }
}
impl From<OsString> for PathBuf {
    #[inline]
    fn from(v: OsString) -> PathBuf {
        PathBuf(v)
    }
}
impl From<Box<Path>> for PathBuf {
    #[inline]
    fn from(v: Box<Path>) -> PathBuf {
        v.into_path_buf()
    }
}
impl PartialEq<Path> for PathBuf {
    #[inline]
    fn eq(&self, other: &Path) -> bool {
        self.0.eq(&other.0)
    }
}
impl PartialOrd<Path> for PathBuf {
    #[inline]
    fn partial_cmp(&self, other: &Path) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl PartialEq<OsStr> for PathBuf {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        self.0.eq(other)
    }
}
impl PartialOrd<OsStr> for PathBuf {
    #[inline]
    fn partial_cmp(&self, other: &OsStr) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}
impl PartialEq<OsString> for PathBuf {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        self.0.eq(other)
    }
}
impl PartialOrd<OsString> for PathBuf {
    #[inline]
    fn partial_cmp(&self, other: &OsString) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}
impl<'a> IntoIterator for &'a PathBuf {
    type Item = &'a OsStr;
    type IntoIter = Iter<'a>;

    #[inline]
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}
impl<'a> PartialEq<&'a Path> for PathBuf {
    #[inline]
    fn eq(&self, other: &&'a Path) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a> From<Cow<'a, Path>> for PathBuf {
    #[inline]
    fn from(v: Cow<'a, Path>) -> PathBuf {
        v.into_owned()
    }
}
impl<'a> PartialOrd<&'a Path> for PathBuf {
    #[inline]
    fn partial_cmp(&self, other: &&'a Path) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<'a> PartialEq<&'a OsStr> for PathBuf {
    #[inline]
    fn eq(&self, other: &&'a OsStr) -> bool {
        self.0.eq(other)
    }
}
impl<'a> PartialOrd<&'a OsStr> for PathBuf {
    #[inline]
    fn partial_cmp(&self, other: &&'a OsStr) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}
impl<P: AsRef<Path>> Extend<P> for PathBuf {
    #[inline]
    fn extend_one(&mut self, v: P) {
        self.push(v.as_ref());
    }
    #[inline]
    fn extend<I: IntoIterator<Item = P>>(&mut self, i: I) {
        i.into_iter().for_each(move |p| self.push(p.as_ref()));
    }
}
impl<'a> PartialEq<Cow<'a, Path>> for PathBuf {
    #[inline]
    fn eq(&self, other: &Cow<'a, Path>) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a> PartialOrd<Cow<'a, Path>> for PathBuf {
    #[inline]
    fn partial_cmp(&self, other: &Cow<'a, Path>) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl<'a> PartialEq<Cow<'a, OsStr>> for PathBuf {
    #[inline]
    fn eq(&self, other: &Cow<'a, OsStr>) -> bool {
        self.0.eq(other)
    }
}
impl<'a> PartialOrd<Cow<'a, OsStr>> for PathBuf {
    #[inline]
    fn partial_cmp(&self, other: &Cow<'a, OsStr>) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}
impl<P: AsRef<Path>> FromIterator<P> for PathBuf {
    #[inline]
    fn from_iter<I: IntoIterator<Item = P>>(i: I) -> PathBuf {
        let mut v = PathBuf::new();
        v.extend(i);
        v
    }
}
impl<T: ?Sized + AsRef<OsStr>> From<&T> for PathBuf {
    #[inline]
    fn from(v: &T) -> PathBuf {
        PathBuf::from(v.as_ref().to_os_string())
    }
}

impl Clone for Box<Path> {
    #[inline]
    fn clone(&self) -> Box<Path> {
        self.to_path_buf().into_boxed_path()
    }
}
impl From<&Path> for Box<Path> {
    #[inline]
    fn from(v: &Path) -> Box<Path> {
        unsafe { Box::from_raw(Box::into_raw(v.0.to_box()) as *mut Path) }
    }
}
impl From<PathBuf> for Box<Path> {
    #[inline]
    fn from(v: PathBuf) -> Box<Path> {
        v.into_boxed_path()
    }
}
impl From<&mut Path> for Box<Path> {
    fn from(v: &mut Path) -> Box<Path> {
        unsafe { Box::from_raw(Box::into_raw(v.0.to_box()) as *mut Path) }
    }
}
impl From<Cow<'_, Path>> for Box<Path> {
    #[inline]
    fn from(v: Cow<'_, Path>) -> Box<Path> {
        match v {
            Cow::Owned(x) => Box::from(x),
            Cow::Borrowed(x) => Box::from(x),
        }
    }
}

impl From<&Path> for Arc<Path> {
    #[inline]
    fn from(v: &Path) -> Arc<Path> {
        unsafe { Arc::from_raw(Arc::into_raw(v.0.to_arc()) as *const Path) }
    }
}
impl From<PathBuf> for Arc<Path> {
    #[inline]
    fn from(v: PathBuf) -> Arc<Path> {
        unsafe { Arc::from_raw(Arc::into_raw(v.0.into_arc()) as *const Path) }
    }
}
impl From<&mut Path> for Arc<Path> {
    #[inline]
    fn from(v: &mut Path) -> Arc<Path> {
        Arc::from(&*v)
    }
}

impl From<&Path> for Rc<Path> {
    #[inline]
    fn from(v: &Path) -> Rc<Path> {
        unsafe { Rc::from_raw(Rc::into_raw(v.0.to_rc()) as *const Path) }
    }
}
impl From<PathBuf> for Rc<Path> {
    #[inline]
    fn from(v: PathBuf) -> Rc<Path> {
        unsafe { Rc::from_raw(Rc::into_raw(v.0.into_rc()) as *const Path) }
    }
}
impl From<&mut Path> for Rc<Path> {
    #[inline]
    fn from(v: &mut Path) -> Rc<Path> {
        Rc::from(&*v)
    }
}

impl<'a> From<PathBuf> for Cow<'a, Path> {
    #[inline]
    fn from(v: PathBuf) -> Cow<'a, Path> {
        Cow::Owned(v)
    }
}
impl<'a> From<&'a Path> for Cow<'a, Path> {
    #[inline]
    fn from(v: &'a Path) -> Cow<'a, Path> {
        Cow::Borrowed(v)
    }
}
impl<'a> PartialEq<Path> for Cow<'a, Path> {
    #[inline]
    fn eq(&self, other: &Path) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a> PartialEq<OsStr> for Cow<'a, Path> {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        self.0.eq(other)
    }
}
impl<'a> PartialOrd<Path> for Cow<'a, Path> {
    #[inline]
    fn partial_cmp(&self, other: &Path) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<'a> PartialOrd<OsStr> for Cow<'a, Path> {
    #[inline]
    fn partial_cmp(&self, other: &OsStr) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}
impl<'a> From<&'a PathBuf> for Cow<'a, Path> {
    #[inline]
    fn from(v: &'a PathBuf) -> Cow<'a, Path> {
        Cow::Borrowed(v.as_path())
    }
}
impl<'a> PartialEq<PathBuf> for Cow<'a, Path> {
    #[inline]
    fn eq(&self, other: &PathBuf) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a> PartialEq<OsString> for Cow<'a, Path> {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        self.0.eq(other)
    }
}
impl<'a> PartialOrd<PathBuf> for Cow<'a, Path> {
    #[inline]
    fn partial_cmp(&self, other: &PathBuf) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<'a> PartialOrd<OsString> for Cow<'a, Path> {
    #[inline]
    fn partial_cmp(&self, other: &OsString) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}
impl<'a, 'b> PartialEq<&'b Path> for Cow<'a, Path> {
    #[inline]
    fn eq(&self, other: &&'b Path) -> bool {
        self.0.eq(&other.0)
    }
}
impl<'a, 'b> PartialEq<&'b OsStr> for Cow<'a, Path> {
    #[inline]
    fn eq(&self, other: &&'b OsStr) -> bool {
        self.0.eq(*other)
    }
}
impl<'a, 'b> PartialOrd<&'b Path> for Cow<'a, Path> {
    #[inline]
    fn partial_cmp(&self, other: &&'b Path) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<'a, 'b> PartialOrd<&'b OsStr> for Cow<'a, Path> {
    #[inline]
    fn partial_cmp(&self, other: &&'b OsStr) -> Option<Ordering> {
        self.0.partial_cmp(*other)
    }
}

impl Eq for StripPrefixError {}
impl Clone for StripPrefixError {
    #[inline]
    fn clone(&self) -> StripPrefixError {
        StripPrefixError(())
    }
}
impl PartialEq for StripPrefixError {
    #[inline]
    fn eq(&self, _other: &StripPrefixError) -> bool {
        true
    }
}

impl AsRef<Path> for str {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}
impl AsRef<Path> for OsStr {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}
impl AsRef<Path> for String {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}
impl AsRef<Path> for OsString {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}
impl AsRef<Path> for Cow<'_, OsStr> {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl From<PathBuf> for OsString {
    #[inline]
    fn from(v: PathBuf) -> OsString {
        v.0
    }
}
impl PartialEq<Path> for OsString {
    #[inline]
    fn eq(&self, other: &Path) -> bool {
        self.eq(&other.0)
    }
}
impl PartialOrd<Path> for OsString {
    #[inline]
    fn partial_cmp(&self, other: &Path) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl PartialEq<PathBuf> for OsString {
    #[inline]
    fn eq(&self, other: &PathBuf) -> bool {
        self.eq(&other.0)
    }
}
impl PartialOrd<PathBuf> for OsString {
    #[inline]
    fn partial_cmp(&self, other: &PathBuf) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl<'a> PartialEq<&'a Path> for OsString {
    #[inline]
    fn eq(&self, other: &&'a Path) -> bool {
        self.eq(&other.0)
    }
}
impl<'a> PartialOrd<&'a Path> for OsString {
    #[inline]
    fn partial_cmp(&self, other: &&'a Path) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl<'a> PartialEq<Cow<'a, Path>> for OsString {
    #[inline]
    fn eq(&self, other: &Cow<'a, Path>) -> bool {
        self.eq(&other.0)
    }
}
impl<'a> PartialOrd<Cow<'a, Path>> for OsString {
    #[inline]
    fn partial_cmp(&self, other: &Cow<'a, Path>) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl PartialEq<Path> for OsStr {
    #[inline]
    fn eq(&self, other: &Path) -> bool {
        self.eq(&other.0)
    }
}
impl PartialOrd<Path> for OsStr {
    #[inline]
    fn partial_cmp(&self, other: &Path) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl PartialEq<PathBuf> for OsStr {
    #[inline]
    fn eq(&self, other: &PathBuf) -> bool {
        self.eq(&other.0)
    }
}
impl PartialOrd<PathBuf> for OsStr {
    #[inline]
    fn partial_cmp(&self, other: &PathBuf) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl<'a> PartialEq<Path> for &'a OsStr {
    #[inline]
    fn eq(&self, other: &Path) -> bool {
        (*self).eq(&other.0)
    }
}
impl<'a> PartialEq<&'a Path> for OsStr {
    #[inline]
    fn eq(&self, other: &&'a Path) -> bool {
        self.eq(&other.0)
    }
}
impl<'a> PartialOrd<Path> for &'a OsStr {
    #[inline]
    fn partial_cmp(&self, other: &Path) -> Option<Ordering> {
        (*self).partial_cmp(&other.0)
    }
}
impl<'a> PartialOrd<&'a Path> for OsStr {
    #[inline]
    fn partial_cmp(&self, other: &&'a Path) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl<'a> PartialEq<PathBuf> for &'a OsStr {
    #[inline]
    fn eq(&self, other: &PathBuf) -> bool {
        self.eq(&other.0)
    }
}
impl<'a> PartialOrd<PathBuf> for &'a OsStr {
    #[inline]
    fn partial_cmp(&self, other: &PathBuf) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl<'a> PartialEq<Cow<'a, Path>> for OsStr {
    #[inline]
    fn eq(&self, other: &Cow<'a, Path>) -> bool {
        self.eq(&other.0)
    }
}
impl<'a> PartialOrd<Cow<'a, Path>> for OsStr {
    #[inline]
    fn partial_cmp(&self, other: &Cow<'a, Path>) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl<'a, 'b> PartialEq<Cow<'a, Path>> for &'b OsStr {
    #[inline]
    fn eq(&self, other: &Cow<'a, Path>) -> bool {
        (*self).eq(&other.0)
    }
}
impl<'a, 'b> PartialOrd<Cow<'a, Path>> for &'b OsStr {
    #[inline]
    fn partial_cmp(&self, other: &Cow<'a, Path>) -> Option<Ordering> {
        (*self).partial_cmp(&other.0)
    }
}

impl<'a> PartialEq<Path> for Cow<'a, OsStr> {
    #[inline]
    fn eq(&self, other: &Path) -> bool {
        self.eq(&other.0)
    }
}
impl<'a> PartialOrd<Path> for Cow<'a, OsStr> {
    #[inline]
    fn partial_cmp(&self, other: &Path) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl<'a> PartialEq<PathBuf> for Cow<'a, OsStr> {
    #[inline]
    fn eq(&self, other: &PathBuf) -> bool {
        self.eq(&other.0)
    }
}
impl<'a> PartialOrd<PathBuf> for Cow<'a, OsStr> {
    #[inline]
    fn partial_cmp(&self, other: &PathBuf) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl<'a, 'b> PartialEq<&'a Path> for Cow<'b, OsStr> {
    #[inline]
    fn eq(&self, other: &&'a Path) -> bool {
        self.eq(&other.0)
    }
}
impl<'a, 'b> PartialOrd<&'a Path> for Cow<'b, OsStr> {
    #[inline]
    fn partial_cmp(&self, other: &&'a Path) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl Debug for StripPrefixError {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("StripPrefixError")
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("0x404")
    }
}
impl Error for StripPrefixError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Display for StripPrefixError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
    }
}

impl From<Char> for PathBuf {
    #[inline]
    fn from(v: Char) -> PathBuf {
        v.into_string().into()
    }
}
impl<'a> From<CharPtr<'a>> for PathBuf {
    #[inline]
    fn from(v: CharPtr<'a>) -> PathBuf {
        v.into_string().into()
    }
}
impl<'a> From<CharLike<'a>> for PathBuf {
    #[inline]
    fn from(v: CharLike<'a>) -> PathBuf {
        v.into_string().into()
    }
}
impl<'a> From<CharSlice<'a>> for PathBuf {
    #[inline]
    fn from(v: CharSlice<'a>) -> PathBuf {
        v.into_string().into()
    }
}

impl From<WChar> for PathBuf {
    #[inline]
    fn from(v: WChar) -> PathBuf {
        v.into_string().into()
    }
}
impl<'a> From<WCharPtr<'a>> for PathBuf {
    #[inline]
    fn from(v: WCharPtr<'a>) -> PathBuf {
        v.into_string().into()
    }
}
impl<'a> From<WCharLike<'a>> for PathBuf {
    #[inline]
    fn from(v: WCharLike<'a>) -> PathBuf {
        v.into_string().into()
    }
}
impl<'a> From<WCharSlice<'a>> for PathBuf {
    #[inline]
    fn from(v: WCharSlice<'a>) -> PathBuf {
        v.into_string().into()
    }
}

impl From<&Path> for Char {
    #[inline]
    fn from(v: &Path) -> Char {
        Char::from(v.as_bytes())
    }
}
impl From<PathBuf> for Char {
    #[inline]
    fn from(v: PathBuf) -> Char {
        Char::from(v.as_bytes())
    }
}
impl From<&PathBuf> for Char {
    #[inline]
    fn from(v: &PathBuf) -> Char {
        Char::from(v.as_bytes())
    }
}

impl<'a> From<&'a Path> for CharPtr<'a> {
    #[inline]
    fn from(v: &'a Path) -> CharPtr<'a> {
        CharPtr::from(v.as_bytes())
    }
}
impl<'a> From<&'a PathBuf> for CharPtr<'a> {
    #[inline]
    fn from(v: &'a PathBuf) -> CharPtr<'a> {
        CharPtr::from(v.as_bytes())
    }
}

impl<'a> From<PathBuf> for CharLike<'a> {
    #[inline]
    fn from(v: PathBuf) -> CharLike<'a> {
        CharLike::Owned(Char::from(v))
    }
}
impl<'a> From<&'a Path> for CharLike<'a> {
    #[inline]
    fn from(v: &'a Path) -> CharLike<'a> {
        CharLike::Slice(CharSlice::from(v))
    }
}
impl<'a> From<&'a PathBuf> for CharLike<'a> {
    #[inline]
    fn from(v: &'a PathBuf) -> CharLike<'a> {
        CharLike::Slice(CharSlice::from(v))
    }
}

impl<'a> From<&'a Path> for CharSlice<'a> {
    #[inline]
    fn from(v: &'a Path) -> CharSlice<'a> {
        CharSlice::from(v.as_bytes())
    }
}
impl<'a> From<&'a PathBuf> for CharSlice<'a> {
    #[inline]
    fn from(v: &'a PathBuf) -> CharSlice<'a> {
        CharSlice::from(v.as_bytes())
    }
}

impl From<&Path> for WChar {
    #[inline]
    fn from(v: &Path) -> WChar {
        WChar::from(v.as_bytes())
    }
}
impl From<PathBuf> for WChar {
    #[inline]
    fn from(v: PathBuf) -> WChar {
        WChar::from(v.as_bytes())
    }
}
impl From<&PathBuf> for WChar {
    #[inline]
    fn from(v: &PathBuf) -> WChar {
        WChar::from(v.as_bytes())
    }
}

impl<'a> From<&Path> for WCharLike<'a> {
    #[inline]
    fn from(v: &Path) -> WCharLike<'a> {
        WCharLike::Owned(WChar::from(v))
    }
}
impl<'a> From<PathBuf> for WCharLike<'a> {
    #[inline]
    fn from(v: PathBuf) -> WCharLike<'a> {
        WCharLike::Owned(WChar::from(v))
    }
}
impl<'a> From<&PathBuf> for WCharLike<'a> {
    #[inline]
    fn from(v: &PathBuf) -> WCharLike<'a> {
        WCharLike::Owned(WChar::from(v))
    }
}

impl From<Fiber> for PathBuf {
    #[inline]
    fn from(v: Fiber) -> PathBuf {
        PathBuf(v.into())
    }
}

impl<A: Allocator> AsRef<Path> for Fiber<A> {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

/// Determines whether the character is one of the permitted path
/// separators for the current platform.
///
/// # Examples
///
/// ```
/// use xrmt_stx::path;
///
/// assert!(path::is_separator('/')); // '/' works for both Unix and Windows
/// assert!(!path::is_separator('❤'));
/// ```
#[inline]
pub fn is_separator(c: char) -> bool {
    c.is_ascii() && is_sep(c as u8)
}
/// Makes the path absolute without accessing the filesystem.
///
/// If the path is relative, the current directory is used as the base
/// directory. All intermediate components will be resolved according to
/// platform-specific rules, but unlike
/// [`canonicalize`][crate::fs::canonicalize], this does not resolve symlinks
/// and may succeed even if the path does not exist.
///
/// If the `path` is empty or getting the
/// [current directory][crate::env::current_dir] fails, then an error will be
/// returned.
///
/// # Platform-specific behavior
///
/// On POSIX platforms, the path is resolved using [POSIX
/// semantics][posix-semantics], except that it stops short of resolving
/// symlinks. This means it will keep `..` components and trailing slashes.
///
/// On Windows, for verbatim paths, this will simply return the path as given.
/// For other paths, this is currently equivalent to calling
/// [`GetFullPathNameW`][windows-path].
///
/// Note that these [may change in the future][changes].
///
/// # Errors
///
/// This function may return an error in the following situations:
///
/// * If `path` is syntactically invalid; in particular, if it is empty.
/// * If getting the [current directory][crate::env::current_dir] fails.
///
/// # Examples
///
/// ## POSIX paths
///
/// ```
/// # #[cfg(unix)]
/// fn main() -> xrmt_stx::IoResult<()> {
///     use xrmt_stx::path::{self, Path};
///
///     // Relative to absolute
///     let absolute = path::absolute("foo/./bar")?;
///     assert!(absolute.ends_with("foo/bar"));
///
///     // Absolute to absolute
///     let absolute = path::absolute("/foo//test/.././bar.rs")?;
///     assert_eq!(absolute, Path::new("/foo/test/../bar.rs"));
///     Ok(())
/// }
/// # #[cfg(not(unix))]
/// # fn main() {}
/// ```
///
/// ## Windows paths
///
/// ```
/// # #[cfg(windows)]
/// fn main() -> xrmt_stx::IoResult<()> {
///     use xrmt_stx::path::{self, Path};
///
///     // Relative to absolute
///     let absolute = path::absolute("foo/./bar")?;
///     assert!(absolute.ends_with(r"foo\bar"));
///
///     // Absolute to absolute
///     let absolute = path::absolute(r"C:\foo//test\..\./bar.rs")?;
///
///     assert_eq!(absolute, Path::new(r"C:\foo\bar.rs"));
///     Ok(())
/// }
/// # #[cfg(not(windows))]
/// # fn main() {}
/// ```
///
/// Note that this [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
/// [posix-semantics]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap04.html#tag_04_13
/// [windows-path]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfullpathnamew
#[inline]
pub fn absolute(path: impl AsRef<Path>) -> IoResult<PathBuf> {
    Ok(path_normalize(path.as_ref()).into())
}

#[inline]
fn sep(v: &u8) -> bool {
    is_sep(*v)
}
#[inline]
fn is_sep(v: u8) -> bool {
    v == SEP_SLASH || v == SEP_BACKSLASH
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Formatter, Result};

    use crate::path::{Path, PathBuf};

    impl Debug for Path {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.0, f)
        }
    }
    impl Debug for PathBuf {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.0, f)
        }
    }
}
