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

extern crate alloc;
extern crate core;

extern crate xrmt_winapi;

use alloc::string::String;
use core::convert::{AsRef, Into};
use core::default::Default;
use core::option::Option::{self, None, Some};
use core::result::Result::Ok;

use xrmt_winapi::functions::{file_name, NtQueryInformationFile, NtReadFile, NtWriteFile};
use xrmt_winapi::structs::FileBasicInformation;

use crate::fs::{DirEntry, File, FileTimes, FileType, Metadata, OpenOptions, Permissions, ReadDir, DELETE, EXCLUSIVE, NO_SYMLINK, SYNCHRONOUS};
use crate::io::IoResult;
use crate::os::windows::fs::{FileExt, FileTimesExt, FileTypeExt, MetadataExt, OpenOptionsExt};
use crate::os::Handle;
use crate::path::PathBuf;
use crate::time::extra::Time;
use crate::time::SystemTime;

pub trait FileExtra {
    fn access(&self) -> IoResult<u32>;
    fn name(&self) -> IoResult<String>;
    fn path(&self) -> IoResult<PathBuf>;
    fn delete(&mut self) -> IoResult<()>;
    fn attributes(&self) -> IoResult<u32>;
    fn read_dir(&self) -> IoResult<ReadDir>;
    fn set_system(&self, system: bool) -> IoResult<()>;
    fn set_hidden(&self, hidden: bool) -> IoResult<()>;
    fn set_archive(&self, archive: bool) -> IoResult<()>;
    fn set_attributes(&self, attrs: u32) -> IoResult<()>;
    fn set_readonly(&self, readonly: bool) -> IoResult<()>;
}
pub trait MetadataExtra {
    fn mode(&self) -> u32;
    fn change_time(&self) -> Option<Time>;
    fn created_time(&self) -> Option<Time>;
    fn last_write_time(&self) -> Option<Time>;
    fn last_access_time(&self) -> Option<Time>;
}
pub trait DirEntryExtra {
    fn len(&self) -> u64;
    fn size(&self) -> u64;
    fn is_dir(&self) -> bool;
    fn is_file(&self) -> bool;
    fn is_symlink(&self) -> bool;
    fn full_name(&self) -> String;
    fn created_time(&self) -> Time;
    fn file_attributes(&self) -> u32;
    fn is_symlink_dir(&self) -> bool;
    fn is_symlink_file(&self) -> bool;
    fn last_write_time(&self) -> Time;
    fn last_access_time(&self) -> Time;
    fn open(&self, opts: &OpenOptions) -> IoResult<File>;
}
pub trait PermissionsExtra {
    fn set_system(&mut self, system: bool);
    fn set_hidden(&mut self, hidden: bool);
    fn set_archive(&mut self, archive: bool);
    fn set_attributes(&mut self, attrs: u32);
}
pub trait OpenOptionsExtra {
    fn directory(&mut self) -> &mut Self;
    fn delete(&mut self, delete: bool) -> &mut Self;
    fn exclusive(&mut self, exclusive: bool) -> &mut Self;
    fn follow_symlink(&mut self, follow: bool) -> &mut Self;
    fn synchronous(&mut self, synchronous: bool) -> &mut Self;
}

impl FileExt for File {
    #[inline]
    fn seek_write(&mut self, buf: &[u8], offset: u64) -> IoResult<usize> {
        Ok(NtWriteFile(self, None, buf, Some(offset))?)
    }
    #[inline]
    fn seek_read(&mut self, buf: &mut [u8], offset: u64) -> IoResult<usize> {
        Ok(NtReadFile(self, None, buf, Some(offset))?)
    }
}
impl FileExt for &File {
    #[inline]
    fn seek_write(&mut self, buf: &[u8], offset: u64) -> IoResult<usize> {
        Ok(NtWriteFile(self, None, buf, Some(offset))?)
    }
    #[inline]
    fn seek_read(&mut self, buf: &mut [u8], offset: u64) -> IoResult<usize> {
        Ok(NtReadFile(self, None, buf, Some(offset))?)
    }
}

impl FileExtra for File {
    #[inline]
    fn access(&self) -> IoResult<u32> {
        let mut i = 0u32;
        // 0x8 - FileBasicInformation
        NtQueryInformationFile(self, 0x4, &mut i, 0x4)?;
        Ok(i)
    }
    #[inline]
    fn name(&self) -> IoResult<String> {
        Ok(file_name(self)?)
    }
    #[inline]
    fn path(&self) -> IoResult<PathBuf> {
        Ok(self.name()?.into())
    }
    #[inline]
    fn delete(&mut self) -> IoResult<()> {
        self.delete()
    }
    #[inline]
    fn attributes(&self) -> IoResult<u32> {
        let mut i = FileBasicInformation::default();
        // 0x4 - FileBasicInformation
        NtQueryInformationFile(self, 0x4, &mut i, 0x28)?;
        Ok(i.attributes)
    }
    #[inline]
    fn read_dir(&self) -> IoResult<ReadDir> {
        ReadDir::new(self.try_clone()?)
    }
    #[inline]
    fn set_system(&self, system: bool) -> IoResult<()> {
        let a = match self.attributes()? {
            v if system && v & 0x4 != 0 => return Ok(()),
            v if !system && v & 0x4 == 0 => return Ok(()),
            v if system => v | 0x4,  // 0x4 - FILE_ATTRIBUTE_SYSTEM
            v if !system => v ^ 0x4, // 0x4 - FILE_ATTRIBUTE_SYSTEM
            v => v,
        };
        self.set_attributes(a)
    }
    #[inline]
    fn set_hidden(&self, hidden: bool) -> IoResult<()> {
        let a = match self.attributes()? {
            v if hidden && v & 0x2 != 0 => return Ok(()),
            v if !hidden && v & 0x2 == 0 => return Ok(()),
            v if hidden => v | 0x2,  // 0x2 - FILE_ATTRIBUTE_HIDDEN
            v if !hidden => v ^ 0x2, // 0x2 - FILE_ATTRIBUTE_HIDDEN
            v => v,
        };
        self.set_attribute(a)
    }
    #[inline]
    fn set_archive(&self, archive: bool) -> IoResult<()> {
        let a = match self.attributes()? {
            v if archive && v & 0x20 != 0 => return Ok(()),
            v if !archive && v & 0x20 == 0 => return Ok(()),
            v if archive => v | 0x20,  // 0x20 - FILE_ATTRIBUTE_ARCHIVE
            v if !archive => v ^ 0x20, // 0x20 - FILE_ATTRIBUTE_ARCHIVE
            v => v,
        };
        self.set_attributes(a)
    }
    #[inline]
    fn set_attributes(&self, attrs: u32) -> IoResult<()> {
        self.set_attribute(attrs)
    }
    #[inline]
    fn set_readonly(&self, readonly: bool) -> IoResult<()> {
        let a = match self.attributes()? {
            v if readonly && v & 0x1 != 0 => return Ok(()),
            v if !readonly && v & 0x1 == 0 => return Ok(()),
            v if readonly => v | 0x1,  // 0x1 - FILE_ATTRIBUTE_READONLY
            v if !readonly => v ^ 0x1, // 0x1 - FILE_ATTRIBUTE_READONLY
            v => v,
        };
        self.set_attributes(a)
    }
}
impl FileExt for &mut File {
    #[inline]
    fn seek_write(&mut self, buf: &[u8], offset: u64) -> IoResult<usize> {
        Ok(NtWriteFile(&self.0, None, buf, Some(offset))?)
    }
    #[inline]
    fn seek_read(&mut self, buf: &mut [u8], offset: u64) -> IoResult<usize> {
        Ok(NtReadFile(&self.0, None, buf, Some(offset))?)
    }
}
impl AsRef<Handle> for File {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}
impl FileExtra for &mut File {
    #[inline]
    fn access(&self) -> IoResult<u32> {
        (**self).access()
    }
    #[inline]
    fn name(&self) -> IoResult<String> {
        (**self).name()
    }
    #[inline]
    fn path(&self) -> IoResult<PathBuf> {
        (**self).path()
    }
    #[inline]
    fn delete(&mut self) -> IoResult<()> {
        (*self).delete()
    }
    #[inline]
    fn attributes(&self) -> IoResult<u32> {
        (**self).attributes()
    }
    #[inline]
    fn read_dir(&self) -> IoResult<ReadDir> {
        ReadDir::new(self.try_clone()?)
    }
    #[inline]
    fn set_system(&self, system: bool) -> IoResult<()> {
        (**self).set_system(system)
    }
    #[inline]
    fn set_hidden(&self, hidden: bool) -> IoResult<()> {
        (**self).set_hidden(hidden)
    }
    #[inline]
    fn set_archive(&self, archive: bool) -> IoResult<()> {
        (**self).set_archive(archive)
    }
    #[inline]
    fn set_attributes(&self, attrs: u32) -> IoResult<()> {
        (**self).set_attribute(attrs)
    }
    #[inline]
    fn set_readonly(&self, readonly: bool) -> IoResult<()> {
        (**self).set_readonly(readonly)
    }
}

impl MetadataExt for Metadata {
    #[inline]
    fn file_size(&self) -> u64 {
        self.file_size
    }
    #[inline]
    fn creation_time(&self) -> u64 {
        *self.creation_time as u64
    }
    #[inline]
    fn file_attributes(&self) -> u32 {
        self.attributes
    }
    #[inline]
    fn last_write_time(&self) -> u64 {
        *self.last_write_time as u64
    }
    #[inline]
    fn last_access_time(&self) -> u64 {
        *self.last_access_time as u64
    }
    #[inline]
    fn file_index(&self) -> Option<u64> {
        Some(self.file_index)
    }
    #[inline]
    fn change_time(&self) -> Option<u64> {
        Some(*self.change_time as u64)
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
        let mut m = if self.is_symlink() { 0x8000000 } else { 0 };
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
    fn change_time(&self) -> Option<Time> {
        Some(self.creation_time.as_time())
    }
    #[inline]
    fn created_time(&self) -> Option<Time> {
        Some(self.creation_time.as_time())
    }
    #[inline]
    fn last_write_time(&self) -> Option<Time> {
        Some(self.last_write_time.as_time())
    }
    #[inline]
    fn last_access_time(&self) -> Option<Time> {
        Some(self.last_access_time.as_time())
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
        self.path().to_string_lossy().into_owned()
    }
    #[inline]
    fn created_time(&self) -> Time {
        self.meta.creation_time.as_time()
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
    fn last_write_time(&self) -> Time {
        self.meta.last_write_time.as_time()
    }
    #[inline]
    fn last_access_time(&self) -> Time {
        self.meta.last_access_time.as_time()
    }
    #[inline]
    fn open(&self, opts: &OpenOptions) -> IoResult<File> {
        opts.open_inner(self.owner, 0, &self.name)
    }
}

impl FileTimesExt for FileTimes {
    #[inline]
    fn set_created(self, t: SystemTime) -> FileTimes {
        FileTimes {
            created:  Some(t),
            accessed: self.accessed,
            modified: self.modified,
        }
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
    fn delete(&mut self, delete: bool) -> &mut OpenOptions {
        if delete {
            self.opts |= DELETE;
        } else {
            self.opts &= !DELETE;
        }
        self
    }
    #[inline]
    fn exclusive(&mut self, exclusive: bool) -> &mut OpenOptions {
        if exclusive {
            self.opts |= EXCLUSIVE;
        } else {
            self.opts &= !EXCLUSIVE;
        }
        self
    }
    #[inline]
    fn follow_symlink(&mut self, follow: bool) -> &mut OpenOptions {
        if follow {
            self.opts &= NO_SYMLINK;
        } else {
            self.opts |= !NO_SYMLINK;
        }
        self
    }
    #[inline]
    fn synchronous(&mut self, synchronous: bool) -> &mut OpenOptions {
        if synchronous {
            self.opts |= SYNCHRONOUS;
        } else {
            self.opts &= !SYNCHRONOUS;
        }
        self
    }
}

impl PermissionsExtra for Permissions {
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
