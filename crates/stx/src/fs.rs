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

//! Filesystem manipulation operations.
//!
//! This module contains basic methods to manipulate the contents of the local
//! filesystem. All methods in this module represent cross-platform filesystem
//! operations. Extra platform-specific functionality can be found in the
//! extension traits of `xrmt_stx::os::$platform`.

#![no_implicit_prelude]
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate alloc;
extern crate core;

extern crate xrmt_winapi;

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::clone::Clone;
use core::cmp::{Eq, PartialEq};
use core::convert::{AsRef, From, Into};
use core::default::Default;
use core::iter::Iterator;
use core::marker::Copy;
use core::option::Option::{self, None, Some};
use core::result::Result::{Err, Ok};
use core::slice::from_raw_parts;
use core::{matches, u64};

use xrmt_winapi::functions::{
    file_delete,
    file_name,
    file_rename,
    file_set_attrs,
    file_set_time,
    is_terminal,
    privilege_accquire,
    privilege_release,
    win32_file_flags_to_nt,
    CloseHandle,
    CopyFileEx,
    CreateDirectory,
    CreateHardLink,
    CreateSymbolicLink,
    DeleteFile,
    LockFile,
    MoveFileEx,
    NtCreateFile,
    NtFlushBuffersFile,
    NtQueryDirectoryFile,
    NtQueryInformationFile,
    NtReadFile,
    NtSetInformationFile,
    NtWriteFile,
    SetFileAttributes,
    SetFilePointerEx,
    UnlockFile,
};
use xrmt_winapi::info::is_min_windows_10;
use xrmt_winapi::structs::{DecodeUtf16, FileAllInformation, FileBasicInformation, FileIdBothDirInfo, FileStandardInformation, FileStatInformation, Handle, OwnedHandle, Privilege, SysTime};
use xrmt_winapi::{path_normalize, Win32Error};

use crate::ffi::OsString;
use crate::io::{BufReader, IoError, IoResult, IsTerminal, Read, Seek, SeekFrom, Write};
use crate::path::{Path, PathBuf};
use crate::time::SystemTime;

const READ: u16 = 0x1u16;
const WRITE: u16 = 0x2u16;
const APPEND: u16 = 0x4u16;
const TRUNCATE: u16 = 0x8u16;
const CREATE: u16 = 0x10u16;
const CREATE_NEW: u16 = 0x20u16;
const SYNCHRONOUS: u16 = 0x40u16;
const NO_SYMLINK: u16 = 0x80u16;
const EXCLUSIVE: u16 = 0x100u16;
const DELETE: u16 = 0x200u16;

#[doc(hidden)]
#[path = "extra/fs.rs"]
pub mod extra;

/// Iterator over the entries in a directory.
///
/// This iterator is returned from the [`read_dir`] function of this module and
/// will yield instances of <code>[`Result`]<[DirEntry]></code>. Through a
/// [`DirEntry`] information like the entry's path and possibly other metadata
/// can be learned.
///
/// The order in which this iterator returns entries is platform and filesystem
/// dependent.
///
/// # Errors
///
/// This [`Result`] will be an [`Err`] if there's some sort of intermittent
/// IO error during iteration.
///
/// [`Result`]: crate::IoResult
pub struct ReadDir {
    buf:   [u8; 4096],
    pos:   usize,
    path:  Arc<PathBuf>,
    owner: File,
    state: ReadState,
}
/// Metadata information about a file.
///
/// This structure is returned from the [`metadata`] or
/// [`symlink_metadata`] function or method and represents known
/// metadata about a file such as its permissions, size, modification
/// times, etc.
pub struct Metadata {
    attributes:       u32,
    creation_time:    SysTime,
    last_access_time: SysTime,
    last_write_time:  SysTime,
    change_time:      SysTime,
    file_size:        u64,
    reparse_tag:      u32,
    number_of_links:  u32,
    file_index:       u64,
    access:           u32,
}
/// Entries returned by the [`ReadDir`] iterator.
///
/// An instance of `DirEntry` represents an entry inside of a directory on the
/// filesystem. Each entry can be inspected via methods to learn about the full
/// path or possibly other metadata through per-platform extension traits.
///
/// # Platform-specific behavior
///
/// On Unix, the `DirEntry` struct contains an internal reference to the open
/// directory. Holding `DirEntry` objects will consume a file handle even
/// after the `ReadDir` iterator is dropped.
///
/// Note that this [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
pub struct DirEntry {
    name:  String,
    meta:  Metadata,
    root:  Arc<PathBuf>,
    owner: Handle,
}
/// A structure representing a type of file with accessors for each file type.
/// It is returned by [`Metadata::file_type`] method.
pub struct FileType {
    attrs:   u32,
    reparse: u32,
}
/// Representation of the various timestamps on a file.
pub struct FileTimes {
    created:  Option<SystemTime>,
    accessed: Option<SystemTime>,
    modified: Option<SystemTime>,
}
/// Representation of the various permissions on a file.
///
/// This module only currently provides one bit of information,
/// [`Permissions::readonly`], which is exposed on all currently supported
/// platforms. Unix-specific functionality, such as mode bits, is available
/// through the `PermissionsExt` trait.
pub struct Permissions {
    access:     u32,
    attributes: u32,
}
/// Options and flags which can be used to configure how a file is opened.
///
/// This builder exposes the ability to configure how a [`File`] is opened and
/// what operations are permitted on the open file. The [`File::open`] and
/// [`File::create`] methods are aliases for commonly used options using this
/// builder.
///
/// Generally speaking, when using `OpenOptions`, you'll first call
/// [`OpenOptions::new`], then chain calls to methods to set each option, then
/// call [`OpenOptions::open`], passing the path of the file you're trying to
/// open. This will give you a [`Result`] with a [`File`] inside that you
/// can further operate on.
///
/// [`Result`]: crate::IoResult
///
/// # Examples
///
/// Opening a file to read:
///
/// ```no_run
/// use xrmt_stx::fs::OpenOptions;
///
/// let file = OpenOptions::new().read(true).open("foo.txt");
/// ```
///
/// Opening a file for both reading and writing, as well as creating it if it
/// doesn't exist:
///
/// ```no_run
/// use xrmt_stx::fs::OpenOptions;
///
/// let file = OpenOptions::new()
///             .read(true)
///             .write(true)
///             .create(true)
///             .open("foo.txt");
/// ```
pub struct OpenOptions {
    opts:   u16,
    share:  u32,
    attrs:  u32,
    access: u32,
}
/// A builder used to create directories in various manners.
///
/// This builder also supports platform-specific options.
pub struct DirBuilder(bool);
/// An object providing access to an open file on the filesystem.
///
/// An instance of a `File` can be read and/or written depending on what options
/// it was opened with. Files also implement [`Seek`] to alter the logical
/// cursor that the file contains internally.
///
/// Files are automatically closed when they go out of scope.  Errors detected
/// on closing are ignored by the implementation of `Drop`.  Use the method
/// [`sync_all`] if these errors must be manually handled.
///
/// `File` does not buffer reads and writes. For efficiency, consider wrapping
/// the file in a [`BufReader`] or [`BufWriter`] when performing many small
/// [`read`] or [`write`] calls, unless unbuffered reads and writes are
/// required.
///
/// # Examples
///
/// Creates a new file and write bytes to it (you can also use [`write`]):
///
/// ```no_run
/// use xrmt_stx::fs::File;
/// use xrmt_stx::io::prelude::*;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let mut file = File::create("foo.txt")?;
///     file.write_all(b"Hello, world!")?;
///     Ok(())
/// }
/// ```
///
/// Reads the contents of a file into a [`String`] (you can also use [`read`]):
///
/// ```no_run
/// use xrmt_stx::fs::File;
/// use xrmt_stx::io::prelude::*;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let mut file = File::open("foo.txt")?;
///     let mut contents = String::new();
///     file.read_to_string(&mut contents)?;
///     assert_eq!(contents, "Hello, world!");
///     Ok(())
/// }
/// ```
///
/// Using a buffered [`Read`]er:
///
/// ```no_run
/// use xrmt_stx::fs::File;
/// use xrmt_stx::io::BufReader;
/// use xrmt_stx::io::prelude::*;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let file = File::open("foo.txt")?;
///     let mut buf_reader = BufReader::new(file);
///     let mut contents = String::new();
///     buf_reader.read_to_string(&mut contents)?;
///     assert_eq!(contents, "Hello, world!");
///     Ok(())
/// }
/// ```
///
/// Note that, although read and write methods require a `&mut File`, because
/// of the interfaces for [`Read`] and [`Write`], the holder of a `&File` can
/// still modify the file, either through methods that take `&File` or by
/// retrieving the underlying OS object and modifying the file that way.
/// Additionally, many operating systems allow concurrent modification of files
/// by different processes. Avoid assuming that holding a `&File` means that the
/// file will not change.
///
/// # Platform-specific behavior
///
/// On Windows, the implementation of [`Read`] and [`Write`] traits for `File`
/// perform synchronous I/O operations. Therefore the underlying file must not
/// have been opened for asynchronous I/O (e.g. by using
/// `FILE_FLAG_OVERLAPPED`).
///
/// [`BufWriter`]: crate::io::BufWriter
/// [`sync_all`]: File::sync_all
/// [`write`]: File::write
/// [`read`]: File::read
pub struct File(OwnedHandle);

enum ReadState {
    Empty,
    Error,
    Filled,
}

impl File {
    /// Returns a new OpenOptions object.
    ///
    /// This function returns a new OpenOptions object that you can use to
    /// open or create a file with specific options if `open()` or `create()`
    /// are not appropriate.
    ///
    /// It is equivalent to `OpenOptions::new()`, but allows you to write more
    /// readable code. Instead of
    /// `OpenOptions::new().append(true).open("example.log")`,
    /// you can write `File::options().append(true).open("example.log")`. This
    /// also avoids the need to import `OpenOptions`.
    ///
    /// See the [`OpenOptions::new`] function for more details.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::Write;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut f = File::options().append(true).open("example.log")?;
    ///     writeln!(&mut f, "new line")?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn options() -> OpenOptions {
        OpenOptions::new()
    }
    /// Attempts to open a file in read-only mode.
    ///
    /// See the [`OpenOptions::open`] method for more details.
    ///
    /// If you only need to read the entire file contents,
    /// consider [`xrmt_stx::fs::read()`][self::read] or
    /// [`xrmt_stx::fs::read_to_string()`][self::read_to_string] instead.
    ///
    /// # Errors
    ///
    /// This function will return an error if `path` does not already exist.
    /// Other errors may also be returned according to [`OpenOptions::open`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::Read;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut data = vec![];
    ///     f.read_to_end(&mut data)?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn open(path: impl AsRef<Path>) -> IoResult<File> {
        OpenOptions::new().read(true).open(path)
    }
    /// Opens a file in write-only mode.
    ///
    /// This function will create a file if it does not exist,
    /// and will truncate it if it does.
    ///
    /// Depending on the platform, this function may fail if the
    /// full directory path does not exist.
    /// See the [`OpenOptions::open`] function for more details.
    ///
    /// See also [`xrmt_stx::fs::write()`][self::write] for a simple function to
    /// create a file with some given data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::Write;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut f = File::create("foo.txt")?;
    ///     f.write_all(&1234_u32.to_be_bytes())?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn create(path: impl AsRef<Path>) -> IoResult<File> {
        OpenOptions::new().write(true).create(true).truncate(true).open(path)
    }
    /// Creates a new file in read-write mode; error if the file exists.
    ///
    /// This function will create a file if it does not exist, or return an
    /// error if it does. This way, if the call succeeds, the file returned
    /// is guaranteed to be new. If a file exists at the target location,
    /// creating a new file will fail with [`AlreadyExists`] or another
    /// error based on the situation. See [`OpenOptions::open`] for a
    /// non-exhaustive list of likely errors.
    ///
    /// This option is useful because it is atomic. Otherwise between checking
    /// whether a file exists and creating a new one, the file may have been
    /// created by another process (a TOCTOU race condition / attack).
    ///
    /// This can also be written using
    /// `File::options().read(true).write(true).create_new(true).open(...)`.
    ///
    /// [`AlreadyExists`]: crate::io::ErrorKind::AlreadyExists
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::Write;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut f = File::create_new("foo.txt")?;
    ///     f.write_all("Hello, world!".as_bytes())?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn create_new(path: impl AsRef<Path>) -> IoResult<File> {
        OpenOptions::new().read(true).write(true).create_new(true).open(path)
    }
    /// Attempts to open a file in read-only mode with buffering.
    ///
    /// See the [`OpenOptions::open`] method, the
    /// [`BufReader`][crate::io::BufReader] type, and the
    /// [`BufRead`][crate::io::BufRead] trait for more details.
    ///
    /// If you only need to read the entire file contents,
    /// consider [`xrmt_stx::fs::read()`][self::read] or
    /// [`xrmt_stx::fs::read_to_string()`][self::read_to_string] instead.
    ///
    /// # Errors
    ///
    /// This function will return an error if `path` does not already exist,
    /// or if memory allocation fails for the new buffer.
    /// Other errors may also be returned according to [`OpenOptions::open`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::BufRead;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut f = File::open_buffered("foo.txt")?;
    ///     assert!(f.capacity() > 0);
    ///     for (line, i) in f.lines().zip(1..) {
    ///         println!("{i:6}: {}", line?);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn open_buffered(path: impl AsRef<Path>) -> IoResult<BufReader<File>> {
        Ok(BufReader::with_capacity(0x2000, File::open(path)?))
    }
    /// Opens a file in write-only mode with buffering.
    ///
    /// This function will create a file if it does not exist,
    /// and will truncate it if it does.
    ///
    /// Depending on the platform, this function may fail if the
    /// full directory path does not exist.
    ///
    /// See the [`OpenOptions::open`] method and the
    /// [`BufWriter`][crate::io::BufWriter] type for more details.
    ///
    /// See also [`xrmt_stx::fs::write()`][self::write] for a simple function to
    /// create a file with some given data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::Write;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut f = File::create_buffered("foo.txt")?;
    ///     assert!(f.capacity() > 0);
    ///     for i in 0..100 {
    ///         writeln!(&mut f, "{i}")?;
    ///     }
    ///     f.flush()?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn create_buffered(path: impl AsRef<Path>) -> IoResult<BufReader<File>> {
        Ok(BufReader::with_capacity(0x2000, File::create(path)?))
    }

    /// Attempts to sync all OS-internal file content and metadata to disk.
    ///
    /// This function will attempt to ensure that all in-memory data reaches the
    /// filesystem before returning.
    ///
    /// This can be used to handle errors that would otherwise only be caught
    /// when the `File` is closed, as dropping a `File` will ignore all errors.
    /// Note, however, that `sync_all` is generally more expensive than closing
    /// a file by dropping it, because the latter is not required to block until
    /// the data has been written to the filesystem.
    ///
    /// If synchronizing the metadata is not required, use [`sync_data`]
    /// instead.
    ///
    /// [`sync_data`]: File::sync_data
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::prelude::*;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut f = File::create("foo.txt")?;
    ///     f.write_all(b"Hello, world!")?;
    ///
    ///     f.sync_all()?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn sync_all(&self) -> IoResult<()> {
        Ok(NtFlushBuffersFile(self)?)
    }
    /// This function is similar to [`sync_all`], except that it might not
    /// synchronize file metadata to the filesystem.
    ///
    /// This is intended for use cases that must synchronize content, but don't
    /// need the metadata on disk. The goal of this method is to reduce disk
    /// operations.
    ///
    /// Note that some platforms may simply implement this in terms of
    /// [`sync_all`].
    ///
    /// [`sync_all`]: File::sync_all
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::prelude::*;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut f = File::create("foo.txt")?;
    ///     f.write_all(b"Hello, world!")?;
    ///
    ///     f.sync_data()?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn sync_data(&self) -> IoResult<()> {
        Ok(NtFlushBuffersFile(self)?)
    }
    /// Creates a new `File` instance that shares the same underlying file
    /// handle as the existing `File` instance. Reads, writes, and seeks
    /// will affect both `File` instances simultaneously.
    ///
    /// # Examples
    ///
    /// Creates two handles for a file named `foo.txt`:
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///     let file_copy = file.try_clone()?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// Assuming there’s a file named `foo.txt` with contents `abcdef\n`, create
    /// two handles, seek one of them, and read the remaining bytes from the
    /// other handle:
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::SeekFrom;
    /// use xrmt_stx::io::prelude::*;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///     let mut file_copy = file.try_clone()?;
    ///
    ///     file.seek(SeekFrom::Start(3))?;
    ///
    ///     let mut contents = vec![];
    ///     file_copy.read_to_end(&mut contents)?;
    ///     assert_eq!(contents, b"def\n");
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn try_clone(&self) -> IoResult<File> {
        Ok(File(self.0.duplicate()?))
    }
    /// Queries metadata about the underlying file.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let metadata = f.metadata()?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn metadata(&self) -> IoResult<Metadata> {
        Metadata::file(self)
    }
    /// Truncates or extends the underlying file, updating the size of
    /// this file to become `size`.
    ///
    /// If the `size` is less than the current file's size, then the file will
    /// be shrunk. If it is greater than the current file's size, then the file
    /// will be extended to `size` and have all of the intermediate data filled
    /// in with 0s.
    ///
    /// The file's cursor isn't changed. In particular, if the cursor was at the
    /// end and the file is shrunk using this operation, the cursor will now be
    /// past the end.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file is not opened for
    /// writing.
    /// Also, [`InvalidInput`](crate::io::ErrorKind::InvalidInput)
    /// will be returned if the desired length would cause an overflow due to
    /// the implementation specifics.
    ///
    /// [`InvalidInput`]: ErrorKind::InvalidInput
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut f = File::create("foo.txt")?;
    ///     f.set_len(10)?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// Note that this method alters the content of the underlying file, even
    /// though it takes `&self` rather than `&mut self`.
    #[inline]
    pub fn set_len(&self, size: u64) -> IoResult<()> {
        // 0x14 - FileEndOfFileInformation
        NtSetInformationFile(self, 0x14, &size, 8)?;
        Ok(())
    }
    /// Changes the timestamps of the underlying file.
    ///
    /// # Platform-specific behavior
    ///
    /// This function currently corresponds to the `futimens` function on Unix
    /// (falling back to `futimes` on macOS before 10.13) and the
    /// `SetFileTime` function on Windows. Note that this [may change in the
    /// future][changes].
    ///
    /// [changes]: crate::io#platform-specific-behavior
    ///
    /// # Errors
    ///
    /// This function will return an error if the user lacks permission to
    /// change timestamps on the underlying file. It may also return an
    /// error in other os-specific unspecified cases.
    ///
    /// This function may return an error if the operating system lacks support
    /// to change one or more of the timestamps set in the `FileTimes`
    /// structure.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     use xrmt_stx::fs::{self, File, FileTimes};
    ///
    ///     let src = fs::metadata("src")?;
    ///     let dest = File::options().write(true).open("dest")?;
    ///     let times = FileTimes::new()
    ///         .set_accessed(src.accessed()?)
    ///         .set_modified(src.modified()?);
    ///     dest.set_times(times)?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn set_times(&self, times: FileTimes) -> IoResult<()> {
        Ok(file_set_time(
            self,
            times.created.map(|v| v.into()),
            times.modified.map(|v| v.into()),
            times.accessed.map(|v| v.into()),
        )?)
    }
    /// Changes the modification time of the underlying file.
    ///
    /// This is an alias for `set_times(FileTimes::new().set_modified(time))`.
    #[inline]
    pub fn set_modified(&self, time: SystemTime) -> IoResult<()> {
        Ok(file_set_time(self, None, Some(time.into()), None)?)
    }
    /// Changes the permissions on the underlying file.
    ///
    /// # Platform-specific behavior
    ///
    /// This function currently corresponds to the `fchmod` function on Unix and
    /// the `SetFileInformationByHandle` function on Windows. Note that, this
    /// [may change in the future][changes].
    ///
    /// [changes]: crate::io#platform-specific-behavior
    ///
    /// # Errors
    ///
    /// This function will return an error if the user lacks permission change
    /// attributes on the underlying file. It may also return an error in other
    /// os-specific unspecified cases.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     use xrmt_stx::fs::File;
    ///
    ///     let file = File::open("foo.txt")?;
    ///     let mut perms = file.metadata()?.permissions();
    ///     perms.set_readonly(true);
    ///     file.set_permissions(perms)?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// Note that this method alters the permissions of the underlying file,
    /// even though it takes `&self` rather than `&mut self`.
    #[inline]
    pub fn set_permissions(&self, perm: Permissions) -> IoResult<()> {
        self.set_attribute(perm.attributes)
    }

    /// Acquire an exclusive lock on the file. Blocks until the lock can be
    /// acquired.
    ///
    /// This acquires an exclusive lock; no other file handle to this file may
    /// acquire another lock.
    ///
    /// This lock may be advisory or mandatory. This lock is meant to interact
    /// with [`lock`], [`try_lock`], [`lock_shared`], [`try_lock_shared`],
    /// and [`unlock`]. Its interactions with other methods, such as
    /// [`read`] and [`write`] are platform specific, and it may or may not
    /// cause non-lockholders to block.
    ///
    /// If this file handle/descriptor, or a clone of it, already holds an lock
    /// the exact behavior is unspecified and platform dependent, including
    /// the possibility that it will deadlock. However, if this method
    /// returns, then an exclusive lock is held.
    ///
    /// If the file not open for writing, it is unspecified whether this
    /// function returns an error.
    ///
    /// The lock will be released when this file (along with any other file
    /// descriptors/handles duplicated or inherited from it) is closed, or
    /// if the [`unlock`] method is called.
    ///
    /// # Platform-specific behavior
    ///
    /// This function currently corresponds to the `flock` function on Unix with
    /// the `LOCK_EX` flag, and the `LockFileEx` function on Windows with
    /// the `LOCKFILE_EXCLUSIVE_LOCK` flag. Note that, this [may change in
    /// the future][changes].
    ///
    /// On Windows, locking a file will fail if the file is opened only for
    /// append. To lock a file, open it with one of `.read(true)`,
    /// `.read(true).append(true)`, or `.write(true)`.
    ///
    /// [changes]: crate::io#platform-specific-behavior
    ///
    /// [`lock`]: File::lock
    /// [`lock_shared`]: File::lock_shared
    /// [`try_lock`]: File::try_lock
    /// [`try_lock_shared`]: File::try_lock_shared
    /// [`unlock`]: File::unlock
    /// [`read`]: Read::read
    /// [`write`]: Write::write
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let f = File::create("foo.txt")?;
    ///     f.lock()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn lock(&self) -> IoResult<()> {
        // 0x2 - LOCKFILE_EXCLUSIVE_LOCK
        Ok(LockFile(self, None, 0x2, u64::MAX, None)?)
    }
    /// Acquire a shared (non-exclusive) lock on the file. Blocks until the lock
    /// can be acquired.
    ///
    /// This acquires a shared lock; more than one file handle may hold a shared
    /// lock, but none may hold an exclusive lock at the same time.
    ///
    /// This lock may be advisory or mandatory. This lock is meant to interact
    /// with [`lock`], [`try_lock`], [`lock_shared`], [`try_lock_shared`],
    /// and [`unlock`]. Its interactions with other methods, such as
    /// [`read`] and [`write`] are platform specific, and it may or may not
    /// cause non-lockholders to block.
    ///
    /// If this file handle/descriptor, or a clone of it, already holds an lock,
    /// the exact behavior is unspecified and platform dependent, including
    /// the possibility that it will deadlock. However, if this method
    /// returns, then a shared lock is held.
    ///
    /// The lock will be released when this file (along with any other file
    /// descriptors/handles duplicated or inherited from it) is closed, or
    /// if the [`unlock`] method is called.
    ///
    /// # Platform-specific behavior
    ///
    /// This function currently corresponds to the `flock` function on Unix with
    /// the `LOCK_SH` flag, and the `LockFileEx` function on Windows. Note
    /// that, this [may change in the future][changes].
    ///
    /// On Windows, locking a file will fail if the file is opened only for
    /// append. To lock a file, open it with one of `.read(true)`,
    /// `.read(true).append(true)`, or `.write(true)`.
    ///
    /// [changes]: crate::io#platform-specific-behavior
    ///
    /// [`lock`]: File::lock
    /// [`lock_shared`]: File::lock_shared
    /// [`try_lock`]: File::try_lock
    /// [`try_lock_shared`]: File::try_lock_shared
    /// [`unlock`]: File::unlock
    /// [`read`]: Read::read
    /// [`write`]: Write::write
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let f = File::open("foo.txt")?;
    ///     f.lock_shared()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn lock_shared(&self) -> IoResult<()> {
        Ok(LockFile(self, None, 0, u64::MAX, None)?)
    }
    /// Try to acquire an exclusive lock on the file.
    ///
    /// Returns `Ok(false)` if a different lock is already held on this file
    /// (via another handle/descriptor).
    ///
    /// This acquires an exclusive lock; no other file handle to this file may
    /// acquire another lock.
    ///
    /// This lock may be advisory or mandatory. This lock is meant to interact
    /// with [`lock`], [`try_lock`], [`lock_shared`], [`try_lock_shared`],
    /// and [`unlock`]. Its interactions with other methods, such as
    /// [`read`] and [`write`] are platform specific, and it may or may not
    /// cause non-lockholders to block.
    ///
    /// If this file handle/descriptor, or a clone of it, already holds an lock,
    /// the exact behavior is unspecified and platform dependent, including
    /// the possibility that it will deadlock. However, if this method
    /// returns `Ok(true)`, then it has acquired an exclusive lock.
    ///
    /// If the file not open for writing, it is unspecified whether this
    /// function returns an error.
    ///
    /// The lock will be released when this file (along with any other file
    /// descriptors/handles duplicated or inherited from it) is closed, or
    /// if the [`unlock`] method is called.
    ///
    /// # Platform-specific behavior
    ///
    /// This function currently corresponds to the `flock` function on Unix with
    /// the `LOCK_EX` and `LOCK_NB` flags, and the `LockFileEx` function on
    /// Windows with the `LOCKFILE_EXCLUSIVE_LOCK`
    /// and `LOCKFILE_FAIL_IMMEDIATELY` flags. Note that, this
    /// [may change in the future][changes].
    ///
    /// On Windows, locking a file will fail if the file is opened only for
    /// append. To lock a file, open it with one of `.read(true)`,
    /// `.read(true).append(true)`, or `.write(true)`.
    ///
    /// [changes]: crate::io#platform-specific-behavior
    ///
    /// [`lock`]: File::lock
    /// [`lock_shared`]: File::lock_shared
    /// [`try_lock`]: File::try_lock
    /// [`try_lock_shared`]: File::try_lock_shared
    /// [`unlock`]: File::unlock
    /// [`read`]: Read::read
    /// [`write`]: Write::write
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let f = File::create("foo.txt")?;
    ///     f.try_lock()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn try_lock(&self) -> IoResult<bool> {
        // 0x3 - LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY
        match LockFile(self, None, 0x3, u64::MAX, None) {
            Ok(_) => Ok(true),
            Err(Win32Error::IoPending) => Ok(false),
            Err(e) => Err(IoError::from(e)),
        }
    }
    /// Try to acquire a shared (non-exclusive) lock on the file.
    ///
    /// Returns `Ok(false)` if an exclusive lock is already held on this file
    /// (via another handle/descriptor).
    ///
    /// This acquires a shared lock; more than one file handle may hold a shared
    /// lock, but none may hold an exclusive lock at the same time.
    ///
    /// This lock may be advisory or mandatory. This lock is meant to interact
    /// with [`lock`], [`try_lock`], [`lock_shared`], [`try_lock_shared`],
    /// and [`unlock`]. Its interactions with other methods, such as
    /// [`read`] and [`write`] are platform specific, and it may or may not
    /// cause non-lockholders to block.
    ///
    /// If this file handle, or a clone of it, already holds an lock, the exact
    /// behavior is unspecified and platform dependent, including the
    /// possibility that it will deadlock. However, if this method returns
    /// `Ok(true)`, then it has acquired a shared lock.
    ///
    /// The lock will be released when this file (along with any other file
    /// descriptors/handles duplicated or inherited from it) is closed, or
    /// if the [`unlock`] method is called.
    ///
    /// # Platform-specific behavior
    ///
    /// This function currently corresponds to the `flock` function on Unix with
    /// the `LOCK_SH` and `LOCK_NB` flags, and the `LockFileEx` function on
    /// Windows with the `LOCKFILE_FAIL_IMMEDIATELY` flag. Note that, this
    /// [may change in the future][changes].
    ///
    /// On Windows, locking a file will fail if the file is opened only for
    /// append. To lock a file, open it with one of `.read(true)`,
    /// `.read(true).append(true)`, or `.write(true)`.
    ///
    /// [changes]: crate::io#platform-specific-behavior
    ///
    /// [`lock`]: File::lock
    /// [`lock_shared`]: File::lock_shared
    /// [`try_lock`]: File::try_lock
    /// [`try_lock_shared`]: File::try_lock_shared
    /// [`unlock`]: File::unlock
    /// [`read`]: Read::read
    /// [`write`]: Write::write
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let f = File::open("foo.txt")?;
    ///     f.try_lock_shared()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn try_lock_shared(&self) -> IoResult<bool> {
        // 0x1 - LOCKFILE_FAIL_IMMEDIATELY
        match LockFile(self, None, 0x1, u64::MAX, None) {
            Ok(_) => Ok(true),
            Err(Win32Error::IoPending) => Ok(false),
            Err(e) => Err(IoError::from(e)),
        }
    }
    /// Release all locks on the file.
    ///
    /// All locks are released when the file (along with any other file
    /// descriptors/handles duplicated or inherited from it) is closed. This
    /// method allows releasing locks without closing the file.
    ///
    /// If no lock is currently held via this file descriptor/handle, this
    /// method may return an error, or may return successfully without
    /// taking any action.
    ///
    /// # Platform-specific behavior
    ///
    /// This function currently corresponds to the `flock` function on Unix with
    /// the `LOCK_UN` flag, and the `UnlockFile` function on Windows. Note
    /// that, this [may change in the future][changes].
    ///
    /// On Windows, locking a file will fail if the file is opened only for
    /// append. To lock a file, open it with one of `.read(true)`,
    /// `.read(true).append(true)`, or `.write(true)`.
    ///
    /// [changes]: crate::io#platform-specific-behavior
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let f = File::open("foo.txt")?;
    ///     f.lock()?;
    ///     f.unlock()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn unlock(&self) -> IoResult<()> {
        // Unlock the file twice to make sure any shared AND exclusive locks
        // are removed.
        Ok(UnlockFile(self, None, u64::MAX, None).and_then(|_| UnlockFile(self, None, u64::MAX, None))?)
    }

    #[inline]
    fn is_file(&self) -> bool {
        let mut a = [0u32, 0u32];
        // Always size 8.
        //
        // 0x23 - FileAttributeTagInformation
        // 0x10 - FILE_ATTRIBUTE_DIRECTORY
        NtQueryInformationFile(self, 0x23, &mut a, 0x8).map_or(false, |_| a[0] & 0x10 == 0)
    }
    #[inline]
    fn delete(&mut self) -> IoResult<()> {
        // 0xD - FileDispositionInformation
        NtSetInformationFile::<u32>(&self, 0xD, &1, 4)?;
        CloseHandle(unsafe { *self.0.take() })?;
        Ok(())
    }
    #[inline]
    fn read_dir(self) -> IoResult<ReadDir> {
        ReadDir::new(self)
    }
    #[inline]
    fn set_attribute(&self, attrs: u32) -> IoResult<()> {
        Ok(file_set_attrs(self, attrs)?)
    }
}
impl ReadDir {
    #[inline]
    fn new(f: File) -> IoResult<ReadDir> {
        let mut i = ReadDir {
            buf:   [0u8; 4096],
            pos:   0usize,
            path:  Arc::new(file_name(&f)?.into()),
            owner: f,
            state: ReadState::Error,
        };
        if NtQueryDirectoryFile(&i.owner, &mut i.buf, 0x25, false, true, None)? > 0 {
            i.state = ReadState::Filled;
        }
        Ok(i)
    }

    #[inline]
    fn refill(&mut self) {
        if matches!(self.state, ReadState::Filled) {
            return;
        }
        if NtQueryDirectoryFile(&self.owner, &mut self.buf, 0x25, false, false, None).map_or(false, |v| v > 0) {
            (self.pos, self.state) = (0, ReadState::Filled);
        } else {
            self.state = ReadState::Error;
        }
    }
    fn pull(&mut self) -> Option<DirEntry> {
        let e = unsafe { (self.buf.as_ptr().add(self.pos) as *const FileIdBothDirInfo).as_ref()? };
        self.pos += e.next_entry as usize;
        if e.next_entry == 0 || e.name_length == 0 {
            self.state = ReadState::Empty;
        }
        let b = unsafe { from_raw_parts(e.file_name.as_ptr(), e.name_length as usize / 2) };
        return match e.name_length {
            0 => return None,
            2 if unsafe { *b.get_unchecked(0) == 0x2E } => return None,
            4 if unsafe { *b.get_unchecked(0) == 0x2E && *b.get_unchecked(1) == 0x2E } => return None,
            _ => Some(DirEntry::new(
                *self.owner.0,
                self.path.clone(),
                e,
                b.decode_utf16(),
            )),
        };
    }
}
impl Metadata {
    /// Returns the size of the file, in bytes, this metadata is for.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     assert_eq!(0, metadata.len());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn len(&self) -> u64 {
        self.file_size
    }
    /// Returns `true` if this metadata is for a directory. The
    /// result is mutually exclusive to the result of
    /// [`Metadata::is_file`], and will be false for symlink metadata
    /// obtained from [`symlink_metadata`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     use xrmt_stx::fs;
    ///
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     assert!(!metadata.is_dir());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn is_dir(&self) -> bool {
        !self.is_symlink() && self.attributes & 0x10 != 0
    }
    /// Returns `true` if this metadata is for a regular file. The
    /// result is mutually exclusive to the result of
    /// [`Metadata::is_dir`], and will be false for symlink metadata
    /// obtained from [`symlink_metadata`].
    ///
    /// When the goal is simply to read from (or write to) the source, the most
    /// reliable way to test the source can be read (or written to) is to open
    /// it. Only using `is_file` can break workflows like `diff <( prog_a )` on
    /// a Unix-like system for example. See [`File::open`] or
    /// [`OpenOptions::open`] for more information.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     assert!(metadata.is_file());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn is_file(&self) -> bool {
        !self.is_symlink() && self.attributes & 0x10 == 0
    }
    /// Returns `true` if this metadata is for a symbolic link.
    ///
    /// # Examples
    /// ```no_run
    /// use xrmt_stx::fs;
    /// use xrmt_stx::path::Path;
    /// use xrmt_stx::os::unix::fs::symlink;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let link_path = Path::new("link");
    ///     symlink("/origin_does_not_exist/", link_path)?;
    ///
    ///     let metadata = fs::symlink_metadata(link_path)?;
    ///
    ///     assert!(metadata.is_symlink());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn is_symlink(&self) -> bool {
        // 0x00000400 - FILE_ATTRIBUTE_REPARSE_POINT
        // 0x20000000 - SYMLINK_MASK
        self.attributes & 0x400 != 0 && self.reparse_tag & 0x20000000 != 0
    }
    /// Returns the file type for this metadata.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     use xrmt_stx::fs;
    ///
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     println!("{:?}", metadata.file_type());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn file_type(&self) -> FileType {
        FileType {
            attrs:   self.attributes,
            reparse: self.reparse_tag,
        }
    }
    /// Returns the permissions of the file this metadata is for.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     assert!(!metadata.permissions().readonly());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn permissions(&self) -> Permissions {
        Permissions {
            access:     self.access,
            attributes: self.attributes,
        }
    }
    /// Returns the creation time listed in this metadata.
    ///
    /// The returned value corresponds to the `btime` field of `statx` on
    /// Linux kernel starting from to 4.11, the `birthtime` field of `stat` on
    /// other Unix platforms, and the `ftCreationTime` field on Windows
    /// platforms.
    ///
    /// # Errors
    ///
    /// This field might not be available on all platforms, and will return an
    /// `Err` on platforms or filesystems where it is not available.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     if let Ok(time) = metadata.created() {
    ///         println!("{time:?}");
    ///     } else {
    ///         println!("Not supported on this platform or filesystem");
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn created(&self) -> IoResult<SystemTime> {
        Ok(self.creation_time.as_time().into())
    }
    /// Returns the last access time of this metadata.
    ///
    /// The returned value corresponds to the `atime` field of `stat` on Unix
    /// platforms and the `ftLastAccessTime` field on Windows platforms.
    ///
    /// Note that not all platforms will keep this field update in a file's
    /// metadata, for example Windows has an option to disable updating this
    /// time when files are accessed and Linux similarly has `noatime`.
    ///
    /// # Errors
    ///
    /// This field might not be available on all platforms, and will return an
    /// `Err` on platforms where it is not available.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     if let Ok(time) = metadata.accessed() {
    ///         println!("{time:?}");
    ///     } else {
    ///         println!("Not supported on this platform");
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn accessed(&self) -> IoResult<SystemTime> {
        Ok(self.last_access_time.as_time().into())
    }
    /// Returns the last modification time listed in this metadata.
    ///
    /// The returned value corresponds to the `mtime` field of `stat` on Unix
    /// platforms and the `ftLastWriteTime` field on Windows platforms.
    ///
    /// # Errors
    ///
    /// This field might not be available on all platforms, and will return an
    /// `Err` on platforms where it is not available.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     if let Ok(time) = metadata.modified() {
    ///         println!("{time:?}");
    ///     } else {
    ///         println!("Not supported on this platform");
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn modified(&self) -> IoResult<SystemTime> {
        Ok(self.last_write_time.as_time().into())
    }

    fn file(f: &File) -> IoResult<Metadata> {
        // We're gonna use a tiered approach to this. There's some ways to get this
        // info quickly (FileStatInformation) but we have fallbacks for doing it
        // into multiple calls.
        //
        // - FileStatInformation (W10+, but fails for UNC/Shares)
        //
        // (Both below call FileAttributeTagInformation as the All struct does not
        // contain it.)
        // - FileAllInformation (Will fail if the name is +300 chars since we only
        //   allocate 300).
        // - FileAttributeTagInformation
        //
        // - or -
        //
        // - FileBasicInformation
        // - FileStandardInformation
        // - FileInternalInformation
        // - FileAccessInformation
        // - FileAttributeTagInformation
        //
        // StatInfo is the quickest way to get all this info but it's Win10+ only.
        if is_min_windows_10() {
            let mut a = FileStatInformation::default();
            // Stat info is Win10+ only! This also fails on mapped drives, so
            // ignore it's errors  and fallback if it fails.
            //
            // 0x44 - FileStatInformation
            if NtQueryInformationFile(f, 0x44, &mut a, 0x48).is_ok() {
                return Ok(a.into());
            }
        }
        let mut a = FileAllInformation::default();
        // Always size 712, ignore errors and fallback if it fails.
        //
        // 0x12 - FileAllInformation
        if NtQueryInformationFile(f, 0x12, &mut a, 0x2C8).is_ok() {
            // Don't lookup data if it's not a Reparse Point
            //
            // 0x400 - FILE_ATTRIBUTE_REPARSE_POINT
            let r = if a.basic.attributes & 0x400 != 0 {
                let mut v = [0u32, 0u32];
                // Always size 8.
                //
                // 0x23 - FileAttributeTagInformation
                NtQueryInformationFile(f, 0x23, &mut v, 0x8)?;
                v[1]
            } else {
                0
            };
            return Ok(Metadata {
                access:           a.access,
                file_size:        a.standard.end_of_file,
                attributes:       a.basic.attributes,
                file_index:       a.file_id,
                reparse_tag:      r,
                change_time:      a.basic.change_time,
                creation_time:    a.basic.creation_time,
                number_of_links:  a.standard.number_of_links,
                last_write_time:  a.basic.last_write_time,
                last_access_time: a.basic.last_access_time,
            });
        }
        let (mut i, mut a) = (0u64, 0u32);
        let mut b = FileBasicInformation::default();
        let mut s = FileStandardInformation::default();
        // Always size 40.
        //
        // 0x4 - FileBasicInformation
        NtQueryInformationFile(f, 0x4, &mut b, 0x28)?;
        // Always size 32.
        //
        // 0x5 - FileStandardInformation
        NtQueryInformationFile(f, 0x5, &mut s, 0x20)?;
        // Always size 8.
        //
        // 0x6 - FileInternalInformation
        NtQueryInformationFile(f, 0x6, &mut i, 0x8)?;
        // Always size 4.
        //
        // 0x8 - FileAccessInformation
        NtQueryInformationFile(f, 0x8, &mut a, 0x4)?;
        // Don't lookup data if it's not a Reparse Point
        //
        // 0x400 - FILE_ATTRIBUTE_REPARSE_POINT
        let r = if b.attributes & 0x400 != 0 {
            let mut v = [0u32, 0u32];
            // Always size 8.
            //
            // 0x23 - FileAttributeTagInformation
            NtQueryInformationFile(f, 0x23, &mut v, 0x8)?;
            v[1]
        } else {
            0
        };
        Ok(Metadata {
            access:           a,
            attributes:       b.attributes,
            file_size:        s.allocation_size,
            file_index:       i,
            reparse_tag:      r,
            change_time:      b.change_time,
            creation_time:    b.creation_time,
            last_access_time: b.last_access_time,
            last_write_time:  b.last_write_time,
            number_of_links:  s.number_of_links,
        })
    }
}
impl DirEntry {
    /// Returns the full path to the file that this entry represents.
    ///
    /// The full path is created by joining the original path to `read_dir`
    /// with the filename of this entry.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     for entry in fs::read_dir(".")? {
    ///         let dir = entry?;
    ///         println!("{:?}", dir.path());
    ///     }
    ///     Ok(())
    /// }
    /// ```
    ///
    /// This prints output like:
    ///
    /// ```text
    /// "./whatever.txt"
    /// "./foo.html"
    /// "./hello_world.rs"
    /// ```
    ///
    /// The exact text, of course, depends on what files you have in `.`.
    #[inline]
    pub fn path(&self) -> PathBuf {
        self.root.join(&self.name)
    }
    /// Returns the file name of this directory entry without any
    /// leading path component(s).
    ///
    /// As an example,
    /// the output of the function will result in "foo" for all the following
    /// paths:
    /// - "./foo"
    /// - "/the/foo"
    /// - "../../foo"
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::fs;
    ///
    /// if let Ok(entries) = fs::read_dir(".") {
    ///     for entry in entries {
    ///         if let Ok(entry) = entry {
    ///             // Here, `entry` is a `DirEntry`.
    ///             println!("{:?}", entry.file_name());
    ///         }
    ///     }
    /// }
    /// ```
    #[inline]
    pub fn file_name(&self) -> OsString {
        OsString::from(&self.name)
    }
    /// Returns the metadata for the file that this entry points at.
    ///
    /// This function will not traverse symlinks if this entry points at a
    /// symlink. To traverse symlinks use [`metadata`] or
    /// [`File::metadata`].
    ///
    /// # Platform-specific behavior
    ///
    /// On Windows this function is cheap to call (no extra system calls
    /// needed), but on Unix platforms this function is the equivalent of
    /// calling `symlink_metadata` on the path.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::fs;
    ///
    /// if let Ok(entries) = fs::read_dir(".") {
    ///     for entry in entries {
    ///         if let Ok(entry) = entry {
    ///             // Here, `entry` is a `DirEntry`.
    ///             if let Ok(metadata) = entry.metadata() {
    ///                 // Now let's show our entry's permissions!
    ///                 println!("{:?}: {:?}", entry.path(), metadata.permissions());
    ///             } else {
    ///                 println!("Couldn't get metadata for {:?}", entry.path());
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    #[inline]
    pub fn metadata(&self) -> IoResult<Metadata> {
        Ok(self.meta.clone())
    }
    /// Returns the file type for the file that this entry points at.
    ///
    /// This function will not traverse symlinks if this entry points at a
    /// symlink.
    ///
    /// # Platform-specific behavior
    ///
    /// On Windows and most Unix platforms this function is free (no extra
    /// system calls needed), but some Unix platforms may require the equivalent
    /// call to `symlink_metadata` to learn about the target file type.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::fs;
    ///
    /// if let Ok(entries) = fs::read_dir(".") {
    ///     for entry in entries {
    ///         if let Ok(entry) = entry {
    ///             // Here, `entry` is a `DirEntry`.
    ///             if let Ok(file_type) = entry.file_type() {
    ///                 // Now let's show our entry's file type!
    ///                 println!("{:?}: {:?}", entry.path(), file_type);
    ///             } else {
    ///                 println!("Couldn't get file type for {:?}", entry.path());
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    #[inline]
    pub fn file_type(&self) -> IoResult<FileType> {
        Ok(self.meta.file_type())
    }

    #[inline]
    fn new(h: Handle, r: Arc<PathBuf>, v: &FileIdBothDirInfo, n: String) -> DirEntry {
        DirEntry {
            root:  r,
            name:  n,
            meta:  Metadata::from(v),
            owner: h,
        }
    }
}
impl FileType {
    /// Tests whether this file type represents a directory. The
    /// result is mutually exclusive to the results of
    /// [`is_file`] and [`is_symlink`]; only zero or one of these
    /// tests may pass.
    ///
    /// [`is_file`]: FileType::is_file
    /// [`is_symlink`]: FileType::is_symlink
    ///
    /// # Examples
    ///
    /// ```no_run
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     use xrmt_stx::fs;
    ///
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let file_type = metadata.file_type();
    ///
    ///     assert_eq!(file_type.is_dir(), false);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn is_dir(&self) -> bool {
        self.attrs & 0x10 != 0
    }
    /// Tests whether this file type represents a regular file.
    /// The result is mutually exclusive to the results of
    /// [`is_dir`] and [`is_symlink`]; only zero or one of these
    /// tests may pass.
    ///
    /// When the goal is simply to read from (or write to) the source, the most
    /// reliable way to test the source can be read (or written to) is to open
    /// it. Only using `is_file` can break workflows like `diff <( prog_a )` on
    /// a Unix-like system for example. See [`File::open`] or
    /// [`OpenOptions::open`] for more information.
    ///
    /// [`is_dir`]: FileType::is_dir
    /// [`is_symlink`]: FileType::is_symlink
    ///
    /// # Examples
    ///
    /// ```no_run
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     use xrmt_stx::fs;
    ///
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let file_type = metadata.file_type();
    ///
    ///     assert_eq!(file_type.is_file(), true);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn is_file(&self) -> bool {
        self.attrs & 0x10 == 0
    }
    /// Tests whether this file type represents a symbolic link.
    /// The result is mutually exclusive to the results of
    /// [`is_dir`] and [`is_file`]; only zero or one of these
    /// tests may pass.
    ///
    /// The underlying [`Metadata`] struct needs to be retrieved
    /// with the [`symlink_metadata`] function and not the
    /// [`metadata`] function. The [`metadata`] function
    /// follows symbolic links, so [`is_symlink`] would always
    /// return `false` for the target file.
    ///
    /// [`is_dir`]: FileType::is_dir
    /// [`is_file`]: FileType::is_file
    /// [`is_symlink`]: FileType::is_symlink
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let metadata = fs::symlink_metadata("foo.txt")?;
    ///     let file_type = metadata.file_type();
    ///
    ///     assert_eq!(file_type.is_symlink(), false);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn is_symlink(&self) -> bool {
        self.attrs & 0x400 != 0 && self.reparse & 0x20000000 != 0
    }
}
impl FileTimes {
    /// Creates a new `FileTimes` with no times set.
    ///
    /// Using the resulting `FileTimes` in [`File::set_times`] will not modify
    /// any timestamps.
    #[inline]
    pub fn new() -> FileTimes {
        FileTimes {
            created:  None,
            accessed: None,
            modified: None,
        }
    }

    /// Set the last access time of a file.
    #[inline]
    pub fn set_accessed(self, t: SystemTime) -> FileTimes {
        FileTimes {
            created:  self.created,
            accessed: Some(t),
            modified: self.modified,
        }
    }
    /// Set the last modified time of a file.
    #[inline]
    pub fn set_modified(self, t: SystemTime) -> FileTimes {
        FileTimes {
            created:  self.created,
            accessed: self.accessed,
            modified: Some(t),
        }
    }
}
impl DirBuilder {
    /// Creates a new set of options with default mode/security settings for all
    /// platforms and also non-recursive.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::fs::DirBuilder;
    ///
    /// let builder = DirBuilder::new();
    /// ```
    #[inline]
    pub fn new() -> DirBuilder {
        DirBuilder(false)
    }

    /// Creates the specified directory with the options configured in this
    /// builder.
    ///
    /// It is considered an error if the directory already exists unless
    /// recursive mode is enabled.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::{self, DirBuilder};
    ///
    /// let path = "/tmp/foo/bar/baz";
    /// DirBuilder::new()
    ///     .recursive(true)
    ///     .create(path).unwrap();
    ///
    /// assert!(fs::metadata(path).unwrap().is_dir());
    /// ```
    #[inline]
    pub fn create(&self, path: impl AsRef<Path>) -> IoResult<()> {
        Ok(CreateDirectory(path.as_ref(), self.0)?)
    }
    /// Indicates that directories should be created recursively, creating all
    /// parent directories. Parents that do not exist are created with the same
    /// security and permissions settings.
    ///
    /// This option defaults to `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::fs::DirBuilder;
    ///
    /// let mut builder = DirBuilder::new();
    /// builder.recursive(true);
    /// ```
    #[inline]
    pub fn recursive(&mut self, recursive: bool) -> &mut DirBuilder {
        self.0 = recursive;
        self
    }
}
impl OpenOptions {
    /// Creates a blank new set of options ready for configuration.
    ///
    /// All options are initially set to `false`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::OpenOptions;
    ///
    /// let mut options = OpenOptions::new();
    /// let file = options.read(true).open("foo.txt");
    /// ```
    #[inline]
    pub fn new() -> OpenOptions {
        OpenOptions {
            opts:   SYNCHRONOUS,
            share:  0u32,
            attrs:  0u32,
            access: 0u32,
        }
    }

    /// Sets the option for read access.
    ///
    /// This option, when true, will indicate that the file should be
    /// `read`-able if opened.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().read(true).open("foo.txt");
    /// ```
    #[inline]
    pub fn read(&mut self, read: bool) -> &mut OpenOptions {
        if read {
            self.opts |= READ;
        } else {
            self.opts &= !READ;
        }
        self
    }
    /// Sets the option for write access.
    ///
    /// This option, when true, will indicate that the file should be
    /// `write`-able if opened.
    ///
    /// If the file already exists, any write calls on it will overwrite its
    /// contents, without truncating it.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().write(true).open("foo.txt");
    /// ```
    #[inline]
    pub fn write(&mut self, write: bool) -> &mut OpenOptions {
        if write {
            self.opts |= WRITE;
        } else {
            self.opts &= !WRITE;
        }
        self
    }
    /// Sets the option for the append mode.
    ///
    /// This option, when true, means that writes will append to a file instead
    /// of overwriting previous contents.
    /// Note that setting `.write(true).append(true)` has the same effect as
    /// setting only `.append(true)`.
    ///
    /// Append mode guarantees that writes will be positioned at the current end
    /// of file, even when there are other processes or threads appending to
    /// the same file. This is unlike
    /// <code>[seek]\([SeekFrom]::[End]\(0))</code> followed by
    /// `write()`, which has a race between seeking and writing during which
    /// another writer can write, with our `write()` overwriting their data.
    ///
    /// Keep in mind that this does not necessarily guarantee that data appended
    /// by different processes or threads does not interleave. The amount of
    /// data accepted a single `write()` call depends on the operating
    /// system and file system. A successful `write()` is allowed to write
    /// only part of the given data, so even if you're careful to provide
    /// the whole message in a single call to `write()`, there
    /// is no guarantee that it will be written out in full. If you rely on the
    /// filesystem accepting the message in a single write, make sure that
    /// all data that belongs together is written in one operation. This can
    /// be done by concatenating strings before passing them to [`write()`].
    ///
    /// If a file is opened with both read and append access, beware that after
    /// opening, and after every write, the position for reading may be set at
    /// the end of the file. So, before writing, save the current position
    /// (using <code>[Seek]::[stream_position]</code>), and restore it
    /// before the next read.
    ///
    /// ## Note
    ///
    /// This function doesn't create the file if it doesn't exist. Use the
    /// [`OpenOptions::create`] method to do so.
    ///
    /// [`write()`]: Write::write "io::Write::write"
    /// [`flush()`]: Write::flush "io::Write::flush"
    /// [stream_position]: Seek::stream_position "io::Seek::stream_position"
    /// [seek]: Seek::seek "io::Seek::seek"
    /// [Current]: SeekFrom::Current "io::SeekFrom::Current"
    /// [End]: SeekFrom::End "io::SeekFrom::End"
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().append(true).open("foo.txt");
    /// ```
    #[inline]
    pub fn append(&mut self, append: bool) -> &mut OpenOptions {
        if append {
            self.opts |= APPEND;
        } else {
            self.opts &= !APPEND;
        }
        self
    }
    /// Sets the option to create a new file, or open it if it already exists.
    ///
    /// In order for the file to be created, [`OpenOptions::write`] or
    /// [`OpenOptions::append`] access must be used.
    ///
    /// See also [`xrmt_stx::fs::write()`][self::write] for a simple function to
    /// create a file with some given data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().write(true).create(true).open("foo.txt");
    /// ```
    #[inline]
    pub fn create(&mut self, create: bool) -> &mut OpenOptions {
        if create {
            self.opts |= CREATE;
        } else {
            self.opts &= !CREATE;
        }
        self
    }
    /// Opens a file at `path` with the options specified by `self`.
    ///
    /// # Errors
    ///
    /// This function will return an error under a number of different
    /// circumstances. Some of these error conditions are listed here, together
    /// with their [`ErrorKind`]. The mapping to [`ErrorKind`]s is not
    /// part of the compatibility contract of the function.
    ///
    /// * [`NotFound`]: The specified file does not exist and neither `create`
    ///   or `create_new` is set.
    /// * [`NotFound`]: One of the directory components of the file path does
    ///   not exist.
    /// * [`PermissionDenied`]: The user lacks permission to get the specified
    ///   access rights for the file.
    /// * [`PermissionDenied`]: The user lacks permission to open one of the
    ///   directory components of the specified path.
    /// * [`AlreadyExists`]: `create_new` was specified and the file already
    ///   exists.
    /// * [`InvalidInput`]: Invalid combinations of open options (truncate
    ///   without write access, no access mode set, etc.).
    ///
    /// The following errors don't match any existing [`ErrorKind`] at the
    /// moment:
    /// * One of the directory components of the specified file path was not, in
    ///   fact, a directory.
    /// * Filesystem-level errors: full disk, write permission requested on a
    ///   read-only file system, exceeded disk quota, too many open files, too
    ///   long filename, too many symbolic links in the specified path
    ///   (Unix-like systems only), etc.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().read(true).open("foo.txt");
    /// ```
    ///
    /// [`AlreadyExists`]: ErrorKind::AlreadyExists
    /// [`InvalidInput`]: ErrorKind::InvalidInput
    /// [`NotFound`]: ErrorKind::NotFound
    /// [`PermissionDenied`]: ErrorKind::PermissionDenied
    #[inline]
    pub fn open(&self, path: impl AsRef<Path>) -> IoResult<File> {
        self.open_inner(Handle::EMPTY, 0, path)
    }
    // Sets the option for truncating a previous file.
    ///
    /// If a file is successfully opened with this option set to true, it will
    /// truncate the file to 0 length if it already exists.
    ///
    /// The file must be opened with write access for truncate to work.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().write(true).truncate(true).open("foo.txt");
    /// ```
    #[inline]
    pub fn truncate(&mut self, truncate: bool) -> &mut OpenOptions {
        if truncate {
            self.opts |= TRUNCATE;
        } else {
            self.opts &= !TRUNCATE;
        }
        self
    }
    /// Sets the option to create a new file, failing if it already exists.
    ///
    /// No file is allowed to exist at the target location, also no (dangling)
    /// symlink. In this way, if the call succeeds, the file returned is
    /// guaranteed to be new. If a file exists at the target location,
    /// creating a new file will fail with [`AlreadyExists`] or another
    /// error based on the situation. See [`OpenOptions::open`] for a
    /// non-exhaustive list of likely errors.
    ///
    /// This option is useful because it is atomic. Otherwise between checking
    /// whether a file exists and creating a new one, the file may have been
    /// created by another process (a TOCTOU race condition / attack).
    ///
    /// If `.create_new(true)` is set, [`.create()`] and [`.truncate()`] are
    /// ignored.
    ///
    /// The file must be opened with write or append access in order to create
    /// a new file.
    ///
    /// [`.create()`]: OpenOptions::create
    /// [`.truncate()`]: OpenOptions::truncate
    /// [`AlreadyExists`]: ErrorKind::AlreadyExists
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().write(true)
    ///                              .create_new(true)
    ///                              .open("foo.txt");
    /// ```
    #[inline]
    pub fn create_new(&mut self, create_new: bool) -> &mut OpenOptions {
        if create_new {
            self.opts |= CREATE_NEW;
        } else {
            self.opts &= !CREATE_NEW;
        }
        self
    }

    #[inline]
    fn new_with(v: u32, opts: u16) -> OpenOptions {
        OpenOptions {
            opts:   SYNCHRONOUS | opts,
            share:  0u32,
            attrs:  v,
            access: 0u32,
        }
    }
    fn open_inner(&self, parent: Handle, add_attrs: u32, path: impl AsRef<Path>) -> IoResult<File> {
        let (a, b) = if self.access > 0 {
            (self.access, 0)
        } else {
            match (
                self.opts & READ > 0,
                self.opts & WRITE > 0,
                self.opts & APPEND > 0,
                self.opts & DELETE > 0,
            ) {
                (_, _, _, true) => (0x110001, 0x7),           // DELETE | FILE_LIST_DIRECTORY | SYNCHRONIZE, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
                (true, false, false, _) => (0x80100080, 0x1), // GENERIC_READ | READ_ATTRIBUTES | SYNCHRONIZE, FILE_SHARE_READ
                (false, true, false, _) => (0x40110000, 0x6), // GENERIC_WRITE | DELETE | SYNCHRONIZE, FILE_SHARE_WRITE | FILE_SHARE_DELETE
                (true, true, false, _) => (0xC0110080, 0x7),  // GENERIC_READ | READ_ATTRIBUTES | DELETE | SYNCHRONIZE | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
                (false, _, true, _) => (0x110114, 0x7),       // DELETE | SYNCHRONIZE | FILE_WRITE_ATTRIBUTES | FILE_APPEND_DATA | FILE_WRITE_EA, FILE_SHARE_READ | FILE_SHARE_WRITE |
                // FILE_SHARE_DELETE
                (true, _, true, _) => (0x80110194, 0x7), // GENERIC_READ | FILE READ_ATTRIBUTES | | DELETE | SYNCHRONIZE | FILE_WRITE_ATTRIBUTES | FILE_APPEND_DATA |
                // FILE_WRITE_EA, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
                _ => (0, 0),
            }
        };
        let c = match (
            self.opts & CREATE > 0,
            self.opts & TRUNCATE > 0,
            self.opts & CREATE_NEW > 0,
        ) {
            (false, false, false) => 0x3, // 0x3 - OPEN_EXISTING
            (true, false, false) => 0x4,  // 0x4 - OPEN_ALWAYS
            (false, true, false) => 0x5,  // 0x5 - TRUNCATE_EXISTING
            (true, true, false) => 0x2,   // 0x2 - CREATE_ALWAYS
            (_, _, true) => 0x1,          // 0x1 - CREATE_NEW
        };
        let (h, j, k, l) = win32_file_flags_to_nt(
            a,
            c,
            self.attrs
                | add_attrs
                | if self.opts & CREATE_NEW > 0 || self.opts & NO_SYMLINK > 0 {
                    0x200000 // 0x200000 - FILE_FLAG_BACKUP_SEMANTICS
                } else {
                    0
                },
        );
        Ok(File(NtCreateFile(
            path.as_ref(),
            parent,
            h,
            None,
            j,
            self.share | if self.opts & EXCLUSIVE > 0 { 0 } else { b },
            k,
            l,
        )?))
    }
}
impl Permissions {
    /// Returns `true` if these permissions describe a readonly (unwritable)
    /// file.
    ///
    /// # Note
    ///
    /// This function does not take Access Control Lists (ACLs), Unix group
    /// membership and other nuances into account.
    /// Therefore the return value of this function cannot be relied upon
    /// to predict whether attempts to read or write the file will actually
    /// succeed.
    ///
    /// # Windows
    ///
    /// On Windows this returns [`FILE_ATTRIBUTE_READONLY`](https://docs.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants).
    /// If `FILE_ATTRIBUTE_READONLY` is set then writes to the file will fail
    /// but the user may still have permission to change this flag. If
    /// `FILE_ATTRIBUTE_READONLY` is *not* set then writes may still fail due
    /// to lack of write permission.
    /// The behavior of this attribute for directories depends on the Windows
    /// version.
    ///
    /// # Unix (including macOS)
    ///
    /// On Unix-based platforms this checks if *any* of the owner, group or
    /// others write permission bits are set. It does not consider anything
    /// else, including:
    ///
    /// * Whether the current user is in the file's assigned group.
    /// * Permissions granted by ACL.
    /// * That `root` user can write to files that do not have any write bits
    ///   set.
    /// * Writable files on a filesystem that is mounted read-only.
    ///
    /// The `PermissionsExt` trait gives direct access to the permission bits
    /// but also does not read ACLs.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut f = File::create("foo.txt")?;
    ///     let metadata = f.metadata()?;
    ///
    ///     assert_eq!(false, metadata.permissions().readonly());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn readonly(&self) -> bool {
        // 0x1 - FILE_ATTRIBUTE_READONLY
        self.attributes & 0x1 != 0
    }
    /// Modifies the readonly flag for this set of permissions. If the
    /// `readonly` argument is `true`, using the resulting `Permission` will
    /// update file permissions to forbid writing. Conversely, if it's `false`,
    /// using the resulting `Permission` will update file permissions to allow
    /// writing.
    ///
    /// This operation does **not** modify the files attributes. This only
    /// changes the in-memory value of these attributes for this `Permissions`
    /// instance. To modify the files attributes use the [`set_permissions`]
    /// function which commits these attribute changes to the file.
    ///
    /// # Note
    ///
    /// `set_readonly(false)` makes the file *world-writable* on Unix.
    /// You can use the `PermissionsExt` trait on Unix to avoid this issue.
    ///
    /// It also does not take Access Control Lists (ACLs) or Unix group
    /// membership into account.
    ///
    /// # Windows
    ///
    /// On Windows this sets or clears [`FILE_ATTRIBUTE_READONLY`](https://docs.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants).
    /// If `FILE_ATTRIBUTE_READONLY` is set then writes to the file will fail
    /// but the user may still have permission to change this flag. If
    /// `FILE_ATTRIBUTE_READONLY` is *not* set then the write may still fail if
    /// the user does not have permission to write to the file.
    ///
    /// In Windows 7 and earlier this attribute prevents deleting empty
    /// directories. It does not prevent modifying the directory contents.
    /// On later versions of Windows this attribute is ignored for directories.
    ///
    /// # Unix (including macOS)
    ///
    /// On Unix-based platforms this sets or clears the write access bit for
    /// the owner, group *and* others, equivalent to `chmod a+w <file>`
    /// or `chmod a-w <file>` respectively. The latter will grant write access
    /// to all users! You can use the `PermissionsExt` trait on Unix
    /// to avoid this issue.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let f = File::create("foo.txt")?;
    ///     let metadata = f.metadata()?;
    ///     let mut permissions = metadata.permissions();
    ///
    ///     permissions.set_readonly(true);
    ///
    ///     // filesystem doesn't change, only the in memory state of the
    ///     // readonly permission
    ///     assert_eq!(false, metadata.permissions().readonly());
    ///
    ///     // just this particular `permissions`.
    ///     assert_eq!(true, permissions.readonly());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn set_readonly(&mut self, readonly: bool) {
        // 0x1 - FILE_ATTRIBUTE_READONLY
        self.attributes = if readonly {
            self.attributes | 0x1
        } else {
            self.attributes ^ &0x1
        }
    }
}

impl Seek for File {
    #[inline]
    fn stream_len(&mut self) -> IoResult<u64> {
        let mut s = FileStandardInformation::default();
        // Always size 32.
        //
        // 0x5 - FileStandardInformation
        NtQueryInformationFile(self, 0x5, &mut s, 0x20)?;
        Ok(s.end_of_file)
    }
    #[inline]
    fn stream_position(&mut self) -> IoResult<u64> {
        let mut n = 0u64;
        // 0xE - FilePositionInformation
        NtQueryInformationFile(self, 0xE, &mut n, 8)?;
        Ok(n)
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        let (w, n) = match pos {
            SeekFrom::End(n) => (2, n),
            SeekFrom::Start(n) => (0, n as i64),
            SeekFrom::Current(n) => (1, n),
        };
        Ok(SetFilePointerEx(self, n, w)?)
    }
}
impl Read for File {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        Ok(NtReadFile(&self.0, None, buf, None)?)
    }
}
impl Write for File {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(NtFlushBuffersFile(self)?)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        Ok(NtWriteFile(&self.0, None, buf, None)?)
    }
}
impl Seek for &File {
    #[inline]
    fn stream_len(&mut self) -> IoResult<u64> {
        let mut s = FileStandardInformation::default();
        // Always size 32.
        //
        // 0x5 - FileStandardInformation
        NtQueryInformationFile(self, 0x5, &mut s, 0x20)?;
        Ok(s.end_of_file)
    }
    #[inline]
    fn stream_position(&mut self) -> IoResult<u64> {
        let mut n = 0u64;
        // 0xE - FilePositionInformation
        NtQueryInformationFile(self, 0xE, &mut n, 8)?;
        Ok(n)
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        let (w, n) = match pos {
            SeekFrom::End(n) => (2, n),
            SeekFrom::Start(n) => (0, n as i64),
            SeekFrom::Current(n) => (1, n),
        };
        Ok(SetFilePointerEx(self, n, w)?)
    }
}
impl Read for &File {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        Ok(NtReadFile(self, None, buf, None)?)
    }
}
impl Write for &File {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(NtFlushBuffersFile(self)?)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        Ok(NtWriteFile(self, None, buf, None)?)
    }
}
impl IsTerminal for File {
    #[inline]
    fn is_terminal(&self) -> bool {
        is_terminal(self)
    }
}
impl From<OwnedHandle> for File {
    #[inline]
    fn from(v: OwnedHandle) -> File {
        File(v)
    }
}

impl Eq for FileType {}
impl Copy for FileType {}
impl Clone for FileType {
    #[inline]
    fn clone(&self) -> FileType {
        FileType {
            attrs:   self.attrs,
            reparse: self.reparse,
        }
    }
}
impl PartialEq for FileType {
    #[inline]
    fn eq(&self, other: &FileType) -> bool {
        self.attrs == other.attrs && self.reparse == other.reparse
    }
}

impl Eq for Permissions {}
impl Clone for Permissions {
    #[inline]
    fn clone(&self) -> Permissions {
        Permissions {
            access:     self.access,
            attributes: self.attributes,
        }
    }
}
impl PartialEq for Permissions {
    #[inline]
    fn eq(&self, other: &Permissions) -> bool {
        self.attributes == other.attributes && self.access == self.access
    }
}

impl Iterator for ReadDir {
    type Item = IoResult<DirEntry>;

    #[inline]
    fn next(&mut self) -> Option<IoResult<DirEntry>> {
        self.refill();
        match self.state {
            ReadState::Empty | ReadState::Error => return None,
            _ => (),
        }
        if let Some(e) = self.pull() {
            return Some(Ok(e));
        }
        self.next()
    }
}

impl Default for DirBuilder {
    #[inline]
    fn default() -> DirBuilder {
        DirBuilder::new()
    }
}

impl Clone for OpenOptions {
    #[inline]
    fn clone(&self) -> OpenOptions {
        OpenOptions {
            opts:   self.opts,
            share:  self.share,
            attrs:  self.attrs,
            access: self.access,
        }
    }
}
impl Default for OpenOptions {
    #[inline]
    fn default() -> OpenOptions {
        OpenOptions::new()
    }
}

impl From<File> for Handle {
    #[inline]
    fn from(v: File) -> Handle {
        unsafe { Handle::take(v.0) }
    }
}
impl From<File> for OwnedHandle {
    #[inline]
    fn from(v: File) -> OwnedHandle {
        v.0
    }
}

impl Clone for Metadata {
    #[inline]
    fn clone(&self) -> Metadata {
        Metadata {
            access:           self.access,
            file_size:        self.file_size,
            file_index:       self.file_index,
            attributes:       self.attributes,
            reparse_tag:      self.reparse_tag,
            change_time:      self.change_time,
            creation_time:    self.creation_time,
            last_write_time:  self.last_write_time,
            number_of_links:  self.number_of_links,
            last_access_time: self.last_access_time,
        }
    }
}
impl From<&FileIdBothDirInfo> for Metadata {
    #[inline]
    fn from(v: &FileIdBothDirInfo) -> Metadata {
        Metadata {
            access:           0u32,
            file_size:        v.end_of_file,
            file_index:       v.file_id,
            attributes:       v.attributes,
            reparse_tag:      0u32,
            change_time:      v.change_time,
            creation_time:    v.creation_time,
            number_of_links:  0u32,
            last_write_time:  v.last_write_time,
            last_access_time: v.last_access_time,
        }
    }
}
impl From<FileStatInformation> for Metadata {
    #[inline]
    fn from(v: FileStatInformation) -> Metadata {
        Metadata {
            access:           v.access,
            file_size:        v.end_of_file,
            file_index:       v.file_id,
            attributes:       v.attributes,
            reparse_tag:      v.reparse_tag,
            change_time:      v.change_time,
            creation_time:    v.creation_time,
            last_access_time: v.last_access_time,
            last_write_time:  v.last_write_time,
            number_of_links:  v.number_of_links,
        }
    }
}

impl Copy for FileTimes {}
impl Clone for FileTimes {
    #[inline]
    fn clone(&self) -> FileTimes {
        FileTimes {
            created:  self.created.clone(),
            accessed: self.accessed.clone(),
            modified: self.modified.clone(),
        }
    }
}
impl Default for FileTimes {
    #[inline]
    fn default() -> FileTimes {
        FileTimes::new()
    }
}

/// Returns `Ok(true)` if the path points at an existing entity.
///
/// This function will traverse symbolic links to query information about the
/// destination file. In case of broken symbolic links this will return
/// `Ok(false)`.
///
/// As opposed to the [`Path::exists`] method, this will only return `Ok(true)`
/// or `Ok(false)` if the path was _verified_ to exist or not exist. If its
/// existence can neither be confirmed nor denied, an `Err(_)` will be
/// propagated instead. This can be the case if e.g. listing permission is
/// denied on one of the parent directories.
///
/// Note that while this avoids some pitfalls of the `exists()` method, it still
/// can not prevent time-of-check to time-of-use (TOCTOU) bugs. You should only
/// use it in scenarios where those bugs are not an issue.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// assert!(!fs::exists("does_not_exist.txt").expect("Can't check existence of file does_not_exist.txt"));
/// assert!(fs::exists("/root/secret_file.txt").is_err());
/// ```
///
/// [`Path::exists`]: crate::path::Path::exists
#[inline]
pub fn exists(path: impl AsRef<Path>) -> IoResult<bool> {
    File::open(path).map(|f| f.is_file())
}
/// Reads the entire contents of a file into a bytes vector.
///
/// This is a convenience function for using [`File::open`] and [`read_to_end`]
/// with fewer imports and without an intermediate variable.
///
/// [`read_to_end`]: Read::read_to_end
///
/// # Errors
///
/// This function will return an error if `path` does not already exist.
/// Other errors may also be returned according to [`OpenOptions::open`].
///
/// While reading from the file, this function handles
/// [`ErrorKind::Interrupted`] with automatic retries. See [Read]
/// documentation for details.
///
/// [`ErrorKind`]: crate::io::ErrorKind
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> Result<(), Box<dyn xrmt_stx::error::Error + 'static>> {
///     let data: Vec<u8> = fs::read("image.jpg")?;
///     assert_eq!(data[0..3], [0xFF, 0xD8, 0xFF]);
///     Ok(())
/// }
/// ```
#[inline]
pub fn read(path: impl AsRef<Path>) -> IoResult<Vec<u8>> {
    let mut f = File::open(path)?;
    let mut b = Vec::with_capacity(f.metadata().map(|m| m.file_size).unwrap_or_default() as usize);
    f.read_to_end(&mut b)?;
    Ok(b)
}
/// Creates a new, empty directory at the provided path
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `mkdir` function on Unix
/// and the `CreateDirectoryW` function on Windows.
/// Note that, this [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
///
/// **NOTE**: If a parent of the given path doesn't exist, this function will
/// return an error. To create a directory and all its missing parents at the
/// same time, use the [`create_dir_all`] function.
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * User lacks permissions to create directory at `path`.
/// * A parent of the given path doesn't exist. (To create a directory and all
///   its missing parents at the same time, use the [`create_dir_all`]
///   function.)
/// * `path` already exists.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     fs::create_dir("/some/dir")?;
///     Ok(())
/// }
/// ```
#[inline]
pub fn create_dir(path: impl AsRef<Path>) -> IoResult<()> {
    DirBuilder::new().create(path.as_ref())
}
/// Removes an empty directory.
///
/// If you want to remove a directory that is not empty, as well as all
/// of its contents recursively, consider using [`remove_dir_all`]
/// instead.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `rmdir` function on Unix
/// and the `RemoveDirectory` function on Windows.
/// Note that, this [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * `path` doesn't exist.
/// * `path` isn't a directory.
/// * The user lacks permissions to remove the directory at the provided `path`.
/// * The directory isn't empty.
///
/// This function will only ever return an error of kind `NotFound` if the given
/// path does not exist. Note that the inverse is not true,
/// ie. if a path does not exist, its removal may fail for a number of reasons,
/// such as insufficient permissions.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     fs::remove_dir("/some/dir")?;
///     Ok(())
/// }
/// ```
#[inline]
pub fn remove_dir(path: impl AsRef<Path>) -> IoResult<()> {
    // 0x2200000 - FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT
    OpenOptions::new_with(0x2200000, DELETE | NO_SYMLINK).open(path)?.delete()
}
/// Removes a file from the filesystem.
///
/// Note that there is no
/// guarantee that the file is immediately deleted (e.g., depending on
/// platform, other open file descriptors may prevent immediate removal).
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `unlink` function on Unix.
/// On Windows, `DeleteFile` is used or `CreateFileW` and
/// `SetInformationByHandle` for readonly files. Note that, this [may change in
/// the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * `path` points to a directory.
/// * The file doesn't exist.
/// * The user lacks permissions to remove the file.
///
/// This function will only ever return an error of kind `NotFound` if the given
/// path does not exist. Note that the inverse is not true,
/// ie. if a path does not exist, its removal may fail for a number of reasons,
/// such as insufficient permissions.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     fs::remove_file("a.txt")?;
///     Ok(())
/// }
/// ```
#[inline]
pub fn remove_file(path: impl AsRef<Path>) -> IoResult<()> {
    Ok(DeleteFile(path.as_ref())?)
}
/// Returns an iterator over the entries within a directory.
///
/// The iterator will yield instances of <code>[Result]<[DirEntry]></code>.
/// New errors may be encountered after an iterator is initially constructed.
/// Entries for the current and parent directories (typically `.` and `..`) are
/// skipped.
///
/// [Result]: crate::IoResult
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `opendir` function on Unix
/// and the `FindFirstFileEx` function on Windows. Advancing the iterator
/// currently corresponds to `readdir` on Unix and `FindNextFile` on Windows.
/// Note that, this [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
///
/// The order in which this iterator returns entries is platform and filesystem
/// dependent.
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * The provided `path` doesn't exist.
/// * The process lacks permissions to view the contents.
/// * The `path` points at a non-directory file.
///
/// # Examples
///
/// ```
/// use xrmt_stx::io::{self, IoResult};
/// use xrmt_stx::fs::{self, DirEntry};
/// use xrmt_stx::path::Path;
///
/// // one possible implementation of walking a directory only visiting files
/// fn visit_dirs(dir: &Path, cb: &dyn Fn(&DirEntry)) -> IoResult<()> {
///     if dir.is_dir() {
///         for entry in fs::read_dir(dir)? {
///             let entry = entry?;
///             let path = entry.path();
///             if path.is_dir() {
///                 visit_dirs(&path, cb)?;
///             } else {
///                 cb(&entry);
///             }
///         }
///     }
///     Ok(())
/// }
/// ```
///
/// ```rust,no_run
/// use xrmt_stx::{fs, io};
///
/// fn main() -> IoResult<()> {
///     let mut entries = fs::read_dir(".")?
///         .map(|res| res.map(|e| e.path()))
///         .collect::<Result<Vec<_>, io::Error>>()?;
///
///     // The order in which `read_dir` returns entries is not guaranteed. If reproducible
///     // ordering is required the entries should be explicitly sorted.
///
///     entries.sort();
///
///     // The entries have now been sorted by their path.
///
///     Ok(())
/// }
/// ```
#[inline]
pub fn read_dir(path: impl AsRef<Path>) -> IoResult<ReadDir> {
    // 0x2200000 - FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT
    OpenOptions::new_with(0x2200000, 0).read(true).open(path)?.read_dir()
}
/// Reads a symbolic link, returning the file that the link points to.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `readlink` function on Unix
/// and the `CreateFile` function with `FILE_FLAG_OPEN_REPARSE_POINT` and
/// `FILE_FLAG_BACKUP_SEMANTICS` flags on Windows.
/// Note that, this [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * `path` is not a symbolic link.
/// * `path` does not exist.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let path = fs::read_link("a.txt")?;
///     Ok(())
/// }
/// ```
#[inline]
pub fn read_link(path: impl AsRef<Path>) -> IoResult<PathBuf> {
    Ok(file_name(&OpenOptions::new_with(0, NO_SYMLINK).read(true).open(path)?)?.into())
}
/// Removes a directory at this path, after removing all its contents. Use
/// carefully!
///
/// This function does **not** follow symbolic links and it will simply remove
/// the symbolic link itself.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to `openat`, `fdopendir`, `unlinkat` and
/// `lstat` functions on Unix (except for REDOX) and the `CreateFileW`,
/// `GetFileInformationByHandleEx`, `SetFileInformationByHandle`, and
/// `NtCreateFile` functions on Windows. Note that, this [may change in the
/// future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
///
/// On REDOX, as well as when running in Miri for any target, this function is
/// not protected against time-of-check to time-of-use (TOCTOU) race conditions,
/// and should not be used in security-sensitive code on those platforms. All
/// other platforms are protected.
///
/// # Errors
///
/// See [`remove_file`] and [`remove_dir`].
///
/// [`remove_dir_all`] will fail if [`remove_dir`] or [`remove_file`] fail on
/// *any* constituent paths, *including* the root `path`. Consequently,
///
/// - The directory you are deleting *must* exist, meaning that this function is
///   *not idempotent*.
/// - [`remove_dir_all`] will fail if the `path` is *not* a directory.
///
/// Consider ignoring the error if validating the removal is not required for
/// your use case.
///
/// [`ErrorKind::NotFound`] is only returned if no removal occurs.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     fs::remove_dir_all("/some/dir")?;
///     Ok(())
/// }
/// ```
#[inline]
pub fn remove_dir_all(path: impl AsRef<Path>) -> IoResult<()> {
    // 0x2200000- FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT
    let o = OpenOptions::new_with(0x2200000, DELETE | NO_SYMLINK);
    remove_dir_inner(o.open(path)?, &o)
}
/// Given a path, queries the file system to get information about a file,
/// directory, etc.
///
/// This function will traverse symbolic links to query information about the
/// destination file.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `stat` function on Unix
/// and the `GetFileInformationByHandle` function on Windows.
/// Note that, this [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * The user lacks permissions to perform `metadata` call on `path`.
/// * `path` does not exist.
///
/// # Examples
///
/// ```rust,no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let attr = fs::metadata("/some/file/path.txt")?;
///     // inspect attr ...
///     Ok(())
/// }
/// ```
#[inline]
pub fn metadata(path: impl AsRef<Path>) -> IoResult<Metadata> {
    // FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT
    OpenOptions::new_with(0x2200000, 0).read(true).open(path)?.metadata()
}
/// Recursively create a directory and all of its parent components if they
/// are missing.
///
/// If this function returns an error, some of the parent components might have
/// been created already.
///
/// If the empty path is passed to this function, it always succeeds without
/// creating any directories.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to multiple calls to the `mkdir`
/// function on Unix and the `CreateDirectoryW` function on Windows.
///
/// Note that, this [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
///
/// # Errors
///
/// The function will return an error if any directory specified in path does
/// not exist and could not be created. There may be other error conditions; see
/// [`create_dir`] for specifics.
///
/// Notable exception is made for situations where any of the directories
/// specified in the `path` could not be created as it was being created
/// concurrently. Such cases are considered to be successful. That is, calling
/// `create_dir_all` concurrently from multiple threads or processes is
/// guaranteed not to fail due to a race condition with itself.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     fs::create_dir_all("/some/dir")?;
///     Ok(())
/// }
/// ```
#[inline]
pub fn create_dir_all(path: impl AsRef<Path>) -> IoResult<()> {
    DirBuilder::new().recursive(true).create(path.as_ref())
}
/// Returns the canonical, absolute form of a path with all intermediate
/// components normalized and symbolic links resolved.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `realpath` function on Unix
/// and the `CreateFile` and `GetFinalPathNameByHandle` functions on Windows.
/// Note that this [may change in the future][changes].
///
/// On Windows, this converts the path to use [extended length path][path]
/// syntax, which allows your program to use longer path names, but means you
/// can only join backslash-delimited paths to it, and it may be incompatible
/// with other applications (if passed to the application on the command-line,
/// or written to a file another application may read).
///
/// [changes]: crate::io#platform-specific-behavior
/// [path]: https://docs.microsoft.com/en-us/windows/win32/fileio/naming-a-file
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * `path` does not exist.
/// * A non-final component in path is not a directory.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let path = fs::canonicalize("../a/../foo.txt")?;
///     Ok(())
/// }
/// ```
#[inline]
pub fn canonicalize(path: impl AsRef<Path>) -> IoResult<PathBuf> {
    Ok(file_name(NtCreateFile(
        path_normalize(path.as_ref()),
        Handle::EMPTY,
        0x80100080,
        None,
        0,
        0x1,
        0x1,
        0,
    )?)?
    .into())
}
/// Reads the entire contents of a file into a string.
///
/// This is a convenience function for using [`File::open`] and
/// [`read_to_string`] with fewer imports and without an intermediate variable.
///
/// [`read_to_string`]: Read::read_to_string
///
/// # Errors
///
/// This function will return an error if `path` does not already exist.
/// Other errors may also be returned according to [`OpenOptions::open`].
///
/// If the contents of the file are not valid UTF-8, then an error will also be
/// returned.
///
/// While reading from the file, this function handles
/// [`ErrorKind::Interrupted`] with automatic retries. See [Read]
/// documentation for details.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
/// use xrmt_stx::error::Error;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let message: String = fs::read_to_string("message.txt")?;
///     println!("{}", message);
///     Ok(())
/// }
/// ```
#[inline]
pub fn read_to_string(path: impl AsRef<Path>) -> IoResult<String> {
    let mut f = File::open(path)?;
    let s = f.metadata().map(|m| m.file_size).unwrap_or_default();
    let mut b = String::with_capacity(s as usize);
    unsafe { f.read_exact(b.as_bytes_mut())? };
    Ok(b)
}
/// Queries the metadata about a file without following symlinks.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `lstat` function on Unix
/// and the `GetFileInformationByHandle` function on Windows.
/// Note that, this [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * The user lacks permissions to perform `metadata` call on `path`.
/// * `path` does not exist.
///
/// # Examples
///
/// ```rust,no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let attr = fs::symlink_metadata("/some/file/path.txt")?;
///     // inspect attr ...
///     Ok(())
/// }
/// ```
#[inline]
pub fn symlink_metadata(path: impl AsRef<Path>) -> IoResult<Metadata> {
    OpenOptions::new_with(0, NO_SYMLINK).read(true).open(path)?.metadata()
}
/// Copies the contents of one file to another. This function will also
/// copy the permission bits of the original file to the destination file.
///
/// This function will **overwrite** the contents of `to`.
///
/// Note that if `from` and `to` both point to the same file, then the file
/// will likely get truncated by this operation.
///
/// On success, the total number of bytes copied is returned and it is equal to
/// the length of the `to` file as reported by `metadata`.
///
/// If you want to copy the contents of one file to another and you’re
/// working with [`File`]s, see the [`io::copy`](crate::io::copy()) function.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `open` function in Unix
/// with `O_RDONLY` for `from` and `O_WRONLY`, `O_CREAT`, and `O_TRUNC` for
/// `to`. `O_CLOEXEC` is set for returned file descriptors.
///
/// On Linux (including Android), this function attempts to use
/// `copy_file_range(2)`, and falls back to reading and writing if that is not
/// possible.
///
/// On Windows, this function currently corresponds to `CopyFileEx`. Alternate
/// NTFS streams are copied but only the size of the main stream is returned by
/// this function.
///
/// On MacOS, this function corresponds to `fclonefileat` and `fcopyfile`.
///
/// Note that platform-specific behavior [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * `from` is neither a regular file nor a symlink to a regular file.
/// * `from` does not exist.
/// * The current process does not have the permission rights to read `from` or
///   write `to`.
/// * The parent directory of `to` doesn't exist.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     fs::copy("foo.txt", "bar.txt")?;  // Copy foo.txt to bar.txt
///     Ok(())
/// }
/// ```
#[inline]
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> IoResult<u64> {
    Ok(CopyFileEx(from.as_ref(), to.as_ref(), 0)?)
}
/// Renames a file or directory to a new name, replacing the original file if
/// `to` already exists.
///
/// This will not work if the new name is on a different mount point.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `rename` function on Unix
/// and the `SetFileInformationByHandle` function on Windows.
///
/// Because of this, the behavior when both `from` and `to` exist differs. On
/// Unix, if `from` is a directory, `to` must also be an (empty) directory. If
/// `from` is not a directory, `to` must also be not a directory. The behavior
/// on Windows is the same on Windows 10 1607 and higher if `FileRenameInfoEx`
/// is supported by the filesystem; otherwise, `from` can be anything, but
/// `to` must *not* be a directory.
///
/// Note that, this [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * `from` does not exist.
/// * The user lacks permissions to view contents.
/// * `from` and `to` are on separate filesystems.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     fs::rename("a.txt", "b.txt")?; // Rename a.txt to b.txt
///     Ok(())
/// }
/// ```
#[inline]
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> IoResult<()> {
    // If they're on the same volume this will work. If not, we'll fallback.
    if file_rename(from.as_ref(), to.as_ref()).is_ok() {
        return Ok(());
    }
    // MOVEFILE_COPY_ALLOWED is needed if we have to work across volumes/mounts.
    // 0x3 - MOVEFILE_REPLACE_EXISTING | MOVEFILE_COPY_ALLOWED
    Ok(MoveFileEx(from.as_ref(), to.as_ref(), 0x3)?)
}
/// Writes a slice as the entire contents of a file.
///
/// This function will create a file if it does not exist,
/// and will entirely replace its contents if it does.
///
/// Depending on the platform, this function may fail if the
/// full directory path does not exist.
///
/// This is a convenience function for using [`File::create`] and [`write_all`]
/// with fewer imports.
///
/// [`write_all`]: Write::write_all
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     fs::write("foo.txt", b"Lorem ipsum")?;
///     fs::write("bar.txt", "dolor sit")?;
///     Ok(())
/// }
/// ```
#[inline]
pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> IoResult<()> {
    File::create(path)?.write_all(contents.as_ref())
}
/// Changes the permissions found on a file or a directory.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `chmod` function on Unix
/// and the `SetFileAttributes` function on Windows.
/// Note that, this [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * `path` does not exist.
/// * The user lacks the permission to change attributes of the file.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let mut perms = fs::metadata("foo.txt")?.permissions();
///     perms.set_readonly(true);
///     fs::set_permissions("foo.txt", perms)?;
///     Ok(())
/// }
/// ```
#[inline]
pub fn set_permissions(path: impl AsRef<Path>, perm: Permissions) -> IoResult<()> {
    Ok(SetFileAttributes(path.as_ref(), perm.attributes)?)
}
/// Creates a new hard link on the filesystem.
///
/// The `link` path will be a link pointing to the `original` path. Note that
/// systems often require these two paths to both be located on the same
/// filesystem.
///
/// If `original` names a symbolic link, it is platform-specific whether the
/// symbolic link is followed. On platforms where it's possible to not follow
/// it, it is not followed, and the created hard link points to the symbolic
/// link itself.
///
/// # Platform-specific behavior
///
/// This function currently corresponds the `CreateHardLink` function on
/// Windows. On most Unix systems, it corresponds to the `linkat` function with
/// no flags. On Android, VxWorks, and Redox, it instead corresponds to the
/// `link` function. On MacOS, it uses the `linkat` function if it is available,
/// but on very old systems where `linkat` is not available, `link` is selected
/// at runtime instead. Note that, this [may change in the future][changes].
///
/// [changes]: crate::io#platform-specific-behavior
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * The `original` path is not a file or doesn't exist.
/// * The 'link' path already exists.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     fs::hard_link("a.txt", "b.txt")?; // Hard link a.txt to b.txt
///     Ok(())
/// }
/// ```
#[inline]
pub fn hard_link(original: impl AsRef<Path>, link: impl AsRef<Path>) -> IoResult<()> {
    Ok(CreateHardLink(link.as_ref(), original.as_ref(), false)?)
}
/// Creates a new symbolic link on the filesystem.
///
/// The `link` path will be a symbolic link pointing to the `original` path.
/// On Windows, this will be a file symlink, not a directory symlink;
/// for this reason, the platform-specific `symlink` and [`symlink_file`] or
/// [`symlink_dir`] should be used instead to make the intent explicit.
///
/// [`symlink_dir`]: crate::os::windows::fs::symlink_dir
/// [`symlink_file`]: crate::os::windows::fs::symlink_file
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::fs;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     fs::soft_link("a.txt", "b.txt")?;
///     Ok(())
/// }
/// ```
#[inline]
pub fn soft_link(original: impl AsRef<Path>, link: impl AsRef<Path>) -> IoResult<()> {
    let _ = privilege_accquire(Privilege::SeCreateSymbolicLink)?;
    let r = CreateSymbolicLink(link.as_ref(), original.as_ref(), 0);
    let _ = privilege_release(Privilege::SeCreateSymbolicLink);
    Ok(r?)
}

fn remove_dir_inner(f: File, o: &OpenOptions) -> IoResult<()> {
    let mut r = f.read_dir()?;
    for e in r.by_ref() {
        // This will never fail as we don't error on ReadDir entries.
        let t = unsafe { e.unwrap_unchecked() };
        let h = o.open_inner(t.owner, 0, &t.name)?;
        if t.meta.is_dir() {
            remove_dir_inner(h, o)?;
        } else {
            file_delete(h)?;
        }
    }
    Ok(file_delete(r.owner)?)
}
