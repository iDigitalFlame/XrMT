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
#![cfg(windows)]

use core::sync::atomic::{AtomicU8, Ordering};
use core::{mem, ptr};

use crate::data::blob::Blob;
use crate::device::winapi::loader::{advapi32, ntdll};
use crate::device::winapi::{self, stdio, AsHandle, Handle, LUIDAndAttributes, LsaAccountDomainInfo, LsaAttributes, MaybeString, OwnedHandle, ProcessBasicInfo, TokenPrivileges, TokenUser, UnicodeString, Win32Error, Win32Result, LUID};
use crate::util::stx::prelude::*;
use crate::util::{crypt, ToStr};

pub const SE_DEBUG_PRIVILEGE: u32 = 0x14;
pub const SE_SHUTDOWN_PRIVILEGE: u32 = 0x13;
pub const SE_INCREASE_PRIORITY_PRIVILEGE: u32 = 0xE;

const WINVER_WIN_UNKNOWN: u8 = 0;
const WINVER_WIN_XP: u8 = 1;
const WINVER_WIN_VISTA: u8 = 2;
const WINVER_WIN_7: u8 = 3;
const WINVER_WIN_8: u8 = 4;
const WINVER_WIN_8_1: u8 = 5;
const WINVER_WIN_10: u8 = 6;
const WINVER_WIN_11: u8 = 7;

const ADMINS: [u8; 16] = [1, 2, 0, 0, 0, 0, 0, 5, 32, 0, 0, 0, 32, 2, 0, 0];
const SYSTEM_ROOT: [u16; 11] = [83, 89, 83, 84, 69, 77, 82, 79, 79, 84, 61];

// NOTE(dij): This is an atomic as there's no way for this to get deadlocked as
//            this is checked BEFORE a thread can even be created, so only one
//            thread could realistically load this.
static VERSION: AtomicU8 = AtomicU8::new(0xFF);

pub fn is_elevated() -> bool {
    // 0x8 - TOKEN_QUERY
    if let Ok(t) = winapi::OpenThreadToken(winapi::CURRENT_THREAD, 0x8, true) {
        // Check thread Token, Xp can check these for Admin membership.
        return if !is_min_windows_vista() {
            in_admin_group(t.0)
        } else {
            token_elevated(t)
        };
    }
    if !is_min_windows_vista() {
        // No such thing as Integrity, check with 'empty' token.
        return in_admin_group(0);
    }
    // 0x8 - TOKEN_QUERY
    winapi::OpenProcessToken(winapi::CURRENT_PROCESS, 0x8).map_or(false, |t| token_elevated(t))
}
#[inline]
pub fn is_windows_xp() -> bool {
    if is_min_windows_vista() {
        return false;
    }
    winapi::init_advapi32();
    // Server 2003 has this, but Xp does not.
    advapi32::CreateProcessWithToken == false
}
#[inline]
pub fn last_error() -> Win32Error {
    Win32Error::Code(winapi::GetLastError())
}
#[inline]
pub fn is_min_windows_7() -> bool {
    version() >= WINVER_WIN_7
}
#[inline]
pub fn is_min_windows_8() -> bool {
    version() >= WINVER_WIN_8
}
#[inline]
pub fn is_min_windows_10() -> bool {
    version() >= WINVER_WIN_10
}
#[inline]
pub fn is_min_windows_vista() -> bool {
    version() >= WINVER_WIN_VISTA
}
#[inline]
pub fn exit_thread(exit_code: u32) -> ! {
    // This won't error.
    let _ = winapi::TerminateThread(winapi::CURRENT_THREAD, exit_code); // IGNORE ERROR
    core::unreachable!()
}
#[inline]
pub fn exit_process(exit_code: u32) -> ! {
    // This won't error.
    let _ = winapi::TerminateProcess(winapi::CURRENT_PROCESS, exit_code); // IGNORE ERROR
    core::unreachable!()
}
#[inline]
pub fn local_user() -> Win32Result<String> {
    // 0x8 - TOKEN_QUERY
    let t = winapi::current_token(0x8)?;
    let mut buf = Blob::new();
    TokenUser::from_token(t, &mut buf)?.user.sid.user()
}
pub fn system_sid() -> Win32Result<String> {
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
        return Err(Win32Error::Code(r));
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
            return Err(Win32Error::Code(r));
        }
        (*i).sid.to_string()
    };
    winapi::LocalFree(i);
    r
}
#[inline]
pub fn current_directory() -> Blob<u8, 256> {
    unsafe {
        (*(*winapi::GetCurrentProcessPEB()).process_parameters)
            .current_directory
            .dos_path
            .to_u8_blob()
    }
}
#[inline]
pub fn in_wow64_process() -> Win32Result<bool> {
    winapi::IsWoW64Process(winapi::CURRENT_PROCESS)
}
#[inline]
pub fn is_token_elevated(h: impl AsHandle) -> bool {
    if !is_min_windows_vista() {
        // 0xA - TOKEN_QUERY | TOKEN_DUPLICATE
        winapi::DuplicateTokenEx(h, 0xA, None, 1, 2).map_or(false, |t| in_admin_group(t.0))
    } else {
        token_elevated(h)
    }
}
#[inline]
pub fn acquire_privilege(privilege: u32) -> Win32Result<()> {
    set_privilege(privilege, true)
}
#[inline]
pub fn release_privilege(privilege: u32) -> Win32Result<()> {
    set_privilege(privilege, false)
}
#[inline]
pub fn current_token(access: u32) -> Win32Result<OwnedHandle> {
    winapi::OpenThreadToken(winapi::CURRENT_THREAD, access, true).or(winapi::OpenProcessToken(winapi::CURRENT_PROCESS, access))
}
#[inline]
pub fn current_process_info() -> Win32Result<ProcessBasicInfo> {
    let mut i = ProcessBasicInfo::default();
    // 0x0 - ProcessBasicInformation
    winapi::NtQueryInformationProcess(
        winapi::CURRENT_PROCESS,
        0,
        &mut i,
        mem::size_of::<ProcessBasicInfo>() as u32,
    )?;
    Ok(i)
}
#[inline]
pub fn write_stdout(msg: impl AsRef<str>) -> Win32Result<usize> {
    let h = unsafe { (*(*(winapi::GetCurrentProcessPEB())).process_parameters).standard_output };
    if h.is_invalid() {
        return Err(Win32Error::InvalidOperation);
    }
    stdio::write_console(h, msg.as_ref().as_bytes())
}
#[inline]
pub fn write_stderr(msg: impl AsRef<str>) -> Win32Result<usize> {
    let h = unsafe { (*(*(winapi::GetCurrentProcessPEB())).process_parameters).standard_error };
    if h.is_invalid() {
        return Err(Win32Error::InvalidOperation);
    }
    stdio::write_console(h, msg.as_ref().as_bytes())
}

pub(super) fn fix_name(n: impl MaybeString) -> Option<String> {
    if let Some(v) = n.into_string() {
        if v.as_bytes().first().map_or(false, |c| *c == b'\\') {
            return Some(v.to_string());
        }
        // NOTE(dij): This SYMLINK is valid from >= WinXp and can be used to
        //            translate Global, Local and Session namespaces.
        //
        // See: https://learn.microsoft.com/en-us/windows/win32/termserv/kernel-object-namespaces
        let mut b = crypt::get_or(0, r"\Sessions\BNOLINKS\").to_string();
        unsafe { (*winapi::GetCurrentProcessPEB()).session_id.into_vec(b.as_mut_vec()) };
        unsafe { b.as_mut_vec().push(b'\\') };
        b.push_str(v);
        return Some(b);
    }
    None
}
pub(super) fn build_env<T: AsRef<str>>(split: bool, env: &[T]) -> Blob<u16, 256> {
    let mut r = !split;
    let mut b = Blob::with_capacity((env.len() * 5) + 64);
    for e in env.iter() {
        let v = e.as_ref().as_bytes();
        if !r && v.len() > 12 {
            r = (v[0] == b'S' || v[0] == b's') && (v[6] == b'R' || v[6] == b'r') && v[10] == b'=';
        }
        b.reserve(v.len() + 1);
        b.extend(e.as_ref().encode_utf16());
        b.push(0);
    }
    if !split {
        b.extend_from_slice(winapi::GetEnvironment().as_slice())
    } else if !r {
        b.reserve(64);
        b.extend_from_slice(&SYSTEM_ROOT);
        let s = unsafe { (*winapi::kernel_user_shared()).system_root };
        match s.iter().position(|v| *v == 0) {
            Some(x) => b.extend_from_slice(&s[..x]),
            None => b.extend_from_slice(&s),
        }
        b.push(0);
    }
    b.push(0);
    b
}

#[inline]
pub(crate) fn close_handle(h: Handle) {
    if h.is_invalid() {
        crate::bugtrack!("winapi::CloseHandle(): CloseHandle on an invalid Handle!");
        // TODO(dij): Add logging here to capture closures
        return;
    }
    winapi::init_ntdll();
    let r = unsafe { winapi::syscall!(*ntdll::NtClose, extern "stdcall" fn(Handle) -> u32, h) };
    if r > 0 {
        crate::bugtrack!("winapi::CloseHandle(): CloseHandle 0x{h:X} resulted in an error 0x{r:X}!");
    }
}
#[inline]
pub(crate) fn is_user_network_token(t: impl AsHandle) -> bool {
    let h = t.as_handle();
    if h.is_invalid() {
        return false;
    }
    let mut b = [0u8; 16];
    // 0x7 - TokenSource
    winapi::GetTokenInformation(h, 0x7, b.as_mut_ptr(), 0x10).map_or(false, |_| {
        b[0] == 65 && b[1] == 100 && b[7] == 32 && b[7] == 32
    })
    // Match [65 100 118 97 112 105 32 32] == "Advapi"
}
pub(crate) fn std_flags_to_nt(access: u32, disposition: u32, attrs: u32) -> (u32, u32, u32, u32) {
    let d = match disposition {
        // CREATE_NEW
        0x1 => 0x2, // FILE_CREATE
        // CREATE_ALWAYS
        0x2 => 0x5, // FILE_OVERWRITE_IF
        // OPEN_EXISTING
        0x3 => 0x1, // FILE_OPEN
        // OPEN_ALWAYS
        0x4 => 0x3, // FILE_OPEN_IF
        // TRUNCATE_EXISTING
        0x5 => 0x4,       // FILE_OVERWRITE
        _ => disposition, // shrug
    };
    let (mut a, mut f) = (access, 0);
    if attrs & 0x40000000 == 0 {
        // FILE_FLAG_OVERLAPPED
        f |= 0x20; // FILE_SYNCHRONOUS_IO_NONALERT
    }
    if attrs & 0x80000000 > 0 {
        // FILE_FLAG_WRITE_THROUGH
        f |= 0x2; // FILE_WRITE_THROUGH
    }
    if attrs & 0x20000000 > 0 {
        // FILE_FLAG_NO_BUFFERING
        f |= 0x8; // FILE_NO_INTERMEDIATE_BUFFERING
    }
    if attrs & 0x10000000 > 0 {
        // FILE_FLAG_RANDOM_ACCESS
        f |= 0x800; // FILE_RANDOM_ACCESS
    }
    if attrs & 0x8000000 > 0 {
        // FILE_FLAG_SEQUENTIAL_SCAN
        f |= 0x4; // FILE_SEQUENTIAL_ONLY
    }
    if attrs & 0x4000000 > 0 {
        // FILE_FLAG_DELETE_ON_CLOSE
        f |= 0x1000; // FILE_DELETE_ON_CLOSE
        a |= 0x10000; // DELETE
    }
    if attrs & 0x2000000 > 0 {
        // FILE_FLAG_BACKUP_SEMANTICS
        if attrs & 0x10000000 > 0 {
            // GENERIC_ALL
            f |= 0x4400; // FILE_OPEN_FOR_BACKUP_INTENT |
                         // FILE_OPEN_REMOTE_INSTANCE
        } else {
            if attrs & 0x80000000 > 0 {
                // GENERIC_READ
                f |= 0x4000; // FILE_OPEN_FOR_BACKUP_INTENT
            }
            if attrs & 0x40000000 > 0 {
                // GENERIC_WRITE
                f |= 0x400; // FILE_OPEN_REMOTE_INSTANCE
            }
        }
    } else {
        f |= 0x40; // FILE_NON_DIRECTORY_FILE
    }
    if attrs & 0x200000 > 0 {
        // FILE_FLAG_OPEN_REPARSE_POINT
        f |= 0x200000; // FILE_OPEN_REPARSE_POINT
    }
    if attrs & 0x100000 > 0 {
        // FILE_FLAG_OPEN_NO_RECALL
        f |= 0x400000; // FILE_OPEN_NO_RECALL
    }
    // 0x3FA7 - FILE_ATTRIBUTE_VALID_FLAGS & ~FILE_ATTRIBUTE_DIRECTORY
    let v = attrs & 0x3FA7;
    a |= 0x100080; // SYNCHRONIZE | FILE_READ_ATTRIBUTES
    if attrs & 0x1000000 > 0 {
        // FILE_FLAG_POSIX_SEMANTICS
        f |= 0x1000000; // FILE_FLAG_POSIX_SEMANTICS
                        // We add this here as we're going to remove it later in
                        // the Nt call.
    }
    (a, v, d, f)
}

#[inline]
fn version() -> u8 {
    if let Err(r) = VERSION.compare_exchange(0xFF, 0xFE, Ordering::AcqRel, Ordering::Relaxed) {
        return r;
    }
    let (m, x, _) = winapi::GetVersionNumbers();
    let r = match m {
        11 => WINVER_WIN_11,
        10 => WINVER_WIN_10,
        6 => match x {
            0 => WINVER_WIN_VISTA,
            1 => WINVER_WIN_7,
            2 => WINVER_WIN_8,
            3 => WINVER_WIN_8_1,
            _ => WINVER_WIN_UNKNOWN,
        },
        5 => WINVER_WIN_XP,
        _ => WINVER_WIN_UNKNOWN,
    };
    VERSION.store(r, Ordering::Release);
    r
}
#[inline]
fn in_admin_group(h: usize) -> bool {
    winapi::init_advapi32();
    let mut a = 0u32;
    let r = unsafe {
        winapi::syscall!(
            *advapi32::CheckTokenMembership,
            extern "stdcall" fn(usize, *const u8, *mut u32) -> u32,
            h,
            ADMINS.as_ptr(),
            &mut a
        )
    };
    r > 0 && a > 0
}
fn token_elevated(h: impl AsHandle) -> bool {
    let mut buf = [0u8; 32];
    // 0x19 - TokenIntegrityLevel
    let r = winapi::GetTokenInformation(&h, 0x19, &mut buf, 32).unwrap_or_default();
    if r == 0 {
        return false;
    }
    let p = buf[r as usize - 4] as u32 | (buf[r as usize - 3] as u32) << 8 | (buf[r as usize - 2] as u32) << 16 | (buf[r as usize - 1] as u32) << 24;
    if p > 0x3000 {
        return true;
    }
    let mut e = 0u32;
    // 0x14 - TokenElevation
    winapi::GetTokenInformation(h, 0x14, &mut e, 4).unwrap_or_default() == 4 && e != 0
}
fn set_privilege(privilege: u32, enabled: bool) -> Win32Result<()> {
    // 0x200E8 - TOKEN_READ (STANDARD_RIGHTS_READ | TOKEN_QUERY) | TOKEN_WRITE
    //            (TOKEN_ADJUST_PRIVILEGES | TOKEN_ADJUST_GROUPS |
    //              TOKEN_ADJUST_DEFAULT)
    let t = current_token(0x200E8)?;
    let v = LUIDAndAttributes {
        luid:       LUID { low: privilege, high: 0 },
        attributes: if enabled { 0x2 } else { 0x4 },
        // 0x2 - SE_PRIVILEGE_ENABLED
        // 0x4 - SE_PRIVILEGE_REMOVED
    };
    let mut p = TokenPrivileges::default();
    p.set(0, v);
    let mut n = 0u32;
    winapi::AdjustTokenPrivileges(
        t,
        false,
        &mut p,
        mem::size_of::<TokenPrivileges>() as u32,
        ptr::null_mut(),
        &mut n,
    )
}
