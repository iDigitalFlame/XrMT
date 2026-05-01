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
#![allow(non_snake_case)]

extern crate alloc;
extern crate core;

extern crate xrmt_crypt;
extern crate xrmt_data;

use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::convert::{AsRef, From, Into};
use core::default::Default;
use core::hint::unreachable_unchecked;
use core::iter::Iterator;
use core::mem::drop;
use core::option::Option::{self, None, Some};
use core::ptr::null;
use core::result::Result::{Err, Ok};

use xrmt_crypt::crypt;
use xrmt_data::text::{str_to_u16_unchecked, utf16_to_string};
use xrmt_data::Blob;

use crate::functions::{
    close_handle,
    current_user_sid,
    len_to_u32,
    CloseHandle,
    GetObjectInformation,
    NtCreateFile,
    NtCreateKey,
    NtDeleteKey,
    NtDeleteValueKey,
    NtDeviceIoControlFile,
    NtEnumerateKey,
    NtEnumerateValueKey,
    NtFlushKey,
    NtOpenKey,
    NtQueryKeyInfo,
    NtQueryValueKey,
    NtSetValueKey,
};
use crate::info::{is_min_windows_7, is_windows_xp};
use crate::structs::{
    Handle,
    Key,
    LsaHandle,
    LsaPointer,
    ObjectAttributes,
    OwnedHandle,
    OwnedKey,
    ProcessInfo,
    RegKeyBasicInfo,
    RegKeyInfo,
    RegKeyIter,
    RegValueFullInfo,
    RegValueIter,
    SecAttrs,
    StartInfo,
    StringLike,
    StringLikeU16,
    StringWritable,
    UnicodeString,
    WCharLike,
    HKEY_ROOT,
};
use crate::{advapi32, object_attrs, str_const, Win32Error, Win32Result};

pub fn RtlGenRandom(buf: &mut [u8]) -> Win32Result<usize> {
    if is_min_windows_7() {
        // Windows 7 added the CNG device, which can be used to get
        // better (and more secure) RNG data using an IOCTL.
        //
        // This function will use this better version if we are at least
        // Win7. If this fails, this function will fallback to the old
        // way of doing it instead.
        //
        // "\Device\CNG" = 11 + 1 NULL
        str_const!(0, r"\Device\CNG", 12, v);
        let r = NtCreateFile(
            &v,
            Handle::EMPTY,
            0x100001, // SYNCHRONIZE | FILE_READ_DATA
            None,
            0,
            0x7,  // FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
            0x1,  // OPEN_EXISTING
            0x20, // FILE_SYNCHRONOUS_IO_NONALERT
        )
        .and_then(|f| {
            NtDeviceIoControlFile::<(), _>(
                f,
                0x390004, // IOCTL_KSEC_RNG
                None,
                null(),
                0,
                buf.as_mut_ptr(),
                len_to_u32(buf.len()),
            )
        });
        if let Ok(n) = r {
            return Ok(n);
        }
    }
    let r = syscall!(advapi32().SystemFunction036, (*mut u8, u32) -> u32, buf.as_mut_ptr(), len_to_u32(buf.len()));
    if r == 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(r as usize)
    }
}

#[inline]
pub fn LsaClose(h: LsaHandle) -> Win32Result<()> {
    lsa_close(*h)
}
pub fn LsaQueryInformationPolicy<T>(h: &LsaHandle, class: u32) -> Win32Result<LsaPointer<T>> {
    let mut v = LsaPointer::null();
    let s = syscall!(advapi32().LsaQueryInformationPolicy, (Handle, u32, *mut *mut T) -> u32, *h.as_ref(), class, &mut *v);
    if s > 0 {
        Err(Win32Error::from_code(s))
    } else {
        Ok(v)
    }
}
pub fn LsaOpenPolicy<'a>(access: u32, server: impl Into<WCharLike<'a>>) -> Win32Result<LsaHandle> {
    let mut h = Handle::default();
    object_attrs!(name server.into(), false, 0, None, None, o);
    let r = syscall!(
        advapi32().LsaOpenPolicy,
       (*const UnicodeString<'_>, *const ObjectAttributes<'_>, u32, &mut Handle) -> u32,
        null(),
        &o,
        access,
        &mut h
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}

#[inline]
pub fn RegFlushKey(key: Key) -> Win32Result<()> {
    if !key.is_predefined() {
        NtFlushKey(key)
    } else if key.is_root() {
        NtFlushKey(*classes(0, WCharLike::Null)?)
    } else {
        NtFlushKey(*root(key)?)
    }
}
#[inline]
pub fn RegCloseKey(key: Key) -> Win32Result<()> {
    if key.is_invalid() || key.is_predefined() {
        Ok(())
    } else {
        CloseHandle(*key)
    }
}
#[inline]
pub fn RegGetKeys<'a>(key: Key) -> Win32Result<RegKeyIter<'a>> {
    if !key.is_predefined() {
        RegKeyIter::new(key)
    } else if key.is_root() {
        // 0x8 - KEY_ENUMERATE_SUB_KEYS
        RegKeyIter::new(*classes(0x8, WCharLike::Null)?)
    } else {
        RegKeyIter::new(*root(key)?)
    }
}
#[inline]
pub fn RegGetValues<'a>(key: Key) -> Win32Result<RegValueIter<'a>> {
    if !key.is_predefined() {
        RegValueIter::new(key)
    } else if key.is_root() {
        // 0x1 - KEY_QUERY_VALUE
        RegValueIter::new(*classes(0x1, WCharLike::Null)?)
    } else {
        RegValueIter::new(*root(key)?)
    }
}
#[inline]
pub fn RegQueryInfoKey(key: Key) -> Win32Result<(String, RegKeyInfo)> {
    let mut b = Vec::new();
    let i = if !key.is_predefined() {
        NtQueryKeyInfo(key, &mut b)
    } else if key.is_root() {
        // 0x1 - KEY_QUERY_VALUE
        NtQueryKeyInfo(*classes(0x1, WCharLike::Null)?, &mut b)
    } else {
        NtQueryKeyInfo(*root(key)?, &mut b)
    }?;
    Ok((utf16_to_string(i.as_slice()), i.into()))
}
/// 'subkey' can be WCharLike::Null here.
#[inline]
pub fn RegDeleteTree<'a>(key: Key, subkey: impl Into<WCharLike<'a>>) -> Win32Result<()> {
    // 0x1000B - DELETE | KEY_SET_VALUE | KEY_QUERY_VALUE | KEY_ENUMERATE_SUB_KEYS
    let k = OwnedKey::from(RegOpenKeyEx(key, subkey, 0, 0x1000B)?);
    let mut b = Vec::new();
    delete(*k, &mut b)
}
/// 'subkey' can be WCharLike::Null here.
#[inline]
pub fn RegDeleteKeyValue<'a>(key: Key, value: impl Into<WCharLike<'a>>) -> Win32Result<()> {
    if !key.is_predefined() {
        NtDeleteValueKey(key, value)
    } else if key.is_root() {
        // 0x2 - KEY_SET_VALUE
        NtDeleteValueKey(*classes(0x2, WCharLike::Null)?, value)
    } else {
        NtDeleteValueKey(*root(key)?, value)
    }
}
#[inline]
pub fn RegDeleteKey<'a>(key: Key, subkey: impl Into<WCharLike<'a>>, access: u32) -> Win32Result<()> {
    let v = subkey.into();
    if v.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    // 0x10000 - DELETE
    let k = OwnedKey::from(RegOpenKeyEx(key, v, 0, access | 0x10000)?);
    NtDeleteKey(*k)
}
#[inline]
pub fn RegEnumKey<'a>(key: Key, index: u32, buf: &'a mut Vec<u8>) -> Win32Result<Option<&'a RegKeyBasicInfo>> {
    if !key.is_predefined() {
        NtEnumerateKey(key, index, buf)
    } else if key.is_root() {
        // 0x9 - KEY_ENUMERATE_SUB_KEYS | KEY_QUERY_VALUE
        NtEnumerateKey(*classes(0x9, WCharLike::Null)?, index, buf)
    } else {
        NtEnumerateKey(*root(key)?, index, buf)
    }
}
/// 'subkey' can be WCharLike::Null here.
#[inline]
pub fn RegOpenKeyEx<'a>(key: Key, subkey: impl Into<WCharLike<'a>>, opts: u32, access: u32) -> Win32Result<Key> {
    if !key.is_predefined() {
        NtOpenKey(key, subkey, opts, access)
    } else if key.is_root() {
        let v = subkey.into();
        NtOpenKey(*classes(access, &v)?, v, opts, access)
    } else {
        NtOpenKey(*root(key)?, subkey, opts, access)
    }
}
#[inline]
pub fn RegEnumValue<'a>(key: Key, index: u32, buf: &'a mut Vec<u8>) -> Win32Result<Option<&'a RegValueFullInfo>> {
    if !key.is_predefined() {
        NtEnumerateValueKey(key, index, buf)
    } else if key.is_root() {
        // 0x1 - KEY_QUERY_VALUE
        NtEnumerateValueKey(*classes(0x1, WCharLike::Null)?, index, buf)
    } else {
        NtEnumerateValueKey(*root(key)?, index, buf)
    }
}
/// 'subkey' can be WCharLike::Null here.
#[inline]
pub fn RegSetValueEx<'a>(key: Key, value: impl Into<WCharLike<'a>>, value_type: u32, data: Option<impl AsRef<[u8]>>, data_size: u32) -> Win32Result<()> {
    if !key.is_predefined() {
        NtSetValueKey(key, value, value_type, data, data_size)
    } else if key.is_root() {
        // 0x2 - KEY_SET_VALUE
        NtSetValueKey(
            *classes(0x2, WCharLike::Null)?,
            value,
            value_type,
            data,
            data_size,
        )
    } else {
        NtSetValueKey(*root(key)?, value, value_type, data, data_size)
    }
}
/// 'subkey' can be WCharLike::Null here.
#[inline]
pub fn RegQueryValueEx<'a, A: Allocator, const N: usize>(key: Key, value: impl Into<WCharLike<'a>>, buf: &mut Blob<u8, N, A>, size: u32) -> Win32Result<u8> {
    if !key.is_predefined() {
        NtQueryValueKey(key, value, buf, size)
    } else if key.is_root() {
        // 0x1 - KEY_QUERY_VALUE
        NtQueryValueKey(*classes(0x1, WCharLike::Null)?, value, buf, size)
    } else {
        NtQueryValueKey(*root(key)?, value, buf, size)
    }
}
/// 'class' can be WCharLike::Null here.
#[inline]
pub fn RegCreateKeyEx<'a>(key: Key, subkey: impl Into<WCharLike<'a>>, class: impl Into<WCharLike<'a>>, opts: u32, access: u32, sa: SecAttrs) -> Win32Result<(Key, bool)> {
    let v = subkey.into();
    if v.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    if !key.is_predefined() {
        create(key, v, class, opts, access, sa)
    } else if key.is_root() {
        create(*classes(access, &v)?, v, class, opts, access, sa)
    } else {
        create(*root(key)?, v, class, opts, access, sa)
    }
}

pub fn LoginUser<'a>(user: impl Into<WCharLike<'a>>, domain: impl Into<WCharLike<'a>>, password: impl Into<WCharLike<'a>>, login_type: u32, provider: u32) -> Win32Result<OwnedHandle> {
    let u = user.into();
    if u.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let (d, p) = (domain.into(), password.into());
    let mut h = Handle::default();
    let r = syscall!(
        advapi32().LogonUserW,
        (*const u16, *const u16, *const u16, u32, u32, *mut Handle) -> u32,
        u.as_ptr(),
        d.as_ptr(),
        p.as_ptr(),
        login_type,
        provider,
        &mut h
    );
    if r == 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(h.into())
    }
}

pub fn CreateProcessWithToken<'a, T: Into<WCharLike<'a>>>(token: impl AsRef<Handle>, login_flags: u32, name: T, cmd: T, flags: u32, env: T, dir: T, start: StartInfo<'a>) -> Win32Result<ProcessInfo> {
    if is_windows_xp() {
        // Windows Xp does not have this function, but Server2003/XpX64 does.
        return Err(Win32Error::NotImplemented);
    }
    let c = cmd.into();
    if c.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let (n, d, e) = (name.into(), dir.into(), env.into());
    // 0x00400 - CREATE_UNICODE_ENVIRONMENT
    // 0x80000 - EXTENDED_STARTUPINFO_PRESENT
    let f = flags | 0x400 | if start.is_extended() { 0x80000 } else { 0 };
    let mut i = ProcessInfo::default();
    let r = syscall!(
        advapi32().CreateProcessWithTokenW,
        (Handle, u32, *const u16, *const u16, u32, *const u16, *const u16, *const usize, *mut ProcessInfo) -> u32,
        *token.as_ref(),
        login_flags,
        n.as_ptr(),
        c.as_ptr(),
        f,
        e.as_ptr(),
        d.as_ptr(),
        start.as_ptr(),
        &mut i
    ) == 0;
    if r {
        Err(Win32Error::last_error())
    } else {
        Ok(i)
    }
}
pub fn CreateProcessWithLogon<'a, T: Into<WCharLike<'a>>>(user: T, domain: T, password: T, login_flags: u32, name: T, cmd: T, flags: u32, env: T, dir: T, start: StartInfo<'a>) -> Win32Result<ProcessInfo> {
    let c = cmd.into();
    if c.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let (n, d, e) = (name.into(), dir.into(), env.into());
    let (u, w, p) = (user.into(), domain.into(), password.into());
    // 0x00400 - CREATE_UNICODE_ENVIRONMENT
    // 0x80000 - EXTENDED_STARTUPINFO_PRESENT
    let f = flags | 0x400 | if start.is_extended() { 0x80000 } else { 0 };
    let mut i = ProcessInfo::default();
    let r = syscall!(
        advapi32().CreateProcessWithLogonW,
        (*const u16, *const u16, *const u16, u32, *const u16, *const u16, u32, *const u16, *const u16, *const usize, *mut ProcessInfo) -> u32,
        u.as_ptr(),
        w.as_ptr(),
        p.as_ptr(),
        login_flags,
        n.as_ptr(),
        c.as_ptr(),
        f,
        e.as_ptr(),
        d.as_ptr(),
        start.as_ptr(),
        &mut i
    ) == 0;
    if r {
        Err(Win32Error::last_error())
    } else {
        Ok(i)
    }
}

#[inline]
pub(crate) fn lsa_close(h: Handle) -> Win32Result<()> {
    let r = syscall!(advapi32().LsaClose, (Handle) -> u32, h);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}

fn root(root: Key) -> Win32Result<OwnedKey> {
    // 200 should cover "most" entries from growing to the heap.
    let mut b: Blob<u16, 200> = Blob::new();
    match root.as_usize() & 0xFFFFFFF {
        // HKEY_ROOT
        // Previous check proved this.
        0 => unsafe { unreachable_unchecked() },
        // HKEY_CURRENT_USER
        1 => unsafe {
            str_to_u16_unchecked(&mut b, crypt!(0, r"\Registry\User\"));
            let _ = current_user_sid(|s| {
                let _ = s.into_vec(&mut b);
            })?;
        },
        // HKEY_LOCAL_MACHINE
        2 => unsafe { str_to_u16_unchecked(&mut b, crypt!(0, r"\Registry\Machine")) },
        // HKEY_USER
        3 => unsafe { str_to_u16_unchecked(&mut b, crypt!(0, r"\Registry\User")) },
        4 => return Err(Win32Error::NotImplemented),
        5 => unsafe {
            str_to_u16_unchecked(
                &mut b,
                crypt!(
                    0,
                    r"\Registry\Machine\System\CurrentControlSet\Hardware Profiles\Current"
                ),
            )
        },
        6 => return Err(Win32Error::NotImplemented),
        _ => return Err(Win32Error::InvalidHandle),
    }
    // 0x2000000 - MAXIMUM_ALLOWED
    Ok(NtOpenKey(HKEY_ROOT, &b, 0, 0x2000000)?.into())
}
fn delete(key: Key, buf: &mut Vec<u8>) -> Win32Result<()> {
    // Get count of subkeys/values.
    let d = NtQueryKeyInfo(key, buf)?;
    // Save since we're overriting these later.
    let (h, j) = (d.values, d.subkeys);
    // As much as we could use the iter here, we don't so we can
    // reuse the Vec buffer.
    //
    // Find and delete all values first.
    for i in 0..h {
        match NtEnumerateValueKey(key, i, buf)? {
            Some(v) => NtDeleteValueKey(key, v.as_slice())?,
            None => break, // We don't have any more entries.
        }
    }
    // Loop through subkeys and re-enter this function with them.
    //
    // They ask for a delete of themselves at the end, so we just have to close
    // the handle.
    for i in 0..j {
        match NtEnumerateKey(key, i, buf)? {
            Some(s) => {
                // 0x1000B - DELETE | KEY_SET_VALUE | KEY_QUERY_VALUE | KEY_ENUMERATE_SUB_KEYS
                let k = OwnedKey::from(NtOpenKey(key, s.as_slice(), 0, 0x1000B)?);
                // We can reuse the buffer since this is just for temporary data.
                delete(*k, buf)?;
                drop(k);
            },
            None => break, // We don't have any more entries.
        }
    }
    // Ask to delete outselves, the outer function will close our handle.
    NtDeleteKey(key)
}
fn classes<'a>(access: u32, subkey: impl Into<WCharLike<'a>>) -> Win32Result<OwnedKey> {
    // Check if the key access is valid under the Machine Key root.
    {
        // Prevent heap allocation.
        // 35 is "\Registry\Machine\Software\Classes" + NULL
        str_const!(0, r"\Registry\Machine\Software\Classes", 35, t);
        // 0x2000000 - MAXIMUM_ALLOWED
        let p = OwnedKey::from(NtOpenKey(HKEY_ROOT, &t, 0, 0x2000000)?);
        let s = subkey.into();
        let r = if s.is_empty() {
            GetObjectInformation(&p).map_or(false, |i| i.access & access > 0)
        } else {
            // 0x2000000 - MAXIMUM_ALLOWED
            match NtOpenKey(*p, s, 0, 0x200000).map(OwnedKey::from) {
                Ok(_) => GetObjectInformation(&p).map_or(false, |i| i.access & access > 0),
                Err(_) => false,
            }
        };
        if r {
            return Ok(p);
        }
    }
    // It's not, so let's open the User Key root instead.
    // 256 should cover "most" entries from growing to the heap.
    let mut b: Blob<u16, 256> = Blob::new();
    unsafe {
        str_to_u16_unchecked(&mut b, crypt!(0, r"\Registry\User\"));
        let _ = current_user_sid(|s| {
            let _ = s.into_vec(&mut b);
        })?;
        str_to_u16_unchecked(&mut b, crypt!(0, r"_Classes"));
    }
    // Open under User Key root.
    // 0x2000000 - MAXIMUM_ALLOWED
    Ok(NtOpenKey(HKEY_ROOT, &b, 0, 0x2000000)?.into())
}
fn create<'a>(key: Key, subkey: impl Into<WCharLike<'a>>, class: impl Into<WCharLike<'a>>, opts: u32, access: u32, sa: SecAttrs) -> Win32Result<(Key, bool)> {
    let (s, c) = (subkey.into(), class.into());
    match NtCreateKey(key, &s, &c, opts, access, sa) {
        Err(e) if e != Win32Error::NotFound || s.is_empty() => return Err(e),
        Ok(r) => return Ok(r),
        _ => (),
    }
    let mut e = s.len_without_null();
    if e < 2 {
        return Err(Win32Error::NotFound);
    }
    if unsafe { *s.get_unchecked(e - 1) == 0x5C } {
        e -= 1; // Do we end in a seperator?
    }
    let mut v = unsafe { s.get_unchecked(0..e) }.split(|v| *v == 0x5C || *v == 0x2F);
    let f = match v.next() {
        None => return Err(Win32Error::InvalidArgument),
        Some(i) if i.is_empty() => return Err(Win32Error::InvalidArgument),
        Some(i) => i,
    };
    let (mut p, mut z) = NtCreateKey(key, f, &c, opts, access, sa)?;
    for i in v {
        if i.is_empty() {
            continue;
        }
        let r = NtCreateKey(p, i, &c, opts, access, sa);
        unsafe { close_handle(*p) }; // Close Parent Handle
        (p, z) = r?;
    }
    Ok((p, z))
}
