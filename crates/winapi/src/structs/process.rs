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

extern crate xrmt_data;

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::clone::Clone;
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::{AsRef, From};
use core::default::Default;
use core::hint::unreachable_unchecked;
use core::iter::{IntoIterator, Iterator};
use core::marker::{Copy, PhantomData};
use core::matches;
use core::mem::{size_of, transmute, MaybeUninit};
use core::ops::{Deref, DerefMut, Drop, FnMut};
use core::option::Option::{self, None, Some};
use core::ptr::null;
use core::slice::from_raw_parts;

use xrmt_data::text::{str_to_u16_unchecked, str_to_utf16_vec, utf16_to_string, utf8_to_lossy_u16};

use crate::functions::{process_cmdline, process_user, system_root, GetEnvironment, OpenProcess, OpenThread};
use crate::structs::{CharLike, ClientID, EnvironmentBlock, Handle, Module, OwnedHandle, StringLikeU16, SysTime, UnicodeString, WChar, WCharLike, WCharPtr, WCharSlice};
use crate::{Win32Result, PTR_SIZE};

const SYSTEM: [u8; 4] = [0x5C, 0x3F, 0x3F, 0x5C];
const SYSTEM_ROOT: [u16; 11] = [0x53, 0x59, 0x53, 0x54, 0x45, 0x4D, 0x52, 0x4F, 0x4F, 0x54, 0x3D]; // SYSTEMROOT=

#[repr(u8)]
pub enum Protection {
    None           = 0,
    Authenticode   = 1,
    CodeGeneration = 2,
    Antimalware    = 3,
    Lsa            = 4,
    Windows        = 5,
    WinTrusted     = 6,
    WinSystem      = 7,
    StoreApp       = 8,
}

pub enum StartInfo<'a> {
    Basic(&'a StartupInfo<'a>),
    Extended(&'a StartupInfoEx<'a>),
}

#[repr(C)]
pub struct HandleEntry {
    pub pid:         u16,
    pub backtrace:   u16,
    pub object_type: u8,
    pub attributes:  u8,
    pub value:       u16,
    pub object:      usize,
    pub access:      u32,
}
#[repr(C)]
pub struct ProcessInfo {
    pub process:    OwnedHandle,
    pub thread:     OwnedHandle,
    pub process_id: u32,
    pub thread_id:  u32,
}
#[repr(C)]
pub struct ModuleEntry {
    pub section:     usize,
    pub base_mapped: usize,
    pub base_image:  usize,
    pub image_size:  u32,
    pub flags:       u32,
    pub order_load:  u16,
    pub order_init:  u16,
    pub load_count:  u16,
    pub name_offset: u16,
    pub full_name:   [u8; 256],
}
#[repr(C)]
pub struct ThreadEntry {
    pub time_kernel:      SysTime,
    pub time_user:        SysTime,
    pub time_create:      SysTime,
    pub time_wait:        u32,
    pub start_address:    usize,
    pub client_id:        ClientID,
    pub priority:         i32,
    pub priority_base:    i32,
    pub context_switches: u32,
    pub state:            u32,
    pub wait_reason:      u32,
}
#[repr(C)]
pub struct HandleEntryEx {
    pub object:      usize,
    pub pid:         usize,
    pub value:       usize,
    pub access:      u32,
    pub backtrace:   u16,
    pub object_type: u16,
    pub attributes:  u32,
    pad:             u32,
}
#[repr(C)]
pub struct StartupInfo<'a> {
    pub size:           u32,
    pad1:               *const u16,
    pub desktop:        WCharPtr<'a>,
    pub title:          WCharPtr<'a>,
    pub pos_x:          i32,
    pub pos_y:          i32,
    pub size_x:         u32,
    pub size_y:         u32,
    pub count_chars_x:  u32,
    pub count_chars_y:  u32,
    pub fill_attribute: u32,
    pub flags:          u32,
    pub show_window:    u16,
    pad2:               u16,
    pad3:               *const u8,
    pub stdin:          Handle,
    pub stdout:         Handle,
    pub stderr:         Handle,
}
#[repr(C)]
pub struct ProcessEntry<'a> {
    pub next:                 u32,
    pub thread_count:         u32,
    pub work_set_private:     u64,
    pub hard_faults:          u32,
    pad1:                     u32,
    pub cycle_time:           u64,
    pub time_create:          SysTime,
    pub time_user:            SysTime,
    pub time_kernel:          SysTime,
    pub name:                 UnicodeString<'a>,
    pub priority:             i32,
    pub pid:                  usize,
    pub ppid:                 usize,
    pub handles:              u32,
    pub session:              u32,
    pad3:                     usize,
    pub virtual_peak_size:    usize,
    pub virtual_size:         usize,
    pub page_faults:          u32,
    pub working_set_peak:     usize,
    pub working_set:          usize,
    pub quota_peak_paged:     usize,
    pub quota_paged:          usize,
    pub quota_peak_non_paged: usize,
    pub quota_non_paged:      usize,
    pub page_file_use:        usize,
    pub page_file_peak_use:   usize,
    pub private_pages:        usize,
    pub read_ops:             u64,
    pub write_ops:            u64,
    pub other_opts:           u64,
    pub read_transfers:       u64,
    pub write_transfers:      u64,
    pub other_transfers:      u64,
    thread_entries:           [ThreadEntry; 1],
}
#[repr(C)]
pub struct ProcessThreadAttr {
    attr:  usize,
    size:  usize,
    value: *const usize,
}
#[repr(C)]
pub struct StartupInfoEx<'a> {
    pub info:  StartupInfo<'a>,
    pub attrs: &'a ProcessThreadAttrList<'a>,
}
pub struct ProcessEnvironment {
    buf:  Vec<u16>,
    root: bool,
}
#[repr(C)]
pub struct ProcessThreadAttrList<'a> {
    mask:  u32,
    size:  u32,
    count: u32,
    pad:   u32,
    unk:   usize,
    attrs: [MaybeUninit<ProcessThreadAttr>; 5],
    _p:    PhantomData<&'a ()>,
}
pub struct ProcessVariable<'a>(UnsafeCell<&'a mut Vec<u16>>);

pub trait ProcessEnvItem {
    fn write_item(&self, buf: ProcessVariable<'_>);
}

impl Protection {
    #[inline]
    pub fn is_none(&self) -> bool {
        match self {
            Protection::None => true,
            _ => false,
        }
    }
}
impl ModuleEntry {
    #[inline]
    pub fn path(&self) -> &str {
        // End will always be in bounds.
        unsafe { transmute(self.full_name.get_unchecked(0..self.end())) }
    }
    #[inline]
    pub fn filename(&self) -> &str {
        // End will always be in bounds.
        let v = self.end();
        unsafe { transmute(self.full_name.get_unchecked((self.name_offset as usize).min(v)..v)) }
    }
    #[inline]
    pub fn full_path(&self) -> String {
        self.full(&system_root())
    }
    #[inline]
    pub fn path_without_systemroot(&self) -> &str {
        if self.name_offset < 11 {
            return self.path();
        }
        if self.in_systemroot() {
            // BCE between that and the largest
            return unsafe { transmute(&self.full_name[11..(11 + self.end()).min(256)]) };
        }
        // Size is array of 256, can never be smaller.
        // Match \??\
        let v = self.end();
        if unsafe { self.full_name.get_unchecked(0..4).eq(&SYSTEM) } && v > 4 {
            // Remove NT Prefix
            unsafe { transmute(self.full_name.get_unchecked(4..v)) }
        } else {
            unsafe { transmute(self.full_name.get_unchecked(0..v)) }
        }
    }

    #[inline]
    pub(crate) fn module(&self, root: &[u16]) -> Module {
        Module {
            path:       self.full(root),
            size:       self.image_size,
            order:      self.order_load,
            flags:      self.flags,
            address:    self.base_image,
            load_count: self.load_count,
        }
    }

    #[inline]
    fn end(&self) -> usize {
        if self.name_offset > 256 {
            return 0;
        }
        unsafe { self.full_name.get_unchecked(self.name_offset as usize..) }
            .iter()
            .position(|v| *v == 0)
            .map(|v| (v + (self.name_offset as usize)).min(256))
            .unwrap_or(256)
    }
    #[inline]
    fn in_systemroot(&self) -> bool {
        // \SystemRoot\
        unsafe { *self.full_name.get_unchecked(0) == 0x5C && *self.full_name.get_unchecked(11) == 0x5C && *self.full_name.get_unchecked(1) == 0x53 && *self.full_name.get_unchecked(7) == 0x52 }
    }
    #[inline]
    fn full(&self, root: &[u16]) -> String {
        let mut n = if self.in_systemroot() {
            utf16_to_string(root)
        } else {
            String::new()
        };
        n.push_str(self.path_without_systemroot());
        n
    }
}
impl ThreadEntry {
    #[inline]
    pub fn pid(&self) -> u32 {
        self.client_id.process as u32
    }
    #[inline]
    pub fn tid(&self) -> u32 {
        self.client_id.thread as u32
    }
    #[inline]
    pub fn is_waiting(&self) -> bool {
        // 0x5 - Waiting
        self.state == 0x5
    }
    #[inline]
    pub fn is_suspended(&self) -> bool {
        // 0x5 - Waiting
        // 0x5 - Suspended
        self.state == 0x5 && self.wait_reason == 0x5
    }
    #[inline]
    pub fn handle(&self, access: u32) -> Win32Result<OwnedHandle> {
        OpenThread(access, false, self.client_id.thread as u32)
    }
}
impl StartInfo<'_> {
    #[inline]
    pub fn is_extended(&self) -> bool {
        match self {
            StartInfo::Extended(_) => true,
            _ => false,
        }
    }
    #[inline]
    pub fn as_ptr(&self) -> *const usize {
        match self {
            StartInfo::Basic(i) => *i as *const StartupInfo as *const usize,
            StartInfo::Extended(i) => *i as *const StartupInfoEx as *const usize,
        }
    }
}
impl ProcessEnvironment {
    #[inline]
    pub const fn empty() -> ProcessEnvironment {
        ProcessEnvironment { buf: Vec::new(), root: false }
    }

    #[inline]
    pub fn from_str<'a, I: Iterator<Item = &'a str>>(v: I) -> ProcessEnvironment {
        ProcessEnvironment::new(|i, o| o.write_str(i), v)
    }
    #[inline]
    pub fn from_u16<'a, I: Iterator<Item = &'a [u16]>>(v: I) -> ProcessEnvironment {
        ProcessEnvironment::new(|i, o| o.write(i), v)
    }
    #[inline]
    pub fn from_wchar<'a, I: Iterator<Item = &'a WCharLike<'a>>>(v: I) -> ProcessEnvironment {
        ProcessEnvironment::new(|i, o| o.write(i), v)
    }
    #[inline]
    pub fn new<'a, T, F: FnMut(&T, ProcessVariable<'_>), I: Iterator<Item = T>>(f: F, iter: I) -> ProcessEnvironment {
        let mut b = ProcessEnvironment { buf: Vec::new(), root: false };
        b.items(f, iter);
        b
    }

    #[inline]
    pub fn into_wchar(mut self) -> WChar {
        if !self.root {
            self.pop();
            self.buf.extend_from_slice(&SYSTEM_ROOT);
            self.buf.extend_from_slice(&system_root());
            self.buf.push(0);
            self.root = true;
        }
        // Make sure we have a properly formatted double NULL
        self.push();
        WChar::from(self.buf)
    }
    #[inline]
    pub fn as_env<'a>(&'a self) -> Option<EnvironmentBlock<'a>> {
        // Check if empty or if we're missing a NULL padding.
        if self.buf.last().is_some_and(|v| *v == 0) {
            Some(EnvironmentBlock::new(&self.buf))
        } else {
            None
        }
    }
    #[inline]
    pub fn include_system(&mut self) -> &mut ProcessEnvironment {
        // Remove trailing NULL
        self.pop();
        self.buf.extend_from_slice(&GetEnvironment());
        self.buf.push(0);
        self.root = true;
        self
    }
    #[inline]
    pub fn env<T: ProcessEnvItem>(&mut self, var: T) -> &mut ProcessEnvironment {
        self.pop();
        self.append(&mut |i: &T, b| i.write_item(b), var);
        self.push();
        self
    }
    #[inline]
    pub fn envs<T: ProcessEnvItem, I: IntoIterator<Item = T>>(&mut self, vars: I) -> &mut ProcessEnvironment {
        self.pop();
        for v in vars {
            self.append(&mut |i: &T, b| i.write_item(b), v);
        }
        // We don't need to "add_null" here as it /SHOULD/ be properly
        // formatted here, since "add" will add the NULL end here.
        self.buf.push(0);
        self
    }
    #[inline]
    pub fn item<'a, T, F: FnMut(&T, ProcessVariable<'_>)>(&'a mut self, f: &mut F, v: T) -> &'a mut ProcessEnvironment {
        self.pop();
        self.append(f, v);
        self.push();
        self
    }
    #[inline]
    pub fn items<'a, T, F: FnMut(&T, ProcessVariable<'_>), I: Iterator<Item = T>>(&'a mut self, mut f: F, iter: I) -> &'a mut ProcessEnvironment {
        self.pop();
        for i in iter {
            self.append(&mut f, i);
        }
        // We don't need to "add_null" here as it /SHOULD/ be properly
        // formatted here, since "add" will add the NULL end here.
        self.buf.push(0);
        self
    }

    #[inline]
    fn pop(&mut self) {
        match self.buf.len() {
            0 => (),
            // Clean NULL buffer.
            1 if unsafe { *self.buf.get_unchecked(0) } == 0 => self.buf.clear(),
            // Clear NULL, NULL buffer
            2 if unsafe { *self.buf.get_unchecked(0) == 0 && *self.buf.get_unchecked(1) == 0 } => self.buf.clear(),
            2 if unsafe { *self.buf.get_unchecked(1) } == 0 => (),
            // There shouldn't be a way to reach here
            1 | 2 => unsafe { unreachable_unchecked() },
            // Pop last NULL
            n if unsafe { *self.buf.get_unchecked(n - 2) == 0 && *self.buf.get_unchecked(n - 1) == 0 } => self.buf.truncate(n - 1),
            n if unsafe { *self.buf.get_unchecked(n - 1) == 0 } => (),
            // There shouldn't be a way to reach here
            _ => unsafe { unreachable_unchecked() },
        }
    }
    #[inline]
    fn push(&mut self) {
        match self.buf.len() {
            // Empty block, add terminator.
            0 => self.buf.push(0),
            // Empty block with terminator.
            1 if unsafe { *self.buf.get_unchecked(0) } == 0 => (),
            // Odd case of double NULL, pop the last null.
            2 if unsafe { *self.buf.get_unchecked(0) == 0 && *self.buf.get_unchecked(1) == 0 } => self.buf.truncate(1),
            // Single char string, terminated but no block terminator?
            2 if unsafe { *self.buf.get_unchecked(1) } == 0 => self.buf.push(0),
            // Unterminated string?
            1 | 2 => {
                self.buf.push(0); // End string,
                self.buf.push(0); // End block,
            },
            // Properly terminated string and block.
            n if unsafe { *self.buf.get_unchecked(n - 2) == 0 && *self.buf.get_unchecked(n - 1) == 0 } => (),
            // Needs block terminator
            n if unsafe { *self.buf.get_unchecked(n - 1) == 0 } => self.buf.push(0),
            // Unterminated string with no block terminator.
            _ => {
                self.buf.push(0); // End string,
                self.buf.push(0); // End block,
            },
        }
    }
    fn append<'a, T, F: FnMut(&T, ProcessVariable<'_>)>(&'a mut self, f: &mut F, v: T) {
        let n = self.buf.len();
        {
            let e = ProcessVariable(UnsafeCell::new(&mut self.buf));
            f(&v, e);
        }
        let a = self.buf.len();
        if n == a || a == 0 {
            return; // Nothing was written.
        }
        if !self.root && a.saturating_sub(n) >= 14 {
            // Check for 'SYSTEMROOT='
            self.root = unsafe { matches!(*self.buf.get_unchecked(n), 0x53 | 0x73) && matches!(*self.buf.get_unchecked(n + 6), 0x52 | 0x72) && *self.buf.get_unchecked(n + 10) == 0x3D };
        }
        // Check if it ends with a NULL
        // SAFETY: It has to be at least 1 for this to be triggered.
        let z = *unsafe { self.buf.last().unwrap_unchecked() } == 0;
        // Look for '=' and add it if it does not exist.
        let p = unsafe { self.buf.get_unchecked(n..a) }
            .iter()
            .position(|v| *v == 0x3D)
            .is_some();
        match (p, z) {
            (true, true) => (),            // All good
            (true, _) => self.buf.push(0), // Add NULL.
            (false, true) => {
                // Replace NULL with '=' and add NULL.
                unsafe { *self.buf.get_unchecked_mut(a - 1) = 0x3D };
                self.buf.push(0);
            },
            (false, _) => {
                // Add '=' and NULL.
                self.buf.push(0x3D);
                self.buf.push(0);
            },
        }
    }
}
impl ProcessVariable<'_> {
    #[inline]
    pub fn push_u8(&self, v: u8) {
        unsafe { &mut *self.0.get() }.push(v as u16);
    }
    #[inline]
    pub fn push_u16(&self, v: u16) {
        unsafe { &mut *self.0.get() }.push(v);
    }
    #[inline]
    pub fn write(&self, v: &[u16]) {
        unsafe { &mut *self.0.get() }.extend_from_slice(v);
    }
    #[inline]
    pub fn write_u8(&self, v: &[u8]) {
        utf8_to_lossy_u16(unsafe { *&mut *self.0.get() }, v);
    }
    #[inline]
    pub fn write_str(&self, v: &str) {
        str_to_utf16_vec(unsafe { *&mut *self.0.get() }, v);
    }

    #[inline]
    pub unsafe fn write_str_unchecked(&self, v: &str) {
        unsafe { str_to_u16_unchecked(*&mut *self.0.get(), v) };
    }
}
impl<'a> ProcessEntry<'a> {
    #[inline]
    pub fn pid(&self) -> u32 {
        self.pid as u32
    }
    #[inline]
    pub fn parent_pid(&self) -> u32 {
        self.ppid as u32
    }
    #[inline]
    pub fn name(&'a self) -> WCharSlice<'a> {
        self.name.as_wchar_slice()
    }
    #[inline]
    pub fn threads(&self) -> &[ThreadEntry] {
        unsafe { from_raw_parts(self.thread_entries.as_ptr(), self.thread_count as usize) }
    }
    #[inline]
    pub fn user(&self) -> Win32Result<String> {
        // 0x1400 - PROCESS_QUERY_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION
        process_user(self.handle(0x1400)?)
    }
    #[inline]
    pub fn cmdline(&self) -> Win32Result<String> {
        // 0x1410 - PROCESS_QUERY_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION |
        //           PROCESS_VM_READ
        process_cmdline(self.handle(0x1410)?)
    }
    #[inline]
    pub fn handle(&self, access: u32) -> Win32Result<OwnedHandle> {
        OpenProcess(access, false, self.pid as u32)
    }
}
impl<'a> StartupInfoEx<'a> {
    #[inline]
    pub fn new(mut i: StartupInfo<'a>, attrs: &'a ProcessThreadAttrList<'a>) -> StartupInfoEx<'a> {
        i.size += PTR_SIZE as u32;
        StartupInfoEx { info: i, attrs }
    }
}
impl<'a> ProcessThreadAttrList<'a> {
    #[inline]
    pub fn set_mitigation(&mut self, pos: usize, v: &'a u64) {
        self.set(pos, ProcessThreadAttr {
            attr:  0x20007, // PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY
            size:  8,
            value: v as *const u64 as *const usize,
        })
    }
    #[inline]
    pub fn set_handles(&mut self, pos: usize, v: &'a [Handle]) {
        self.set(pos, ProcessThreadAttr {
            attr:  0x20002, // PROC_THREAD_ATTRIBUTE_HANDLE_LIST
            size:  v.len() * PTR_SIZE,
            value: v.as_ptr() as *const usize,
        })
    }
    #[inline]
    pub fn set_parent(&mut self, pos: usize, h: &'a OwnedHandle) {
        self.set(pos, ProcessThreadAttr {
            size:  PTR_SIZE,
            attr:  0x20000, // PROC_THREAD_ATTRIBUTE_PARENT_PROCESS
            value: h.as_ref(),
        })
    }

    #[inline]
    fn set(&mut self, pos: usize, v: ProcessThreadAttr) {
        if pos > 4 {
            return;
        }
        (self.size, self.count) = (self.size + 1, self.count + 1);
        self.mask |= unsafe { 1u32.unchecked_shl(v.attr.saturating_sub(0x20000) as u32) };
        // BCE Check
        unsafe { self.attrs.get_unchecked_mut(pos).write(v) };
    }
}

impl Clone for ModuleEntry {
    #[inline]
    fn clone(&self) -> ModuleEntry {
        ModuleEntry {
            flags:       self.flags,
            section:     self.section,
            full_name:   self.full_name,
            image_size:  self.image_size,
            base_image:  self.base_image,
            order_load:  self.order_load,
            order_init:  self.order_init,
            load_count:  self.load_count,
            name_offset: self.name_offset,
            base_mapped: self.base_mapped,
        }
    }
}
impl Clone for HandleEntry {
    #[inline]
    fn clone(&self) -> HandleEntry {
        HandleEntry {
            pid:         self.pid,
            value:       self.value,
            object:      self.object,
            access:      self.access,
            backtrace:   self.backtrace,
            attributes:  self.attributes,
            object_type: self.object_type,
        }
    }
}
impl Clone for HandleEntryEx {
    #[inline]
    fn clone(&self) -> HandleEntryEx {
        HandleEntryEx {
            pad:         self.pad,
            pid:         self.pid,
            value:       self.value,
            access:      self.access,
            object:      self.object,
            backtrace:   self.backtrace,
            attributes:  self.attributes,
            object_type: self.object_type,
        }
    }
}

impl Default for ProcessInfo {
    #[inline]
    fn default() -> ProcessInfo {
        ProcessInfo {
            thread:     unsafe { OwnedHandle::empty() },
            process:    unsafe { OwnedHandle::empty() },
            thread_id:  0u32,
            process_id: 0u32,
        }
    }
}
impl<'a> Default for StartupInfo<'a> {
    #[inline]
    fn default() -> StartupInfo<'a> {
        StartupInfo {
            size:           size_of::<StartupInfo>() as u32,
            pad1:           null(),
            pad2:           0u16,
            pad3:           null(),
            pos_x:          0i32,
            pos_y:          0i32,
            flags:          0u32,
            title:          WCharPtr::null(),
            stdin:          Handle::EMPTY,
            stdout:         Handle::EMPTY,
            stderr:         Handle::EMPTY,
            size_x:         0u32,
            size_y:         0u32,
            desktop:        WCharPtr::null(),
            show_window:    0u16,
            count_chars_x:  0u32,
            count_chars_y:  0u32,
            fill_attribute: 0u32,
        }
    }
}

impl Drop for ProcessThreadAttrList<'_> {
    #[inline]
    fn drop(&mut self) {
        for i in 0..self.count as usize {
            unsafe { self.attrs.get_unchecked_mut(i).assume_init_drop() }
        }
    }
}
impl<'a> Default for ProcessThreadAttrList<'a> {
    #[inline]
    fn default() -> ProcessThreadAttrList<'a> {
        ProcessThreadAttrList {
            _p:    PhantomData,
            pad:   0u32,
            unk:   0usize,
            mask:  0u32,
            size:  0u32,
            count: 0u32,
            attrs: [
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
            ],
        }
    }
}

impl Clone for ProcessEnvironment {
    #[inline]
    fn clone(&self) -> ProcessEnvironment {
        ProcessEnvironment {
            buf:  self.buf.clone(),
            root: self.root,
        }
    }
}
impl Default for ProcessEnvironment {
    #[inline]
    fn default() -> ProcessEnvironment {
        ProcessEnvironment::empty()
    }
}

impl Deref for ProcessVariable<'_> {
    type Target = Vec<u16>;

    #[inline]
    fn deref(&self) -> &Vec<u16> {
        unsafe { &*self.0.get() }
    }
}
impl DerefMut for ProcessVariable<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Vec<u16> {
        unsafe { &mut *self.0.get() }
    }
}

impl From<ProcessEnvironment> for WChar {
    #[inline]
    fn from(v: ProcessEnvironment) -> WChar {
        v.into_wchar()
    }
}

impl Eq for Protection {}
impl Ord for Protection {
    #[inline]
    fn cmp(&self, other: &Protection) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}
impl Copy for Protection {}
impl Clone for Protection {
    #[inline]
    fn clone(&self) -> Protection {
        *self
    }
}
impl From<u8> for Protection {
    #[inline]
    fn from(v: u8) -> Protection {
        match v {
            0 => Protection::None,
            1 => Protection::Authenticode,
            2 => Protection::CodeGeneration,
            3 => Protection::Antimalware,
            4 => Protection::Lsa,
            5 => Protection::Windows,
            6 => Protection::WinTrusted,
            7 => Protection::WinSystem,
            8 => Protection::StoreApp,
            _ => Protection::None,
        }
    }
}
impl From<u32> for Protection {
    #[inline]
    fn from(v: u32) -> Protection {
        Protection::from(v as u8)
    }
}
impl PartialEq for Protection {
    #[inline]
    fn eq(&self, other: &Protection) -> bool {
        (*self as u8).eq(&(*other as u8))
    }
}
impl PartialOrd for Protection {
    #[inline]
    fn partial_cmp(&self, other: &Protection) -> Option<Ordering> {
        (*self as u8).partial_cmp(&(*other as u8))
    }
}
impl PartialEq<u8> for Protection {
    #[inline]
    fn eq(&self, other: &u8) -> bool {
        (*self as u8).eq(other)
    }
}
impl PartialEq<u32> for Protection {
    #[inline]
    fn eq(&self, other: &u32) -> bool {
        (*self as u8).eq(&(*other as u8))
    }
}
impl PartialOrd<u8> for Protection {
    #[inline]
    fn partial_cmp(&self, other: &u8) -> Option<Ordering> {
        (*self as u8).partial_cmp(other)
    }
}
impl PartialOrd<u32> for Protection {
    #[inline]
    fn partial_cmp(&self, other: &u32) -> Option<Ordering> {
        (*self as u8).partial_cmp(&(*other as u8))
    }
}

impl ProcessEnvItem for str {
    #[inline]
    fn write_item(&self, buf: ProcessVariable<'_>) {
        buf.write_str(self);
    }
}
impl ProcessEnvItem for &str {
    #[inline]
    fn write_item(&self, buf: ProcessVariable<'_>) {
        buf.write_str(self);
    }
}
impl ProcessEnvItem for &[u8] {
    #[inline]
    fn write_item(&self, buf: ProcessVariable<'_>) {
        buf.write_u8(self);
    }
}
impl ProcessEnvItem for &[u16] {
    #[inline]
    fn write_item(&self, buf: ProcessVariable<'_>) {
        buf.write(self);
    }
}
impl<'a> ProcessEnvItem for CharLike<'a> {
    #[inline]
    fn write_item(&self, buf: ProcessVariable<'_>) {
        buf.write_u8(self);
    }
}
impl<'a> ProcessEnvItem for WCharLike<'a> {
    #[inline]
    fn write_item(&self, buf: ProcessVariable<'_>) {
        buf.write(self);
    }
}
impl<T: AsRef<str>> ProcessEnvItem for (T, T) {
    #[inline]
    fn write_item(&self, buf: ProcessVariable<'_>) {
        buf.write_str(self.0.as_ref());
        buf.push_u8(0x3D); // '='
        buf.write_str(self.1.as_ref());
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Display, Formatter, Result};
    use core::option::Option::{None, Some};

    use crate::structs::{ProcessEnvironment, Protection};

    impl Debug for Protection {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            match self {
                Protection::None => f.write_str("None"),
                Protection::Authenticode => f.write_str("Authenticode"),
                Protection::CodeGeneration => f.write_str("CodeGeneration"),
                Protection::Antimalware => f.write_str("Antimalware"),
                Protection::Lsa => f.write_str("Lsa"),
                Protection::Windows => f.write_str("Windows"),
                Protection::WinTrusted => f.write_str("WinTrusted"),
                Protection::WinSystem => f.write_str("WinSystem"),
                Protection::StoreApp => f.write_str("StoreApp"),
            }
        }
    }

    impl Debug for ProcessEnvironment {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            match self.as_env() {
                Some(v) => Debug::fmt(&v, f),
                None => f.write_str("[]"),
            }
        }
    }
    impl Display for ProcessEnvironment {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            match self.as_env() {
                Some(v) => Display::fmt(&v, f),
                None => f.write_str("\"\""),
            }
        }
    }
}
