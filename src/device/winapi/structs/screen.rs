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

use crate::data::blob::Blob;
use crate::data::str::Fiber;
use crate::data::{Readable, Reader, Writable, Writer};
use crate::device::winapi::{self, user32};
use crate::io;
use crate::prelude::*;

#[repr(u8)]
pub enum WindowState {
    Hide             = 0x0u8,
    Normal           = 0x1u8,
    Minimized        = 0x2u8,
    Maximized        = 0x3u8,
    NoActive         = 0x4u8,
    Show             = 0x5u8,
    Minimize         = 0x6u8,
    MinimizeNoActive = 0x7u8,
    ShowNoActive     = 0x8u8,
    Restore          = 0x9u8,
    Default          = 0xAu8,
    MinimizeForce    = 0xBu8,
}

#[repr(C)]
pub struct Point {
    pub x: u32,
    pub y: u32,
}
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}
pub struct Window {
    pub name:   Fiber,
    pub flags:  u8,
    pub pos_x:  i32,
    pub pos_y:  i32,
    pub width:  i32,
    pub height: i32,
    pub handle: usize,
}
#[repr(C)]
pub struct BitmapInfo {
    pub size:        u32,
    pub width:       i32,
    pub height:      i32,
    pub planes:      u16,
    pub bit_count:   u16,
    pub compression: u32,
    pub image_size:  u32,
    pad1:            [u32; 4],
    pad2:            usize,
}

#[repr(C)]
pub(crate) struct Rect {
    pub left:   i32,
    pub top:    i32,
    pub right:  i32,
    pub bottom: i32,
}

#[repr(C)]
struct WindowInfo {
    size:     u32,
    window:   Rect,
    client:   Rect,
    style:    u32,
    ex_style: u32,
    status:   u32,
    pad1:     u32,
    pad2:     u32,
    pad3:     u16,
    pad4:     u16,
}

impl Bounds {
    #[inline]
    pub fn new(x1: u32, y1: u32, x2: u32, y2: u32) -> Bounds {
        match 1 {
            _ if x1 > x2 && y1 > y2 => Bounds {
                min: Point { x: x2, y: y2 },
                max: Point { x: x1, y: y1 },
            },
            _ if x1 > x2 => Bounds {
                min: Point { x: x2, y: y1 },
                max: Point { x: x1, y: y2 },
            },
            _ if y1 > x2 => Bounds {
                min: Point { x: x1, y: y1 },
                max: Point { x: x2, y: y1 },
            },
            _ => Bounds {
                min: Point { x: x1, y: y1 },
                max: Point { x: x2, y: y2 },
            },
        }
    }
}
impl Window {
    #[inline]
    pub fn new() -> Window {
        Window {
            name:   Fiber::new(),
            flags:  0u8,
            pos_x:  0i32,
            pos_y:  0i32,
            width:  0i32,
            height: 0i32,
            handle: 0usize,
        }
    }

    #[inline]
    pub fn is_minimized(&self) -> bool {
        self.flags & 0x2 != 0
    }
    #[inline]
    pub fn is_maximized(&self) -> bool {
        self.flags & 0x1 != 0
    }
}
impl BitmapInfo {
    #[inline]
    pub fn new(width: i32, height: i32) -> BitmapInfo {
        BitmapInfo {
            pad2: 0usize,
            pad1: [0u32; 4],
            size: 0x28u32,
            planes: 1u16,
            bit_count: 32u16,
            image_size: 0u32,
            compression: 0u32,
            width,
            height,
        }
    }
}

impl Copy for Bounds {}
impl Clone for Bounds {
    #[inline]
    fn clone(&self) -> Bounds {
        Bounds { min: self.min, max: self.max }
    }
}
impl From<Rect> for Bounds {
    #[inline]
    fn from(v: Rect) -> Bounds {
        Bounds::new(v.left as u32, v.top as u32, v.right as u32, v.bottom as u32)
    }
}

impl Default for Window {
    #[inline]
    fn default() -> Window {
        Window::new()
    }
}
impl Writable for Window {
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        w.write_u64(self.handle as u64)?;
        w.write_fiber(&self.name)?;
        w.write_u8(self.flags)?;
        w.write_i32(self.pos_x)?;
        w.write_i32(self.pos_y)?;
        w.write_i32(self.width)?;
        w.write_i32(self.height)
    }
}
impl Readable for Window {
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        self.handle = r.read_u64()? as usize;
        r.read_into_fiber(&mut self.name)?;
        r.read_into_u8(&mut self.flags)?;
        r.read_into_i32(&mut self.pos_x)?;
        r.read_into_i32(&mut self.pos_y)?;
        r.read_into_i32(&mut self.width)?;
        r.read_into_i32(&mut self.height)
    }
}

impl Copy for Point {}
impl Clone for Point {
    #[inline]
    fn clone(&self) -> Point {
        Point { x: self.x, y: self.y }
    }
}
impl Default for Point {
    #[inline]
    fn default() -> Point {
        Point { x: 0u32, y: 0u32 }
    }
}

impl Copy for Rect {}
impl Clone for Rect {
    #[inline]
    fn clone(&self) -> Rect {
        Rect {
            top:    self.top,
            left:   self.left,
            right:  self.right,
            bottom: self.bottom,
        }
    }
}
impl Default for Rect {
    #[inline]
    fn default() -> Rect {
        Rect {
            top:    0i32,
            left:   0i32,
            right:  0i32,
            bottom: 0i32,
        }
    }
}

impl Default for WindowInfo {
    #[inline]
    fn default() -> WindowInfo {
        WindowInfo {
            pad1:     0u32,
            pad2:     0u32,
            pad3:     0u16,
            pad4:     0u16,
            size:     0x32u32,
            style:    0u32,
            window:   Rect::default(),
            client:   Rect::default(),
            status:   0u32,
            ex_style: 0u32,
        }
    }
}

impl Eq for WindowState {}
impl Copy for WindowState {}
impl Clone for WindowState {
    #[inline]
    fn clone(&self) -> WindowState {
        *self
    }
}
impl From<u8> for WindowState {
    #[inline]
    fn from(v: u8) -> WindowState {
        match v {
            0x0 => WindowState::Hide,
            0x1 => WindowState::Normal,
            0x2 => WindowState::Minimized,
            0x3 => WindowState::Maximized,
            0x4 => WindowState::NoActive,
            0x5 => WindowState::Show,
            0x6 => WindowState::Minimize,
            0x7 => WindowState::MinimizeNoActive,
            0x8 => WindowState::ShowNoActive,
            0x9 => WindowState::Restore,
            0xA => WindowState::Default,
            0xB => WindowState::MinimizeForce,
            _ => WindowState::Default,
        }
    }
}
impl PartialEq for WindowState {
    #[inline]
    fn eq(&self, other: &WindowState) -> bool {
        *self as u8 == *other as u8
    }
}
impl PartialEq<u8> for WindowState {
    #[inline]
    fn eq(&self, other: &u8) -> bool {
        *self as u8 == *other
    }
}

pub(crate) unsafe extern "stdcall" fn _enum_window(h: usize, out: *mut Vec<Window>) -> u32 {
    let b = some_or_return!(out.as_mut(), 0);
    let n = winapi::syscall!(
        *user32::GetWindowTextLength,
        extern "stdcall" fn(usize) -> u32,
        h
    );
    if n == 0 {
        return 1;
    }
    let mut i = WindowInfo::default();
    let r = winapi::syscall!(
        *user32::GetWindowInfo,
        extern "stdcall" fn(usize, *mut WindowInfo) -> u32,
        h,
        &mut i
    );
    if r == 0 {
        return 1;
    }
    // 0x80000000 - WS_POPUP
    // 0x20000000 - WS_MINIMIZE
    // 0x10000000 - WS_VISIBLE
    // 0x00000400 - WS_EX_CONTEXTHELP
    // 0x00200000 - WS_EX_NOREDIRECTIONBITMAP
    // Removes popup windows that were created hidden or minimized. Most of them
    // are built-in system dialogs.
    if (i.style & 0x80000000 != 0 && i.style & 0x10000000 == 0) || i.style & 0x10000000 == 0 || i.style & 0x00000400 != 0 || i.ex_style & 0x200000 != 0 {
        return 1;
    }
    let mut d: Blob<u16, 128> = Blob::with_size(n as usize + 1);
    let c = winapi::syscall!(
        *user32::GetWindowText,
        extern "stdcall" fn(usize, *mut u16, u32) -> u32,
        h,
        d.as_mut_ptr(),
        n + 1
    );
    if c == 0 {
        return 1;
    }
    let mut t = 0u8;
    if i.style & 0x1000000 == 0 {
        t |= 0x1
    }
    if i.style & 0x20000000 == 0 {
        t |= 0x2
    }
    if i.style & 0x1E000000 == 0x1E000000 {
        t |= 0x80
    }
    b.push(Window {
        name:   winapi::utf16_to_fiber(&d[0..c as usize], alloc::alloc::Global),
        flags:  t,
        pos_x:  i.window.left,
        pos_y:  i.window.top,
        width:  i.window.right.wrapping_sub(i.window.left),
        height: i.window.bottom.wrapping_sub(i.window.top),
        handle: h,
    });
    1
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Formatter};

    use crate::device::winapi::{Bounds, Point, Window};
    use crate::prelude::*;

    impl Debug for Point {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("Point").field("x", &self.x).field("y", &self.y).finish()
        }
    }
    impl Debug for Bounds {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("Bounds")
                .field("min", &self.min)
                .field("max", &self.max)
                .finish()
        }
    }
    impl Debug for Window {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("Window")
                .field("name", &self.name)
                .field("flags", &self.flags)
                .field("pos_x", &self.pos_x)
                .field("pos_y", &self.pos_y)
                .field("width", &self.width)
                .field("height", &self.height)
                .field("handle", &self.handle)
                .finish()
        }
    }
}
