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

extern crate xrmt_crypt;
extern crate xrmt_data;

use alloc::vec::Vec;
use core::cmp::Ord;
use core::convert::{From, Into};
use core::hint::unlikely;
use core::iter::Iterator;
use core::matches;
use core::ops::FnOnce;
use core::option::Option::{None, Some};
use core::result::Result::{Err, Ok};

use xrmt_crypt::crypt;
use xrmt_data::text::str_to_u16_unchecked;
use xrmt_data::VecLike;

use crate::env::expand;
use crate::functions::{GetCurrentDirectory, GetCurrentProcessPEB, GetEnvironment};
use crate::structs::{StringLike, StringLikeU16, WChar, WCharLike};
use crate::{Win32Error, Win32Result};

const PREFIX_NT: [u16; 4] = [0x5C, 0x3F, 0x3F, 0x5C]; // \??\
const PREFIX_UNC: [u16; 3] = [0x55, 0x4E, 0x43]; // UNC

struct PathBuilder {
    buf:   Vec<u16>,
    end:   usize,
    // ^ We have a separate end value as the buf might end 'earlier' than the 'actual'
    // end point. This is used so we can copy without resizing if needed, regardless
    // of capacity, since the Deref impl of Vec will use the built-in length.
    start: usize,
}

impl PathBuilder {
    #[inline]
    fn new(v: Vec<u16>) -> PathBuilder {
        // Might need to modify it, so we convert it to WChar and take out
        // the Vec to work on it directly.
        PathBuilder {
            end:   v.len(),
            start: 0,
            buf:   v,
        }
    }
    fn expand<'a>(dos: bool, env: bool, path: impl Into<WCharLike<'a>>) -> WCharLike<'a> {
        let v: WCharLike = path.into();
        // Quickpath check for paths we don't modify.
        // Matches:
        // - NT full paths (\??\C:\file.txt)
        // - NT device paths (\Device\)
        let r = v.is_empty()
            || (v.len() > 4 && unsafe { *v.get_unchecked(0) == 0x5C && *v.get_unchecked(1) == 0x3F && *v.get_unchecked(2) == 0x3F && *v.get_unchecked(3) == 0x5C })
            || (v.len() > 8 && unsafe { *v.get_unchecked(0) == 0x5C && matches!(*v.get_unchecked(1), 0x44 | 0x64) && matches!(*v.get_unchecked(2), 0x45 | 0x65) && matches!(*v.get_unchecked(6), 0x45 | 0x65) && *v.get_unchecked(7) == 0x5C });
        if r {
            return v;
        }
        let mut e = PathBuilder::new(v.into_owned().into_vec());
        match (e.len(), e.get(0)) {
            // Match: \\
            (2, 0x5C) if e.get(1) == 0x5C => {
                e.replace(&PREFIX_NT); // \??\
                e.insert(&PREFIX_UNC); // UNC
                e.buf.push(0x5C); // Add end seperator.
                e.end += 1;
                return e.wchar();
            },
            // Match: \\.
            (3, 0x5C) if e.get(1) == 0x5C && e.get(2) == 0x2E => {
                e.replace(&PREFIX_NT); // \??\
                return e.wchar();
            },
            // Match: \\?\**
            // Legacy DOS paths. These are NOT parsed.
            (4.., 0x5C) if e.get(1) == 0x5C && e.get(2) == 0x3F && e.get(3) == 0x5C => {
                unsafe { *e.buf.get_unchecked_mut(1) = 0x3F }; // Fixup "\\?\" to "\??\"
                return e.wchar();
            },
            _ => (),
        }
        // Do env replacements here.
        // Don't parse if 'env' is false.
        let mut b = PathBuilder::new(if env { expand(e.buf, GetEnvironment()) } else { e.buf });
        if unlikely(b.len() == 0) {
            return b.wchar();
        }
        let n = match (b.len(), b.get(0)) {
            // Match: [A-Za-z]:\
            // Absolute path with drive letter.
            (3.., 0x61..=0x7A | 0x41..=0x5A) if b.get(1) == 0x3A && matches!(b.get(2), 0x2F | 0x5C) => 0,
            // Match: [A-Za-z]:
            // Relative Drive path. There might be more chars that 2 but we'll check that
            // later.
            (2.., 0x61..=0x7A | 0x41..=0x5A) if b.get(1) == 0x3A => b.drive(),
            // Match: \\.\
            // Win32 Device/Local path.
            //
            // NOTE(dij): There's a bug in here that prevents changing drives in
            //            the path.
            //              ie: \\.\X:\ABC\..\..\C:\ --> \??\C:\ (We get \??\X:C:\)
            //
            //            I'm not really worried about fixing it as it's an undocumented
            //            path use-case and honestly feels like a security issue.
            // See: https://googleprojectzero.blogspot.com/2016/02/the-definitive-guide-on-win32-to-nt.html
            (4.., 0x2F | 0x5C) if matches!(b.get(1), 0x2F | 0x5C) && b.get(2) == 0x2E && matches!(b.get(3), 0x2F | 0x5C) => 0,
            // Match: \\?\
            // Legacy DOS paths. Incorrect, so normalize it.
            (3.., 0x2F | 0x5C) if matches!(b.get(1), 0x2F | 0x5C) && b.get(2) == 0x2F && matches!(b.get(3), 0x2F | 0x5C) => 0,
            // Match: \\[^.]\
            // UNC/Network path.
            (4.., 0x2F | 0x5C) if matches!(b.get(1), 0x2F | 0x5C) && b.get(2) != 0x2E => b.unc(dos),
            // Possible relative path. Check it to see.
            _ => b.default(),
        };
        if !dos {
            // Don't add seperator unless we need it.
            b.insert(unsafe { PREFIX_NT.get_unchecked(0..if b.first() == 0x5C { 3 } else { 4 }) });
            b.collapse(n + 4); // Ignore NT prefix when collapsing
        } else {
            b.collapse(n);
        }
        b.wchar()
    }

    #[inline]
    fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
    #[inline]
    fn first(&self) -> u16 {
        if self.len() > 0 {
            self.get(0)
        } else {
            0
        }
    }
    fn drive(&mut self) -> usize {
        // Drive relative paths
        let d = GetCurrentDirectory();
        if unlikely(d.len() < 2) {
            return 0; // WTF
        }
        // Look for ':', it should be there..
        let y = match d.iter().position(|v| *v == 0x3A) {
            Some(i) => unsafe { *d.get_unchecked(i.saturating_sub(1)) },
            None => 0, // *shrug*, fallback.
        };
        let x = self.get(0);
        // Case-insensitive compare
        let r = match (x, y) {
            (0x61..=0x7A, 0x41..=0x5A) => x == y + 0x20,
            (0x41..=0x5A, 0x61..=0x7A) => x + 0x20 == y,
            _ => x == y,
        };
        self.start += 2; // Remove [LETTER]:
        if r {
            // We matched the current drive, return it inserted
            self.insert(&d);
            return 0;
        }
        // Look for =[LETTER]:
        let k = [0x3D, if x >= 0x61 { x - 0x20 } else { x } as u16, 0x3A, 0x5C];
        // Ignore the seperator at the end.
        let s = unsafe { k.get_unchecked(0..3) };
        match GetEnvironment().iter().find(|v| v.is_key(s)).and_then(|v| v.value()) {
            Some(v) => self.insert(&v),
            None => self.insert(unsafe { k.get_unchecked(1..) }),
            // ^ Just insert the drive letter then, this also adds a seperator at the end, in-case.
        }
        0
    }
    #[inline]
    fn get(&self, v: usize) -> u16 {
        // We do bounds checking before, so we're ok.
        unsafe { *self.buf.get_unchecked(self.start + v) }
    }
    #[inline]
    fn default(&mut self) -> usize {
        // Relative paths
        let d = GetCurrentDirectory();
        if unlikely(d.len() < 2) {
            return 0;
        }
        if matches!(self.first(), 0x2F | 0x5C) {
            // First path is a seperator.
            if let Some(i) = d.iter().position(|v| *v == 0x3A) {
                self.insert(unsafe { d.get_unchecked(0..=i) });
            }
        } else {
            self.insert(&d);
        }
        0
    }
    fn insert(&mut self, s: &[u16]) {
        if unlikely(s.is_empty()) {
            return; // Nothing to do.
        }
        // Do either have a starting/ending seperator?
        let d = matches!(self.first(), 0x2F | 0x5C) || matches!(s.last(), Some(0x2F | 0x5C));
        let n = s.len() + if d { 0 } else { 1 }; // Add space for seperator
        if n <= self.start {
            // We can copy inside the buffer since we have space at the start
            // avaliable. No copies or re-shifts needed!
            let i = self.start.saturating_sub(n);
            // ^ How much to shift over
            unsafe {
                let b = self.buf.get_unchecked_mut(i..n + i);
                b.copy_from_slice(s);
                if !d {
                    // Add path seperator if needed.
                    *b.get_unchecked_mut(n - 1) = 0x5C;
                }
            }
            self.start -= i;
            return;
        }
        // We need to extend and resize the array.
        let r = n.saturating_sub(self.start);
        // ^ How much we need to add.
        self.buf.resize(self.buf.len() + r, 0);
        self.buf.copy_within(0..self.end, r);
        unsafe {
            self.buf.get_unchecked_mut(0..s.len()).copy_from_slice(s);
            if !d {
                *self.buf.get_unchecked_mut(s.len()) = 0x5C;
            }
        }
        (self.start, self.end) = (0, self.end + r);
    }
    #[inline]
    fn replace(&mut self, v: &[u16]) {
        (self.start, self.end) = (0, v.len());
        self.buf.resize(v.len(), 0);
        self.buf.copy_from_slice(v);
        self.buf.shrink_to(v.len());
    }
    fn collapse(&mut self, start: usize) {
        let (mut i, mut n) = (start, usize::MAX);
        while i < self.len() {
            let v = self.get(i);
            // We're only interested in path seperators.
            if (v != 0x2F && v != 0x5C) || i == start {
                if n != usize::MAX {
                    let d = i - n; // Move over slashes once we hit a non-slash
                    if self.copy(i - 1, n) {
                        self.end -= d - self.start;
                    }
                    i -= d;
                }
                (i, n) = (i + 1, usize::MAX);
                continue;
            }
            if v == 0x2F {
                // Convert all '/' to '\'
                unsafe { *self.buf.get_unchecked_mut(self.start + i) = 0x5C };
            }
            // Check for ".\" and "..\"
            if self.dots(start, &mut i) {
                n = usize::MAX; // Reset the counter
            } else if n == usize::MAX && matches!(self.get(i - 1), 0x2F | 0x5C) {
                n = i - 1; // Set slash start
            }
            i += 1;
        }
        // Check and remove "..." suffix if it exists.
        if self.len() > start && self.end > 0 && unsafe { matches!(*self.buf.get_unchecked(self.end - 1), 0 | 0x20 | 0x2E) } {
            // ^ 'self.end - 1' is the "tail" of the buffer.
            let mut v = 0usize;
            for i in (start..self.len()).rev() {
                match self.get(i) {
                    0 | 0x20 | 0x2E => v += 1,
                    _ => break,
                }
            }
            self.end = self.end.saturating_sub(v);
        }
        if start > 0 {
            // Collapse any missed '/'
            for i in unsafe { self.buf.get_unchecked_mut(self.start..self.end) }
                .iter_mut()
                .filter(|v| **v == 0x2F)
            {
                *i = 0x5C;
            }
        }
    }
    #[inline]
    fn unc(&mut self, dos: bool) -> usize {
        // Look for the "server\share" portion of the path.
        let mut r = unsafe { self.buf.get_unchecked(self.start + 2..self.end) }.iter();
        let n = r.position(|v| *v == 0x2F || *v == 0x5C).unwrap_or(0) + r.position(|v| *v == 0x2F || *v == 0x5C).unwrap_or(0) + self.start + 2;
        if dos {
            // If we want a DOS path we don't shorten it for the NT UNC device
            // prefix.
            n + 2
        } else {
            self.start += 1; // Remove first slash.
            self.insert(&PREFIX_UNC); // UNC
            n + 3
        }
    }
    #[inline]
    fn wchar<'a>(mut self) -> WCharLike<'a> {
        if self.start > 0 {
            self.buf.copy_within(self.start..self.end, 0);
        }
        self.buf.truncate(self.len());
        if self.buf.last().map_or(true, |v| *v != 0) {
            self.buf.push(0); // Ensure a NULL ending.
        }
        self.buf.shrink_to(self.len());
        WCharLike::Owned(WChar::from(self.buf))
    }
    #[inline]
    fn copy(&mut self, start: usize, dest: usize) -> bool {
        if dest == 0 {
            self.start += start; // Only moving up, just truncate from the start.
            false
        } else {
            self.buf.copy_within(self.start + start..self.end, self.start + dest);
            true
        }
    }
    fn dots(&mut self, start: usize, p: &mut usize) -> bool {
        if self.get(*p - 1) != 0x2E {
            return false;
        }
        // Handle special "starting" cases first.
        match p.saturating_sub(start) {
            // Special case where starting with a  ".\". This case means that we
            // should remove the leading seperator.
            1 => {
                if self.copy(*p + 1, *p - 1) {
                    self.end -= 2;
                }
                *p -= 1;
            },
            // Handle simple "..\" cases here. These are also like the top one, as
            // these will only trigger during the first couple characters.
            2 => {
                if self.copy(*p + 1, *p - 2) {
                    self.end -= 3;
                }
                *p -= 2;
            },
            // Handle standard ".\"
            2.. if matches!(self.get(*p - 2), 0x2F | 0x5C) => {
                if self.copy(*p, *p - 2) {
                    self.end -= 2;
                }
                *p -= 2;
            },
            // Capture standard "..\" handle that below.
            3.. if self.get(*p - 2) == 0x2E && self.get(*p - 3) != 0x2E => {
                let v = unsafe { self.buf.get_unchecked(self.start..p.saturating_sub(3)) }
                    .iter()
                    .rposition(|v| *v == 0x2F || *v == 0x5C || *v == 0x3A)
                    .map_or(start, |v| v + 1)
                    .max(start);
                if self.copy(*p + 1, v) {
                    self.end -= (*p + 1).saturating_sub(v);
                }
                *p = v;
            },
            // No match
            _ => return false,
        }
        true
    }
}

#[inline]
pub fn path_normalize<'a>(path: impl Into<WCharLike<'a>>) -> WCharLike<'a> {
    PathBuilder::expand(false, true, path)
}
#[inline]
pub fn path_normalize_win32<'a>(path: impl Into<WCharLike<'a>>) -> WCharLike<'a> {
    PathBuilder::expand(true, true, path)
}

#[inline]
pub fn object_normalize<'a>(path: impl Into<WCharLike<'a>>) -> WCharLike<'a> {
    // SAFETY: NULL is allowed so this will never error.
    unsafe { object_normalize_path(true, path).unwrap_unchecked() }
}
pub fn object_normalize_path<'a>(null: bool, path: impl Into<WCharLike<'a>>) -> Win32Result<WCharLike<'a>> {
    let n = path.into();
    if n.is_empty() {
        return if null { Ok(n) } else { Err(Win32Error::InvalidName) };
    }
    // This is not what Windows normally does, but technically this would fail
    // anyway if we try to use an empty name, so we just remove the name.
    //
    // Also we'll return Null if the name is just '\'. If it begins with a
    // slash, so we'll let it free, since it's most likely a full path.
    if n.len() >= 1 && unsafe { *n.get_unchecked(0) == 0x5C } {
        if n.len() == 1 {
            return if null {
                Ok(WCharLike::Null)
            } else {
                Err(Win32Error::InvalidName)
            };
        }
        return Ok(n);
    }
    // Many of the name conversions by the ObjectManager follow this format.
    //   "\Sessions\<session_id>\BaseNamedObjects"
    //
    // However, Session ID == 0 will translate to "\BaseNamedObjects".
    //
    // We could put a check in here, but using
    //   "\Sessions\BNOLINKS\"
    //
    // Allows us to translate all Session IDs.
    //   "\Sessions\BNOLINKS\0" links to "\BaseNamedObjects"
    //   "\Sessions\BNOLINKS\1" links to "\Sessions\1\BaseNamedObjects"
    //
    // This saves us some string work, and we only need the one string to translate
    // both! This link is here since Windows XP SP0 (I checked!).
    //
    // Also, these do both have links to "Local" and "Global" so those both work
    //
    // More info here
    // https://learn.microsoft.com/en-us/windows/win32/termserv/kernel-object-namespaces
    let mut v = Vec::with_capacity(n.len_without_null() + 30);
    unsafe { str_to_u16_unchecked(&mut v, crypt!(0, r"\Sessions\BNOLINKS\")) };
    // Append Session ID.
    write_u32(&mut v, GetCurrentProcessPEB().session_id);
    // Push seperator
    v.push(0x5C);
    // Append name.
    v.extend_from_slice(&n);
    Ok(WCharLike::from(v))
}

#[inline]
pub(crate) fn raw_path<'a>(n: WCharLike<'a>, add: impl FnOnce() -> &'a str) -> WCharLike<'a> {
    if n.len() < 4 {
        return n;
    }
    if n.iter().find(|v| **v == 0x2F || **v == 0x5C).is_none() {
        let mut r = Vec::with_capacity(n.len());
        unsafe { str_to_u16_unchecked(&mut r, add()) };
        // The call MUST ensure that they are ending in a seperator.
        r.extend_from_slice(n.as_slice());
        return WCharLike::from(n);
    }
    path_normalize(n)
}

fn write_u32(b: &mut impl VecLike<u16>, s: u32) -> usize {
    let n = match s {
        0 => {
            b.push(0x30); // 0
            return 1;
        },
        1__________..=9__________ => 1,
        1_________0..=9_________9 => 2,
        1________00..=9________99 => 3,
        1_______000..=9_______999 => 4,
        1_____0_000..=9_____9_999 => 5,
        1____00_000..=9____99_999 => 6,
        1___000_000..=9___999_999 => 7,
        1_0_000_000..=9_9_999_999 => 8,
        100_000_000..=999_999_999 => 9,
        _ => 10,
    };
    let p = b.len();
    b.resize(p + n, 0);
    let mut v = s;
    for i in (1..n).rev() {
        let t = v / 0xA;
        // We reserved enough space, so we have don't need to bounds check.
        unsafe { *b.get_unchecked_mut(i + p) = 0x30 + (v - (t * 0xA)) as u16 };
        v = t;
        if v < 0xA {
            break;
        }
    }
    unsafe { *b.get_unchecked_mut(p) = 0x30 + v as u16 };
    n
}
