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

use core::alloc::Allocator;
use core::mem::size_of;
use core::{cmp, ptr};

use crate::data::blob::Blob;
use crate::data::str::MaybeString;
use crate::data::time::Time;
use crate::device::winapi::{self, stdio, AsHandle, LUIDAndAttributes, OwnedHandle, ProcessBasicInfo, SIDAndAttributes, SystemBasicInfo, TokenPrivileges, Win32Error, Win32Result, CURRENT_PROCESS, LUID, SID};
use crate::ignore_error;
use crate::prelude::*;
use crate::process::Filter;
use crate::util::{self, crypt, ToStr};

pub const SE_DEBUG_PRIVILEGE: u32 = 0x14u32;
pub const SE_SHUTDOWN_PRIVILEGE: u32 = 0x13u32;
pub const SE_INCREASE_PRIORITY_PRIVILEGE: u32 = 0xEu32;

const SYSTEM_ROOT: [u16; 11] = [
    b'S' as u16,
    b'Y' as u16,
    b'S' as u16,
    b'T' as u16,
    b'E' as u16,
    b'M' as u16,
    b'R' as u16,
    b'O' as u16,
    b'O' as u16,
    b'T' as u16,
    b'=' as u16,
];

#[inline]
pub fn last_error() -> Win32Error {
    Win32Error::from_code(winapi::GetLastError())
}
#[inline]
pub fn exit_thread(exit_code: u32) -> ! {
    // This won't error.
    ignore_error!(winapi::TerminateThread(winapi::CURRENT_THREAD, exit_code));
    core::unreachable!()
}
#[inline]
pub fn exit_process(exit_code: u32) -> ! {
    // This won't error.
    ignore_error!(winapi::TerminateProcess(winapi::CURRENT_PROCESS, exit_code));
    core::unreachable!()
}
pub fn erase_pe_header() -> Win32Result<()> {
    let mut i = SystemBasicInfo::default();
    // 0x0 - SystemBasicInformation
    winapi::NtQuerySystemInformation(0, &mut i, size_of::<SystemBasicInfo>() as u32)?;
    let b = winapi::GetCurrentProcessPEB().image_base_address.into();
    // 0x40 - PAGE_EXECUTE_READWRITE
    let p = winapi::NtProtectVirtualMemory(CURRENT_PROCESS, b, i.page_size, 0x40)?;
    unsafe {
        b.as_slice_mut(i.page_size as usize).fill(0); // Should make a memclr.
    };
    winapi::NtProtectVirtualMemory(CURRENT_PROCESS, b, i.page_size, p)?;
    Ok(())
}
#[inline]
pub fn untrust(pid: u32) -> Win32Result<()> {
    // 0x400 - PROCESS_QUERY_INFORMATION
    untrust_handle(winapi::OpenProcess(0x400, false, pid)?)
}
#[inline]
pub fn time_to_windows_time(t: Time) -> i64 {
    (t.unix() / 100) + winapi::WIN_TIME_EPOCH
}
#[inline]
pub fn time_from_windows_time(v: i64) -> Time {
    Time::from_unix(0, (v - winapi::WIN_TIME_EPOCH) * 100)
}
#[inline]
pub fn acquire_privilege(privilege: u32) -> Win32Result<()> {
    set_local_privilege(privilege, true)
}
#[inline]
pub fn release_privilege(privilege: u32) -> Win32Result<()> {
    set_local_privilege(privilege, false)
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
        size_of::<ProcessBasicInfo>() as u32,
    )?;
    Ok(i)
}
#[inline]
pub fn write_stdout(msg: impl AsRef<str>) -> Win32Result<usize> {
    let h = winapi::GetCurrentProcessPEB().process_params().standard_output;
    if h.is_invalid() {
        return Err(Win32Error::InvalidOperation);
    }
    stdio::write_console(h, msg.as_ref().as_bytes())
}
#[inline]
pub fn write_stderr(msg: impl AsRef<str>) -> Win32Result<usize> {
    let h = winapi::GetCurrentProcessPEB().process_params().standard_error;
    if h.is_invalid() {
        return Err(Win32Error::InvalidOperation);
    }
    stdio::write_console(h, msg.as_ref().as_bytes())
}
pub fn set_command_line(cmd: impl AsRef<str>) -> Win32Result<()> {
    let p = unsafe { &mut *(winapi::GetCurrentProcessPEB().process_parameters) };
    let m = p.command_line.length as usize;
    let a = p.command_line.as_slice_mut();
    let s = cmd.as_ref().encode_utf16().collect::<Blob<u16, 128>>();
    util::copy(a, &s);
    p.command_line.length = cmp::min(s.len_as_bytes(), m) as u16;
    #[cfg(not(target_pointer_width = "64"))]
    {
        // If we're a WoW64 process, write our command line too.
        if winapi::in_wow64_process() {
            // NOTE(dij): We have to open a Handle to ourselves as the WoW64 functions
            //            won't take the pseudo-Handle.
            // 0x1038 - PROCESS_VM_OPERATION | PROCESS_VM_READ | PROCESS_VM_WRITE |
            //           PROCESS_QUERY_LIMITED_INFORMATION
            let h = winapi::OpenProcess(0x1038, false, winapi::GetCurrentProcessID())?;
            // If we're in WoW64, overwite our 64bit PEB command line too.
            let mut i = winapi::wow::ProcessBasicInfo64::default();
            // 0x0 - ProcessBasicInformation
            winapi::NtWow64QueryInformationProcess64(&h, 0, &mut i, 0x30)?;
            let p = winapi::wow::RemotePEB64::read(&h, i.peb_base)?;
            let a = p.process_params(&h)?;
            a.command_line.write(&h, cmd)?;
        }
    }
    Ok(())
}
#[inline]
pub fn untrust_filter<A: Allocator>(proc: &Filter<A>) -> Win32Result<()> {
    // 0x400 - PROCESS_QUERY_INFORMATION
    untrust_handle(proc.handle_func(0x400, None).map_err(Win32Error::from)?)
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
        winapi::GetCurrentProcessPEB()
            .session_id
            .into_vec(unsafe { b.as_mut_vec() });
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
        let s = winapi::kernel_user_shared().system_root;
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
pub(crate) fn acquire_debug() {
    ignore_error!(set_local_privilege(SE_DEBUG_PRIVILEGE, true));
}
#[inline]
pub(crate) fn release_debug() {
    ignore_error!(set_local_privilege(SE_DEBUG_PRIVILEGE, false));
}
pub(crate) fn std_flags_to_nt(access: u32, disposition: u32, attrs: u32) -> (u32, u32, u32, u32) {
    let d = match disposition {
        0x1 => 0x2,       // CREATE_NEW -> FILE_CREATE
        0x2 => 0x5,       // CREATE_ALWAYS -> FILE_OVERWRITE_IF
        0x3 => 0x1,       // OPEN_EXISTING -> FILE_OPEN
        0x4 => 0x3,       // OPEN_ALWAYS -> FILE_OPEN_IF
        0x5 => 0x4,       // TRUNCATE_EXISTING -> FILE_OVERWRITE
        _ => disposition, // shrug
    };
    let (mut a, mut f) = (access, 0);
    if attrs & 0x40000000 == 0 {
        // FILE_FLAG_OVERLAPPED
        f |= 0x20; // FILE_SYNCHRONOUS_IO_NONALERT
    }
    if attrs & 0x80000000 != 0 {
        // FILE_FLAG_WRITE_THROUGH
        f |= 0x2; // FILE_WRITE_THROUGH
    }
    if attrs & 0x20000000 != 0 {
        // FILE_FLAG_NO_BUFFERING
        f |= 0x8; // FILE_NO_INTERMEDIATE_BUFFERING
    }
    if attrs & 0x10000000 != 0 {
        // FILE_FLAG_RANDOM_ACCESS
        f |= 0x800; // FILE_RANDOM_ACCESS
    }
    if attrs & 0x8000000 != 0 {
        // FILE_FLAG_SEQUENTIAL_SCAN
        f |= 0x4; // FILE_SEQUENTIAL_ONLY
    }
    if attrs & 0x4000000 != 0 {
        // FILE_FLAG_DELETE_ON_CLOSE
        f |= 0x1000; // FILE_DELETE_ON_CLOSE
        a |= 0x10000; // DELETE
    }
    if attrs & 0x2000000 != 0 {
        // FILE_FLAG_BACKUP_SEMANTICS
        if attrs & 0x10000000 != 0 {
            // GENERIC_ALL
            f |= 0x4400; // FILE_OPEN_FOR_BACKUP_INTENT |
                         // FILE_OPEN_REMOTE_INSTANCE
        } else {
            if attrs & 0x80000000 != 0 {
                // GENERIC_READ
                f |= 0x4000; // FILE_OPEN_FOR_BACKUP_INTENT
            }
            if attrs & 0x40000000 != 0 {
                // GENERIC_WRITE
                f |= 0x400; // FILE_OPEN_REMOTE_INSTANCE
            }
        }
    } else {
        f |= 0x40; // FILE_NON_DIRECTORY_FILE
    }
    if attrs & 0x200000 != 0 {
        // FILE_FLAG_OPEN_REPARSE_POINT
        f |= 0x200000; // FILE_OPEN_REPARSE_POINT
    }
    if attrs & 0x100000 != 0 {
        // FILE_FLAG_OPEN_NO_RECALL
        f |= 0x400000; // FILE_OPEN_NO_RECALL
    }
    // 0x3FA7 - FILE_ATTRIBUTE_VALID_FLAGS & ~FILE_ATTRIBUTE_DIRECTORY
    let v = attrs & 0x3FA7;
    a |= 0x100080; // SYNCHRONIZE | FILE_READ_ATTRIBUTES
    if attrs & 0x1000000 != 0 {
        // FILE_FLAG_POSIX_SEMANTICS
        f |= 0x1000000; // FILE_FLAG_POSIX_SEMANTICS
                        // We add this here as we're going to remove it later in
                        // the Nt call.
    }
    (a, v, d, f)
}

fn untrust_handle(h: impl AsHandle) -> Win32Result<()> {
    // 0x200A8 - TOKEN_READ | TOKEN_ADJUST_PRIVILEGES | TOKEN_ADJUST_DEFAULT |
    // TOKEN_QUERY
    let t = winapi::OpenProcessToken(h, 0x200A8)?;
    {
        let r = winapi::GetTokenInformation(&t, 0x3, ptr::null_mut::<usize>(), 0)?;
        let mut b: Blob<u8, 128> = Blob::with_size(r as usize);
        let n = winapi::GetTokenInformation(&t, 0x3, b.as_mut_ptr(), r)? as usize;
        let p = unsafe { &mut *(b.as_mut_ptr() as *mut TokenPrivileges) };
        // Remove all permissions
        for i in p.as_slice_mut() {
            // 0x4 - SE_PRIVILEGE_REMOVED
            i.attributes = 0x4
        }
        let mut z = 0u32;
        winapi::AdjustTokenPrivileges(&t, false, p, n as u32, ptr::null_mut(), &mut z)?;
        ignore_error!(winapi::AdjustTokenPrivileges(
            &t,
            true,
            p,
            n as u32,
            ptr::null_mut(),
            &mut z
        ));
    }
    if !winapi::is_min_windows_vista() {
        // Anything below Vista has no concept of integrity.
        return Ok(());
    }
    // 0x10 - Untrusted Domain
    let s = SID::well_known(0x10, 0);
    // 0x20 - SE_GROUP_INTEGRITY
    let v = SIDAndAttributes {
        sid:        s.as_psid(),
        attributes: 0x20,
    };
    // 0x19 - TokenIntegrityLevel
    winapi::SetTokenInformation(t, 0x19, &v, s.len() + 4)
}
fn set_local_privilege(privilege: u32, enabled: bool) -> Win32Result<()> {
    // 0x200E8 - TOKEN_READ (STANDARD_RIGHTS_READ | TOKEN_QUERY) | TOKEN_WRITE
    //            (TOKEN_ADJUST_PRIVILEGES | TOKEN_ADJUST_GROUPS |
    //              TOKEN_ADJUST_DEFAULT)
    let t = current_token(0x200E8)?;
    let v = LUIDAndAttributes {
        luid:       LUID { low: privilege, high: 0 },
        attributes: if enabled { 2 } else { 0 },
        // 0x2 - SE_PRIVILEGE_ENABLED
        // 0x4 - SE_PRIVILEGE_REMOVED
        // 0x0 - SE_PRIVILEGE_DISABLED
        // NOTE(dij): From M$, 'SE_PRIVILEGE_REMOVED' REMOVES the privilege from
        //            the Token and CANNOT be re-enabled. Use 0 instead to
        //            disable.
    };
    let mut p = TokenPrivileges::default();
    p.set(0, v);
    let mut n = 0u32;
    // Size is always 0x7C.
    winapi::AdjustTokenPrivileges(t, false, &p, 0x7Cu32, ptr::null_mut(), &mut n)
}
