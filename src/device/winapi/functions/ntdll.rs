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

use core::mem::size_of;
use core::slice::from_raw_parts;
use core::{cmp, ptr};

use crate::data::blob::Blob;
use crate::data::rand::Rand;
use crate::data::str::MaybeString;
use crate::data::time::Time;
use crate::device::winapi::functions::{FileLinkInformation, FilePipeWait, IoStatusBlock, RegKeyValuePartialInfo};
use crate::device::winapi::loader::{kernel32, ntdll};
use crate::device::winapi::registry::Key;
use crate::device::winapi::{
    self,
    time_to_windows_time,
    AnsiString,
    AsHandle,
    Chars,
    ClientID,
    DecodeUtf16,
    FileBasicInformation,
    FileStandardInformation,
    Handle,
    ObjectAttributes,
    ObjectBasicInformation,
    Overlapped,
    OverlappedIo,
    OwnedHandle,
    ProcessBasicInfo,
    ReadInto,
    RegKeyBasicInfo,
    RegKeyFullInfo,
    RegKeyValueFullInfo,
    Region,
    SIDAndAttributes,
    SecAttrs,
    SecurityAttributes,
    SecurityQualityOfService,
    ThreadBasicInfo,
    TimerFunc,
    TokenPrivileges,
    TokenUser,
    UnicodeStr,
    UnicodeString,
    WChars,
    Win32Error,
    Win32Result,
    WriteFrom,
    PTR_SIZE,
};
use crate::ignore_error;
use crate::prelude::*;
use crate::util::{self, crypt, HEXTABLE};

macro_rules! object_attrs {
    ($name:expr, $root:expr, $inherit:expr, $attrs:expr, $sa:expr, $qos:expr, $n:ident, $o:ident) => {
        let $n = UnicodeStr::from($name);
        let $o = if $n.is_empty() {
            ObjectAttributes::root(None, $root, $inherit, $attrs, $sa, $qos)
        } else {
            ObjectAttributes::root(Some(&$n.value), $root, $inherit, $attrs, $sa, $qos)
        };
    };
}

/////////////////////////////////////
// Helper Functions
/////////////////////////////////////
#[inline]
pub fn hide_thread(h: impl AsHandle) -> Win32Result<()> {
    // 0x11 - ThreadHideFromDebugger
    NtSetInformationThread(h, 0x11, ptr::null::<usize>(), 0)
}
#[inline]
pub fn delete_file_by_handle(h: impl AsHandle) -> Win32Result<()> {
    // 0xD - FileDispositionInformation
    let d = 1u32; // Prevent optimization of NUL ptr.
    NtSetInformationFile(h, 0xD, &d, 4)?;
    Ok(())
}
pub fn file_name_by_handle(h: impl AsHandle) -> Win32Result<String> {
    winapi::init_ntdll();
    let mut b: Blob<u16, 300> = Blob::new();
    let func = unsafe {
        winapi::make_syscall!(
            *ntdll::NtQueryObject,
            extern "stdcall" fn(Handle, u32, *mut u16, u32, *mut u32) -> u32
        )
    };
    let mut n = 520u32; // 520 = 260/u16 as u8 size.
    let v = h.as_handle();
    loop {
        b.resize_as_bytes(n as usize);
        // 0x1 - ObjectNameInformation
        let r = func(v, 0x1, b.as_mut_ptr(), b.len_as_bytes() as u32, &mut n);
        match r {
            0 => break,
            _ if b.len() < n as usize => continue,
            _ => return Err(winapi::nt_error(r)),
        }
    }
    Ok((&b[winapi::PTR_SIZE..(cmp::min(n, b[0] as u32) as usize / 2) + winapi::PTR_SIZE]).decode_utf16())
}
#[inline]
pub fn set_file_attrs_by_handle(h: impl AsHandle, attrs: u32) -> Win32Result<()> {
    let i = FileBasicInformation {
        attributes:       attrs,
        change_time:      0i64,
        creation_time:    0i64,
        last_write_time:  0i64,
        last_access_time: 0i64,
    };
    // 0x4 - FileBasicInfo
    NtSetInformationFile(h, 0x4, &i, 0x28)?;
    Ok(())
}
pub fn token_user<'a>(h: impl AsHandle, buf: &'a mut Blob<u8, 256>) -> Win32Result<&'a TokenUser> {
    winapi::init_ntdll();
    let (mut n, mut c) = (64u32, 64u32);
    let func = unsafe {
        winapi::make_syscall!(
            *ntdll::NtQueryInformationToken,
            extern "stdcall" fn(Handle, u32, *mut u8, u32, *mut u32) -> u32
        )
    };
    let v = h.as_handle();
    loop {
        buf.resize(n as usize);
        // 0x1 - TokenUser
        let r = func(v, 0x1, buf.as_mut_ptr(), n, &mut c);
        match r {
            0x7A => return Err(winapi::nt_error(r)), // 0x7A - ERROR_INSUFFICIENT_BUFFER
            0 => return Ok(unsafe { &*(buf.as_ptr() as *const TokenUser) }),
            _ if c < n => return Err(winapi::nt_error(r)),
            _ => n = c,
        }
    }
}
pub fn token_groups<'a, const N: usize>(t: impl AsHandle, buf: &'a mut Blob<u8, N>) -> Win32Result<Vec<&'a SIDAndAttributes>> {
    winapi::init_ntdll();
    let func = unsafe {
        winapi::make_syscall!(
            *ntdll::NtQueryInformationToken,
            extern "stdcall" fn(Handle, u32, *mut u8, u32, *mut u32) -> u32
        )
    };
    let h = t.as_handle();
    let mut n = 0u32;
    buf.resize(256);
    loop {
        let r = func(h, 0x2, buf.as_mut_ptr(), buf.len() as u32, &mut n);
        match r {
            // 0xC0000023 - STATUS_BUFFER_TOO_SMALL
            0xC0000023 => {
                buf.resize(n as usize);
                continue;
            },
            0 => break,
            _ => return Err(winapi::nt_error(r)),
        }
    }
    let c = unsafe { *(buf.as_ptr_of::<u32>()) } as usize;
    let mut r = Vec::with_capacity(c);
    if c == 0 {
        return Ok(r);
    }
    let d = unsafe {
        from_raw_parts(
            buf.as_ptr().add(winapi::PTR_SIZE) as *const SIDAndAttributes,
            c,
        )
    };
    for i in d {
        r.push(i)
    }
    Ok(r)
}
#[inline]
pub fn set_file_time_by_handle(h: impl AsHandle, created: Option<Time>, modified: Option<Time>, access: Option<Time>) -> Win32Result<()> {
    let i = FileBasicInformation {
        attributes:       0u32,
        change_time:      modified.map_or(0, time_to_windows_time),
        creation_time:    created.map_or(0, time_to_windows_time),
        last_write_time:  0i64,
        last_access_time: access.map_or(0, time_to_windows_time),
    };
    // 0x4 - FileBasicInfo
    NtSetInformationFile(h, 0x4, &i, 0x28)?;
    Ok(())
}

/////////////////////////////////////
// AsHandle and Handle Functions
/////////////////////////////////////
pub fn CloseHandle(h: Handle) -> Win32Result<()> {
    if h.is_invalid() {
        bugtrack!("winapi::CloseHandle(): Attempted to close an invalid Handle!");
        return Ok(());
    }
    winapi::init_ntdll();
    let r = unsafe { winapi::syscall!(*ntdll::NtClose, extern "stdcall" fn(Handle) -> u32, h) };
    bugtrack!(
        "winapi::CloseHandle(): Closing Handle '{:x}' (result {r:X}).",
        h.0
    );
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn GetObjectInformation(h: impl AsHandle) -> Win32Result<ObjectBasicInformation> {
    winapi::init_ntdll();
    let mut i = ObjectBasicInformation::default();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtQueryObject,
            extern "stdcall" fn(Handle, u32, *mut ObjectBasicInformation, u32, *mut u32) -> u32,
            h.as_handle(),
            0, // 0x0 - ObjectBasicInformation
            &mut i,
            0x38, // ObjectBasicInformation has a fixed size of 56.
            ptr::null_mut()
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(i)
    }
}
pub fn SetHandleInformation(h: impl AsHandle, inherit: bool, protect: bool) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        let b = [if inherit { 1 } else { 0 }, if protect { 1 } else { 0 }];
        winapi::syscall!(
            *ntdll::NtSetInformationObject,
            extern "stdcall" fn(Handle, u32, *const u8, u32) -> u32,
            h.as_handle(),
            0x4, // 0x4 - ObjectFlagInformation
            b.as_ptr(),
            0x2
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn DuplicateHandle(src: impl AsHandle, access: u32, opts: u32) -> Win32Result<OwnedHandle> {
    DuplicateHandleEx(
        src,
        winapi::CURRENT_PROCESS,
        winapi::CURRENT_PROCESS,
        access,
        false,
        opts,
    )
    .map(|h| h.into())
}
pub fn DuplicateHandleEx(src: impl AsHandle, src_proc: impl AsHandle, dst_proc: impl AsHandle, access: u32, inherit: bool, opts: u32) -> Win32Result<Handle> {
    let (v, s, d) = (src.as_handle(), src_proc.as_handle(), dst_proc.as_handle());
    // NOTE(dij) - Check to see if the Handle is a STDIN Console Handle THEN check
    //             to see if we're older than Win8 as we need to use
    //             'DuplicateConsoleHandle' as the NtDuplicateObject call can't
    //             handle those types of Handles.
    //
    //             This is what kernel32.dll does!
    if s == winapi::CURRENT_PROCESS && d == winapi::CURRENT_PROCESS && (v.0 & 0x10000003) == 0x3 && !winapi::is_min_windows_8() {
        winapi::init_kernel32();
        let h = unsafe {
            winapi::syscall!(
                *kernel32::DuplicateConsoleHandle,
                extern "stdcall" fn(Handle, u32, u32, u32) -> Handle,
                v,
                access,
                if inherit { 1 } else { 0 },
                opts
            )
        };
        return if h.is_invalid() {
            Err(winapi::last_error())
        } else {
            Ok(h)
        };
    }
    winapi::init_ntdll();
    let mut h = Handle::default();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtDuplicateObject,
            extern "stdcall" fn(Handle, usize, Handle, *mut Handle, u32, u32, u32) -> u32,
            s,
            winapi::check_nt_handle(v),
            d,
            &mut h,
            access,
            if inherit { 0x2 } else { 0 }, // 0x2 - OBJ_INHERIT
            opts
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h)
    }
}

/////////////////////////////////////
// Library/DLL Loader Functions
/////////////////////////////////////
pub fn FreeLibrary(dll: Handle) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::LdrUnloadDll,
            extern "stdcall" fn(Handle) -> u32,
            dll
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn LoadLibraryW(path: &[u16]) -> Win32Result<Handle> {
    // Raw UTF16 version to prevent allocating back into a string.
    winapi::init_ntdll();
    unsafe {
        let n = UnicodeString::new(&path);
        ldl_load_library(&n)
    }
}
#[inline]
pub fn LoadLibrary(path: impl AsRef<str>) -> Win32Result<Handle> {
    winapi::init_ntdll();
    unsafe {
        let b: WChars = path.as_ref().into();
        let n = UnicodeString::new(&b);
        ldl_load_library(&n)
    }
}
pub fn GetModuleHandleExW(flags: u32, name: &[u16]) -> Win32Result<Handle> {
    // Raw UTF16 version to prevent allocating back into a string.
    if flags & 0x4 != 0 {
        // GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
        // NOTE(dij): This is too annoying to support.
        return Err(Win32Error::InvalidOperation);
    }
    if name.len() == 0 {
        return Ok(winapi::GetCurrentProcessPEB().image_base_address);
    }
    let mut f = 0u32;
    // Translate kernel32 flags to NT flags.
    if flags & 0x1 != 0 {
        // GET_MODULE_HANDLE_EX_FLAG_PIN
        f |= 0x2 // LDR_GET_DLL_HANDLE_EX_PIN
    }
    if flags & 0x2 != 0 {
        // GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT
        f |= 0x1 // LDR_GET_DLL_HANDLE_EX_UNCHANGED_REFCOUNT
    }
    let mut n: WChars = name.iter().collect();
    let i = n.len();
    // Add '.dll' extension if it isn't present.
    if n.len() > 5 && n[i - 5] != b'.' as u16 && n[i - 4] != b'd' as u16 {
        n[i - 1] = b'.' as u16;
        n.reserve(4);
        n.push(b'd' as u16);
        n.push(b'l' as u16);
        n.push(b'l' as u16);
        n.push(0);
    }
    winapi::init_ntdll();
    let u = UnicodeString::new(&n);
    let func = unsafe {
        winapi::make_syscall!(
            *ntdll::LdrGetDllHandleEx,
            extern "stdcall" fn(u32, *const u16, *const u32, *const UnicodeString, *mut Handle) -> u32
        )
    };
    let mut h = Handle::default();
    let r = if name.iter().position(|v| *v == b'\\' as u16 || *v == b'/' as u16).is_some() {
        func(f, ptr::null(), ptr::null(), &u, &mut h)
    } else {
        let d = winapi::system_dir();
        func(f, d.as_ptr(), ptr::null(), &u, &mut h)
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h)
    }
}
pub fn GetModuleHandleEx(flags: u32, name: impl MaybeString) -> Win32Result<Handle> {
    if flags & 0x4 != 0 {
        // GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
        // NOTE(dij): This is too annoying to support.
        return Err(Win32Error::InvalidOperation);
    }
    if let Some(s) = name.into_string() {
        // Cache this in a temporary array
        let t = s.encode_utf16().collect::<Blob<u16, 128>>();
        {
            GetModuleHandleExW(flags, &t)
        }
    } else {
        Ok(winapi::GetCurrentProcessPEB().image_base_address)
    }
}
#[inline]
pub fn GetProcAddress(h: Handle, ordinal: u16, name: impl AsRef<str>) -> Win32Result<usize> {
    winapi::init_ntdll();
    unsafe {
        let b: Chars = name.as_ref().into();
        let a = AnsiString::new(&b);
        ldl_load_address(h, ordinal, &a)
    }
}

/////////////////////////////////////
// Pipe/NamedPipe Functions
/////////////////////////////////////
pub fn DisconnectNamedPipe(h: impl AsHandle) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        let mut t = IoStatusBlock::default();
        let r = winapi::syscall!(
            *ntdll::NtFsControlFile,
            extern "stdcall" fn(Handle, usize, usize, *mut Overlapped, *mut IoStatusBlock, u32, *const u8, u32, *mut u8, u32) -> u32,
            h.as_handle(),
            0,
            0,
            ptr::null_mut(),
            &mut t,
            0x110004, // 0x110004 - FSCTL_PIPE_DISCONNECT
            ptr::null(),
            0,
            ptr::null_mut(),
            0
        );
        // 0x103 - STATUS_PENDING
        if r == 0x103 {
            ignore_error!(WaitForSingleObject(h, -1, false));
            t.status as u32
        } else {
            r
        }
    };
    match r {
        0x103 => Err(Win32Error::IoPending), // 0x103 - STATUS_PENDING
        0 => Ok(()),
        _ => Err(winapi::nt_error(r)),
    }
}
pub fn ImpersonateNamedPipeClient(h: impl AsHandle) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        let mut i = IoStatusBlock::default();
        winapi::syscall!(
            *ntdll::NtFsControlFile,
            extern "stdcall" fn(Handle, usize, usize, usize, *mut IoStatusBlock, u32, *const u8, u32, *mut u8, u32) -> u32,
            h.as_handle(),
            0,
            0,
            0,
            &mut i,
            0x11001C, // 0x11001C - FSCTL_PIPE_IMPERSONATE
            ptr::null(),
            0,
            ptr::null_mut(),
            0
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn WaitNamedPipe(name: impl AsRef<str>, timeout: i32) -> Win32Result<()> {
    let b = name.as_ref().as_bytes();
    if b.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let n = winapi::normalize_path_to_nt(if b[0] != b'\\' {
        // Local non fully quantified pipe name (ie: "MyPipe").
        let mut t = crypt::get_or(0, r"\\.\pipe\").to_string();
        unsafe { t.as_mut_vec().extend_from_slice(b) };
        t
    } else if b.len() > 3 && b[0] == b'\\' && b[1] == b'\\' && b[2] != b'.' {
        // Remote non-fully quantified pipe name (NT-sense) (ie:
        // "\\server\pipe\MyPipe").
        let mut t = crypt::get_or(0, r"\??\DosDevices\UNC\").to_string();
        unsafe { t.as_mut_vec().extend_from_slice(&b[2..]) };
        t
    } else {
        name.as_ref().to_string()
    });
    winapi::init_ntdll();
    let r = unsafe {
        let k = n.encode_utf16().collect::<Blob<u16, 256>>();
        // 0xD Is the size of 'FilePipeWait'.
        let mut p: Blob<u16, 256> = Blob::with_capacity(0xD + k.len_as_bytes());
        p.write_item(FilePipeWait::new(
            timeout,
            cmp::min(k.len(), 0xFFFFFFFF) as u32,
        ));
        p.extend_from_slice(&k);
        // 0x000003 - FILE_SHARE_READ | FILE_SHARE_WRITE
        // 0x000020 - FILE_SYNCHRONOUS_IO_NONALERT
        // 0x100080 - FILE_READ_ATTRIBUTES | SYNCHRONIZE
        let h = NtCreateFile(n, Handle::INVALID, 0x100080, None, 0, 0x3, 0, 0x20)?;
        let mut t = IoStatusBlock::default();
        winapi::syscall!(
            *ntdll::NtFsControlFile,
            extern "stdcall" fn(Handle, usize, usize, *mut Overlapped, *mut IoStatusBlock, u32, *const u16, u32, *mut u8, u32) -> u32,
            *h,
            0,
            0,
            ptr::null_mut(),
            &mut t,
            0x110018, // 0x110018 - FSCTL_PIPE_WAIT
            p.as_ptr(),
            cmp::min(p.len_as_bytes(), 0xFFFFFFFF) as u32,
            ptr::null_mut(),
            0
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn ConnectNamedPipe(h: impl AsHandle, olp: OverlappedIo) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        let mut t = Overlapped::default();
        let (w, x) = olp.map_or_else(|| (true, &mut t), |v| (false, v));
        let r = winapi::syscall!(
            *ntdll::NtFsControlFile,
            extern "stdcall" fn(Handle, usize, usize, *mut Overlapped, *mut Overlapped, u32, *const u8, u32, *mut u8, u32) -> u32,
            h.as_handle(),
            x.event.0,
            0,
            if x.event.0 & 1 == 0 { x } else { ptr::null_mut() },
            x,
            0x110008, // 0x110008 - FSCTL_PIPE_LISTEN
            ptr::null(),
            0,
            ptr::null_mut(),
            0
        );
        // 0x103 - STATUS_PENDING
        if r == 0x103 && w {
            ignore_error!(WaitForSingleObject(h, -1, false));
            x.internal as u32
        } else {
            r
        }
    };
    match r {
        0x103 => Err(Win32Error::IoPending), // 0x103 - STATUS_PENDING
        0 => Ok(()),
        _ => Err(winapi::nt_error(r)),
    }
}
pub fn CreatePipe(sa: SecAttrs, size: u32) -> Win32Result<(OwnedHandle, OwnedHandle)> {
    winapi::init_ntdll();
    let func = unsafe {
        winapi::make_syscall!(
            *ntdll::NtCreateNamedPipeFile,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes, *mut IoStatusBlock, u32, u32, u32, u32, u32, u32, i32, u32, u32, *const i64) -> u32
        )
    };
    let d = -500000i64;
    let p = winapi::GetCurrentProcessID();
    let b = crypt::get_or(0, r"\Device\NamedPipe\Win32Pipes.");
    // \Device\NamedPipe\Win32Pipes.%08x.%08x
    //                              ^ ProcessID
    //                                   ^ PipeID
    let mut r = Rand::new();
    let mut buf = String::with_capacity(64);
    let mut i = IoStatusBlock::default();
    let pipe_one = loop {
        let mut h = Handle::default();
        // TODO(dij): Revisit this, as the pipe name is somehow named? and NOT
        //            unnamed? I'm not sure how this works as I'm doing exactly
        //            what kernelbase.dll and ReactOS are doing.
        buf.push_str(b);
        unsafe {
            write_hex_padded(&mut buf, 8, p);
            buf.as_mut_vec().push(b'.');
            write_hex_padded(&mut buf, 8, cmp::max(5000, r.rand_u32()));
        }
        object_attrs!(
            buf.as_str(),
            Handle::INVALID,
            false,
            0x40, // 0x40 - OBJ_CASE_INSENSITIVE
            sa,
            None,
            _n,
            obj
        );
        let r = func(
            &mut h,
            0x80100100, // 0x80100100 - GENERIC_READ | FILE_WRITE_ATTRIBUTES | SYNCHRONIZE
            &obj,
            &mut i,
            0x3, // 0x3 - FILE_SHARE_WRITE | FILE_SHARE_READ
            0x2, // 0x2 - FILE_CREATE
            0,
            // 0x2 - PIPE_REJECT_REMOTE_CLIENTS (>= Vista only).
            if winapi::is_min_windows_vista() { 0x2 } else { 0 },
            0,
            0,
            0x1,
            size,
            size,
            &d,
        );
        if r == 0 {
            break h;
        }
        // 0xC0000035 - STATUS_OBJECT_NAME_COLLISION
        if r == 0xC0000035 || i.info == 0xC0000035 {
            buf.clear();
            continue;
        }
        return Err(winapi::nt_error(r));
    };
    let mut pipe_two = Handle::default();
    let r = unsafe {
        object_attrs!(
            buf.as_str(),
            Handle::INVALID,
            false,
            0x40, // 0x40 - OBJ_CASE_INSENSITIVE
            sa,
            None,
            _n,
            obj
        );
        winapi::syscall!(
            *ntdll::NtCreateFile,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes, *mut IoStatusBlock, *mut u32, u32, u32, u32, u32, *mut u8, u32) -> u32,
            &mut pipe_two,
            0x40100080, // 0x120116 - FILE_GENERIC_WRITE
            &obj,
            &mut i,
            ptr::null_mut(),
            0,
            0x1,  // 0x01 - FILE_SHARE_READ
            0x1,  // 0x01 - FILE_OPEN
            0x40, // 0x60 - FILE_NON_DIRECTORY_FILE
            ptr::null_mut(),
            0
        )
    };
    if r > 0 {
        winapi::close_handle(pipe_one);
        Err(winapi::nt_error(r))
    } else {
        Ok((pipe_one.into(), pipe_two.into()))
    }
}
pub fn CreateNamedPipe(name: impl AsRef<str>, mode: u32, pipe_mode: u32, max: u32, in_buf: u32, out_buf: u32, timeout_micro: u32, sa: SecAttrs) -> Win32Result<OwnedHandle> {
    let n = name.as_ref();
    if n.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    // NOTE(dij): This is up here as the 'is_min_windows_vista' call will load
    //            ntdll.dll anyway.
    winapi::init_ntdll();
    // 0x100000 - SYNCHRONIZE
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
    let w_mode = if pipe_mode & 0x4 != 0 {
        // PIPE_TYPE_MESSAGE
        0x1 // FILE_PIPE_MESSAGE_TYPE
    } else {
        0 // FILE_PIPE_BYTE_STREAM_TYPE
    } | if winapi::is_min_windows_vista() && pipe_mode & 0x8 != 0 {
        0x2
    } else {
        0
    };
    // 0x8 - PIPE_REJECT_REMOTE_CLIENTS
    //       The 0x2 is the NT flag for this and will cause any Xp pipe operations
    //       to fail, so we only add it if we're >= Vista.
    let r_mode = if pipe_mode & 0x2 != 0 {
        // PIPE_READMODE_MESSAGE
        0x1 // FILE_PIPE_MESSAGE_MODE
    } else {
        // FILE_PIPE_BYTE_STREAM_MODE
        0
    };
    let b_mode = if pipe_mode & 0x1 != 0 {
        // PIPE_NOWAIT
        0x1 // FILE_PIPE_COMPLETE_OPERATION
    } else {
        // FILE_PIPE_QUEUE_OPERATION
        0
    };
    let t_mode = if timeout_micro > 0 {
        (timeout_micro as i64).wrapping_mul(-10)
    } else {
        -500000i64
    };
    let p_max = match max {
        0xFF => -1,
        _ if max > 0xFF => -1,
        _ => max as i32,
    };
    let mut h = Handle::default();
    let r = unsafe {
        let mut i = IoStatusBlock::default();
        let n = UnicodeStr::from(if n.as_bytes()[0] != b'\\' {
            let mut t = crypt::get_or(0, r"\\.\pipe\").to_string();
            t.push_str(n);
            winapi::normalize_path_to_nt(t)
        } else {
            winapi::normalize_path_to_nt(n)
        });
        // 0x40 - OBJ_CASE_INSENSITIVE
        let obj = ObjectAttributes::new(Some(&n.value), false, 0x40, sa, None);
        winapi::syscall!(
            *ntdll::NtCreateNamedPipeFile,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes, *mut IoStatusBlock, u32, u32, u32, u32, u32, u32, i32, u32, u32, *const i64) -> u32,
            &mut h,
            a,
            &obj,
            &mut i,
            s,
            // 0x80000 - FILE_FLAG_FIRST_PIPE_INSTANCE
            if mode & 0x80000 != 0 { 0x2 } else { 0x3 }, // 0x2 - FILE_CREATE | 0x3 - FILE_OPEN_IF
            m,
            w_mode,
            r_mode,
            b_mode,
            p_max,
            in_buf,
            out_buf,
            &t_mode
        )
    };
    // 0xC0000010 - STATUS_INVALID_DEVICE_REQUEST
    // 0xC00000BB - STATUS_NOT_SUPPORTED
    // 0xC0000033 - STATUS_OBJECT_NAME_INVALID
    match r {
        0xC0000010 | 0xC00000BB => Err(winapi::nt_error(0xC0000033)),
        0 => Ok(h.into()),
        _ => Err(winapi::nt_error(r)),
    }
}

/////////////////////////////////////
// Registry Functions
/////////////////////////////////////
pub fn NtFlushKey(key: Key) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe { winapi::syscall!(*ntdll::NtFlushKey, extern "stdcall" fn(Key) -> u32, key) };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn NtDeleteKey(key: Key) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe { winapi::syscall!(*ntdll::NtDeleteKey, extern "stdcall" fn(Key) -> u32, key) };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn NtQueryKeyInfo(key: Key) -> Win32Result<RegKeyFullInfo> {
    winapi::init_ntdll();
    let mut i = RegKeyFullInfo::default();
    let r = unsafe {
        let mut d = 0u32;
        winapi::syscall!(
            *ntdll::NtQueryKey,
            extern "stdcall" fn(Key, u32, *mut RegKeyFullInfo, u32, *mut u32) -> u32,
            key,
            0x2, // 0x2 - KeyFullInformation
            &mut i,
            0x238, // RegKeyFullInfo has a fixed size of 0x238.
            &mut d
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(i)
    }
}
pub fn NtDeleteValueKey(key: Key, value: impl MaybeString) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        let v: WChars = value.into_string().into();
        let n = UnicodeString::new(&v);
        winapi::syscall!(
            *ntdll::NtDeleteValueKey,
            extern "stdcall" fn(Key, *const UnicodeString) -> u32,
            key,
            if n.is_empty() { ptr::null() } else { &n }
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn NtEnumerateKey(key: Key, index: u32) -> Win32Result<Option<RegKeyBasicInfo>> {
    winapi::init_ntdll();
    let mut i = RegKeyBasicInfo::default();
    let r = unsafe {
        let mut d = 0u32;
        winapi::syscall!(
            *ntdll::NtEnumerateKey,
            extern "stdcall" fn(Key, u32, u32, *mut RegKeyBasicInfo, u32, *mut u32) -> u32,
            key,
            index,
            0, // 0x0 - KeyBasicInformation
            &mut i,
            0x210, // RegKeyBasicInfo has a fixed size of 0x210.
            &mut d
        )
    };
    match r {
        0x8000001A => Ok(None), // 0x8000001A - STATUS_NO_MORE_ENTRIES
        0 => Ok(Some(i)),
        _ => Err(winapi::nt_error(r)),
    }
}
pub fn NtOpenKey(root: Key, subkey: impl MaybeString, opts: u32, access: u32) -> Win32Result<Key> {
    winapi::init_ntdll();
    let mut k = Key::default();
    let r = unsafe {
        // 0x100 - OBJ_OPENLINK
        // 0x008 - REG_OPTION_OPEN_LINK
        // 0x040 - OBJ_CASE_INSENSITIVE
        object_attrs!(
            subkey.into_string(),
            root.0,
            false,
            0x40 | if opts & 0x8 != 0 { 0x100 } else { 0 },
            None,
            None,
            _n,
            obj
        );
        winapi::syscall!(
            *ntdll::NtOpenKey,
            extern "stdcall" fn(*mut Key, u32, *const ObjectAttributes) -> u32,
            &mut k,
            access,
            &obj
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(k)
    }
}
pub fn NtQueryValueKey(key: Key, value: impl MaybeString, size: u32) -> Win32Result<(Blob<u8, 256>, u8)> {
    winapi::init_ntdll();
    let mut b = Blob::with_capacity(size as usize);
    let t = unsafe {
        let v: WChars = value.into_string().into();
        let n = UnicodeString::new(&v);
        let func = winapi::make_syscall!(
            *ntdll::NtQueryValueKey,
            extern "stdcall" fn(Key, *const UnicodeString, u32, *mut u8, u32, *mut u32) -> u32
        );
        let mut s = size;
        loop {
            b.resize(s as usize);
            let r = func(
                key,
                if n.is_empty() { ptr::null() } else { &n },
                0x2, // 0x2 - KeyValuePartialInformation
                b.as_mut_ptr(),
                s,
                &mut s,
            );
            match r {
                0xC0000023 => continue, // 0xC0000023 - STATUS_BUFFER_TOO_SMALL
                0x80000005 => continue, // 0x80000005 - STATUS_BUFFER_OVERFLOW
                0 => {
                    b.truncate(s as usize);
                    let i = b.as_ptr() as *const RegKeyValuePartialInfo;
                    let (t, x) = ((*i).value_type as u8, (*i).length);
                    b.cut(0xC); // 0xC is size of RegKeyValuePartialInfo.
                    b.truncate(x as usize);
                    break t;
                },
                _ => return Err(winapi::nt_error(r)),
            }
        }
    };
    Ok((b, t))
}
pub fn NtEnumerateValueKey(key: Key, index: u32, data: Option<&mut Blob<u8, 256>>) -> Win32Result<Option<RegKeyValueFullInfo>> {
    winapi::init_ntdll();
    unsafe {
        let mut s = 0x220; // Size of RegKeyValueFullInfo
        let mut t = Blob::with_capacity(0x220);
        // NOTE(dij): Size 0x220 will push this Blob to the Heap.
        let b = data.unwrap_or_else(|| &mut t);
        let func = winapi::make_syscall!(
            *ntdll::NtEnumerateValueKey,
            extern "stdcall" fn(Key, u32, u32, *mut u8, u32, *mut u32) -> u32
        );
        loop {
            b.resize(s as usize);
            // 0x1 - KeyValueFullInformation
            let r = func(key, index, 0x1, b.as_mut_ptr(), s, &mut s);
            match r {
                0x80000005 => continue,        // 0x80000005 - STATUS_BUFFER_OVERFLOW
                0x8000001A => return Ok(None), // 0x8000001A - STATUS_NO_MORE_ENTRIES
                0 => return Ok(Some(*(b.as_ptr() as *const RegKeyValueFullInfo))),
                _ => return Err(winapi::nt_error(r)),
            }
        }
    }
}
pub fn NtSetValueKey(key: Key, value: impl MaybeString, value_type: u32, data: Option<impl AsRef<[u8]>>, data_size: u32) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        let v: WChars = value.into_string().into();
        let n = UnicodeString::new(&v);
        winapi::syscall!(
            *ntdll::NtSetValueKey,
            extern "stdcall" fn(Key, *const UnicodeString, u32, u32, *const u8, u32) -> u32,
            key,
            if n.is_empty() { ptr::null() } else { &n },
            0,
            value_type,
            data.map_or_else(ptr::null, |v| v.as_ref().as_ptr()),
            data_size
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn NtCreateKey(root: Key, subkey: impl AsRef<str>, class: impl MaybeString, opts: u32, access: u32, sa: SecAttrs) -> Win32Result<(Key, bool)> {
    winapi::init_ntdll();
    let mut d = 0u32;
    let mut k = Key::default();
    let r = unsafe {
        // 0x100 - OBJ_OPENLINK
        // 0x008 - REG_OPTION_OPEN_LINK
        // 0x040 - OBJ_CASE_INSENSITIVE
        object_attrs!(
            subkey.as_ref(),
            root.0,
            false,
            0x40 | if opts & 0x8 != 0 { 0x100 } else { 0 },
            sa,
            None,
            _n,
            o
        );
        let u: WChars = class.into_string().into();
        let n = UnicodeString::new(&u);
        winapi::syscall!(
            *ntdll::NtCreateKey,
            extern "stdcall" fn(*mut Key, u32, *const ObjectAttributes, u32, *const UnicodeString, u32, *mut u32) -> u32,
            &mut k,
            access,
            &o,
            0,
            if n.is_empty() { ptr::null() } else { &n },
            opts,
            &mut d
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok((k, d == 1))
    }
}

/////////////////////////////////////
// Mailslot Functions
/////////////////////////////////////
pub fn CreateMailslot(name: impl AsRef<str>, max_message: u32, timeout: i32, sa: SecAttrs) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut h = Handle::default();
    let r = unsafe {
        let t = if timeout == -1 {
            0xFFFFFFFFFFFFFFFi64
        } else {
            timeout as i64 * -10000
        };
        let mut i = IoStatusBlock::default();
        let n = UnicodeStr::from(if name.as_ref().as_bytes()[0] != b'\\' {
            winapi::normalize_path_to_nt(crypt::get_or(0, r"\\.\mailslot\").to_string() + name.as_ref())
        } else {
            winapi::normalize_path_to_nt(name.as_ref())
        });
        // 0x40 - OBJ_CASE_INSENSITIVE
        let o = ObjectAttributes::new(Some(&n.value), false, 0x40, sa, None);
        winapi::syscall!(
            *ntdll::NtCreateMailslotFile,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes, *mut IoStatusBlock, u32, u32, u32, *const i64) -> u32,
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
        0xC00000BB | 0xC0000010 => Err(Win32Error::InvalidFilename),
        0 => Ok(h.into()),
        _ => Err(winapi::nt_error(r)),
    }
}

/////////////////////////////////////
// Heap Functions
/////////////////////////////////////
#[inline]
pub fn RtlFreeUnicodeString(v: &UnicodeString) {
    ignore_error!(HeapFree(winapi::GetProcessHeap(), v.buffer.as_ptr()));
}
pub fn HeapDestroy(h: impl AsHandle) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::RtlDestroyHeap,
            extern "stdcall" fn(Handle) -> Handle,
            h.as_handle()
        )
    };
    if r.is_invalid() {
        Ok(())
    } else {
        Err(winapi::last_error())
    }
}
#[inline]
pub fn HeapFree<T>(h: impl AsHandle, v: *const T) -> bool {
    winapi::init_ntdll();
    unsafe {
        winapi::syscall!(
            *ntdll::RtlFreeHeap,
            extern "stdcall" fn(Handle, u32, *const T) -> u32,
            h.as_handle(),
            0,
            v
        ) == 1
    }
}
pub fn HeapAlloc(h: impl AsHandle, s: usize, zeroed: bool) -> Win32Result<Region> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::RtlAllocateHeap,
            extern "stdcall" fn(Handle, u32, usize) -> usize,
            h.as_handle(),
            if zeroed { 0x8 } else { 0 },
            s
        )
    };
    if r == 0 {
        Err(winapi::last_error())
    } else {
        Ok(r.into())
    }
}
pub fn HeapCreate(flags: u32, initial_size: usize, max_size: usize) -> Win32Result<Handle> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::RtlCreateHeap,
            extern "stdcall" fn(u32, *const (), usize, usize, *const (), *const ()) -> Handle,
            flags,
            ptr::null(),
            initial_size,
            max_size,
            ptr::null(),
            ptr::null()
        )
    };
    if r.is_invalid() {
        Err(winapi::last_error())
    } else {
        Ok(r)
    }
}
pub fn HeapReAlloc(h: impl AsHandle, flags: u32, mem: Region, size: usize) -> Win32Result<Region> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::RtlReAllocateHeap,
            extern "stdcall" fn(Handle, u32, usize, usize) -> Region,
            h.as_handle(),
            flags,
            mem.0,
            size
        )
    };
    if r.is_invalid() {
        Err(winapi::last_error())
    } else {
        Ok(r)
    }
}

/////////////////////////////////////
// Mutex Functions
/////////////////////////////////////
pub fn QueryMutex(h: impl AsHandle) -> Win32Result<i32> {
    winapi::init_ntdll();
    let mut n = [0u32; 2];
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtQueryMutant,
            extern "stdcall" fn(Handle, u32, *mut u32, u32, *mut u32) -> u32,
            h.as_handle(),
            0,
            n.as_mut_ptr(),
            0x8,
            ptr::null_mut()
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(n[0] as i32)
    }
}
pub fn ReleaseMutex(h: impl AsHandle) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtReleaseMutant,
            extern "stdcall" fn(Handle, *mut u32) -> u32,
            h.as_handle(),
            ptr::null_mut()
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn OpenMutex(access: u32, inherit: bool, name: impl AsRef<str>) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut h = Handle::default();
    let r = unsafe {
        object_attrs!(
            winapi::fix_name(name.as_ref()),
            Handle::INVALID,
            inherit,
            0,
            None,
            None,
            _n,
            o
        );
        winapi::syscall!(
            *ntdll::NtOpenMutant,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes) -> u32,
            &mut h,
            access,
            &o
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h.into())
    }
}
pub fn CreateMutex(sa: SecAttrs, inherit: bool, initial: bool, name: impl MaybeString) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut h = Handle::default();
    let r = unsafe {
        object_attrs!(
            winapi::fix_name(name),
            Handle::INVALID,
            inherit,
            0,
            sa,
            None,
            _n,
            o
        );
        winapi::syscall!(
            *ntdll::NtCreateMutant,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes, u32) -> u32,
            &mut h,
            0x1F0001,
            &o,
            if initial { 1 } else { 0 }
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h.into())
    }
}

/////////////////////////////////////
// Event Functions
/////////////////////////////////////
pub fn SetEvent(h: impl AsHandle) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtSetEvent,
            extern "stdcall" fn(Handle, *mut u32) -> u32,
            h.as_handle(),
            ptr::null_mut()
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn ResetEvent(h: impl AsHandle) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtResetEvent,
            extern "stdcall" fn(Handle, *mut u32) -> u32,
            h.as_handle(),
            ptr::null_mut()
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn QueryEvent(h: impl AsHandle) -> Win32Result<u32> {
    winapi::init_ntdll();
    let mut n = [0u32; 2];
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtQueryEvent,
            extern "stdcall" fn(Handle, u32, *mut u32, u32, *mut u32) -> u32,
            h.as_handle(),
            0,
            n.as_mut_ptr(),
            0x8,
            ptr::null_mut()
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(n[1])
    }
}
pub fn OpenEvent(access: u32, inherit: bool, name: impl AsRef<str>) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut h = Handle::default();
    let r = unsafe {
        object_attrs!(
            winapi::fix_name(name.as_ref()),
            Handle::INVALID,
            inherit,
            0,
            None,
            None,
            _n,
            o
        );
        winapi::syscall!(
            *ntdll::NtOpenEvent,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes) -> u32,
            &mut h,
            access,
            &o
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h.into())
    }
}
pub fn CreateEvent(sa: SecAttrs, inherit: bool, initial: bool, manual: bool, name: impl MaybeString) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut h = Handle::default();
    let r = unsafe {
        object_attrs!(
            winapi::fix_name(name),
            Handle::INVALID,
            inherit,
            0,
            sa,
            None,
            _n,
            o
        );
        winapi::syscall!(
            *ntdll::NtCreateEvent,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes, u32, u32) -> u32,
            &mut h,
            0x1F0003,
            &o,
            if manual { 0 } else { 1 },
            if initial { 1 } else { 0 }
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h.into())
    }
}

/////////////////////////////////////
// Semaphore Functions
/////////////////////////////////////
pub fn QuerySemaphore(h: impl AsHandle) -> Win32Result<(u32, u32)> {
    winapi::init_ntdll();
    let mut n = [0u32; 2];
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtQuerySemaphore,
            extern "stdcall" fn(Handle, u32, *mut u32, u32, *mut u32) -> u32,
            h.as_handle(),
            0,
            n.as_mut_ptr(),
            0x8,
            ptr::null_mut()
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok((n[0], n[1]))
    }
}
pub fn ReleaseSemaphore(h: impl AsHandle, count: u32) -> Win32Result<u32> {
    winapi::init_ntdll();
    let mut n = 0u32;
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtReleaseSemaphore,
            extern "stdcall" fn(Handle, u32, *mut u32) -> u32,
            h.as_handle(),
            count,
            &mut n
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(n)
    }
}
pub fn OpenSemaphore(access: u32, inherit: bool, name: impl AsRef<str>) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut h = Handle::default();
    let r = unsafe {
        object_attrs!(
            winapi::fix_name(name.as_ref()),
            Handle::INVALID,
            inherit,
            0,
            None,
            None,
            _n,
            o
        );
        winapi::syscall!(
            *ntdll::NtOpenSemaphore,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes) -> u32,
            &mut h,
            access,
            &o
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h.into())
    }
}
pub fn CreateSemaphore(sa: SecAttrs, inherit: bool, initial: u32, max: u32, name: impl MaybeString) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut h = Handle::default();
    let r = unsafe {
        object_attrs!(
            winapi::fix_name(name),
            Handle::INVALID,
            inherit,
            0,
            sa,
            None,
            _n,
            o
        );
        winapi::syscall!(
            *ntdll::NtCreateSemaphore,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes, u32, u32) -> u32,
            &mut h,
            0x1F0003,
            &o,
            initial,
            max
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h.into())
    }
}

/////////////////////////////////////
// Timer Functions
/////////////////////////////////////
pub fn CancelWaitableTimer(h: impl AsHandle) -> Win32Result<bool> {
    winapi::init_ntdll();
    let mut n = 0u32;
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtCancelTimer,
            extern "stdcall" fn(Handle, *mut u32) -> u32,
            h.as_handle(),
            &mut n
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(n == 1)
    }
}
pub fn QueryWaitableTimer(h: impl AsHandle) -> Win32Result<(bool, u64)> {
    winapi::init_ntdll();
    let mut i = [0u64; 2];
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtQueryTimer,
            extern "stdcall" fn(Handle, u32, *mut u64, u32, *mut u32) -> u32,
            h.as_handle(),
            0,
            i.as_mut_ptr(),
            0x10,
            ptr::null_mut()
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok((i[1] > 0, i[0]))
    }
}
pub fn OpenWaitableTimer(access: u32, inherit: bool, name: impl AsRef<str>) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut h = Handle::default();
    let r = unsafe {
        object_attrs!(
            winapi::fix_name(name.as_ref()),
            Handle::INVALID,
            inherit,
            0,
            None,
            None,
            _n,
            o
        );
        winapi::syscall!(
            *ntdll::NtOpenTimer,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes) -> u32,
            &mut h,
            access,
            &o
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h.into())
    }
}
pub fn CreateWaitableTimer(sa: SecAttrs, inherit: bool, manual: bool, name: impl MaybeString) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut h = Handle::default();
    let r = unsafe {
        object_attrs!(
            winapi::fix_name(name),
            Handle::INVALID,
            inherit,
            0,
            sa,
            None,
            _n,
            o
        );
        winapi::syscall!(
            *ntdll::NtCreateTimer,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes, u32) -> u32,
            &mut h,
            0x1F0003, // 0x1F0003 - FULL_CONTROL (for Timers)
            &o,
            if manual { 0 } else { 1 }
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h.into())
    }
}
pub fn SetWaitableTimer(h: impl AsHandle, microseconds: u64, repeat_mills: u32, func: Option<TimerFunc>, arg: Option<usize>, resume: bool) -> Win32Result<bool> {
    winapi::init_ntdll();
    let mut n = 0u32;
    let r = unsafe {
        let t = (microseconds as i64).wrapping_mul(-10);
        winapi::syscall!(
            *ntdll::NtSetTimer,
            extern "stdcall" fn(Handle, *const i64, Option<TimerFunc>, usize, u32, u32, *mut u32) -> u32,
            h.as_handle(),
            &t,
            func,
            arg.unwrap_or_default(),
            if resume { 1 } else { 0 },
            repeat_mills,
            &mut n
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(n == 1)
    }
}

/////////////////////////////////////
// Sleep/Yield Functions
/////////////////////////////////////
pub fn NtYieldExecution() -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe { winapi::syscall!(*ntdll::NtYieldExecution, extern "stdcall" fn() -> u32,) };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn SleepEx(microseconds: i64, alertable: bool) -> Win32Result<bool> {
    winapi::init_ntdll();
    let r = unsafe {
        let t = microseconds.wrapping_mul(-10);
        winapi::syscall!(
            *ntdll::NtDelayExecution,
            extern "stdcall" fn(u32, *const i64) -> u32,
            if alertable { 1 } else { 0 },
            if microseconds == -1 { ptr::null() } else { &t }
        )
    };
    match r {
        0xC0 => Ok(true), // 0xC0 - STATUS_USER_APC
        0 => Ok(false),
        _ => Err(winapi::nt_error(r)),
    }
}

/////////////////////////////////////
// System Information Functions
/////////////////////////////////////
pub fn GetVersionNumbers() -> (u32, u32, u16) {
    winapi::init_ntdll();
    let (mut major, mut minor, mut sp) = (0u32, 0u32, 0u32);
    unsafe {
        winapi::syscall!(
            *ntdll::RtlGetNtVersionNumbers,
            extern "stdcall" fn(*mut u32, *mut u32, *mut u32) -> u32,
            &mut major,
            &mut minor,
            &mut sp
        )
    };
    // NOTE(dij): Why the sp is ONLY u16
    //
    // Source: https://www.geoffchappell.com/studies/windows/win32/ntdll/api/ldrinit/getntversionnumbers.htm
    //
    // The optional BuildNumber argument gives the address of a variable that is to
    // receive a number that describes the build. This too can be NULL if the number
    // is not wanted. The low 16 bits are the build number as commonly understood.
    // The high four bits of the number distinguish free and checked builds.
    (major, minor, sp as u16)
}
#[inline]
pub fn GetLogicalDrives() -> Win32Result<u32> {
    let mut buf = [0u32; 14];
    // 0x17 - ProcessDeviceMap
    NtQueryInformationProcess(winapi::CURRENT_PROCESS, 0x17, buf.as_mut_ptr(), 0x24)?;
    Ok(buf[0])
}
pub fn NtQuerySystemInformation<T>(class: u32, buf: *mut T, size: u32) -> Win32Result<u32> {
    winapi::init_ntdll();
    let mut n: u32 = 0u32;
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtQuerySystemInformation,
            extern "stdcall" fn(u32, *mut T, u32, *mut u32) -> u32,
            class,
            buf,
            size,
            &mut n
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(n)
    }
}

/////////////////////////////////////
// Environment Functions
/////////////////////////////////////
pub fn SetCurrentDirectory(dir: impl AsRef<str>) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        let n: WChars = dir.as_ref().into();
        let u = UnicodeString::new(&n);
        winapi::syscall!(
            *ntdll::RtlSetCurrentDirectory,
            extern "stdcall" fn(*const UnicodeString) -> u32,
            &u
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn SetEnvironmentVariable(key: impl AsRef<str>, value: impl MaybeString) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        let k: WChars = key.as_ref().into();
        let v: WChars = value.into_string().into();
        winapi::syscall!(
            *ntdll::RtlSetEnvironmentVar,
            extern "stdcall" fn(usize, *const u16, usize, *const u16, usize) -> u32,
            0,
            k.as_ptr(),
            k.len(),
            if v.is_empty() { ptr::null() } else { v.as_ptr() },
            v.len()
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}

/////////////////////////////////////
// Thread Functions
/////////////////////////////////////
#[inline]
pub fn GetThreadID(h: impl AsHandle) -> Win32Result<u32> {
    let mut i = ThreadBasicInfo::default();
    // 0x0 - ThreadBasicInformation
    NtQueryInformationThread(h, 0, &mut i, size_of::<ThreadBasicInfo>() as u32)?;
    Ok(i.client_id.thread as u32)
}
pub fn ResumeThread(h: impl AsHandle) -> Win32Result<u32> {
    winapi::init_ntdll();
    let mut n = 0u32;
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtResumeThread,
            extern "stdcall" fn(Handle, *mut u32) -> u32,
            h.as_handle(),
            &mut n
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(n)
    }
}
pub fn SuspendThread(h: impl AsHandle) -> Win32Result<u32> {
    winapi::init_ntdll();
    let mut n = 0u32;
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtSuspendThread,
            extern "stdcall" fn(Handle, *mut u32) -> u32,
            h.as_handle(),
            &mut n
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(n)
    }
}
#[inline]
pub fn GetExitCodeThread(h: impl AsHandle) -> Win32Result<u32> {
    let mut i = ThreadBasicInfo::default();
    // 0x0 - ThreadBasicInformation
    NtQueryInformationThread(h, 0, &mut i, size_of::<ThreadBasicInfo>() as u32)?;
    Ok(i.exit_status)
}
pub fn TerminateThread(h: impl AsHandle, code: u32) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtTerminateThread,
            extern "stdcall" fn(Handle, u32) -> u32,
            h.as_handle(),
            code
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn OpenThread(access: u32, inherit: bool, tid: u32) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut h = Handle::default();
    let r = unsafe {
        let c = ClientID {
            thread:  tid as usize,
            process: 0usize,
        };
        let o = ObjectAttributes::new(None, inherit, 0, None, None);
        winapi::syscall!(
            *ntdll::NtOpenThread,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes, *const ClientID) -> u32,
            &mut h,
            access,
            &o,
            &c
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h.into())
    }
}
pub fn NtSetInformationThread<T>(h: impl AsHandle, class: u32, buf: *const T, size: u32) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtSetInformationThread,
            extern "stdcall" fn(Handle, u32, *const T, u32) -> u32,
            h.as_handle(),
            class,
            buf,
            size
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn NtQueryInformationThread<T>(h: impl AsHandle, class: u32, buf: *mut T, size: u32) -> Win32Result<u32> {
    winapi::init_ntdll();
    let mut n: u32 = 0u32;
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtQueryInformationThread,
            extern "stdcall" fn(Handle, u32, *mut T, u32, *mut u32) -> u32,
            h.as_handle(),
            class,
            buf,
            size,
            &mut n
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(n)
    }
}
pub fn NtImpersonateThread(h: impl AsHandle, src: impl AsHandle, s: &SecurityQualityOfService) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtImpersonateThread,
            extern "stdcall" fn(Handle, Handle, *const SecurityQualityOfService) -> u32,
            h.as_handle(),
            src.as_handle(),
            s
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn CreateThreadEx(h: impl AsHandle, stack_size: usize, start: usize, args: usize, suspended: bool) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    // NOTE(dij): NtCreateThreadEx is avaliable after Windows Xp.
    if !winapi::is_min_windows_vista() {
        winapi::init_kernel32();
        let t = unsafe {
            winapi::syscall!(
                *kernel32::CreateRemoteThread,
                extern "stdcall" fn(Handle, *const SecurityAttributes, usize, usize, usize, u32, *mut u32) -> Handle,
                h.as_handle(),
                ptr::null(),
                stack_size,
                start,
                args,
                // 0x00004 - CREATE_SUSPENDED
                // 0x10000 - STACK_SIZE_PARAM_IS_A_RESERVATION
                if suspended { 0x4 } else { 0 } | if stack_size > 0 { 0x10000 } else { 0 },
                ptr::null_mut()
            )
        };
        return if t.is_invalid() {
            Err(winapi::last_error())
        } else {
            Ok(t.into())
        };
    }
    let mut t = Handle::default();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtCreateThreadEx,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes, Handle, usize, usize, u32, usize, usize, usize, usize) -> u32,
            &mut t,
            0x1FFFFF, // 0x1FFFFF - ALL_ACCESS
            ptr::null(),
            h.as_handle(),
            start,
            args,
            if suspended { 1 } else { 0 },
            0,
            stack_size,
            0,
            0
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(t.into())
    }
}

/////////////////////////////////////
// Process Functions
/////////////////////////////////////
#[inline]
pub fn GetProcessID(h: impl AsHandle) -> Win32Result<u32> {
    let mut i = ProcessBasicInfo::default();
    // 0x0 - ProcessBasicInformation
    NtQueryInformationProcess(h, 0, &mut i, size_of::<ProcessBasicInfo>() as u32)?;
    Ok(i.process_id as u32)
}
pub fn NtResumeProcess(h: impl AsHandle) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtResumeProcess,
            extern "stdcall" fn(Handle) -> u32,
            h.as_handle()
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn NtSuspendProcess(h: impl AsHandle) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtSuspendProcess,
            extern "stdcall" fn(Handle) -> u32,
            h.as_handle()
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn IsWoW64Process(h: impl AsHandle) -> Win32Result<bool> {
    winapi::init_ntdll();
    if !winapi::is_min_windows_7() {
        let mut v: u32 = 0u32; // <- PEB64 Address
                               // 0x1A - ProcessWow64Information
        NtQueryInformationProcess(h, 0x1A, &mut v, 4)?;
        return Ok(v > 0);
    }
    // TODO(dij): Test this as we might not need to do this below and the Nt call
    //            seems to be called by the Rtl function.
    if !ntdll::RtlWow64GetProcessMachines.is_loaded() {
        return Ok(false);
    }
    let (mut p, mut n) = (0u16, 0u16);
    let r = unsafe {
        winapi::syscall!(
            *ntdll::RtlWow64GetProcessMachines,
            extern "stdcall" fn(Handle, *mut u16, *mut u16) -> u32,
            h.as_handle(),
            &mut p,
            &mut n
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(p > 0)
    }
}
#[inline]
pub fn GetExitCodeProcess(h: impl AsHandle) -> Win32Result<u32> {
    let mut i = ProcessBasicInfo::default();
    // 0x0 - ProcessBasicInformation
    NtQueryInformationProcess(h, 0, &mut i, size_of::<ProcessBasicInfo>() as u32)?;
    Ok(i.exit_status)
}
pub fn GetProcessFileName(h: impl AsHandle) -> Win32Result<String> {
    winapi::init_ntdll();
    let func = unsafe {
        winapi::make_syscall!(
            *ntdll::NtQueryInformationProcess,
            extern "stdcall" fn(Handle, u32, *mut u16, u32, *mut u32) -> u32
        )
    };
    let v = h.as_handle();
    let mut n = 0x20Au32;
    let mut b: Blob<u16, 300> = Blob::new();
    loop {
        b.resize_as_bytes(n as usize);
        // 0x1B - ProcessImageFileName
        let r = func(v, 0x1B, b.as_mut_ptr(), b.len_as_bytes() as u32, &mut n);
        match r {
            0 => break,
            0xC0000004 => continue, // STATUS_INFO_LENGTH_MISMATCH
            _ => return Err(winapi::nt_error(r)),
        }
    }
    // Retrieves a UNICODE_STRING value containing the name of the image file
    // for the process.
    //
    // First u16 is the 'length' value.
    let y = cmp::min(n as usize, b.len());
    let s = &b[winapi::PTR_SIZE..(y / 2) - 1];
    let t = cmp::min(y, b[0] as usize) / 2;
    match s.iter().rposition(|v| *v == b'\\' as u16) {
        Some(i) => Ok((&s[i + 1..t]).decode_utf16()),
        None => Ok((&s[0..t]).decode_utf16()),
    }
}
pub fn TerminateProcess(h: impl AsHandle, code: u32) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtTerminateProcess,
            extern "stdcall" fn(Handle, u32) -> u32,
            h.as_handle(),
            code
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn CheckRemoteDebuggerPresent(h: impl AsHandle) -> Win32Result<bool> {
    let mut r = 0usize;
    winapi::NtQueryInformationProcess(h, 0x7, &mut r, PTR_SIZE as u32)?;
    Ok(r > 0)
}
pub fn OpenProcess(access: u32, inherit: bool, pid: u32) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut h = Handle::default();
    let r = unsafe {
        let c = ClientID {
            thread:  0usize,
            process: pid as usize,
        };
        let o = ObjectAttributes::new(None, inherit, 0, None, None);
        winapi::syscall!(
            *ntdll::NtOpenProcess,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes, *const ClientID) -> u32,
            &mut h,
            access,
            &o,
            &c
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h.into())
    }
}
pub fn NtSetInformationProcess<T>(h: impl AsHandle, class: u32, buf: *const T, size: u32) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtSetInformationProcess,
            extern "stdcall" fn(Handle, u32, *const T, u32) -> u32,
            h.as_handle(),
            class,
            buf,
            size
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn NtQueryInformationProcess<T>(h: impl AsHandle, class: u32, buf: *mut T, size: u32) -> Win32Result<u32> {
    winapi::init_ntdll();
    let mut n = 0u32;
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtQueryInformationProcess,
            extern "stdcall" fn(Handle, u32, *mut T, u32, *mut u32) -> u32,
            h.as_handle(),
            class,
            buf,
            size,
            &mut n
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(n)
    }
}

/////////////////////////////////////
// Process Functions (WoW64)
/////////////////////////////////////
#[cfg(not(target_pointer_width = "64"))]
pub fn NtWow64QueryInformationProcess64<T>(h: impl AsHandle, class: u32, buf: *mut T, size: u32) -> Win32Result<u32> {
    winapi::init_ntdll();
    let mut n = 0u32;
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtWow64QueryInformationProcess64,
            extern "stdcall" fn(Handle, u32, *mut T, u32, *mut u32) -> u32,
            h.as_handle(),
            class,
            buf,
            size,
            &mut n
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(n)
    }
}

/////////////////////////////////////
// IO/Async Functions
/////////////////////////////////////
pub fn CancelIoEx(h: impl AsHandle, olp: &mut Overlapped) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        let mut s = IoStatusBlock::default();
        if winapi::is_min_windows_7() {
            // NOTE(dij): NtCancelIoFileEx only exists on Win7+
            winapi::syscall!(
                *ntdll::NtCancelIoFileEx,
                extern "stdcall" fn(Handle, *mut Overlapped, *mut IoStatusBlock) -> u32,
                h.as_handle(),
                olp,
                &mut s
            )
        } else {
            winapi::syscall!(
                *ntdll::NtCancelIoFile,
                extern "stdcall" fn(Handle, *mut IoStatusBlock) -> u32,
                h.as_handle(),
                &mut s
            )
        }
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn GetOverlappedResult(h: impl AsHandle, olp: &Overlapped, wait: bool) -> Win32Result<usize> {
    // 0x103 - STATUS_PENDING
    if olp.internal == 0x103 {
        if !wait {
            return Err(Win32Error::IoPending);
        }
        WaitForSingleObject(
            if olp.event.0 > 0 { olp.event } else { h.as_handle() },
            -1,
            true,
        )?;
    }
    match olp.internal {
        // 0xC0000011 - STATUS_END_OF_FILE
        0xC0000011 => Ok(0),
        0 => Ok(olp.internal_high),
        _ => Err(winapi::nt_error(olp.internal as u32)),
    }
}
pub fn WaitForSingleObject(h: impl AsHandle, microseconds: i32, alertable: bool) -> Win32Result<u32> {
    winapi::init_ntdll();
    let r = unsafe {
        let t = (microseconds as i64).wrapping_mul(-10);
        winapi::syscall!(
            *ntdll::NtWaitForSingleObject,
            extern "stdcall" fn(usize, u32, *const i64) -> u32,
            winapi::check_nt_handle(h.as_handle()),
            if alertable { 1 } else { 0 },
            if microseconds == -1 { ptr::null() } else { &t }
        )
    };
    // 0x000 - STATUS_WAIT_0
    // 0x0C0 - STATUS_USER_APC
    // 0x101 - STATUS_ALERTED
    // 0x102 - STATUS_TIMEOUT
    match r {
        0 | 0xC0 | 0x101 | 0x102 => Ok(r),
        _ => Err(winapi::nt_error(r)),
    }
}
pub fn WaitForMultipleObjects<T: AsHandle>(h: &[T], size: usize, all: bool, microseconds: i32, alertable: bool) -> Win32Result<u32> {
    if h.len() > 64 || size == 0 {
        return Err(Win32Error::InvalidArgument);
    }
    let mut x: Blob<usize, 64> = Blob::with_capacity(h.len());
    for (n, i) in h.iter().enumerate() {
        if n >= size {
            break;
        }
        x.push(winapi::check_nt_handle(i.as_handle()));
    }
    wait_for_multiple_objects(&x, x.len(), all, microseconds, alertable)
}

/////////////////////////////////////
// Token Functions
/////////////////////////////////////
#[inline]
pub fn RevertToSelf() -> Win32Result<()> {
    SetThreadToken(winapi::CURRENT_THREAD, Handle::INVALID)
}
#[inline]
pub fn SetThreadToken(h: impl AsHandle, token: impl AsHandle) -> Win32Result<()> {
    let t = token.as_handle();
    // 0x5 - ThreadImpersonationToken
    NtSetInformationThread(h, 0x5, &t, winapi::PTR_SIZE as u32)
}
pub fn OpenProcessToken(h: impl AsHandle, access: u32) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut t = Handle::default();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtOpenProcessToken,
            extern "stdcall" fn(Handle, u32, *mut Handle) -> u32,
            h.as_handle(),
            access,
            &mut t
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(t.into())
    }
}
pub fn OpenThreadToken(h: impl AsHandle, access: u32, s: bool) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut t = Handle::default();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtOpenThreadToken,
            extern "stdcall" fn(Handle, u32, u32, *mut Handle) -> u32,
            h.as_handle(),
            access,
            if s { 1 } else { 0 },
            &mut t
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(t.into())
    }
}
pub fn GetTokenInformation<T>(h: impl AsHandle, class: u32, buf: *mut T, size: u32) -> Win32Result<u32> {
    winapi::init_ntdll();
    let mut n = size;
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtQueryInformationToken,
            extern "stdcall" fn(Handle, u32, *mut T, u32, *mut u32) -> u32,
            h.as_handle(),
            class,
            buf,
            size,
            &mut n
        )
    };
    // 0xC0000023 - STATUS_BUFFER_TOO_SMALL
    if r == 0xC0000023 && n > 0 {
        Ok(n)
    } else if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(n)
    }
}
pub fn SetTokenInformation<T>(h: impl AsHandle, class: u32, buf: *const T, size: u32) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtSetInformationToken,
            extern "stdcall" fn(Handle, u32, *const T, u32) -> u32,
            h.as_handle(),
            class,
            buf,
            size
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn DuplicateTokenEx(h: impl AsHandle, access: u32, sa: SecAttrs, level: u32, token: u32) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut n = Handle::default();
    let r = unsafe {
        let q = SecurityQualityOfService::level(level);
        let o = ObjectAttributes::new(None, false, 0, sa, Some(&q));
        winapi::syscall!(
            *ntdll::NtDuplicateToken,
            extern "stdcall" fn(Handle, u32, *const ObjectAttributes, u32, u32, *mut Handle) -> u32,
            h.as_handle(),
            access,
            &o,
            0,
            token,
            &mut n
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(n.into())
    }
}
pub fn AdjustTokenPrivileges(h: impl AsHandle, dis: bool, new: *const TokenPrivileges, new_len: u32, old: *mut TokenPrivileges, old_len: &mut u32) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtAdjustTokenPrivileges,
            extern "stdcall" fn(Handle, u32, *const TokenPrivileges, u32, *mut TokenPrivileges, *mut u32) -> u32,
            h.as_handle(),
            if dis { 1 } else { 0 },
            new,
            new_len,
            old,
            old_len
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}

/////////////////////////////////////
// Virtual Memory Functions
/////////////////////////////////////
pub fn NtFreeVirtualMemory(h: impl AsHandle, address: Region, size: usize, flags: u32) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        let mut s = size;
        let mut a = address;
        winapi::syscall!(
            *ntdll::NtFreeVirtualMemory,
            extern "stdcall" fn(Handle, *mut Region, *mut usize, u32) -> u32,
            h.as_handle(),
            &mut a,
            &mut s,
            flags
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn NtProtectVirtualMemory(h: impl AsHandle, address: Region, size: u32, access: u32) -> Win32Result<u32> {
    winapi::init_ntdll();
    let mut s = 0u32;
    let r = unsafe {
        let (mut n, mut x) = (size, address);
        winapi::syscall!(
            *ntdll::NtProtectVirtualMemory,
            extern "stdcall" fn(Handle, *mut Region, *mut u32, u32, *mut u32) -> u32,
            h.as_handle(),
            &mut x,
            &mut n,
            access,
            &mut s
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(s)
    }
}
pub fn NtReadVirtualMemory<T>(h: impl AsHandle, address: Region, size: usize, to: ReadInto<T>) -> Win32Result<usize> {
    winapi::init_ntdll();
    let mut s = 0usize;
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtReadVirtualMemory,
            extern "stdcall" fn(Handle, Region, *mut T, usize, *mut usize) -> u32,
            h.as_handle(),
            address,
            match to {
                ReadInto::Direct(v) => v,
                ReadInto::Pointer(p) => p,
            },
            size,
            &mut s
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(s)
    }
}
pub fn NtWriteVirtualMemory<T>(h: impl AsHandle, address: Region, size: usize, from: WriteFrom<T>) -> Win32Result<usize> {
    winapi::init_ntdll();
    let mut s = 0usize;
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtWriteVirtualMemory,
            extern "stdcall" fn(Handle, Region, *const T, usize, *mut usize) -> u32,
            h.as_handle(),
            address,
            match from {
                WriteFrom::Null => ptr::null(),
                WriteFrom::Direct(v) => v,
                WriteFrom::Pointer(p) => p,
            },
            size,
            &mut s
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(s)
    }
}
pub fn NtAllocateVirtualMemory(h: impl AsHandle, base: Region, size: usize, flags: u32, access: u32) -> Win32Result<Region> {
    winapi::init_ntdll();
    let mut a = base;
    let r = unsafe {
        let mut s = size;
        winapi::syscall!(
            *ntdll::NtAllocateVirtualMemory,
            extern "stdcall" fn(Handle, *mut Region, usize, *mut usize, u32, u32) -> u32,
            h.as_handle(),
            &mut a,
            0,
            &mut s,
            flags,
            access
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(a)
    }
}

/////////////////////////////////////
// Virtual Memory Functions (WoW64)
/////////////////////////////////////
#[cfg(not(target_pointer_width = "64"))]
pub fn NtWoW64ReadVirtualMemory64<T>(h: impl AsHandle, address: u64, size: u64, to: ReadInto<T>) -> Win32Result<u64> {
    winapi::init_ntdll();
    let mut s = 0u64;
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtWow64ReadVirtualMemory64,
            extern "stdcall" fn(Handle, u64, *mut T, u64, *mut u64) -> u32,
            h.as_handle(),
            address,
            match to {
                ReadInto::Direct(v) => v,
                ReadInto::Pointer(p) => p,
            },
            size,
            &mut s
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(s)
    }
}
#[cfg(not(target_pointer_width = "64"))]
pub fn NtWoW64WriteVirtualMemory64<T>(h: impl AsHandle, address: u64, size: u64, from: WriteFrom<T>) -> Win32Result<u64> {
    winapi::init_ntdll();
    let mut s = 0u64;
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtWow64WriteVirtualMemory64,
            extern "stdcall" fn(Handle, u64, *const T, u64, *mut u64) -> u32,
            h.as_handle(),
            address,
            match from {
                WriteFrom::Null => ptr::null(),
                WriteFrom::Direct(v) => v,
                WriteFrom::Pointer(p) => p,
            },
            size,
            &mut s
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(s)
    }
}
#[cfg(not(target_pointer_width = "64"))]
pub fn NtWoW64AllocateVirtualMemory64(h: impl AsHandle, base: u64, size: u64, flags: u32, access: u32) -> Win32Result<u64> {
    winapi::init_ntdll();
    let mut a = base;
    let r = unsafe {
        let mut s = size;
        winapi::syscall!(
            *ntdll::NtWow64AllocateVirtualMemory64,
            extern "stdcall" fn(Handle, *mut u64, u64, *mut u64, u32, u32) -> u32,
            h.as_handle(),
            &mut a,
            0,
            &mut s,
            flags,
            access
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(a)
    }
}

/////////////////////////////////////
// Section Functions
/////////////////////////////////////
pub fn NtUnmapViewOfSection(sec: impl AsHandle, proc: impl AsHandle) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtUnmapViewOfSection,
            extern "stdcall" fn(Handle, Handle) -> u32,
            sec.as_handle(),
            proc.as_handle()
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn NtCreateSection(access: u32, max_size: Option<u64>, protection: u32, attrs: u32, file: impl AsHandle) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut h = Handle::default();
    let r = unsafe {
        let m = max_size.map_or_else(ptr::null, |v| &v);
        winapi::syscall!(
            *ntdll::NtCreateSection,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes, *const u64, u32, u32, Handle) -> u32,
            &mut h,
            access,
            ptr::null(),
            m,
            protection,
            attrs,
            file.as_handle()
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h.into())
    }
}
pub fn NtMapViewOfSection(sec: impl AsHandle, proc: impl AsHandle, offset: usize, size: usize, inherit: u32, alloc_type: u32, access: u32) -> Win32Result<(usize, usize)> {
    winapi::init_ntdll();
    let (mut h, mut s) = (0usize, size);
    let r = unsafe {
        let mut o = offset;
        winapi::syscall!(
            *ntdll::NtMapViewOfSection,
            extern "stdcall" fn(Handle, Handle, *mut usize, usize, usize, *mut usize, *mut usize, u32, u32, u32) -> u32,
            sec.as_handle(),
            proc.as_handle(),
            &mut h, // Base Address
            0,
            0,
            &mut o,
            &mut s, // Size of Map
            inherit,
            alloc_type,
            access
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok((h, s))
    }
}

/////////////////////////////////////
// File (Non-NT) Functions
/////////////////////////////////////
#[inline]
pub fn DeleteFile(file: impl AsRef<str>) -> Win32Result<()> {
    // 0x10000 - DELETE
    // 0x00004 - FILE_SHARE_DELETE
    // 0x00001 - FILE_OPEN
    let h = winapi::NtCreateFile(file, Handle::INVALID, 0x10000, None, 0, 0x4, 0x1, 0)?;
    // 0xD - FileDispositionInformation
    let v = 1u32;
    NtSetInformationFile(h, 0xD, &v, 4)?;
    Ok(())
}
pub fn CreateHardLink<T: AsRef<str>>(original: T, link: T) -> Win32Result<()> {
    unsafe {
        let o = winapi::normalize_path_to_nt(link)
            .encode_utf16()
            .collect::<Blob<u16, 256>>();
        let mut b: Blob<u16, 256> = Blob::with_capacity(0x14 + o.len());
        b.write_item(FileLinkInformation {
            pad:            0u32,
            replace:        0u32,
            name_length:    cmp::min(o.len_as_bytes(), 0xFFFFFFFF) as u32, // Length, in bytes, of the file name string.
            root_directory: 0usize,
        });
        b.extend_from_slice(&o);
        // 0x100100 - SYNCHRONIZE | FILE_WRITE_ATTRIBUTES
        // 0x000007 - FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
        // 0x200000 - FILE_FLAG_OPEN_REPARSE_POINT
        let h = CreateFile(original, 0x100100, 0x7, None, 0x3, 0x200000)?;
        // 0xB - FileLinkInformation
        NtSetInformationFile(
            h,
            0xB,
            b.as_ptr(),
            cmp::min(b.len_as_bytes(), 0xFFFFFFFF) as u32,
        )?;
        Ok(())
    }
}
#[inline]
pub fn SetFileAttributes(file: impl AsRef<str>, attrs: u32) -> Win32Result<()> {
    // 0x100110 - SYNCHRONIZE | FILE_WRITE_ATTRIBUTES | FILE_WRITE_EA
    set_file_attrs_by_handle(CreateFile(file, 0x100110, 0x7, None, 0x3, 0)?, attrs)
}
pub fn CreateDirectory(path: impl AsRef<str>, recurse: bool) -> Win32Result<()> {
    if !recurse {
        // 0x100001 - SYNCHRONIZE | FILE_LIST_DIRECTORY
        // 0x000080 - FILE_ATTRIBUTE_NORMAL
        // 0x204021 - FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT |
        //             FILE_OPEN_FOR_BACKUP_INTENT | FILE_OPEN_REPARSE_POINT
        // 0x000002 - FILE_CREATE
        NtCreateFile(path, Handle::INVALID, 0x100001, None, 0x80, 3, 2, 0x204021)?;
        return Ok(());
    }
    let n = winapi::normalize_path_to_nt(path);
    let v = n.split('\\');
    let mut r = false;
    let mut b = String::with_capacity(n.len());
    for i in v {
        if i.is_empty() {
            continue;
        }
        b.push('\\');
        b.push_str(i);
        if r {
            // 0x100001 - SYNCHRONIZE | FILE_LIST_DIRECTORY
            // 0x000080 - FILE_ATTRIBUTE_NORMAL
            // 0x204021 - FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT |
            //             FILE_OPEN_FOR_BACKUP_INTENT | FILE_OPEN_REPARSE_POINT
            // 0x000003 - FILE_OPEN_IF
            NtCreateFile(&b, Handle::INVALID, 0x100001, None, 0x80, 3, 0x3, 0x204021)?;
        }
        // Let's pass the drive path before attempting to create anything as this
        // should be the full path.
        if !r && i.contains(':') {
            r = true;
        }
    }
    Ok(())
}
#[inline]
pub fn WriteFile(h: impl AsHandle, buf: &[u8], olp: OverlappedIo) -> Win32Result<usize> {
    NtWriteFile(h, olp, buf, None)
}
pub fn SetFilePointerEx(h: impl AsHandle, distance: i64, method: u32) -> Win32Result<u64> {
    let n = match method {
        // FILE_BEGIN
        0 => distance as u64,
        // FILE_CURRENT
        1 => {
            let mut n = 0i64;
            // 0xE - FilePositionInformation
            NtQueryInformationFile(&h, 0xE, &mut n, 8)?;
            (n + distance) as u64
        },
        // FILE_END
        2 => {
            let mut i = FileStandardInformation::default();
            // 0x5 - FileStandardInfo
            NtQueryInformationFile(&h, 0x5, &mut i, 0x20)?;
            (i.end_of_file as i64 + distance) as u64
        },
        _ => return Err(Win32Error::InvalidArgument),
    };
    // 0xE - FilePositionInformation
    NtSetInformationFile(h, 0xE, &n, 8)?;
    Ok(n)
}
#[inline]
pub fn ReadFile(h: impl AsHandle, buf: &mut [u8], olp: OverlappedIo) -> Win32Result<usize> {
    NtReadFile(h, olp, buf, None)
}
#[inline]
pub fn CreateFile(file: impl AsRef<str>, access: u32, share_mode: u32, sa: SecAttrs, disposition: u32, attrs: u32) -> Win32Result<OwnedHandle> {
    let (a, v, d, f) = winapi::std_flags_to_nt(access, disposition, attrs);
    NtCreateFile(file, Handle::INVALID, a, sa, v, share_mode, d, f)
}

/////////////////////////////////////
// File Functions
/////////////////////////////////////
pub fn NtFlushBuffersFile(h: impl AsHandle) -> Win32Result<()> {
    winapi::init_ntdll();
    let r = unsafe {
        let mut i = IoStatusBlock::default();
        winapi::syscall!(
            *ntdll::NtFlushBuffersFile,
            extern "stdcall" fn(Handle, *mut IoStatusBlock) -> u32,
            h.as_handle(),
            &mut i
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(())
    }
}
pub fn NtQueryInformationFile<T>(h: impl AsHandle, class: u32, buf: *mut T, size: u32) -> Win32Result<u32> {
    winapi::init_ntdll();
    let mut i = IoStatusBlock::default();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtQueryInformationFile,
            extern "stdcall" fn(Handle, *mut IoStatusBlock, *mut T, u32, u32) -> u32,
            h.as_handle(),
            &mut i,
            buf,
            size,
            class
        )
    };
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(i.info as u32)
    }
}
pub fn NtSetInformationFile<T>(h: impl AsHandle, class: u32, buf: *const T, size: u32) -> Win32Result<u32> {
    winapi::init_ntdll();
    let mut i = IoStatusBlock::default();
    let r = unsafe {
        winapi::syscall!(
            *ntdll::NtSetInformationFile,
            extern "stdcall" fn(Handle, *mut IoStatusBlock, *const T, u32, u32) -> u32,
            h.as_handle(),
            &mut i,
            buf,
            size,
            class
        )
    };
    match r {
        // 0x0000000D - FileDispositionInformation
        // 0x00000040 - FileDispositionInformationEx
        // 0xC0000101 - STATUS_DIRECTORY_NOT_EMPTY
        0xC0000101 if class == 0xD || class == 0x40 => Err(Win32Error::DirectoryNotEmpty),
        0 => Ok(i.info as u32),
        _ => Err(winapi::nt_error(r)),
    }
}
pub fn NtWriteFile(h: impl AsHandle, olp: OverlappedIo, buf: &[u8], offset: Option<u64>) -> Win32Result<usize> {
    winapi::init_ntdll();
    let (r, n) = unsafe {
        let mut t = Overlapped::default();
        let (w, x) = olp.map_or_else(
            || (true, &mut t),
            |v| {
                v.internal_high = 0;
                (false, v)
            },
        );
        let o = if offset.is_some() {
            //&& x.offset == 0 && x.offset_high == 0 {
            let t = offset.unwrap_or_default();
            x.offset = (t & 0xFFFFFFFF) as u32;
            x.offset_high = (t >> 32) as u32;
            t
        } else {
            ((x.offset_high as u64) << 32) | x.offset as u64
        };
        let r = winapi::syscall!(
            *ntdll::NtWriteFile,
            extern "stdcall" fn(usize, usize, usize, *mut Overlapped, *mut Overlapped, *const u8, u32, *const u64, usize) -> u32,
            winapi::check_nt_handle(h.as_handle()),
            x.event.0,
            0,
            if x.event.0 & 1 == 0 { ptr::null_mut() } else { x },
            x,
            buf.as_ptr(),
            cmp::min(buf.len(), 0xFFFFFFFF) as u32,
            if offset.is_some() { &o } else { ptr::null() },
            0
        );
        // 0x103 - STATUS_PENDING
        let z = if r == 0x103 && w {
            ignore_error!(WaitForSingleObject(h, -1, false));
            x.internal as u32
        } else {
            r
        };
        (z, x.internal_high as usize)
    };
    match r {
        0x103 => Err(Win32Error::IoPending), // 0x103 - STATUS_PENDING
        0 => Ok(n),
        _ => Err(winapi::nt_error(r)),
    }
}
pub fn NtReadFile(h: impl AsHandle, olp: OverlappedIo, buf: &mut [u8], offset: Option<u64>) -> Win32Result<usize> {
    winapi::init_ntdll();
    let (r, n) = unsafe {
        let mut t = Overlapped::default();
        let (w, x) = olp.map_or_else(
            || (true, &mut t),
            |v| {
                v.internal_high = 0;
                (false, v)
            },
        );
        let o = if offset.is_some() {
            let t = offset.unwrap_or_default();
            x.offset = (t & 0xFFFFFFFF) as u32;
            x.offset_high = (t >> 32) as u32;
            t
        } else {
            ((x.offset_high as u64) << 32) | x.offset as u64
        };
        let r = winapi::syscall!(
            *ntdll::NtReadFile,
            extern "stdcall" fn(usize, usize, usize, *mut Overlapped, *mut Overlapped, *const u8, u32, *const u64, usize) -> u32,
            winapi::check_nt_handle(h.as_handle()),
            x.event.0,
            0,
            if x.event.0 & 1 == 0 { ptr::null_mut() } else { x },
            x,
            buf.as_mut_ptr(),
            cmp::min(buf.len(), 0xFFFFFFFF) as u32,
            if offset.is_some() { &o } else { ptr::null() },
            0
        );
        // 0x103 - STATUS_PENDING
        let z = if r == 0x103 && w {
            ignore_error!(WaitForSingleObject(h, -1, false));
            x.internal as u32
        } else {
            r
        };
        (z, x.internal_high as usize)
    };
    // 0xC0000011 - STATUS_END_OF_FILE
    // 0xC000014B - STATUS_PIPE_BROKEN
    // 0x00000103 - STATUS_PENDING
    match r {
        0xC000014B | 0xC0000011 => Ok(n),
        0x103 => Err(Win32Error::IoPending),
        0 => Ok(n),
        _ => Err(winapi::nt_error(r)),
    }
}
pub fn NtQueryDirectoryFile(h: impl AsHandle, buf: &mut [u8], class: u32, single: bool, restart: bool, glob: impl MaybeString) -> Win32Result<usize> {
    winapi::init_ntdll();
    let mut i = IoStatusBlock::default();
    let r = unsafe {
        let g: WChars = glob.into_string().into();
        winapi::syscall!(
            *ntdll::NtQueryDirectoryFile,
            extern "stdcall" fn(Handle, usize, usize, usize, *mut IoStatusBlock, *mut u8, u32, u32, u32, *const u16, u32) -> u32,
            h.as_handle(),
            0,
            0,
            0,
            &mut i,
            buf.as_mut_ptr(),
            cmp::min(buf.len(), 0xFFFFFFFF) as u32,
            class,
            if single { 1 } else { 0 },
            g.as_null_or_ptr(),
            if restart { 1 } else { 0 }
        )
    };
    match r {
        0x80000006 => Ok(0), // 0x80000006 - ERROR_NO_MORE_FILES
        0 => Ok(i.info),
        _ => Err(winapi::nt_error(r)),
    }
}
pub fn NtCreateFile(file: impl AsRef<str>, root: Handle, access: u32, sa: SecAttrs, attrs: u32, share: u32, disposition: u32, flags: u32) -> Win32Result<OwnedHandle> {
    winapi::init_ntdll();
    let mut h = Handle::default();
    let mut i = IoStatusBlock::default();
    let r = unsafe {
        let n = if root.0 > 0 {
            UnicodeStr::from(file.as_ref())
        } else {
            UnicodeStr::from(winapi::normalize_path_to_nt(file))
        };
        // 0x1000000 - FILE_FLAG_POSIX_SEMANTICS
        // 0x0000020 - OBJ_EXCLUSIVE
        // 0x0000040 - OBJ_CASE_INSENSITIVE
        let o = ObjectAttributes::root(
            Some(&n.value),
            root,
            false,
            if flags & 0x1000000 != 0 { 0 } else { 0x40 },
            // NOTE(dij): This 'OBJ_EXCLUSIVE' flag seems to cause problems?
            //            looking at the calls Windows makes seems not to use it
            //            either? Weird. When used throws "invalid argument".
            // | if share == 0 { 0x20 } else { 0 },
            sa,
            None,
        );
        winapi::syscall!(
            *ntdll::NtCreateFile,
            extern "stdcall" fn(*mut Handle, u32, *const ObjectAttributes, *mut IoStatusBlock, *mut u32, u32, u32, u32, u32, *mut u8, u32) -> u32,
            &mut h,
            access,
            &o,
            &mut i,
            ptr::null_mut(),
            attrs,
            share,
            disposition,
            flags & !0x1000000, // Remove FILE_FLAG_POSIX_SEMANTICS
            ptr::null_mut(),
            0
        )
    };
    match r {
        _ if i.info == 0x5 => Err(Win32Error::FileNotFound),
        0 => Ok(h.into()),
        _ => Err(winapi::nt_error(r)),
    }
}

/////////////////////////////////////
// Device/IO Control Functions
/////////////////////////////////////
pub fn NtDeviceIoControlFile<T, O>(h: impl AsHandle, code: u32, olp: OverlappedIo, input: *const T, in_len: u32, out: *mut O, out_len: u32) -> Win32Result<usize> {
    winapi::init_ntdll();
    let (r, n) = unsafe {
        let mut t = Overlapped::default();
        let (w, x) = olp.map_or_else(
            || (true, &mut t),
            |v| {
                v.internal_high = 0;
                (false, v)
            },
        );
        let r = winapi::syscall!(
            *ntdll::NtDeviceIoControlFile,
            extern "stdcall" fn(Handle, Handle, usize, *mut Overlapped, *mut Overlapped, u32, *const T, u32, *mut O, u32) -> u32,
            h.as_handle(),
            x.event,
            0,
            if x.event.0 & 1 == 0 { x } else { ptr::null_mut() },
            x,
            code,
            input,
            in_len,
            out,
            out_len
        );
        // 0x103 - STATUS_PENDING
        let z = if r == 0x103 && w {
            ignore_error!(WaitForSingleObject(h, -1, false));
            x.internal as u32
        } else {
            r
        };
        (z, x.internal_high as usize)
    };
    match r {
        0x103 => Err(Win32Error::IoPending), // 0x103 - STATUS_PENDING
        0 => Ok(n),
        _ => Err(winapi::nt_error(r)),
    }
}

pub(crate) unsafe fn ldl_load_library(name: &UnicodeString) -> Win32Result<Handle> {
    let mut h = Handle::default();
    let d = winapi::system_dir();
    let e = 0u32;
    let r = winapi::syscall!(
        *ntdll::LdrLoadDll,
        extern "stdcall" fn(*const u16, *const u32, *const UnicodeString, *mut Handle) -> u32,
        d.as_ptr(),
        &e,
        name,
        &mut h
    );
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(h)
    }
}
pub(crate) unsafe fn ldl_load_address(h: Handle, ordinal: u16, name: &AnsiString) -> Win32Result<usize> {
    let mut f = 0usize;
    let r = winapi::syscall!(
        *ntdll::LdrGetProcedureAddress,
        extern "stdcall" fn(Handle, *const AnsiString, u16, *mut usize) -> u32,
        h,
        name,
        ordinal,
        &mut f
    );
    if r > 0 {
        Err(winapi::nt_error(r))
    } else {
        Ok(f)
    }
}

#[inline]
pub(crate) fn close_handle(h: Handle) {
    if h.is_invalid() {
        bugtrack!("winapi::CloseHandle(): CloseHandle on an invalid Handle!");
        return;
    }
    winapi::init_ntdll();
    let r = unsafe { winapi::syscall!(*ntdll::NtClose, extern "stdcall" fn(Handle) -> u32, h) };
    if r > 0 {
        bugtrack!("winapi::CloseHandle(): CloseHandle 0x{h:X} resulted in an error 0x{r:X}!");
    }
}
pub(crate) fn wait_for_multiple_objects(h: &[usize], size: usize, all: bool, microseconds: i32, alertable: bool) -> Win32Result<u32> {
    // NOTE(dij): This function does NOT do the stdlib checks for Handles.
    //            Use the 'WaitForMultipleAsHandles' function for that.
    if h.len() > 64 || size == 0 || h.len() < size {
        return Err(Win32Error::InvalidArgument);
    }
    winapi::init_ntdll();
    let r = unsafe {
        let t = (microseconds as i64).wrapping_mul(-10);
        winapi::syscall!(
            *ntdll::NtWaitForMultipleObjects,
            extern "stdcall" fn(u32, *const usize, u32, u32, *const i64) -> u32,
            size as u32,
            h.as_ptr(),
            if all { 0 } else { 1 },
            if alertable { 1 } else { 0 },
            if microseconds == -1 { ptr::null() } else { &t }
        )
    };
    // STATUS_WAIT_0 .. STATUS_WAIT_63
    if r == 0 || r < 64 {
        return Ok(r);
    }
    // 0x0C0 - STATUS_USER_APC
    // 0x101 - STATUS_ALERTED
    // 0x102 - STATUS_TIMEOUT
    match r {
        0xC0 | 0x101 | 0x102 => Ok(r),
        _ => Err(winapi::nt_error(r)),
    }
}

unsafe fn write_hex_padded(buf: &mut String, pad: usize, r: u32) {
    let s = buf.len();
    let m = buf.as_mut_vec();
    let mut b = [0u8; 11];
    let mut i = 10usize;
    m.resize(s + 10, 0);
    loop {
        let n = (r >> (4 * (10 - i))) as usize;
        let v = HEXTABLE[n & 0xF];
        b[i] = if v < b'A' { v } else { v + 32 };
        i -= 1;
        if n <= 0xF {
            break;
        }
    }
    while (10 - i) < pad {
        b[i] = b'0';
        i -= 1;
    }
    let r = util::copy(&mut m[s..], &b[i + 1..]);
    m.truncate(s + r);
}
