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

extern crate core;

extern crate xrmt_bugtrack;

use core::array::IntoIter;
use core::clone::Clone;
use core::cmp::Ord;
use core::convert::Into;
use core::default::Default;
use core::iter::{ExactSizeIterator, FusedIterator, IntoIterator, Iterator};
use core::marker::{Copy, PhantomData};
use core::mem::transmute;
use core::ops::{Deref, DerefMut, Drop};
use core::option::Option::{self, None, Some};
use core::ptr::{copy_nonoverlapping, NonNull};
use core::result::Result::{Err, Ok};
use core::slice::from_raw_parts;

use xrmt_bugtrack::bugtrack;

use crate::functions::{close_handle, FreeLibrary, LdlLoadAddress};
use crate::structs::{AnsiString, Handle, NonZeroHandle};
use crate::utils::copy;
use crate::{load_dll, load_dll_hash, Win32Error, Win32Result};

const FORWARD_COUNT: usize = 3usize;

pub struct Forward {
    pub hash: u32,
    addr:     NonZeroHandle,
}
#[repr(C)]
pub struct ImageNtHeader {
    pub signature: u32,
    pub file:      ImageFileHeader,
}
#[repr(C)]
pub struct ImageResource {
    pub len:   u16,
    pub flags: u16,
    pub text:  [u16; 1],
}
#[repr(C)]
pub struct ImageDosHeader {
    pub magic: u16,
    pad1:      [u8; 56],
    pub pos:   u32,
}
#[repr(C)]
pub struct ImageExportDir {
    pad1:                         [u32; 3],
    pub name:                     u32,
    pub base:                     u32,
    pub number_of_functions:      u32,
    pub number_of_names:          u32,
    pub address_of_functions:     u32,
    pub address_of_names:         u32,
    pub address_of_name_ordinals: u32,
}
pub struct ImageExport<'a> {
    pub buf:     &'a [u8],
    pub address: usize,
    forward:     bool,
    base:        NonNull<ImageExportsIter<'a>>,
}
#[repr(C)]
pub struct ImageFileHeader {
    pub machine:         u16,
    pub section_size:    u16,
    pad1:                [u32; 3],
    pub opt_header_size: u16,
    pub characteristics: u16,
}
#[repr(C)]
pub struct ImageDataDirectory {
    pub address: u32,
    pub size:    u32,
}
pub struct ImageExportsIter<'a> {
    ord:     usize,
    pos:     u32,
    name:    usize,
    func:    usize,
    base:    Handle,
    count:   u32,
    index:   usize,
    forward: ForwardList,
    _p:      PhantomData<&'a ()>,
}
#[repr(C)]
pub struct ImageOptionalHeader32 {
    pad1:                    [u8; 92],
    pub number_of_rva_sizes: u32,
    pub directory:           [ImageDataDirectory; 16],
}
#[repr(C)]
pub struct ImageOptionalHeader64 {
    pad1:                    [u8; 108],
    pub number_of_rva_sizes: u32,
    pub directory:           [ImageDataDirectory; 16],
}
pub struct ForwardList([Forward; FORWARD_COUNT]);

impl Forward {
    #[inline]
    pub const fn empty() -> Forward {
        Forward {
            addr: NonZeroHandle::invalid(),
            hash: 0u32,
        }
    }

    #[inline]
    pub fn close(&mut self) {
        if !self.addr.is_invalid() {
            let _ = FreeLibrary(self.addr.get());
        }
    }
    #[inline]
    pub fn handle(&self) -> Handle {
        self.addr.get()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.hash == 0
    }
    #[inline]
    fn set(&mut self, v: u32, h: NonZeroHandle) {
        (self.addr, self.hash) = (h, v)
    }
}
impl ForwardList {
    #[inline]
    pub const fn new() -> ForwardList {
        ForwardList([Forward::empty(); FORWARD_COUNT])
    }

    #[inline]
    pub fn add(&mut self, hash: u32, h: NonZeroHandle) -> bool {
        for i in self.0.iter_mut() {
            if !i.is_empty() {
                continue;
            }
            i.set(hash, h);
            return true;
        }
        false
    }
    #[inline]
    pub fn get_by_hash(&self, hash: u32) -> Option<NonZeroHandle> {
        // Fastpath, check any already loaded DLLs.
        if let Some(h) = load_dll_hash(hash) {
            return Some(h);
        }
        // Check self.
        for i in self.0.iter() {
            if i.hash == hash {
                return Some(i.addr);
            }
        }
        None
    }
}
impl ImageResource {
    #[inline]
    pub fn is_u16(&self) -> bool {
        self.flags & 1 != 0
    }
    #[inline]
    pub fn as_u8_slice(&self) -> &[u8] {
        unsafe { from_raw_parts(self.text.as_ptr() as *const u8, self.len as usize) }
    }
    #[inline]
    pub fn as_u16_slice(&self) -> &[u16] {
        unsafe { from_raw_parts(self.text.as_ptr(), self.len as usize) }
    }
    #[inline]
    pub fn copy_into(&self, b: &mut [u16]) -> usize {
        if self.is_u16() {
            return copy(self.as_u16_slice(), b);
        }
        let (n, s) = (b.len().min(self.len as usize), self.as_u8_slice());
        for (i, v) in unsafe { b.get_unchecked_mut(0..n) }.iter_mut().enumerate() {
            *v = unsafe { *s.get_unchecked(i) as u16 };
        }
        n
    }
}
impl<'a> ImageExport<'a> {
    #[inline]
    pub fn hash(&self) -> u32 {
        hash(self.buf, true)
    }
    #[inline]
    pub fn name(&self) -> &'a str {
        unsafe {
            transmute(
                self.buf
                    .get_unchecked(0..self.buf.iter().position(|v| *v == 0).unwrap_or(255)),
            )
        }
    }
    #[inline]
    pub fn is_forward(&self) -> bool {
        self.forward
    }
    #[inline]
    pub fn address(&self) -> Win32Result<usize> {
        if self.is_forward() {
            unsafe { (&mut *self.base.as_ptr()).forward(from_raw_parts(self.address as *const u8, 255)) }
        } else {
            Ok(self.address)
        }
    }
}
impl<'a> ImageExportsIter<'a> {
    pub fn new(h: Handle) -> Win32Result<ImageExportsIter<'a>> {
        let d = unsafe { &*(h.as_usize() as *const ImageDosHeader) };
        if d.magic != 0x5A4D {
            return Err(Win32Error::InvalidHeader);
        }
        let n = unsafe { &*((h + (d.pos as usize)) as *const ImageNtHeader) };
        match 0 {
            _ if n.signature != 0x00004550 => return Err(Win32Error::InvalidHeader),
            _ if n.file.characteristics & 0x2000 == 0 => return Err(Win32Error::InvalidLibrary),
            _ => (),
        }
        match n.file.machine {
            0 | 0x14C | 0x1C4 | 0xAA64 | 0x8664 => (),
            _ => return Err(Win32Error::InvalidImage),
        }
        let p = d.pos as usize + 0x18;
        let i = unsafe {
            match *((h + p) as *const u16) {
                0x20B => &(&*((h + p) as *const ImageOptionalHeader64)).directory[0],
                _ => &(&*((h + p) as *const ImageOptionalHeader32)).directory[0],
            }
        };
        if i.size == 0 || i.address == 0 {
            return Err(Win32Error::InvalidObject);
        }
        let e = unsafe { &*((h + (i.address as usize)) as *const ImageExportDir) };
        Ok(ImageExportsIter {
            ord:     h + (e.address_of_name_ordinals as usize),
            pos:     0u32,
            name:    h + (e.address_of_names as usize),
            func:    h + (e.address_of_functions as usize),
            base:    h,
            count:   e.number_of_names,
            index:   h + (i.address as usize + i.size as usize),
            forward: ForwardList::new(),
            _p:      PhantomData,
        })
    }

    #[inline]
    pub fn forwards(&mut self) -> &mut ForwardList {
        &mut self.forward
    }

    fn forward(&mut self, b: &[u8]) -> Win32Result<usize> {
        if unsafe { *b.get_unchecked(0) == 0 } {
            return Err(Win32Error::InvalidName);
        }
        let mut i = b.iter();
        // Since one is after the other, this is ok! (It allows us to not have
        // to do checks on length :D).
        let n = i.position(|v| *v == 0x2E).ok_or(Win32Error::InvalidName)?;
        let e = i.position(|v| *v == 0).ok_or(Win32Error::InvalidName)?;
        if e > 255 || n > e {
            return Err(Win32Error::InvalidName);
        }
        // Should never realistically be more than 256, but we'll be careful.
        let mut d = [0u8; 261]; // 256 + 4 + NULL
        let f = unsafe {
            copy_nonoverlapping(b.as_ptr(), d.as_mut_ptr(), n);
            *d.get_unchecked_mut(n + 4) = 0;
            *d.get_unchecked_mut(n + 3) = 0x6C;
            *d.get_unchecked_mut(n + 2) = 0x6C;
            *d.get_unchecked_mut(n + 1) = 0x64;
            *d.get_unchecked_mut(n) = 0x2E;
            d.get_unchecked(0..n + 5)
        };
        bugtrack!(
            "(ImageExportsIter).forward(): Found forwarded Function on {:X} pointing to '{}'.",
            self.base,
            unsafe { core::str::from_utf8_unchecked(&f[..f.len() - 1]) }
        );
        // hash will keep the names all lowercase.
        let v = hash(f, false);
        bugtrack!(
            "(ImageExportsIter).forward(): Using {} computed Hash {v:X}",
            unsafe { core::str::from_utf8_unchecked(&f[..f.len() - 1]) }
        );
        let (h, c) = match self.forward.get_by_hash(v) {
            Some(h) => (h, true),
            None => (load_dll(f)?, false),
        };
        unsafe {
            copy_nonoverlapping(b.as_ptr().add(n + 1), d.as_mut_ptr(), e);
            *d.get_unchecked_mut(e) = 0; // Add NULL.
        }
        bugtrack!(
            "(ImageExportsIter).forward(): Loading forwarded Function '{}' from {h:X}..",
            unsafe { core::str::from_utf8_unchecked(&d[0..e]) }
        );
        let x = match load_func(*h, unsafe { d.get_unchecked(0..e + 1) }) {
            Ok(x) => x,
            Err(e) => {
                if !c {
                    // Only unload if not cached.
                    let _ = unsafe { close_handle(*h) };
                }
                return Err(e);
            },
        };
        bugtrack!(
            "(ImageExportsIter).forward(): Loaded forward DLL {x:X} function '{}'.",
            unsafe { core::str::from_utf8_unchecked(&d[0..e]) }
        );
        if !c && !self.forward.add(v, h) {
            bugtrack!("(ImageExportsIter).forward(): Cannot save Handle {h:?}, too many links, leaking Handle!");
        }
        Ok(x)
    }
}

impl Copy for Forward {}
impl Clone for Forward {
    #[inline]
    fn clone(&self) -> Forward {
        Forward { addr: self.addr, hash: self.hash }
    }
}

impl Drop for ForwardList {
    #[inline]
    fn drop(&mut self) {
        // Drop the forwards left
        // Normally, they will be swapped out so this is ok if we're not using them.
        for i in self.0.iter_mut() {
            i.close();
        }
    }
}
impl Clone for ForwardList {
    #[inline]
    fn clone(&self) -> ForwardList {
        ForwardList(self.0.clone())
    }
}
impl Deref for ForwardList {
    type Target = [Forward];

    #[inline]
    fn deref(&self) -> &[Forward] {
        &self.0
    }
}
impl Default for ForwardList {
    #[inline]
    fn default() -> ForwardList {
        ForwardList::new()
    }
}
impl DerefMut for ForwardList {
    #[inline]
    fn deref_mut(&mut self) -> &mut [Forward] {
        &mut self.0
    }
}
impl IntoIterator for ForwardList {
    type Item = Forward;
    type IntoIter = IntoIter<Forward, FORWARD_COUNT>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> Iterator for ImageExportsIter<'a> {
    type Item = ImageExport<'a>;

    #[inline]
    fn next(&mut self) -> Option<ImageExport<'a>> {
        if self.pos >= self.count {
            return None;
        }
        //
        // 1. Ordinal Position of Name at 'pos': (u16 sized, then deref)
        //
        //   *((self.base +self.ord as usize + (self.pos as usize * 2usize)) as *const
        // u16)
        //
        // 2. Function Position of Ordinal from #1: (u32 sized, then deref)
        //
        //   *((self.base + self.func as usize + ($1 as usize * 4usize)) as *const u32)
        //
        // 3. Add to Image Base
        //
        //   self.base + $2 as usize
        //
        let a = unsafe { self.base + (*((self.func + (((*((self.ord + (self.pos as usize) * 2) as *const u16)) as usize) * 4)) as *const u32)) as usize };
        let b = unsafe {
            from_raw_parts(
                (self.base + (*((self.name + ((self.pos as usize) * 4)) as *const u32) as usize)) as *const u8,
                0x100,
            )
        };
        self.pos += 1;
        Some(ImageExport {
            buf:     b,
            base:    unsafe { NonNull::new_unchecked(self) },
            address: a,
            forward: a < self.index && a > self.func,
        })
        //
        // If a > max_function_addr && a < function_index == Forwarded function.
        //
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.count as usize, Some(self.count as usize))
    }
}
impl FusedIterator for ImageExportsIter<'_> {}
impl<'a> ExactSizeIterator for ImageExportsIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.count as usize
    }
}

#[inline]
fn hash(b: &[u8], case: bool) -> u32 {
    let mut h = 0x811C9DC5u32;
    for i in b {
        if *i == 0 {
            break;
        }
        h = h.wrapping_mul(0x1000193);
        h ^= if case {
            *i as u32
        } else {
            (match *i {
                (0x41..=0x5A) => *i + 0x20,
                _ => *i,
            }) as u32
        };
    }
    h
}
#[inline]
fn load_func(dll: Handle, name: &[u8]) -> Win32Result<usize> {
    let w = name.into();
    let a = AnsiString::new(&w);
    unsafe { LdlLoadAddress(dll, 0, &a) }
}
