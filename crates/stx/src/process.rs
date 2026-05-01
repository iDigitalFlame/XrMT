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

//! A module for working with processes.
//!
//! This module is mostly concerned with spawning and interacting with child
//! processes, but it also provides [`abort`] and [`exit`] for terminating the
//! current process.
//!
//! # Spawning a process
//!
//! The [`Command`] struct is used to configure and spawn processes:
//!
//! ```no_run
//! use xrmt_stx::process::Command;
//!
//! let output = Command::new("echo")
//!     .arg("Hello world")
//!     .output()
//!     .expect("Failed to execute command");
//!
//! assert_eq!(b"Hello world\n", output.stdout.as_slice());
//! ```
//!
//! Several methods on [`Command`], such as [`spawn`] or [`output`], can be used
//! to spawn a process. In particular, [`output`] spawns the child process and
//! waits until the process terminates, while [`spawn`] will return a [`Child`]
//! that represents the spawned child process.
//!
//! # Handling I/O
//!
//! The [`stdout`], [`stdin`], and [`stderr`] of a child process can be
//! configured by passing an [`Stdio`] to the corresponding method on
//! [`Command`]. Once spawned, they can be accessed from the [`Child`]. For
//! example, piping output from one command into another command can be done
//! like so:
//!
//! ```no_run
//! use xrmt_stx::process::{Command, Stdio};
//!
//! // stdout must be configured with `Stdio::piped` in order to use
//! // `echo_child.stdout`
//! let echo_child = Command::new("echo")
//!     .arg("Oh no, a tpyo!")
//!     .stdout(Stdio::piped())
//!     .spawn()
//!     .expect("Failed to start echo process");
//!
//! // Note that `echo_child` is moved here, but we won't be needing
//! // `echo_child` anymore
//! let echo_out = echo_child.stdout.expect("Failed to open echo stdout");
//!
//! let mut sed_child = Command::new("sed")
//!     .arg("s/tpyo/typo/")
//!     .stdin(Stdio::from(echo_out))
//!     .stdout(Stdio::piped())
//!     .spawn()
//!     .expect("Failed to start sed process");
//!
//! let output = sed_child.wait_with_output().expect("Failed to wait on sed");
//! assert_eq!(b"Oh no, a typo!\n", output.stdout.as_slice());
//! ```
//!
//! Note that [`ChildStderr`] and [`ChildStdout`] implement [`Read`] and
//! [`ChildStdin`] implements [`Write`]:
//!
//! ```no_run
//! use xrmt_stx::process::{Command, Stdio};
//! use xrmt_stx::io::Write;
//!
//! let mut child = Command::new("/bin/cat")
//!     .stdin(Stdio::piped())
//!     .stdout(Stdio::piped())
//!     .spawn()
//!     .expect("failed to execute child");
//!
//! // If the child process fills its stdout buffer, it may end up
//! // waiting until the parent reads the stdout, and not be able to
//! // read stdin in the meantime, causing a deadlock.
//! // Writing from another thread ensures that stdout is being read
//! // at the same time, avoiding the problem.
//! let mut stdin = child.stdin.take().expect("failed to get stdin");
//! xrmt_stx::thread::spawn(move || {
//!     stdin.write_all(b"test").expect("failed to write to stdin");
//! });
//!
//! let output = child
//!     .wait_with_output()
//!     .expect("failed to wait on child");
//!
//! assert_eq!(b"test", output.stdout.as_slice());
//! ```
//!
//! # Windows argument splitting
//!
//! On Unix systems arguments are passed to a new process as an array of
//! strings, but on Windows arguments are passed as a single commandline string
//! and it is up to the child process to parse it into an array. Therefore the
//! parent and child processes must agree on how the commandline string is
//! encoded.
//!
//! Most programs use the standard C run-time `argv`, which in practice results
//! in consistent argument handling. However, some programs have their own way
//! of parsing the commandline string. In these cases using [`arg`] or [`args`]
//! may result in the child process seeing a different array of arguments than
//! the parent process intended.
//!
//! Two ways of mitigating this are:
//!
//! * Validate untrusted input so that only a safe subset is allowed.
//! * Use [`raw_arg`] to build a custom commandline. This bypasses the escaping
//!   rules used by [`arg`] so should be used with due caution.
//!
//! `cmd.exe` and `.bat` files use non-standard argument parsing and are
//! especially vulnerable to malicious input as they may be used to run
//! arbitrary shell commands. Untrusted arguments should be restricted as much
//! as possible. For examples on handling this see [`raw_arg`].
//!
//! ### Batch file special handling
//!
//! On Windows, `Command` uses the Windows API function [`CreateProcessW`] to
//! spawn new processes. An undocumented feature of this function is that
//! when given a `.bat` file as the application to run, it will automatically
//! convert that into running `cmd.exe /c` with the batch file as the next
//! argument.
//!
//! For historical reasons Rust currently preserves this behavior when using
//! [`Command::new`], and escapes the arguments according to `cmd.exe` rules.
//! Due to the complexity of `cmd.exe` argument handling, it might not be
//! possible to safely escape some special characters, and using them will
//! result in an error being returned at process spawn. The set of unescapeable
//! special characters might change between releases.
//!
//! Also note that running batch scripts in this way may be removed in the
//! future and so should not be relied upon.
//!
//! [`spawn`]: Command::spawn
//! [`output`]: Command::output
//!
//! [`stdout`]: Command::stdout
//! [`stdin`]: Command::stdin
//! [`stderr`]: Command::stderr
//!
//! [`arg`]: Command::arg
//! [`args`]: Command::args
//! [`raw_arg`]: crate::os::windows::process::CommandExt::raw_arg
//!
//! [`CreateProcessW`]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw

#![no_implicit_prelude]
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate alloc;
extern crate core;

extern crate xrmt_crypt;
extern crate xrmt_data;
extern crate xrmt_winapi;

use alloc::boxed::Box;
use alloc::collections::{btree_map, BTreeMap};
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::clone::Clone;
use core::cmp::{Eq, PartialEq};
use core::convert::{AsRef, From, Infallible, Into};
use core::default::Default;
use core::error::Error;
use core::fmt::{Debug, Display, Formatter};
use core::hint::spin_loop;
use core::iter::{ExactSizeIterator, Extend, FusedIterator, IntoIterator, Iterator};
use core::marker::{Copy, PhantomData};
use core::mem::{drop, size_of, swap};
use core::num::{NonZeroI32, NonZeroU64};
use core::ops::{Deref, DerefMut, Drop};
use core::option::Option::{self, None, Some};
use core::ptr::{copy, copy_nonoverlapping};
use core::result::Result::{self, Err, Ok};
use core::slice::Iter;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering};
use core::{debug_assert, unreachable};

use xrmt_data::text::{ToStrSigned, U16Encoder};
use xrmt_winapi::functions::{
    close_handle,
    exit_process,
    file_is_file,
    system_dir,
    system_root,
    take_current_thread_token,
    wait_for_multiple_objects,
    CancelIoEx,
    CreatePipe,
    CreateProcess,
    CreateProcessWithLogon,
    CreateProcessWithToken,
    DuplicateHandleEx,
    GetCurrentProcessID,
    GetCurrentProcessPEB,
    GetEnvironment,
    GetOverlappedResult,
    NtCreateFile,
    NtFlushBuffersFile,
    NtQueryInformationProcess,
    NtReadFile,
    NtWriteFile,
    SetHandleInformation,
    SetThreadToken,
    TerminateProcess,
    WaitForSingleObject,
};
use xrmt_winapi::info::{is_min_windows_8, is_windows_xp};
use xrmt_winapi::structs::{
    Handle,
    OwnedHandle,
    OwnedOverlapped,
    ProcessBasicInfo,
    ProcessEnvironment,
    ProcessInfo,
    ProcessThreadAttrList,
    SecurityAttributes,
    StartInfo,
    StartupInfo,
    StartupInfoEx,
    StringLike,
    StringLikeU16,
    SystemVersion,
    WChar,
    WCharLike,
};
use xrmt_winapi::{path_normalize, str_const, Win32Error, CURRENT_PROCESS, CURRENT_THREAD, INFINITE};

use crate::ffi::{OsStr, OsString};
use crate::fs::File;
use crate::io::{ErrorKind, FmtResult, IoError, IoResult, PipeReader, PipeWriter, Read, Stderr, Stdout, Write};
use crate::path::{Path, PathBuf};

const PATHEXT: [u16; 7] = [
    b'P' as u16,
    b'A' as u16,
    b'T' as u16,
    b'H' as u16,
    b'E' as u16,
    b'X' as u16,
    b'T' as u16,
];

static VERSION: AtomicU8 = AtomicU8::new(0u8);

#[doc(hidden)]
#[path = "extra/process.rs"]
pub mod extra;

/// Representation of a running or exited child process.
///
/// This structure is used to represent and manage child processes. A child
/// process is created via the [`Command`] struct, which configures the
/// spawning process and can itself be constructed using a builder-style
/// interface.
///
/// There is no implementation of [`Drop`] for child processes,
/// so if you do not ensure the `Child` has exited then it will continue to
/// run, even after the `Child` handle to the child process has gone out of
/// scope.
///
/// Calling [`wait`] (or other functions that wrap around it) will make
/// the parent process wait until the child has actually exited before
/// continuing.
///
/// # Warning
///
/// On some systems, calling [`wait`] or similar is necessary for the OS to
/// release resources. A process that terminated but has not been waited on is
/// still around as a "zombie". Leaving too many zombies around may exhaust
/// global resources (for example process IDs).
///
/// The standard library does *not* automatically wait on child processes (not
/// even if the `Child` is dropped), it is up to the application developer to do
/// so. As a consequence, dropping `Child` handles without waiting on them first
/// is not recommended in long-running applications.
///
/// # Examples
///
/// ```should_panic
/// use xrmt_stx::process::Command;
///
/// let mut child = Command::new("/bin/cat")
///     .arg("file.txt")
///     .spawn()
///     .expect("failed to execute child");
///
/// let ecode = child.wait().expect("failed to wait on child");
///
/// assert!(ecode.success());
/// ```
///
/// [`wait`]: Child::wait
pub struct Child {
    /// The handle for writing to the child's standard input (stdin), if it
    /// has been captured. You might find it helpful to do
    ///
    /// ```ignore (incomplete)
    /// let stdin = child.stdin.take().expect("handle present");
    /// ```
    ///
    /// to avoid partially moving the `child` and thus blocking yourself from
    /// calling functions on `child` while using `stdin`.
    pub stdin:  Option<ChildStdin>,
    /// The handle for reading from the child's standard output (stdout), if it
    /// has been captured. You might find it helpful to do
    ///
    /// ```ignore (incomplete)
    /// let stdout = child.stdout.take().expect("handle present");
    /// ```
    ///
    /// to avoid partially moving the `child` and thus blocking yourself from
    /// calling functions on `child` while using `stdout`.
    pub stdout: Option<ChildStdout>,
    /// The handle for reading from the child's standard error (stderr), if it
    /// has been captured. You might find it helpful to do
    ///
    /// ```ignore (incomplete)
    /// let stderr = child.stderr.take().expect("handle present");
    /// ```
    ///
    /// to avoid partially moving the `child` and thus blocking yourself from
    /// calling functions on `child` while using `stderr`.
    pub stderr: Option<ChildStderr>,
    info:       ProcessInfo,
    exit:       AtomicI32,
    done:       AtomicBool,
}
/// Describes what to do with a standard I/O stream for a child process when
/// passed to the [`stdin`], [`stdout`], and [`stderr`] methods of [`Command`].
///
/// [`stdin`]: Command::stdin
/// [`stdout`]: Command::stdout
/// [`stderr`]: Command::stderr
pub struct Stdio {
    v: StdioType,
    h: OwnedHandle,
}
/// The output of a finished process.
///
/// This is returned in a Result by either the [`output`] method of a
/// [`Command`], or the [`wait_with_output`] method of a [`Child`]
/// process.
///
/// [`output`]: Command::output
/// [`wait_with_output`]: Child::wait_with_output
pub struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
/// A process builder, providing fine-grained control
/// over how a new process should be spawned.
///
/// A default configuration can be
/// generated using `Command::new(program)`, where `program` gives a path to the
/// program to be executed. Additional builder methods allow the configuration
/// to be changed (for example, by adding arguments) prior to spawning:
///
/// ```
/// use xrmt_stx::process::Command;
///
/// let output = if cfg!(target_os = "windows") {
///     Command::new("cmd")
///         .args(["/C", "echo hello"])
///         .output()
///         .expect("failed to execute process")
/// } else {
///     Command::new("sh")
///         .arg("-c")
///         .arg("echo hello")
///         .output()
///         .expect("failed to execute process")
/// };
///
/// let hello = output.stdout;
/// ```
///
/// `Command` can be reused to spawn multiple processes. The builder methods
/// change the command without needing to immediately spawn the process.
///
/// ```no_run
/// use xrmt_stx::process::Command;
///
/// let mut echo_hello = Command::new("sh");
/// echo_hello.arg("-c").arg("echo hello");
/// let hello_1 = echo_hello.output().expect("failed to execute process");
/// let hello_2 = echo_hello.output().expect("failed to execute process");
/// ```
///
/// Similarly, you can call builder methods after spawning a process and then
/// spawn a new process with the modified settings.
///
/// ```no_run
/// use xrmt_stx::process::Command;
///
/// let mut list_dir = Command::new("ls");
///
/// // Execute `ls` in the current directory of the program.
/// list_dir.status().expect("process failed to execute");
///
/// println!();
///
/// // Change `ls` to execute in the root directory.
/// list_dir.current_dir("/");
///
/// // And then execute `ls` again but in the root directory.
/// list_dir.status().expect("process failed to execute");
/// ```
pub struct Command {
    dir:          Option<PathBuf>,
    env:          BTreeMap<OsString, OsString>,
    exe:          OsString,
    args:         Vec<Arg>,
    mode:         u8,
    flags:        u32,
    clear:        bool,
    stdin:        Stdio,
    stdout:       Stdio,
    stderr:       Stdio,
    force_quotes: bool,
    _p:           PhantomData<UnsafeCell<()>>,
}
/// A handle to a child process's standard output (stdout).
///
/// This struct is used in the [`stdout`] field on [`Child`].
///
/// When an instance of `ChildStdout` is [dropped], the `ChildStdout`'s
/// underlying file handle will be closed.
///
/// [`stdout`]: Child::stdout
/// [dropped]: Drop
pub struct ChildStdout {
    h:   OwnedHandle,
    olp: OwnedOverlapped,
    // parent: Handle,
}
/// This type represents the status code the current process can return
/// to its parent under normal termination.
///
/// `ExitCode` is intended to be consumed only by the standard library (via
/// [`Termination::report()`]). For forwards compatibility with potentially
/// unusual targets, this type currently does not provide `Eq`, `Hash`, or
/// access to the raw value. This type does provide `PartialEq` for
/// comparison, but note that there may potentially be multiple failure
/// codes, some of which will _not_ compare equal to `ExitCode::FAILURE`.
/// The standard library provides the canonical `SUCCESS` and `FAILURE`
/// exit codes as well as `From<u8> for ExitCode` for constructing other
/// arbitrary exit codes.
///
/// # Portability
///
/// Numeric values used in this type don't have portable meanings, and
/// different platforms may mask different amounts of them.
///
/// For the platform's canonical successful and unsuccessful codes, see
/// the [`SUCCESS`] and [`FAILURE`] associated items.
///
/// [`SUCCESS`]: ExitCode::SUCCESS
/// [`FAILURE`]: ExitCode::FAILURE
///
/// # Differences from `ExitStatus`
///
/// `ExitCode` is intended for terminating the currently running process, via
/// the `Termination` trait, in contrast to [`ExitStatus`], which represents the
/// termination of a child process. These APIs are separate due to platform
/// compatibility differences and their expected usage; it is not generally
/// possible to exactly reproduce an `ExitStatus` from a child for the current
/// process after the fact.
///
/// # Examples
///
/// `ExitCode` can be returned from the `main` function of a crate, as it
/// implements [`Termination`]:
///
/// ```
/// use xrmt_stx::process::ExitCode;
/// # fn check_foo() -> bool { true }
///
/// fn main() -> ExitCode {
///     if !check_foo() {
///         return ExitCode::from(42);
///     }
///
///     ExitCode::SUCCESS
/// }
/// ```
pub struct ExitCode(i32);
/// Describes the result of a process after it has terminated.
///
/// This `struct` is used to represent the exit status or other termination of a
/// child process. Child processes are created via the [`Command`] struct and
/// their exit status is exposed through the [`status`] method, or the [`wait`]
/// method of a [`Child`] process.
///
/// An `ExitStatus` represents every possible disposition of a process.  On Unix
/// this is the **wait status**.  It is *not* simply an *exit status* (a value
/// passed to `exit`).
///
/// For proper error reporting of failed processes, print the value of
/// `ExitStatus` or `ExitStatusError` using their implementations of
/// [`Display`](core::fmt::Display).
///
/// # Differences from `ExitCode`
///
/// [`ExitCode`] is intended for terminating the currently running process, via
/// the `Termination` trait, in contrast to `ExitStatus`, which represents the
/// termination of a child process. These APIs are separate due to platform
/// compatibility differences and their expected usage; it is not generally
/// possible to exactly reproduce an `ExitStatus` from a child for the current
/// process after the fact.
///
/// [`status`]: Command::status
/// [`wait`]: Child::wait
pub struct ExitStatus(i32);
pub struct CommandEnvs<'a> {
    iter: btree_map::Iter<'a, OsString, OsString>,
}
/// An iterator over the command arguments.
///
/// This struct is created by [`Command::get_args`]. See its documentation for
/// more.
pub struct CommandArgs<'a> {
    iter: Iter<'a, Arg>,
    _p:   PhantomData<*mut ()>,
}
pub struct StartParameters<'a> {
    x:           i32,
    y:           i32,
    user:        WCharLike<'a>,
    flags:       u32,
    title:       WCharLike<'a>,
    token:       Option<&'a OwnedHandle>,
    width:       u32,
    height:      u32,
    domain:      WCharLike<'a>,
    desktop:     WCharLike<'a>,
    password:    WCharLike<'a>,
    mitigations: Option<NonZeroU64>,
}
/// A handle to a child process's standard input (stdin).
///
/// This struct is used in the [`stdin`] field on [`Child`].
///
/// When an instance of `ChildStdin` is [dropped], the `ChildStdin`'s underlying
/// file handle will be closed. If the child process was blocked on input prior
/// to being dropped, it will become unblocked after dropping.
///
/// [`stdin`]: Child::stdin
/// [dropped]: Drop
pub struct ChildStdin(OwnedHandle);
/// A handle to a child process's stderr.
///
/// This struct is used in the [`stderr`] field on [`Child`].
///
/// When an instance of `ChildStderr` is [dropped], the `ChildStderr`'s
/// underlying file handle will be closed.
///
/// [`stderr`]: Child::stderr
/// [dropped]: Drop
pub struct ChildStderr(ChildStdout);
/// Describes the result of a process after it has failed
///
/// Produced by the [`.exit_ok`](ExitStatus::exit_ok) method on [`ExitStatus`].
///
/// # Examples
///
/// ```
/// use xrmt_stx::process::{Command, ExitStatusError};
///
/// fn run(cmd: &str) -> Result<(),ExitStatusError> {
///     Command::new(cmd).status().unwrap().exit_ok()?;
///     Ok(())
/// }
///
/// run("true").unwrap();
/// run("false").unwrap_err();
/// ```
pub struct ExitStatusError(ExitStatus);

/// A trait for implementing arbitrary return types in the `main` function.
///
/// The C-main function only supports returning integers.
/// So, every type implementing the `Termination` trait has to be converted
/// to an integer.
///
/// The default implementations are returning `libc::EXIT_SUCCESS` to indicate
/// a successful execution. In case of a failure, `libc::EXIT_FAILURE` is
/// returned.
///
/// Because different runtimes have different specifications on the return value
/// of the `main` function, this trait is likely to be available only on
/// standard library's runtime for convenience. Other runtimes are not required
/// to provide similar functionality.
pub trait Termination {
    fn report(self) -> ExitCode;
}

enum Arg {
    Raw(OsString),
    Auto(OsString),
}
enum PipeType {
    Stdin,
    Stdout,
    Stderr,
}
enum StdioType {
    Null,
    Pipe,
    Handle,
    Inherit,
}
enum PipeHandle<'a> {
    // Handle to be inherited by the child process. We'll close it when we're
    // done with it.
    Local(OwnedHandle),
    // Handle to be inherited by a remote process. It's a Handle for another
    // process and we'll close it when we're done by duplicating it back into
    // our process then closing it.
    Remote(&'a OwnedHandle, Handle),
    // Our handle to the read/write end of the Pipe, the other is the Handle to
    // be inherited by the child process. We'll close it when we're done with it.
    LocalPipe(OwnedHandle, OwnedHandle),
    // Our handle to the read/write end of the Pipe, the other is the Handle to
    // be inherited by a remote process. We'll close it when we're done by
    // duplicating it back into our process then closing it.
    RemotePipe(&'a OwnedHandle, OwnedHandle, Handle),
}

struct Async(ChildStdout);

impl Arg {
    #[inline]
    fn new(v: impl AsRef<OsStr>) -> Arg {
        Arg::Auto(v.as_ref().to_os_string())
    }
    #[inline]
    fn raw(v: impl AsRef<OsStr>) -> Arg {
        Arg::Raw(v.as_ref().to_os_string())
    }

    #[inline]
    fn is_raw(&self) -> bool {
        match self {
            Arg::Raw(_) => true,
            _ => false,
        }
    }
    #[inline]
    fn as_os_str(&self) -> &OsStr {
        match self {
            Arg::Raw(v) => v.as_ref(),
            Arg::Auto(v) => v.as_ref(),
        }
    }
    #[inline]
    fn is_quoted(&self, force: bool) -> bool {
        match self {
            Arg::Auto(_) if force => true,
            Arg::Auto(v) => v.is_empty() || v.as_encoded_bytes().iter().any(|v| *v == 0x20 || *v == 0x9),
            _ => false,
        }
    }
}
impl Async {
    #[inline]
    fn result(&mut self, wait: bool) -> IoResult<Option<usize>> {
        match GetOverlappedResult(&self.0.h, &mut self.0.olp, wait) {
            Err(Win32Error::IoPending) => Ok(None),
            Err(Win32Error::BrokenPipe) => Ok(Some(0)),
            Err(e) => Err(IoError::from(e)),
            Ok(n) => Ok(Some(n)),
        }
    }
    #[inline]
    fn start(&mut self, pos: usize, buf: &mut Vec<u8>) -> IoResult<usize> {
        let mut i = pos;
        loop {
            i += match self.read(i, buf)? {
                Some(0) | None => break,
                Some(v) => v,
            };
        }
        Ok(i)
    }
    #[inline]
    fn empty(&mut self, pos: &mut usize, buf: &mut Vec<u8>) -> IoResult<bool> {
        let i = match self.result(true)? {
            Some(0) => return Ok(true),
            Some(v) => v,
            None => 0,
        };
        let r = self.start(*pos + i, buf)?;
        // No read was done?
        if r == *pos {
            return Ok(true);
        }
        *pos = r;
        Ok(false)
    }
    #[inline]
    fn complete(&mut self, pos: &mut usize, buf: &mut Vec<u8>) -> IoResult<()> {
        loop {
            if self.empty(pos, buf)? {
                break;
            }
            *pos += match self.read(*pos, buf)? {
                Some(0) => break,
                Some(v) => v,
                None => 0,
            };
            spin_loop();
        }
        Ok(())
    }
    #[inline]
    fn read(&mut self, pos: usize, buf: &mut Vec<u8>) -> IoResult<Option<usize>> {
        if buf.len() <= pos {
            buf.resize(pos + 0x100, 0);
        }
        match NtReadFile(&self.0.h, Some(&mut self.0.olp), &mut buf[pos..], None) {
            Err(Win32Error::IoPending) => Ok(None),
            Err(Win32Error::BrokenPipe) => Ok(Some(0)),
            Err(e) => Err(IoError::from(e)),
            Ok(n) => Ok(Some(n)),
        }
    }
}
impl Child {
    /// Returns the OS-assigned process identifier associated with this child.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// let mut command = Command::new("ls");
    /// if let Ok(child) = command.spawn() {
    ///     println!("Child's ID is {}", child.id());
    /// } else {
    ///     println!("ls command didn't start");
    /// }
    /// ```
    #[inline]
    pub fn id(&self) -> u32 {
        self.info.process_id
    }
    /// Forces the child process to exit. If the child has already exited,
    /// `Ok(())` is returned.
    ///
    /// The mapping to [`ErrorKind`]s is not part of the compatibility contract
    /// of the function.
    ///
    /// This is equivalent to sending a SIGKILL on Unix platforms.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// let mut command = Command::new("yes");
    /// if let Ok(mut child) = command.spawn() {
    ///     child.kill().expect("command couldn't be killed");
    /// } else {
    ///     println!("yes command didn't start");
    /// }
    /// ```
    ///
    /// [`ErrorKind`]: ErrorKind
    /// [`InvalidInput`]: ErrorKind::InvalidInput
    #[inline]
    pub fn kill(&mut self) -> IoResult<()> {
        Ok(TerminateProcess(&self.info.process, 0x1337)?)
    }
    /// Waits for the child to exit completely, returning the status that it
    /// exited with. This function will continue to have the same return value
    /// after it has been called at least once.
    ///
    /// The stdin handle to the child process, if any, will be closed
    /// before waiting. This helps avoid deadlock: it ensures that the
    /// child does not block waiting for input from the parent, while
    /// the parent waits for the child to exit.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// let mut command = Command::new("ls");
    /// if let Ok(mut child) = command.spawn() {
    ///     child.wait().expect("command wasn't running");
    ///     println!("Child has finished its execution!");
    /// } else {
    ///     println!("ls command didn't start");
    /// }
    /// ```
    #[inline]
    pub fn wait(&mut self) -> IoResult<ExitStatus> {
        // Close Stdin Handle.
        drop(self.stdin.take());
        Ok(WaitForSingleObject(&self.info.process, INFINITE, false).map(|_| self.exit_code())?)
    }
    // Simultaneously waits for the child to exit and collect all remaining
    /// output on the stdout/stderr handles, returning an `Output`
    /// instance.
    ///
    /// The stdin handle to the child process, if any, will be closed
    /// before waiting. This helps avoid deadlock: it ensures that the
    /// child does not block waiting for input from the parent, while
    /// the parent waits for the child to exit.
    ///
    /// By default, stdin, stdout and stderr are inherited from the parent.
    /// In order to capture the output into this `Result<Output>` it is
    /// necessary to create new pipes between parent and child. Use
    /// `stdout(Stdio::piped())` or `stderr(Stdio::piped())`, respectively.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use xrmt_stx::process::{Command, Stdio};
    ///
    /// let child = Command::new("/bin/cat")
    ///     .arg("file.txt")
    ///     .stdout(Stdio::piped())
    ///     .spawn()
    ///     .expect("failed to execute child");
    ///
    /// let output = child
    ///     .wait_with_output()
    ///     .expect("failed to wait on child");
    ///
    /// assert!(output.status.success());
    /// ```
    #[inline]
    pub fn wait_with_output(self) -> IoResult<Output> {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        Ok(Output {
            status: self.wait_output(&mut o, &mut e)?,
            stdout: o,
            stderr: e,
        })
    }
    /// Attempts to collect the exit status of the child if it has already
    /// exited.
    ///
    /// This function will not block the calling thread and will only
    /// check to see if the child process has exited or not. If the child has
    /// exited then on Unix the process ID is reaped. This function is
    /// guaranteed to repeatedly return a successful exit status so long as the
    /// child has already exited.
    ///
    /// If the child has exited, then `Ok(Some(status))` is returned. If the
    /// exit status is not available at this time then `Ok(None)` is returned.
    /// If an error occurs, then that error is returned.
    ///
    /// Note that unlike `wait`, this function will not attempt to drop stdin.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// let mut child = Command::new("ls").spawn()?;
    ///
    /// match child.try_wait() {
    ///     Ok(Some(status)) => println!("exited with: {status}"),
    ///     Ok(None) => {
    ///         println!("status not ready yet, let's really wait");
    ///         let res = child.wait();
    ///         println!("result: {res:?}");
    ///     }
    ///     Err(e) => println!("error attempting to wait: {e}"),
    /// }
    /// # xrmt_stx::IoResult::Ok(())
    /// ```
    #[inline]
    pub fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        Ok(WaitForSingleObject(&self.info.process, 0, false).map(|v| if v == 0 { Some(self.exit_code()) } else { None })?)
    }

    fn exit_code(&mut self) -> ExitStatus {
        // Wait for the Child process to finish if it hasn't yet.
        // Calls to this function when the process is running is similar to
        // 'wait' except this will ALWAYS return an ExitStatus even during
        // failure.
        let _ = WaitForSingleObject(&self.info.process, INFINITE, false);
        // ^ The wait is up here as we should make everyone wait instead of one
        // winning the race and waiting while the loosers return a bogus result.
        if self
            .done
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            let mut i = ProcessBasicInfo::default();
            // 0x0 - ProcessBasicInformation
            let _ = NtQueryInformationProcess(
                &self.info.process,
                0,
                &mut i,
                size_of::<ProcessBasicInfo>() as u32,
            );
            self.exit.store(i.exit_status as i32, Ordering::Release);
        }
        ExitStatus(self.exit.load(Ordering::Relaxed))
    }
    #[inline]
    fn wait_output(mut self, o: &mut Vec<u8>, e: &mut Vec<u8>) -> IoResult<ExitStatus> {
        drop(self.stdin.take()); // Close Stdin Handle.
        let _ = match (self.stdout.take(), self.stderr.take()) {
            (Some(so), Some(se)) => self.read_to_end(so, se, o, e)?,
            (Some(mut so), None) => so.read_to_end(o)?,
            (None, Some(mut se)) => se.read_to_end(e)?,
            (None, None) => 0,
        };
        Ok(self.exit_code())
    }
    fn read_to_end(&mut self, so: ChildStdout, se: ChildStderr, o: &mut Vec<u8>, e: &mut Vec<u8>) -> IoResult<usize> {
        let (mut s, mut x) = (Async(so), Async(se.0));
        let (mut j, mut k) = (s.start(0, o)?, x.start(0, e)?);
        let h = [*s.0.olp.event, *x.0.olp.event, self.info.process.as_usize()];
        loop {
            match unsafe { wait_for_multiple_objects(&h, 3, false, INFINITE, false) } {
                Err(e) => return Err(e.into()),
                Ok(c) => match c {
                    /* STDOUT */
                    0 => {
                        if s.empty(&mut j, o)? {
                            s.complete(&mut j, o)?;
                            break;
                        }
                    },
                    /* STDERR */
                    1 => {
                        if x.empty(&mut k, e)? {
                            x.complete(&mut k, e)?;
                            break;
                        }
                    },
                    /* PROCESS */ 2 => break,
                    _ => unreachable!(),
                },
            }
        }
        s.complete(&mut j, o)?;
        x.complete(&mut k, e)?;
        o.truncate(j);
        e.truncate(k);
        Ok(0)
    }
}
impl Stdio {
    /// This stream will be ignored. This is the equivalent of attaching the
    /// stream to `/dev/null`.
    ///
    /// # Examples
    ///
    /// With stdout:
    ///
    /// ```no_run
    /// use xrmt_stx::process::{Command, Stdio};
    ///
    /// let output = Command::new("echo")
    ///     .arg("Hello, world!")
    ///     .stdout(Stdio::null())
    ///     .output()
    ///     .expect("Failed to execute command");
    ///
    /// assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    /// // Nothing echoed to console
    /// ```
    ///
    /// With stdin:
    ///
    /// ```no_run
    /// use xrmt_stx::process::{Command, Stdio};
    ///
    /// let output = Command::new("rev")
    ///     .stdin(Stdio::null())
    ///     .stdout(Stdio::piped())
    ///     .output()
    ///     .expect("Failed to execute command");
    ///
    /// assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    /// // Ignores any piped-in input
    /// ```
    #[inline]
    pub fn null() -> Stdio {
        Stdio {
            v: StdioType::Null,
            h: unsafe { OwnedHandle::empty() },
        }
    }
    /// A new pipe should be arranged to connect the parent and child processes.
    ///
    /// # Examples
    ///
    /// With stdout:
    ///
    /// ```no_run
    /// use xrmt_stx::process::{Command, Stdio};
    ///
    /// let output = Command::new("echo")
    ///     .arg("Hello, world!")
    ///     .stdout(Stdio::piped())
    ///     .output()
    ///     .expect("Failed to execute command");
    ///
    /// assert_eq!(String::from_utf8_lossy(&output.stdout), "Hello, world!\n");
    /// // Nothing echoed to console
    /// ```
    ///
    /// With stdin:
    ///
    /// ```no_run
    /// use xrmt_stx::io::Write;
    /// use xrmt_stx::process::{Command, Stdio};
    ///
    /// let mut child = Command::new("rev")
    ///     .stdin(Stdio::piped())
    ///     .stdout(Stdio::piped())
    ///     .spawn()
    ///     .expect("Failed to spawn child process");
    ///
    /// let mut stdin = child.stdin.take().expect("Failed to open stdin");
    /// xrmt_stx::thread::spawn(move || {
    ///     stdin.write_all("Hello, world!".as_bytes()).expect("Failed to write to stdin");
    /// });
    ///
    /// let output = child.wait_with_output().expect("Failed to read stdout");
    /// assert_eq!(String::from_utf8_lossy(&output.stdout), "!dlrow ,olleH");
    /// ```
    ///
    /// Writing more than a pipe buffer's worth of input to stdin without also
    /// reading stdout and stderr at the same time may cause a deadlock.
    /// This is an issue when running any program that doesn't guarantee that it
    /// reads its entire stdin before writing more than a pipe buffer's
    /// worth of output. The size of a pipe buffer varies on different
    /// targets.
    #[inline]
    pub fn piped() -> Stdio {
        Stdio {
            v: StdioType::Pipe,
            h: unsafe { OwnedHandle::empty() },
        }
    }
    /// The child inherits from the corresponding parent descriptor.
    ///
    /// # Examples
    ///
    /// With stdout:
    ///
    /// ```no_run
    /// use xrmt_stx::process::{Command, Stdio};
    ///
    /// let output = Command::new("echo")
    ///     .arg("Hello, world!")
    ///     .stdout(Stdio::inherit())
    ///     .output()
    ///     .expect("Failed to execute command");
    ///
    /// assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    /// // "Hello, world!" echoed to console
    /// ```
    ///
    /// With stdin:
    ///
    /// ```no_run
    /// use xrmt_stx::process::{Command, Stdio};
    /// use xrmt_stx::io::{self, Write};
    ///
    /// let output = Command::new("rev")
    ///     .stdin(Stdio::inherit())
    ///     .stdout(Stdio::piped())
    ///     .output()?;
    ///
    /// print!("You piped in the reverse of: ");
    /// io::stdout().write_all(&output.stdout)?;
    /// # IoResult::Ok(())
    /// ```
    #[inline]
    pub fn inherit() -> Stdio {
        Stdio {
            v: StdioType::Inherit,
            h: unsafe { OwnedHandle::empty() },
        }
    }

    /// Returns `true` if this requires [`Command`] to create a new pipe.
    ///
    /// # Example
    ///
    /// ```
    /// use xrmt_stx::process::Stdio;
    ///
    /// let io = Stdio::piped();
    /// assert_eq!(io.makes_pipe(), true);
    /// ```
    #[inline]
    pub fn makes_pipe(&self) -> bool {
        match self.v {
            StdioType::Pipe => true,
            _ => false,
        }
    }
}
impl Output {
    #[inline]
    pub fn exit_ok(self) -> Result<Output, ExitStatusError> {
        self.status.exit_ok()?;
        Ok(self)
    }
}
impl Command {
    /// Constructs a new `Command` for launching the program at
    /// path `program`, with the following default configuration:
    ///
    /// * No arguments to the program
    /// * Inherit the current process's environment
    /// * Inherit the current process's working directory
    /// * Inherit stdin/stdout/stderr for [`spawn`] or [`status`], but create
    ///   pipes for [`output`]
    ///
    /// [`spawn`]: Command::spawn
    /// [`status`]: Command::status
    /// [`output`]: Command::output
    ///
    /// Builder methods are provided to change these defaults and
    /// otherwise configure the process.
    ///
    /// If `program` is not an absolute path, the `PATH` will be searched in
    /// an OS-defined way.
    ///
    /// The search path to be used may be controlled by setting the
    /// `PATH` environment variable on the Command,
    /// but this has some implementation limitations on Windows
    /// (see issue #37519).
    ///
    /// # Platform-specific behavior
    ///
    /// Note on Windows: For executable files with the .exe extension,
    /// it can be omitted when specifying the program for this Command.
    /// However, if the file has a different extension,
    /// a filename including the extension needs to be provided,
    /// otherwise the file won't be found.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// Command::new("sh")
    ///     .spawn()
    ///     .expect("sh command failed to start");
    /// ```
    ///
    /// # Caveats
    ///
    /// [`Command::new`] is only intended to accept the path of the program. If
    /// you pass a program path along with arguments like `Command::new("ls
    /// -l").spawn()`, it will try to search for `ls -l` literally. The
    /// arguments need to be passed separately, such as via [`arg`] or
    /// [`args`].
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// Command::new("ls")
    ///     .arg("-l") // arg passed separately
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    ///
    /// [`arg`]: Command::arg
    /// [`args`]: Command::args
    #[inline]
    pub fn new(exe: impl AsRef<OsStr>) -> Command {
        Command {
            _p:           PhantomData,
            dir:          None,
            env:          BTreeMap::new(),
            exe:          exe.as_ref().to_os_string(),
            args:         Vec::new(),
            mode:         0u8,
            flags:        0u32,
            clear:        false,
            stdin:        Stdio::inherit(),
            stdout:       Stdio::inherit(),
            stderr:       Stdio::inherit(),
            force_quotes: false,
        }
    }

    /// Returns the path to the program that was given to [`Command::new`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::process::Command;
    ///
    /// let cmd = Command::new("echo");
    /// assert_eq!(cmd.get_program(), "echo");
    /// ```
    #[inline]
    pub fn get_program(&self) -> &OsStr {
        &self.exe
    }
    /// Returns an iterator of the arguments that will be passed to the program.
    ///
    /// This does not include the path to the program as the first argument;
    /// it only includes the arguments specified with [`Command::arg`] and
    /// [`Command::args`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsStr;
    /// use xrmt_stx::process::Command;
    ///
    /// let mut cmd = Command::new("echo");
    /// cmd.arg("first").arg("second");
    /// let args: Vec<&OsStr> = cmd.get_args().collect();
    /// assert_eq!(args, &["first", "second"]);
    /// ```
    #[inline]
    pub fn get_args(&self) -> CommandArgs<'_> {
        CommandArgs {
            iter: self.args.iter(),
            _p:   PhantomData,
        }
    }
    /// Returns an iterator of the environment variables explicitly set for the
    /// child process.
    ///
    /// Environment variables explicitly set using [`Command::env`],
    /// [`Command::envs`], and [`Command::env_remove`] can be retrieved with
    /// this method.
    ///
    /// Note that this output does not include environment variables inherited
    /// from the parent process.
    ///
    /// Each element is a tuple key/value pair `(&OsStr, Option<&OsStr>)`. A
    /// [`None`] value indicates its key was explicitly removed via
    /// [`Command::env_remove`]. The associated key for the [`None`] value
    /// will no longer inherit from its parent process.
    ///
    /// An empty iterator can indicate that no explicit mappings were added or
    /// that [`Command::env_clear`] was called. After calling
    /// [`Command::env_clear`], the child process will not inherit any
    /// environment variables from its parent process.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsStr;
    /// use xrmt_stx::process::Command;
    ///
    /// let mut cmd = Command::new("ls");
    /// cmd.env("TERM", "dumb").env_remove("TZ");
    /// let envs: Vec<(&OsStr, Option<&OsStr>)> = cmd.get_envs().collect();
    /// assert_eq!(envs, &[
    ///     (OsStr::new("TERM"), Some(OsStr::new("dumb"))),
    ///     (OsStr::new("TZ"), None)
    /// ]);
    /// ```
    #[inline]
    pub fn get_envs(&self) -> CommandEnvs<'_> {
        CommandEnvs { iter: self.env.iter() }
    }
    /// Executes the command as a child process, returning a handle to it.
    ///
    /// By default, stdin, stdout and stderr are inherited from the parent.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// Command::new("ls")
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[inline]
    pub fn spawn(&mut self) -> IoResult<Child> {
        self.spawn_base(None)
    }
    /// Clears all explicitly set environment variables and prevents inheriting
    /// any parent process environment variables.
    ///
    /// This method will remove all explicitly added environment variables set
    /// via [`Command::env`] or [`Command::envs`]. In addition, it will
    /// prevent the spawned child process from inheriting any environment
    /// variable from its parent process.
    ///
    /// After calling [`Command::env_clear`], the iterator from
    /// [`Command::get_envs`] will be empty.
    ///
    /// You can use [`Command::env_remove`] to clear a single mapping.
    ///
    /// # Examples
    ///
    /// The behavior of `sort` is affected by `LANG` and `LC_*` environment
    /// variables. Clearing the environment makes `sort`'s behavior
    /// independent of the parent processes' language.
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// Command::new("sort")
    ///     .arg("file.txt")
    ///     .env_clear()
    ///     .spawn()?;
    /// # xrmt_stx::IoResult::Ok(())
    /// ```
    #[inline]
    pub fn env_clear(&mut self) -> &mut Command {
        self.env.clear();
        self.clear = true;
        self
    }
    /// Executes the command as a child process, waiting for it to finish and
    /// collecting all of its output.
    ///
    /// By default, stdout and stderr are captured (and used to provide the
    /// resulting output). Stdin is not inherited from the parent and any
    /// attempt by the child process to read from the stdin stream will result
    /// in the stream immediately closing.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use xrmt_stx::process::Command;
    /// use xrmt_stx::io::{self, Write};
    /// let output = Command::new("/bin/cat")
    ///     .arg("file.txt")
    ///     .output()?;
    ///
    /// println!("status: {}", output.status);
    /// io::stdout().write_all(&output.stdout)?;
    /// io::stderr().write_all(&output.stderr)?;
    ///
    /// assert!(output.status.success());
    /// # IoResult::Ok(())
    /// ```
    #[inline]
    pub fn output(&mut self) -> IoResult<Output> {
        self.spawn_outer(None, &self.stdin, &Stdio::piped(), &Stdio::piped())?
            .wait_with_output()
    }
    /// Returns the working directory for the child process.
    ///
    /// This returns [`None`] if the working directory will not be changed.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::path::Path;
    /// use xrmt_stx::process::Command;
    ///
    /// let mut cmd = Command::new("ls");
    /// assert_eq!(cmd.get_current_dir(), None);
    /// cmd.current_dir("/bin");
    /// assert_eq!(cmd.get_current_dir(), Some(Path::new("/bin")));
    /// ```
    #[inline]
    pub fn get_current_dir(&self) -> Option<&Path> {
        self.dir.as_ref().map(|v| v.as_path())
    }
    /// Executes a command as a child process, waiting for it to finish and
    /// collecting its status.
    ///
    /// By default, stdin, stdout and stderr are inherited from the parent.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use xrmt_stx::process::Command;
    ///
    /// let status = Command::new("/bin/cat")
    ///     .arg("file.txt")
    ///     .status()
    ///     .expect("failed to execute process");
    ///
    /// println!("process finished with: {status}");
    ///
    /// assert!(status.success());
    /// ```
    #[inline]
    pub fn status(&mut self) -> IoResult<ExitStatus> {
        self.spawn_base(None)?.wait()
    }
    /// Configuration for the child process's standard input (stdin) handle.
    ///
    /// Defaults to [`inherit`] when used with [`spawn`] or [`status`], and
    /// defaults to [`piped`] when used with [`output`].
    ///
    /// [`inherit`]: Stdio::inherit
    /// [`piped`]: Stdio::piped
    /// [`spawn`]: Command::spawn
    /// [`status`]: Command::status
    /// [`output`]: Command::output
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::{Command, Stdio};
    ///
    /// Command::new("ls")
    ///     .stdin(Stdio::null())
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[inline]
    pub fn stdin(&mut self, v: impl Into<Stdio>) -> &mut Command {
        self.stdin = v.into();
        self
    }
    /// Configuration for the child process's standard output (stdout) handle.
    ///
    /// Defaults to [`inherit`] when used with [`spawn`] or [`status`], and
    /// defaults to [`piped`] when used with [`output`].
    ///
    /// [`inherit`]: Stdio::inherit
    /// [`piped`]: Stdio::piped
    /// [`spawn`]: Command::spawn
    /// [`status`]: Command::status
    /// [`output`]: Command::output
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::{Command, Stdio};
    ///
    /// Command::new("ls")
    ///     .stdout(Stdio::null())
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[inline]
    pub fn stdout(&mut self, v: impl Into<Stdio>) -> &mut Command {
        self.stdout = v.into();
        self
    }
    /// Configuration for the child process's standard error (stderr) handle.
    ///
    /// Defaults to [`inherit`] when used with [`spawn`] or [`status`], and
    /// defaults to [`piped`] when used with [`output`].
    ///
    /// [`inherit`]: Stdio::inherit
    /// [`piped`]: Stdio::piped
    /// [`spawn`]: Command::spawn
    /// [`status`]: Command::status
    /// [`output`]: Command::output
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::{Command, Stdio};
    ///
    /// Command::new("ls")
    ///     .stderr(Stdio::null())
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[inline]
    pub fn stderr(&mut self, v: impl Into<Stdio>) -> &mut Command {
        self.stderr = v.into();
        self
    }
    /// Adds an argument to pass to the program.
    ///
    /// Only one argument can be passed per use. So instead of:
    ///
    /// ```no_run
    /// # xrmt_stx::process::Command::new("sh")
    /// .arg("-C /path/to/repo")
    /// # ;
    /// ```
    ///
    /// usage would be:
    ///
    /// ```no_run
    /// # xrmt_stx::process::Command::new("sh")
    /// .arg("-C")
    /// .arg("/path/to/repo")
    /// # ;
    /// ```
    ///
    /// To pass multiple arguments see [`args`].
    ///
    /// [`args`]: Command::args
    ///
    /// Note that the argument is not passed through a shell, but given
    /// literally to the program. This means that shell syntax like quotes,
    /// escaped characters, word splitting, glob patterns, variable
    /// substitution, etc. have no effect.
    ///
    /// <div class="warning">
    ///
    /// On Windows, use caution with untrusted inputs. Most applications use the
    /// standard convention for decoding arguments passed to them. These are
    /// safe to use with `arg`. However, some applications such as `cmd.exe`
    /// and `.bat` files use a non-standard way of decoding arguments. They
    /// are therefore vulnerable to malicious input.
    ///
    /// In the case of `cmd.exe` this is especially important because a
    /// malicious argument can potentially run arbitrary shell commands.
    ///
    /// See [Windows argument splitting][windows-args] for more details
    /// or [`raw_arg`] for manually implementing non-standard argument encoding.
    ///
    /// [`raw_arg`]: crate::os::windows::process::CommandExt::raw_arg
    /// [windows-args]: crate::process#windows-argument-splitting
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// Command::new("ls")
    ///     .arg("-l")
    ///     .arg("-a")
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[inline]
    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Command {
        self.args.push(Arg::new(arg));
        self
    }
    /// Removes an explicitly set environment variable and prevents inheriting
    /// it from a parent process.
    ///
    /// This method will remove the explicit value of an environment variable
    /// set via [`Command::env`] or [`Command::envs`]. In addition, it will
    /// prevent the spawned child process from inheriting that environment
    /// variable from its parent process.
    ///
    /// After calling [`Command::env_remove`], the value associated with its key
    /// from [`Command::get_envs`] will be [`None`].
    ///
    /// To clear all explicitly set environment variables and disable all
    /// environment variable inheritance, you can use
    /// [`Command::env_clear`].
    ///
    /// # Examples
    ///
    /// Prevent any inherited `GIT_DIR` variable from changing the target of the
    /// `git` command, while allowing all other variables, like
    /// `GIT_AUTHOR_NAME`.
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// Command::new("git")
    ///     .arg("commit")
    ///     .env_remove("GIT_DIR")
    ///     .spawn()?;
    /// # xrmt_stx::IoResult::Ok(())
    /// ```
    #[inline]
    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Command {
        self.env.remove(key.as_ref());
        self
    }
    /// Sets the working directory for the child process.
    ///
    /// # Platform-specific behavior
    ///
    /// If the program path is relative (e.g., `"./script.sh"`), it's ambiguous
    /// whether it should be interpreted relative to the parent's working
    /// directory or relative to `current_dir`. The behavior in this case is
    /// platform specific and unstable, and it's recommended to use
    /// [`canonicalize`] to get an absolute program path instead.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// Command::new("ls")
    ///     .current_dir("/bin")
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    ///
    /// [`canonicalize`]: crate::fs::canonicalize
    #[inline]
    pub fn current_dir(&mut self, dir: impl AsRef<Path>) -> &mut Command {
        self.dir = Some(dir.as_ref().to_path_buf());
        self
    }
    /// Inserts or updates an explicit environment variable mapping.
    ///
    /// This method allows you to add an environment variable mapping to the
    /// spawned process or overwrite a previously set value. You can use
    /// [`Command::envs`] to set multiple environment
    /// variables simultaneously.
    ///
    /// Child processes will inherit environment variables from their parent
    /// process by default. Environment variables explicitly set using
    /// [`Command::env`] take precedence over inherited variables. You can
    /// disable environment variable inheritance entirely using
    /// [`Command::env_clear`] or for a single key using
    /// [`Command::env_remove`].
    ///
    /// Note that environment variable names are case-insensitive (but
    /// case-preserving) on Windows and case-sensitive on all other platforms.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// Command::new("ls")
    ///     .env("PATH", "/bin")
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[inline]
    pub fn env(&mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) -> &mut Command {
        self.env.insert(key.as_ref().to_os_string(), val.as_ref().to_os_string());
        self
    }
    /// Adds multiple arguments to pass to the program.
    ///
    /// To pass a single argument see [`arg`].
    ///
    /// [`arg`]: Command::arg
    ///
    /// Note that the arguments are not passed through a shell, but given
    /// literally to the program. This means that shell syntax like quotes,
    /// escaped characters, word splitting, glob patterns, variable
    /// substitution, etc. have no effect.
    ///
    /// <div class="warning">
    ///
    /// On Windows, use caution with untrusted inputs. Most applications use the
    /// standard convention for decoding arguments passed to them. These are
    /// safe to use with `arg`. However, some applications such as `cmd.exe`
    /// and `.bat` files use a non-standard way of decoding arguments. They
    /// are therefore vulnerable to malicious input.
    ///
    /// In the case of `cmd.exe` this is especially important because a
    /// malicious argument can potentially run arbitrary shell commands.
    ///
    /// See [Windows argument splitting][windows-args] for more details
    /// or [`raw_arg`] for manually implementing non-standard argument encoding.
    ///
    /// [`raw_arg`]: crate::os::windows::process::CommandExt::raw_arg
    /// [windows-args]: crate::process#windows-argument-splitting
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// Command::new("ls")
    ///     .args(["-l", "-a"])
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[inline]
    pub fn args<T: AsRef<OsStr>>(&mut self, args: impl IntoIterator<Item = T>) -> &mut Command {
        self.args.extend(args.into_iter().map(Arg::new));
        self
    }
    /// Inserts or updates multiple explicit environment variable mappings.
    ///
    /// This method allows you to add multiple environment variable mappings to
    /// the spawned process or overwrite previously set values. You can use
    /// [`Command::env`] to set a single environment variable.
    ///
    /// Child processes will inherit environment variables from their parent
    /// process by default. Environment variables explicitly set using
    /// [`Command::envs`] take precedence over inherited variables. You can
    /// disable environment variable inheritance entirely using
    /// [`Command::env_clear`] or for a single key using
    /// [`Command::env_remove`].
    ///
    /// Note that environment variable names are case-insensitive (but
    /// case-preserving) on Windows and case-sensitive on all other
    /// platforms.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::{Command, Stdio};
    /// use xrmt_stx::env;
    /// use xrmt_stx::collections::HashMap;
    ///
    /// let filtered_env : HashMap<String, String> =
    ///     env::vars().filter(|&(ref k, _)|
    ///         k == "TERM" || k == "TZ" || k == "LANG" || k == "PATH"
    ///     ).collect();
    ///
    /// Command::new("printenv")
    ///     .stdin(Stdio::null())
    ///     .stdout(Stdio::inherit())
    ///     .env_clear()
    ///     .envs(&filtered_env)
    ///     .spawn()
    ///     .expect("printenv failed to start");
    /// ```
    #[inline]
    pub fn envs<T: AsRef<OsStr>>(&mut self, vars: impl IntoIterator<Item = (T, T)>) -> &mut Command {
        self.env.extend(
            vars.into_iter()
                .map(|(k, v)| (v.as_ref().to_os_string(), k.as_ref().to_os_string())),
        );
        self
    }

    fn resolve_path(&self) -> IoResult<WChar> {
        let mut b = WChar::from(&self.exe);
        // Check if file exists at raw path.
        if file_is_file(&b) {
            return Ok(b);
        }
        let e = GetEnvironment();
        // Get PATHEXT, but fallback to default if we can't find it.
        let p = e.find(&PATHEXT);
        str_const!(0, ".exe;.com;.bat;.cmd", 20, x);
        let v = p.and_then(|v| v.value_as_slice()).unwrap_or(&x);
        // Try once to see if the binary exists with the "full path" before
        // looking in PATH.
        if find_in_dir(None, &v, &mut b) {
            // If this returns true, 'b' will have been modified by the above func.
            return Ok(b);
        }
        if b.iter().rposition(|x| *x == 0x5C || *x == 0x2F || *x == 0x3A).is_some() {
            // It was a full path, fail it as we shouldn't search for it.
            return Err(IoError::from(ErrorKind::NotFound));
        }
        // Get the PATH env var, fallback to WinDir/System32
        if let Some(k) = e.find(&PATHEXT[0..4]).and_then(|v| v.value_as_slice()) {
            for i in k.split(|v| *v == 0x3B) {
                if find_in_dir(Some(i), &v, &mut b) {
                    return Ok(b);
                }
            }
        }
        // Look in %WinDir%\System32 first
        if find_in_dir(Some(&system_dir()), &v, &mut b) {
            return Ok(b);
        }
        // Look in %WinDir% next
        if find_in_dir(Some(&system_root()), &v, &mut b) {
            return Ok(b);
        }
        // Can't find it.
        Err(ErrorKind::NotFound.into())
    }
    fn cmdline(&self) -> IoResult<(WChar, WChar)> {
        let mut a = self.resolve_path()?;
        let k = unsafe { a.as_mut_vec() };
        // NOTE(dij): CreateProcess does not like it when we use the NT path for
        //            things, so we have to remove it.
        //
        if k.len() > 4 && k[0] == 0x5C && k[1] == 0x3F && k[2] == 0x3F && k[3] == 0x5C {
            unsafe {
                let n = k.len() - 4;
                copy(k.as_ptr().add(4), k.as_mut_ptr(), n);
                k.set_len(n);
            }
        }
        let mut e = a.clone();
        let p = unsafe { e.as_mut_vec() };
        // Remove NULL if it exists
        // It shouldn't, but let's 100% be sure about that.
        if p.last().is_some_and(|v| *v == 0) {
            unsafe { p.set_len(p.len() - 1) };
        }
        // Do we have whitespace? Add quotes around the binary path (argv[0])
        if p.iter().any(|v| *v == 0x20 || *v == 0x9) {
            p.insert(0, 0x22);
            p.push(0x22);
        }
        if self.args.is_empty() {
            e.add_null();
            return Ok((a, e));
        }
        for i in self.args.iter() {
            let (q, x) = (i.is_quoted(self.force_quotes), !i.is_raw());
            let v = i.as_os_str().as_encoded_bytes();
            p.reserve(v.len() + if q { 3 } else { 1 });
            p.push(0x20);
            if q {
                p.push(0x22);
            }
            let mut k = q; // Set initial quote flag if we're quoted.
            for i in U16Encoder::new(v) {
                match i {
                    0x5C if x => p.push(0x5C), // Add another '\' to '\' to escape it.
                    0x22 if x => {
                        if k {
                            p.push(0x5C); // Add a '\' before any '"' if quotes
                                          // are in play
                        }
                        if !q {
                            k = false; // Remove quote flag if the arg isn't
                                       // quoted.
                        }
                    },
                    _ => (),
                }
                p.push(i);
            }
            if q {
                p.push(0x22);
            }
        }
        // Ensure the command line is NULL padded.
        e.add_null();
        Ok((a, e))
    }
    #[inline]
    fn spawn_base<'a>(&self, params: Option<&'a StartParameters>) -> IoResult<Child> {
        self.spawn_outer(params, &self.stdin, &self.stdout, &self.stderr)
    }
    fn spawn_outer<'a>(&self, params: Option<&'a StartParameters>, stdin: &Stdio, stdout: &Stdio, stderr: &Stdio) -> IoResult<Child> {
        let (a, c) = self.cmdline()?;
        let p = match params {
            Some(v) => v.parent()?,
            None => None,
        };
        let (mut i, mut o, mut e) = (
            PipeHandle::new(stdin, PipeType::Stdin, &p)?,
            PipeHandle::new(stdout, PipeType::Stdout, &p)?,
            PipeHandle::new(stderr, PipeType::Stderr, &p)?,
        );
        // NOTE(dij): We start with outer as we hold these pipes from dropping
        //            so we can setup the rest of the stuff first and handle
        //            "cleaner" if it fails.
        let r = self.spawn_center(a, c, &p, &i, &o, &e, params)?;
        // let h = r.process.as_handle(); // Take a non-owned Copy that won't get
        // dropped.
        Ok(Child {
            stdin:  i.take().map(ChildStdin),
            stdout: match o.take() {
                Some(v) => Some(ChildStdout {
                    h:   v,
                    olp: OwnedOverlapped::new()?,
                    // parent: h,
                }),
                None => None,
            },
            stderr: match e.take() {
                Some(v) => Some(ChildStderr(ChildStdout {
                    h:   v,
                    olp: OwnedOverlapped::new()?,
                    // parent: h,
                })),
                None => None,
            },
            done:   AtomicBool::new(false),
            exit:   AtomicI32::new(0i32),
            info:   r,
        })
    }
    fn spawn_inner<'a>(&self, app: WChar, cmd: WChar, env: WChar, parent: &'a Option<OwnedHandle>, params: Option<&'a StartParameters>, info: StartInfo) -> IoResult<ProcessInfo> {
        debug_assert!(app.is_null_padded());
        debug_assert!(cmd.is_null_padded());
        debug_assert!(env.is_null_padded());
        // Convert Startup Directory
        let d = match &self.dir {
            Some(v) => v.into(),
            None => WChar::null(),
        };
        let f = self.flags | params.map_or(0, |v| v.flags);
        let r = match params {
            Some(a) => match (&a.user, &a.token) {
                (WCharLike::Null, Some(t)) => CreateProcessWithToken(t, 0x2, app, cmd, f, env, d, info),
                (u, _) if !u.is_null() => {
                    // In order to impersonate a user, the current Thread MUST
                    // clear it's impersonation Token first! If one exists, we
                    // 'Rev2Self' then restore it after the Create call.
                    //
                    // We only do this if we're the parent Process.
                    let p = if parent.is_none() && !is_windows_xp() {
                        take_current_thread_token().ok().flatten()
                    } else {
                        None
                    };
                    let r = CreateProcessWithLogon(
                        u,
                        &a.domain,
                        &a.password,
                        0,
                        &app.into(),
                        &cmd.into(),
                        f,
                        &env.into(),
                        &d.into(),
                        info,
                    );
                    if let Some(v) = p {
                        let _ = SetThreadToken(CURRENT_THREAD, v);
                    }
                    r
                },
                _ => CreateProcess(app, cmd, None, None, true, f, env, d, info),
            },
            _ => CreateProcess(app, cmd, None, None, true, f, env, d, info),
        };
        Ok(r?)
    }
    fn spawn_center<'a>(&self, app: WChar, cmd: WChar, parent: &'a Option<OwnedHandle>, si: &'a PipeHandle<'a>, so: &'a PipeHandle<'a>, se: &'a PipeHandle<'a>, params: Option<&'a StartParameters>) -> IoResult<ProcessInfo> {
        // Make Environment WChar string
        let mut e = ProcessEnvironment::new(
            |(k, v), x| {
                x.write_u8(k.as_encoded_bytes());
                x.push_u8(0x3D); // =
                x.write_u8(v.as_encoded_bytes());
            },
            self.env.iter(),
        );
        if !self.clear {
            e.include_system();
        }
        let w = e.into_wchar();
        let mut s = StartupInfo::default();
        // We always add the handles, they may be NUL but we always add them.
        // If we're doing a parent process, we always need these.
        s.flags = 0x100; // STARTF_USESTDHANDLES
        (s.stdin, s.stdout, s.stderr) = (si.handle(), so.handle(), se.handle());
        if self.mode & 0x40 != 0 {
            s.flags |= 0x1; // STARTF_USESHOWWINDOW
            s.show_window = (self.mode & 0x3F) as u16;
        }
        if self.mode & 0x80 != 0 {
            s.flags |= 0x20; // STARTF_RUNFULLSCREEN
        }
        if let Some(v) = params {
            (s.pos_x, s.pos_y) = (v.x, v.y);
            if s.pos_x != 0 || s.pos_y != 0 {
                s.flags |= 0x4; // STARTF_USEPOSITION
            }
            (s.size_x, s.size_y) = (v.width, v.height);
            if s.size_x > 0 || s.size_y > 0 {
                s.flags |= 0x2; // STARTF_USESIZE
            }
            if !v.title.is_null() {
                // SAFETY: We guard the addition of this, so it'll always have a NULL end.
                s.title = unsafe { v.title.as_char_ptr() };
            }
            if !v.desktop.is_null() {
                // SAFETY: We guard the addition of this, so it'll always have a NULL end.
                s.desktop = unsafe { v.title.as_char_ptr() };
            }
        }
        // Check StartupInfoEx support in 'x'. The 'm' value checks it we can
        // specify mitigations
        let (x, m) = version_support();
        let z = match (m, params.and_then(|v| v.mitigations).map(|v| v.get())) {
            // If 'v' is -1, this will disable mitigations
            (true, Some(0xFFFFFFFFFFFFFFFF)) => 0u64,
            (true, Some(v)) => v,
            (..) => 0x100100000000u64,
        };
        if !x || (z == 0 && parent.is_none()) {
            // StartupInfoEx support does not exist, or there's no reason to use
            // an *Ex structure. (No parent/no mitigations, etc).
            return self.spawn_inner(app, cmd, w, parent, params, StartInfo::Basic(&s));
        }
        // Build StartupInfoEx.
        //
        // At this point we can guarantee that we have Handles to be set (they
        // may be a NUL dev). Now we need to check Sec and Parent settings.
        // We also know we have StartupInfoEx support here.
        //
        // We Box this to make sure it stays alive while we start the Process.
        let q = if z > 0 { Some(Box::new(z)) } else { None };
        // Handles shouldn't need a Box since they are simple.
        let k = [s.stdin, s.stdout, s.stderr];
        let mut a = ProcessThreadAttrList::default();
        match parent {
            Some(h) => {
                a.set_parent(0, h);
                a.set_handles(1, &k);
                if let Some(v) = &q {
                    a.set_mitigation(3, v.as_ref());
                }
            },
            None => {
                a.set_handles(0, &k);
                if let Some(v) = &q {
                    a.set_mitigation(1, v.as_ref());
                }
            },
        }
        let v = StartupInfoEx::new(s, &a);
        self.spawn_inner(app, cmd, w, parent, params, StartInfo::Extended(&v))
    }
}
impl ExitCode {
    /// The canonical `ExitCode` for successful termination on this platform.
    ///
    /// Note that a `()`-returning `main` implicitly results in a successful
    /// termination, so there's no need to return this from `main` unless
    /// you're also returning other possible codes.
    pub const SUCCESS: ExitCode = ExitCode(0i32);
    /// The canonical `ExitCode` for unsuccessful termination on this platform.
    ///
    /// If you're only returning this and `SUCCESS` from `main`, consider
    /// instead returning `Err(_)` and `Ok(())` respectively, which will
    /// return the same codes (but will also `eprintln!` the error).
    pub const FAILURE: ExitCode = ExitCode(1i32);

    /// Exit the current process with the given `ExitCode`.
    ///
    /// Note that this has the same caveats as [`process::exit()`][exit], namely
    /// that this function terminates the process immediately, so no
    /// destructors on the current stack or any other thread's stack will be
    /// run. If a clean shutdown is needed, it is recommended to simply
    /// return this ExitCode from the `main` function, as demonstrated in the
    /// [type documentation](#examples).
    ///
    /// # Differences from `process::exit()`
    ///
    /// `process::exit()` accepts any `i32` value as the exit code for the
    /// process; however, there are platforms that only use a subset of that
    /// value (see [`process::exit` platform-specific behavior][exit#
    /// platform-specific-behavior]). `ExitCode` exists because of this; only
    /// `ExitCode`s that are supported by a majority of our platforms can be
    /// created, so those problems don't exist (as much) with this method.
    ///
    /// # Examples
    ///
    /// ```
    /// // there's no way to gracefully recover from an UhOhError, so we just
    /// // print a message and exit
    /// fn handle_unrecoverable_error(err: UhOhError) -> ! {
    ///     eprintln!("UH OH! {err}");
    ///     let code = match err {
    ///         UhOhError::GenericProblem => ExitCode::FAILURE,
    ///         UhOhError::Specific => ExitCode::from(3),
    ///         UhOhError::WithCode { exit_code, .. } => exit_code,
    ///     };
    ///     code.exit_process()
    /// }
    /// ```
    #[inline]
    pub fn exit_process(self) -> ! {
        exit_process(self.0 as u32)
    }

    #[inline]
    pub(crate) fn code(&self) -> u32 {
        self.0 as u32
    }
}
impl ExitStatus {
    /// Was termination successful? Signal termination is not considered a
    /// success, and success is defined as a zero exit status.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use xrmt_stx::process::Command;
    ///
    /// let status = Command::new("mkdir")
    ///     .arg("projects")
    ///     .status()
    ///     .expect("failed to execute mkdir");
    ///
    /// if status.success() {
    ///     println!("'projects/' directory created");
    /// } else {
    ///     println!("failed to create 'projects/' directory: {status}");
    /// }
    /// ```
    #[inline]
    pub fn success(&self) -> bool {
        self.0 == 0
    }
    // Returns the exit code of the process, if any.
    ///
    /// In Unix terms the return value is the **exit status**: the value passed
    /// to `exit`, if the process finished by calling `exit`.  Note that on
    /// Unix the exit status is truncated to 8 bits, and that values that
    /// didn't come from a program's call to `exit` may be invented by the
    /// runtime system (often, for example, 255, 254, 127 or 126).
    ///
    /// On Unix, this will return `None` if the process was terminated by a
    /// signal. `ExitStatusExt` (`xrmt_stx::os::unix::process::ExitStatusExt``)
    /// is an extension trait for extracting any such signal, and other
    /// details, from the `ExitStatus`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    ///
    /// let status = Command::new("mkdir")
    ///     .arg("projects")
    ///     .status()
    ///     .expect("failed to execute mkdir");
    ///
    /// match status.code() {
    ///     Some(code) => println!("Exited with status code: {code}"),
    ///     None => println!("Process terminated by signal")
    /// }
    /// ```
    #[inline]
    pub fn code(&self) -> Option<i32> {
        Some(self.0)
    }
    /// Was termination successful?  Returns a `Result`.
    ///
    /// # Examples
    ///
    /// ```
    /// # if cfg!(unix) {
    /// use xrmt_stx::process::Command;
    ///
    /// let status = Command::new("ls")
    ///     .arg("/dev/nonexistent")
    ///     .status()
    ///     .expect("ls could not be executed");
    ///
    /// println!("ls: {status}");
    /// status.exit_ok().expect_err("/dev/nonexistent could be listed!");
    /// # } // cfg!(unix)
    /// ```
    #[inline]
    pub fn exit_ok(&self) -> Result<(), ExitStatusError> {
        if self.0 == 0 {
            Ok(())
        } else {
            Err(ExitStatusError(*self))
        }
    }
}
impl ExitStatusError {
    /// Reports the exit code, if applicable, from an `ExitStatusError`.
    ///
    /// In Unix terms the return value is the **exit status**: the value passed
    /// to `exit`, if the process finished by calling `exit`.  Note that on
    /// Unix the exit status is truncated to 8 bits, and that values that
    /// didn't come from a program's call to `exit` may be invented by the
    /// runtime system (often, for example, 255, 254, 127 or 126).
    ///
    /// On Unix, this will return `None` if the process was terminated by a
    /// signal.  If you want to handle such situations specially, consider
    /// using methods from
    /// `ExitStatusExt` (`xrmt_stx::os::unix::process::ExitStatusExt`).
    ///
    /// If the process finished by calling `exit` with a nonzero value, this
    /// will return that exit status.
    ///
    /// If the error was something else, it will return `None`.
    ///
    /// If the process exited successfully (ie, by calling `exit(0)`), there is
    /// no `ExitStatusError`.  So the return value from
    /// `ExitStatusError::code()` is always nonzero.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::process::Command;
    ///
    /// let bad = Command::new("false").status().unwrap().exit_ok().unwrap_err();
    /// assert_eq!(bad.code(), Some(1));
    /// ```
    #[inline]
    pub fn code(&self) -> Option<i32> {
        Some(self.0 .0)
    }
    /// Converts an `ExitStatusError` (back) to an `ExitStatus`.
    #[inline]
    pub fn into_status(self) -> ExitStatus {
        self.0
    }
    /// Reports the exit code, if applicable, from an `ExitStatusError`, as a
    /// [`NonZero`].
    ///
    /// This is exactly like [`code()`](Self::code), except that it returns a
    /// <code>[NonZero]<[i32]></code>.
    ///
    /// [NonZero]: core::num::NonZero
    /// [`NonZero`]: core::num::NonZero
    ///
    /// Plain `code`, returning a plain integer, is provided because it is often
    /// more convenient. The returned value from `code()` is indeed also
    /// nonzero; use `code_nonzero()` when you want a type-level guarantee
    /// of nonzeroness.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::num::NonZero;
    /// use xrmt_stx::process::Command;
    ///
    /// let bad = Command::new("false").status().unwrap().exit_ok().unwrap_err();
    /// assert_eq!(bad.code_nonzero().unwrap(), NonZero::new(1).unwrap());
    /// ```
    #[inline]
    pub fn code_nonzero(&self) -> Option<NonZeroI32> {
        NonZeroI32::new(self.0 .0)
    }
}
impl<'a> PipeHandle<'a> {
    #[inline]
    fn handle(&self) -> Handle {
        match self {
            PipeHandle::Local(h) => **h,
            PipeHandle::Remote(_, h) => *h,
            PipeHandle::LocalPipe(_, h) => **h,
            PipeHandle::RemotePipe(_, _, h) => *h,
        }
    }
    #[inline]
    fn take(&mut self) -> Option<OwnedHandle> {
        // We're gonna use OwnedHandle.take so we can preserve drop glue.
        let h = match self {
            PipeHandle::Local(_) => return None,
            PipeHandle::Remote(..) => return None,
            PipeHandle::LocalPipe(h, _) => unsafe { h.take() },
            PipeHandle::RemotePipe(_, h, _) => unsafe { h.take() },
        };
        // Remove Inheritance
        let _ = SetHandleInformation(&h, false, false);
        Some(h)
    }

    #[inline]
    fn make_pipe(pipe: PipeType, parent: &'a Option<OwnedHandle>) -> IoResult<PipeHandle<'a>> {
        let (r, w) = {
            let s = SecurityAttributes::inherit();
            CreatePipe(Some(&s), 0x1000, true)?
        };
        match (pipe, parent) {
            // Flip read/write if stdin
            (PipeType::Stdin, Some(v)) => Ok(PipeHandle::RemotePipe(v, w, r.into_duplicate(true, v)?)),
            (_, Some(v)) => Ok(PipeHandle::RemotePipe(v, r, w.into_duplicate(true, v)?)),
            (PipeType::Stdin, None) => Ok(PipeHandle::LocalPipe(w, r)),
            (_, None) => Ok(PipeHandle::LocalPipe(r, w)),
        }
    }
    #[inline]
    fn make_null(pipe: PipeType, parent: &'a Option<OwnedHandle>) -> IoResult<PipeHandle<'a>> {
        let h = {
            let s = SecurityAttributes::inherit();
            str_const!(0, r"\??\NUL", 8, n);
            NtCreateFile(
                &n,
                Handle::EMPTY,
                match pipe {
                    PipeType::Stdin => 0x80100080,
                    _ => 0x40000000,
                },
                Some(&s),
                0,
                0x3,
                0x1,
                0,
            )?
        };
        match parent {
            Some(v) => Ok(PipeHandle::Remote(v, h.into_duplicate(true, v)?)),
            None => Ok(PipeHandle::Local(h)),
        }
    }
    #[inline]
    fn make_inherit(pipe: PipeType, parent: &'a Option<OwnedHandle>) -> IoResult<PipeHandle<'a>> {
        let p = GetCurrentProcessPEB().process_params();
        if let Some(v) = parent {
            return match pipe {
                // NOTE(dij): Until Win8 we can't duplicate a STDIN Handle for
                //            a child process that's under a different parent
                //            so we copy a NUL Handle as a fallback.
                PipeType::Stdin if !p.standard_input.is_invalid() && is_min_windows_8() => Ok(PipeHandle::Remote(
                    v,
                    DuplicateHandleEx(p.standard_input, CURRENT_PROCESS, v, 0, true, 0x2)?,
                )),
                PipeType::Stdout if !p.standard_output.is_invalid() => Ok(PipeHandle::Remote(
                    v,
                    DuplicateHandleEx(p.standard_output, CURRENT_PROCESS, v, 0, true, 0x2)?,
                )),
                PipeType::Stderr if !p.standard_error.is_invalid() => Ok(PipeHandle::Remote(
                    v,
                    DuplicateHandleEx(p.standard_error, CURRENT_PROCESS, v, 0, true, 0x2)?,
                )),
                _ => PipeHandle::make_null(pipe, parent),
            };
        }
        match pipe {
            PipeType::Stdin if !p.standard_input.is_invalid() => Ok(PipeHandle::Local(
                DuplicateHandleEx(
                    p.standard_input,
                    CURRENT_PROCESS,
                    CURRENT_PROCESS,
                    0,
                    true,
                    0x2,
                )?
                .into(),
            )),
            PipeType::Stdout if !p.standard_output.is_invalid() => Ok(PipeHandle::Local(
                DuplicateHandleEx(
                    p.standard_output,
                    CURRENT_PROCESS,
                    CURRENT_PROCESS,
                    0,
                    true,
                    0x2,
                )?
                .into(),
            )),
            PipeType::Stderr if !p.standard_error.is_invalid() => Ok(PipeHandle::Local(
                DuplicateHandleEx(
                    p.standard_error,
                    CURRENT_PROCESS,
                    CURRENT_PROCESS,
                    0,
                    true,
                    0x2,
                )?
                .into(),
            )),
            _ => PipeHandle::make_null(pipe, parent),
        }
    }
    #[inline]
    fn make_handle(h: &OwnedHandle, parent: &'a Option<OwnedHandle>) -> IoResult<PipeHandle<'a>> {
        match parent {
            Some(v) => Ok(PipeHandle::Remote(
                v,
                DuplicateHandleEx(h, CURRENT_PROCESS, v, 0, true, 0x2)?,
            )),
            None => Ok(PipeHandle::Local(
                DuplicateHandleEx(h, CURRENT_PROCESS, parent, 0, true, 0x2)?.into(),
            )),
        }
    }
    #[inline]
    fn new(s: &Stdio, pipe: PipeType, parent: &'a Option<OwnedHandle>) -> IoResult<PipeHandle<'a>> {
        match s.v {
            StdioType::Null => PipeHandle::make_null(pipe, parent),
            StdioType::Pipe => PipeHandle::make_pipe(pipe, parent),
            StdioType::Inherit => PipeHandle::make_inherit(pipe, parent),
            StdioType::Handle => PipeHandle::make_handle(&s.h, parent),
        }
    }
}
impl<'a> StartParameters<'a> {
    fn parent(&self) -> IoResult<Option<OwnedHandle>> {
        Ok(None)
    }
}

impl Drop for Async {
    #[inline]
    fn drop(&mut self) {
        // Cancel any pending IO.
        let _ = CancelIoEx(&self.0.h, &mut self.0.olp);
    }
}
impl<'a> Drop for PipeHandle<'a> {
    #[inline]
    fn drop(&mut self) {
        // NOTE(dij): Remote Handles need to be Duplicated with specific flags to close
        //            them remotely. Open them with the close_open flag then close them
        //            on our side.
        //
        // Local Handles can be closed by dropping them directly.
        match self {
            PipeHandle::Remote(p, h) | PipeHandle::RemotePipe(p, _, h) => {
                // 0x3 - DUPLICATE_CLOSE_SOURCE | DUPLICATE_SAME_ACCESS
                let _ = DuplicateHandleEx(h, *p, CURRENT_PROCESS, 0, false, 0x3).map(|v| unsafe { close_handle(v) });
            },
            _ => (),
        }
    }
}

impl From<Child> for OwnedHandle {
    #[inline]
    fn from(v: Child) -> OwnedHandle {
        v.info.process
    }
}
impl From<ChildStdin> for OwnedHandle {
    #[inline]
    fn from(v: ChildStdin) -> OwnedHandle {
        v.0
    }
}
impl From<ChildStdout> for OwnedHandle {
    #[inline]
    fn from(v: ChildStdout) -> OwnedHandle {
        v.h
    }
}
impl From<ChildStderr> for OwnedHandle {
    #[inline]
    fn from(v: ChildStderr) -> OwnedHandle {
        v.0.h
    }
}

impl From<File> for Stdio {
    #[inline]
    fn from(v: File) -> Stdio {
        Stdio {
            v: StdioType::Handle,
            h: v.into(),
        }
    }
}
impl From<Handle> for Stdio {
    #[inline]
    fn from(v: Handle) -> Stdio {
        Stdio {
            v: StdioType::Handle,
            h: v.into(),
        }
    }
}
impl From<Stdout> for Stdio {
    #[inline]
    fn from(v: Stdout) -> Stdio {
        Stdio {
            v: StdioType::Handle,
            h: v.as_ref()
                .into_duplicate(false, CURRENT_PROCESS)
                .unwrap_or(Handle::EMPTY)
                .into(),
        }
    }
}
impl From<Stderr> for Stdio {
    #[inline]
    fn from(v: Stderr) -> Stdio {
        let h = match v.as_ref().duplicate() {
            Ok(x) => x,
            Err(_) => unsafe { OwnedHandle::empty() },
        };
        Stdio { h, v: StdioType::Handle }
    }
}
impl From<PipeReader> for Stdio {
    #[inline]
    fn from(v: PipeReader) -> Stdio {
        Stdio {
            v: StdioType::Handle,
            h: v.into(),
        }
    }
}
impl From<PipeWriter> for Stdio {
    #[inline]
    fn from(v: PipeWriter) -> Stdio {
        Stdio {
            v: StdioType::Handle,
            h: v.into(),
        }
    }
}
impl From<ChildStdin> for Stdio {
    #[inline]
    fn from(v: ChildStdin) -> Stdio {
        Stdio { v: StdioType::Handle, h: v.0 }
    }
}
impl From<ChildStdout> for Stdio {
    #[inline]
    fn from(v: ChildStdout) -> Stdio {
        Stdio { v: StdioType::Handle, h: v.h }
    }
}
impl From<ChildStderr> for Stdio {
    #[inline]
    fn from(v: ChildStderr) -> Stdio {
        Stdio { v: StdioType::Handle, h: v.0.h }
    }
}
impl From<OwnedHandle> for Stdio {
    #[inline]
    fn from(v: OwnedHandle) -> Stdio {
        Stdio { v: StdioType::Handle, h: v }
    }
}

impl Eq for Output {}
impl Clone for Output {
    #[inline]
    fn clone(&self) -> Output {
        Output {
            status: self.status,
            stdout: self.stdout.clone(),
            stderr: self.stderr.clone(),
        }
    }
}
impl Debug for Output {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let (o, e) = (
            core::str::from_utf8(&self.stdout),
            core::str::from_utf8(&self.stderr),
        );
        f.debug_struct("Output")
            .field("status", &self.status)
            .field("stdout", &o)
            .field("stderr", &e)
            .finish()
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl PartialEq for Output {
    #[inline]
    fn eq(&self, other: &Output) -> bool {
        self.status == other.status && self.stdout == other.stdout && self.stderr == other.stderr
    }
}

impl Write for ChildStdin {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(NtFlushBuffersFile(&self.0)?)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        Ok(NtWriteFile(&self.0, None, buf, None)?)
    }
}
impl Write for &ChildStdin {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(NtFlushBuffersFile(&self.0)?)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        Ok(NtWriteFile(&self.0, None, buf, None)?)
    }
}

impl Read for ChildStdout {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match NtReadFile(&self.h, Some(&mut self.olp), buf, None) {
            Ok(v) => Ok(v),
            Err(Win32Error::IoPending) => {
                //let _ = unsafe { wait_for_multiple_objects(&[*self.olp.event, *self.parent],
                // 2, false, -1, false) };
                let _ = WaitForSingleObject(&self.olp.event, INFINITE, false);
                Ok(self.olp.internal_high)
            },
            Err(e) => Err(IoError::from(e)),
        }
    }
}

impl Read for ChildStderr {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.0.read(buf)
    }
}
impl Deref for ChildStderr {
    type Target = ChildStdout;

    #[inline]
    fn deref(&self) -> &ChildStdout {
        &self.0
    }
}
impl DerefMut for ChildStderr {
    #[inline]
    fn deref_mut(&mut self) -> &mut ChildStdout {
        &mut self.0
    }
}

impl Eq for ExitCode {}
impl Copy for ExitCode {}
impl Clone for ExitCode {
    #[inline]
    fn clone(&self) -> ExitCode {
        ExitCode(self.0)
    }
}
impl Debug for ExitCode {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.into_fmt(f)
    }
}
impl Display for ExitCode {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.into_fmt(f)
    }
}
impl Default for ExitCode {
    #[inline]
    fn default() -> ExitCode {
        ExitCode::SUCCESS
    }
}
impl From<u8> for ExitCode {
    #[inline]
    fn from(v: u8) -> ExitCode {
        ExitCode(v as i32)
    }
}
impl PartialEq for ExitCode {
    #[inline]
    fn eq(&self, other: &ExitCode) -> bool {
        self.0.eq(&other.0)
    }
}
impl Termination for ExitCode {
    #[inline]
    fn report(self) -> ExitCode {
        self
    }
}

impl Eq for ExitStatus {}
impl Copy for ExitStatus {}
impl Clone for ExitStatus {
    #[inline]
    fn clone(&self) -> ExitStatus {
        ExitStatus(self.0)
    }
}
impl Debug for ExitStatus {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.into_fmt(f)
    }
}
impl Display for ExitStatus {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.into_fmt(f)
    }
}
impl Default for ExitStatus {
    #[inline]
    fn default() -> ExitStatus {
        ExitStatus(0)
    }
}
impl PartialEq for ExitStatus {
    #[inline]
    fn eq(&self, other: &ExitStatus) -> bool {
        self.0.eq(&other.0)
    }
}
impl From<ExitStatusError> for ExitStatus {
    #[inline]
    fn from(v: ExitStatusError) -> ExitStatus {
        v.0
    }
}

impl Eq for ExitStatusError {}
impl Copy for ExitStatusError {}
impl Debug for ExitStatusError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}
impl Error for ExitStatusError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Clone for ExitStatusError {
    #[inline]
    fn clone(&self) -> ExitStatusError {
        ExitStatusError(self.0)
    }
}
impl Display for ExitStatusError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let mut b = [0u8; 21];
        f.write_str(self.0 .0.into_str(&mut b))
    }
}
impl PartialEq for ExitStatusError {
    #[inline]
    fn eq(&self, other: &ExitStatusError) -> bool {
        self.0 == other.0
    }
}

impl<'a> Iterator for CommandArgs<'a> {
    type Item = &'a OsStr;

    #[inline]
    fn next(&mut self) -> Option<&'a OsStr> {
        self.iter.next().map(|v| v.as_os_str())
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}
impl FusedIterator for CommandArgs<'_> {}
impl<'a> ExactSizeIterator for CommandArgs<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a> Iterator for CommandEnvs<'a> {
    type Item = (&'a OsStr, Option<&'a OsStr>);

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
    #[inline]
    fn next(&mut self) -> Option<(&'a OsStr, Option<&'a OsStr>)> {
        self.iter
            .next()
            .map(|(k, v)| (k.as_os_str(), Some(OsStr::new(v.as_os_str()))))
    }
}
impl FusedIterator for CommandEnvs<'_> {}
impl<'a> ExactSizeIterator for CommandEnvs<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl Termination for ! {
    #[inline]
    fn report(self) -> ExitCode {
        self
    }
}
impl Termination for () {
    #[inline]
    fn report(self) -> ExitCode {
        ExitCode::SUCCESS
    }
}
impl Termination for Infallible {
    #[inline]
    fn report(self) -> ExitCode {
        match self {}
    }
}
impl<T: Termination, E: Debug> Termination for Result<T, E> {
    #[inline]
    fn report(self) -> ExitCode {
        match self {
            Ok(v) => v.report(),
            Err(_) => ExitCode::FAILURE,
        }
    }
}

/// Returns the OS-assigned process identifier associated with this process.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::process;
///
/// println!("My pid is {}", process::id());
/// ```
#[inline]
pub fn id() -> u32 {
    GetCurrentProcessID()
}
/// Terminates the process in an abnormal fashion.
///
/// The function will never return and will immediately terminate the current
/// process in a platform specific "abnormal" manner. As a consequence,
/// no destructors on the current stack or any other thread's stack
/// will be run, Rust IO buffers (eg, from `BufWriter`) will not be flushed,
/// and C stdio buffers will (on most platforms) not be flushed.
///
/// This is in contrast to the default behavior of [`panic!`] which
/// unwinds the current thread's stack and calls all destructors.
/// When `panic="abort"` is set, either as an argument to `rustc` or in a
/// crate's Cargo.toml, [`panic!`] and `abort` are similar. However,
/// [`panic!`] will still call the [panic hook] while `abort` will not.
///
/// If a clean shutdown is needed it is recommended to only call
/// this function at a known point where there are no more destructors left
/// to run.
///
/// The process's termination will be similar to that from the C `abort()`
/// function.  On Unix, the process will terminate with signal `SIGABRT`, which
/// typically means that the shell prints "Aborted".
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::process;
///
/// fn main() {
///     println!("aborting");
///
///     process::abort();
///
///     // execution never gets here
/// }
/// ```
///
/// The `abort` function terminates the process, so the destructor will not
/// get run on the example below:
///
/// ```no_run
/// use xrmt_stx::process;
///
/// struct HasDrop;
///
/// impl Drop for HasDrop {
///     fn drop(&mut self) {
///         println!("This will never be printed!");
///     }
/// }
///
/// fn main() {
///     let _x = HasDrop;
///     process::abort();
///     // the destructor implemented for HasDrop will never get run
/// }
/// ```
/// [`panic!`]: core::panic!
#[inline]
pub fn abort() -> ! {
    exit(1)
}
/// Terminates the current process with the specified exit code.
///
/// This function will never return and will immediately terminate the current
/// process. The exit code is passed through to the underlying OS and will be
/// available for consumption by another process.
///
/// Note that because this function never returns, and that it terminates the
/// process, no destructors on the current stack or any other thread's stack
/// will be run. If a clean shutdown is needed it is recommended to only call
/// this function at a known point where there are no more destructors left
/// to run; or, preferably, simply return a type implementing [`Termination`]
/// (such as [`ExitCode`] or `Result`) from the `main` function and avoid this
/// function altogether:
///
/// ```
/// # use xrmt_stx::io::Error as MyError;
/// fn main() -> Result<(), MyError> {
///     // ...
///     Ok(())
/// }
/// ```
///
/// In its current implementation, this function will execute exit handlers
/// registered with `atexit` as well as other platform-specific exit handlers
/// (e.g. `fini` sections of ELF shared objects). This means that Rust requires
/// that all exit handlers are safe to execute at any time. In particular, if an
/// exit handler cleans up some state that might be concurrently accessed by
/// other threads, it is required that the exit handler performs suitable
/// synchronization with those threads. (The alternative to this requirement
/// would be to not run exit handlers at all, which is considered undesirable.
/// Note that returning from `main` also calls `exit`, so making `exit` an
/// unsafe operation is not an option.)
///
/// ## Platform-specific behavior
///
/// **Unix**: On Unix-like platforms, it is unlikely that all 32 bits of `exit`
/// will be visible to a parent process inspecting the exit code. On most
/// Unix-like platforms, only the eight least-significant bits are considered.
///
/// For example, the exit code for this example will be `0` on Linux, but `256`
/// on Windows:
///
/// ```no_run
/// use xrmt_stx::process;
///
/// process::exit(0x0100);
/// ```
#[inline]
pub fn exit(code: i32) -> ! {
    exit_process(code as u32)
}

fn version_support() -> (bool, bool) {
    match VERSION.compare_exchange(0, 0x80, Ordering::AcqRel, Ordering::Relaxed) {
        Ok(_) => {
            let v = SystemVersion::get();
            let r = match v.major {
                0..=5 => (false, false),
                6 => (v.minor > 2, true),
                _ => (true, true),
            };
            VERSION.store(
                0x80 | if r.0 { 0x1 } else { 0 } | if r.1 { 0x2 } else { 0 },
                Ordering::Release,
            );
            r
        },
        Err(v) => (v & 0x1 != 0, v & 0x2 != 0),
    }
}
fn find_in_dir(dir: Option<&[u16]>, ext: &[u16], file: &mut WChar) -> bool {
    let p = unsafe { file.as_mut_vec() };
    // Remove NULL if it exists
    if p.last().is_some_and(|v| *v == 0) {
        unsafe { p.set_len(p.len() - 1) };
    }
    // If we have a dir to look into, append that to the START of the WChar.
    let e = match dir {
        Some(d) => {
            let n = p.len();
            // repr:
            //  d,a,t,a,p,a,t,h
            //
            // 'd' might have a NULL ending. If there's no NULL, we can copy
            // the entire thing.
            let v = if d.last().is_some_and(|v| *v != 0) {
                d.len()
            } else {
                d.len() - 1
            };
            // Add it plus NULL
            p.resize(n + v + 2, 0);
            // repr:
            //  d,a,t,a,p,a,t,h,0,0,0,0,0,0,0,0,0,0,0
            //
            // Copy the orig data to the "end"
            unsafe { copy(p.as_ptr(), p.as_mut_ptr().add(v + 1), n) };
            // repr:
            //  d,a,t,a,p,a,t,h,0,0,d,a,t,a,p,a,t,h,0
            //
            // Copy the dir as the prefix
            unsafe { copy_nonoverlapping(d.as_ptr(), p.as_mut_ptr(), v) };
            // repr:
            //  m,y,\,d,i,r,\,a,b,0,d,a,t,a,p,a,t,h,0
            //
            // Insert the path seperator
            p[v] = 0x5C;
            //       ^ '\'
            // repr:
            //  m,y,\,d,i,r,\,a,b,\,d,a,t,a,p,a,t,h,0
            //
            v + 1 // Return count extended.
        },
        None => 0,
    };
    // Convert the path to NT to save allocations during checks.
    // Copy it if it's a reference.
    {
        let mut n = path_normalize(&*p).into_owned();
        if file_is_file(&n) {
            // First check
            swap(file, &mut n);
            return true;
        }
        let x = unsafe { n.as_mut_vec() };
        // Remove NULL, it should be added by the 'into_wchar' contact.
        unsafe { x.set_len(x.len() - 1) };
        x.reserve(4);
        for i in ext.split(|v| *v == 0x3B) {
            x.extend_from_slice(&i);
            x.push(0); // Add NULL
            if file_is_file(&*x) {
                // If this passes, we gonna do something funny
                // We're gonna swap the two pointers, so the "good" path get's
                // "returned" in 'file' and the old one gets dropped, saving
                // us another NT conversion.
                swap(file, &mut n);
                return true;
            }
            // Remove ext + NULL
            unsafe { x.set_len(x.len() - (i.len() + 1)) };
        }
    }
    // Did we extend it? Fix it back.
    if e > 0 {
        // This will include the NULL ending.
        let n = p.len() - e;
        // repr:
        //  m,y,\,d,i,r,\,a,b,\,d,a,t,a,p,a,t,h,0
        //
        unsafe { copy(p.as_ptr().add(e), p.as_mut_ptr(), n) };
        // repr:
        //  d,a,t,a,p,a,t,h,0,\,d,a,t,a,p,a,t,h,0
        //
        unsafe { p.set_len(n) };
    } else {
        // Add back NULL
        p.push(0);
    }
    false
}
