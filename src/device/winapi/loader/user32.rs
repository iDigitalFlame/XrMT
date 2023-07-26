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
#![allow(non_snake_case, non_upper_case_globals)]

use crate::device::winapi::loader::{Function, Loader};

pub(crate) static GetDC: Function = Function::new();
pub(crate) static ReleaseDC: Function = Function::new();

pub(crate) static SetFocus: Function = Function::new();
pub(crate) static MessageBox: Function = Function::new();
pub(crate) static SendNotifyMessage: Function = Function::new();

pub(crate) static SendInput: Function = Function::new();
pub(crate) static BlockInput: Function = Function::new();

pub(crate) static ShowWindow: Function = Function::new();
pub(crate) static EnableWindow: Function = Function::new();
pub(crate) static SetWindowPos: Function = Function::new();
pub(crate) static GetWindowText: Function = Function::new();
pub(crate) static GetWindowInfo: Function = Function::new();
pub(crate) static GetWindowLongW: Function = Function::new();
pub(crate) static SetWindowLongW: Function = Function::new();
pub(crate) static GetDesktopWindow: Function = Function::new();
pub(crate) static GetWindowTextLength: Function = Function::new();
pub(crate) static SetForegroundWindow: Function = Function::new();
pub(crate) static SetLayeredWindowAttributes: Function = Function::new();

pub(crate) static GetMonitorInfo: Function = Function::new();
pub(crate) static SystemParametersInfo: Function = Function::new();

pub(crate) static EnumWindows: Function = Function::new();
pub(crate) static EnumDisplayMonitors: Function = Function::new();
pub(crate) static EnumDisplaySettings: Function = Function::new();

pub(super) static DLL: Loader = Loader::new(|user32| {
    user32.proc(&GetDC, 0xC9AB9064);
    user32.proc(&ReleaseDC, 0x934A6B3);

    user32.proc(&SetFocus, 0x1AF3F781);
    user32.proc(&MessageBox, 0x1C4E3F6C);
    user32.proc(&SendNotifyMessage, 0xDEBEDBC0);

    user32.proc(&SendInput, 0xB22A0065);
    user32.proc(&BlockInput, 0x1359E3BC);

    user32.proc(&ShowWindow, 0xB408886A);
    user32.proc(&EnableWindow, 0x64DED01C);
    user32.proc(&SetWindowPos, 0x57C8D93B);
    user32.proc(&GetWindowText, 0x123362FD);
    user32.proc(&GetWindowInfo, 0x971B836B);
    user32.proc(&GetWindowLongW, 0x31A5F5B0);
    user32.proc(&SetWindowLongW, 0x8BD0F82C);
    user32.proc(&GetDesktopWindow, 0x1921BE95);
    user32.proc(&GetWindowTextLength, 0x85381939);
    user32.proc(&SetForegroundWindow, 0x52EF9094);
    user32.proc(&SetLayeredWindowAttributes, 0x950A5A2E);

    user32.proc(&GetMonitorInfo, 0x9B68BE4A);
    user32.proc(&SystemParametersInfo, 0xF1855EA9);

    user32.proc(&EnumWindows, 0x9A29AD49);
    user32.proc(&EnumDisplayMonitors, 0x6FA69AB9);
    user32.proc(&EnumDisplaySettings, 0x83B28A2E);
});
