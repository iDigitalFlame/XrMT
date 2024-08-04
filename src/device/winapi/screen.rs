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

use core::ptr;

use crate::device::winapi::{self, user32, BitmapInfo, Bounds, Handle, Point, Rect, Region, Win32Error, Win32Result};
use crate::ignore_error;
use crate::io::Write;
use crate::prelude::*;

#[repr(C)]
struct Info {
    data:  Rect,
    index: u32,
    count: u32,
}
#[repr(C)]
struct Mode {
    pad1:     [u8; 68],
    size:     u16,
    pad2:     [u8; 6],
    position: Point,
    pad3:     [u8; 86],
    width:    u32,
    height:   u32,
    pad4:     [u8; 40],
}
struct Capture {
    dc:      Handle,
    comp:    Handle,
    bitmap:  Handle,
    desktop: Handle,
}
#[repr(C)]
struct MonitorInfo {
    size:    u32,
    monitor: Rect,
    work:    Rect,
    flags:   u32,
    name:    [u16; 32],
}

impl Info {
    #[inline]
    fn new(index: u32) -> Info {
        Info {
            data: Rect::default(),
            count: 0u32,
            index,
        }
    }
}
impl Capture {
    #[inline]
    fn new(width: u32, height: u32) -> Win32Result<Capture> {
        let w = winapi::GetDesktopWindow()?;
        let d = winapi::GetDC(w)?;
        let c = winapi::CreateCompatibleDC(d).map_err(|e| {
            ignore_error!(winapi::ReleaseDC(w, d));
            e
        })?;
        let r = match winapi::CreateCompatibleBitmap(d, width, height) {
            Ok(b) => {
                return Ok(Capture {
                    dc:      d,
                    comp:    c,
                    bitmap:  b,
                    desktop: w,
                });
            },
            Err(e) => e,
        };
        ignore_error!(winapi::DeleteDC(c));
        ignore_error!(winapi::ReleaseDC(w, d));
        Err(r)
    }

    fn start(&mut self, x: u32, y: u32, width: u32, height: u32, w: &mut impl Write) -> Win32Result<()> {
        let n = (((width as usize * 32) + 31) / 32) * 4 * height as usize;
        // 0x1002 - MEM_COMMIT? | HEAP_GROWABLE
        let a = winapi::HeapCreate(0x1002, n, n)?;
        let mut h = winapi::HeapAlloc(a, n, false).map_err(|e| {
            ignore_error!(winapi::HeapDestroy(a));
            e
        })?;
        let o = winapi::SelectObject(self.comp, self.bitmap).map_err(|e| {
            ignore_error!(winapi::HeapFree(a, h.as_ptr()));
            e
        })?;
        let r = self.select(&mut h, x, y, width, height, w);
        ignore_error!(winapi::SelectObject(self.comp, o));
        ignore_error!(winapi::HeapFree(a, h.as_ptr()));
        ignore_error!(winapi::HeapDestroy(a));
        r
    }
    #[inline]
    fn select(&mut self, b: &mut Region, x: u32, y: u32, width: u32, height: u32, w: &mut impl Write) -> Win32Result<()> {
        /*

        Const BLACKNESS = &H42
        ' Const CAPTUREBLT = ???
        Const DSTINVERT = &H550009
        Const MERGECOPY = &HC000CA
        Const MERGEPAINT = &HBB0226
        ' Const NOMIRRORBITMAP = ???
        Const NOTSRCCOPY = &H330008
        Const NOTSRCERASE = &H1100A6
        Const PATCOPY = &HF00021
        Const PATINVERT = &H5A0049
        Const PATPAINT = &HFB0A09
        Const SRCAND = &H8800C6
        Const SRCCOPY = &HCC0020
        Const SRCERASE = &H440328
        Const SRCINVERT = &H660046
        Const SRCPAINT = &HEE0086
        Const WHITENESS = &HFF0062

                 */
        winapi::BitBlt(self.comp, 0, 0, width, height, self.dc, x, y, 0xCC0020)?;
        let mut i = BitmapInfo::new(width as i32, height as i32 * -1);
        winapi::GetDIBits(self.dc, self.bitmap, 0, height, b.as_mut_ptr(), &mut i, 0)?;
        {
            // TODO(dij): Image encoding.
            let _ = w;
        }
        Ok(())
    }

    #[inline]
    fn screen_shot(x: u32, y: u32, width: u32, height: u32, w: &mut impl Write) -> Win32Result<()> {
        Capture::new(width, height)?.start(x, y, width, height, w)
    }
}

impl Drop for Capture {
    #[inline]
    fn drop(&mut self) {
        if !self.bitmap.is_invalid() {
            ignore_error!(winapi::DeleteObject(self.bitmap));
        }
        if !self.comp.is_invalid() {
            ignore_error!(winapi::DeleteDC(self.comp));
        }
        if !self.dc.is_invalid() && !self.desktop.is_invalid() {
            ignore_error!(winapi::ReleaseDC(self.desktop, self.dc));
        }
    }
}

impl Default for Mode {
    #[inline]
    fn default() -> Mode {
        Mode {
            pad1:     [0u8; 68],
            pad2:     [0u8; 6],
            pad3:     [0u8; 86],
            pad4:     [0u8; 40],
            size:     0xDC,
            width:    0u32,
            height:   0u32,
            position: Point::default(),
        }
    }
}
impl Default for MonitorInfo {
    #[inline]
    fn default() -> MonitorInfo {
        MonitorInfo {
            name:    [0u16; 32],
            size:    0x68u32,
            flags:   0u32,
            work:    Rect::default(),
            monitor: Rect::default(),
        }
    }
}

pub fn active_displays() -> Win32Result<u32> {
    winapi::init_user32();
    let mut c = Box::new(0u32);
    let r = unsafe {
        winapi::syscall!(
            *user32::EnumDisplayMonitors,
            extern "stdcall" fn(usize, *const Rect, unsafe extern "stdcall" fn(usize, usize, *const Rect, *mut Box<u32>) -> u32, *mut Box<u32>) -> u32,
            0,
            ptr::null(),
            _enum_count,
            &mut c
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(*c)
    }
}
pub fn display_bounds(index: u32) -> Win32Result<Bounds> {
    winapi::init_user32();
    let mut c = Box::new(Info::new(index));
    let r = unsafe {
        winapi::syscall!(
            *user32::EnumDisplayMonitors,
            extern "stdcall" fn(usize, *const Rect, unsafe extern "stdcall" fn(usize, usize, *const Rect, *mut Box<Info>) -> u32, *mut Box<Info>) -> u32,
            0,
            ptr::null(),
            _enum_bounds,
            &mut c
        )
    };
    // The only error here is if a display number that is unknown is specified.
    // This will cause the callback to exit on a '1' instead of a '0', which is
    // given when we short-circut the enum.
    if r == 1 {
        Err(Win32Error::FileNotFound)
    } else {
        Ok(c.data.into())
    }
}

fn monitor_size(h: usize) -> Option<Rect> {
    let mut i = MonitorInfo::default();
    let r = unsafe {
        winapi::syscall!(
            *user32::GetMonitorInfo,
            extern "stdcall" fn(usize, *mut MonitorInfo) -> u32,
            h,
            &mut i
        )
    };
    if r == 0 {
        return None;
    }
    let mut m = Mode::default();
    let k = unsafe {
        winapi::syscall!(
            *user32::EnumDisplaySettings,
            extern "stdcall" fn(*const u16, u32, *mut Mode) -> u32,
            i.name.as_ptr(),
            0xFFFFFFFF,
            &mut m
        )
    };
    if k == 0 {
        return None;
    }
    Some(Rect {
        top:    m.position.y as i32,
        left:   m.position.x as i32,
        right:  (m.position.x + m.width) as i32,
        bottom: (m.position.y + m.height) as i32,
    })
}

unsafe extern "stdcall" fn _enum_count(_h: usize, _c: usize, _r: *const Rect, p: *mut Box<u32>) -> u32 {
    let n = some_or_return!(p.as_mut(), 0);
    *(*n) = *(*n) + 1;
    1
}
unsafe extern "stdcall" fn _enum_bounds(h: usize, _c: usize, r: *const Rect, p: *mut Box<Info>) -> u32 {
    let d = some_or_return!(p.as_mut(), 0);
    if d.count != d.index {
        d.count += 1;
        1
    } else {
        d.data = monitor_size(h).unwrap_or_else(|| r.as_ref().map(|v| *v).unwrap_or_else(Rect::default));
        0
    }
}

#[inline]
pub fn screen_shot(x: u32, y: u32, width: u32, height: u32, w: &mut impl Write) -> Win32Result<()> {
    Capture::screen_shot(x, y, width, height, w)
}
