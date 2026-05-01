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

extern crate xrmt_bugtrack;
extern crate xrmt_crypt;
extern crate xrmt_data;

use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::cell::UnsafeCell;
use core::cmp::Ord;
use core::convert::{AsRef, From, Into};
use core::default::Default;
use core::iter::Iterator;
use core::matches;
use core::mem::drop;
use core::num::NonZeroU32;
use core::option::Option::{self, None, Some};
use core::ptr::{null, null_mut};
use core::result::Result::{Err, Ok};
use core::slice::from_raw_parts;

use xrmt_bugtrack::bugtrack;
use xrmt_crypt::crypt;
use xrmt_data::text::{str_to_u16_unchecked, utf16_to_string};
use xrmt_data::Blob;

use crate::functions::{len_to_u32, system_dir, wait_for_multiple_objects, win32_handle_to_nt, GetCurrentProcessID, GetCurrentProcessPEB, RtlGenRandom};
use crate::info::{is_min_windows_7, is_min_windows_8, is_min_windows_vista};
use crate::loader::kernel32_or_base;
use crate::structs::{
    AnsiString,
    CharLike,
    ClientID,
    Handle,
    ImageResource,
    IoStatusBlock,
    Key,
    MaybeOverlapped,
    NonZeroHandle,
    ObjectAttributes,
    ObjectBasicInformation,
    Overlapped,
    OverlappedPtr,
    OwnedHandle,
    ReadInto,
    RegKeyBasicInfo,
    RegKeyFullInfo,
    RegValueFullInfo,
    Region,
    SecAttrs,
    SecurityAttributes,
    SecurityQualityOfService,
    StringLike,
    TimerFunc,
    TokenPrivileges,
    UnicodeString,
    WChar,
    WCharLike,
    WChars,
    WriteFrom,
};
use crate::utils::write_hex_padded;
use crate::{ntdll, object_attrs, object_normalize, object_normalize_path, path_normalize, raw_path, str_const, unicode_string, Win32Error, Win32Result, CURRENT_PROCESS, INFINITE, PTR_SIZE};

/////////////////////////////////////
// AsHandle and Handle Functions
/////////////////////////////////////
pub fn CloseHandle(h: Handle) -> Win32Result<()> {
    if h.is_invalid() {
        bugtrack!("CloseHandle(): Attempted to close an invalid Handle!");
        return Ok(());
    }
    if *h & 0x10000003 == 0x3 {
        bugtrack!("CloseHandle(): Attempting to close a Pseudo-Handle 0x{h:X}!");
    }
    let r = syscall!(ntdll().NtClose, (Handle) -> u32, h);
    bugtrack!("CloseHandle(): Closing Handle 0x{h:X} (result {r:X}).");
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn GetObjectInformation(h: impl AsRef<Handle>) -> Win32Result<ObjectBasicInformation> {
    let mut i = ObjectBasicInformation::default();
    let r = syscall!(
        ntdll().NtQueryObject,
        (Handle, u32, *mut ObjectBasicInformation, u32, *mut u32) -> u32,
        *h.as_ref(),
        0, // 0x0 - ObjectBasicInformation
        &mut i,
        0x38, // ObjectBasicInformation has a fixed size of 56.
        null_mut()
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(i)
    }
}
pub fn SetHandleInformation(h: impl AsRef<Handle>, inherit: bool, protect: bool) -> Win32Result<()> {
    let b = [if inherit { 1u8 } else { 0u8 }, if protect { 1u8 } else { 0u8 }];
    let r = syscall!(
        ntdll().NtSetInformationObject,
        (Handle, u32, *const u8, u32) -> u32,
        *h.as_ref(),
        0x4, // 0x4 - ObjectFlagInformation
        b.as_ptr(),
        0x2
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn DuplicateHandle(src: impl AsRef<Handle>, access: u32, opts: u32) -> Win32Result<OwnedHandle> {
    DuplicateHandleEx(src, CURRENT_PROCESS, CURRENT_PROCESS, access, false, opts).map(|h| h.into())
}
pub fn DuplicateHandleEx(src: impl AsRef<Handle>, src_proc: impl AsRef<Handle>, dst_proc: impl AsRef<Handle>, access: u32, inherit: bool, opts: u32) -> Win32Result<Handle> {
    let (v, s, d) = (src.as_ref(), src_proc.as_ref(), dst_proc.as_ref());
    // Check to see if the Handle is a STDIN Console Handle THEN check to see if
    // we're older than Win8 as we need to use 'DuplicateConsoleHandle' as the
    // NtDuplicateObject call can't handle those types of Handles.
    //
    // This is what kernel32.dll does!
    let (f, n) = if *s == CURRENT_PROCESS && *d == CURRENT_PROCESS && (**v & 0x10000003) == 0x3 && !is_min_windows_8() {
        (kernel32_or_base().DuplicateHandle, false)
    } else {
        (ntdll().NtDuplicateObject, true)
    };
    let mut h = Handle::default();
    let r = syscall!(
        f,
        (Handle, usize, Handle, *mut Handle, u32, u32, u32) -> u32,
        *s,
        if n { win32_handle_to_nt(*v) } else { **v},
        *d,
        &mut h,
        access,
        if inherit { 0x2 } else { 0 }, // 0x2 - OBJ_INHERIT
        opts
    );
    if n && r > 0 {
        Err(Win32Error::from_status(r))
    } else if !n && r == 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(h)
    }
}

/////////////////////////////////////
// Library/DLL Loader Functions
/////////////////////////////////////
#[inline]
pub fn FreeLibrary(dll: Handle) -> Win32Result<()> {
    let r = syscall!(ntdll().LdrUnloadDll, (Handle) -> u32, dll);
    bugtrack!("FreeLibrary(): Free-ing DLL 0x{dll:X} (result {r:X}).");
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn LoadLibraryW(path: &[u16]) -> Win32Result<NonZeroHandle> {
    // Raw UTF16 version to prevent allocating back into a string.
    let n = WCharLike::from(path);
    let u = UnicodeString::new(&n);
    // We don't use the macro here as we might not have a NULL ending.
    unsafe { LdlLoadLibrary(&u) }
}
pub fn GetModuleHandleExW(flags: u32, name: &[u16]) -> Win32Result<Handle> {
    // Raw UTF16 version to prevent allocating back into a string.
    if flags & 0x4 != 0 {
        // GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
        // NOTE(dij): This is too annoying to support.
        return Err(Win32Error::InvalidOperation);
    }
    if name.len() == 0 {
        return Ok(GetCurrentProcessPEB().image_base_address);
    }
    let mut f = 0;
    // Translate kernel32 flags to NT flags.
    if flags & 0x1 != 0 {
        // GET_MODULE_HANDLE_EX_FLAG_PIN
        f |= 0x2 // LDR_GET_DLL_HANDLE_EX_PIN
    }
    if flags & 0x2 != 0 {
        // GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT
        f |= 0x1 // LDR_GET_DLL_HANDLE_EX_UNCHANGED_REFCOUNT
    }
    let mut n: WChars = name.into();
    let i = n.len();
    // . d l l NULL
    // 5 4 3 2 1
    // Add '.dll' extension if it isn't present.
    if n.len() > 5 && unsafe { *n.get_unchecked(i - 5) != 0x2E && !matches!(*n.get_unchecked(i - 4), 0x44 | 0x64) && !matches!(*n.get_unchecked(i - 2), 0x4C | 0x6C) } {
        unsafe { *n.get_unchecked_mut(i - 1) = 0x2E }; // Add '.'
        n.reserve(4);
        n.push(0x64);
        n.push(0x6C);
        n.push(0x6C);
        n.push(0); // Re-add NULL
    }
    unicode_string!(&n, u);
    let mut h = Handle::default();
    // Use the system directory if no fullpath is specified.
    let r = if name.iter().position(|v| *v == 0x2F || *v == 0x5C).is_some() {
        syscall!(ntdll().LdrGetDllHandleEx, (u32, *const u16, *const u32, *const UnicodeString, *mut Handle) -> u32, f, null(), null(), &u, &mut h)
    } else {
        let d = system_dir(); // Only create this if we need it.
        syscall!(ntdll().LdrGetDllHandleEx, (u32, *const u16, *const u32, *const UnicodeString, *mut Handle) -> u32, f, d.as_ptr(), null(), &u, &mut h)
    };
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h)
    }
}
#[inline]
pub fn LoadLibrary<'a>(path: impl Into<WCharLike<'a>>) -> Win32Result<NonZeroHandle> {
    // Don't convert to a NT path.
    let n = path.into();
    if n.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let u = UnicodeString::new(&n);
    // We don't use the macro here as we might not have a NULL ending. and we
    // need to prevent empty/null values.
    unsafe { LdlLoadLibrary(&u) }
}
pub fn FindMessage(code: u32, lang: u32, module: Handle, buf: &mut [u16]) -> Win32Result<usize> {
    if code == 0 || !ntdll().RtlFindMessage.is_loaded() {
        return Ok(0);
    }
    let mut v = null_mut();
    let r = syscall!(
        ntdll().RtlFindMessage,
        (Handle, u32, u32, u32, *mut *mut ImageResource) -> u32,
        module,
        0xB, // 0xB - ??
        lang,
        code,
        &mut v
    );
    if r > 0 {
        return Err(Win32Error::from_status(r));
    } else if v.is_null() {
        return Err(Win32Error::NotFound);
    }
    Ok(unsafe { &*v }.copy_into(buf))
}
#[inline]
pub fn GetProcAddress<'a>(h: Handle, ordinal: u16, name: impl Into<CharLike<'a>>) -> Win32Result<usize> {
    let n = name.into();
    let a = AnsiString::new(&n);
    unsafe { LdlLoadAddress(h, ordinal, &a) }
}

/////////////////////////////////////
// Pipe/NamedPipe Functions
/////////////////////////////////////
pub fn DisconnectNamedPipe(h: impl AsRef<Handle>) -> Win32Result<()> {
    // NOTE(dij): I don't think this will ever block as far as I can tell, so
    //            we should be fine omitting the Overlapped.
    let mut i = IoStatusBlock::default();
    let r = syscall!(
        ntdll().NtFsControlFile,
        (Handle, usize, usize, *mut Overlapped, *mut IoStatusBlock, u32, *const u8, u32, *mut u8, u32) -> u32,
        *h.as_ref(),
        0,
        0,
        null_mut(),
        &mut i,
        0x110004, // 0x110004 - FSCTL_PIPE_DISCONNECT
        null(),
        0,
        null_mut(),
        0
    );
    // 0x103 - STATUS_PENDING
    let e = if r == 0x103 {
        let _ = WaitForSingleObject(h, INFINITE, false);
        i.status as u32
    } else {
        r
    };
    if e > 0 {
        Err(Win32Error::from_status(e))
    } else {
        Ok(())
    }
}
#[inline]
pub fn ImpersonateNamedPipeClient(h: impl AsRef<Handle>) -> Win32Result<()> {
    // 0x11001C - FSCTL_PIPE_IMPERSONATE
    let _ = NtFsControlFile::<(), ()>(h, 0x11001C, None, null(), 0, null_mut(), 0)?;
    Ok(())
}
#[inline]
pub fn ConnectNamedPipe(h: impl AsRef<Handle>, olp: MaybeOverlapped) -> Win32Result<()> {
    // 0x110008 - FSCTL_PIPE_LISTEN
    let _ = NtFsControlFile::<(), ()>(h, 0x110008, olp, null(), 0, null_mut(), 0)?;
    Ok(())
}
pub fn CreatePipe(sa: SecAttrs, size: u32, olp: bool) -> Win32Result<(OwnedHandle, OwnedHandle)> {
    if !is_min_windows_vista() {
        // Handle Windows Xp/Server 2003 gracefully.
        return create_pipe_xp(sa, size, olp);
    }
    let mut i = IoStatusBlock::default();
    let (r, h) = {
        let d = {
            str_const!(0, r"\Device\NamedPipe\", 19, p);
            // 0x80100000 - GENERIC_READ | SYNCHRONIZE
            NtCreateFile(&p, Handle::EMPTY, 0x80100000, sa, 0, 0x3, 0x1, 0x20)?
        };
        // 0x40 - OBJ_CASE_INSENSITIVE
        object_attrs!(*d, false, 0x40, sa, None, o);
        let mut h = Handle::default();
        let t = -500000i64;
        let r = syscall!(
            ntdll().NtCreateNamedPipeFile,
            (*mut Handle, u32, *const ObjectAttributes, *mut IoStatusBlock, u32, u32, u32, u32, u32, u32, u32, u32, u32, *const i64) -> u32,
            &mut h,
            0x80100100, // 0x80100100 - GENERIC_READ | FILE_WRITE_ATTRIBUTES | SYNCHRONIZE
            &o,
            &mut i,
            0x3, // 0x3 - FILE_SHARE_WRITE | FILE_SHARE_READ
            0x2, // 0x2 - FILE_CREATE
            if olp { 0 } else { 0x20 }, // 0x20 - FILE_SYNCHRONOUS_IO_NONALERT
            0x2, // 0x2 - PIPE_REJECT_REMOTE_CLIENTS
            0,
            0,
            0x1,
            size,
            size,
            &t
        );
        drop(d); // Forcefully drop the Handle.
        (r, OwnedHandle::from(h))
    };
    if r > 0 {
        return Err(Win32Error::from_status(r));
    }
    let mut n = Handle::default();
    let r = {
        // 0x40 - OBJ_CASE_INSENSITIVE
        object_attrs!(*h, false, 0x40, sa, None, o);
        syscall!(
            ntdll().NtCreateFile,
            (*mut Handle, u32, *const ObjectAttributes, *mut IoStatusBlock, *mut u32, u32, u32, u32, u32, *mut u8, u32) -> u32,
            &mut n,
            0x40100080, // 0x120116 - FILE_GENERIC_WRITE
            &o,
            &mut i,
            null_mut(),
            0,
            0x1,  // 0x01 - FILE_SHARE_READ
            0x1,  // 0x01 - FILE_OPEN
            0x40 | if olp { 0 } else { 0x20 }, // 0x40 - FILE_NON_DIRECTORY_FILE | 0x20 - FILE_SYNCHRONOUS_IO_NONALERT
            null_mut(),
            0
        )
    };
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok((h, n.into()))
    }
}
pub fn WaitNamedPipe<'a>(name: impl Into<WCharLike<'a>>, olp: MaybeOverlapped, microseconds: u64) -> Win32Result<()> {
    let n = name.into();
    if n.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let (k, d) = {
        // Resolve path name, append "\Device\NamedPipe\" if value is just a
        // name and not a path, otherwise, resolve it to an NT path.
        let v = raw_path(n, || crypt!(0, r"\Device\NamedPipe\"));
        // 13 Is the size of 'FilePipeWait'.
        let mut p: WChars = Blob::with_capacity(13 + (v.len() * 2));
        unsafe {
            // FilePipeWait.timeout
            p.write(if microseconds > 0 {
                (microseconds as i64).saturating_mul(-10)
            } else if microseconds == INFINITE {
                0x4000000000000000i64
            } else {
                0i64
            });
            // FilePipeWait.name_length
            p.write(len_to_u32(v.len() * 2));
            // FilePipeWait.timeout_specified
            p.write(if microseconds != 0 { 1u8 } else { 0u8 });
        }
        // Copy converted name over to the "FilePipeWait" struct.
        p.extend_from_slice(&v);
        (v, p)
    };
    // 0x000003 - FILE_SHARE_READ | FILE_SHARE_WRITE
    // 0x000020 - FILE_SYNCHRONOUS_IO_NONALERT
    // 0x100080 - FILE_READ_ATTRIBUTES | SYNCHRONIZE
    let h = NtCreateFile(k, Handle::EMPTY, 0x100080, None, 0, 0x3, 0, 0x20)?;
    // 0x110018 - FSCTL_PIPE_WAIT
    let _ = NtFsControlFile::<_, ()>(
        h,
        0x110018,
        olp,
        d.as_ptr(),
        len_to_u32(d.len_as_bytes()),
        null_mut(),
        0,
    )?;
    Ok(())
}
pub fn CreateNamedPipe<'a>(name: impl Into<WCharLike<'a>>, mode: u32, pipe_mode: u32, max: u32, in_buf: u32, out_buf: u32, microseconds: u64, sa: SecAttrs) -> Win32Result<OwnedHandle> {
    let n = name.into();
    if n.is_empty() || max == 0 {
        return Err(Win32Error::InvalidArgument);
    }
    // 0x0100000 - SYNCHRONIZE
    // 0x10C0000 - WRITE_DAC | WRITE_OWNER | ACCESS_SYSTEM_SECURITY
    let mut a = 0x100000 | (mode & 0x10C0000);
    let (mut m, mut s) = (0u32, 0u32);
    if mode & 0x80000000 != 0 {
        // 0x80000000 - FILE_FLAG_WRITE_THROUGH
        //
        // NOTE(dij): De-compiling kernelbase.dll on Win10 seems to suggest that
        //            a '0x100000000' value also specifies this?
        //              "dwOpenMode >> 0x1f & 2"
        //
        m |= 0x2; // 0x2 - FILE_WRITE_THROUGH
    }
    if mode & 0x40000000 != 0 {
        // 0x40000000 - FILE_FLAG_OVERLAPPED
        m |= 0x20; // 0x20- FILE_SYNCHRONOUS_IO_NONALERT
    }
    if mode & 0x2 != 0 {
        // 0x2 - PIPE_ACCESS_OUTBOUND
        s |= 0x1; // 0x1 - FILE_SHARE_READ
        a |= 0x40000000; // 0x40000000 - GENERIC_WRITE
    }
    if mode & 0x1 != 0 {
        // 0x1 - PIPE_ACCESS_INBOUND
        s |= 0x2; // 0x2 - FILE_SHARE_WRITE
        a |= 0x80000000; // 0x80000000 - GENERIC_READ
    }
    let w = if pipe_mode & 0x4 != 0 {
        // PIPE_TYPE_MESSAGE
        0x1 // FILE_PIPE_MESSAGE_TYPE
    } else {
        0 // FILE_PIPE_BYTE_STREAM_TYPE
    } | if is_min_windows_vista() && pipe_mode & 0x8 != 0 {
        0x2
    } else {
        0
    };
    // 0x8 - PIPE_REJECT_REMOTE_CLIENTS
    //       The 0x2 is the NT flag for this and will cause any Xp pipe operations
    //       to fail, so we only add it if we're >= Vista.
    let q = if pipe_mode & 0x2 != 0 {
        // PIPE_READMODE_MESSAGE
        0x1 // FILE_PIPE_MESSAGE_MODE
    } else {
        0 // FILE_PIPE_BYTE_STREAM_MODE
    };
    let b = if pipe_mode & 0x1 != 0 {
        // PIPE_NOWAIT
        0x1 // FILE_PIPE_COMPLETE_OPERATION
    } else {
        // FILE_PIPE_QUEUE_OPERATION
        0
    };
    let t = if microseconds > 0 {
        (microseconds as i64).wrapping_mul(-10)
    } else {
        -500000
    };
    let c = match max {
        0xFF => -1,
        _ if max > 0xFF => -1,
        _ => max as i32,
    };
    let mut h = Handle::default();
    let r = {
        unicode_string!(raw_path(n, || crypt!(0, r"\Device\NamedPipe\")), u);
        let mut i = IoStatusBlock::default();
        // 0x40 - OBJ_CASE_INSENSITIVE
        let o = ObjectAttributes::file(&u, false, 0x40, sa, None);
        syscall!(
            ntdll().NtCreateNamedPipeFile,
            (*mut Handle, u32, *const ObjectAttributes, *mut IoStatusBlock, u32, u32, u32, u32, u32, u32, i32, u32, u32, *const i64) -> u32,
            &mut h,
            a,
            &o,
            &mut i,
            s,
            // 0x80000 - FILE_FLAG_FIRST_PIPE_INSTANCE
            if mode & 0x80000 != 0 { 0x2 } else { 0x3 }, // 0x2 - FILE_CREATE | 0x3 - FILE_OPEN_IF
            m,
            w,
            q,
            b,
            c,
            in_buf,
            out_buf,
            &t
        )
    };
    // 0xC0000010 - STATUS_INVALID_DEVICE_REQUEST
    // 0xC00000BB - STATUS_NOT_SUPPORTED
    // 0xC0000033 - STATUS_OBJECT_NAME_INVALID
    match r {
        0xC0000010 | 0xC00000BB => Err(Win32Error::InvalidName),
        0 => Ok(h.into()),
        _ => Err(Win32Error::from_status(r)),
    }
}

/////////////////////////////////////
// IoCompletionPort Functions
/////////////////////////////////////
#[inline]
pub fn QueryIoCompletion(h: impl AsRef<Handle>) -> Win32Result<u32> {
    let (mut n, mut c) = (0u32, 0u32);
    // 0x0 - IoCompletionBasicInformation
    let r = syscall!(ntdll().NtQueryIoCompletion, (Handle, u32, *mut u32, u32, *mut u32) -> u32, *h.as_ref(), 0, &mut n, 4, &mut c);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}
pub fn OpenIoCompletion<'a>(name: impl Into<WCharLike<'a>>) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    object_attrs!(
        name object_normalize_path(false, name)?,
        false,
        0,
        None,
        None,
        o
    );
    let r = syscall!(ntdll().NtOpenIoCompletion, (*mut Handle, u32, *const ObjectAttributes) -> u32, &mut h, 0x1F0003, &o);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}
pub fn CreateIoCompletion<'a>(threads: Option<NonZeroU32>, name: impl Into<WCharLike<'a>>) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    object_attrs!(
        name object_normalize(name),
        false,
        0,
        None,
        None,
        o
    );
    let r = syscall!(
        ntdll().NtCreateIoCompletion,
        (*mut Handle, u32, *const ObjectAttributes, u32) -> u32,
        &mut h,
        0x1F0003,
        &o,
        threads.map(|v| v.get()).unwrap_or(0)
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}
pub fn RemoveIoCompletion<T>(h: impl AsRef<Handle>, key: &mut *mut T, olp: &mut *mut Overlapped, microseconds: u64) -> Win32Result<usize> {
    let t = (microseconds as i64).wrapping_mul(-10);
    let mut i = IoStatusBlock::default();
    let r = syscall!(
        ntdll().NtRemoveIoCompletion,
        (Handle, *mut *mut T, *mut *mut Overlapped, *mut IoStatusBlock, *const i64) -> u32,
        *h.as_ref(),
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

/////////////////////////////////////
// Registry Functions
/////////////////////////////////////
#[inline]
pub fn NtFlushKey(key: Key) -> Win32Result<()> {
    let r = syscall!(ntdll().NtFlushKey, (Key) -> u32, key);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn NtDeleteKey(key: Key) -> Win32Result<()> {
    let r = syscall!(ntdll().NtDeleteKey, (Key) -> u32, key);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn NtDeleteValueKey<'a>(key: Key, value: impl Into<WCharLike<'a>>) -> Win32Result<()> {
    let n = value.into();
    let u = UnicodeString::new(&n);
    // Don't use the macro so we don't have to check for null as we might not have
    // it
    let r = syscall!(ntdll().NtDeleteValueKey, (Key, *const UnicodeString) -> u32, key, if u.is_empty() { null() } else { &u });
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn NtQueryKeyInfo<'a>(key: Key, buf: &'a mut Vec<u8>) -> Win32Result<&'a RegKeyFullInfo> {
    let f = syscall!(
        ntdll().NtQueryKey,
        fn(Key, u32, *mut u8, u32, *mut u32) -> u32
    );
    let mut n = 0x12Cu32; // Size of RegKeyFullInfo
    loop {
        buf.resize((n as usize) * 2, 0);
        // 0x2 - KeyFullInformation
        let r = unsafe { f(key, 0x2, buf.as_mut_ptr(), n, &mut n) };
        match r {
            // 0x80000005 - STATUS_BUFFER_OVERFLOW
            // 0xC0000004 - STATUS_INFO_LENGTH_MISMATCH
            // 0xC0000023 - STATUS_BUFFER_TOO_SMALL
            0x80000005 | 0xC0000004 | 0xC0000023 => continue,
            0 => break,
            _ => return Err(Win32Error::from_status(r)),
        }
    }
    Ok(unsafe { &*(buf.as_ptr() as *const RegKeyFullInfo) })
}
pub fn NtOpenKey<'a>(root: Key, subkey: impl Into<WCharLike<'a>>, opts: u32, access: u32) -> Win32Result<Key> {
    let mut k = Key::default();
    let r = {
        // 0x008 - REG_OPTION_OPEN_LINK
        // 0x040 - OBJ_CASE_INSENSITIVE
        // 0x100 - OBJ_OPENLINK
        object_attrs!(
            subkey,
            *root,
            false,
            0x40 | if opts & 0x8 != 0 { 0x100 } else { 0 },
            None,
            None,
            o
        );
        syscall!(
            ntdll().NtOpenKey,
            (*mut Key, u32, *const ObjectAttributes) -> u32,
            &mut k,
            access,
            &o
        )
    };
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(k)
    }
}
pub fn NtEnumerateKey<'a>(key: Key, index: u32, buf: &'a mut Vec<u8>) -> Win32Result<Option<&'a RegKeyBasicInfo>> {
    let f = syscall!(
        ntdll().NtEnumerateKey,
        fn(Key, u32, u32, *mut u8, u32, *mut u32) -> u32
    );
    let mut n = 0x110u32; // Size of RegKeyBasicInfo
    loop {
        buf.resize((n as usize) * 2, 0);
        // 0x0 - KeyBasicInformation
        let r = unsafe { f(key, index, 0, buf.as_mut_ptr(), n, &mut n) };
        match r {
            // 0x8000001A - STATUS_NO_MORE_ENTRIES
            0x8000001A => return Ok(None),
            // 0x80000005 - STATUS_BUFFER_OVERFLOW
            // 0xC0000004 - STATUS_INFO_LENGTH_MISMATCH
            // 0xC0000023 - STATUS_BUFFER_TOO_SMALL
            0x80000005 | 0xC0000004 | 0xC0000023 => continue,
            0 => break,
            _ => return Err(Win32Error::from_status(r)),
        }
    }
    Ok(Some(unsafe { &*(buf.as_ptr() as *const RegKeyBasicInfo) }))
}
pub fn NtEnumerateValueKey<'a>(key: Key, index: u32, buf: &'a mut Vec<u8>) -> Win32Result<Option<&'a RegValueFullInfo>> {
    let f = syscall!(
        ntdll().NtEnumerateValueKey,
        fn(Key, u32, u32, *mut u8, u32, *mut u32) -> u32
    );
    let mut n = 0x114u32; // Size of RegKeyValueFullInfo
    loop {
        buf.resize(n as usize, 0);
        // 0x1 - KeyValueFullInformation
        let r = unsafe { f(key, index, 0x1, buf.as_mut_ptr(), n, &mut n) };
        match r {
            // 0x8000001A - STATUS_NO_MORE_ENTRIES
            0x8000001A => return Ok(None),
            0x80000005 | 0xC0000004 | 0xC0000023 => continue,
            // 0x80000005 - STATUS_BUFFER_OVERFLOW
            // 0xC0000004 - STATUS_INFO_LENGTH_MISMATCH
            // 0xC0000023 - STATUS_BUFFER_TOO_SMALL
            0 => break,
            _ => return Err(Win32Error::from_status(r)),
        }
    }
    Ok(Some(unsafe { &*(buf.as_ptr() as *const RegValueFullInfo) }))
}
pub fn NtSetValueKey<'a>(key: Key, value: impl Into<WCharLike<'a>>, value_type: u32, data: Option<impl AsRef<[u8]>>, data_size: u32) -> Win32Result<()> {
    unicode_string!(value, u);
    let r = syscall!(
        ntdll().NtSetValueKey,
        (Key, *const UnicodeString, u32, u32, *const u8, u32) -> u32,
        key,
        if u.is_empty() { null() } else { &u },
        0,
        value_type,
        data.map_or_else(null, |v| v.as_ref().as_ptr()),
        data_size
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn NtQueryValueKey<'a, A: Allocator, const N: usize>(key: Key, value: impl Into<WCharLike<'a>>, buf: &mut Blob<u8, N, A>, size: u32) -> Win32Result<u8> {
    unicode_string!(value, u);
    let mut s = size;
    let f = syscall!(
        ntdll().NtQueryValueKey,
        fn(Key, *const UnicodeString, u32, *mut u8, u32, *mut u32) -> u32
    );
    loop {
        buf.resize(s as usize);
        let r = unsafe {
            f(
                key,
                if u.is_empty() { null() } else { &u },
                0x2, // 0x2 - KeyValuePartialInformation
                buf.as_mut_ptr(),
                s,
                &mut s,
            )
        };
        match r {
            // 0xC0000023 - STATUS_BUFFER_TOO_SMALL
            // 0x80000005 - STATUS_BUFFER_OVERFLOW
            0x80000005 | 0xC0000023 => continue,
            0 => break,
            _ => return Err(Win32Error::from_status(r)),
        }
    }
    buf.truncate(s as usize);
    // No need to use a struct, it's a "flat" struct, which can be
    // seen as a slice.
    //
    // RegKeyValuePartialInfo {
    //   pub index:      u32, // 0
    //   pub value_type: u32, // 1
    //   pub length:     u32, // 2
    // }
    //
    let i = unsafe { from_raw_parts(buf.as_ptr_of::<u32>(), 3) };
    // Store the info, as we change the data later.
    let (n, t) = unsafe { (*i.get_unchecked(2) as usize, *i.get_unchecked(1) as u8) };
    // 0xC is size of RegKeyValuePartialInfo.
    buf.drain(0xC);
    buf.truncate(n);
    Ok(t)
}
pub fn NtCreateKey<'a>(root: Key, subkey: impl Into<WCharLike<'a>>, class: impl Into<WCharLike<'a>>, opts: u32, access: u32, sa: SecAttrs) -> Win32Result<(Key, bool)> {
    let n = subkey.into();
    if n.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let (mut d, mut k) = (0u32, Key::default());
    let r = {
        // 0x008 - REG_OPTION_OPEN_LINK
        // 0x040 - OBJ_CASE_INSENSITIVE
        // 0x100 - OBJ_OPENLINK
        object_attrs!(
            n,
            *root,
            false,
            0x40 | if opts & 0x8 != 0 { 0x100 } else { 0 },
            sa,
            None,
            o
        );
        unicode_string!(class, u);
        syscall!(
            ntdll().NtCreateKey,
            (*mut Key, u32, *const ObjectAttributes, u32, *const UnicodeString, u32, *mut u32) -> u32,
            &mut k,
            access,
            &o,
            0,
            if u.is_empty() { null() } else { &u },
            opts,
            &mut d
        )
    };
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok((k, d == 1))
    }
}

/////////////////////////////////////
// Mailslot Functions
/////////////////////////////////////
pub fn CreateMailslot<'a>(name: impl Into<WCharLike<'a>>, max_message: u32, microseconds: u64, sa: SecAttrs) -> Win32Result<OwnedHandle> {
    let n = name.into();
    if n.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let t = if microseconds == INFINITE {
        INFINITE as i64
    } else {
        (microseconds as i64).saturating_mul(-10)
    };
    let mut h = Handle::default();
    let mut i = IoStatusBlock::default();
    let r = {
        // 0x40 - OBJ_CASE_INSENSITIVE
        object_attrs!(
            name raw_path(n, || crypt!(0, r"\Device\Mailslot\")),
            false,
            0x40,
            sa,
            None,
            o
        );
        syscall!(
            ntdll().NtCreateMailslotFile,
            (*mut Handle, u32, *const ObjectAttributes, *mut IoStatusBlock, u32, u32, u32, *const i64) -> u32,
            &mut h,
            0x80140000, // 0x80140000 - GENERIC_READ | SYNCHRONIZE | WRITE_DAC
            &o,
            &mut i,
            0x2, // 0x2 - FILE_WRITE_THROUGH
            0,
            max_message,
            &t
        )
    };
    // 0xC0000010 - STATUS_INVALID_DEVICE_REQUEST
    // 0xC00000BB - STATUS_NOT_SUPPORTED
    match r {
        0xC00000BB | 0xC0000010 => Err(Win32Error::InvalidName),
        0 => Ok(h.into()),
        _ => Err(Win32Error::from_status(r)),
    }
}

/////////////////////////////////////
// Heap Functions
/////////////////////////////////////
#[inline]
pub fn HeapDestroy(h: impl AsRef<Handle>) -> Win32Result<()> {
    let r = syscall!(ntdll().RtlDestroyHeap, (Handle) -> Handle, *h.as_ref());
    if r.is_invalid() {
        Ok(())
    } else {
        Err(Win32Error::last_error())
    }
}
#[inline]
pub fn HeapFree(h: impl AsRef<Handle>, addr: impl Into<Region>) -> bool {
    syscall!(ntdll().RtlFreeHeap, (Handle, u32, Region) -> u32, *h.as_ref(), 0, addr.into()) == 1
}
#[inline]
pub fn HeapAlloc(h: impl AsRef<Handle>, s: usize, zeroed: bool) -> Win32Result<Region> {
    let r = syscall!(ntdll().RtlAllocateHeap, (Handle, u32, usize) -> Region, *h.as_ref(), if zeroed { 0x8 } else { 0 }, s);
    if r.is_invalid() {
        Err(Win32Error::last_error())
    } else {
        Ok(r)
    }
}
#[inline]
pub fn HeapCreate(flags: u32, initial_size: usize, max_size: usize) -> Win32Result<Handle> {
    let r = syscall!(
        ntdll().RtlCreateHeap,
        (u32, *const (), usize, usize, *const (), *const ()) -> Handle,
        flags,
        null(),
        initial_size,
        max_size,
        null(),
        null()
    );
    if r.is_invalid() {
        Err(Win32Error::last_error())
    } else {
        Ok(r)
    }
}
#[inline]
pub fn HeapReAlloc(h: impl AsRef<Handle>, flags: u32, old: Region, size: usize) -> Win32Result<Region> {
    let r = syscall!(ntdll().RtlReAllocateHeap, (Handle, u32, Region, usize) -> Region, *h.as_ref(), flags, old, size);
    if r.is_invalid() {
        Err(Win32Error::last_error())
    } else {
        Ok(r)
    }
}

/////////////////////////////////////
// Mutex Functions
/////////////////////////////////////
#[inline]
pub fn QueryMutex(h: impl AsRef<Handle>) -> Win32Result<i32> {
    let mut n = [0u32, 0u32];
    let r = syscall!(
        ntdll().NtQueryMutant,
        (Handle, u32, *mut u32, u32, *mut u32) -> u32,
        *h.as_ref(),
        0,
        n.as_mut_ptr(),
        0x8,
        null_mut()
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(unsafe { *n.get_unchecked(0) } as i32)
    }
}
#[inline]
pub fn ReleaseMutex(h: impl AsRef<Handle>) -> Win32Result<()> {
    let r = syscall!(ntdll().NtReleaseMutant, (Handle, *mut u32) -> u32, *h.as_ref(), null_mut());
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn OpenMutex<'a>(access: u32, inherit: bool, name: impl Into<WCharLike<'a>>) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    object_attrs!(
        name object_normalize_path(false, name)?,
        inherit,
        0,
        None,
        None,
        o
    );
    let r = syscall!(ntdll().NtOpenMutant, (*mut Handle, u32, *const ObjectAttributes) -> u32, &mut h, access, &o);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}
pub fn CreateMutex<'a>(sa: SecAttrs, inherit: bool, initial: bool, name: impl Into<WCharLike<'a>>) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    object_attrs!(
        name object_normalize(name),
        inherit,
        0,
        sa,
        None,
        o
    );
    let r = syscall!(
        ntdll().NtCreateMutant,
        (*mut Handle, u32, *const ObjectAttributes, u32) -> u32,
        &mut h,
        0x1F0001,
        &o,
        if initial { 1 } else { 0 }
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}

/////////////////////////////////////
// Event Functions
/////////////////////////////////////
#[inline]
pub fn SetEvent(h: impl AsRef<Handle>) -> Win32Result<()> {
    let r = syscall!(ntdll().NtSetEvent, (Handle, *mut u32) -> u32, *h.as_ref(), null_mut());
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn ResetEvent(h: impl AsRef<Handle>) -> Win32Result<()> {
    let r = syscall!(ntdll().NtResetEvent, (Handle, *mut u32) -> u32, *h.as_ref(), null_mut());
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn PulseEvent(h: impl AsRef<Handle>) -> Win32Result<()> {
    let r = syscall!(ntdll().NtPulseEvent, (Handle, *mut u32) -> u32, *h.as_ref(), null_mut());
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn QueryEvent(h: impl AsRef<Handle>) -> Win32Result<u32> {
    let mut n = [0u32, 0u32];
    let r = syscall!(
        ntdll().NtQueryEvent,
        (Handle, u32, *mut u32, u32, *mut u32) -> u32,
        *h.as_ref(),
        0,
        n.as_mut_ptr(),
        0x8,
        null_mut()
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(unsafe { *n.get_unchecked(1) })
    }
}
pub fn OpenEvent<'a>(access: u32, inherit: bool, name: impl Into<WCharLike<'a>>) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    object_attrs!(
        name object_normalize_path(false, name.into())?,
        inherit,
        0,
        None,
        None,
        o
    );
    let r = syscall!(
        ntdll().NtOpenEvent,
        (*mut Handle, u32, *const ObjectAttributes) -> u32,
        &mut h,
        access,
        &o
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}
pub fn CreateEvent<'a>(sa: SecAttrs, inherit: bool, initial: bool, manual: bool, name: impl Into<WCharLike<'a>>) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    object_attrs!(
        name object_normalize(name),
        inherit,
        0,
        sa,
        None,
        o
    );
    let r = syscall!(
        ntdll().NtCreateEvent,
        (*mut Handle, u32, *const ObjectAttributes, u32, u32) -> u32,
        &mut h,
        0x1F0003,
        &o,
        if manual { 0 } else { 1 },
        if initial { 1 } else { 0 }
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}

/////////////////////////////////////
// Keyed Event Functions
/////////////////////////////////////
pub fn OpenKeyedEvent<'a>(access: u32, inherit: bool, name: impl Into<WCharLike<'a>>) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    object_attrs!(
        name object_normalize_path(false, name)?,
        inherit,
        0,
        None,
        None,
        o
    );
    let r = syscall!(
        ntdll().NtOpenKeyedEvent,
        (*mut Handle, u32, *const ObjectAttributes) -> u32,
        &mut h,
        access,
        &o
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}
pub fn CreateKeyedEvent<'a>(sa: SecAttrs, inherit: bool, name: impl Into<WCharLike<'a>>) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    object_attrs!(
        name object_normalize(name.into()),
        inherit,
        0,
        sa,
        None,
        o
    );
    let r = syscall!(
        ntdll().NtCreateKeyedEvent,
        (*mut Handle, u32, *const ObjectAttributes, u32) -> u32,
        &mut h,
        0x1F0003,
        &o,
        0
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}
/// "Fires" a KeyedEvent. This will block until another call to
/// [`WaitForKeyedEvent`] or will activate any functions blocked on
/// [`WaitForKeyedEvent`].
pub fn SetKeyedEvent(h: impl AsRef<Handle>, key: impl Into<usize>, microseconds: u64, alertable: bool) -> Win32Result<()> {
    let t = (microseconds as i64).wrapping_mul(-10);
    let r = syscall!(
        ntdll().NtReleaseKeyedEvent,
        (Handle, usize, u32, *const i64) -> u32,
        *h.as_ref(),
        key.into(),
        if alertable { 1 } else { 0 },
        if microseconds == INFINITE { null() } else { &t }
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
/// Waits for a KeyedEvent to be "triggered" by [`SetKeyedEvent`]
pub fn WaitForKeyedEvent(h: impl AsRef<Handle>, key: impl Into<usize>, microseconds: u64, alertable: bool) -> Win32Result<()> {
    let t = (microseconds as i64).wrapping_mul(-10);
    let r = syscall!(
        ntdll().NtWaitForKeyedEvent,
        (Handle, usize, u32, *const i64) -> u32,
        *h.as_ref(),
        key.into(),
        if alertable { 1 } else { 0 },
        if microseconds == INFINITE { null() } else { &t }
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}

/////////////////////////////////////
// Semaphore Functions
/////////////////////////////////////
pub fn QuerySemaphore(h: impl AsRef<Handle>) -> Win32Result<(u32, u32)> {
    let mut n = [0u32, 0u32];
    let r = syscall!(
        ntdll().NtQuerySemaphore,
        (Handle, u32, *mut u32, u32, *mut u32) -> u32,
        *h.as_ref(),
        0,
        n.as_mut_ptr(),
        0x8,
        null_mut()
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(unsafe { (*n.get_unchecked(0), *n.get_unchecked(1)) })
    }
}
#[inline]
pub fn ReleaseSemaphore(h: impl AsRef<Handle>, count: u32) -> Win32Result<u32> {
    let mut n = 0u32;
    let r = syscall!(
        ntdll().NtReleaseSemaphore,
        (Handle, u32, *mut u32) -> u32,
        *h.as_ref(),
        count,
        &mut n
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}
pub fn OpenSemaphore<'a>(access: u32, inherit: bool, name: impl Into<WCharLike<'a>>) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    object_attrs!(
        name object_normalize_path(false, name)?,
        inherit,
        0,
        None,
        None,
        o
    );
    let r = syscall!(
        ntdll().NtOpenSemaphore,
        (*mut Handle, u32, *const ObjectAttributes) -> u32,
        &mut h,
        access,
        &o
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}
pub fn CreateSemaphore<'a>(sa: SecAttrs, inherit: bool, initial: u32, max: u32, name: impl Into<WCharLike<'a>>) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    object_attrs!(
        name object_normalize(name),
        inherit,
        0,
        sa,
        None,
        o
    );
    let r = syscall!(
        ntdll().NtCreateSemaphore,
        (*mut Handle, u32, *const ObjectAttributes, u32, u32) -> u32,
        &mut h,
        0x1F0003,
        &o,
        initial,
        max
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}

/////////////////////////////////////
// Timer Functions
/////////////////////////////////////
#[inline]
pub fn CancelWaitableTimer(h: impl AsRef<Handle>) -> Win32Result<bool> {
    let mut n = 0u32;
    let r = syscall!(ntdll().NtCancelTimer, (Handle, *mut u32) -> u32, *h.as_ref(), &mut n);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n == 1)
    }
}
#[inline]
pub fn QueryWaitableTimer(h: impl AsRef<Handle>) -> Win32Result<(bool, u64)> {
    let mut n = [0u64, 0u64];
    let r = syscall!(
        ntdll().NtQueryTimer,
        (Handle, u32, *mut u64, u32, *mut u32) -> u32,
        *h.as_ref(),
        0,
        n.as_mut_ptr(),
        0x10,
        null_mut()
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(unsafe { (*n.get_unchecked(1) > 0, *n.get_unchecked(0)) })
    }
}
pub fn OpenWaitableTimer<'a>(access: u32, inherit: bool, name: impl Into<WCharLike<'a>>) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    object_attrs!(
        name object_normalize_path(false, name)?,
        inherit,
        0,
        None,
        None,
        o
    );
    let r = syscall!(
        ntdll().NtOpenTimer,
        (*mut Handle, u32, *const ObjectAttributes) -> u32,
        &mut h,
        access,
        &o
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}
pub fn CreateWaitableTimer<'a>(sa: SecAttrs, inherit: bool, manual: bool, name: impl Into<WCharLike<'a>>) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    object_attrs!(
        name object_normalize(name),
        inherit,
        0,
        sa,
        None,
        o
    );
    let r = syscall!(
        ntdll().NtCreateTimer,
        (*mut Handle, u32, *const ObjectAttributes, u32) -> u32,
        &mut h,
        0x1F0003, // 0x1F0003 - FULL_CONTROL (for Timers)
        &o,
        if manual { 0 } else { 1 }
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}
pub fn SetWaitableTimer(h: impl AsRef<Handle>, microseconds: u64, repeat_in_mills: u32, func: Option<TimerFunc>, arg: Option<usize>, resume: bool) -> Win32Result<bool> {
    let mut n = 0u32;
    let t = (microseconds as i64).wrapping_mul(-10);
    let r = syscall!(
        ntdll().NtSetTimer,
        (Handle, *const i64, Option<TimerFunc>, usize, u32, u32, *mut u32) -> u32,
        *h.as_ref(),
        &t,
        func,
        arg.unwrap_or_default(),
        if resume { 1 } else { 0 },
        repeat_in_mills,
        &mut n
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n == 1)
    }
}

/////////////////////////////////////
// Sleep/Yield Functions
/////////////////////////////////////
#[inline]
pub fn NtYieldExecution() -> Win32Result<()> {
    let r = syscall!(ntdll().NtYieldExecution, () -> u32,);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn SleepEx(microseconds: u64, alertable: bool) -> Win32Result<bool> {
    let t = if microseconds == INFINITE {
        0x8000000000000000u64 as i64
    } else {
        (microseconds as i64).wrapping_mul(-10)
    };
    match syscall!(ntdll().NtDelayExecution, (u32, *const i64) -> u32, if alertable { 1 } else { 0 }, &t) {
        // Don't return error if sleep was interrupted.
        0xC0 => Ok(true), // 0xC0 - STATUS_USER_APC
        0 => Ok(false),
        v => Err(Win32Error::from_status(v)),
    }
}

/////////////////////////////////////
// System Information Functions
/////////////////////////////////////
#[inline]
pub fn NtQuerySystemInformation<T>(class: u32, buf: *mut T, size: u32) -> Win32Result<u32> {
    let mut n: u32 = 0u32;
    let r = syscall!(
        ntdll().NtQuerySystemInformation,
        (u32, *mut T, u32, *mut u32) -> u32,
        class,
        buf,
        size,
        &mut n
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}
#[inline]
pub fn NtQuerySystemInformationEx<T>(class: u32, input: Option<&T>, input_len: u32, output: Option<&mut T>, output_len: u32) -> Win32Result<u32> {
    if !is_min_windows_7() {
        return Err(Win32Error::NotImplemented);
    }
    let mut n: u32 = 0u32;
    let r = syscall!(
        ntdll().NtQuerySystemInformationEx,
        (u32, Option<&T>, u32, Option<&mut T>, u32, *mut u32) -> u32,
        class,
        input,
        input_len,
        output,
        output_len,
        &mut n
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}

/////////////////////////////////////
// Environment Functions
/////////////////////////////////////
#[inline]
pub fn SetCurrentDirectory<'a>(dir: impl Into<WCharLike<'a>>) -> Win32Result<()> {
    let n = dir.into();
    if n.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let u = UnicodeString::new(&n);
    let r = syscall!(ntdll().RtlSetCurrentDirectory_U, (*const UnicodeString) -> u32, &u);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn SetEnvironmentVariable<'a>(key: impl Into<WCharLike<'a>>, value: impl Into<WCharLike<'a>>) -> Win32Result<()> {
    let r = {
        let k = key.into();
        if k.is_empty() {
            return Err(Win32Error::InvalidArgument);
        }
        let v = value.into();
        syscall!(
            ntdll().RtlSetEnvironmentVar,
            (usize, *const u16, usize, *const u16, usize) -> u32,
            0,
            k.as_ptr(),
            k.len(),
            v.as_ptr(),
            v.len()
        )
    };
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}

/////////////////////////////////////
// Thread Functions
/////////////////////////////////////
#[inline]
pub fn ResumeThread(h: impl AsRef<Handle>) -> Win32Result<u32> {
    let mut n = 0u32;
    let r = syscall!(ntdll().NtResumeThread, (Handle, *mut u32) -> u32, *h.as_ref(), &mut n);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}
#[inline]
pub fn SuspendThread(h: impl AsRef<Handle>) -> Win32Result<u32> {
    let mut n = 0u32;
    let r = syscall!(ntdll().NtSuspendThread, (Handle, *mut u32) -> u32, *h.as_ref(), &mut n);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}
#[inline]
pub fn TerminateThread(h: impl AsRef<Handle>, code: u32) -> Win32Result<()> {
    let r = syscall!(ntdll().NtTerminateThread, (Handle, u32) -> u32, *h.as_ref(), code);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn OpenThread(access: u32, inherit: bool, tid: u32) -> Win32Result<OwnedHandle> {
    object_attrs!(inherit, 0, None, None, o);
    let c = ClientID::thread(tid);
    let mut h = Handle::default();
    let r = syscall!(
        ntdll().NtOpenThread,
        (*mut Handle, u32, *const ObjectAttributes, *const ClientID) -> u32,
        &mut h,
        access,
        &o,
        &c
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}
#[inline]
pub fn NtSetInformationThread<T>(h: impl AsRef<Handle>, class: u32, buf: *const T, size: u32) -> Win32Result<()> {
    let r = syscall!(
        ntdll().NtSetInformationThread,
        (Handle, u32, *const T, u32) -> u32,
        *h.as_ref(),
        class,
        buf,
        size
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn NtQueryInformationThread<T>(h: impl AsRef<Handle>, class: u32, buf: *mut T, size: u32) -> Win32Result<u32> {
    let mut n: u32 = 0u32;
    let r = syscall!(
        ntdll().NtQueryInformationThread,
        (Handle, u32, *mut T, u32, *mut u32) -> u32,
        *h.as_ref(),
        class,
        buf,
        size,
        &mut n
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}
#[inline]
pub fn NtImpersonateThread(h: impl AsRef<Handle>, src: impl AsRef<Handle>, s: &SecurityQualityOfService) -> Win32Result<()> {
    let r = syscall!(
        ntdll().NtImpersonateThread,
        (Handle, Handle, *const SecurityQualityOfService) -> u32,
        *h.as_ref(),
        *src.as_ref(),
        s
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn CreateThreadEx(h: impl AsRef<Handle>, stack_size: usize, start: usize, args: usize, suspended: bool) -> Win32Result<OwnedHandle> {
    // NtCreateThreadEx is avaliable after Windows Xp.
    if !is_min_windows_vista() {
        let t = syscall!(
            kernel32_or_base().CreateRemoteThread,
            (Handle, *const SecurityAttributes, usize, usize, usize, u32, *mut u32) -> Handle,
            *h.as_ref(),
            null(),
            stack_size,
            start,
            args,
            // 0x00004 - CREATE_SUSPENDED
            // 0x10000 - STACK_SIZE_PARAM_IS_A_RESERVATION
            if suspended { 0x4 } else { 0 } | if stack_size > 0 { 0x10000 } else { 0 },
            null_mut()
        );
        return if t.is_invalid() {
            Err(Win32Error::last_error())
        } else {
            Ok(t.into())
        };
    }
    let mut t = Handle::default();
    let r = syscall!(
        ntdll().NtCreateThreadEx,
        (*mut Handle, u32, *const ObjectAttributes, Handle, usize, usize, u32, usize, usize, usize, usize) -> u32,
        &mut t,
        0x1FFFFF, // 0x1FFFFF - ALL_ACCESS
        null(),
        *h.as_ref(),
        start,
        args,
        if suspended { 1 } else { 0 },
        0,
        stack_size,
        0,
        0
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(t.into())
    }
}

/////////////////////////////////////
// Process Functions
/////////////////////////////////////
#[inline]
pub fn NtResumeProcess(h: impl AsRef<Handle>) -> Win32Result<()> {
    let r = syscall!(ntdll().NtResumeProcess, (Handle) -> u32, *h.as_ref());
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn NtSuspendProcess(h: impl AsRef<Handle>) -> Win32Result<()> {
    let r = syscall!(ntdll().NtSuspendProcess, (Handle) -> u32, *h.as_ref());
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn TerminateProcess(h: impl AsRef<Handle>, code: u32) -> Win32Result<()> {
    let r = syscall!(ntdll().NtTerminateProcess, (Handle, u32) -> u32, *h.as_ref(), code);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn GetProcessFileName(h: impl AsRef<Handle>, full: bool) -> Win32Result<String> {
    let f = syscall!(
        ntdll().NtQueryInformationProcess,
        fn(Handle, u32, *mut u16, u32, *mut u32) -> u32
    );
    let v = *h.as_ref();
    let mut n = 0x20Au32; // 261 WCHARS
    let mut b: WChars = Blob::new();
    loop {
        b.resize_as_bytes(n as usize);
        // 0x1B - ProcessImageFileName
        let r = unsafe { f(v, 0x1B, b.as_mut_ptr(), b.len_as_bytes() as u32, &mut n) };
        match r {
            0xC0000004 => continue, // STATUS_INFO_LENGTH_MISMATCH
            0 => break,
            _ => return Err(Win32Error::from_status(r)),
        }
    }
    // Retrieves a UNICODE_STRING value containing the name of the image file
    // for the process.
    let i = b.len().min(n as usize);
    let s = unsafe { b.get_unchecked(PTR_SIZE..(i / 2) - 1) };
    // First u16 is the 'length' value.
    let t = i.min(unsafe { *b.get_unchecked(0) as usize }) / 2; // UNICODE_STRING.Length
    if !full {
        if let Some(i) = s.iter().rposition(|v| *v == 0x5C) {
            return Ok(utf16_to_string(unsafe { s.get_unchecked(i + 1..t) }));
        }
    }
    Ok(utf16_to_string(unsafe { s.get_unchecked(0..t) }))
}
#[inline]
pub fn OpenProcess(access: u32, inherit: bool, pid: u32) -> Win32Result<OwnedHandle> {
    object_attrs!(inherit, 0, None, None, o);
    let c = ClientID::process(pid);
    let mut h = Handle::default();
    let r = syscall!(
        ntdll().NtOpenProcess,
        (*mut Handle, u32, *const ObjectAttributes, *const ClientID) -> u32,
        &mut h,
        access,
        &o,
        &c
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}
#[inline]
pub fn NtSetInformationProcess<T>(h: impl AsRef<Handle>, class: u32, buf: *const T, size: u32) -> Win32Result<()> {
    let r = syscall!(
        ntdll().NtSetInformationProcess,
        (Handle, u32, *const T, u32) -> u32,
        *h.as_ref(),
        class,
        buf,
        size
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn NtQueryInformationProcess<T>(h: impl AsRef<Handle>, class: u32, buf: *mut T, size: u32) -> Win32Result<u32> {
    let mut n = 0u32;
    let r = syscall!(
        ntdll().NtQueryInformationProcess,
        (Handle, u32, *mut T, u32, *mut u32) -> u32,
        *h.as_ref(),
        class,
        buf,
        size,
        &mut n
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}

/////////////////////////////////////
// Process Functions (WoW64)
/////////////////////////////////////
//#[cfg(not(target_pointer_width = "64"))]
pub fn NtWow64QueryInformationProcess64<T>(h: impl AsRef<Handle>, class: u32, buf: *mut T, size: u32) -> Win32Result<u32> {
    let mut n = 0u32;
    let r = syscall!(
        ntdll().NtWow64QueryInformationProcess64,
        (Handle, u32, *mut T, u32, *mut u32) -> u32,
        *h.as_ref(),
        class,
        buf,
        size,
        &mut n
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}

/////////////////////////////////////
// IO/Async Functions
/////////////////////////////////////
pub fn CancelIoEx(h: impl AsRef<Handle>, olp: &mut Overlapped) -> Win32Result<()> {
    let mut s = IoStatusBlock::default();
    let r = if is_min_windows_7() {
        // NtCancelIoFileEx only exists on Win7+
        syscall!(ntdll().NtCancelIoFileEx, (Handle, *mut Overlapped, *mut IoStatusBlock) -> u32, *h.as_ref(), olp, &mut s)
    } else {
        syscall!(ntdll().NtCancelIoFile, (Handle, *mut IoStatusBlock) -> u32, *h.as_ref(), &mut s)
    };
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn WaitForSingleObject(h: impl AsRef<Handle>, microseconds: u64, alertable: bool) -> Win32Result<u32> {
    let t = (microseconds as i64).wrapping_mul(-10);
    let r = syscall!(
        ntdll().NtWaitForSingleObject,
        (usize, u32, *const i64) -> u32,
        win32_handle_to_nt(*h.as_ref()),
        if alertable { 1 } else { 0 },
        if microseconds == INFINITE { null() } else { &t }
    );
    // 0x000 - WAIT_OBJECT_0
    // 0x080 - WAIT_ABANDONED_0
    // 0x0C0 - STATUS_USER_APC
    // 0x101 - STATUS_ALERTED
    // 0x102 - STATUS_TIMEOUT
    match r {
        0 | 0x80 => Ok(r),
        _ => Err(Win32Error::from_status(r)),
    }
}
pub fn WaitForMultipleObjects<T: AsRef<Handle>>(h: &[T], size: usize, all: bool, microseconds: u64, alertable: bool) -> Win32Result<u32> {
    if h.len() > 64 || size > 64 || size == 0 {
        return Err(Win32Error::InvalidArgument);
    }
    let mut x = [0usize; 64];
    for i in 0..size.min(h.len()) {
        // Size can never be > 64 and never larger than 'h'
        unsafe { *x.get_unchecked_mut(i) = win32_handle_to_nt(*h.get_unchecked(i).as_ref()) };
    }
    unsafe { wait_for_multiple_objects(&x, size, all, microseconds, alertable) }
}

/////////////////////////////////////
// Token Functions
/////////////////////////////////////
pub fn OpenProcessToken(h: impl AsRef<Handle>, access: u32) -> Win32Result<OwnedHandle> {
    let mut t = Handle::default();
    let r = syscall!(ntdll().NtOpenProcessToken, (Handle, u32, *mut Handle) -> u32, *h.as_ref(), access, &mut t);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(t.into())
    }
}
pub fn OpenThreadToken(h: impl AsRef<Handle>, access: u32, s: bool) -> Win32Result<OwnedHandle> {
    let mut t = Handle::default();
    let r = syscall!(
        ntdll().NtOpenThreadToken,
        (Handle, u32, u32, *mut Handle) -> u32,
        *h.as_ref(),
        access,
        if s { 1 } else { 0 },
        &mut t
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(t.into())
    }
}
pub fn GetTokenInformation<T>(h: impl AsRef<Handle>, class: u32, buf: *mut T, size: u32) -> Win32Result<u32> {
    let mut n = size;
    let r = syscall!(
        ntdll().NtQueryInformationToken,
        (Handle, u32, *mut T, u32, *mut u32) -> u32,
        *h.as_ref(),
        class,
        buf,
        size,
        &mut n
    );
    // 0xC0000023 - STATUS_BUFFER_TOO_SMALL
    if r == 0xC0000023 && n > 0 {
        Ok(n)
    } else if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}
#[inline]
pub fn SetTokenInformation<T>(h: impl AsRef<Handle>, class: u32, buf: *const T, size: u32) -> Win32Result<()> {
    let r = syscall!(
        ntdll().NtSetInformationToken,
        (Handle, u32, *const T, u32) -> u32,
        *h.as_ref(),
        class,
        buf,
        size
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn DuplicateTokenEx(h: impl AsRef<Handle>, access: u32, sa: SecAttrs, level: u32, token: u32) -> Win32Result<OwnedHandle> {
    let mut n = Handle::default();
    let q = SecurityQualityOfService::level(level);
    object_attrs!(false, 0, sa, Some(&q), o);
    let r = syscall!(
        ntdll().NtDuplicateToken,
        (Handle, u32, *const ObjectAttributes, u32, u32, *mut Handle) -> u32,
        *h.as_ref(),
        access,
        &o,
        0,
        token,
        &mut n
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n.into())
    }
}
#[inline]
pub fn AdjustTokenPrivileges(h: impl AsRef<Handle>, dis: bool, new: Option<&TokenPrivileges>, new_len: u32, old: Option<&mut TokenPrivileges>, old_len: &mut u32) -> Win32Result<()> {
    let r = syscall!(
        ntdll().NtAdjustPrivilegesToken,
        (Handle, u32, Option<&TokenPrivileges>, u32,  Option<&mut TokenPrivileges>, *mut u32) -> u32,
        *h.as_ref(),
        if dis { 1 } else { 0 },
        new,
        new_len,
        old,
        old_len
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}

/////////////////////////////////////
// Virtual Memory Functions
/////////////////////////////////////
pub fn NtFreeVirtualMemory(h: impl AsRef<Handle>, address: impl Into<Region>, size: usize, flags: u32) -> Win32Result<()> {
    let (mut s, mut a) = (size, address.into());
    let r = syscall!(
        ntdll().NtFreeVirtualMemory,
        (Handle, *mut Region, *mut usize, u32) -> u32,
        *h.as_ref(),
        &mut a,
        &mut s,
        flags
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn NtProtectVirtualMemory(h: impl AsRef<Handle>, address: impl Into<Region>, size: usize, access: u32) -> Win32Result<u32> {
    let (mut s, mut n, mut x) = (0u32, size, address.into());
    let r = syscall!(
        ntdll().NtProtectVirtualMemory,
        (Handle, *mut Region, *mut usize, u32, *mut u32) -> u32,
        *h.as_ref(),
        &mut x,
        &mut n,
        access,
        &mut s
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(s)
    }
}
pub fn NtReadVirtualMemory<T>(h: impl AsRef<Handle>, address: impl Into<Region>, size: usize, to: ReadInto<T>) -> Win32Result<usize> {
    let mut s = 0usize;
    let r = syscall!(
        ntdll().NtReadVirtualMemory,
        (Handle, Region, *mut T, usize, *mut usize) -> u32,
        *h.as_ref(),
        address.into(),
        match to {
            ReadInto::Direct(v) => v,
            ReadInto::Pointer(p) => p,
        },
        size,
        &mut s
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(s)
    }
}
pub fn NtWriteVirtualMemory<T>(h: impl AsRef<Handle>, address: impl Into<Region>, size: usize, from: WriteFrom<T>) -> Win32Result<usize> {
    let mut s = 0usize;
    let r = syscall!(
        ntdll().NtWriteVirtualMemory,
        (Handle, Region, *const T, usize, *mut usize) -> u32,
        *h.as_ref(),
        address.into(),
        match from {
            WriteFrom::Null => null(),
            WriteFrom::Direct(v) => v,
            WriteFrom::Pointer(p) => p,
        },
        size,
        &mut s
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(s)
    }
}
pub fn NtQueryVirtualMemory<T>(h: impl AsRef<Handle>, address: impl Into<Region>, class: u32, buf: *mut T, size: u32) -> Win32Result<u32> {
    let (mut n, a) = (size, address.into());
    let r = syscall!(
        ntdll().NtQueryVirtualMemory,
        (Handle, Region, u32, *mut T, u32, *mut u32) -> u32,
        *h.as_ref(),
        a,
        class,
        buf,
        size,
        &mut n
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}
pub fn NtAllocateVirtualMemory(h: impl AsRef<Handle>, base: impl Into<Region>, size: usize, flags: u32, access: u32) -> Win32Result<Region> {
    let (mut s, mut a) = (size, base.into());
    let r = syscall!(
        ntdll().NtAllocateVirtualMemory,
        (Handle, *mut Region, usize, *mut usize, u32, u32) -> u32,
        *h.as_ref(),
        &mut a,
        0,
        &mut s,
        flags,
        access
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(a)
    }
}

/////////////////////////////////////
// Virtual Memory Functions (WoW64)
/////////////////////////////////////
//#[cfg(not(target_pointer_width = "64"))]
pub fn NtWoW64ReadVirtualMemory64<T>(h: impl AsRef<Handle>, address: u64, size: u64, to: ReadInto<T>) -> Win32Result<u64> {
    let mut s = 0u64;
    let r = syscall!(
        ntdll().NtWow64ReadVirtualMemory64,
        (Handle, u64, *mut T, u64, *mut u64) -> u32,
        *h.as_ref(),
        address,
        match to {
            ReadInto::Direct(v) => v,
            ReadInto::Pointer(p) => p,
        },
        size,
        &mut s
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(s)
    }
}
//#[cfg(not(target_pointer_width = "64"))]
pub fn NtWoW64WriteVirtualMemory64<T>(h: impl AsRef<Handle>, address: u64, size: u64, from: WriteFrom<T>) -> Win32Result<u64> {
    let mut s = 0u64;
    let r = syscall!(
        ntdll().NtWow64WriteVirtualMemory64,
        (Handle, u64, *const T, u64, *mut u64) -> u32,
        *h.as_ref(),
        address,
        match from {
            WriteFrom::Null => null(),
            WriteFrom::Direct(v) => v,
            WriteFrom::Pointer(p) => p,
        },
        size,
        &mut s
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(s)
    }
}
//#[cfg(not(target_pointer_width = "64"))]
pub fn NtWoW64AllocateVirtualMemory64(h: impl AsRef<Handle>, base: u64, size: u64, flags: u32, access: u32) -> Win32Result<u64> {
    let mut a = base;
    let r = {
        let mut s = size;
        syscall!(
            ntdll().NtWow64AllocateVirtualMemory64,
            (Handle, *mut u64, u64, *mut u64, u32, u32) -> u32,
            *h.as_ref(),
            &mut a,
            0,
            &mut s,
            flags,
            access
        )
    };
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(a)
    }
}

/////////////////////////////////////
// Section Functions
/////////////////////////////////////
pub fn NtUnmapViewOfSection(sec: impl AsRef<Handle>, proc: impl AsRef<Handle>) -> Win32Result<()> {
    let r = syscall!(ntdll().NtUnmapViewOfSection, (Handle, Handle) -> u32, *sec.as_ref(), *proc.as_ref());
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn NtCreateSection(access: u32, max_size: Option<u64>, protection: u32, attrs: u32, file: Option<impl AsRef<Handle>>) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    let m = max_size.map_or_else(null, |v| &v);
    let r = syscall!(
        ntdll().NtCreateSection,
        (*mut Handle, u32, *const ObjectAttributes, *const u64, u32, u32, Handle) -> u32,
        &mut h,
        access,
        null(),
        m,
        protection,
        attrs,
        file.map_or(Handle::EMPTY, |v| *v.as_ref())
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}
pub fn NtMapViewOfSection(sec: impl AsRef<Handle>, proc: impl AsRef<Handle>, offset: usize, size: usize, inherit: u32, alloc_type: u32, access: u32) -> Win32Result<(usize, usize)> {
    let (mut h, mut s) = (0usize, size);
    let mut o = offset;
    let r = syscall!(
        ntdll().NtMapViewOfSection,
        (Handle, Handle, *mut usize, usize, usize, *mut usize, *mut usize, u32, u32, u32) -> u32,
        *sec.as_ref(),
        *proc.as_ref(),
        &mut h, // Base Address
        0,
        0,
        &mut o,
        &mut s, // Size of Map
        inherit,
        alloc_type,
        access
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok((h, s))
    }
}

/////////////////////////////////////
// File Functions
/////////////////////////////////////
#[inline]
pub fn NtFlushBuffersFile(h: impl AsRef<Handle>) -> Win32Result<()> {
    let mut i = IoStatusBlock::default();
    let r = syscall!(ntdll().NtFlushBuffersFile, (Handle, *mut IoStatusBlock) -> u32, *h.as_ref(), &mut i);
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn NtQueryInformationFile<T>(h: impl AsRef<Handle>, class: u32, buf: *mut T, size: u32) -> Win32Result<u32> {
    let mut i = IoStatusBlock::default();
    let r = syscall!(
        ntdll().NtQueryInformationFile,
        (Handle, *mut IoStatusBlock, *mut T, u32, u32) -> u32,
        *h.as_ref(),
        &mut i,
        buf,
        size,
        class
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(i.info as u32)
    }
}
pub fn NtSetInformationFile<T>(h: impl AsRef<Handle>, class: u32, buf: *const T, size: u32) -> Win32Result<u32> {
    let mut i = IoStatusBlock::default();
    let r = syscall!(
        ntdll().NtSetInformationFile,
        (Handle, *mut IoStatusBlock, *const T, u32, u32) -> u32,
        *h.as_ref(),
        &mut i,
        buf,
        size,
        class
    );
    match r {
        // 0x0000000D - FileDispositionInformation
        // 0x00000040 - FileDispositionInformationEx
        // 0xC0000101 - STATUS_DIRECTORY_NOT_EMPTY
        0xC0000101 if class == 0xD || class == 0x40 => Err(Win32Error::DirectoryNotEmpty),
        0 => Ok(i.info as u32),
        _ => Err(Win32Error::from_status(r)),
    }
}
pub fn UnlockFile(h: impl AsRef<Handle>, olp: MaybeOverlapped, count: u64, offset: Option<u64>) -> Win32Result<()> {
    let r = {
        let mut i = Overlapped::default();
        let o = OverlappedPtr::new(olp);
        let p = o.offset(offset).unwrap_or(0);
        syscall!(
            ntdll().NtUnlockFile,
            (usize, *mut Overlapped, *const u64, *const u64, u32) -> u32,
            win32_handle_to_nt(*h.as_ref()),
            o.io_olp(&mut i),
            &p,
            &count,
            0
        )
    };
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(())
    }
}
pub fn NtWriteFile(h: impl AsRef<Handle>, olp: MaybeOverlapped, buf: &[u8], offset: Option<u64>) -> Win32Result<usize> {
    let (r, n) = {
        let mut i = IoStatusBlock::default();
        let o = OverlappedPtr::new(olp);
        let p = o.offset(offset);
        let r = syscall!(
            ntdll().NtWriteFile,
            (usize, Handle, usize, *mut Overlapped, *mut IoStatusBlock, *const u8, u32, *const u64, usize) -> u32,
            win32_handle_to_nt(*h.as_ref()),
            o.event(),
            0,
            o.apc(),
            o.io(&mut i),
            buf.as_ptr(),
            len_to_u32(buf.len()),
            p.map_or_else(null, |v| &v),
            0
        );
        (r, o.result(i))
    };
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}
pub fn NtReadFile(h: impl AsRef<Handle>, olp: MaybeOverlapped, buf: &mut [u8], offset: Option<u64>) -> Win32Result<usize> {
    let (r, n) = {
        let mut i = IoStatusBlock::default();
        let o = OverlappedPtr::new(olp);
        let p = o.offset(offset);
        let r = syscall!(
            ntdll().NtReadFile,
            (usize, Handle, usize, *mut Overlapped, *mut IoStatusBlock, *const u8, u32, *const u64, usize) -> u32,
            win32_handle_to_nt(*h.as_ref()),
            o.event(),
            0,
            o.apc(),
            o.io(&mut i),
            buf.as_mut_ptr(),
            len_to_u32(buf.len()),
            p.map_or_else(null, |v| &v),
            0
        );
        (r, o.result(i))
    };
    // 0xC0000011 - STATUS_END_OF_FILE
    // 0xC000014B - STATUS_PIPE_BROKEN
    // 0x00000103 - STATUS_PENDING
    match r {
        0xC0000011 => Ok(n),
        0 => Ok(n),
        _ => Err(Win32Error::from_status(r)),
    }
}
pub fn LockFile(h: impl AsRef<Handle>, olp: MaybeOverlapped, flags: u32, count: u64, offset: Option<u64>) -> Win32Result<()> {
    let r = {
        let mut i = IoStatusBlock::default();
        let o = OverlappedPtr::new(olp);
        let p = o.offset(offset).unwrap_or(0);
        let (e, f) = (
            if flags & 0x2 != 0 { 1u8 } else { 0u8 }, // LOCKFILE_EXCLUSIVE_LOCK
            if flags & 0x1 != 0 { 1u8 } else { 0u8 }, // LOCKFILE_FAIL_IMMEDIATELY
        );
        syscall!(
            ntdll().NtLockFile,
            (usize, Handle, usize, *mut Overlapped, *mut IoStatusBlock, *const u64, *const u64, u32, u8, u8) -> u32,
            win32_handle_to_nt(*h.as_ref()),
            o.event(),
            0,
            o.apc(),
            o.io(&mut i),
            &p,
            &count,
            0,
            f,
            e
        )
    };
    // 0xC0000055 - STATUS_LOCK_NOT_GRANTED
    // 0x00000103 - STATUS_PENDING
    match r {
        0xC0000055 if flags & 0x1 != 0 => Err(Win32Error::IoPending),
        0 => Ok(()),
        _ => Err(Win32Error::from_status(r)),
    }
}
#[inline]
pub fn NtReOpenFile(file: impl AsRef<Handle>, access: u32, sa: SecAttrs, attrs: u32, share: u32, disposition: u32, flags: u32) -> Win32Result<OwnedHandle> {
    NtCreateFile(
        WCharLike::Null,
        file,
        access,
        sa,
        attrs,
        share,
        disposition,
        flags,
    )
}
pub fn NtQueryDirectoryFile<'a>(h: impl AsRef<Handle>, buf: &mut [u8], class: u32, single: bool, restart: bool, glob: impl Into<WCharLike<'a>>) -> Win32Result<usize> {
    let mut i = IoStatusBlock::default();
    let g = glob.into();
    let r = syscall!(
        ntdll().NtQueryDirectoryFile,
        (Handle, usize, usize, usize, *mut IoStatusBlock, *mut u8, u32, u32, u32, *const u16, u32) -> u32,
        *h.as_ref(), // TODO(dij): Can have overlapped, but do we need it?
        0,
        0,
        0,
        &mut i,
        buf.as_mut_ptr(),
        len_to_u32(buf.len()),
        class,
        if single { 1 } else { 0 },
        g.as_ptr(),
        if restart { 1 } else { 0 }
    );
    match r {
        0x80000006 => Ok(0), // 0x80000006 - ERROR_NO_MORE_FILES
        0 => Ok(i.info),
        _ => Err(Win32Error::from_status(r)),
    }
}
pub fn NtCreateFile<'a>(file: impl Into<WCharLike<'a>>, root: impl AsRef<Handle>, access: u32, sa: SecAttrs, attrs: u32, share: u32, disposition: u32, flags: u32) -> Win32Result<OwnedHandle> {
    let mut h = Handle::default();
    let mut i = IoStatusBlock::default();
    let r = {
        // When the file path is empty, we can reopen the same file! WCharLike::Null can
        // be used for this (or None!)
        let p = *root.as_ref();
        // Prevent changing path if a parent Handle is used.
        let e = if p.is_invalid() {
            path_normalize(file)
        } else {
            file.into()
        };
        // 0x1000000 - FILE_FLAG_POSIX_SEMANTICS
        // 0x0000020 - OBJ_EXCLUSIVE
        // 0x0000040 - OBJ_CASE_INSENSITIVE
        object_attrs!(
            e,
            p,
            false,
            if flags & 0x1000000 != 0 { 0 } else { 0x40 },
            // NOTE(dij): This 'OBJ_EXCLUSIVE' flag seems to cause problems?
            //            looking at the calls Windows makes seems not to use it
            //            either? Weird. When used throws "invalid argument".
            // | if share == 0 { 0x20 } else { 0 },
            sa,
            None,
            o
        );
        syscall!(
            ntdll().NtCreateFile,
            (*mut Handle, u32, *const ObjectAttributes, *mut IoStatusBlock, *mut u32, u32, u32, u32, u32, *mut u8, u32) -> u32,
            &mut h,
            access,
            &o,
            &mut i,
            null_mut(),
            attrs,
            share,
            disposition,
            flags & !0x1000000, // Remove FILE_FLAG_POSIX_SEMANTICS
            null_mut(),
            0
        )
    };
    match r {
        _ if i.info == 0x5 => Err(Win32Error::NotFound),
        0 => Ok(h.into()),
        _ => Err(Win32Error::from_status(r)),
    }
}

/////////////////////////////////////
// Device/IO Control Functions
/////////////////////////////////////
pub fn NtFsControlFile<T, O>(h: impl AsRef<Handle>, code: u32, olp: MaybeOverlapped, input: *const T, in_len: u32, out: *mut O, out_len: u32) -> Win32Result<usize> {
    let (r, n) = {
        let mut i = IoStatusBlock::default();
        let o = OverlappedPtr::new(olp);
        let r = syscall!(
            ntdll().NtFsControlFile,
            (Handle, Handle, usize, *mut Overlapped, *mut IoStatusBlock, u32, *const T, u32, *mut O, u32) -> u32,
            *h.as_ref(),
            o.event(),
            0,
            o.apc(),
            o.io(&mut i),
            code,
            input,
            in_len,
            out,
            out_len
        );
        (r, o.result(i))
    };
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}
pub fn NtDeviceIoControlFile<T, O>(h: impl AsRef<Handle>, code: u32, olp: MaybeOverlapped, input: *const T, in_len: u32, out: *mut O, out_len: u32) -> Win32Result<usize> {
    let (r, n) = {
        let mut i = IoStatusBlock::default();
        let o = OverlappedPtr::new(olp);
        let r = syscall!(
            ntdll().NtDeviceIoControlFile,
            (Handle, Handle, usize, *mut Overlapped, *mut IoStatusBlock, u32, *const T, u32, *mut O, u32) -> u32,
            *h.as_ref(),
            o.event(),
            0,
            o.apc(),
            o.io(&mut i),
            code,
            input,
            in_len,
            out,
            out_len
        );
        (r, o.result(i))
    };
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(n)
    }
}

pub unsafe fn LdlLoadLibrary(name: &UnicodeString) -> Win32Result<NonZeroHandle> {
    let mut h = Handle::default();
    let r = {
        let (d, e) = (system_dir(), 0u32);
        syscall!(
            ntdll().LdrLoadDll,
            (*const u16, *const u32, *const UnicodeString, *mut Handle) -> u32,
            d.as_ptr(),
            &e,
            name,
            &mut h
        )
    };
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(h.into())
    }
}
pub unsafe fn LdlLoadAddress(h: Handle, ordinal: u16, name: &AnsiString) -> Win32Result<usize> {
    let mut f = 0usize;
    let r = syscall!(
        ntdll().LdrGetProcedureAddress,
        (Handle, *const AnsiString, u16, *mut usize) -> u32,
        h,
        name,
        ordinal,
        &mut f
    );
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok(f)
    }
}

fn create_pipe_xp(sa: SecAttrs, size: u32, olp: bool) -> Win32Result<(OwnedHandle, OwnedHandle)> {
    // Creating a named pipe does not work the same in Xp.
    // We must use a pseudo name for this.
    let (p, n) = create_pipe_xp_first(sa, size, olp)?;
    let mut h = Handle::default();
    let mut i = IoStatusBlock::default();
    let r = {
        object_attrs!(
            n,
            Handle::EMPTY,
            false,
            0x40, // 0x40 - OBJ_CASE_INSENSITIVE
            sa,
            None,
            o
        );
        syscall!(
            ntdll().NtCreateFile,
            (*mut Handle, u32, *const ObjectAttributes, *mut IoStatusBlock, *mut u32, u32, u32, u32, u32, *mut u8, u32) -> u32,
            &mut h,
            0x40100080, // 0x120116 - FILE_GENERIC_WRITE
            &o,
            &mut i,
            null_mut(),
            0,
            0x1,  // 0x01 - FILE_SHARE_READ
            0x1,  // 0x01 - FILE_OPEN
            0x40 | if olp { 0 } else { 0x20 }, // 0x20 - FILE_SYNCHRONOUS_IO_NONALERT | 0x60 - FILE_NON_DIRECTORY_FILE
            null_mut(),
            0
        )
    };
    if r > 0 {
        Err(Win32Error::from_status(r))
    } else {
        Ok((p, h.into()))
    }
}
fn create_pipe_xp_first(sa: SecAttrs, size: u32, olp: bool) -> Win32Result<(OwnedHandle, WChar)> {
    let d: i64 = -500000i64;
    let f = syscall!(
        ntdll().NtCreateNamedPipeFile,
        fn(*mut Handle, u32, *const ObjectAttributes, *mut IoStatusBlock, u32, u32, u32, u32, u32, u32, u32, u32, u32, *const i64) -> u32
    );
    // "Unnamed" Pipe name format for Windowns Xp
    //
    // \Device\NamedPipe\Win32Pipes.%08x.%08x
    //                    ProcessID ^    ^ PipeID
    //
    // We need to use an UnsafeCell to modify-then-use this WChar
    // as the compiler does not understand how we're using it.
    let b = UnsafeCell::new(WChar::null());
    let x = unsafe { (&mut *b.get()).as_mut_vec() };
    x.reserve(64); // Should cover most of it.
    unsafe { str_to_u16_unchecked(x, crypt!(0, r"\Device\NamedPipe\")) };
    // Done in two lines to de-duplicate the "\Device\NamedPipe\" string when
    // using crypt.
    unsafe { str_to_u16_unchecked(x, crypt!(0, r"Win32Pipes.")) };
    // Buffer should 29 long now.
    x.resize(46, 0);
    // Reserve 17 additional slots.
    //   8 hex | '.' | 8 hex
    let _ = write_hex_padded(
        unsafe { x.get_unchecked_mut(29..) },
        8,
        GetCurrentProcessID(),
    ); // Write Process ID
    unsafe { *x.get_unchecked_mut(37) = 0x2E }; // Add '.'
                                                // We should have: \Device\NamedPipe\Win32Pipes.%08x.
    let mut h = Handle::default();
    let mut i = IoStatusBlock::default();
    let mut r = [0u8, 0u8]; // Random buffer
    let a = if olp { 0 } else { 0x20 }; // 0x20 - FILE_SYNCHRONOUS_IO_NONALERT
    let u = UnicodeString::new_wchar(unsafe { &*b.get() });
    // 0x40 - OBJ_CASE_INSENSITIVE
    let o = ObjectAttributes::new(Some(&u), Handle::EMPTY, false, 0x40, sa, None);
    // We don't use the macros so we don't have to worry about re-creating these
    // on each loop.
    loop {
        // If we looped again, we just re-overrite the previous name data.
        let _ = RtlGenRandom(&mut r);
        // Write Random Number as Hex
        let _ = write_hex_padded(
            unsafe { x.get_unchecked_mut(38..) },
            8,
            5 + (u16::from_ne_bytes(r) as u32),
        );
        let r = unsafe {
            // 0x80100100 - GENERIC_READ | FILE_WRITE_ATTRIBUTES | SYNCHRONIZE
            // 0x00000003 - FILE_SHARE_WRITE | FILE_SHARE_READ
            // 0x00000002 - FILE_CREATE
            f(
                &mut h, 0x80100100, &o, &mut i, 0x3, 0x2, a, 0, 0, 0, 0x1, size, size, &d,
            )
        };
        // 0xC0000035 - STATUS_OBJECT_NAME_COLLISION
        match r {
            _ if i.info == 0xC0000035 => (), // Already exists, try again.
            0xC0000035 => (),                // Already exists, try again.
            0 => break,
            _ => return Err(Win32Error::from_status(r)),
        }
    }
    Ok((h.into(), b.into_inner()))
}
