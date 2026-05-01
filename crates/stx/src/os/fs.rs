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

extern crate xrmt_winapi;

use core::convert::AsRef;
use core::option::Option;
use core::result::Result::Ok;

use xrmt_winapi::functions::{privilege_accquire, privilege_release, CreateDirectoryJunction, CreateSymbolicLink};
use xrmt_winapi::structs::Privilege;

use crate::io::IoResult;
use crate::path::Path;
use crate::time::SystemTime;

/// Windows-specific extensions to [`File`].
///
/// [`File`]: crate::fs::File
pub trait FileExt {
    /// Seeks to a given position and reads a number of bytes.
    ///
    /// Returns the number of bytes read.
    ///
    /// The offset is relative to the start of the file and thus independent
    /// from the current cursor. The current cursor **is** affected by this
    /// function, it is set to the end of the read.
    ///
    /// Reading beyond the end of the file will always return with a length of
    /// 0\.
    ///
    /// Note that similar to `File::read`, it is not an error to return with a
    /// short read. When returning from such a short read, the file pointer is
    /// still updated.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///     let mut buffer = [0; 10];
    ///
    ///     // Read 10 bytes, starting 72 bytes from the
    ///     // start of the file.
    ///     file.seek_read(&mut buffer[..], 72)?;
    ///     Ok(())
    /// }
    /// ```
    fn seek_write(&mut self, buf: &[u8], offset: u64) -> IoResult<usize>;
    /// Seeks to a given position and writes a number of bytes.
    ///
    /// Returns the number of bytes written.
    ///
    /// The offset is relative to the start of the file and thus independent
    /// from the current cursor. The current cursor **is** affected by this
    /// function, it is set to the end of the write.
    ///
    /// When writing beyond the end of the file, the file is appropriately
    /// extended and the intermediate bytes are set to zero.
    ///
    /// Note that similar to `File::write`, it is not an error to return a
    /// short write. When returning from such a short write, the file pointer
    /// is still updated.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     // Write a byte string starting 72 bytes from
    ///     // the start of the file.
    ///     buffer.seek_write(b"some bytes", 72)?;
    ///     Ok(())
    /// }
    /// ```
    fn seek_read(&mut self, buf: &mut [u8], offset: u64) -> IoResult<usize>;
}
/// Windows-specific extensions to [`FileType`].
///
/// On Windows, a symbolic link knows whether it is a file or directory.
///
/// [`FileType`]: crate::fs::FileType
pub trait FileTypeExt {
    /// Returns `true` if this file type is a symbolic link that is also a
    /// directory.
    fn is_symlink_dir(&self) -> bool;
    /// Returns `true` if this file type is a symbolic link that is also a file.
    fn is_symlink_file(&self) -> bool;
}
/// Windows-specific extensions to [`Metadata`].
///
/// The data members that this trait exposes correspond to the members
/// of the [`BY_HANDLE_FILE_INFORMATION`] structure.
///
/// [`BY_HANDLE_FILE_INFORMATION`]:
///     https://docs.microsoft.com/windows/win32/api/fileapi/ns-fileapi-by_handle_file_information
///
/// [`Metadata`]: crate::fs::Metadata
pub trait MetadataExt {
    /// Returns the value of the `nFileSize` fields of this
    /// metadata.
    ///
    /// The returned value does not have meaning for directories.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::fs;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// fn main() -> IoResult<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let file_size = metadata.file_size();
    ///     Ok(())
    /// }
    /// ```
    fn file_size(&self) -> u64;
    /// Returns the value of the `ftCreationTime` field of this metadata.
    ///
    /// The returned 64-bit value is equivalent to a [`FILETIME`] struct,
    /// which represents the number of 100-nanosecond intervals since
    /// January 1, 1601 (UTC). The struct is automatically
    /// converted to a `u64` value, as that is the recommended way
    /// to use it.
    ///
    /// If the underlying filesystem does not support creation time, the
    /// returned value is 0.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::fs;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// fn main() -> IoResult<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let creation_time = metadata.creation_time();
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`FILETIME`]: https://docs.microsoft.com/windows/win32/api/minwinbase/ns-minwinbase-filetime
    fn creation_time(&self) -> u64;
    /// Returns the value of the `dwFileAttributes` field of this metadata.
    ///
    /// This field contains the file system attribute information for a file
    /// or directory. For possible values and their descriptions, see
    /// [File Attribute Constants] in the Windows Dev Center.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::fs;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// fn main() -> IoResult<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let attributes = metadata.file_attributes();
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [File Attribute Constants]:
    ///     https://docs.microsoft.com/windows/win32/fileio/file-attribute-constants
    fn file_attributes(&self) -> u32;
    /// Returns the value of the `ftLastWriteTime` field of this metadata.
    ///
    /// The returned 64-bit value is equivalent to a [`FILETIME`] struct,
    /// which represents the number of 100-nanosecond intervals since
    /// January 1, 1601 (UTC). The struct is automatically
    /// converted to a `u64` value, as that is the recommended way
    /// to use it.
    ///
    /// For a file, the value specifies the last time that a file was written
    /// to. For a directory, the structure specifies when the directory was
    /// created.
    ///
    /// If the underlying filesystem does not support the last write time,
    /// the returned value is 0.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::fs;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// fn main() -> IoResult<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let last_write_time = metadata.last_write_time();
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`FILETIME`]: https://docs.microsoft.com/windows/win32/api/minwinbase/ns-minwinbase-filetime
    fn last_write_time(&self) -> u64;
    // Returns the value of the `ftLastAccessTime` field of this metadata.
    ///
    /// The returned 64-bit value is equivalent to a [`FILETIME`] struct,
    /// which represents the number of 100-nanosecond intervals since
    /// January 1, 1601 (UTC). The struct is automatically
    /// converted to a `u64` value, as that is the recommended way
    /// to use it.
    ///
    /// For a file, the value specifies the last time that a file was read
    /// from or written to. For a directory, the value specifies when
    /// the directory was created. For both files and directories, the
    /// specified date is correct, but the time of day is always set to
    /// midnight.
    ///
    /// If the underlying filesystem does not support last access time, the
    /// returned value is 0.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::fs;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// fn main() -> IoResult<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let last_access_time = metadata.last_access_time();
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`FILETIME`]: https://docs.microsoft.com/windows/win32/api/minwinbase/ns-minwinbase-filetime
    fn last_access_time(&self) -> u64;
    /// Returns the value of the `nFileIndex` fields of this
    /// metadata.
    ///
    /// This will return `None` if the `Metadata` instance was created from a
    /// call to `DirEntry::metadata`. If this `Metadata` was created by using
    /// `fs::metadata` or `File::metadata`, then this will return `Some`.
    fn file_index(&self) -> Option<u64>;
    /// Returns the value of the `ChangeTime` fields of this metadata.
    ///
    /// `ChangeTime` is the last time file metadata was changed, such as
    /// renames, attributes, etc.
    ///
    /// This will return `None` if `Metadata` instance was created from a call
    /// to `DirEntry::metadata` or if the `target_vendor` is outside the
    /// current platform support for this api.
    fn change_time(&self) -> Option<u64>;
    /// Returns the value of the `nNumberOfLinks` field of this
    /// metadata.
    ///
    /// This will return `None` if the `Metadata` instance was created from a
    /// call to `DirEntry::metadata`. If this `Metadata` was created by using
    /// `fs::metadata` or `File::metadata`, then this will return `Some`.
    fn number_of_links(&self) -> Option<u32>;
    /// Returns the value of the `dwVolumeSerialNumber` field of this
    /// metadata.
    ///
    /// This will return `None` if the `Metadata` instance was created from a
    /// call to `DirEntry::metadata`. If this `Metadata` was created by using
    /// `fs::metadata` or `File::metadata`, then this will return `Some`.
    fn volume_serial_number(&self) -> Option<u32>;
}
/// Windows-specific extensions to [`FileTimes`].
///
/// [`FileTimes`]: crate::fs::FileTimes
pub trait FileTimesExt {
    /// Set the creation time of a file.
    fn set_created(self, t: SystemTime) -> Self;
}
/// Windows-specific extensions to [`OpenOptions`].
///
/// [`OpenOptions`]: crate::fs::OpenOptions
pub trait OpenOptionsExt {
    /// Overrides the `dwShareMode` argument to the call to [`CreateFile`] with
    /// the specified value.
    ///
    /// By default `share_mode` is set to
    /// `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE`. This allows
    /// other processes to read, write, and delete/rename the same file
    /// while it is open. Removing any of the flags will prevent other
    /// processes from performing the corresponding operation until the file
    /// handle is closed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::OpenOptions;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// // Do not allow others to read or modify this file while we have it open
    /// // for writing.
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .share_mode(0)
    ///     .open("foo.txt");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    fn share_mode(&mut self, share: u32) -> &mut Self;
    /// Overrides the `dwDesiredAccess` argument to the call to [`CreateFile`]
    /// with the specified value.
    ///
    /// This will override the `read`, `write`, and `append` flags on the
    /// `OpenOptions` structure. This method provides fine-grained control over
    /// the permissions to read, write and append data, attributes (like hidden
    /// and system), and extended attributes.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::OpenOptions;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// // Open without read and write permission, for example if you only need
    /// // to call `stat` on the file
    /// let file = OpenOptions::new().access_mode(0).open("foo.txt");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    fn access_mode(&mut self, access: u32) -> &mut Self;
    // Sets extra flags for the `dwFileFlags` argument to the call to
    /// [`CreateFile2`] to the specified value (or combines it with
    /// `attributes` and `security_qos_flags` to set the `dwFlagsAndAttributes`
    /// for [`CreateFile`]).
    ///
    /// Custom flags can only set flags, not remove flags set by Rust's options.
    /// This option overwrites any previously set custom flags.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #![allow(unexpected_cfgs)]
    /// # #[cfg(for_demonstration_only)]
    /// extern crate winapi;
    /// # mod winapi { pub const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x04000000; }
    ///
    /// use xrmt_stx::fs::OpenOptions;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// let file = OpenOptions::new()
    ///     .create(true)
    ///     .write(true)
    ///     .custom_flags(winapi::FILE_FLAG_DELETE_ON_CLOSE)
    ///     .open("foo.txt");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    /// [`CreateFile2`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfile2
    fn custom_flags(&mut self, flags: u32) -> &mut Self;
    /// Sets the `dwFileAttributes` argument to the call to [`CreateFile2`] to
    /// the specified value (or combines it with `custom_flags` and
    /// `security_qos_flags` to set the `dwFlagsAndAttributes` for
    /// [`CreateFile`]).
    ///
    /// If a _new_ file is created because it does not yet exist and
    /// `.create(true)` or `.create_new(true)` are specified, the new file is
    /// given the attributes declared with `.attributes()`.
    ///
    /// If an _existing_ file is opened with `.create(true).truncate(true)`, its
    /// existing attributes are preserved and combined with the ones declared
    /// with `.attributes()`.
    ///
    /// In all other cases the attributes get ignored.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #![allow(unexpected_cfgs)]
    /// # #[cfg(for_demonstration_only)]
    /// extern crate winapi;
    /// # mod winapi { pub const FILE_ATTRIBUTE_HIDDEN: u32 = 2; }
    ///
    /// use xrmt_stx::fs::OpenOptions;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .create(true)
    ///     .attributes(winapi::FILE_ATTRIBUTE_HIDDEN)
    ///     .open("foo.txt");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    /// [`CreateFile2`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfile2
    fn attributes(&mut self, attributes: u32) -> &mut Self;
    /// Sets the `dwSecurityQosFlags` argument to the call to [`CreateFile2`] to
    /// the specified value (or combines it with `custom_flags` and `attributes`
    /// to set the `dwFlagsAndAttributes` for [`CreateFile`]).
    ///
    /// By default `security_qos_flags` is not set. It should be specified when
    /// opening a named pipe, to control to which degree a server process can
    /// act on behalf of a client process (security impersonation level).
    ///
    /// When `security_qos_flags` is not set, a malicious program can gain the
    /// elevated privileges of a privileged Rust process when it allows opening
    /// user-specified paths, by tricking it into opening a named pipe. So
    /// arguably `security_qos_flags` should also be set when opening arbitrary
    /// paths. However the bits can then conflict with other flags, specifically
    /// `FILE_FLAG_OPEN_NO_RECALL`.
    ///
    /// For information about possible values, see [Impersonation Levels] on the
    /// Windows Dev Center site. The `SECURITY_SQOS_PRESENT` flag is set
    /// automatically when using this method.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #![allow(unexpected_cfgs)]
    /// # #[cfg(for_demonstration_only)]
    /// extern crate winapi;
    /// # mod winapi { pub const SECURITY_IDENTIFICATION: u32 = 0; }
    /// use xrmt_stx::fs::OpenOptions;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .create(true)
    ///
    ///     // Sets the flag value to `SecurityIdentification`.
    ///     .security_qos_flags(winapi::SECURITY_IDENTIFICATION)
    ///
    ///     .open(r"\\.\pipe\MyPipe");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    /// [`CreateFile2`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfile2
    /// [Impersonation Levels]:
    ///     https://docs.microsoft.com/en-us/windows/win32/api/winnt/ne-winnt-security_impersonation_level
    fn security_qos_flags(&mut self, flags: u32) -> &mut Self;
}

/// Creates a new symlink to a non-directory file on the filesystem.
///
/// The `link` path will be a file symbolic link pointing to the `original`
/// path.
///
/// The `original` path should not be a directory or a symlink to a directory,
/// otherwise the symlink will be broken. Use [`symlink_dir`] for directories.
///
/// This function currently corresponds to
/// [`CreateSymbolicLinkW`][CreateSymbolicLinkW]. Note that this [may change in
/// the future][changes].
///
/// [CreateSymbolicLinkW]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createsymboliclinkw
/// [changes]: io#platform-specific-behavior
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::os::windows::fs;
///
/// fn main() -> IoResult<()> {
///     fs::symlink_file("a.txt", "b.txt")?;
///     Ok(())
/// }
/// ```
///
/// # Limitations
///
/// Windows treats symlink creation as a [privileged action][symlink-security],
/// therefore this function is likely to fail unless the user makes changes to
/// their system to permit symlink creation. Users can try enabling Developer
/// Mode, granting the `SeCreateSymbolicLinkPrivilege` privilege, or running
/// the process as an administrator.
///
/// [symlink-security]: https://docs.microsoft.com/en-us/windows/security/threat-protection/security-policy-settings/create-symbolic-links
#[inline]
pub fn symlink_dir(original: impl AsRef<Path>, link: impl AsRef<Path>) -> IoResult<()> {
    let _ = privilege_accquire(Privilege::SeCreateSymbolicLink)?;
    // 0x1 - SYMBOLIC_LINK_FLAG_DIRECTORY
    let r = CreateSymbolicLink(link.as_ref(), original.as_ref(), 0x1);
    let _ = privilege_release(Privilege::SeCreateSymbolicLink);
    Ok(r?)
}
/// Creates a new symlink to a directory on the filesystem.
///
/// The `link` path will be a directory symbolic link pointing to the `original`
/// path.
///
/// The `original` path must be a directory or a symlink to a directory,
/// otherwise the symlink will be broken. Use [`symlink_file`] for other files.
///
/// This function currently corresponds to
/// [`CreateSymbolicLinkW`][CreateSymbolicLinkW]. Note that this [may change in
/// the future][changes].
///
/// [CreateSymbolicLinkW]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createsymboliclinkw
/// [changes]: io#platform-specific-behavior
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::os::windows::fs;
///
/// fn main() -> IoResult<()> {
///     fs::symlink_dir("a", "b")?;
///     Ok(())
/// }
/// ```
///
/// # Limitations
///
/// Windows treats symlink creation as a [privileged action][symlink-security],
/// therefore this function is likely to fail unless the user makes changes to
/// their system to permit symlink creation. Users can try enabling Developer
/// Mode, granting the `SeCreateSymbolicLinkPrivilege` privilege, or running
/// the process as an administrator.
///
/// [symlink-security]: https://docs.microsoft.com/en-us/windows/security/threat-protection/security-policy-settings/create-symbolic-links
#[inline]
pub fn symlink_file(original: impl AsRef<Path>, link: impl AsRef<Path>) -> IoResult<()> {
    let _ = privilege_accquire(Privilege::SeCreateSymbolicLink)?;
    let r = CreateSymbolicLink(link.as_ref(), original.as_ref(), 0);
    let _ = privilege_release(Privilege::SeCreateSymbolicLink);
    Ok(r?)
}
/// Creates a junction point.
///
/// The `link` path will be a directory junction pointing to the original path.
/// If `link` is a relative path then it will be made absolute prior to creating
/// the junction point. The `original` path must be a directory or a link to a
/// directory, otherwise the junction point will be broken.
///
/// If either path is not a local file path then this will fail.
#[inline]
pub fn junction_point(original: impl AsRef<Path>, link: impl AsRef<Path>) -> IoResult<()> {
    Ok(CreateDirectoryJunction(link.as_ref(), original.as_ref())?)
}
