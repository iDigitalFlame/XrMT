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

//! Inspection and manipulation of the process's environment.
//!
//! This module contains functions to inspect various aspects such as
//! environment variables, process arguments, the current directory, and various
//! other important directories.
//!
//! There are several functions and structs in this module that have a
//! counterpart ending in `os`. Those ending in `os` will return an [`OsString`]
//! and those without will return a [`String`].

#![no_implicit_prelude]
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate alloc;
extern crate core;

extern crate xrmt_crypt;
extern crate xrmt_data;
extern crate xrmt_winapi;

use alloc::string::String;
use alloc::vec::{IntoIter, Vec};
use core::clone::Clone;
use core::convert::{AsRef, From, Into};
use core::error::Error;
use core::fmt::{Debug, Display, Formatter};
use core::iter::{DoubleEndedIterator, ExactSizeIterator, FusedIterator, IntoIterator, Iterator};
use core::marker::{PhantomData, Sized};
use core::option::Option::{self, None, Some};
use core::result::Result::{self, Err, Ok};

use xrmt_data::text::utf16_to_vec;
use xrmt_winapi::functions::{current_token, GetCommandLine, GetCurrentDirectory, GetEnvironment, GetEnvironmentVariable, GetModuleFileName, GetTempPath, GetUserProfileDirectory, SetCurrentDirectory, SetEnvironmentVariable};
use xrmt_winapi::str_const;
use xrmt_winapi::structs::{Handle, StringLikeU16, WCharSlice};

use crate::ffi::{OsStr, OsString};
use crate::io::{FmtResult, IoResult};
use crate::path::PathBuf;

/// The error type for operations interacting with environment variables.
/// Possibly returned from [`env::var()`].
///
/// [`env::var()`]: var
pub enum VarError {
    NotPresent,
    NotUnicode(OsString),
}

/// An iterator over the arguments of a process, yielding an [`OsString`] value
/// for each argument.
///
/// This struct is created by [`env::args_os()`]. See its documentation
/// for more.
///
/// The first element is traditionally the path of the executable, but it can be
/// set to arbitrary text, and might not even exist. This means this property
/// should not be relied upon for security purposes.
///
/// [`env::args_os()`]: args_os
pub struct ArgsOs {
    v:  IntoIter<OsString>,
    _p: PhantomData<*mut ()>, // Make it not Send + Sync
}
/// An iterator over a snapshot of the environment variables of this process.
///
/// This structure is created by [`env::vars_os()`]. See its documentation for
/// more.
///
/// [`env::vars_os()`]: vars_os
pub struct VarsOs {
    v:  IntoIter<(OsString, OsString)>,
    _p: PhantomData<*mut ()>, // Make it not Send + Sync
}
/// An iterator over the arguments of a process, yielding a [`String`] value for
/// each argument.
///
/// This struct is created by [`env::args()`]. See its documentation
/// for more.
///
/// The first element is traditionally the path of the executable, but it can be
/// set to arbitrary text, and might not even exist. This means this property
/// should not be relied upon for security purposes.
///
/// [`env::args()`]: args
pub struct Args(ArgsOs);
/// An iterator over a snapshot of the environment variables of this process.
///
/// This structure is created by [`env::vars()`]. See its documentation for
/// more.
///
/// [`env::vars()`]: vars
pub struct Vars(VarsOs);
/// The error type for operations on the `PATH` variable. Possibly returned from
/// [`env::join_paths()`].
///
/// [`env::join_paths()`]: join_paths
pub struct JoinPathsError(());

/// An iterator that splits an environment variable into paths according to
/// platform-specific conventions.
///
/// The iterator element type is [`PathBuf`].
///
/// This structure is created by [`env::split_paths()`]. See its
/// documentation for more.
///
/// [`env::split_paths()`]: split_paths
pub struct SplitPaths(IntoIter<PathBuf>);

impl Clone for VarError {
    #[inline]
    fn clone(&self) -> VarError {
        match self {
            VarError::NotUnicode(v) => VarError::NotUnicode(v.clone()),
            VarError::NotPresent => VarError::NotPresent,
        }
    }
}
impl Debug for VarError {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("VarError")
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("0x404")
    }
}
impl Error for VarError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Display for VarError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
    }
}

impl Debug for JoinPathsError {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("JoinPathsError")
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("0x400")
    }
}
impl Error for JoinPathsError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Display for JoinPathsError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
    }
}

impl Iterator for Args {
    type Item = String;

    #[inline]
    fn next(&mut self) -> Option<String> {
        self.0.v.next().map(|v| v.to_string_lossy().into_owned())
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.v.size_hint()
    }
}
impl FusedIterator for Args {}
impl ExactSizeIterator for Args {
    #[inline]
    fn len(&self) -> usize {
        self.0.v.len()
    }
}
impl DoubleEndedIterator for Args {
    #[inline]
    fn next_back(&mut self) -> Option<String> {
        self.0.v.next_back().map(|v| v.to_string_lossy().into_owned())
    }
}

impl Iterator for ArgsOs {
    type Item = OsString;

    #[inline]
    fn next(&mut self) -> Option<OsString> {
        self.v.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.v.size_hint()
    }
}
impl FusedIterator for ArgsOs {}
impl ExactSizeIterator for ArgsOs {
    #[inline]
    fn len(&self) -> usize {
        self.v.len()
    }
}
impl DoubleEndedIterator for ArgsOs {
    #[inline]
    fn next_back(&mut self) -> Option<OsString> {
        self.v.next_back()
    }
}

impl Iterator for Vars {
    type Item = (String, String);

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.v.size_hint()
    }
    #[inline]
    fn next(&mut self) -> Option<(String, String)> {
        self.0.v.next().map(|(a, b)| {
            (
                a.to_string_lossy().into_owned(),
                b.to_string_lossy().into_owned(),
            )
        })
    }
}
impl FusedIterator for Vars {}
impl ExactSizeIterator for Vars {
    #[inline]
    fn len(&self) -> usize {
        self.0.v.len()
    }
}
impl DoubleEndedIterator for Vars {
    #[inline]
    fn next_back(&mut self) -> Option<(String, String)> {
        self.0.v.next_back().map(|(a, b)| {
            (
                a.to_string_lossy().into_owned(),
                b.to_string_lossy().into_owned(),
            )
        })
    }
}

impl Iterator for VarsOs {
    type Item = (OsString, OsString);

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.v.size_hint()
    }
    #[inline]
    fn next(&mut self) -> Option<(OsString, OsString)> {
        self.v.next()
    }
}
impl FusedIterator for VarsOs {}
impl ExactSizeIterator for VarsOs {
    #[inline]
    fn len(&self) -> usize {
        self.v.len()
    }
}
impl DoubleEndedIterator for VarsOs {
    #[inline]
    fn next_back(&mut self) -> Option<(OsString, OsString)> {
        self.v.next_back()
    }
}

impl Iterator for SplitPaths {
    type Item = PathBuf;

    #[inline]
    fn next(&mut self) -> Option<PathBuf> {
        self.0.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl FusedIterator for SplitPaths {}
impl ExactSizeIterator for SplitPaths {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}
impl DoubleEndedIterator for SplitPaths {
    #[inline]
    fn next_back(&mut self) -> Option<PathBuf> {
        self.0.next_back()
    }
}

/// Returns the arguments that this program was started with (normally passed
/// via the command line).
///
/// The first element is traditionally the path of the executable, but it can be
/// set to arbitrary text, and might not even exist. This means this property
/// should not be relied upon for security purposes.
///
/// On Unix systems the shell usually expands unquoted arguments with glob
/// patterns (such as `*` and `?`). On Windows this is not done, and such
/// arguments are passed as-is.
///
/// On glibc Linux systems, arguments are retrieved by placing a function in
/// `.init_array`. glibc passes `argc`, `argv`, and `envp` to functions in
/// `.init_array`, as a non-standard extension. This allows
/// `xrmt_stx::env::args` to work even in a `cdylib` or `staticlib`, as it does
/// on macOS and Windows.
///
/// # Panics
///
/// The returned iterator will panic during iteration if any argument to the
/// process is not valid Unicode. If this is not desired,
/// use the [`args_os`] function instead.
///
/// # Examples
///
/// ```
/// use xrmt_stx::env;
///
/// // Prints each argument on a separate line
/// for argument in env::args() {
///     println!("{argument}");
/// }
/// ```
#[inline]
pub fn args() -> Args {
    Args(args_os())
}
/// Returns an iterator of (variable, value) pairs of strings, for all the
/// environment variables of the current process.
///
/// The returned iterator contains a snapshot of the process's environment
/// variables at the time of this invocation. Modifications to environment
/// variables afterwards will not be reflected in the returned iterator.
///
/// # Panics
///
/// While iterating, the returned iterator will panic if any key or value in the
/// environment is not valid unicode. If this is not desired, consider using
/// [`env::vars_os()`].
///
/// # Examples
///
/// ```
/// // Print all environment variables.
/// for (key, value) in xrmt_stx::env::vars() {
///     println!("{key}: {value}");
/// }
/// ```
///
/// [`env::vars_os()`]: vars_os
#[inline]
pub fn vars() -> Vars {
    Vars(vars_os())
}
/// Returns the arguments that this program was started with (normally passed
/// via the command line).
///
/// The first element is traditionally the path of the executable, but it can be
/// set to arbitrary text, and might not even exist. This means this property
/// should not be relied upon for security purposes.
///
/// On Unix systems the shell usually expands unquoted arguments with glob
/// patterns (such as `*` and `?`). On Windows this is not done, and such
/// arguments are passed as-is.
///
/// On glibc Linux systems, arguments are retrieved by placing a function in
/// `.init_array`. glibc passes `argc`, `argv`, and `envp` to functions in
/// `.init_array`, as a non-standard extension. This allows
/// `xrmt_stx::env::args_os` to work even in a `cdylib` or `staticlib`, as it
/// does on macOS and Windows.
///
/// Note that the returned iterator will not check if the arguments to the
/// process are valid Unicode. If you want to panic on invalid UTF-8,
/// use the [`args`] function instead.
///
/// # Examples
///
/// ```
/// use xrmt_stx::env;
///
/// // Prints each argument on a separate line
/// for argument in env::args_os() {
///     println!("{argument:?}");
/// }
/// ```
#[inline]
pub fn args_os() -> ArgsOs {
    ArgsOs {
        v:  split_args(GetCommandLine()).into_iter(),
        _p: PhantomData,
    }
}
/// Returns an iterator of (variable, value) pairs of OS strings, for all the
/// environment variables of the current process.
///
/// The returned iterator contains a snapshot of the process's environment
/// variables at the time of this invocation. Modifications to environment
/// variables afterwards will not be reflected in the returned iterator.
///
/// Note that the returned iterator will not check if the environment variables
/// are valid Unicode. If you want to panic on invalid UTF-8,
/// use the [`vars`] function instead.
///
/// # Examples
///
/// ```
/// // Print all environment variables.
/// for (key, value) in xrmt_stx::env::vars_os() {
///     println!("{key:?}: {value:?}");
/// }
/// ```
#[inline]
pub fn vars_os() -> VarsOs {
    VarsOs {
        v:  GetEnvironment()
            .iter()
            .map(|v| {
                (
                    v.key_as_string().unwrap_or_default().into(),
                    v.value_as_string().unwrap_or_default().into(),
                )
            })
            .collect::<Vec<(OsString, OsString)>>()
            .into_iter(),
        _p: PhantomData,
    }
}
/// Returns the path of a temporary directory.
///
/// The temporary directory may be shared among users, or between processes
/// with different privileges; thus, the creation of any files or directories
/// in the temporary directory must use a secure method to create a uniquely
/// named file. Creating a file or directory with a fixed or predictable name
/// may result in "insecure temporary file" security vulnerabilities. Consider
/// using a crate that securely creates temporary files or directories.
///
/// Note that the returned value may be a symbolic link, not a directory.
///
/// # Platform-specific behavior
///
/// On Unix, returns the value of the `TMPDIR` environment variable if it is
/// set, otherwise the value is OS-specific:
/// - On Android, there is no global temporary folder (it is usually allocated
///   per-app), it will return the application's cache dir if the program runs
///   in application's namespace and system version is Android 13 (or above), or
///   `/data/local/tmp` otherwise.
/// - On Darwin-based OSes (macOS, iOS, etc) it returns the directory provided
///   by `confstr(_CS_DARWIN_USER_TEMP_DIR, ...)`, as recommended by [Apple's
///   security guidelines][appledoc].
/// - On all other unix-based OSes, it returns `/tmp`.
///
/// On Windows, the behavior is equivalent to that of
/// [`GetTempPath2`][GetTempPath2] / [`GetTempPath`][GetTempPath], which this
/// function uses internally.
///
/// Note that, this [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
/// [GetTempPath2]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-gettemppath2a
/// [GetTempPath]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-gettemppatha
/// [appledoc]: https://developer.apple.com/library/archive/documentation/Security/Conceptual/SecureCodingGuide/Articles/RaceConditions.html#//apple_ref/doc/uid/TP40002585-SW10
///
/// ```no_run
/// use xrmt_stx::env;
///
/// fn main() {
///     let dir = env::temp_dir();
///     println!("Temporary directory: {}", dir.display());
/// }
/// ```
#[inline]
pub fn temp_dir() -> PathBuf {
    GetTempPath().into()
}
/// Returns the path of the current user's home directory if known.
///
/// This may return `None` if getting the directory fails or if the platform
/// does not have user home directories.
///
/// For storing user data and configuration it is often preferable to use more
/// specific directories. For example, [XDG Base Directories] on Unix or the
/// `LOCALAPPDATA` and `APPDATA` environment variables on Windows.
///
/// [XDG Base Directories]: https://specifications.freedesktop.org/basedir-spec/latest/
///
/// # Unix
///
/// - Returns the value of the 'HOME' environment variable if it is set
///   (including to an empty string).
/// - Otherwise, it tries to determine the home directory by invoking the
///   `getpwuid_r` function using the UID of the current user. An empty home
///   directory field returned from the `getpwuid_r` function is considered to
///   be a valid value.
/// - Returns `None` if the current user has no entry in the /etc/passwd file.
///
/// # Windows
///
/// - Returns the value of the 'USERPROFILE' environment variable if it is set,
///   and is not an empty string.
/// - Otherwise, [`GetUserProfileDirectory`][msdn] is used to return the path.
///   This may change in the future.
///
/// [msdn]: https://docs.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-getuserprofiledirectorya
///
/// In UWP (Universal Windows Platform) targets this function is unimplemented
/// and always returns `None`.
///
/// Before Rust 1.85.0, this function used to return the value of the 'HOME'
/// environment variable on Windows, which in Cygwin or Mingw environments could
/// return non-standard paths like `/home/you` instead of `C:\Users\you`.
///
/// # Examples
///
/// ```
/// use xrmt_stx::env;
///
/// match env::home_dir() {
///     Some(path) => println!("Your home directory, probably: {}", path.display()),
///     None => println!("Impossible to get your home dir!"),
/// }
/// ```
#[inline]
pub fn home_dir() -> Option<PathBuf> {
    str_const!(0, "USERPROFILE", 12, n);
    if let Some(v) = GetEnvironment().find(&n).and_then(|v| v.value()) {
        return Some(v.into());
    }
    // 0x20008 - TOKEN_READ | TOKEN_QUERY
    current_token(0x20008)
        .and_then(GetUserProfileDirectory)
        .ok()
        .map(|v| v.into())
}
/// Returns the current working directory as a [`PathBuf`].
///
/// # Platform-specific behavior
///
/// This function [currently] corresponds to the `getcwd` function on Unix
/// and the `GetCurrentDirectoryW` function on Windows.
///
/// [currently]: crate::io#platform-specific-behavior
///
/// # Errors
///
/// Returns an [`Err`] if the current working directory value is invalid.
/// Possible cases:
///
/// * Current directory does not exist.
/// * There are insufficient permissions to access the current directory.
///
/// # Examples
///
/// ```
/// use xrmt_stx::env;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let path = env::current_dir()?;
///     println!("The current directory is {}", path.display());
///     Ok(())
/// }
/// ```
#[inline]
pub fn current_dir() -> IoResult<PathBuf> {
    Ok(GetCurrentDirectory().into())
}
/// Returns the full filesystem path of the current running executable.
///
/// # Platform-specific behavior
///
/// If the executable was invoked through a symbolic link, some platforms will
/// return the path of the symbolic link and other platforms will return the
/// path of the symbolic link’s target.
///
/// If the executable is renamed while it is running, platforms may return the
/// path at the time it was loaded instead of the new path.
///
/// # Errors
///
/// Acquiring the path of the current executable is a platform-specific
/// operation that can fail for a good number of reasons. Some errors can
/// include, but not be limited to, filesystem operations failing or general
/// syscall failures.
///
/// # Security
///
/// The output of this function should not be trusted for anything
/// that might have security implications. Basically, if users can run
/// the executable, they can change the output arbitrarily.
///
/// As an example, you can easily introduce a race condition. It goes
/// like this:
///
/// 1. You get the path to the current executable using `current_exe()`, and
///    store it in a variable.
/// 2. Time passes. A malicious actor removes the current executable, and
///    replaces it with a malicious one.
/// 3. You then use the stored path to re-execute the current executable.
///
/// You expected to safely execute the current executable, but you're
/// instead executing something completely different. The code you
/// just executed run with your privileges.
///
/// This sort of behavior has been known to [lead to privilege escalation] when
/// used incorrectly.
///
/// [lead to privilege escalation]: https://securityvulns.com/Wdocument183.html
///
/// # Examples
///
/// ```
/// use xrmt_stx::env;
///
/// match env::current_exe() {
///     Ok(exe_path) => println!("Path of this executable is: {}",
///                              exe_path.display()),
///     Err(e) => println!("failed to get current exe path: {e}"),
/// };
/// ```
#[inline]
pub fn current_exe() -> IoResult<PathBuf> {
    Ok(GetModuleFileName(Handle::EMPTY).map(|v| v.into())?)
}
/// Fetches the environment variable `key` from the current process, returning
/// [`None`] if the variable isn't set or if there is another error.
///
/// It may return `None` if the environment variable's name contains
/// the equal sign character (`=`) or the NUL character.
///
/// Note that this function will not check if the environment variable
/// is valid Unicode. If you want to have an error on invalid UTF-8,
/// use the [`var`] function instead.
///
/// # Examples
///
/// ```
/// use xrmt_stx::env;
///
/// let key = "HOME";
/// match env::var_os(key) {
///     Some(val) => println!("{key}: {val:?}"),
///     None => println!("{key} is not defined in the environment.")
/// }
/// ```
///
/// If expecting a delimited variable (such as `PATH`), [`split_paths`]
/// can be used to separate items.
#[inline]
pub fn var_os(key: impl AsRef<OsStr>) -> Option<OsString> {
    GetEnvironmentVariable(key.as_ref()).map(|v| v.into())
}
/// Fetches the environment variable `key` from the current process.
///
/// # Errors
///
/// Returns [`VarError::NotPresent`] if:
/// - The variable is not set.
/// - The variable's name contains an equal sign or NUL (`'='` or `'\0'`).
///
/// Returns [`VarError::NotUnicode`] if the variable's value is not valid
/// Unicode. If this is not desired, consider using [`var_os`].
///
/// # Examples
///
/// ```
/// use xrmt_stx::env;
///
/// let key = "HOME";
/// match env::var(key) {
///     Ok(val) => println!("{key}: {val:?}"),
///     Err(e) => println!("couldn't interpret {key}: {e}"),
/// }
/// ```
#[inline]
pub fn var(key: impl AsRef<OsStr>) -> Result<String, VarError> {
    GetEnvironmentVariable(key.as_ref())
        .ok_or(VarError::NotPresent)
        .map(|v| v.into())
}
/// Changes the current working directory to the specified path.
///
/// # Platform-specific behavior
///
/// This function [currently] corresponds to the `chdir` function on Unix
/// and the `SetCurrentDirectoryW` function on Windows.
///
/// Returns an [`Err`] if the operation fails.
///
/// [currently]: crate::io#platform-specific-behavior
///
/// # Examples
///
/// ```
/// use xrmt_stx::env;
/// use xrmt_stx::path::Path;
///
/// let root = Path::new("/");
/// assert!(env::set_current_dir(&root).is_ok());
/// println!("Successfully changed working directory to {}!", root.display());
/// ```
#[inline]
pub fn set_current_dir(path: impl AsRef<OsStr>) -> IoResult<()> {
    Ok(SetCurrentDirectory(path.as_ref())?)
}
/// Parses input according to platform conventions for the `PATH`
/// environment variable.
///
/// Returns an iterator over the paths contained in `unparsed`. The iterator
/// element type is [`PathBuf`].
///
/// On most Unix platforms, the separator is `:` and on Windows it is `;`. This
/// also performs unquoting on Windows.
///
/// [`join_paths`] can be used to recombine elements.
///
/// # Panics
///
/// This will panic on systems where there is no delimited `PATH` variable,
/// such as UEFI.
///
/// # Examples
///
/// ```
/// use xrmt_stx::env;
///
/// let key = "PATH";
/// match env::var_os(key) {
///     Some(paths) => {
///         for path in env::split_paths(&paths) {
///             println!("'{}'", path.display());
///         }
///     }
///     None => println!("{key} is not defined in the environment.")
/// }
/// ```
pub fn split_paths<T: ?Sized + AsRef<OsStr>>(paths: &T) -> SplitPaths {
    let v = &*paths.as_ref().to_string_lossy();
    let (mut o, mut l, mut d) = (Vec::new(), 0usize, false);
    for (i, x) in v.as_bytes().iter().enumerate() {
        match *x {
            b'"' if !d => d = true,
            b'"' => d = false,
            b';' if d => (),
            b';' => {
                if i - l > 0 {
                    o.push(PathBuf::from(&v[l..i]))
                }
                l = i + 1;
            },
            _ => (),
        }
    }
    if l == 0 {
        o.push(v.into());
    } else if l < v.len() {
        o.push(PathBuf::from(&v[l..]));
    }
    SplitPaths(o.into_iter())
}
/// Joins a collection of [`Path`]s appropriately for the `PATH`
/// environment variable.
///
/// # Errors
///
/// Returns an [`Err`] (containing an error message) if one of the input
/// [`Path`]s contains an invalid character for constructing the `PATH`
/// variable (a double quote on Windows or a colon on Unix), or if the system
/// does not have a `PATH`-like variable (e.g. UEFI or WASI).
///
/// [`Path`]: crate::path::Path
///
/// # Examples
///
/// Joining paths on a Unix-like platform:
///
/// ```
/// use xrmt_stx::env;
/// use xrmt_stx::ffi::OsString;
/// use xrmt_stx::path::Path;
///
/// fn main() -> Result<(), env::JoinPathsError> {
/// # if cfg!(unix) {
///     let paths = [Path::new("/bin"), Path::new("/usr/bin")];
///     let path_os_string = env::join_paths(paths.iter())?;
///     assert_eq!(path_os_string, OsString::from("/bin:/usr/bin"));
/// # }
///     Ok(())
/// }
/// ```
///
/// Joining a path containing a colon on a Unix-like platform results in an
/// error:
///
/// ```
/// # if cfg!(unix) {
/// use xrmt_stx::env;
/// use xrmt_stx::path::Path;
///
/// let paths = [Path::new("/bin"), Path::new("/usr/bi:n")];
/// assert!(env::join_paths(paths.iter()).is_err());
/// # }
/// ```
///
/// Using `env::join_paths()` with [`env::split_paths()`] to append an item to
/// the `PATH` environment variable:
///
/// ```
/// use xrmt_stx::env;
/// use xrmt_stx::path::PathBuf;
///
/// fn main() -> Result<(), env::JoinPathsError> {
///     if let Some(path) = env::var_os("PATH") {
///         let mut paths = env::split_paths(&path).collect::<Vec<_>>();
///         paths.push(PathBuf::from("/home/xyz/bin"));
///         let new_path = env::join_paths(paths)?;
///         unsafe { env::set_var("PATH", &new_path); }
///     }
///
///     Ok(())
/// }
/// ```
///
/// [`env::split_paths()`]: split_paths
pub fn join_paths<T: AsRef<OsStr>>(paths: impl IntoIterator<Item = T>) -> Result<OsString, JoinPathsError> {
    let mut b = String::new();
    for (i, v) in paths.into_iter().enumerate() {
        let d = &*v.as_ref().to_string_lossy();
        if d.contains('"') {
            return Err(JoinPathsError(()));
        }
        if i > 0 {
            b.push(';');
        }
        b.reserve(d.len() + 1);
        if !d.contains(';') {
            b.push_str(d);
            continue;
        }
        b.push('"');
        b.push_str(d);
        b.push('"');
    }
    Ok(b.into())
}

/// Removes an environment variable from the environment of the currently
/// running process.
///
/// # Safety
///
/// This function is safe to call in a single-threaded program.
///
/// This function is also always safe to call on Windows, in single-threaded
/// and multi-threaded programs.
///
/// In multi-threaded programs on other operating systems, the only safe option
/// is to not use `set_var` or `remove_var` at all.
///
/// The exact requirement is: you
/// must ensure that there are no other threads concurrently writing or
/// *reading*(!) the environment through functions or global variables other
/// than the ones in this module. The problem is that these operating systems
/// do not provide a thread-safe way to read the environment, and most C
/// libraries, including libc itself, do not advertise which functions read
/// from the environment. Even functions from the Rust standard library may
/// read the environment without going through this module, e.g. for DNS
/// lookups from [`xrmt_stx::net::ToSocketAddrs`]. No stable guarantee is made
/// about which functions may read from the environment in future versions of a
/// library. All this makes it not practically possible for you to guarantee
/// that no other thread will read the environment, so the only safe option is
/// to not use `set_var` or `remove_var` in multi-threaded programs at all.
///
/// Discussion of this unsafety on Unix may be found in:
///
///  - [Austin Group Bugzilla](https://austingroupbugs.net/view.php?id=188)
///  - [GNU C library Bugzilla](https://sourceware.org/bugzilla/show_bug.cgi?id=15607#c2)
///
/// To prevent a child process from inheriting an environment variable, you can
/// instead use [`Command::env_remove`] or [`Command::env_clear`].
///
/// [`xrmt_stx::net::ToSocketAddrs`]: crate::net::ToSocketAddrs
/// [`Command::env_remove`]: crate::process::Command::env_remove
/// [`Command::env_clear`]: crate::process::Command::env_clear
///
/// # Panics
///
/// This function may panic if `key` is empty, contains an ASCII equals sign
/// `'='` or the NUL character `'\0'`, or when the value contains the NUL
/// character.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::env;
///
/// let key = "KEY";
/// unsafe {
///     env::set_var(key, "VALUE");
/// }
/// assert_eq!(env::var(key), Ok("VALUE".to_string()));
///
/// unsafe {
///     env::remove_var(key);
/// }
/// assert!(env::var(key).is_err());
/// ```
#[inline]
pub unsafe fn remove_var(key: impl AsRef<OsStr>) {
    let _ = SetEnvironmentVariable(key.as_ref(), None);
}
/// Sets the environment variable `key` to the value `value` for the currently
/// running process.
///
/// # Safety
///
/// This function is safe to call in a single-threaded program.
///
/// This function is also always safe to call on Windows, in single-threaded
/// and multi-threaded programs.
///
/// In multi-threaded programs on other operating systems, the only safe option
/// is to not use `set_var` or `remove_var` at all.
///
/// The exact requirement is: you
/// must ensure that there are no other threads concurrently writing or
/// *reading*(!) the environment through functions or global variables other
/// than the ones in this module. The problem is that these operating systems
/// do not provide a thread-safe way to read the environment, and most C
/// libraries, including libc itself, do not advertise which functions read
/// from the environment. Even functions from the Rust standard library may
/// read the environment without going through this module, e.g. for DNS
/// lookups from [`xrmt_stx::net::ToSocketAddrs`]. No stable guarantee is made
/// about which functions may read from the environment in future versions of a
/// library. All this makes it not practically possible for you to guarantee
/// that no other thread will read the environment, so the only safe option is
/// to not use `set_var` or `remove_var` in multi-threaded programs at all.
///
/// Discussion of this unsafety on Unix may be found in:
///
///  - [Austin Group Bugzilla](https://austingroupbugs.net/view.php?id=188)
///  - [GNU C library Bugzilla](https://sourceware.org/bugzilla/show_bug.cgi?id=15607#c2)
///
/// To pass an environment variable to a child process, you can instead use
/// [`Command::env`].
///
/// [`xrmt_stx::net::ToSocketAddrs`]: crate::net::ToSocketAddrs
/// [`Command::env`]: crate::process::Command::env
///
/// # Panics
///
/// This function may panic if `key` is empty, contains an ASCII equals sign
/// `'='` or the NUL character `'\0'`, or when `value` contains the NUL
/// character.
///
/// # Examples
///
/// ```
/// use xrmt_stx::env;
///
/// let key = "KEY";
/// unsafe {
///     env::set_var(key, "VALUE");
/// }
/// assert_eq!(env::var(key), Ok("VALUE".to_string()));
/// ```
#[inline]
pub unsafe fn set_var(key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) {
    let _ = SetEnvironmentVariable(key.as_ref(), value.as_ref());
}

fn split_args<'a>(v: WCharSlice<'a>) -> Vec<OsString> {
    let b = v.to_u8();
    // TODO(dij): Update this handeling
    let (mut c, mut q, mut l) = (0usize, false, 0usize);
    let (mut r, mut s) = (Vec::new(), Vec::new());
    for (i, x) in v.iter().enumerate() {
        match *x {
            0x9 | 0x20 if i == l => l += 1, // Skip leading whitespace.
            0x5C => c += 1,                 // Count slashes.
            0x22 if c == 0 => q = true,
            0x22 if c % 2 == 0 => {
                for _ in 0..(c / 2) {
                    s.push(0x5C);
                }
                c = 0;
                if b.len() > i + 1 && v[i + 1] == 0x22 {
                    s.push(0x22);
                } else {
                    q = !q;
                }
            },
            0x22 if c % 2 == 1 => {
                for _ in 0..c {
                    s.push(0x5C)
                }
                s.push(0x22);
                c = 0;
            },
            0x9 | 0x20 if !q && !s.is_empty() => {
                let mut t = OsString::with_capacity(s.len());
                utf16_to_vec(t.as_mut_vec(), &s);
                r.push(t);
                s.clear();
                l = i;
            },
            0x9 | 0x20 if !q => {
                s.clear();
                l = i;
            },
            _ => s.push(*x),
        }
    }
    if s.len() > 0 {
        let mut t = OsString::with_capacity(s.len());
        utf16_to_vec(t.as_mut_vec(), &s);
        r.push(t);
    }
    r
}
