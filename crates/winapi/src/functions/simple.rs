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

// This is for functions that are just "util" or are not syscalls as they
// have been abstracted out or are aliases.
//
// ASM raw calls stay in "functions.rs"

extern crate alloc;
extern crate core;

extern crate xrmt_crypt;
extern crate xrmt_data;

use alloc::string::String;
use core::convert::{AsRef, From, Into};
use core::default::Default;
use core::iter::Iterator;
use core::mem::{replace, size_of};
use core::option::Option::{self, None, Some};
use core::ptr::null_mut;
use core::result::Result::{Err, Ok};

use xrmt_crypt::crypt;
use xrmt_data::text::{str_to_u16_unchecked, utf16_to_string};
use xrmt_data::Blob;

use crate::env::expand_slice;
use crate::functions::{
    current_token,
    file_delete,
    len_to_u32,
    system_root,
    token_user_func,
    win32_file_flags_to_nt,
    GetCurrentProcessPEB,
    GetModuleHandleExW,
    GetProcessHeap,
    HeapFree,
    LsaOpenPolicy,
    LsaQueryInformationPolicy,
    NtCreateFile,
    NtFsControlFile,
    NtOpenKey,
    NtQueryInformationFile,
    NtQueryInformationProcess,
    NtQueryInformationThread,
    NtQueryValueKey,
    NtReadFile,
    NtSetInformationFile,
    NtSetInformationProcess,
    NtSetInformationThread,
    NtWriteFile,
    WaitForSingleObject,
};
use crate::info::is_min_windows_vista;
use crate::structs::{
    to_handle,
    to_writer,
    EnvironmentBlock,
    FileBasicInformation,
    FileStandardInformation,
    Handle,
    HeldPrivilege,
    JoinState,
    LsaDomainInfo,
    LsaPointer,
    MaybeOverlapped,
    MinDumpOutput,
    Overlapped,
    OwnedHandle,
    OwnedKey,
    Privilege,
    ProcessBasicInfo,
    QuotaLimit,
    SecAttrs,
    StringLike,
    StringLikeU16,
    StringWritable,
    SystemVersion,
    ThreadBasicInfo,
    UnicodeString,
    WCharLike,
    WCharSlice,
    HKEY_ROOT,
    VALUE_EXPAND_STRING,
    VALUE_STRING,
};
use crate::{path_normalize, str_const, Win32Error, Win32Result, CURRENT_PROCESS, CURRENT_THREAD, INFINITE, PTR_SIZE};

const TEMP_1: [u16; 3] = [0x54, 0x4D, 0x50];
const TEMP_2: [u16; 4] = [0x54, 0x45, 0x4D, 0x50];
const TEMP_3: [u16; 11] = [0x55, 0x53, 0x45, 0x52, 0x50, 0x52, 0x4F, 0x46, 0x49, 0x4C, 0x45];

pub fn EmptyWorkingSet() -> Win32Result<()> {
    let q = QuotaLimit::default();
    let _h = HeldPrivilege::new(Privilege::SeIncreaseBasePriority);
    let _ = NtSetInformationProcess(
        CURRENT_PROCESS,
        0x1, // 0x1 - ProcessQuotaLimits
        &q,
        size_of::<QuotaLimit>() as u32,
    )?;
    Ok(())
}
#[inline]
pub fn RtlFreeUnicodeString(v: &UnicodeString) {
    let _ = HeapFree(GetProcessHeap(), v.buffer.as_ptr());
}
#[inline]
pub fn CheckRemoteDebuggerPresent(h: impl AsRef<Handle>) -> Win32Result<bool> {
    let mut r = 0usize;
    let _ = NtQueryInformationProcess(h, 0x7, &mut r, PTR_SIZE as u32)?;
    Ok(r > 0)
}
pub fn GetOverlappedResult(h: impl AsRef<Handle>, olp: &mut Overlapped, wait: bool) -> Win32Result<usize> {
    // 0x103 - STATUS_PENDING
    if olp.internal == 0x103 {
        if !wait {
            return Err(Win32Error::IoPending);
        }
        let _ = WaitForSingleObject(
            if *olp.event > 0 { olp.event } else { *h.as_ref() },
            INFINITE,
            true,
        )?;
    }
    let r = replace(&mut olp.internal_high, 0);
    // NOTE(dij): We 'replace' the 'internal_high' value here so we can
    //            prevent 're-reading' it when another call comes in and
    //            completes without waiting OR fails. If we don't clear
    //            it, the previous results still stay and get reported
    //            instead since Windows won't actually update it unless
    //            it does /some/ work. Also I'm betting M$ expects people
    //            to use different Overlapped structures each time?
    match olp.internal {
        0xC0000011 => Ok(0), // 0xC0000011 - STATUS_END_OF_FILE
        0 => Ok(r),
        _ => Err(Win32Error::from_status(olp.internal as u32)),
    }
}

#[inline]
pub fn IsStdinValid() -> bool {
    GetCurrentProcessPEB().process_params().window_flags & 0x200 == 0
}
#[inline]
pub fn IsStdoutValid() -> bool {
    GetCurrentProcessPEB().process_params().window_flags & 0x400 == 0
}

pub fn GetTempPath() -> String {
    let e = GetEnvironment();
    let mut v = if let Some(v) = e
        .iter()
        .find(|v| v.is_key(&TEMP_1) || v.is_key(&TEMP_2))
        .and_then(|v| v.value().map(|s| utf16_to_string(&s)))
    {
        v
    } else if let Some(v) = e
        .iter()
        .find(|v| v.is_key(&TEMP_3))
        .and_then(|v| v.value().map(|s| utf16_to_string(&s)))
    {
        v
    } else {
        // 0x8 - TOKEN_QUERY
        current_token(0x8)
            .and_then(|v| GetUserProfileDirectory(v))
            .unwrap_or_else(|_| utf16_to_string(&system_root()))
    };
    // 1. Try %TEMP% and %TEMP%
    // 2. Try %USERPROFILE%
    // 3. Use 'GetUserProfileDirectory()'
    // 4. Use the Windows directory.
    //
    // Kernel32.dll does the same thing *shrug*
    if v.as_bytes().last().map_or(true, |v| *v != 0x5C) {
        unsafe { v.as_mut_vec().push(0x5C) }
    }
    v
}
#[inline]
pub fn GetVersionNumbers() -> SystemVersion {
    SystemVersion::get()
}
#[inline]
pub fn GetLogicalDrives() -> Win32Result<u32> {
    let mut b = [0u32; 9];
    // 0x17 - ProcessDeviceMap
    NtQueryInformationProcess(CURRENT_PROCESS, 0x17, b.as_mut_ptr(), 0x24)?;
    Ok(unsafe { *b.get_unchecked(0) })
}
#[inline]
pub fn GetCommandLine<'a>() -> WCharSlice<'a> {
    GetCurrentProcessPEB().process_params().command_line.as_slice().into()
}
pub fn GetComputerName() -> Win32Result<String> {
    str_const!(
        0,
        r"\Registry\Machine\System\CurrentControlSet\Services\Tcpip\Parameters",
        69,
        r
    );
    // 0x20019 - READ_CONTROL
    let k = OwnedKey::from(NtOpenKey(HKEY_ROOT, r, 0, 0x20019)?);
    str_const!(0, "Hostname", 9, n);
    let mut b: Blob<u8, 128> = Blob::with_capacity(64);
    match NtQueryValueKey(*k, n, &mut b, 64)? {
        VALUE_STRING | VALUE_EXPAND_STRING => (),
        _ => return Err(Win32Error::InvalidType),
    }
    unsafe {
        let v = b.as_slice_of::<u16>();
        // Remove NULL
        Ok(utf16_to_string(
            v.get_unchecked(0..v.len().saturating_sub(1)),
        ))
    }
}
#[inline]
pub fn GetCurrentDirectory<'a>() -> WCharSlice<'a> {
    GetCurrentProcessPEB()
        .process_params()
        .current_directory
        .dos_path
        .as_slice()
        .into()
}
#[inline]
pub fn GetEnvironment<'a>() -> &'a EnvironmentBlock<'a> {
    &(GetCurrentProcessPEB().process_params().environment)
}
pub fn GetUserProfileDirectory(h: impl AsRef<Handle>) -> Win32Result<String> {
    let k = {
        let mut b: Blob<u16, 200> = Blob::with_capacity(128);
        unsafe {
            str_to_u16_unchecked(
                &mut b,
                crypt!(
                    0,
                    r"\Registry\Machine\Software\Microsoft\Windows NT\CurrentVersion\ProfileList\"
                ),
            )
        };
        // Append user SID
        token_user_func(h, |u| {
            let _ = u.user.sid.into_vec(&mut b);
            Ok(())
        })?;
        // 0x20019 - READ_CONTROL
        OwnedKey::from(NtOpenKey(HKEY_ROOT, &b, 0, 0x20019)?)
    };
    str_const!(0, "ProfileImagePath", 17, n);
    let mut b: Blob<_, 128> = Blob::with_capacity(100);
    match NtQueryValueKey(*k, n, &mut b, 100)? {
        VALUE_STRING | VALUE_EXPAND_STRING => (),
        _ => return Err(Win32Error::InvalidType),
    }
    Ok(utf16_to_string(&expand_slice(
        unsafe { b.as_slice_of::<u16>() },
        GetEnvironment(),
    )))
}
#[inline]
pub fn GetModuleFileName<'a>(h: impl AsRef<Handle>) -> Win32Result<WCharSlice<'a>> {
    let v = *h.as_ref();
    let p = GetCurrentProcessPEB();
    let m = if v.is_invalid() { p.image_base_address } else { v };
    p.modules()
        .find(|i| i.dll_base == m)
        .map(|v| v.name_full.as_slice().into())
        .ok_or(Win32Error::InvalidHandle)
}
#[inline]
pub fn GetEnvironmentVariable<'a>(key: impl Into<WCharLike<'a>>) -> Option<WCharSlice<'a>> {
    unsafe {
        (*(*GetCurrentProcessPEB()).process_parameters)
            .environment
            .find(key)
            .and_then(|v| v.value())
    }
}
#[inline]
pub fn GetModuleHandleEx<'a>(flags: u32, name: impl Into<WCharLike<'a>>) -> Win32Result<Handle> {
    if flags & 0x4 != 0 {
        // GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
        // TODO(dij): Maybe we can suppot this by looping through the modules
        //            loaded and find one in-between the base address and module
        //            size
        //
        // NOTE(dij): Maybe this is too annoying to support?
        return Err(Win32Error::InvalidOperation);
    }
    let n = name.into();
    if n.is_empty() {
        Ok(GetCurrentProcessPEB().image_base_address)
    } else {
        GetModuleHandleExW(flags, n.as_slice())
    }
}

#[inline]
pub fn GetProcessID(h: impl AsRef<Handle>) -> Win32Result<u32> {
    let mut i = ProcessBasicInfo::default();
    // 0x0 - ProcessBasicInformation
    let _ = NtQueryInformationProcess(h, 0, &mut i, size_of::<ProcessBasicInfo>() as u32)?;
    Ok(i.process_id as u32)
}
pub fn IsWoW64Process(h: impl AsRef<Handle>) -> Win32Result<bool> {
    let mut v = 0usize;
    // ^ PEB64 Address will be put in here.
    // 0x1A - ProcessWow64Information
    let _ = NtQueryInformationProcess(h, 0x1A, &mut v, PTR_SIZE as u32)?;
    // If PEB64 address is zero, then we're not running under WoW.
    Ok(v > 0)
}
#[inline]
pub fn GetExitCodeProcess(h: impl AsRef<Handle>) -> Win32Result<u32> {
    let mut i = ProcessBasicInfo::default();
    // 0x0 - ProcessBasicInformation
    let _ = NtQueryInformationProcess(h, 0, &mut i, size_of::<ProcessBasicInfo>() as u32)?;
    Ok(i.exit_status)
}
#[inline]
pub fn SetProcessIsCritical(is_critical: bool) -> Win32Result<bool> {
    let (mut c, s) = (0u32, if is_critical { 1u32 } else { 0u32 });
    // 0x1D - ProcessBreakOnTermination
    let _ = NtQueryInformationProcess(CURRENT_PROCESS, 0x1D, &mut c, 0x4)?;
    // 0x1D - ProcessBreakOnTermination
    let _ = NtSetInformationProcess(CURRENT_PROCESS, 0x1D, &s, 0x4)?;
    Ok(c == 1)
}

pub fn NetGetJoinInformation<'a>(server: impl Into<WCharLike<'a>>) -> Win32Result<(String, JoinState)> {
    // 0x1 - POLICY_VIEW_LOCAL_INFORMATION
    let h = LsaOpenPolicy(0x1, server)?;
    // 0x5 - PolicyAccountDomainInformation
    let i: LsaPointer<LsaDomainInfo> = LsaQueryInformationPolicy(&h, 0xC)?;
    let d = i.as_ref().ok_or(Win32Error::InvalidObject)?;
    let v = match (d.name.is_empty(), d.domain.is_empty()) {
        (false, true) => JoinState::Workgroup,
        (false, false) => JoinState::Domain,
        _ => JoinState::Unjoined,
    };
    Ok((utf16_to_string(&d.name), v))
}

#[inline]
pub fn MiniDumpWriteDump<'a>(h: impl AsRef<Handle>, pid: u32, flags: u32, out: impl Into<MinDumpOutput<'a>>) -> Win32Result<usize> {
    match out.into() {
        MinDumpOutput::Handle(d) => to_handle(h.as_ref(), d, pid, flags),
        MinDumpOutput::Writer(_) if !is_min_windows_vista() => Err(Win32Error::InvalidArgument),
        MinDumpOutput::Writer(w) => to_writer(h.as_ref(), w, pid, flags),
    }
}

#[inline]
pub fn RevertToSelf() -> Win32Result<()> {
    SetThreadToken(CURRENT_THREAD, Handle::EMPTY)
}
#[inline]
pub fn GetThreadID(h: impl AsRef<Handle>) -> Win32Result<u32> {
    let mut i = ThreadBasicInfo::default();
    // 0x0 - ThreadBasicInformation
    let _ = NtQueryInformationThread(h, 0, &mut i, size_of::<ThreadBasicInfo>() as u32)?;
    Ok(i.client_id.thread as u32)
}
#[inline]
pub fn GetExitCodeThread(h: impl AsRef<Handle>) -> Win32Result<u32> {
    let mut i = ThreadBasicInfo::default();
    // 0x0 - ThreadBasicInformation
    let _ = NtQueryInformationThread(h, 0, &mut i, size_of::<ThreadBasicInfo>() as u32)?;
    Ok(i.exit_status)
}
#[inline]
pub fn SetThreadToken(h: impl AsRef<Handle>, token: impl AsRef<Handle>) -> Win32Result<()> {
    // 0x5 - ThreadImpersonationToken
    NtSetInformationThread(h, 0x5, token.as_ref(), PTR_SIZE as u32)
}

pub fn DeleteFile<'a>(file: impl Into<WCharLike<'a>>) -> Win32Result<()> {
    let f = file.into();
    if f.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    // 0x10000 - DELETE
    // 0x00004 - FILE_SHARE_DELETE
    // 0x00001 - FILE_OPEN
    let h = NtCreateFile(f, Handle::EMPTY, 0x10000, None, 0, 0x4, 0x1, 0)?;
    // 0xD - FileDispositionInformation
    let v = 1u32;
    NtSetInformationFile(h, 0xD, &v, 4)?;
    Ok(())
}
#[inline]
pub fn SetFileAttributes<'a>(file: impl Into<WCharLike<'a>>, attrs: u32) -> Win32Result<()> {
    // 0x100110 - SYNCHRONIZE | FILE_WRITE_ATTRIBUTES | FILE_READ_ATTRIBUTES |
    //             FILE_WRITE_EA 0x000007 - FILE_SHARE_READ | FILE_SHARE_WRITE |
    //             FILE_SHARE_DELETE
    // 0x000020 - FILE_SYNCHRONOUS_IO_NONALERT
    // 0x000001 - FILE_OPEN
    let h = NtCreateFile(file, Handle::EMPTY, 0x100190, None, 0, 0x7, 0x1, 0x20)?;
    let i = FileBasicInformation::with_attrs(attrs);
    // 0x4 - FileBasicInfo
    let _ = NtSetInformationFile(h, 0x4, &i, 0x28)?;
    Ok(())
}
pub fn CreateDirectory<'a>(path: impl Into<WCharLike<'a>>, recurse: bool) -> Win32Result<()> {
    let t = path.into();
    if t.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    if !recurse {
        // 0x100001 - SYNCHRONIZE | FILE_LIST_DIRECTORY
        // 0x000080 - FILE_ATTRIBUTE_NORMAL
        // 0x204021 - FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT |
        //             FILE_OPEN_FOR_BACKUP_INTENT | FILE_OPEN_REPARSE_POINT
        // 0x000002 - FILE_CREATE
        let _ = NtCreateFile(t, Handle::EMPTY, 0x100001, None, 0x80, 3, 2, 0x204021)?;
        return Ok(());
    }
    let n = path_normalize(t); // Path should be cleaned up here.
    if n.len() <= 6 {
        // Should be: \??\[LETTER]:
        // This won't work in the object space (it's not supposed to).
        return Err(Win32Error::InvalidArgument);
    }
    // Get position of ':'
    let s = match n.iter().position(|v| *v == 0x3A) {
        Some(i) if i >= 5 && i + 1 < n.len() => i + 1, // Should be after '\??\[LETTER]:'
        _ => return Err(Win32Error::InvalidArgument),  // Shouldn't happen, but gate it.
    };
    // Open Parent
    //
    // 0x100001 - SYNCHRONIZE | FILE_LIST_DIRECTORY
    // 0x000080 - FILE_ATTRIBUTE_NORMAL
    // 0x204021 - FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT |
    //             FILE_OPEN_FOR_BACKUP_INTENT | FILE_OPEN_REPARSE_POINT
    // 0x000003 - FILE_OPEN_IF
    let mut p = NtCreateFile(
        unsafe { n.get_unchecked(0..s) },
        Handle::EMPTY,
        0x100001,
        None,
        0x80,
        0x3,
        0x3,
        0x204021,
    )?;
    // Now iterate and down-create all directories
    // Safe as we already verified it above. Skip the first seperator.
    for i in unsafe { n.get_unchecked(s + 1..n.len_without_null()) }.split(|v| *v == 0x5C) {
        if i.is_empty() {
            continue;
        }
        // 0x100001 - SYNCHRONIZE | FILE_LIST_DIRECTORY
        // 0x000080 - FILE_ATTRIBUTE_NORMAL
        // 0x204021 - FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT |
        //             FILE_OPEN_FOR_BACKUP_INTENT | FILE_OPEN_REPARSE_POINT
        // 0x000003 - FILE_OPEN_IF
        p = NtCreateFile(i, *p, 0x100001, None, 0x80, 0x3, 0x3, 0x204021)?;
    }
    Ok(())
}
pub fn SetFilePointerEx(h: impl AsRef<Handle>, distance: i64, method: u32) -> Win32Result<u64> {
    let n = match method {
        // FILE_BEGIN
        0 => distance as u64,
        // FILE_CURRENT
        1 => {
            let mut n = 0i64;
            // 0xE - FilePositionInformation
            let _ = NtQueryInformationFile(&h, 0xE, &mut n, 8)?;
            (n + distance) as u64
        },
        // FILE_END
        2 => {
            let mut i = FileStandardInformation::default();
            // 0x5 - FileStandardInfo
            let _ = NtQueryInformationFile(&h, 0x5, &mut i, 0x20)?;
            (i.end_of_file as i64 + distance) as u64
        },
        _ => return Err(Win32Error::InvalidArgument),
    };
    // 0xE - FilePositionInformation
    let _ = NtSetInformationFile(h, 0xE, &n, 8)?;
    Ok(n)
}
#[inline]
pub fn WriteFile(h: impl AsRef<Handle>, buf: &[u8], olp: MaybeOverlapped) -> Win32Result<usize> {
    NtWriteFile(h, olp, buf, None)
}
#[inline]
pub fn ReadFile(h: impl AsRef<Handle>, buf: &mut [u8], olp: MaybeOverlapped) -> Win32Result<usize> {
    NtReadFile(h, olp, buf, None)
}
pub fn CreateDirectoryJunction<'a>(link: impl Into<WCharLike<'a>>, target: impl Into<WCharLike<'a>>) -> Win32Result<()> {
    if !is_min_windows_vista() {
        return Err(Win32Error::NotImplemented);
    }
    let l = link.into();
    if l.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let t = target.into();
    if t.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let d = {
        let o = path_normalize(&t);
        let (n, m) = (o.len_without_null() * 2, (o.len_without_null() - 2) * 2); // Length of string in bytes
        let mut b: Blob<u16, 256> = Blob::with_capacity(8 + n);
        unsafe {
            // 0xA000000C - IO_REPARSE_TAG_MOUNT_POINT
            b.write(0xA0000003u32); // ReparseTag
                                    // (NT Target Name in Bytes) + (Target Name in Bytes) + (Size of Union = 8)
                                    // -or-
                                    // (NT Target Name in Bytes) + (NT Target Name in Bytes) - 4 (\??\ Removed)
            b.write((n + m + 8) as u16); // ReparseDataLength
            b.write(0u16); // Reserved
            b.write(0u16); // SubstituteNameOffset
            b.write(n as u16); // SubstituteNameLength (NT Target Name in Bytes)
            b.write((n + 2) as u16); // PrintNameOffset (NT Target Name in Bytes) - 2 (?? Fuck if I know)
            b.write((n - 8) as u16); // PrintNameLength (NT Target Name in Bytes) - 8 (Size of \??\ in Bytes)
            let v = o.len_without_null();
            b.extend_from_slice(o.get_unchecked(0..v)); // PathBuffer
            b.push(0u16); // Add NULL
            b.extend_from_slice(o.get_unchecked(4..v)); // PathBuffer with NT removed.
            b.push(0u16); // Add NULL
        }
        b
    };
    symlink(d, l, true)
}
pub fn CreateHardLink<'a>(link: impl Into<WCharLike<'a>>, target: impl Into<WCharLike<'a>>, replace: bool) -> Win32Result<()> {
    let l = link.into();
    if l.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let t = target.into();
    if t.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let d = {
        let o = path_normalize(l);
        let mut b: Blob<u16, 256> = Blob::with_capacity(24 + o.len());
        unsafe {
            // FileLinkInformation.ReplaceIfExists
            b.write(if replace { 1u32 } else { 0u32 });
            // FileLinkInformation.pad
            #[cfg(target_pointer_width = "64")]
            {
                b.write(0u32); // Padding only needed on 64 bit systems
            }
            // FileLinkInformation.RootDirectory
            b.write(0usize);
            // FileLinkInformation.FileNameLength
            b.write(len_to_u32(o.len_without_null() * 2));
        };
        // Write the ANYSIZE array data
        b.extend_from_slice(&o);
        b
    };
    // 0x100180 - SYNCHRONIZE | FILE_WRITE_ATTRIBUTES | FILE_READ_ATTRIBUTES
    // 0x000007 - FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
    // 0x200020 - FILE_SYNCHRONOUS_IO_NONALERT | FILE_FLAG_OPEN_REPARSE_POINT
    // 0x000001 - FILE_OPEN
    {
        let h = NtCreateFile(t, Handle::EMPTY, 0x100180, None, 0, 0x7, 0x1, 0x200020)?;
        // 0xB - FileLinkInformation
        let e = match NtSetInformationFile(&h, 0xB, d.as_ptr(), len_to_u32(d.len_as_bytes())) {
            Ok(_) => return Ok(()),
            Err(e) => e,
        };
        let _ = file_delete(h);
        Err(e)
    }
}
/// You must have the SeCreateSymbolicLink privilege in order to create a
/// symbolic link. This function does *NOT* do it for you.
///
/// See using [`privilege_accquire`] with [`Privilege`] `SeCreateSymbolicLink`
/// to assist.
pub fn CreateSymbolicLink<'a>(link: impl Into<WCharLike<'a>>, target: impl Into<WCharLike<'a>>, flags: u32) -> Win32Result<()> {
    if !is_min_windows_vista() {
        return Err(Win32Error::NotImplemented);
    }
    let l = link.into();
    if l.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let t = target.into();
    if t.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let d = {
        let o = path_normalize(&t);
        let n = o.len_without_null() * 2; // Length of string in bytes
        let mut b: Blob<u16, 256> = Blob::with_capacity(20 + n);
        unsafe {
            // 0xA000000C - IO_REPARSE_TAG_SYMLINK
            b.write(0xA000000Cu32); // ReparseTag
                                    // (NT Target Name in Bytes) * 2 + (Size of Union = 12)
            b.write((n + n + 12) as u16); // ReparseDataLength
            b.write(0u16); // Reserved
            b.write(0u16); // SubstituteNameOffset
            b.write(n as u16); // SubstituteNameLength (NT Target Name in Bytes)
            b.write(n as u16); // PrintNameOffset (NT Target Name in Bytes)
            b.write(n as u16); // PrintNameLength (NT Target Name in Bytes)
            b.write(0u32); // Flags
                           // Since we're expanding the path, all paths are absolute (0).
            let v = o.len_without_null();
            b.extend_from_slice(o.get_unchecked(0..v)); // PathBuffer
            b.extend_from_slice(o.get_unchecked(0..v)); // PrintBuffer
        }
        b
    };
    symlink(d, l, flags & 0x1 != 0)
}
#[inline]
pub fn ReOpenFile(file: impl AsRef<Handle>, access: u32, share_mode: u32, sa: SecAttrs, disposition: u32, attrs: u32) -> Win32Result<OwnedHandle> {
    let (a, v, d, f) = win32_file_flags_to_nt(access, disposition, attrs);
    NtCreateFile(WCharLike::Null, file, a, sa, v, share_mode, d, f)
}
#[inline]
pub fn CreateFile<'a>(file: impl Into<WCharLike<'a>>, access: u32, share_mode: u32, sa: SecAttrs, disposition: u32, attrs: u32) -> Win32Result<OwnedHandle> {
    let p = file.into();
    if p.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let (a, v, d, f) = win32_file_flags_to_nt(access, disposition, attrs);
    NtCreateFile(p, Handle::EMPTY, a, sa, v, share_mode, d, f)
}

fn symlink<'a>(data: Blob<u16, 256>, link: WCharLike<'a>, dir: bool) -> Win32Result<()> {
    // 0x110100 - FILE_WRITE_ATTRIBUTES | DELETE | SYNCHRONIZE
    // 0x000080 - FILE_ATTRIBUTE_NORMAL
    // 0x000002 - FILE_CREATE
    // 0x200020 - FILE_SYNCHRONOUS_IO_NONALERT | FILE_OPEN_REPARSE_POINT
    // 0x000040 - FILE_NON_DIRECTORY_FILE
    // 0x000001 - FILE_DIRECTORY_FILE
    // 0x004000 - FILE_FLAG_BACKUP_SEMANTICS
    {
        let h = NtCreateFile(
            link,
            Handle::EMPTY,
            0x110100,
            None,
            0x80,
            0,
            0x2,
            0x204020 | if dir { 0x1 } else { 0x40 },
        )?;
        // 0x900A4 - FSCTL_SET_REPARSE_POINT
        let r = NtFsControlFile::<_, ()>(
            &h,
            0x900A4,
            None,
            data.as_ptr(),
            len_to_u32(data.len_as_bytes()),
            null_mut(),
            0,
        );
        let e = match r {
            Ok(_) => return Ok(()),
            Err(e) => e,
        };
        // Remove the failed link file.
        let _ = file_delete(h);
        Err(e)
    }
}
