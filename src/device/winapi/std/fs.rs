// Copyright (C) 2023 iDigitalFlame
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
#![cfg(target_family = "windows")]

use alloc::sync::Arc;
use core::ops::{Deref, DerefMut};
use core::slice::from_raw_parts;
use core::{cmp, matches};

use crate::data::time::Time;
use crate::device::winapi::{self, AsHandle, DecodeUtf16, FileAllInformation, FileBasicInformation, FileIdBothDirInfo, FileStandardInformation, FileStatInformation, Handle, Overlapped, OwnedHandle, Win32Error, WIN_TIME_EPOCH};
use crate::ffi::OsString;
use crate::ignore_error;
use crate::io::{self, Error, Read, Seek, SeekFrom, Write};
use crate::path::{Path, PathBuf};
use crate::prelude::*;
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

pub struct ReadDir {
    buf:   [u8; 4096],
    pos:   usize,
    path:  Arc<PathBuf>,
    owner: File,
    state: ReadState,
}
pub struct Metadata {
    pub attributes:       u32,
    pub creation_time:    i64,
    pub last_access_time: i64,
    pub last_write_time:  i64,
    pub file_size:        u64,
    pub reparse_tag:      u32,
    pub number_of_links:  u32,
    pub file_index:       u64,
    pub access:           u32,
}
pub struct DirEntry {
    pub name: String,
    pub meta: Metadata,
    root:     Arc<PathBuf>,
    owner:    Handle,
}
pub struct FileType {
    attrs:   u32,
    reparse: u32,
}
pub struct FileTimes {
    created:  Option<Time>,
    accessed: Option<Time>,
    modified: Option<Time>,
}
pub struct AsyncFile {
    pos:  u64,
    olp:  Box<Overlapped>,
    file: File,
}
pub struct Permissions {
    access:     u32,
    attributes: u32,
}
pub struct OpenOptions {
    opts:   u16,
    share:  u32,
    attrs:  u32,
    access: u32,
}
pub struct DirBuilder(bool);
pub struct File(OwnedHandle);

pub trait FileExt {
    fn seek_write(&mut self, buf: &[u8], offset: u64) -> io::Result<usize>;
    fn seek_read(&mut self, buf: &mut [u8], offset: u64) -> io::Result<usize>;
}
pub trait FileExtra {
    fn access(&self) -> io::Result<u32>;
    fn name(&self) -> io::Result<String>;
    fn path(&self) -> io::Result<PathBuf>;
    fn delete(&mut self) -> io::Result<()>;
    fn attributes(&self) -> io::Result<u32>;
    fn read_dir(&self) -> io::Result<ReadDir>;
    fn set_system(&self, system: bool) -> io::Result<()>;
    fn set_hidden(&self, hidden: bool) -> io::Result<()>;
    fn set_modified(&self, time: Time) -> io::Result<()>;
    fn set_archive(&self, archive: bool) -> io::Result<()>;
    fn set_attributes(&self, attrs: u32) -> io::Result<()>;
    fn set_times(&self, times: FileTimes) -> io::Result<()>;
    fn set_readonly(&self, readonly: bool) -> io::Result<()>;
}
pub trait FileTypeExt {
    fn is_symlink_dir(&self) -> bool;
    fn is_symlink_file(&self) -> bool;
}
pub trait MetadataExt {
    fn file_size(&self) -> u64;
    fn creation_time(&self) -> u64;
    fn file_attributes(&self) -> u32;
    fn last_write_time(&self) -> u64;
    fn last_access_time(&self) -> u64;
    fn file_index(&self) -> Option<u64>;
    fn number_of_links(&self) -> Option<u32>;
    fn volume_serial_number(&self) -> Option<u32>;
}
pub trait MetadataExtra {
    fn mode(&self) -> u32;
    fn created_time(&self) -> Option<Time>;
    fn accessed_time(&self) -> Option<Time>;
    fn modified_time(&self) -> Option<Time>;
}
pub trait DirEntryExtra {
    fn len(&self) -> u64;
    fn size(&self) -> u64;
    fn is_dir(&self) -> bool;
    fn is_file(&self) -> bool;
    fn is_symlink(&self) -> bool;
    fn full_name(&self) -> String;
    fn created_time(&self) -> Time;
    fn accessed_time(&self) -> Time;
    fn modified_time(&self) -> Time;
    fn file_attributes(&self) -> u32;
    fn is_symlink_dir(&self) -> bool;
    fn is_symlink_file(&self) -> bool;
    fn open(&self, opts: &OpenOptions) -> io::Result<File>;
}
pub trait PermissionsExt {
    fn set_system(&mut self, system: bool);
    fn set_hidden(&mut self, hidden: bool);
    fn set_archive(&mut self, archive: bool);
    fn set_attributes(&mut self, attrs: u32);
}
pub trait OpenOptionsExt {
    fn share_mode(&mut self, share: u32) -> &mut OpenOptions;
    fn access_mode(&mut self, access: u32) -> &mut OpenOptions;
    fn custom_flags(&mut self, flags: u32) -> &mut OpenOptions;
    fn attributes(&mut self, attributes: u32) -> &mut OpenOptions;
    fn security_qos_flags(&mut self, flags: u32) -> &mut OpenOptions;
}
pub trait FileTimesExtra {
    fn set_created(self, t: Time) -> Self;
    fn set_accessed(self, t: Time) -> Self;
    fn set_modified(self, t: Time) -> Self;
}
pub trait OpenOptionsExtra {
    fn directory(&mut self) -> &mut OpenOptions;
    fn exclusive(&mut self, exclusive: bool) -> &mut OpenOptions;
    fn follow_symlink(&mut self, follow: bool) -> &mut OpenOptions;
    fn synchronous(&mut self, synchronous: bool) -> &mut OpenOptions;
}

enum ReadState {
    Empty,
    Error,
    Filled,
}

impl File {
    #[inline]
    pub fn options() -> OpenOptions {
        OpenOptions::new()
    }
    #[inline]
    pub fn open(path: impl AsRef<Path>) -> io::Result<File> {
        OpenOptions::new().read(true).open(path)
    }
    #[inline]
    pub fn create(path: impl AsRef<Path>) -> io::Result<File> {
        OpenOptions::new().write(true).create(true).truncate(true).open(path)
    }
    #[inline]
    pub fn create_new(path: impl AsRef<Path>) -> io::Result<File> {
        OpenOptions::new().read(true).write(true).create_new(true).open(path)
    }

    #[inline]
    pub fn as_handle(&self) -> Handle {
        *self.0
    }
    #[inline]
    pub fn sync_all(&self) -> io::Result<()> {
        winapi::NtFlushBuffersFile(self).map_err(Error::from)
    }
    #[inline]
    pub fn sync_data(&self) -> io::Result<()> {
        winapi::NtFlushBuffersFile(self).map_err(Error::from)
    }
    #[inline]
    pub fn try_clone(&self) -> io::Result<File> {
        Ok(File(self.0.duplicate()?))
    }
    #[inline]
    pub fn metadata(&self) -> io::Result<Metadata> {
        Metadata::file(self)
    }
    #[inline]
    pub fn set_len(&self, size: u64) -> io::Result<()> {
        // 0x14 - FileEndOfFileInformation
        winapi::NtSetInformationFile(self, 0x14, &size, 8)?;
        Ok(())
    }
    #[inline]
    pub fn set_permissions(&self, perm: Permissions) -> io::Result<()> {
        self.set_attribute(perm.attributes)
    }

    #[inline]
    fn is_file(&self) -> bool {
        let mut a = [0u32, 0u32];
        // 0x23 - FileAttributeTagInformation
        // Always size 8.
        // 0x10 - FILE_ATTRIBUTE_DIRECTORY
        winapi::NtQueryInformationFile(self, 0x23, &mut a, 0x8).map_or(false, |_| a[0] & 0x10 == 0)
    }
    #[inline]
    fn read_dir(self) -> io::Result<ReadDir> {
        ReadDir::new(self)
    }
    #[inline]
    fn set_attribute(&self, attrs: u32) -> io::Result<()> {
        winapi::set_file_attrs_by_handle(self, attrs).map_err(Error::from)
    }
}
impl ReadDir {
    #[inline]
    fn new(f: File) -> io::Result<ReadDir> {
        let mut i = ReadDir {
            buf:   [0u8; 4096],
            pos:   0usize,
            path:  Arc::new(f.path()?),
            owner: f,
            state: ReadState::Error,
        };
        if winapi::NtQueryDirectoryFile(&i.owner, &mut i.buf, 0x25, false, true, None).map_err(Error::from)? > 0 {
            i.state = ReadState::Filled;
        }
        Ok(i)
    }

    #[inline]
    fn refill(&mut self) {
        if matches!(self.state, ReadState::Filled) {
            return;
        }
        if winapi::NtQueryDirectoryFile(&self.owner, &mut self.buf, 0x25, false, false, None).map_or(false, |v| v > 0) {
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
        let b = unsafe { from_raw_parts(&e.file_name[0], e.name_length as usize / 2) };
        return match e.name_length {
            0 => return None,
            2 if b[0] == b'.' as u16 => return None,
            4 if b[0] == b'.' as u16 && b[1] == b'.' as u16 => return None,
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
    #[inline]
    pub fn len(&self) -> u64 {
        self.file_size
    }
    #[inline]
    pub fn is_dir(&self) -> bool {
        !self.is_symlink() && self.attributes & 0x10 != 0
    }
    #[inline]
    pub fn is_file(&self) -> bool {
        !self.is_symlink() && self.attributes & 0x10 == 0
    }
    #[inline]
    pub fn is_symlink(&self) -> bool {
        // 0x00000400 - FILE_ATTRIBUTE_REPARSE_POINT
        // 0x20000000 - SYMLINK_MASK
        self.attributes & 0x400 != 0 && self.reparse_tag & 0x20000000 != 0
    }
    #[inline]
    pub fn file_type(&self) -> FileType {
        FileType {
            attrs:   self.attributes,
            reparse: self.reparse_tag,
        }
    }
    #[inline]
    pub fn permissions(&self) -> Permissions {
        Permissions {
            access:     self.access,
            attributes: self.attributes,
        }
    }
    #[inline]
    pub fn created(&self) -> io::Result<SystemTime> {
        Ok(winapi::time_from_windows_time(self.creation_time).into())
    }
    #[inline]
    pub fn accessed(&self) -> io::Result<SystemTime> {
        Ok(winapi::time_from_windows_time(self.last_access_time).into())
    }
    #[inline]
    pub fn modified(&self) -> io::Result<SystemTime> {
        Ok(winapi::time_from_windows_time(self.last_write_time).into())
    }

    fn file(f: &File) -> io::Result<Metadata> {
        // We're gonna use a tiered approach to this. There's some ways to get this
        // info quickly (FileStatInformation) but we have fallbacks for doing it
        // into multiple calls.
        //
        // FileStatInformation (W10+, but fails for UNC/Shares)
        //
        // (Both below call FileAttributeTagInformation as the All struct does not
        // contain it.)
        // - FileAllInformation (Will fail if the name is +256 chars since we only
        //   allocate 256).
        // - FileAttributeTagInformation
        //
        // -or -
        //
        // - FileBasicInformation
        // - FileStandardInformation
        // - FileInternalInformation
        // - FileAccessInformation
        // - FileAttributeTagInformation
        //
        // StatInfo is the quickest way to get all this info but it's Win10+ only.
        if winapi::is_min_windows_10() {
            let mut a = FileStatInformation::default();
            // NOTE(dij): Stat info is Win10+ only!
            // This fails on mapped drives, so ignore it's errors.
            // 0x44 - FileStatInformation
            if winapi::NtQueryInformationFile(f, 0x44, &mut a, 0x48).is_ok() {
                return Ok(a.into());
            }
        }
        let mut a = FileAllInformation::default();
        // 0x12 - FileAllInformation
        // Always size 624.
        // Ignore errors and fallback if this fails.
        if winapi::NtQueryInformationFile(f, 0x12, &mut a, 0x270).is_ok() {
            // 0x400 - FILE_ATTRIBUTE_REPARSE_POINT
            // Don't lookup data if it's not a Reparse Point
            let r = if a.basic.attributes & 0x400 != 0 {
                let mut v = [0u32, 0u32];
                // 0x23 - FileAttributeTagInformation
                // Always size 8.
                winapi::NtQueryInformationFile(f, 0x23, &mut v, 0x8)?;
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
                creation_time:    a.basic.creation_time,
                number_of_links:  a.standard.number_of_links,
                last_write_time:  a.basic.last_write_time,
                last_access_time: a.basic.last_access_time,
            });
        }
        let (mut i, mut a) = (0u64, 0u32);
        let mut b = FileBasicInformation::default();
        let mut s = FileStandardInformation::default();
        // 0x4 - FileBasicInformation
        // Always size 40.
        winapi::NtQueryInformationFile(f, 0x4, &mut b, 0x28)?;
        // 0x5 - FileStandardInformation
        // Always size 32.
        winapi::NtQueryInformationFile(f, 0x5, &mut s, 0x20)?;
        // 0x6 - FileInternalInformation
        // Always size 8.
        winapi::NtQueryInformationFile(f, 0x6, &mut i, 0x8)?;
        // 0x8 - FileAccessInformation
        // Always size 4.
        winapi::NtQueryInformationFile(f, 0x8, &mut a, 0x4)?;
        // 0x400 - FILE_ATTRIBUTE_REPARSE_POINT
        // Don't lookup data if it's not a Reparse Point
        let r = if b.attributes & 0x400 != 0 {
            let mut v = [0u32, 0u32];
            // 0x23 - FileAttributeTagInformation
            // Always size 8.
            winapi::NtQueryInformationFile(f, 0x23, &mut v, 0x8)?;
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
            creation_time:    b.creation_time,
            last_access_time: b.last_access_time,
            last_write_time:  b.last_write_time,
            number_of_links:  s.number_of_links,
        })
    }
}
impl DirEntry {
    #[inline]
    pub fn path(&self) -> PathBuf {
        self.root.join(&self.name)
    }
    #[inline]
    pub fn file_name(&self) -> OsString {
        OsString::from(&self.name)
    }
    #[inline]
    pub fn metadata(&self) -> io::Result<Metadata> {
        Ok(self.meta.clone())
    }
    #[inline]
    pub fn file_type(&self) -> io::Result<FileType> {
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
    #[inline]
    pub fn is_dir(&self) -> bool {
        self.attrs & 0x10 != 0
    }
    #[inline]
    pub fn is_file(&self) -> bool {
        self.attrs & 0x10 == 0
    }
    #[inline]
    pub fn is_symlink(&self) -> bool {
        self.attrs & 0x400 != 0 && self.reparse & 0x20000000 != 0
    }
}
impl AsyncFile {
    #[inline]
    fn new(f: File) -> io::Result<AsyncFile> {
        let mut o = Box::new(Overlapped::default());
        o.event = Handle::take(winapi::CreateEvent(None, false, false, true, None)?);
        Ok(AsyncFile { file: f, pos: 0u64, olp: o })
    }

    #[inline]
    pub fn total(&self) -> u64 {
        self.pos
    }
    #[inline]
    pub fn event(&self) -> Handle {
        self.olp.event
    }
    #[inline]
    pub fn finish(&mut self, m: usize) -> io::Result<()> {
        loop {
            let r = some_or_continue!(self.update(true)?);
            println!("loop {r}");
            if r == m {
                break;
            }
        }
        Ok(())
    }
    #[inline]
    pub fn update(&mut self, wait: bool) -> io::Result<Option<usize>> {
        match winapi::GetOverlappedResult(&self.file, &self.olp, wait) {
            Ok(n) => {
                self.pos += n as u64;
                Ok(Some(n))
            },
            Err(Win32Error::IoPending) => Ok(None),
            Err(Win32Error::BrokenPipe) => Ok(Some(0)),
            Err(e) => Err(e.into()),
        }
    }
    pub fn write_async(&mut self, size: usize, buf: &[u8]) -> io::Result<bool> {
        if self.update(false)?.is_some() && (self.pos as usize) >= buf.len() {
            return Ok(true);
        }
        println!("enter");
        loop {
            if (self.pos as usize) >= buf.len() {
                return Ok(true);
            }
            let i = self.pos as usize;
            let m = cmp::min(i + size, buf.len());
            println!("write? pos {i} to {m}");
            self.pos += match winapi::NtWriteFile(
                &mut self.file,
                Some(&mut self.olp),
                &buf[i..m],
                Some(self.pos),
            ) {
                Ok(v) if v == 0 => return Ok(true),
                Ok(v) => v as u64,
                Err(e) if e == Win32Error::IoPending => return Ok(false),
                Err(e) => return Err(e.into()),
            };
        }
    }
    pub fn read_async(&mut self, size: usize, buf: &mut Vec<u8>) -> io::Result<bool> {
        match self.update(false)? {
            Some(v) if v == 0 && self.pos > 0 => return Ok(true),
            Some(v) => {
                let i = buf.len();
                buf.truncate(i - (size - v));
            },
            _ => (),
        }
        loop {
            let n = buf.len();
            buf.resize(n + size, 0);
            let i = match winapi::NtReadFile(
                &mut self.file,
                Some(&mut self.olp),
                &mut buf[n..n + size],
                Some(self.pos),
            ) {
                Ok(v) if v == 0 => return Ok(true),
                Ok(v) => v,
                Err(e) if e == Win32Error::IoPending => return Ok(false),
                Err(e) => return Err(e.into()),
            };
            buf.truncate(n + i);
        }
    }
}
impl FileTimes {
    #[inline]
    pub const fn new() -> FileTimes {
        FileTimes {
            created:  None,
            accessed: None,
            modified: None,
        }
    }
}
impl DirBuilder {
    #[inline]
    pub fn new() -> DirBuilder {
        DirBuilder(false)
    }

    #[inline]
    pub fn create(&self, path: impl AsRef<Path>) -> io::Result<()> {
        winapi::CreateDirectory(path.as_ref().to_string_lossy(), self.0).map_err(Error::from)
    }
    #[inline]
    pub fn recursive(&mut self, recursive: bool) -> &mut DirBuilder {
        self.0 = recursive;
        self
    }
}
impl OpenOptions {
    #[inline]
    pub fn new() -> OpenOptions {
        OpenOptions {
            opts:   SYNCHRONOUS,
            share:  0u32,
            attrs:  0u32,
            access: 0u32,
        }
    }

    #[inline]
    pub fn read(&mut self, read: bool) -> &mut OpenOptions {
        if read {
            self.opts |= READ;
        } else {
            self.opts &= READ;
        }
        self
    }
    #[inline]
    pub fn write(&mut self, write: bool) -> &mut OpenOptions {
        if write {
            self.opts |= WRITE;
        } else {
            self.opts &= WRITE;
        }
        self
    }
    #[inline]
    pub fn delete(&mut self, delete: bool) -> &mut OpenOptions {
        if delete {
            self.opts |= DELETE;
        } else {
            self.opts &= DELETE;
        }
        self
    }
    #[inline]
    pub fn append(&mut self, append: bool) -> &mut OpenOptions {
        if append {
            self.opts |= APPEND;
        } else {
            self.opts &= APPEND;
        }
        self
    }
    #[inline]
    pub fn create(&mut self, create: bool) -> &mut OpenOptions {
        if create {
            self.opts |= CREATE;
        } else {
            self.opts &= CREATE;
        }
        self
    }
    #[inline]
    pub fn truncate(&mut self, truncate: bool) -> &mut OpenOptions {
        if truncate {
            self.opts |= TRUNCATE;
        } else {
            self.opts &= TRUNCATE;
        }
        self
    }
    #[inline]
    pub fn open(&self, path: impl AsRef<Path>) -> io::Result<File> {
        self.open_inner(Handle::INVALID, 0, path)
    }
    #[inline]
    pub fn create_new(&mut self, create_new: bool) -> &mut OpenOptions {
        if create_new {
            self.opts |= CREATE_NEW;
        } else {
            self.opts &= CREATE_NEW;
        }
        self
    }
    #[inline]
    pub fn open_async(&self, path: impl AsRef<Path>) -> io::Result<AsyncFile> {
        AsyncFile::new(self.open_inner(Handle::INVALID, 0x40000000, path)?)
    }

    fn open_inner(&self, parent: Handle, add_attrs: u32, path: impl AsRef<Path>) -> io::Result<File> {
        let (a, b) = if self.access > 0 {
            (self.access, 0u32)
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
        let (h, j, k, l) = winapi::std_flags_to_nt(
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
        Ok(File(
            winapi::NtCreateFile(
                path.as_ref().to_string_lossy(),
                parent,
                h,
                None,
                j,
                self.share | if self.opts & EXCLUSIVE > 0 { 0 } else { b },
                k,
                l,
            )
            .map_err(Error::from)?,
        ))
    }
}
impl Permissions {
    #[inline]
    pub fn readonly(&self) -> bool {
        // 0x1 - FILE_ATTRIBUTE_READONLY
        self.attributes & 0x1 != 0
    }
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
    fn stream_len(&mut self) -> io::Result<u64> {
        let mut s = FileStandardInformation::default();
        // 0x5 - FileStandardInformation
        // Always size 32.
        winapi::NtQueryInformationFile(self, 0x5, &mut s, 0x20)?;
        Ok(s.end_of_file)
    }
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        let mut n: u64 = 0;
        // 0xE - FilePositionInformation
        winapi::NtQueryInformationFile(self, 0xE, &mut n, 8)?;
        Ok(n)
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let (w, n) = match pos {
            SeekFrom::End(n) => (2, n),
            SeekFrom::Start(n) => (0, n as i64),
            SeekFrom::Current(n) => (1, n),
        };
        winapi::SetFilePointerEx(self, n, w).map_err(Error::from)
    }
}
impl Read for File {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        winapi::NtReadFile(self, None, buf, None).map_err(Error::from)
    }
}
impl Write for File {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        winapi::NtFlushBuffersFile(self).map_err(Error::from)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        winapi::NtWriteFile(self, None, buf, None).map_err(Error::from)
    }
}
impl Seek for &File {
    #[inline]
    fn stream_len(&mut self) -> io::Result<u64> {
        let mut s = FileStandardInformation::default();
        // 0x5 - FileStandardInformation
        // Always size 32.
        winapi::NtQueryInformationFile(self, 0x5, &mut s, 0x20)?;
        Ok(s.end_of_file)
    }
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        let mut n = 0u64;
        // 0xE - FilePositionInformation
        winapi::NtQueryInformationFile(self, 0xE, &mut n, 8)?;
        Ok(n)
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let (w, n) = match pos {
            SeekFrom::End(n) => (2, n),
            SeekFrom::Start(n) => (0, n as i64),
            SeekFrom::Current(n) => (1, n),
        };
        winapi::SetFilePointerEx(self, n, w).map_err(Error::from)
    }
}
impl Read for &File {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        winapi::NtReadFile(self, None, buf, None).map_err(Error::from)
    }
}
impl Write for &File {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        winapi::NtFlushBuffersFile(self).map_err(Error::from)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        winapi::NtWriteFile(self, None, buf, None).map_err(Error::from)
    }
}
impl FileExt for File {
    #[inline]
    fn seek_write(&mut self, buf: &[u8], offset: u64) -> io::Result<usize> {
        winapi::NtWriteFile(self, None, buf, Some(offset)).map_err(Error::from)
    }
    #[inline]
    fn seek_read(&mut self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        winapi::NtReadFile(self, None, buf, Some(offset)).map_err(Error::from)
    }
}
impl FileExt for &File {
    #[inline]
    fn seek_write(&mut self, buf: &[u8], offset: u64) -> io::Result<usize> {
        winapi::NtWriteFile(self, None, buf, Some(offset)).map_err(Error::from)
    }
    #[inline]
    fn seek_read(&mut self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        winapi::NtReadFile(self, None, buf, Some(offset)).map_err(Error::from)
    }
}
impl AsHandle for File {
    #[inline]
    fn as_handle(&self) -> Handle {
        *self.0
    }
}
impl FileExtra for File {
    #[inline]
    fn access(&self) -> io::Result<u32> {
        let mut i = 0u32;
        // 0x8 - FileBasicInformation
        winapi::NtQueryInformationFile(self, 0x4, &mut i, 0x4)?;
        Ok(i)
    }
    #[inline]
    fn name(&self) -> io::Result<String> {
        winapi::file_name_by_handle(self).map_err(Error::from)
    }
    #[inline]
    fn path(&self) -> io::Result<PathBuf> {
        Ok(self.name()?.into())
    }
    #[inline]
    fn delete(&mut self) -> io::Result<()> {
        // 0xD - FileDispositionInformation
        winapi::NtSetInformationFile::<u32>(&self, 0xD, &1, 4)?;
        winapi::CloseHandle(*self.0)?;
        self.0.set(0);
        Ok(())
    }
    #[inline]
    fn attributes(&self) -> io::Result<u32> {
        let mut i = FileBasicInformation::default();
        // 0x4 - FileBasicInformation
        winapi::NtQueryInformationFile(self, 0x4, &mut i, 0x28)?;
        Ok(i.attributes)
    }
    #[inline]
    fn read_dir(&self) -> io::Result<ReadDir> {
        ReadDir::new(self.try_clone()?)
    }
    #[inline]
    fn set_system(&self, system: bool) -> io::Result<()> {
        let mut f = self.attributes()?;
        match f {
            _ if system && f & 0x4 != 0 => return Ok(()),
            _ if !system && f & 0x4 == 0 => return Ok(()),
            _ if system => f |= 0x4,  // 0x4 - FILE_ATTRIBUTE_SYSTEM
            _ if !system => f ^= 0x4, // 0x4 - FILE_ATTRIBUTE_SYSTEM
            _ => (),
        };
        self.set_attributes(f)
    }
    #[inline]
    fn set_hidden(&self, hidden: bool) -> io::Result<()> {
        let mut f = self.attributes()?;
        match f {
            _ if hidden && f & 0x2 != 0 => return Ok(()),
            _ if !hidden && f & 0x2 == 0 => return Ok(()),
            _ if hidden => f |= 0x2,  // 0x2 - FILE_ATTRIBUTE_HIDDEN
            _ if !hidden => f ^= 0x2, // 0x2 - FILE_ATTRIBUTE_HIDDEN
            _ => (),
        };
        self.set_attribute(f)
    }
    #[inline]
    fn set_modified(&self, time: Time) -> io::Result<()> {
        winapi::set_file_time_by_handle(self, None, Some(time), None).map_err(Error::from)
    }
    #[inline]
    fn set_archive(&self, archive: bool) -> io::Result<()> {
        let mut f = self.attributes()?;
        match f {
            _ if archive && f & 0x20 != 0 => return Ok(()),
            _ if !archive && f & 0x20 == 0 => return Ok(()),
            _ if archive => f |= 0x20,  // 0x20 - FILE_ATTRIBUTE_ARCHIVE
            _ if !archive => f ^= 0x20, // 0x20 - FILE_ATTRIBUTE_ARCHIVE
            _ => (),
        };
        self.set_attributes(f)
    }
    #[inline]
    fn set_attributes(&self, attrs: u32) -> io::Result<()> {
        self.set_attribute(attrs)
    }
    #[inline]
    fn set_times(&self, times: FileTimes) -> io::Result<()> {
        winapi::set_file_time_by_handle(self, times.created, times.modified, times.accessed).map_err(Error::from)
    }
    #[inline]
    fn set_readonly(&self, readonly: bool) -> io::Result<()> {
        let mut f = self.attributes()?;
        match f {
            _ if readonly && f & 0x1 != 0 => return Ok(()),
            _ if !readonly && f & 0x1 == 0 => return Ok(()),
            _ if readonly => f |= 0x1,  // 0x1 - FILE_ATTRIBUTE_READONLY
            _ if !readonly => f ^= 0x1, // 0x1 - FILE_ATTRIBUTE_READONLY
            _ => (),
        };
        self.set_attributes(f)
    }
}
impl FileExt for &mut File {
    #[inline]
    fn seek_write(&mut self, buf: &[u8], offset: u64) -> io::Result<usize> {
        winapi::NtWriteFile(&self.0, None, buf, Some(offset)).map_err(Error::from)
    }
    #[inline]
    fn seek_read(&mut self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        winapi::NtReadFile(&self.0, None, buf, Some(offset)).map_err(Error::from)
    }
}
impl AsHandle for &mut File {
    #[inline]
    fn as_handle(&self) -> Handle {
        *self.0
    }
}
impl AsHandle for &mut &File {
    #[inline]
    fn as_handle(&self) -> Handle {
        *self.0
    }
}
impl From<OwnedHandle> for File {
    #[inline]
    fn from(v: OwnedHandle) -> File {
        File(v)
    }
}

impl Seek for AsyncFile {
    #[inline]
    fn stream_len(&mut self) -> io::Result<u64> {
        let mut s = FileStandardInformation::default();
        // 0x5 - FileStandardInformation
        // Always size 32.
        winapi::NtQueryInformationFile(self, 0x5, &mut s, 0x20)?;
        Ok(s.end_of_file)
    }
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        let mut n: u64 = 0;
        // 0xE - FilePositionInformation
        winapi::NtQueryInformationFile(self, 0xE, &mut n, 8)?;
        Ok(n)
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let (w, n) = match pos {
            SeekFrom::End(n) => (2, n),
            SeekFrom::Start(n) => (0, n as i64),
            SeekFrom::Current(n) => (1, n),
        };
        winapi::SetFilePointerEx(self, n, w).map_err(Error::from)
    }
}
impl Read for AsyncFile {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        winapi::NtReadFile(&mut self.file, Some(&mut self.olp), buf, Some(self.pos)).map_err(Error::from)
    }
}
impl Drop for AsyncFile {
    #[inline]
    fn drop(&mut self) {
        ignore_error!(winapi::CloseHandle(self.olp.event));
    }
}
impl Write for AsyncFile {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        winapi::NtFlushBuffersFile(self).map_err(Error::from)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        winapi::NtWriteFile(&mut self.file, Some(&mut self.olp), buf, Some(self.pos)).map_err(Error::from)
    }
}
impl Seek for &AsyncFile {
    #[inline]
    fn stream_len(&mut self) -> io::Result<u64> {
        let mut s = FileStandardInformation::default();
        // 0x5 - FileStandardInformation
        // Always size 32.
        winapi::NtQueryInformationFile(self, 0x5, &mut s, 0x20)?;
        Ok(s.end_of_file)
    }
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        let mut n = 0u64;
        // 0xE - FilePositionInformation
        winapi::NtQueryInformationFile(self, 0xE, &mut n, 8)?;
        Ok(n)
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let (w, n) = match pos {
            SeekFrom::End(n) => (2, n),
            SeekFrom::Start(n) => (0, n as i64),
            SeekFrom::Current(n) => (1, n),
        };
        winapi::SetFilePointerEx(self, n, w).map_err(Error::from)
    }
}
impl Deref for AsyncFile {
    type Target = File;

    #[inline]
    fn deref(&self) -> &File {
        &self.file
    }
}
impl FileExt for AsyncFile {
    #[inline]
    fn seek_write(&mut self, buf: &[u8], offset: u64) -> io::Result<usize> {
        winapi::NtWriteFile(&mut self.file, Some(&mut self.olp), buf, Some(offset)).map_err(Error::from)
    }
    #[inline]
    fn seek_read(&mut self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        winapi::NtReadFile(&mut self.file, Some(&mut self.olp), buf, Some(offset)).map_err(Error::from)
    }
}
impl DerefMut for AsyncFile {
    #[inline]
    fn deref_mut(&mut self) -> &mut File {
        &mut self.file
    }
}
impl AsHandle for AsyncFile {
    #[inline]
    fn as_handle(&self) -> Handle {
        *self.file.0
    }
}
impl FileExtra for AsyncFile {
    #[inline]
    fn access(&self) -> io::Result<u32> {
        let mut i = 0u32;
        // 0x8 - FileBasicInformation
        winapi::NtQueryInformationFile(self, 0x4, &mut i, 0x4)?;
        Ok(i)
    }
    #[inline]
    fn name(&self) -> io::Result<String> {
        winapi::file_name_by_handle(self).map_err(Error::from)
    }
    #[inline]
    fn path(&self) -> io::Result<PathBuf> {
        Ok(self.name()?.into())
    }
    #[inline]
    fn delete(&mut self) -> io::Result<()> {
        // 0xD - FileDispositionInformation
        winapi::NtSetInformationFile::<u32>(&self, 0xD, &1, 4)?;
        winapi::CloseHandle(*(self.file.0))?;
        self.file.0.set(0);
        Ok(())
    }
    #[inline]
    fn attributes(&self) -> io::Result<u32> {
        let mut i = FileBasicInformation::default();
        // 0x4 - FileBasicInformation
        winapi::NtQueryInformationFile(self, 0x4, &mut i, 0x28)?;
        Ok(i.attributes)
    }
    #[inline]
    fn read_dir(&self) -> io::Result<ReadDir> {
        ReadDir::new(self.file.try_clone()?)
    }
    #[inline]
    fn set_system(&self, system: bool) -> io::Result<()> {
        let mut f = self.attributes()?;
        match f {
            _ if system && f & 0x4 != 0 => return Ok(()),
            _ if !system && f & 0x4 == 0 => return Ok(()),
            _ if system => f |= 0x4,  // 0x4 - FILE_ATTRIBUTE_SYSTEM
            _ if !system => f ^= 0x4, // 0x4 - FILE_ATTRIBUTE_SYSTEM
            _ => (),
        };
        self.file.set_attributes(f)
    }
    #[inline]
    fn set_hidden(&self, hidden: bool) -> io::Result<()> {
        let mut f = self.attributes()?;
        match f {
            _ if hidden && f & 0x2 != 0 => return Ok(()),
            _ if !hidden && f & 0x2 == 0 => return Ok(()),
            _ if hidden => f |= 0x2,  // 0x2 - FILE_ATTRIBUTE_HIDDEN
            _ if !hidden => f ^= 0x2, // 0x2 - FILE_ATTRIBUTE_HIDDEN
            _ => (),
        };
        self.file.set_attribute(f)
    }
    #[inline]
    fn set_modified(&self, time: Time) -> io::Result<()> {
        winapi::set_file_time_by_handle(self, None, Some(time), None).map_err(Error::from)
    }
    #[inline]
    fn set_archive(&self, archive: bool) -> io::Result<()> {
        let mut f = self.attributes()?;
        match f {
            _ if archive && f & 0x20 != 0 => return Ok(()),
            _ if !archive && f & 0x20 == 0 => return Ok(()),
            _ if archive => f |= 0x20,  // 0x20 - FILE_ATTRIBUTE_ARCHIVE
            _ if !archive => f ^= 0x20, // 0x20 - FILE_ATTRIBUTE_ARCHIVE
            _ => (),
        };
        self.set_attributes(f)
    }
    #[inline]
    fn set_attributes(&self, attrs: u32) -> io::Result<()> {
        self.file.set_attribute(attrs)
    }
    #[inline]
    fn set_times(&self, times: FileTimes) -> io::Result<()> {
        winapi::set_file_time_by_handle(self, times.created, times.modified, times.accessed).map_err(Error::from)
    }
    #[inline]
    fn set_readonly(&self, readonly: bool) -> io::Result<()> {
        let mut f = self.attributes()?;
        match f {
            _ if readonly && f & 0x1 != 0 => return Ok(()),
            _ if !readonly && f & 0x1 == 0 => return Ok(()),
            _ if readonly => f |= 0x1,  // 0x1 - FILE_ATTRIBUTE_READONLY
            _ if !readonly => f ^= 0x1, // 0x1 - FILE_ATTRIBUTE_READONLY
            _ => (),
        };
        self.set_attributes(f)
    }
}
impl FileExt for &mut AsyncFile {
    #[inline]
    fn seek_write(&mut self, buf: &[u8], offset: u64) -> io::Result<usize> {
        winapi::NtWriteFile(&mut self.file, Some(&mut self.olp), buf, Some(offset)).map_err(Error::from)
    }
    #[inline]
    fn seek_read(&mut self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        winapi::NtReadFile(&mut self.file, Some(&mut self.olp), buf, Some(offset)).map_err(Error::from)
    }
}
impl AsHandle for &mut AsyncFile {
    #[inline]
    fn as_handle(&self) -> Handle {
        *self.file.0
    }
}
impl AsHandle for &mut &AsyncFile {
    #[inline]
    fn as_handle(&self) -> Handle {
        *self.file.0
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
impl FileTypeExt for FileType {
    #[inline]
    fn is_symlink_dir(&self) -> bool {
        self.is_symlink() && self.is_dir()
    }
    #[inline]
    fn is_symlink_file(&self) -> bool {
        self.is_symlink() && self.is_file()
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
impl PermissionsExt for Permissions {
    #[inline]
    fn set_system(&mut self, system: bool) {
        // 0x4 - FILE_ATTRIBUTE_SYSTEM
        self.attributes = if system {
            self.attributes | 0x4
        } else {
            self.attributes ^ &0x4
        }
    }
    #[inline]
    fn set_hidden(&mut self, hidden: bool) {
        // 0x2 - FILE_ATTRIBUTE_HIDDEN
        self.attributes = if hidden {
            self.attributes | 0x2
        } else {
            self.attributes ^ &0x2
        }
    }
    #[inline]
    fn set_archive(&mut self, archive: bool) {
        // 0x20 - FILE_ATTRIBUTE_ARCHIVE
        self.attributes = if archive {
            self.attributes | 0x20
        } else {
            self.attributes ^ &0x20
        }
    }
    #[inline]
    fn set_attributes(&mut self, attrs: u32) {
        self.attributes = attrs
    }
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    #[inline]
    fn next(&mut self) -> Option<io::Result<DirEntry>> {
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
impl OpenOptionsExt for OpenOptions {
    #[inline]
    fn share_mode(&mut self, share: u32) -> &mut OpenOptions {
        self.share = share;
        self
    }
    #[inline]
    fn access_mode(&mut self, access: u32) -> &mut OpenOptions {
        self.access = access;
        self
    }
    #[inline]
    fn custom_flags(&mut self, flags: u32) -> &mut OpenOptions {
        self.attrs |= flags;
        self
    }
    #[inline]
    fn attributes(&mut self, attributes: u32) -> &mut OpenOptions {
        self.attrs = attributes;
        self
    }
    #[inline]
    fn security_qos_flags(&mut self, flags: u32) -> &mut OpenOptions {
        self.attrs |= flags;
        self
    }
}
impl OpenOptionsExtra for OpenOptions {
    #[inline]
    fn directory(&mut self) -> &mut OpenOptions {
        self.attrs |= 0x2200000; // FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT
        self
    }
    #[inline]
    fn exclusive(&mut self, exclusive: bool) -> &mut OpenOptions {
        if exclusive {
            self.opts |= EXCLUSIVE;
        } else {
            self.opts &= EXCLUSIVE;
        }
        self
    }
    #[inline]
    fn follow_symlink(&mut self, follow: bool) -> &mut OpenOptions {
        if follow {
            self.opts &= NO_SYMLINK;
        } else {
            self.opts |= NO_SYMLINK;
        }
        self
    }
    #[inline]
    fn synchronous(&mut self, synchronous: bool) -> &mut OpenOptions {
        if synchronous {
            self.opts |= SYNCHRONOUS;
        } else {
            self.opts &= SYNCHRONOUS;
        }
        self
    }
}

impl From<File> for Handle {
    fn from(v: File) -> Handle {
        Handle::take(v.0)
    }
}
impl From<File> for OwnedHandle {
    #[inline]
    fn from(v: File) -> OwnedHandle {
        v.0
    }
}

impl DirEntryExtra for DirEntry {
    #[inline]
    fn len(&self) -> u64 {
        self.meta.file_size
    }
    #[inline]
    fn size(&self) -> u64 {
        self.meta.file_size
    }
    #[inline]
    fn is_dir(&self) -> bool {
        self.meta.is_dir()
    }
    #[inline]
    fn is_file(&self) -> bool {
        !self.meta.is_dir()
    }
    #[inline]
    fn is_symlink(&self) -> bool {
        self.meta.is_symlink()
    }
    #[inline]
    fn full_name(&self) -> String {
        self.path().to_string_lossy().to_string()
    }
    #[inline]
    fn created_time(&self) -> Time {
        Time::from_nano((self.meta.creation_time - WIN_TIME_EPOCH) * 100)
    }
    #[inline]
    fn accessed_time(&self) -> Time {
        Time::from_nano((self.meta.last_access_time - WIN_TIME_EPOCH) * 100)
    }
    #[inline]
    fn modified_time(&self) -> Time {
        Time::from_nano((self.meta.last_write_time - WIN_TIME_EPOCH) * 100)
    }
    #[inline]
    fn file_attributes(&self) -> u32 {
        self.meta.attributes
    }
    #[inline]
    fn is_symlink_dir(&self) -> bool {
        self.meta.is_symlink() && self.meta.attributes & 0x10 != 0
    }
    #[inline]
    fn is_symlink_file(&self) -> bool {
        self.meta.is_symlink() && self.meta.attributes & 0x10 == 0
    }
    #[inline]
    fn open(&self, opts: &OpenOptions) -> io::Result<File> {
        opts.open_inner(self.owner, 0, &self.name)
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
            creation_time:    self.creation_time,
            last_write_time:  self.last_write_time,
            number_of_links:  self.number_of_links,
            last_access_time: self.last_access_time,
        }
    }
}
impl MetadataExt for Metadata {
    #[inline]
    fn file_size(&self) -> u64 {
        self.file_size
    }
    #[inline]
    fn creation_time(&self) -> u64 {
        self.creation_time as u64
    }
    #[inline]
    fn file_attributes(&self) -> u32 {
        self.attributes
    }
    #[inline]
    fn last_write_time(&self) -> u64 {
        self.last_write_time as u64
    }
    #[inline]
    fn last_access_time(&self) -> u64 {
        self.last_access_time as u64
    }
    #[inline]
    fn file_index(&self) -> Option<u64> {
        Some(self.file_index)
    }
    #[inline]
    fn number_of_links(&self) -> Option<u32> {
        Some(self.number_of_links)
    }
    #[inline]
    fn volume_serial_number(&self) -> Option<u32> {
        None
    }
}
impl MetadataExtra for Metadata {
    #[inline]
    fn mode(&self) -> u32 {
        let mut m = if self.is_symlink() { 0x8000000u32 } else { 0u32 };
        if self.is_dir() {
            m |= 0x80000000;
        }
        if self.access == 0 {
            return m | 0x1B4;
        }
        // 0x80000000 - GENERIC_READ
        if self.access & 0x80000000 != 0 {
            m |= 0x124;
        }
        // 0x40000000 - GENERIC_WRITE
        if self.access & 0x40000000 != 0 {
            m |= 0x92;
        }
        // 0x20000000 - GENERIC_EXECUTE
        if self.access & 0x20000000 != 0 {
            m |= 0x49;
        }
        // 0x10000000 - GENERIC_ALL
        if self.access & 0x10000000 != 0 {
            m |= 0x1FF;
        }
        // 0x00020000 - STANDARD_RIGHTS_READ
        if self.access & 0x20000 != 0 {
            m |= 0x180;
        }
        // 0x1 - FILE_READ_DATA
        if self.access & 0x1 != 0 {
            m |= 0x100;
        }
        // 0x2 - FILE_WRITE_DATA
        // 0x4 - FILE_APPEND_DATA
        if self.access & 0x6 != 0 {
            m |= 0x80;
        }
        // 0x20 - FILE_EXECUTE
        if self.access & 0x20 != 0 {
            m |= 0x40;
        }
        // 0x1F01FF - FILE_ALL_ACCESS
        if self.access & 0x1F01FF != 0 {
            m |= 0x1C0
        }
        m
    }
    #[inline]
    fn created_time(&self) -> Option<Time> {
        Some(winapi::time_from_windows_time(self.creation_time))
    }
    #[inline]
    fn accessed_time(&self) -> Option<Time> {
        Some(winapi::time_from_windows_time(self.last_access_time))
    }
    #[inline]
    fn modified_time(&self) -> Option<Time> {
        Some(winapi::time_from_windows_time(self.last_write_time))
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
impl FileTimesExtra for FileTimes {
    #[inline]
    fn set_created(self, t: Time) -> FileTimes {
        FileTimes {
            created:  Some(t),
            accessed: self.accessed,
            modified: self.modified,
        }
    }
    #[inline]
    fn set_accessed(self, t: Time) -> FileTimes {
        FileTimes {
            created:  self.created,
            accessed: Some(t),
            modified: self.modified,
        }
    }
    #[inline]
    fn set_modified(self, t: Time) -> FileTimes {
        FileTimes {
            created:  self.created,
            accessed: self.accessed,
            modified: Some(t),
        }
    }
}

#[inline]
pub fn exists(path: impl AsRef<Path>) -> bool {
    File::open(path).map_or(false, |f| f.is_file())
}
#[inline]
pub fn dirname(path: impl AsRef<str>) -> String {
    match path.as_ref().bytes().position(|x| x == b'\\' || x == b'/') {
        Some(i) => path.as_ref()[0..i].to_string(),
        None => path.as_ref().to_string(),
    }
}
#[inline]
pub fn basename(path: impl AsRef<str>) -> String {
    match path.as_ref().as_bytes().iter().rposition(|x| *x == b'\\' || *x == b'/') {
        Some(i) => {
            if i + 1 > path.as_ref().len() {
                path.as_ref().to_string()
            } else {
                path.as_ref()[i + 1..].to_string()
            }
        },
        None => path.as_ref().to_string(),
    }
}
#[inline]
pub fn normalize(path: impl AsRef<str>) -> String {
    winapi::normalize_path_to_dos(path)
}
#[inline]
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let mut f = File::open(path)?;
    let mut b = Vec::with_capacity(f.metadata().map(|m| m.file_size).unwrap_or_default() as usize);
    f.read_to_end(&mut b)?;
    Ok(b)
}
#[inline]
pub fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
    DirBuilder::new().create(path.as_ref())
}
#[inline]
pub fn remove_dir(path: impl AsRef<Path>) -> io::Result<()> {
    OpenOptions::new()
        .directory()
        .follow_symlink(false)
        .delete(true)
        .open(path)?
        .delete()
}
#[inline]
pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    winapi::DeleteFile(path.as_ref().to_string_lossy()).map_err(Error::from)
}
#[inline]
pub fn read_dir(path: impl AsRef<Path>) -> io::Result<ReadDir> {
    OpenOptions::new()
        .follow_symlink(false)
        .read(true)
        .directory()
        .open(path)?
        .read_dir()
}
#[inline]
pub fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let mut o = OpenOptions::new();
    let b = o.directory().follow_symlink(false).delete(true).open(path)?;
    remove_dir_inner(&b, &o)
}
#[inline]
pub fn metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    OpenOptions::new().read(true).directory().open(path)?.metadata()
}
#[inline]
pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    DirBuilder::new().recursive(true).create(path.as_ref())
}
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    let mut f = File::open(path)?;
    let s = f.metadata().map(|m| m.file_size).unwrap_or_default();
    let mut b = String::with_capacity(s as usize);
    unsafe { f.read_exact(b.as_bytes_mut())? };
    Ok(b)
}
#[inline]
pub fn symlink_metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    OpenOptions::new().read(true).follow_symlink(false).open(path)?.metadata()
}
#[inline]
pub fn set_attributes(path: impl AsRef<Path>, attrs: u32) -> io::Result<()> {
    winapi::SetFileAttributes(path.as_ref().to_string_lossy(), attrs).map_err(Error::from)
}
#[inline]
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
    winapi::CopyFileEx(
        from.as_ref().to_string_lossy(),
        to.as_ref().to_string_lossy(),
        0,
    )
    .map_err(Error::from)
}
#[inline]
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    // 0x1 - MOVEFILE_REPLACE_EXISTING
    winapi::MoveFileEx(
        from.as_ref().to_string_lossy(),
        to.as_ref().to_string_lossy(),
        0x1,
    )
    .map_err(Error::from)
}
#[inline]
pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    File::create(path)?.write_all(contents.as_ref())
}

fn remove_dir_inner(f: &File, o: &OpenOptions) -> io::Result<()> {
    for e in f.read_dir()? {
        // This will never fail as we don't error on ReadDir entries.
        let t = unsafe { e.unwrap_unchecked() };
        let h = t.open(o)?;
        if t.is_dir() {
            remove_dir_inner(&h, o)?;
        }
        winapi::delete_file_by_handle(h).map_err(Error::from)?;
    }
    winapi::delete_file_by_handle(&f.0).map_err(Error::from)
}
