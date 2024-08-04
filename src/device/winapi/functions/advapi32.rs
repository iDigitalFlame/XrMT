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
#![allow(non_snake_case)]

use core::{cmp, ptr};

use crate::data::blob::{Blob, Slice};
use crate::data::str::MaybeString;
use crate::device::winapi::loader::advapi32;
use crate::device::winapi::registry::{Key, OwnedKey};
use crate::device::winapi::{self, AsHandle, DecodeUtf16, Handle, LsaAccountDomainInfo, LsaAttributes, OwnedHandle, ProcessInfo, RegKeyBasicInfo, RegKeyInfo, RegKeyValueFullInfo, SecAttrs, StartInfo, UnicodeString, WChars, Win32Error, Win32Result, SID};
use crate::prelude::*;
use crate::util::crypt;

const HKEY_ROOT: Key = Key(Handle(0usize));

pub fn local_system_sid() -> Win32Result<String> {
    winapi::init_advapi32();
    let mut h = Handle::default();
    // 0x1 - PolicyInformation
    let r = unsafe {
        let v = LsaAttributes::default();
        winapi::syscall!(
            *advapi32::LsaOpenPolicy,
            extern "stdcall" fn(*const UnicodeString, *const LsaAttributes, u32, &mut Handle) -> u32,
            ptr::null(),
            &v,
            0x1,
            &mut h
        )
    };
    if r > 0 {
        return Err(Win32Error::from_code(r));
    }
    let mut i: *mut LsaAccountDomainInfo = ptr::null_mut();
    let r = unsafe {
        // 0x5 - PolicyAccountDomainInformation
        let s = winapi::syscall!(
            *advapi32::LsaQueryInformationPolicy,
            extern "stdcall" fn(Handle, u32, *mut *mut LsaAccountDomainInfo) -> u32,
            h,
            0x5,
            &mut i
        );
        winapi::syscall!(*advapi32::LsaClose, extern "stdcall" fn(Handle) -> u32, h);
        if s > 0 {
            Err(Win32Error::from_code(r))
        } else {
            Ok((*i).sid.to_string())
        }
    };
    winapi::LocalFree(i);
    r
}
pub fn username_from_sid(sid: &SID) -> Win32Result<String> {
    winapi::init_advapi32();
    let (mut c, mut x, mut t) = (64u32, 64u32, 0u32);
    let mut n = Blob::<u16, 256>::with_capacity(c as usize);
    let mut d = Blob::<u16, 256>::with_capacity(x as usize);
    let func = unsafe {
        winapi::make_syscall!(
            *advapi32::LookupAccountSid,
            extern "stdcall" fn(*const u16, *const SID, *mut u16, *mut u32, *mut u16, *mut u32, *mut u32) -> u32
        )
    };
    loop {
        n.resize(c as usize * 2);
        d.resize(x as usize * 2);
        let r = func(
            ptr::null(),
            sid,
            n.as_mut_ptr(),
            &mut c,
            d.as_mut_ptr(),
            &mut x,
            &mut t,
        );
        match r {
            // 0x7A - ERROR_INSUFFICIENT_BUFFER
            0x7A => (),
            _ if x > 0 => {
                d.truncate(x as usize);
                d.push(b'\\' as u16);
                d.extend_from_slice(&n[0..c as usize]);
                return Ok((&d[0..(c + x) as usize + 1]).decode_utf16());
            },
            _ if x == 0 => return Ok((&n[0..c as usize]).decode_utf16()),
            _ => return Err(winapi::last_error()),
        }
    }
}

pub fn RtlGenRandom(buf: &mut [u8]) -> Win32Result<usize> {
    if winapi::is_min_windows_7() {
        // NOTE(dij): Windows 7 added the CNG device, which can be used to get
        //            better (and more secure) RNG data using an IOCTL.
        //
        //            This function will use this better version if we are at least
        //            Win7. If this fails, this function will fallback to the old
        //            way of doing it instead.
        let r = winapi::NtCreateFile(
            crypt::get_or(0, r"\Device\CNG"),
            Handle::INVALID,
            0x100001, // SYNCHRONIZE | FILE_READ_DATA
            None,
            0,
            0x7,  // FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
            0x1,  // OPEN_EXISTING
            0x20, // FILE_SYNCHRONOUS_IO_NONALERT
        )
        .and_then(|f| {
            winapi::NtDeviceIoControlFile(
                f,
                0x390004, // IOCTL_KSEC_RNG
                None,
                ptr::null() as *const usize,
                0,
                buf.as_mut_ptr(),
                cmp::min(buf.len(), 0xFFFFFFFF) as u32,
            )
        });
        if let Ok(n) = r {
            return Ok(n);
        }
    }
    winapi::init_advapi32();
    let r = unsafe {
        winapi::syscall!(
            *advapi32::SystemFunction036,
            extern "stdcall" fn(*mut u8, u32) -> u32,
            buf.as_mut_ptr(),
            cmp::min(buf.len(), 0xFFFFFFFF) as u32
        )
    };
    if r == 0 {
        Err(winapi::last_error())
    } else {
        Ok(r as usize)
    }
}

#[inline]
pub fn RegFlushKey(key: Key) -> Win32Result<()> {
    if !key.is_predefined() {
        winapi::NtFlushKey(key)
    } else if key.is_class_root() {
        winapi::NtFlushKey(*map_class_root(0, None)?)
    } else {
        winapi::NtFlushKey(*map_predefined_root(key)?)
    }
}
#[inline]
pub fn RegCloseKey(key: Key) -> Win32Result<()> {
    if key.is_invalid() || key.is_predefined() {
        Ok(())
    } else {
        winapi::CloseHandle(key.0)
    }
}
#[inline]
pub fn RegGetKeyNames(key: Key) -> Win32Result<Vec<String>> {
    if !key.is_predefined() {
        key_names(key)
    } else if key.is_class_root() {
        // 0x8 - KEY_ENUMERATE_SUB_KEYS
        key_names(*map_class_root(0x8, None)?)
    } else {
        key_names(*map_predefined_root(key)?)
    }
}
#[inline]
pub fn RegGetValueNames(key: Key) -> Win32Result<Vec<String>> {
    if !key.is_predefined() {
        value_names(key)
    } else if key.is_class_root() {
        // 0x1 - KEY_QUERY_VALUE
        value_names(*map_class_root(0x1, None)?)
    } else {
        value_names(*map_predefined_root(key)?)
    }
}
#[inline]
pub fn RegQueryInfoKey(key: Key) -> Win32Result<(String, RegKeyInfo)> {
    let i = if !key.is_predefined() {
        winapi::NtQueryKeyInfo(key)
    } else if key.is_class_root() {
        // 0x1 - KEY_QUERY_VALUE
        winapi::NtQueryKeyInfo(*map_class_root(0x1, None)?)
    } else {
        winapi::NtQueryKeyInfo(*map_predefined_root(key)?)
    }?;
    Ok((
        if i.class_length > 0 {
            (&i.class_name[0..i.class_length as usize / 2]).decode_utf16()
        } else {
            String::new()
        },
        i.into(),
    ))
}
#[inline]
pub fn RegDeleteTree(key: Key, subkey: impl MaybeString) -> Win32Result<()> {
    // 0x1000B - DELETE | KEY_SET_VALUE | KEY_QUERY_VALUE | KEY_ENUMERATE_SUB_KEYS
    let k = RegOpenKeyEx(key, subkey, 0, 0x1000B)?;
    let r = map_del_subkey_values(k);
    winapi::close_handle(k.0);
    r
}
#[inline]
pub fn RegDeleteKeyValue(key: Key, value: impl MaybeString) -> Win32Result<()> {
    if !key.is_predefined() {
        winapi::NtDeleteValueKey(key, value)
    } else if key.is_class_root() {
        // 0x2 - KEY_SET_VALUE
        winapi::NtDeleteValueKey(*map_class_root(0x2, None)?, value)
    } else {
        winapi::NtDeleteValueKey(*map_predefined_root(key)?, value)
    }
}
#[inline]
pub fn RegEnumKey(key: Key, index: u32) -> Win32Result<Option<RegKeyBasicInfo>> {
    if !key.is_predefined() {
        winapi::NtEnumerateKey(key, index)
    } else if key.is_class_root() {
        // 0x9 - KEY_ENUMERATE_SUB_KEYS | KEY_QUERY_VALUE
        winapi::NtEnumerateKey(*map_class_root(0x9, None)?, index)
    } else {
        winapi::NtEnumerateKey(*map_predefined_root(key)?, index)
    }
}
#[inline]
pub fn RegDeleteKey(key: Key, subkey: impl AsRef<str>, access: u32) -> Win32Result<()> {
    // 0x10000- DELETE
    let k = RegOpenKeyEx(key, subkey.as_ref(), 0, access | 0x10000)?;
    let r = winapi::NtDeleteKey(k);
    // Close the 'delete' Key before we return the result.
    winapi::close_handle(k.0);
    r
}
#[inline]
pub fn RegOpenKeyEx(key: Key, subkey: impl MaybeString, opts: u32, access: u32) -> Win32Result<Key> {
    if !key.is_predefined() {
        winapi::NtOpenKey(key, subkey, opts, access)
    } else if key.is_class_root() {
        let v = subkey.into_string();
        winapi::NtOpenKey(*map_class_root(access, v)?, v, opts, access)
    } else {
        winapi::NtOpenKey(*map_predefined_root(key)?, subkey, opts, access)
    }
}
#[inline]
pub fn RegQueryValueEx(key: Key, value: impl MaybeString, size: u32) -> Win32Result<(Blob<u8, 256>, u8)> {
    if !key.is_predefined() {
        winapi::NtQueryValueKey(key, value, size)
    } else if key.is_class_root() {
        // 0x1 - KEY_QUERY_VALUE
        winapi::NtQueryValueKey(*map_class_root(0x1, None)?, value, size)
    } else {
        winapi::NtQueryValueKey(*map_predefined_root(key)?, value, size)
    }
}
#[inline]
pub fn RegEnumValue(key: Key, index: u32, data: Option<&mut Blob<u8, 256>>) -> Win32Result<Option<RegKeyValueFullInfo>> {
    if !key.is_predefined() {
        winapi::NtEnumerateValueKey(key, index, data)
    } else if key.is_class_root() {
        // 0x1 - KEY_QUERY_VALUE
        winapi::NtEnumerateValueKey(*map_class_root(0x1, None)?, index, data)
    } else {
        winapi::NtEnumerateValueKey(*map_predefined_root(key)?, index, data)
    }
}
#[inline]
pub fn RegSetValueEx(key: Key, value: impl MaybeString, value_type: u32, data: Option<impl AsRef<[u8]>>, data_size: u32) -> Win32Result<()> {
    if !key.is_predefined() {
        winapi::NtSetValueKey(key, value, value_type, data, data_size)
    } else if key.is_class_root() {
        // 0x2 - KEY_SET_VALUE
        winapi::NtSetValueKey(
            *map_class_root(0x2, None)?,
            value,
            value_type,
            data,
            data_size,
        )
    } else {
        winapi::NtSetValueKey(
            *map_predefined_root(key)?,
            value,
            value_type,
            data,
            data_size,
        )
    }
}
#[inline]
pub fn RegCreateKeyEx(key: Key, subkey: impl AsRef<str>, class: impl MaybeString, opts: u32, access: u32, sa: SecAttrs) -> Win32Result<(Key, bool)> {
    if !key.is_predefined() {
        create_key(key, subkey, class, opts, access, sa)
    } else if key.is_class_root() {
        create_key(
            *map_class_root(access, Some(subkey.as_ref()))?,
            subkey,
            class,
            opts,
            access,
            sa,
        )
    } else {
        create_key(*map_predefined_root(key)?, subkey, class, opts, access, sa)
    }
}

pub fn LoginUser<U: AsRef<str>, M: MaybeString>(user: U, domain: M, password: M, login_type: u32, provider: u32) -> Win32Result<OwnedHandle> {
    winapi::init_advapi32();
    let n: WChars = user.as_ref().into();
    let d: WChars = domain.into_string().into();
    let p: WChars = password.into_string().into();
    let mut h = Handle::default();
    let r = unsafe {
        winapi::syscall!(
            *advapi32::LogonUser,
            extern "stdcall" fn(*const u16, *const u16, *const u16, u32, u32, *mut Handle) -> u32,
            n.as_ptr(),
            d.as_null_or_ptr(),
            p.as_null_or_ptr(),
            login_type,
            provider,
            &mut h
        )
    };
    if r == 0 {
        Err(winapi::last_error())
    } else {
        Ok(h.into())
    }
}

pub fn CreateProcessWithToken<T: AsRef<str>, E: AsRef<str>, M: MaybeString>(token: impl AsHandle, login_flags: u32, name: T, cmd: T, flags: u32, env_split: bool, env: &[E], dir: M, start: StartInfo) -> Win32Result<ProcessInfo> {
    if winapi::is_windows_xp() {
        return Err(Win32Error::NotImplemented);
    }
    winapi::init_advapi32();
    let mut i = ProcessInfo::default();
    let r = unsafe {
        let c: WChars = cmd.as_ref().into();
        let n: WChars = name.as_ref().into();
        let d: WChars = dir.into_string().into();
        let e = winapi::build_env(env_split, env);
        // 0x00400 - CREATE_UNICODE_ENVIRONMENT
        // 0x80000 - EXTENDED_STARTUPINFO_PRESENT
        let f = flags | 0x400 | if start.is_extended() { 0x80000 } else { 0 };
        winapi::syscall!(
            *advapi32::CreateProcessWithToken,
            extern "stdcall" fn(Handle, u32, *const u16, *const u16, u32, *const u16, *const u16, *const usize, *mut ProcessInfo) -> u32,
            token.as_handle(),
            login_flags,
            n.as_ptr(),
            c.as_ptr(),
            f,
            e.as_ptr(),
            d.as_null_or_ptr(),
            start.as_ptr(),
            &mut i
        )
    };
    if r == 0 {
        Err(winapi::last_error())
    } else {
        Ok(i)
    }
}
pub fn CreateProcessWithLogon<T: AsRef<str>, E: AsRef<str>, U: AsRef<str>, M: MaybeString>(user: U, domain: M, password: M, login_flags: u32, name: T, cmd: T, flags: u32, env_split: bool, env: &[E], dir: M, start: StartInfo) -> Win32Result<ProcessInfo> {
    winapi::init_advapi32();
    let mut i = ProcessInfo::default();
    let r = unsafe {
        let c: WChars = cmd.as_ref().into();
        let n: WChars = name.as_ref().into();
        let u: WChars = user.as_ref().into();
        let d: WChars = dir.into_string().into();
        let m: WChars = domain.into_string().into();
        let p: WChars = password.into_string().into();
        let e = winapi::build_env(env_split, env);
        // 0x00400 - CREATE_UNICODE_ENVIRONMENT
        // 0x80000 - EXTENDED_STARTUPINFO_PRESENT
        let f = flags | 0x400 | if start.is_extended() { 0x80000 } else { 0 };
        winapi::syscall!(
            *advapi32::CreateProcessWithLogon,
            extern "stdcall" fn(*const u16, *const u16, *const u16, u32, *const u16, *const u16, u32, *const u16, *const u16, *const usize, *mut ProcessInfo) -> u32,
            u.as_ptr(),
            m.as_null_or_ptr(),
            p.as_null_or_ptr(),
            login_flags,
            n.as_ptr(),
            c.as_ptr(),
            f,
            e.as_ptr(),
            d.as_null_or_ptr(),
            start.as_ptr(),
            &mut i
        )
    };
    if r == 0 {
        Err(winapi::last_error())
    } else {
        Ok(i)
    }
}

fn key_names(key: Key) -> Win32Result<Vec<String>> {
    let i = winapi::NtQueryKeyInfo(key)?;
    let mut n = Vec::with_capacity(i.subkeys as usize);
    for x in 0..i.values {
        if let Some(v) = winapi::NtEnumerateKey(key, x)? {
            n.push(v.to_string())
        }
    }
    Ok(n)
}
fn value_names(key: Key) -> Win32Result<Vec<String>> {
    let i = winapi::NtQueryKeyInfo(key)?;
    let mut n = Vec::with_capacity(i.values as usize);
    for x in 0..i.values {
        if let Some(v) = winapi::NtEnumerateValueKey(key, x, None)? {
            n.push(v.to_string())
        }
    }
    Ok(n)
}
#[inline]
fn current_user_sid() -> Win32Result<Slice<u8, 256>> {
    // 0x8 - TOKEN_QUERY
    winapi::token_info(winapi::current_token(0x8)?, |u| Ok(u.user.sid.to_slice()))
}
fn map_del_subkey_values(key: Key) -> Win32Result<()> {
    // Get count of subkeys/values.
    let i = winapi::NtQueryKeyInfo(key)?;
    // Find and delete all values first.
    for x in 0..i.values {
        match winapi::NtEnumerateValueKey(key, x, None)? {
            Some(v) => winapi::NtDeleteValueKey(key, v.to_string())?,
            None => break, // We don't have any more entries.
        }
    }
    // Loop through subkeys and re-enter this function with them.
    //
    // They ask for a delete of themselves at the end, so we just have to close
    // the handle.
    for x in 0..i.subkeys {
        match winapi::NtEnumerateKey(key, x)? {
            Some(s) => {
                // 0x1000B - DELETE | KEY_SET_VALUE | KEY_QUERY_VALUE | KEY_ENUMERATE_SUB_KEYS
                let k = winapi::NtOpenKey(key, s.to_string(), 0, 0x1000B)?;
                map_del_subkey_values(k)?;
                winapi::close_handle(k.0);
            },
            None => break, // We don't have any more entries.
        }
    }
    // Ask to delete outselves, the outer function will close our handle.
    winapi::NtDeleteKey(key)
}
fn map_predefined_root(root: Key) -> Win32Result<OwnedKey> {
    let n = match root.0 .0 & 0xFFFFFFF {
        0 => core::unreachable!(),
        1 => {
            let mut s = String::with_capacity(128);
            s.push_str(crypt::get_or(0, r"\Registry\User\"));
            unsafe { s.as_mut_vec().extend_from_slice(&current_user_sid()?.as_slice()) };
            s
        },
        2 => crypt::get_or(0, r"\Registry\Machine").to_string(),
        3 => crypt::get_or(0, r"\Registry\User").to_string(),
        4 => return Err(Win32Error::NotImplemented),
        5 => crypt::get_or(
            0,
            r"\Registry\Machine\System\CurrentControlSet\Hardware Profiles\Current",
        )
        .to_string(),
        6 => return Err(Win32Error::NotImplemented),
        _ => return Err(Win32Error::InvalidHandle),
    };
    // 0x2000000 - MAXIMUM_ALLOWED
    Ok(winapi::NtOpenKey(HKEY_ROOT, n, 0, 0x2000000)?.into())
}
fn map_class_root(access: u32, subkey: Option<&str>) -> Win32Result<OwnedKey> {
    // Check if the key access is valid under the Machine Key root.
    {
        // 0x2000000 - MAXIMUM_ALLOWED
        let p: OwnedKey = winapi::NtOpenKey(
            HKEY_ROOT,
            crypt::get_or(0, r"\Registry\Machine\Software\Classes"),
            0,
            0x2000000,
        )?
        .into();
        let r = subkey.map_or_else(
            || winapi::GetObjectInformation(&p).map_or(false, |i| i.access & access > 0),
            |v| {
                // 0x2000000 - MAXIMUM_ALLOWED
                winapi::NtOpenKey(*p, v, 0, 0x2000000).map_or(false, |h| {
                    winapi::close_handle(h.0);
                    winapi::GetObjectInformation(&p).map_or(false, |i| i.access & access > 0)
                })
            },
        );
        if r {
            return Ok(p);
        }
    }
    // It's not, so let's open the User Key root instead.
    let mut s = String::with_capacity(128);
    s.push_str(crypt::get_or(0, r"\Registry\User\"));
    unsafe { s.as_mut_vec().extend_from_slice(&current_user_sid()?.as_slice()) };
    s.push_str(crypt::get_or(0, r"_Classes"));
    // Open under User Key root.
    // 0x2000000 - MAXIMUM_ALLOWED
    Ok(winapi::NtOpenKey(HKEY_ROOT, s, 0, 0x2000000)?.into())
}
fn create_key(key: Key, subkey: impl AsRef<str>, class: impl MaybeString, opts: u32, access: u32, sa: SecAttrs) -> Win32Result<(Key, bool)> {
    let c = class.into_string();
    let v = subkey.as_ref();
    match winapi::NtCreateKey(key, v, c, opts, access, sa) {
        Ok(r) => return Ok(r),
        Err(e) => {
            if e != Win32Error::FileNotFound || v.is_empty() {
                return Err(e);
            }
        },
    }
    if v.len() < 2 {
        return Err(Win32Error::FileNotFound);
    }
    let b = v.as_bytes();
    let mut l = b.len();
    if b[l - 1] == b'\\' {
        l -= 1;
    }
    let mut p = l + 1;
    for i in (0..l).rev() {
        if b[i] != b'\\' {
            continue;
        }
        match winapi::NtCreateKey(key, &v[0..i], c, opts, access, sa) {
            Err(e) => {
                if e != Win32Error::FileNotFound {
                    return Err(e);
                }
                continue;
            },
            Ok(r) => {
                winapi::close_handle(r.0 .0);
                p = i;
                break;
            },
        }
    }
    if p > l {
        return Err(Win32Error::FileNotFound);
    }
    for i in p + 1..l {
        if b[i] != b'\\' {
            continue;
        }
        winapi::close_handle(winapi::NtCreateKey(key, &v[0..i], c, opts, access, sa)?.0 .0);
    }
    winapi::NtCreateKey(key, &v, c, opts, access, sa)
}
