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
#![cfg(target_family = "windows")]

extern crate core;

use core::cell::UnsafeCell;
use core::clone::Clone;
use core::convert::{AsRef, From};
use core::default::Default;
use core::marker::Copy;
use core::mem::{replace, size_of};
use core::ops::{Deref, DerefMut, Drop};
use core::option::Option::{self, None, Some};
use core::ptr::{null, null_mut, NonNull};
use core::result::Result::{Err, Ok};

use crate::functions::{close_handle, CreateEvent, CreateIoCompletion, GetOverlappedResult, NtSetInformationFile};
use crate::info::is_min_windows_8_1;
use crate::structs::{Handle, OwnedHandle, SecurityAttributes, SecurityDescriptor, SecurityQualityOfService, SysTime, UnicodeString};
use crate::{ntdll, syscall, Win32Error, Win32Result, INFINITE, PTR_SIZE};

#[repr(C)]
pub struct FileTime {
    pub low:  u32,
    pub high: u32,
}
#[repr(C)]
pub struct Overlapped {
    pub internal:      usize,
    pub internal_high: usize,
    pub offset:        u32,
    pub offset_high:   u32,
    pub event:         Handle,
}
#[repr(C)]
pub struct IoStatusBlock {
    pub status: usize,
    pub info:   usize,
}
pub struct IoCompletionPort {
    p: OwnedHandle,
    e: OwnedHandle,
}
#[repr(C)]
pub struct FileIdBothDirInfo {
    pub next_entry:        u32,
    pub file_index:        u32,
    pub creation_time:     SysTime,
    pub last_access_time:  SysTime,
    pub last_write_time:   SysTime,
    pub change_time:       SysTime,
    pub end_of_file:       u64,
    pub allocation_size:   u64,
    pub attributes:        u32,
    pub name_length:       u32,
    pub ea_size:           u32,
    pub short_name_length: u8,
    pub short_name:        [u16; 12],
    pub file_id:           u64,
    pub file_name:         [u16; 1],
}
#[repr(C)]
pub struct FileAllInformation {
    pub basic:          FileBasicInformation,
    pub standard:       FileStandardInformation,
    pub file_id:        u64,
    pub ea_size:        u32,
    pub access:         u32,
    pub current_offset: u64,
    pub mode:           u32,
    pub alignment:      u32,
    pub name_length:    u32,
    pub name:           [u16; 300],
}
#[repr(C)]
pub struct FileStatInformation {
    pub file_id:          u64,
    pub creation_time:    SysTime,
    pub last_access_time: SysTime,
    pub last_write_time:  SysTime,
    pub change_time:      SysTime,
    pub allocation_size:  u64,
    pub end_of_file:      u64,
    pub attributes:       u32,
    pub reparse_tag:      u32,
    pub number_of_links:  u32,
    pub access:           u32,
}
#[repr(C)]
pub struct ObjectAttributes<'a> {
    pub length:              u32,
    pub root_directory:      Handle,
    pub object_name:         Option<&'a UnicodeString<'a>>,
    pub attributes:          u32,
    pub security_descriptor: Option<&'a SecurityDescriptor>,
    pub security_qos:        Option<&'a SecurityQualityOfService>,
}
#[repr(C)]
pub struct FileBasicInformation {
    pub creation_time:    SysTime,
    pub last_access_time: SysTime,
    pub last_write_time:  SysTime,
    pub change_time:      SysTime,
    pub attributes:       u32,
}
#[repr(C)]
pub struct FileRenameInformation {
    pub replace:  u32,
    pub root:     Handle,
    pub name_len: u32,
    pub name:     u16,
}
#[repr(C)]
pub struct ObjectBasicInformation {
    pub attributes:     u32,
    pub access:         u32,
    pub handles:        u32,
    pub pointers:       u32,
    pub paged_pool:     u32,
    pub non_paged_pool: u32,
    pad:                [u32; 3],
    pub name_size:      u32,
    pub type_size:      u32,
    pub sec_desc_size:  u32,
    pub created:        u64,
}
#[repr(C)]
pub struct FileStandardInformation {
    pub allocation_size: u64,
    pub end_of_file:     u64,
    pub number_of_links: u32,
    pub delete_pending:  u32,
    pub is_directory:    u32,
}
#[repr(transparent)]
pub struct OwnedOverlapped(Overlapped);
#[repr(transparent)]
pub struct OverlappedPtr<'a>(UnsafeCell<Option<&'a mut Overlapped>>);

pub type MaybeOverlapped<'a> = Option<&'a mut Overlapped>;

impl FileTime {
    #[inline]
    pub fn as_unix(&self) -> u64 {
        ((self.high as u64) << 32 | self.low as u64).saturating_div(100)
    }
}
impl Overlapped {
    #[inline]
    pub fn wait(&mut self) -> Win32Result<usize> {
        GetOverlappedResult(self.event, self, true)
    }
    #[inline]
    pub fn status_no_wait(&mut self) -> Win32Result<usize> {
        // Clear the status, see the note in 'GetOverlappedResult'
        let r = replace(&mut self.internal_high, 0);
        match self.internal {
            // 0xC0000011 - STATUS_END_OF_FILE
            0xC0000011 => Ok(0),
            0 => Ok(r),
            _ => Err(Win32Error::from_status(self.internal as u32)),
        }
    }
}
impl OwnedOverlapped {
    #[inline]
    pub fn new() -> Win32Result<OwnedOverlapped> {
        let mut v = Overlapped::default();
        v.event = unsafe { Handle::take(CreateEvent(None, false, false, false, None)?) };
        Ok(OwnedOverlapped(v))
    }
}
impl IoCompletionPort {
    #[inline]
    pub fn new() -> Win32Result<IoCompletionPort> {
        Ok(IoCompletionPort {
            e: CreateEvent(None, false, false, false, None)?,
            p: CreateIoCompletion(None, None)?,
        })
    }

    #[inline]
    pub fn event(&self) -> &Handle {
        &self.e
    }
    #[inline]
    pub fn remove(&self, h: impl AsRef<Handle>) -> Win32Result<()> {
        if !is_min_windows_8_1() {
            // FileReplaceCompletionInformation is not avaliable until Win 8.1
            return Ok(());
        }
        let v = [0usize, 0usize];
        // 0x3D - FileReplaceCompletionInformation
        NtSetInformationFile(h, 0x3D, v.as_ptr(), (PTR_SIZE * 2) as u32)?;
        Ok(())
    }
    #[inline]
    pub fn add<T>(&self, h: impl AsRef<Handle>, key: &T) -> Win32Result<()> {
        let v = [self.p.as_usize(), key as *const T as usize];
        // 0x1E - FileCompletionInformation
        NtSetInformationFile(h, 0x1E, v.as_ptr(), (PTR_SIZE * 2) as u32)?;
        Ok(())
    }
    /// Ignores the Overlapped structure. Basically just a wrapper for
    /// 'RemoveIoCompletion'
    #[inline]
    pub fn status<T>(&self, key: &mut NonNull<T>, microseconds: u64) -> Win32Result<usize> {
        let mut o = null_mut();
        self.status_olp(key, &mut o, microseconds)
    }
    /// Captures the raw Overlapped pointer. Basically just a wrapper for
    /// 'RemoveIoCompletion'
    #[inline]
    pub fn status_olp<T>(&self, key: &mut NonNull<T>, olp: &mut *mut Overlapped, microseconds: u64) -> Win32Result<usize> {
        let t = (microseconds as i64).wrapping_mul(-10);
        let mut i = IoStatusBlock::default();
        let r = syscall!(
            ntdll().NtRemoveIoCompletion,
            (Handle, *mut NonNull<T>, *mut *mut Overlapped, *mut IoStatusBlock, *const i64) -> u32,
            *self.p,
            key,
            olp,
            &mut i,
            if microseconds == INFINITE { null() } else { &t }
        );
        if r > 0 {
            Err(Win32Error::from_status(r))
        } else {
            Ok(i.info)
        }
    }
}
impl FileBasicInformation {
    #[inline]
    pub const fn with_attrs(attrs: u32) -> FileBasicInformation {
        FileBasicInformation {
            attributes:       attrs,
            change_time:      SysTime::empty(),
            creation_time:    SysTime::empty(),
            last_write_time:  SysTime::empty(),
            last_access_time: SysTime::empty(),
        }
    }
}
impl<'a> OverlappedPtr<'a> {
    #[inline]
    pub fn new(v: Option<&'a mut Overlapped>) -> OverlappedPtr<'a> {
        OverlappedPtr(UnsafeCell::new(v))
    }
    #[inline]
    pub fn new_no_notify(v: Option<&'a mut Overlapped>) -> OverlappedPtr<'a> {
        let v = OverlappedPtr(UnsafeCell::new(v));
        match unsafe { &mut *v.0.get() } {
            Some(o) if o.event != 0 => o.event = Handle::new(*o.event | 0x1), // Set low order bit.
            _ => (),
        }
        v
    }

    #[inline]
    pub fn event(&self) -> Handle {
        unsafe { &*self.0.get() }.as_ref().map_or(Handle::EMPTY, |v| v.event)
    }
    #[inline]
    pub fn is_some(&self) -> bool {
        unsafe { &*self.0.get() }.is_some()
    }
    #[inline]
    pub fn apc(&self) -> *mut Overlapped {
        match unsafe { &mut *self.0.get() }.as_deref_mut() {
            // Check low order bit.
            Some(v) if *v.event & 0x1 == 0 => v as *mut Overlapped,
            _ => null_mut(),
        }
    }
    #[inline]
    pub fn result(&self, i: IoStatusBlock) -> usize {
        unsafe { &mut *self.0.get() }
            .as_mut()
            // See note in 'GetOverlappedResult'
            .map_or(i.info, |v| replace(&mut v.internal_high, 0))
    }
    #[inline]
    pub fn result_olp(&self, o: Overlapped) -> usize {
        unsafe { &mut *self.0.get() }
            .as_mut()
            // See note in 'GetOverlappedResult'
            .map_or(o.internal_high, |v| replace(&mut v.internal_high, 0))
    }
    #[inline]
    pub fn offset(&self, v: Option<u64>) -> Option<u64> {
        if v.is_some() {
            return v;
        }
        match unsafe { &*self.0.get() }.as_ref() {
            Some(x) if x.offset != 0 || x.offset_high != 0 => Some(unsafe { (x.offset_high as u64).unchecked_shl(32) | x.offset as u64 }),
            _ => None,
        }
    }
    #[inline]
    pub fn io_olp(&self, o: &mut Overlapped) -> *mut Overlapped {
        unsafe { &mut *self.0.get() }
            .as_deref_mut()
            .map_or(o as *mut Overlapped, |v| v as *mut Overlapped)
    }
    #[inline]
    pub fn io(&self, i: &mut IoStatusBlock) -> *mut IoStatusBlock {
        unsafe { &mut *self.0.get() }
            .as_deref_mut()
            .map_or(i as *mut IoStatusBlock, |v| {
                v as *mut Overlapped as *mut IoStatusBlock
            })
    }
}
impl<'a> ObjectAttributes<'a> {
    #[inline]
    pub fn file(name: &'a UnicodeString, inherit: bool, attrs: u32, sa: Option<&'a SecurityAttributes>, qos: Option<&'a SecurityQualityOfService>) -> ObjectAttributes<'a> {
        ObjectAttributes::new(Some(name), Handle::EMPTY, inherit, attrs, sa, qos)
    }
    #[inline]
    pub fn new(name: Option<&'a UnicodeString>, root: Handle, inherit: bool, attrs: u32, sa: Option<&'a SecurityAttributes>, qos: Option<&'a SecurityQualityOfService>) -> ObjectAttributes<'a> {
        let mut o = ObjectAttributes {
            length:              size_of::<ObjectAttributes>() as u32,
            attributes:          attrs | if inherit { 0x2 } else { 0 }, // 0x2 - OBJ_INHERIT,
            object_name:         name,
            security_qos:        qos,
            root_directory:      root,
            security_descriptor: None,
        };
        if let Some(v) = sa {
            o.security_descriptor = v.security_descriptor;
            if v.inherit == 1 {
                o.attributes |= 0x2; // 0x2 - OBJ_INHERIT
            }
        }
        o
    }
}

impl Copy for FileTime {}
impl Clone for FileTime {
    #[inline]
    fn clone(&self) -> FileTime {
        FileTime { low: self.low, high: self.high }
    }
}
impl Default for FileTime {
    #[inline]
    fn default() -> FileTime {
        FileTime { low: 0u32, high: 0u32 }
    }
}
impl From<u64> for FileTime {
    #[inline]
    fn from(v: u64) -> FileTime {
        let r = v.saturating_mul(100);
        FileTime {
            low:  r as u32,
            high: unsafe { r.unchecked_shr(32) } as u32,
        }
    }
}

impl Drop for OwnedOverlapped {
    #[inline]
    fn drop(&mut self) {
        unsafe { close_handle(self.0.event) };
    }
}
impl Deref for OwnedOverlapped {
    type Target = Overlapped;

    #[inline]
    fn deref(&self) -> &Overlapped {
        &self.0
    }
}
impl DerefMut for OwnedOverlapped {
    #[inline]
    fn deref_mut(&mut self) -> &mut Overlapped {
        &mut self.0
    }
}
impl AsRef<Handle> for OwnedOverlapped {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0.event
    }
}

impl AsRef<Handle> for IoCompletionPort {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.e
    }
}

impl Default for Overlapped {
    #[inline]
    fn default() -> Overlapped {
        Overlapped {
            event:         Handle::EMPTY,
            internal:      0usize,
            internal_high: 0usize,
            offset:        0u32,
            offset_high:   0u32,
        }
    }
}
impl Default for IoStatusBlock {
    #[inline]
    fn default() -> IoStatusBlock {
        IoStatusBlock { status: 0usize, info: 0usize }
    }
}
impl Default for FileAllInformation {
    #[inline]
    fn default() -> FileAllInformation {
        FileAllInformation {
            mode:           0u32,
            name:           [0u16; 300],
            basic:          FileBasicInformation::default(),
            access:         0u32,
            file_id:        0u64,
            ea_size:        0u32,
            standard:       FileStandardInformation::default(),
            alignment:      0u32,
            name_length:    0u32,
            current_offset: 0u64,
        }
    }
}
impl Default for FileStatInformation {
    #[inline]
    fn default() -> FileStatInformation {
        FileStatInformation {
            access:           0u32,
            file_id:          0u64,
            attributes:       0u32,
            change_time:      SysTime::empty(),
            end_of_file:      0u64,
            reparse_tag:      0u32,
            creation_time:    SysTime::empty(),
            last_write_time:  SysTime::empty(),
            allocation_size:  0u64,
            number_of_links:  0u32,
            last_access_time: SysTime::empty(),
        }
    }
}
impl Default for FileBasicInformation {
    #[inline]
    fn default() -> FileBasicInformation {
        FileBasicInformation {
            attributes:       0u32,
            change_time:      SysTime::empty(),
            creation_time:    SysTime::empty(),
            last_write_time:  SysTime::empty(),
            last_access_time: SysTime::empty(),
        }
    }
}
impl Default for ObjectBasicInformation {
    #[inline]
    fn default() -> ObjectBasicInformation {
        ObjectBasicInformation {
            pad:            [0u32; 3],
            access:         0u32,
            handles:        0u32,
            created:        0u64,
            pointers:       0u32,
            name_size:      0u32,
            type_size:      0u32,
            paged_pool:     0u32,
            attributes:     0u32,
            sec_desc_size:  0u32,
            non_paged_pool: 0u32,
        }
    }
}
impl Default for FileStandardInformation {
    #[inline]
    fn default() -> FileStandardInformation {
        FileStandardInformation {
            end_of_file:     0u64,
            is_directory:    0u32,
            delete_pending:  0u32,
            allocation_size: 0u64,
            number_of_links: 0u32,
        }
    }
}
impl<'a> Default for ObjectAttributes<'a> {
    #[inline]
    fn default() -> ObjectAttributes<'a> {
        ObjectAttributes {
            length:              size_of::<ObjectAttributes>() as u32,
            attributes:          0u32,
            object_name:         None,
            security_qos:        None,
            root_directory:      Handle::EMPTY,
            security_descriptor: None,
        }
    }
}

impl<'a> From<Option<&'a mut Overlapped>> for OverlappedPtr<'a> {
    #[inline]
    fn from(v: Option<&'a mut Overlapped>) -> OverlappedPtr<'a> {
        OverlappedPtr::new(v)
    }
}

#[macro_export]
macro_rules! object_attrs {
    ($inherit:expr, $attrs:expr, $sa:expr, $qos:expr, $o:ident) => {
        let $o = ObjectAttributes::new(core::option::Option::None, $crate::structs::Handle::EMPTY, $inherit, $attrs, $sa, $qos);
    };
    ($root:expr, $inherit:expr, $attrs:expr, $sa:expr, $qos:expr, $o:ident) => {
        let __unicode_str = $crate::structs::UnicodeString::empty();
        let $o = ObjectAttributes::new(core::option::Option::Some(&__unicode_str), $root, $inherit, $attrs, $sa, $qos);
    };
    (name $name:expr, $inherit:expr, $attrs:expr, $sa:expr, $qos:expr, $o:ident) => {
        $crate::unicode_string!($name, __unicode_str);
        let $o = ObjectAttributes::new(core::option::Option::Some(&__unicode_str), $crate::structs::Handle::EMPTY, $inherit, $attrs, $sa, $qos);
    };
    ($name:expr, $root:expr, $inherit:expr, $attrs:expr, $sa:expr, $qos:expr, $o:ident) => {
        // Split this from using "unicode_string!" as this shouldn't need to
        // check for a NULL end as we might not have it anyway.
        let __wchar_name = core::convert::Into::<$crate::structs::WCharLike>::into($name);
        let __unicode_str = $crate::structs::UnicodeString::new(&__wchar_name);
        let $o = ObjectAttributes::new(core::option::Option::Some(&__unicode_str), $root, $inherit, $attrs, $sa, $qos);
    };
}

pub(crate) unsafe extern "system" fn copy_file_ex(_z: u64, _t: u64, _c: u64, n: u64, i: u32, _r: u32, _s: usize, _d: usize, d: *mut usize) -> u32 {
    if i == 1 {
        unsafe { *(d as *mut u64) = n };
    }
    0
}
