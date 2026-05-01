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

extern crate alloc;
extern crate core;

extern crate xrmt_bugtrack;
extern crate xrmt_data;
extern crate xrmt_time;

use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ord;
use core::convert::{AsRef, Into};
use core::default::Default;
use core::hint::unreachable_unchecked;
use core::iter::Iterator;
use core::mem::{replace, size_of};
use core::ops::FnOnce;
use core::option::Option::{self, None, Some};
use core::ptr::{copy_nonoverlapping, null};
use core::result::Result::{Err, Ok};
use core::slice::from_raw_parts;
use core::time::Duration;

use xrmt_bugtrack::bugtrack;
use xrmt_data::text::utf16_to_string;
use xrmt_data::{Blob, Slice, VecLike};
use xrmt_time::Time;

use crate::functions::{
    AdjustTokenPrivileges,
    CheckRemoteDebuggerPresent,
    GetCurrentProcessID,
    GetCurrentProcessPEB,
    GetCurrentTEB,
    GetObjectInformation,
    GetProcessFileName,
    GetTokenInformation,
    LsaOpenPolicy,
    LsaQueryInformationPolicy,
    NetGetJoinInformation,
    NtAllocateVirtualMemory,
    NtCreateFile,
    NtDeviceIoControlFile,
    NtFreeVirtualMemory,
    NtQueryInformationFile,
    NtQueryInformationProcess,
    NtQuerySystemInformation,
    NtSetInformationFile,
    OpenProcess,
    OpenProcessToken,
    OpenThreadToken,
    ReOpenFile,
    SetThreadToken,
    SetTokenInformation,
    TerminateProcess,
    TerminateThread,
    WaitForSingleObject,
};
use crate::info::{is_min_windows_8, is_min_windows_8_1, is_min_windows_vista};
use crate::structs::{
    DiskGeometry,
    FileBasicInformation,
    FileRenameInformation,
    Handle,
    JoinState,
    KernelUserShared,
    LUIDAndAttributes,
    LsaAccountDomainInfo,
    LsaPointer,
    OwnedHandle,
    Privilege,
    ProcessBasicInfo,
    Protection,
    ReadInto,
    Region,
    RemotePEB,
    SIDAndAttributes,
    SidSlice,
    StringLike,
    StringLikeU16,
    SysTime,
    SystemBasicInfo,
    TimeZoneInfo,
    TokenPrivileges,
    TokenUser,
    WCharLike,
    WCharPtr,
    WCharSlice,
    WriteFrom,
    SID,
    WIN_TIME_EPOCH,
};
use crate::{ntdll, path_normalize, Win32Error, Win32Result, CURRENT_PROCESS, CURRENT_THREAD, INFINITE, PTR_SIZE};

/////////////////////////////////////
// Helper Functions
/////////////////////////////////////

#[inline]
pub fn current_user() -> Win32Result<String> {
    // 0x8 - TOKEN_QUERY
    token_username(current_token(0x8)?)
}
#[inline]
pub fn current_user_sid_slice() -> Win32Result<SidSlice> {
    // 0x8 - TOKEN_QUERY
    token_user_func(current_token(0x8)?, |u| Ok(u.user.sid.to_slice()))
}
#[inline]
pub fn current_token(access: u32) -> Win32Result<OwnedHandle> {
    // Check if impersonating, and try to get the Thread Token.
    // If that fails, fallback to the Process Token.
    if let Some(t) = current_thread_token(access)? {
        return Ok(t);
    }
    OpenProcessToken(CURRENT_PROCESS, access)
}
#[inline]
pub fn current_process_name(full: bool) -> Win32Result<String> {
    GetProcessFileName(CURRENT_PROCESS, full)
}
#[inline]
pub fn current_user_sid(f: impl FnOnce(&SID)) -> Win32Result<()> {
    // 0x8 - TOKEN_QUERY
    token_user_func(current_token(0x8)?, |u| {
        f(&u.user.sid);
        Ok(())
    })?;
    Ok(())
}
#[inline]
pub fn current_process_info<'a>() -> Win32Result<ProcessBasicInfo<'a>> {
    let mut i = ProcessBasicInfo::default();
    // 0x0 - ProcessBasicInformation
    let _ = NtQueryInformationProcess(
        CURRENT_PROCESS,
        0,
        &mut i,
        size_of::<ProcessBasicInfo>() as u32,
    )?;
    Ok(i)
}
#[inline]
pub fn current_thread_token(access: u32) -> Win32Result<Option<OwnedHandle>> {
    // Check if impersonating, and try to get the Thread Token.
    // If that fails, fallback to the Process Token.
    if GetCurrentTEB().is_impersonating > 0 {
        if let Ok(h) = OpenThreadToken(CURRENT_THREAD, access, true) {
            return Ok(Some(h));
        }
    }
    Ok(None)
}

#[inline]
pub fn exit_thread(exit_code: u32) -> ! {
    let _ = TerminateThread(CURRENT_THREAD, exit_code);
    unsafe { unreachable_unchecked() } // Should be fucking obvious
}
#[inline]
pub fn exit_process(exit_code: u32) -> ! {
    let _ = TerminateProcess(CURRENT_PROCESS, exit_code);
    unsafe { unreachable_unchecked() } // Should be fucking obvious
}

pub fn reopen_file(h: OwnedHandle, pos: bool, attrs: u32) -> Win32Result<OwnedHandle> {
    let mut a = 0u32;
    // 0x8 - FileAccessInformation
    let _ = NtQueryInformationFile(&h, 0x8, &mut a, 0x4)?;
    let mut n = 0u64;
    if pos {
        // 0xE - FilePositionInformation
        let _ = NtQueryInformationFile(&h, 0xE, &mut n, 0x8);
        // ^ Ignore this call if it fails, as it might not work on streams
        // anyway.
    }
    // 0x1 - FILE_OPEN
    let r = ReOpenFile(h, a, 0, None, 0x1, attrs)?;
    if pos && n > 0 {
        let _ = NtSetInformationFile(&r, 0xE, &n, 0x8);
    }
    Ok(r)
}

#[inline]
pub fn process_user(h: impl AsRef<Handle>) -> Win32Result<String> {
    // 0x8 - TOKEN_QUERY
    token_username(OpenProcessToken(h, 0x8)?)
}
#[inline]
pub fn process_cmdline(h: impl AsRef<Handle>) -> Win32Result<String> {
    RemotePEB::new(h.as_ref())?.command_line()
}
#[inline]
pub fn process_protection(h: impl AsRef<Handle>) -> Win32Result<Protection> {
    if !is_min_windows_8_1() {
        // Process Protection is Windows 8.1+
        return Ok(Protection::None);
    }
    let mut p = 0u32;
    // 0x3D - ProcessProtectionInformation
    let _ = NtQueryInformationProcess(h, 0x3D, &mut p, 4)?;
    Ok(p.into())
}

pub fn set_current_command_line<'a>(cmd: impl Into<WCharLike<'a>>) -> Win32Result<()> {
    // TODO(dij): Make a per process one that can set other processes's names.
    let n = cmd.into();
    let x = (n.len_without_null() as u16).saturating_mul(2);
    let p = &mut unsafe { &mut *(GetCurrentProcessPEB().process_parameters) }.command_line;
    if p.max_length >= x {
        p.length = x; // Reuse the same buffer since it's smaller than we need.
        unsafe { copy_nonoverlapping(n.as_ptr(), p.buffer.as_mut_ptr(), x as usize / 2) };
        return Ok(());
    }
    // 0x3000 - MEM_COMMIT | MEM_RESERVE
    let r = NtAllocateVirtualMemory(CURRENT_PROCESS, Region::EMPTY, x as usize, 0x3000, 0x4)?;
    // Query the Region and check the size allocated to it, try to set the max size
    // to the allocation size
    let s = r.query(CURRENT_PROCESS).map_or(x, |v| v.size as u16);
    unsafe { copy_nonoverlapping(n.as_ptr(), r.as_mut_ptr_of(), n.len_without_null()) };
    // Copy data to the new buffer.
    let o = replace(&mut p.buffer, WCharPtr::new(r.as_ptr_of()));
    let c = replace(&mut p.max_length, s) as usize;
    p.length = x;
    // Try to free the old buffer, ignore it if it fails.
    // 0x4000 - MEM_DECOMMIT
    let _ = NtFreeVirtualMemory(CURRENT_PROCESS, o, c, 0x4000);
    Ok(())
}

#[inline]
pub fn token_sid(t: impl AsRef<Handle>) -> Win32Result<SidSlice> {
    token_user_func(t, |u| Ok(u.user.sid.to_slice()))
}
#[inline]
pub fn token_username(t: impl AsRef<Handle>) -> Win32Result<String> {
    token_user_func(t, |u| u.user.sid.username())
}
pub fn token_info(t: impl AsRef<Handle>, class: u32, b: &mut impl VecLike<u8>) -> Win32Result<()> {
    let f = syscall!(
        ntdll().NtQueryInformationToken,
        fn(Handle, u32, *mut u8, u32, *mut u32) -> u32
    );
    let (h, mut n) = (*t.as_ref(), 0u32);
    b.resize(128, 0);
    loop {
        let r = unsafe { f(h, class, b.as_mut_ptr(), b.len() as u32, &mut n) };
        match r {
            // 0x0000007A - ERROR_INSUFFICIENT_BUFFER
            // 0xC0000023 - STATUS_BUFFER_TOO_SMALL
            0x7A | 0xC0000023 => {
                b.resize(n as usize, 0);
                continue;
            },
            0 => break,
            _ => return Err(Win32Error::from_status(r)),
        }
    }
    Ok(())
}
#[inline]
pub fn token_user<'a>(t: impl AsRef<Handle>, b: &'a mut Blob<u8, 128>) -> Win32Result<&'a TokenUser<'a>> {
    // 0x2 - TokenUser
    let _ = token_info(t, 0x1, b)?;
    Ok(unsafe { &*(b.as_ptr() as *const TokenUser) })
}
#[inline]
pub fn token_groups<'a>(t: impl AsRef<Handle>, b: &'a mut Vec<u8>) -> Win32Result<&'a [SIDAndAttributes<'a>]> {
    // 0x2 - TokenGroups
    let _ = token_info(t, 0x2, b)?;
    Ok(unsafe {
        from_raw_parts(
            b.as_ptr().add(PTR_SIZE) as *const SIDAndAttributes,
            *(b.as_ptr() as *const u32) as usize,
        )
    })
}
#[inline]
pub fn token_user_func<'a, T, F: FnOnce(&TokenUser) -> Win32Result<T>>(t: impl AsRef<Handle>, func: F) -> Win32Result<T> {
    let mut b = Blob::new();
    let r = {
        // Do stuff down here to avoid freeing the Blob too early.
        let u = token_user(t, &mut b)?;
        func(u)
    };
    Ok(r?)
}

#[inline]
pub fn take_current_thread_token() -> Win32Result<Option<OwnedHandle>> {
    // 0xF01FF - TOKEN_ALL_ACCESS
    let v = current_thread_token(0xF01FF)?;
    if v.is_some() {
        let _ = SetThreadToken(CURRENT_THREAD, Handle::EMPTY);
        // Ignore error, but clear Token.
    }
    Ok(v)
}

#[inline]
pub fn system_dir() -> Slice<u16, 270> {
    let mut b = system_root().to_u16_slice();
    if b.len() + 10 >= b.capacity() {
        return b;
    }
    unsafe {
        let (mut n, s) = (b.len(), b.as_array_mut());
        // Eliminate bounds checks.
        //
        // We could use 'push' function here, but we KNOW what the size is
        // so we don't need to check it.
        if *s.get_unchecked(n) != 0x5C {
            *s.get_unchecked_mut(n) = 0x5C;
            n += 1;
        }
        *s.get_unchecked_mut(n) = 0x53;
        *s.get_unchecked_mut(n + 1) = 0x79;
        *s.get_unchecked_mut(n + 2) = 0x73;
        *s.get_unchecked_mut(n + 3) = 0x74;
        *s.get_unchecked_mut(n + 4) = 0x65;
        *s.get_unchecked_mut(n + 5) = 0x6D;
        *s.get_unchecked_mut(n + 6) = 0x33;
        *s.get_unchecked_mut(n + 7) = 0x32;
        *s.get_unchecked_mut(n + 8) = 0;
        b.set_len(n + 8);
    }
    b
}
#[inline]
pub fn system_root<'a>() -> WCharSlice<'a> {
    KernelUserShared::get().system_root()
}
#[inline]
pub fn system_dir_u8() -> Slice<u8, 270> {
    // This conversion is safe as these values will never be a non ASCII sequence.
    let mut b = unsafe { Slice::from_utf16_unchecked(system_root().as_slice()) };
    if b.len() + 10 >= b.capacity() {
        return b;
    }
    unsafe {
        let (mut n, s) = (b.len(), b.as_array_mut());
        // Eliminate bounds checks.
        //
        // We could use 'push' function here, but we KNOW what the size is
        // so we don't need to check it.
        if *s.get_unchecked(n) != 0x5C {
            *s.get_unchecked_mut(n) = 0x5C;
            n += 1;
        }
        *s.get_unchecked_mut(n) = 0x53;
        *s.get_unchecked_mut(n + 1) = 0x79;
        *s.get_unchecked_mut(n + 2) = 0x73;
        *s.get_unchecked_mut(n + 3) = 0x74;
        *s.get_unchecked_mut(n + 4) = 0x65;
        *s.get_unchecked_mut(n + 5) = 0x6D;
        *s.get_unchecked_mut(n + 6) = 0x33;
        *s.get_unchecked_mut(n + 7) = 0x32;
        *s.get_unchecked_mut(n + 8) = 0;
        b.set_len(n + 8);
    }
    b
}
#[inline]
pub fn system_sid() -> Win32Result<SidSlice> {
    // 0x1 - POLICY_VIEW_LOCAL_INFORMATION
    let h = LsaOpenPolicy(0x1, WCharLike::Null)?;
    // 0x5 - PolicyAccountDomainInformation
    let i: LsaPointer<LsaAccountDomainInfo> = LsaQueryInformationPolicy(&h, 0x5)?;
    i.as_ref().ok_or(Win32Error::InvalidObject).map(|v| v.sid.to_slice())
}
#[inline]
pub fn system_domain() -> Win32Result<(String, JoinState)> {
    NetGetJoinInformation(WCharLike::Null)
}

#[inline]
pub fn is_terminal(h: impl AsRef<Handle>) -> bool {
    if !is_min_windows_8() {
        return **h.as_ref() & 0x10000003 == 3;
    } else {
        // Why this works I'm not 100% sure lol.
        GetObjectInformation(h).map_or(false, |v| v.attributes == 0x2 && v.access == 0x12019F)
    }
}

pub fn is_debugged() -> bool {
    let s = KernelUserShared::get();
    if s.debugger_enabled > 1 || s.shared_flags & 0x1 != 0 {
        // s.shared_flags&0x1 == 1
        //
        // ^ This returns true when on a Debug/Checked version on
        // Windows. Not sure if we want to ignore this or not,
        // but I doubt that actual systems are using
        // "Multiprocessor Debug/Checked" unless the system is a
        // driver test or builder.
        return true;
    }
    let mut f = 0u16;
    // 0x23 - SystemKernelDebuggerInformation
    let r = NtQuerySystemInformation(0x23, &mut f, 2).unwrap_or(0);
    // The SYSTEM_KERNEL_DEBUGGER_INFORMATION short offset 1 (last 8 bits) is not
    // filled out by systems older than Vista, so we ignore them.
    if r == 2 && ((f & 0xFF) > 1 || (unsafe { f.unchecked_shr(8) } == 0 && is_min_windows_vista())) {
        return true;
    }
    let p = GetCurrentProcessPEB();
    // 0x70 - FLG_HEAP_ENABLE_TAIL_CHECK | FLG_HEAP_ENABLE_FREE_CHECK |
    //         FLG_HEAP_VALIDATE_PARAMETERS
    if p.being_debugged > 0 || p.nt_global_flag & 0x70 != 0 {
        return true;
    }
    #[cfg(target_arch = "x86_64")]
    {
        // NOTE(dij): x64 specific hack.
        //
        // For some reason, the XMM0 (floating point) register will ALWAYS be
        // non-zero when 'OutputDebugStringA' returns successfully and a debugger
        // is able to receive the Debug string.
        //
        // In Golang, this was the output of the 'r2' on 'syscall', which translates
        // to:
        //   - XMM0 (x64)
        //   - EDX  (x86)
        //   - R1?  (ARM64) [Technically Unimplemented]
        //   - N/A  (ARM) [Not Implemented]
        let b = [b'_', 0u8];
        syscall!(
            crate::loader::kernel32_or_base().OutputDebugStringA,
            (*const u8) -> (),
            b.as_ptr()
        );
        let e: usize;
        unsafe { core::arch::asm!("movq {}, XMM0", out(xmm_reg) e) };
        if e > 0 {
            return true;
        }
    }
    OpenProcess(0x400, false, GetCurrentProcessID()).map_or(false, |h| CheckRemoteDebuggerPresent(h).unwrap_or(false))
}
#[inline]
pub fn is_elevated() -> bool {
    // 0x8 - TOKEN_QUERY
    current_token(0x8).map_or(false, is_token_elevated)
}
#[inline]
pub fn is_thread_impersonating() -> bool {
    GetCurrentTEB().is_impersonating > 0
}
#[inline]
pub fn is_token_network(t: impl AsRef<Handle>) -> bool {
    let h = t.as_ref();
    if h.is_invalid() {
        return false;
    }
    let mut b = [0u8; 16];
    // 0x7 - TokenSource
    GetTokenInformation(h, 0x7, b.as_mut_ptr(), 0x10).map_or(false, |_| unsafe {
        *b.get_unchecked(0) == 0x41 && *b.get_unchecked(1) == 0x64 && *b.get_unchecked(6) == 0x20 && *b.get_unchecked(7) == 0x20
    })
    // Match [65 100 118 97 112 105 32 32] == "Advapi"
}
#[inline]
pub fn is_token_elevated(h: impl AsRef<Handle>) -> bool {
    if !is_min_windows_vista() {
        token_group(h)
    } else {
        token_elevated(h)
    }
}

#[inline]
pub fn is_utc_time() -> bool {
    // We're gonna check KUSER_SHARED first, so we don't have to make any syscalls.
    // if that check fails we do the syscall to check.
    if kernel_time_offset() == 0 {
        return true;
    }
    // Now that the fast check didn't pass, check here.
    let mut i = TimeZoneInfo::default();
    // 0x2C - SystemCurrentTimeZoneInformation
    NtQuerySystemInformation(0x2C, &mut i, 0xAC).map_or(false, |_| {
        i.bias == 0 && i.daylight_bias == 0 && i.standard_bias == 0
    })
}
#[inline]
pub fn in_safe_mode() -> bool {
    KernelUserShared::get().safe_boot_mode > 0
}
#[inline]
pub fn is_system_eval() -> bool {
    KernelUserShared::get().expiration_date > 0
}
#[inline]
pub fn is_uac_enabled() -> bool {
    // 0x2 - DbgElevationEnabled
    KernelUserShared::get().shared_flags & 0x2 != 0
}
#[inline]
pub fn is_secure_boot_enabled() -> bool {
    // Fastpath, might not work 100%, so we fallback to the syscall.
    // 0x80 - DbgSecureBootEnabled
    if KernelUserShared::get().shared_flags & 0x80 != 0 {
        return true;
    }
    let mut f = 0u16;
    // 0x91 - SystemSecureBootInformation
    NtQuerySystemInformation(0x91, &mut f, 2).map_or(false, |_| f & 0xFF == 1)
}
#[inline]
pub fn is_stack_tracing_enabled() -> bool {
    KernelUserShared::get().max_stack_trace > 0
}

#[inline]
pub fn total_memory() -> Win32Result<u32> {
    let mut i = SystemBasicInfo::default();
    // 0x0 - SystemBasicInformation
    let _ = NtQuerySystemInformation(0, &mut i, size_of::<SystemBasicInfo>() as u32)?;
    Ok((((i.page_size as u64).saturating_mul(i.number_physical_pages as u64)) / 0x100000) as u32 + 1)
}
#[inline]
pub fn code_integrity_status() -> Win32Result<u32> {
    let mut i = [8u32, 0u32];
    // 0x67 - SystemCodeIntegrityInformation
    let _ = NtQuerySystemInformation(0x67, &mut i, 0x8)?;
    Ok(unsafe { *i.get_unchecked(1) })
}
#[inline]
pub fn disk_size<'a>(name: impl Into<WCharLike<'a>>) -> Win32Result<u64> {
    let f = NtCreateFile(name, Handle::EMPTY, 0x100080, None, 0, 0x1, 0, 0x40)?;
    let mut g = DiskGeometry::default();
    // 0x700A0 - IOCTL_DISK_GET_DRIVE_GEOMETRY_EX
    let _ = NtDeviceIoControlFile::<(), _>(
        f,
        0x700A0,
        None,
        null(),
        0,
        &mut g,
        0x20 + (PTR_SIZE as u32),
    )?;
    Ok(g.size)
}

#[inline]
pub fn kernel_time_offset() -> i64 {
    KernelUserShared::get().kernel_time_offset()
}
#[inline]
pub fn kernel_nano_sec_time() -> i64 {
    KernelUserShared::get().system_time.as_unix_ns()
}
#[inline]
pub fn kernel_nano_sec_local_time() -> i64 {
    (kernel_time_offset().saturating_sub(WIN_TIME_EPOCH)).saturating_mul(100)
}

#[cfg(target_pointer_width = "64")]
#[inline(always)]
pub fn len_to_u32(v: usize) -> u32 {
    (v & 0xFFFFFFFF) as u32
}
#[cfg(not(target_pointer_width = "64"))]
#[inline(always)]
pub fn len_to_u32(v: usize) -> u32 {
    // When the pointer size is anything less than 64bits, usize values
    // cannot be larger than 32bit numbers, so no restriction is needed.
    v as u32
}

#[inline]
pub fn time_to_windows_time(t: Time) -> i64 {
    (t.unix() / 100).saturating_add(WIN_TIME_EPOCH)
}
#[inline]
pub fn time_from_windows_time(v: i64) -> Time {
    Time::from_unix(0, v.saturating_sub(WIN_TIME_EPOCH).saturating_mul(100))
}

#[inline]
pub fn untrust_pid(pid: u32) -> Win32Result<()> {
    // 0x400 - PROCESS_QUERY_INFORMATION
    untrust(OpenProcess(0x400, false, pid)?)
}
pub fn untrust(h: impl AsRef<Handle>) -> Win32Result<()> {
    // 0x200A8 - TOKEN_READ | TOKEN_ADJUST_PRIVILEGES | TOKEN_ADJUST_DEFAULT |
    //            TOKEN_QUERY
    let t = OpenProcessToken(h, 0x200A8)?;
    let mut b: Blob<u8, 200> = Blob::new();
    // 0x3 - TokenPrivileges
    let _ = token_info(&t, 0x3, &mut b)?;
    let p = unsafe { &mut *(b.as_mut_ptr() as *mut TokenPrivileges) };
    // Remove all permissions
    for i in p.as_slice_mut() {
        // 0x4 - SE_PRIVILEGE_REMOVED
        i.attributes = 0x4
    }
    let (mut z, n) = (0u32, p.len() as u32);
    // Set all updated Privileges
    let _ = AdjustTokenPrivileges(&t, false, Some(p), n, None, &mut z)?;
    // Disable all Privileges
    let _ = AdjustTokenPrivileges(&t, true, Some(p), n, None, &mut z);
    if !is_min_windows_vista() {
        // Anything below Vista has no concept of integrity.
        return Ok(());
    }
    // 0x10 - Untrusted Mandatory Label
    let s = SID::well_known_raw(0x10, 0);
    // 0x20 - SE_GROUP_INTEGRITY
    let v = SIDAndAttributes {
        sid:        s.as_psid(),
        attributes: 0x20u32,
    };
    // 0x19 - TokenIntegrityLevel
    SetTokenInformation(t, 0x19, &v, v.len())
}

#[inline]
pub fn privilege_raw_release(privilege: u32) -> Win32Result<()> {
    set_local_privilege(privilege as u32, false)
}
#[inline]
pub fn privilege_raw_accquire(privilege: u32) -> Win32Result<()> {
    set_local_privilege(privilege as u32, true)
}
#[inline]
pub fn privilege_release(privilege: Privilege) -> Win32Result<()> {
    set_local_privilege(privilege as u32, false)
}
#[inline]
pub fn privilege_accquire(privilege: Privilege) -> Win32Result<()> {
    set_local_privilege(privilege as u32, true)
}

#[inline]
pub fn read_memory<P, T>(proc: impl AsRef<Handle>, ptr: *mut P, to: ReadInto<T>) -> Win32Result<usize> {
    Region::into(ptr).read(proc, to)
}
#[inline]
pub fn write_memory<P, T>(proc: impl AsRef<Handle>, ptr: *const P, from: WriteFrom<T>) -> Win32Result<usize> {
    Region::into(ptr).write(proc, from)
}

#[inline]
pub fn duration_to_micros(dur: Duration) -> u64 {
    dur.as_micros() as u64 & INFINITE
}
#[inline]
pub fn duration_option_to_micros(dur: Option<Duration>) -> u64 {
    dur.map_or(INFINITE, duration_to_micros)
}

#[inline]
pub fn wait_on(h: impl AsRef<Handle>, dur: Option<Duration>) -> bool {
    match WaitForSingleObject(h, duration_option_to_micros(dur), false) {
        Ok(0) => true,
        _ => false,
    }
}
#[inline]
pub fn wait_on_multiple(h: &[usize], all: bool, dur: Option<Duration>) -> i8 {
    match unsafe { wait_for_multiple_objects(h, h.len(), all, duration_option_to_micros(dur), false) } {
        Ok(v) if v < 0x40 => v as i8,
        _ => -1,
    }
}

#[inline]
pub fn win32_handle_to_nt(h: Handle) -> usize {
    if h > -10isize as usize && h < -12isize as usize {
        return h.as_usize();
    }
    let p = GetCurrentProcessPEB();
    match h.as_usize() as isize {
        -10 => p.process_params().standard_input.as_usize(),  // -10 | STD_INPUT_HANDLE
        -11 => p.process_params().standard_output.as_usize(), // -11 | STD_OUTPUT_HANDLE
        -12 => p.process_params().standard_error.as_usize(),  // -12 | STD_ERROR_HANDLE
        _ => h.as_usize(),
    }
}
pub fn win32_file_flags_to_nt(access: u32, disposition: u32, attrs: u32) -> (u32, u32, u32, u32) {
    let d = match disposition {
        1 => 2,           // CREATE_NEW -> FILE_CREATE
        2 => 5,           // CREATE_ALWAYS -> FILE_OVERWRITE_IF
        3 => 1,           // OPEN_EXISTING -> FILE_OPEN
        4 => 3,           // OPEN_ALWAYS -> FILE_OPEN_IF
        5 => 4,           // TRUNCATE_EXISTING -> FILE_OVERWRITE
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
                        // We add this here as we're going to remove it later
                        // in the Nt call.
    }
    (a, v, d, f)
}

#[inline]
pub fn file_delete(h: impl AsRef<Handle>) -> Win32Result<()> {
    // 0xD - FileDispositionInformation
    let d = 1u32; // Prevent optimization of NUL ptr.
    let _ = NtSetInformationFile(h, 0xD, &d, 4)?;
    Ok(())
}
#[inline]
pub fn file_is_file<'a>(f: impl Into<WCharLike<'a>>) -> bool {
    let h = match NtCreateFile(f, Handle::EMPTY, 0x80100080, None, 0, 0x1, 0x1, 0) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let mut a = [0u32, 0u32];
    // 0x23 - FileAttributeTagInformation
    NtQueryInformationFile(h, 0x23, &mut a, 0x8).map_or(false, |_| unsafe { *a.get_unchecked(0) & 0x10 == 0 })
}
pub fn file_name(h: impl AsRef<Handle>) -> Win32Result<String> {
    let mut b: Blob<u16, 300> = Blob::new();
    let f = syscall!(
        ntdll().NtQueryObject,
        fn(Handle, u32, *mut u16, u32, *mut u32) -> u32
    );
    let mut n = 0x208u32; // 520 = 260/u16 as u8 size.
    let v = *h.as_ref();
    loop {
        b.resize_as_bytes(n as usize);
        // 0x1 - ObjectNameInformation
        let r = unsafe { f(v, 0x1, b.as_mut_ptr(), b.len_as_bytes() as u32, &mut n) };
        match r {
            0 => break,
            _ if b.len() < n as usize => continue,
            _ => return Err(Win32Error::from_status(r)),
        }
    }
    Ok(utf16_to_string(unsafe {
        b.get_unchecked(PTR_SIZE..((n.min(*b.get_unchecked(0) as u32) as usize / 2) + PTR_SIZE).min(b.len()))
    }))
}
#[inline]
pub fn file_set_attrs(h: impl AsRef<Handle>, attrs: u32) -> Win32Result<()> {
    let i = FileBasicInformation::with_attrs(attrs);
    // 0x4 - FileBasicInfo
    let _ = NtSetInformationFile(h, 0x4, &i, 0x28)?;
    Ok(())
}
#[inline]
pub fn file_rename<'a>(src: impl Into<WCharLike<'a>>, dst: impl Into<WCharLike<'a>>) -> Win32Result<()> {
    let h = NtCreateFile(src, Handle::EMPTY, 0xC0110080, None, 0, 0x7, 0x1, 0x20)?;
    let n = path_normalize(dst);
    if n.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let c = n.len_without_null();
    let mut b: Blob<u16, 128> = Blob::new();
    unsafe {
        b.write(FileRenameInformation {
            root:     Handle::EMPTY,
            name:     0u16,
            replace:  0x1u32,
            name_len: (c as u32).saturating_mul(2),
        });
        // Remove weird padding.
        b.set_len_as_bytes(b.len_as_bytes() - 4);
    };
    b.extend_from_slice(unsafe { n.get_unchecked(0..c) });
    // 0xA - FileRenameInformation
    let _ = NtSetInformationFile(h, 0xA, b.as_ptr(), len_to_u32(b.len_as_bytes()))?;
    Ok(())
}
#[inline]
pub fn file_set_time(h: impl AsRef<Handle>, created: Option<Time>, modified: Option<Time>, access: Option<Time>) -> Win32Result<()> {
    let i = FileBasicInformation {
        attributes:       0u32,
        change_time:      modified.into(),
        creation_time:    created.into(),
        last_write_time:  SysTime::empty(),
        last_access_time: access.into(),
    };
    // 0x4 - FileBasicInfo
    let _ = NtSetInformationFile(h, 0x4, &i, 0x28)?;
    Ok(())
}

#[inline]
pub unsafe fn debug_release() {
    let _ = set_local_privilege(Privilege::SeDebug as u32, false);
}
#[inline]
pub unsafe fn debug_accquire() {
    let _ = set_local_privilege(Privilege::SeDebug as u32, true);
}
#[inline]
pub unsafe fn close_handle(h: Handle) {
    bugtrack!("close_handle(): Closing handle {h:X}!");
    if h.is_invalid() {
        bugtrack!("close_handle(): Attempted to close an invalid Handle!");
        return;
    }
    if *h & 0x10000003 == 0x3 {
        bugtrack!("close_handle(): Attempting to close a Pseudo-Handle 0x{h:X}!");
    }
    let r = syscall!(ntdll().NtClose, (Handle) -> u32, h);
    if r > 0 {
        bugtrack!("close_handle(): NtClose 0x{h:X} resulted in an error 0x{r:X}!");
    }
}
/// This function does NOT do the stdlib checks for Handles.
/// Use the 'WaitForMultipleAsHandles' function for that.
///
/// This function returns Ok(n) for:
/// - 0x0C0 - STATUS_USER_APC
/// - 0x101 - STATUS_ALERTED
/// - 0x102 - STATUS_TIMEOUT
///
/// instead of errors.
pub unsafe fn wait_for_multiple_objects(h: &[usize], size: usize, all: bool, microseconds: u64, alertable: bool) -> Win32Result<u32> {
    if h.len() > 64 || size == 0 || h.len() < size {
        return Err(Win32Error::InvalidArgument);
    }
    let t = (microseconds as i64).wrapping_mul(-10);
    let r = syscall!(
        ntdll().NtWaitForMultipleObjects,
        (u32, *const usize, u32, u32, *const i64) -> u32,
        size as u32,
        h.as_ptr(),
        if all { 0 } else { 1 },
        if alertable { 1 } else { 0 },
        if microseconds == INFINITE { null() } else { &t }
    );
    // STATUS_WAIT_0 .. STATUS_WAIT_63
    if r == 0 || r < 64 {
        return Ok(r);
    }
    // 0x000 - WAIT_OBJECT_0
    // 0x03F - WAIT_OBJECT_63
    // 0x080 - WAIT_ABANDONED_0
    // 0x0BF - WAIT_ABANDONED_63
    match r {
        0x000..=0x3F => Ok(r),
        0x080..=0xBF => Ok(r),
        _ => Err(Win32Error::from_status(r)),
    }
}

#[inline]
fn token_group(h: impl AsRef<Handle>) -> bool {
    let mut b = Vec::new();
    // For Windows XP hack the SID and check for the Local Administrators group.
    token_groups(h, &mut b).map_or(false, |e| {
        e.iter().position(|v| v.sid.is_administrators()).is_some()
    })
}
fn token_elevated(h: impl AsRef<Handle>) -> bool {
    let mut b = [0u8; 32];
    // 0x19 - TokenIntegrityLevel
    let r = GetTokenInformation(&h, 0x19, &mut b, 0x20).unwrap_or_default() as usize;
    if r == 0 || r > 32 {
        return false;
    }
    let p = unsafe { *b.get_unchecked(r - 4) as u32 | (*b.get_unchecked(r - 3) as u32).unchecked_shl(8) | (*b.get_unchecked(r - 2) as u32).unchecked_shl(16) | (*b.get_unchecked(r - 1) as u32).unchecked_shl(24) };
    if p > 0x3000 {
        return true;
    }
    let mut e = 0u32;
    // 0x14 - TokenElevation
    GetTokenInformation(h, 0x14, &mut e, 0x4).unwrap_or_default() == 4 && e != 0
}
fn set_local_privilege(privilege: u32, enabled: bool) -> Win32Result<()> {
    // 0x200E8 - TOKEN_READ (STANDARD_RIGHTS_READ | TOKEN_QUERY) | TOKEN_WRITE
    //            (TOKEN_ADJUST_PRIVILEGES | TOKEN_ADJUST_GROUPS |
    //              TOKEN_ADJUST_DEFAULT)
    let t = current_token(0x200E8)?;
    let p = TokenPrivileges::simple(LUIDAndAttributes::new_u32(privilege, enabled));
    let mut n = 0;
    // Size is always 0x7C.
    AdjustTokenPrivileges(t, false, Some(&p), 0x7C, None, &mut n)
}
