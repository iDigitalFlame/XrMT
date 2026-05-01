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
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate core;

use core::convert::AsRef;

use crate::ffi::OsStr;
use crate::os::windows::io::BorrowedHandle;

pub trait ChildExt {
    /// Extracts the main thread raw handle, without taking ownership
    fn main_thread_handle(&self) -> BorrowedHandle<'_>;
}
/// Windows-specific extensions to the [`Command`] builder.
///
/// This trait is sealed: it cannot be implemented outside the standard library.
/// This is so that future additional methods are not breaking changes.
///
/// [`Command`]: crate::process::Command
pub trait CommandExt {
    /// Sets the [process creation flags][1] to be passed to `CreateProcess`.
    ///
    /// These will always be ORed with `CREATE_UNICODE_ENVIRONMENT`.
    ///
    /// [1]: https://docs.microsoft.com/en-us/windows/win32/procthread/process-creation-flags
    fn creation_flags(&mut self, flags: u32) -> &mut Self;
    /// Sets the field `wShowWindow` of [STARTUPINFO][1] that is passed to
    /// `CreateProcess`. Allowed values are the ones listed in
    /// <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow>
    ///
    /// [1]: <https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/ns-processthreadsapi-startupinfow>
    fn show_window(&mut self, cmd_show: u16) -> &mut Self;
    /// Forces all arguments to be wrapped in quote (`"`) characters.
    ///
    /// This is useful for passing arguments to [MSYS2/Cygwin][1] based
    /// executables: these programs will expand unquoted arguments containing
    /// wildcard characters (`?` and `*`) by searching for any file paths
    /// matching the wildcard pattern.
    ///
    /// Adding quotes has no effect when passing arguments to programs
    /// that use [msvcrt][2]. This includes programs built with both
    /// MinGW and MSVC.
    ///
    /// [1]: <https://github.com/msys2/MSYS2-packages/issues/2176>
    /// [2]: <https://msdn.microsoft.com/en-us/library/17w5ykft.aspx>
    fn force_quotes(&mut self, enabled: bool) -> &mut Self;
    /// Append literal text to the command line without any quoting or escaping.
    ///
    /// This is useful for passing arguments to applications that don't follow
    /// the standard C run-time escaping rules, such as `cmd.exe /c`.
    ///
    /// # Batch files
    ///
    /// Note the `cmd /c` command line has slightly different escaping rules
    /// than batch files themselves. If possible, it may be better to write
    /// complex arguments to a temporary `.bat` file, with appropriate
    /// escaping, and simply run that using:
    ///
    /// ```no_run
    /// # use xrmt_stx::process::Command;
    /// # let temp_bat_file = "";
    /// # #[allow(unused)]
    /// let output = Command::new("cmd").args(["/c", &format!("\"{temp_bat_file}\"")]).output();
    /// ```
    ///
    /// # Example
    ///
    /// Run a batch script using both trusted and untrusted arguments.
    ///
    /// ```no_run
    /// #[cfg(windows)]
    /// // `my_script_path` is a path to known bat file.
    /// // `user_name` is an untrusted name given by the user.
    /// fn run_script(
    ///     my_script_path: &str,
    ///     user_name: &str,
    /// ) -> Result<xrmt_stx::process::Output, xrmt_stx::io::Error> {
    ///     use xrmt_stx::io::{Error, ErrorKind};
    ///     use xrmt_stx::os::windows::process::CommandExt;
    ///     use xrmt_stx::process::Command;
    ///
    ///     // Create the command line, making sure to quote the script path.
    ///     // This assumes the fixed arguments have been tested to work with the script we're using.
    ///     let mut cmd_args = format!(r#""{my_script_path}" "--features=[a,b,c]""#);
    ///
    ///     // Make sure the user name is safe. In particular we need to be
    ///     // cautious of ascii symbols that cmd may interpret specially.
    ///     // Here we only allow alphanumeric characters.
    ///     if !user_name.chars().all(|c| c.is_alphanumeric()) {
    ///         return Err(Error::new(ErrorKind::InvalidInput, "invalid user name"));
    ///     }
    ///
    ///     // now we have validated the user name, let's add that too.
    ///     cmd_args.push_str(" --user ");
    ///     cmd_args.push_str(user_name);
    ///
    ///     // call cmd.exe and return the output
    ///     Command::new("cmd.exe")
    ///         .arg("/c")
    ///         // surround the entire command in an extra pair of quotes, as required by cmd.exe.
    ///         .raw_arg(&format!("\"{cmd_args}\""))
    ///         .output()
    /// }
    /// ````
    fn raw_arg(&mut self, raw: impl AsRef<OsStr>) -> &mut Self;

    /// When [`Command`] creates pipes, request that our side is
    /// always async.
    ///
    /// By default [`Command`] may choose to use pipes where
    /// both ends are opened for synchronous read or write operations. By
    /// using `async_pipes(true)`, this behavior is overridden so that our
    /// side is always async.
    ///
    /// This is important because if doing async I/O a pipe or a file has to be
    /// opened for async access.
    ///
    /// The end of the pipe sent to the child process will always be synchronous
    /// regardless of this option.
    ///
    /// # Example
    ///
    /// ```
    /// #![feature(windows_process_extensions_async_pipes)]
    /// use xrmt_stx::os::windows::process::CommandExt;
    /// use xrmt_stx::process::{Command, Stdio};
    ///
    /// # let program = "";
    ///
    /// Command::new(program)
    ///     .async_pipes(true)
    ///     .stdin(Stdio::piped())
    ///     .stdout(Stdio::piped())
    ///     .stderr(Stdio::piped());
    /// ```
    ///
    /// [`Command`]: crate::process::Command
    fn async_pipes(&mut self, _always_async: bool) -> &mut Self {
        self
    }
}
/// Windows-specific extensions to [`ExitCode`].
///
/// This trait is sealed: it cannot be implemented outside the standard library.
/// This is so that future additional methods are not breaking changes.
///
/// [`ExitCode`]: crate::process::ExitCode
pub trait ExitCodeExt {
    /// Creates a new `ExitCode` from the raw underlying `u32` return value of
    /// a process.
    ///
    /// The exit code should not be 259, as this conflicts with the
    /// `STILL_ACTIVE` macro returned from the `GetExitCodeProcess` function
    /// to signal that the process has yet to run to completion.
    fn from_raw(raw: u32) -> Self;
}
/// Windows-specific extensions to [`ExitStatus`].
///
/// This trait is sealed: it cannot be implemented outside the standard library.
/// This is so that future additional methods are not breaking changes.
///
/// [`ExitStatus`]: crate::process::ExitStatus
pub trait ExitStatusExt {
    /// Creates a new `ExitStatus` from the raw underlying `u32` return value of
    /// a process.
    fn from_raw(raw: u32) -> Self;
}
