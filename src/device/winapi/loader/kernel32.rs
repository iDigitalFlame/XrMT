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
#![allow(non_snake_case, non_upper_case_globals)]

use crate::device::winapi::loader::{Function, Loader};

// Only in kernel32.dll and only needed for < Win8 to duplicate Stdin.
pub(crate) static DuplicateConsoleHandle: Function = Function::new();

// These could be in kernelbase.dll OR kernel32.dll depending on the OS version.
pub(crate) static CopyFileEx: Function = Function::new();
pub(crate) static MoveFileEx: Function = Function::new();
pub(crate) static GetTempPath: Function = Function::new();
// TODO(dij) ^

pub(crate) static DebugBreak: Function = Function::new();
pub(crate) static WriteConsoleA: Function = Function::new();

#[cfg(target_arch = "x86_64")]
pub(crate) static OutputDebugStringA: Function = Function::new();

pub(crate) static GetVersionExW: Function = Function::new();

pub(crate) static LocalFree: Function = Function::new();
pub(crate) static FormatMessage: Function = Function::new();
pub(crate) static GetComputerNameEx: Function = Function::new();

pub(crate) static K32EnumDeviceDrivers: Function = Function::new();
pub(crate) static K32GetModuleInformation: Function = Function::new();
pub(crate) static K32GetDeviceDriverFileName: Function = Function::new();

pub(crate) static CreateProcess: Function = Function::new();
pub(crate) static CreateRemoteThread: Function = Function::new();

#[cfg(feature = "snap")]
mod snap {
    use crate::device::winapi::loader::Function;

    pub(crate) static Thread32Next: Function = Function::new();
    pub(crate) static Thread32First: Function = Function::new();
    pub(crate) static Process32Next: Function = Function::new();
    pub(crate) static Process32First: Function = Function::new();
    pub(crate) static CreateToolhelp32Snapshot: Function = Function::new();
}

#[cfg(feature = "snap")]
pub(crate) use self::snap::*;

pub(super) static KERNEL32: Loader = Loader::new(|kernel32| {
    // Load known kernel32.dll first.
    kernel32.proc(&DuplicateConsoleHandle, 0x4F7A9C2D);

    kernel32.proc(&CreateProcess, 0x19C69863);

    kernel32.proc(&K32EnumDeviceDrivers, 0x779D5EFF);
    kernel32.proc(&K32GetModuleInformation, 0xFD5B63D5);
    kernel32.proc(&K32GetDeviceDriverFileName, 0x9EF6FF6D);

    // These should already be loaded in "kernelbase.dll", but we should check
    // them just incase older Operating Systems don't have "kernelbase.dll".
    kernel32.proc(&CopyFileEx, 0x2A7420AA);
    kernel32.proc(&MoveFileEx, 0x913F24C2);
    kernel32.proc(&GetTempPath, 0x1F730D3);

    kernel32.proc(&DebugBreak, 0x7F7E4A57);
    kernel32.proc(&WriteConsoleA, 0x45550ADA);

    #[cfg(target_arch = "x86_64")]
    kernel32.proc(&OutputDebugStringA, 0x58448029);

    kernel32.proc(&GetVersionExW, 0xADC522A9);

    kernel32.proc(&LocalFree, 0x3A5DD394);
    kernel32.proc(&FormatMessage, 0x8233A148);
    kernel32.proc(&GetComputerNameEx, 0x87710E5);

    kernel32.proc(&CreateRemoteThread, 0xEE34539B);

    // Functions loaded when using snap.
    #[cfg(feature = "snap")]
    {
        kernel32.proc(&Thread32Next, 0x9B4B1895);
        kernel32.proc(&Thread32First, 0xC5311BC8);
        kernel32.proc(&Process32Next, 0x80132847);
        kernel32.proc(&Process32First, 0xD4C414BE);
        kernel32.proc(&CreateToolhelp32Snapshot, 0xBAA64095);
    }
});
pub(super) static KERNELBASE: Loader = Loader::new(|kernelbase| {
    // Load known kernelbase.dll first.
    kernelbase.proc(&CopyFileEx, 0x2A7420AA);
    kernelbase.proc(&MoveFileEx, 0x913F24C2);
    kernelbase.proc(&GetTempPath, 0x1F730D3);

    kernelbase.proc(&DebugBreak, 0x7F7E4A57);
    kernelbase.proc(&WriteConsoleA, 0x45550ADA);

    #[cfg(target_arch = "x86_64")]
    kernelbase.proc(&OutputDebugStringA, 0x58448029);

    kernelbase.proc(&GetVersionExW, 0xADC522A9);

    kernelbase.proc(&LocalFree, 0x3A5DD394);
    kernelbase.proc(&FormatMessage, 0x8233A148);
    kernelbase.proc(&GetComputerNameEx, 0x87710E5);

    kernelbase.proc(&CreateProcess, 0x19C69863);

    // These may be here in newer versions, but if not will be loaded in
    // "advapi32.dll".
    // kernelbase.proc(&IsWellKnownSID, 0xF855936A);
});
