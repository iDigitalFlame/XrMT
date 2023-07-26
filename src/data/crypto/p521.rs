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

use core::ops::{Add, AddAssign, Index, IndexMut, Mul, MulAssign, Not, Sub, SubAssign};

use crate::util;
use crate::util::stx::io::{self, Error, ErrorKind, Read};
use crate::util::stx::prelude::*;

struct Num {
    len: u8,
    nat: [usize; 20],
}
struct Point {
    x: Element,
    y: Element,
    z: Element,
}
struct Element([u64; 9]);
struct Table([Point; 15]);

impl Num {
    const WORD_SIZE: usize = usize::BITS as usize;
    const WORD_SIZE_BYTE: usize = Num::WORD_SIZE / 8usize;

    const fn from_bytes(buf: &[u8]) -> Num {
        let (mut i, mut k) = (buf.len(), 0);
        let mut n = Num {
            nat: [0; 20],
            len: ((i + Num::WORD_SIZE_BYTE - 1) / Num::WORD_SIZE_BYTE) as u8,
        };
        while i >= Num::WORD_SIZE_BYTE {
            n.nat[k] = if cfg!(target_pointer_width = "64") {
                (buf[i - 1] as u64 | (buf[i - 2] as u64) << 8 | (buf[i - 3] as u64) << 16 | (buf[i - 4] as u64) << 24 | (buf[i - 5] as u64) << 32 | (buf[i - 6] as u64) << 40 | (buf[i - 7] as u64) << 48 | (buf[i - 8] as u64) << 56) as usize
            } else {
                (buf[i - 1] as u32 | (buf[i - 2] as u32) << 8 | (buf[i - 3] as u32) << 16 | (buf[i - 4] as u32) << 24) as usize
            };
            i -= Num::WORD_SIZE_BYTE;
            k += 1;
        }
        if i > 0 {
            let (mut d, mut s) = (0, 0);
            while i > 0 {
                d |= (buf[i - 1] as usize) << s;
                s += 8;
                i -= 1;
            }
            n.nat[n.len as usize - 1] = d;
        }
        while n.len > 0 && n.nat[n.len as usize - 1] == 0 {
            n.len -= 1
        }
        n
    }

    fn less_than(&self, other: &Num) -> bool {
        let (m, n) = (self.len, other.len);
        if m != n || m == 0 {
            return m < n;
        }
        let mut i = (m - 1) as usize;
        while i > 0 && self.nat[i] == other.nat[i] {
            i -= 1
        }
        self.nat[i] < other.nat[i]
    }
}
impl Point {
    const B: Element = Element([
        0x8014654FAE586387,
        0x78F7A28FEA35A81F,
        0x839AB9EFC41E961A,
        0xBD8B29605E9DD8DF,
        0xF0AB0C9CA8F63F49,
        0xF9DC5A44C8C77884,
        0x77516D392DCCD98A,
        0xFC94D10D05B42A0,
        0x4D,
    ]);
    const N: Num = Num::from_bytes(&[
        0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFA, 0x51, 0x86, 0x87, 0x83, 0xBF, 0x2F, 0x96,
        0x6B, 0x7F, 0xCC, 0x01, 0x48, 0xF7, 0x09, 0xA5, 0xD0, 0x3B, 0xB5, 0xC9, 0xB8, 0x89, 0x9C, 0x47, 0xAE, 0xBB, 0x6F, 0xB7, 0x1E, 0x91, 0x38, 0x64, 0x09,
    ]);

    #[inline]
    const fn new() -> Point {
        Point {
            x: Element::empty(),
            y: Element::one(),
            z: Element::empty(),
        }
    }

    fn scalar_base_mul(scalar: &[u8]) -> Point {
        let t = &table::GEN;
        let (mut p, mut i) = (Point::new(), 131usize);
        for b in scalar {
            p += t[i].select(b >> 0x4);
            i = i.wrapping_sub(1);
            p += t[i].select(b & 0xF);
            i = i.wrapping_sub(1);
        }
        p
    }

    fn double(&self) -> Point {
        let mut t0 = self.x.square();
        let t1 = self.y.square();
        let mut t2 = self.z.square();
        let mut t3 = self.x * self.y;
        t3 += t3;
        let mut z3 = self.x * self.z;
        z3 += z3;
        let mut y3 = (Point::B * t2) - z3;
        y3 += y3 + y3;
        let mut x3 = t1 - y3;
        y3 += t1;
        y3 *= x3;
        x3 *= t3;
        t2 += t2 + t2;
        z3 *= Point::B;
        z3 -= t2;
        z3 -= t0;
        z3 += z3 + z3;
        t0 += t0 + t0;
        t0 -= t2;
        t0 *= z3;
        y3 += t0;
        let mut t0 = self.y * self.z;
        t0 += t0;
        x3 -= t0 * z3;
        let mut z3 = t0 * t1;
        z3 += z3;
        Point { x: x3, y: y3, z: z3 + z3 }
    }
    fn scalar_mul(&self, scalar: &[u8]) -> Point {
        let (mut t, mut i) = (Table::new(*self), 1usize);
        while i < 15 {
            t[i] = t[i / 2].double();
            t[i + 1] = t[i] + *self;
            i += 2;
        }
        let mut r = Point::new();
        for (i, b) in scalar.iter().enumerate() {
            if i > 0 {
                r = r.double().double().double().double();
            }
            r += t.select(b >> 0x4);
            r = r.double().double().double().double();
            r += t.select(b & 0xF);
        }
        r
    }
    fn write(&self, buf: &mut [u8]) -> io::Result<()> {
        if buf.len() < 133 {
            return Err(ErrorKind::InvalidInput.into());
        }
        let z = !self.z;
        let (x, y) = (self.x * z, self.y * z);
        buf[0] = 4;
        x.write(&mut buf[0x01..0x43]);
        y.write(&mut buf[0x43..]);
        Ok(())
    }
    #[inline]
    fn select(&self, rhs: &Point, cond: u64) -> Point {
        Point {
            x: self.x.select(&rhs.x, cond),
            y: self.y.select(&rhs.y, cond),
            z: self.z.select(&rhs.z, cond),
        }
    }
    fn write_secret(&self, buf: &mut [u8]) -> io::Result<()> {
        if buf.len() < 65 {
            return Err(ErrorKind::InvalidInput.into());
        }
        let x = self.x * !self.z;
        let mut t = [0u8; 66];
        x.write(&mut t);
        // NOTE(dij): Converting to an BigInt and back removes the leading zero
        //            (if it exists), anyway even though keys can be 66 bytes,
        //            we're falling back to 65 as that's what the Go version does.
        if t[0] == 0 {
            util::copy(buf, &t[1..]);
        } else {
            util::copy(buf, &t);
        }
        Ok(())
    }
}
impl Table {
    #[inline]
    const fn new(p: Point) -> Table {
        Table([
            p,
            Point::new(),
            Point::new(),
            Point::new(),
            Point::new(),
            Point::new(),
            Point::new(),
            Point::new(),
            Point::new(),
            Point::new(),
            Point::new(),
            Point::new(),
            Point::new(),
            Point::new(),
            Point::new(),
        ])
    }

    #[inline]
    fn select(&self, v: u8) -> Point {
        let mut p = Point::new();
        for i in 1..16usize {
            p = self[i - 1].select(&p, (((i as u8 ^ v) as u32).wrapping_sub(1) >> 31) as u64);
        }
        p
    }
}
impl Element {
    const Z: [u8; 66] = [
        0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE,
    ];

    #[inline]
    const fn one() -> Element {
        Element([0x80000000000000, 0, 0, 0, 0, 0, 0, 0, 0])
    }
    #[inline]
    const fn empty() -> Element {
        Element([0, 0, 0, 0, 0, 0, 0, 0, 0])
    }

    #[inline]
    fn square(&self) -> Element {
        math::square(self)
    }
    #[inline]
    fn square_assign(&mut self) {
        *self = math::square(self)
    }
    #[inline]
    fn write(&self, buf: &mut [u8]) {
        math::write_bytes(self, buf);
        for i in 0..33 {
            (buf[i], buf[65 - i]) = (buf[65 - i], buf[i])
        }
    }
    #[inline]
    fn select(&self, other: &Element, cond: u64) -> Element {
        math::select(cond, other, self)
    }
}

impl Copy for Table {}
impl Clone for Table {
    fn clone(&self) -> Table {
        Table(self.0.clone())
    }
}
impl Index<usize> for Table {
    type Output = Point;

    #[inline]
    fn index(&self, index: usize) -> &Point {
        &self.0[index]
    }
}
impl IndexMut<usize> for Table {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Point {
        &mut self.0[index]
    }
}

impl Add for Point {
    type Output = Point;

    fn add(self, rhs: Point) -> Point {
        let mut t0 = self.x * rhs.x;
        let t1 = self.y * rhs.y;
        let mut t2 = self.z * rhs.z;
        let mut t3 = self.x + self.y;
        t3 *= rhs.x + rhs.y;
        let t3 = t3 - (t0 + t1);
        let mut t4 = self.y + self.z;
        t4 *= rhs.y + rhs.z;
        t4 -= t1 + t2;
        let mut x3 = self.x + self.z;
        x3 *= rhs.x + rhs.z;
        let mut y3 = x3 - (t0 + t2);
        let mut x3 = y3 - (Point::B * t2);
        x3 += x3 + x3;
        let mut z3 = t1 - x3;
        x3 += t1;
        y3 *= Point::B;
        t2 += t2 + t2;
        y3 -= t2;
        y3 -= t0;
        y3 += y3 + y3;
        t0 += t0 + t0;
        t0 -= t2;
        let t1 = t4 * y3;
        let y3 = (x3 * z3) + (t0 * y3);
        x3 *= t3;
        x3 -= t1;
        z3 *= t4;
        Point { x: x3, y: y3, z: z3 + (t3 * t0) }
    }
}
impl Copy for Point {}
impl Clone for Point {
    fn clone(&self) -> Point {
        Point {
            x: self.x.clone(),
            y: self.y.clone(),
            z: self.z.clone(),
        }
    }
}
impl AddAssign for Point {
    #[inline]
    fn add_assign(&mut self, rhs: Point) {
        *self = self.add(rhs)
    }
}
impl TryFrom<&[u8]> for Point {
    type Error = Error;

    #[inline]
    fn try_from(value: &[u8]) -> io::Result<Point> {
        Ok(Point {
            x: Element::try_from(&value[0x01..0x43])?,
            y: Element::try_from(&value[0x43..])?,
            z: Element::one(),
        })
    }
}

impl Add for Element {
    type Output = Element;

    #[inline]
    fn add(self, rhs: Element) -> Element {
        math::add(&self, &rhs)
    }
}
impl Sub for Element {
    type Output = Element;

    #[inline]
    fn sub(self, rhs: Element) -> Element {
        math::sub(&self, &rhs)
    }
}
impl Mul for Element {
    type Output = Element;

    #[inline]
    fn mul(self, rhs: Element) -> Element {
        math::mul(&self, &rhs)
    }
}
impl Not for Element {
    type Output = Element;

    #[inline]
    fn not(self) -> Element {
        math::invert(&self)
    }
}
impl Copy for Element {}
impl Clone for Element {
    fn clone(&self) -> Element {
        Element(self.0.clone())
    }
}
impl AddAssign for Element {
    #[inline]
    fn add_assign(&mut self, rhs: Element) {
        *self = math::add(self, &rhs)
    }
}
impl SubAssign for Element {
    #[inline]
    fn sub_assign(&mut self, rhs: Element) {
        *self = math::sub(self, &rhs)
    }
}
impl MulAssign for Element {
    #[inline]
    fn mul_assign(&mut self, rhs: Element) {
        *self = math::mul(self, &rhs)
    }
}
impl TryFrom<&[u8]> for Element {
    type Error = Error;

    fn try_from(value: &[u8]) -> io::Result<Element> {
        if value.len() < 66 {
            return Err(ErrorKind::InvalidInput.into());
        }
        for i in 0..value.len() {
            if value[i] < Element::Z[i] {
                break;
            }
            if value[i] > Element::Z[i] {
                return Err(ErrorKind::InvalidData.into());
            }
        }
        let mut t = [0u8; 66];
        t.copy_from_slice(&value[0..66]);
        for i in 0..33 {
            (t[i], t[65 - i]) = (t[65 - i], t[i])
        }
        Ok(math::from_bytes(&t))
    }
}

mod math {
    use super::Element;

    mod cm {
        const MASK: u64 = (1 << 32) - 1;

        #[inline]
        pub fn mul(x: u64, y: u64) -> (u64, u64) {
            let (x0, x1) = (x & MASK, x >> 32);
            let (y0, y1) = (y & MASK, y >> 32);
            let t = (x1 * y0) + ((x0 * y0) >> 32);
            (
                (x1 * y1) + (t >> 32) + (((t & MASK) + (x0 * y1)) >> 32),
                x.wrapping_mul(y),
            )
        }
        #[inline]
        pub fn mov(x: u64, y: u64, z: u64) -> u64 {
            let p = x.wrapping_mul(0xFFFFFFFFFFFFFFFF);
            (p & z) | ((!p) & y)
        }
        #[inline]
        pub fn add(x: u64, y: u64, carry: u64) -> (u64, u64) {
            let s = x.wrapping_add(y.wrapping_add(carry));
            (s, ((x & y) | ((x | y) & !s)) >> 63)
        }
        #[inline]
        pub fn sub(x: u64, y: u64, borrow: u64) -> (u64, u64) {
            let (a, b) = x.overflowing_sub(y);
            let (c, d) = a.overflowing_sub(borrow);
            (c, if b || d { 1 } else { 0 })
        }
    }

    pub(super) fn invert(e: &Element) -> Element {
        let mut z = *e * e.square();
        let mut x = z.square();
        for _ in 1..2 {
            x.square_assign();
        }
        z *= x;
        let mut x = z.square();
        for _ in 1..4 {
            x.square_assign();
        }
        z *= x;
        let mut x = z.square();
        for _ in 1..8 {
            x.square_assign();
        }
        z *= x;
        let mut x = z.square();
        for _ in 1..16 {
            x.square_assign();
        }
        z *= x;
        let mut x = z.square();
        for _ in 1..32 {
            x.square_assign();
        }
        z *= x;
        let mut x = *e * z.square();
        for _ in 0..64 {
            x.square_assign();
        }
        z *= x;
        let mut x = *e * z.square();
        for _ in 0..129 {
            x.square_assign();
        }
        z *= x;
        let mut x = *e * z.square();
        for _ in 0..259 {
            x.square_assign();
        }
        z *= x;
        for _ in 0..2 {
            z.square_assign();
        }
        *e * z
    }
    pub(super) fn square(e: &Element) -> Element {
        let (x11, x10) = cm::mul(e.0[0], e.0[8]);
        let (x13, x12) = cm::mul(e.0[0], e.0[7]);
        let (x15, x14) = cm::mul(e.0[0], e.0[6]);
        let (x17, x16) = cm::mul(e.0[0], e.0[5]);
        let (x19, x18) = cm::mul(e.0[0], e.0[4]);
        let (x21, x20) = cm::mul(e.0[0], e.0[3]);
        let (x23, x22) = cm::mul(e.0[0], e.0[2]);
        let (x25, x24) = cm::mul(e.0[0], e.0[1]);
        let (x27, x26) = cm::mul(e.0[0], e.0[0]);
        let (x28, x29) = cm::add(x27, x24, 0);
        let (x30, x31) = cm::add(x25, x22, x29);
        let (x32, x33) = cm::add(x23, x20, x31);
        let (x34, x35) = cm::add(x21, x18, x33);
        let (x36, x37) = cm::add(x19, x16, x35);
        let (x38, x39) = cm::add(x17, x14, x37);
        let (x40, x41) = cm::add(x15, x12, x39);
        let (x42, x43) = cm::add(x13, x10, x41);
        let x44 = x43 + x11;
        let (x46, x45) = cm::mul(x26, 0x1FF);
        let (x48, x47) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x50, x49) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x52, x51) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x54, x53) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x56, x55) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x58, x57) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x60, x59) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x62, x61) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x63, x64) = cm::add(x62, x59, 0);
        let (x65, x66) = cm::add(x60, x57, x64);
        let (x67, x68) = cm::add(x58, x55, x66);
        let (x69, x70) = cm::add(x56, x53, x68);
        let (x71, x72) = cm::add(x54, x51, x70);
        let (x73, x74) = cm::add(x52, x49, x72);
        let (x75, x76) = cm::add(x50, x47, x74);
        let (x77, x78) = cm::add(x48, x45, x76);
        let x79 = x78 + x46;
        let (_, x81) = cm::add(x26, x61, 0);
        let (x82, x83) = cm::add(x28, x63, x81);
        let (x84, x85) = cm::add(x30, x65, x83);
        let (x86, x87) = cm::add(x32, x67, x85);
        let (x88, x89) = cm::add(x34, x69, x87);
        let (x90, x91) = cm::add(x36, x71, x89);
        let (x92, x93) = cm::add(x38, x73, x91);
        let (x94, x95) = cm::add(x40, x75, x93);
        let (x96, x97) = cm::add(x42, x77, x95);
        let (x98, x99) = cm::add(x44, x79, x97);
        let (x101, x100) = cm::mul(e.0[1], e.0[8]);
        let (x103, x102) = cm::mul(e.0[1], e.0[7]);
        let (x105, x104) = cm::mul(e.0[1], e.0[6]);
        let (x107, x106) = cm::mul(e.0[1], e.0[5]);
        let (x109, x108) = cm::mul(e.0[1], e.0[4]);
        let (x111, x110) = cm::mul(e.0[1], e.0[3]);
        let (x113, x112) = cm::mul(e.0[1], e.0[2]);
        let (x115, x114) = cm::mul(e.0[1], e.0[1]);
        let (x117, x116) = cm::mul(e.0[1], e.0[0]);
        let (x118, x119) = cm::add(x117, x114, 0);
        let (x120, x121) = cm::add(x115, x112, x119);
        let (x122, x123) = cm::add(x113, x110, x121);
        let (x124, x125) = cm::add(x111, x108, x123);
        let (x126, x127) = cm::add(x109, x106, x125);
        let (x128, x129) = cm::add(x107, x104, x127);
        let (x130, x131) = cm::add(x105, x102, x129);
        let (x132, x133) = cm::add(x103, x100, x131);
        let x134 = x133 + x101;
        let (x135, x136) = cm::add(x82, x116, 0);
        let (x137, x138) = cm::add(x84, x118, x136);
        let (x139, x140) = cm::add(x86, x120, x138);
        let (x141, x142) = cm::add(x88, x122, x140);
        let (x143, x144) = cm::add(x90, x124, x142);
        let (x145, x146) = cm::add(x92, x126, x144);
        let (x147, x148) = cm::add(x94, x128, x146);
        let (x149, x150) = cm::add(x96, x130, x148);
        let (x151, x152) = cm::add(x98, x132, x150);
        let (x153, x154) = cm::add(x99, x134, x152);
        let (x156, x155) = cm::mul(x135, 0x1FF);
        let (x158, x157) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x160, x159) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x162, x161) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x164, x163) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x166, x165) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x168, x167) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x170, x169) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x172, x171) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x173, x174) = cm::add(x172, x169, 0);
        let (x175, x176) = cm::add(x170, x167, x174);
        let (x177, x178) = cm::add(x168, x165, x176);
        let (x179, x180) = cm::add(x166, x163, x178);
        let (x181, x182) = cm::add(x164, x161, x180);
        let (x183, x184) = cm::add(x162, x159, x182);
        let (x185, x186) = cm::add(x160, x157, x184);
        let (x187, x188) = cm::add(x158, x155, x186);
        let x189 = x188 + x156;
        let (_, x191) = cm::add(x135, x171, 0);
        let (x192, x193) = cm::add(x137, x173, x191);
        let (x194, x195) = cm::add(x139, x175, x193);
        let (x196, x197) = cm::add(x141, x177, x195);
        let (x198, x199) = cm::add(x143, x179, x197);
        let (x200, x201) = cm::add(x145, x181, x199);
        let (x202, x203) = cm::add(x147, x183, x201);
        let (x204, x205) = cm::add(x149, x185, x203);
        let (x206, x207) = cm::add(x151, x187, x205);
        let (x208, x209) = cm::add(x153, x189, x207);
        let x210 = x209 + x154;
        let (x212, x211) = cm::mul(e.0[2], e.0[8]);
        let (x214, x213) = cm::mul(e.0[2], e.0[7]);
        let (x216, x215) = cm::mul(e.0[2], e.0[6]);
        let (x218, x217) = cm::mul(e.0[2], e.0[5]);
        let (x220, x219) = cm::mul(e.0[2], e.0[4]);
        let (x222, x221) = cm::mul(e.0[2], e.0[3]);
        let (x224, x223) = cm::mul(e.0[2], e.0[2]);
        let (x226, x225) = cm::mul(e.0[2], e.0[1]);
        let (x228, x227) = cm::mul(e.0[2], e.0[0]);
        let (x229, x230) = cm::add(x228, x225, 0);
        let (x231, x232) = cm::add(x226, x223, x230);
        let (x233, x234) = cm::add(x224, x221, x232);
        let (x235, x236) = cm::add(x222, x219, x234);
        let (x237, x238) = cm::add(x220, x217, x236);
        let (x239, x240) = cm::add(x218, x215, x238);
        let (x241, x242) = cm::add(x216, x213, x240);
        let (x243, x244) = cm::add(x214, x211, x242);
        let x245 = x244 + x212;
        let (x246, x247) = cm::add(x192, x227, 0);
        let (x248, x249) = cm::add(x194, x229, x247);
        let (x250, x251) = cm::add(x196, x231, x249);
        let (x252, x253) = cm::add(x198, x233, x251);
        let (x254, x255) = cm::add(x200, x235, x253);
        let (x256, x257) = cm::add(x202, x237, x255);
        let (x258, x259) = cm::add(x204, x239, x257);
        let (x260, x261) = cm::add(x206, x241, x259);
        let (x262, x263) = cm::add(x208, x243, x261);
        let (x264, x265) = cm::add(x210, x245, x263);
        let (x267, x266) = cm::mul(x246, 0x1FF);
        let (x269, x268) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x271, x270) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x273, x272) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x275, x274) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x277, x276) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x279, x278) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x281, x280) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x283, x282) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x284, x285) = cm::add(x283, x280, 0);
        let (x286, x287) = cm::add(x281, x278, x285);
        let (x288, x289) = cm::add(x279, x276, x287);
        let (x290, x291) = cm::add(x277, x274, x289);
        let (x292, x293) = cm::add(x275, x272, x291);
        let (x294, x295) = cm::add(x273, x270, x293);
        let (x296, x297) = cm::add(x271, x268, x295);
        let (x298, x299) = cm::add(x269, x266, x297);
        let x300 = x299 + x267;
        let (_, x302) = cm::add(x246, x282, 0);
        let (x303, x304) = cm::add(x248, x284, x302);
        let (x305, x306) = cm::add(x250, x286, x304);
        let (x307, x308) = cm::add(x252, x288, x306);
        let (x309, x310) = cm::add(x254, x290, x308);
        let (x311, x312) = cm::add(x256, x292, x310);
        let (x313, x314) = cm::add(x258, x294, x312);
        let (x315, x316) = cm::add(x260, x296, x314);
        let (x317, x318) = cm::add(x262, x298, x316);
        let (x319, x320) = cm::add(x264, x300, x318);
        let x321 = x320 + x265;
        let (x323, x322) = cm::mul(e.0[3], e.0[8]);
        let (x325, x324) = cm::mul(e.0[3], e.0[7]);
        let (x327, x326) = cm::mul(e.0[3], e.0[6]);
        let (x329, x328) = cm::mul(e.0[3], e.0[5]);
        let (x331, x330) = cm::mul(e.0[3], e.0[4]);
        let (x333, x332) = cm::mul(e.0[3], e.0[3]);
        let (x335, x334) = cm::mul(e.0[3], e.0[2]);
        let (x337, x336) = cm::mul(e.0[3], e.0[1]);
        let (x339, x338) = cm::mul(e.0[3], e.0[0]);
        let (x340, x341) = cm::add(x339, x336, 0);
        let (x342, x343) = cm::add(x337, x334, x341);
        let (x344, x345) = cm::add(x335, x332, x343);
        let (x346, x347) = cm::add(x333, x330, x345);
        let (x348, x349) = cm::add(x331, x328, x347);
        let (x350, x351) = cm::add(x329, x326, x349);
        let (x352, x353) = cm::add(x327, x324, x351);
        let (x354, x355) = cm::add(x325, x322, x353);
        let x356 = x355 + x323;
        let (x357, x358) = cm::add(x303, x338, 0);
        let (x359, x360) = cm::add(x305, x340, x358);
        let (x361, x362) = cm::add(x307, x342, x360);
        let (x363, x364) = cm::add(x309, x344, x362);
        let (x365, x366) = cm::add(x311, x346, x364);
        let (x367, x368) = cm::add(x313, x348, x366);
        let (x369, x370) = cm::add(x315, x350, x368);
        let (x371, x372) = cm::add(x317, x352, x370);
        let (x373, x374) = cm::add(x319, x354, x372);
        let (x375, x376) = cm::add(x321, x356, x374);
        let (x378, x377) = cm::mul(x357, 0x1FF);
        let (x380, x379) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x382, x381) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x384, x383) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x386, x385) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x388, x387) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x390, x389) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x392, x391) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x394, x393) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x395, x396) = cm::add(x394, x391, 0);
        let (x397, x398) = cm::add(x392, x389, x396);
        let (x399, x400) = cm::add(x390, x387, x398);
        let (x401, x402) = cm::add(x388, x385, x400);
        let (x403, x404) = cm::add(x386, x383, x402);
        let (x405, x406) = cm::add(x384, x381, x404);
        let (x407, x408) = cm::add(x382, x379, x406);
        let (x409, x410) = cm::add(x380, x377, x408);
        let x411 = x410 + x378;
        let (_, x413) = cm::add(x357, x393, 0);
        let (x414, x415) = cm::add(x359, x395, x413);
        let (x416, x417) = cm::add(x361, x397, x415);
        let (x418, x419) = cm::add(x363, x399, x417);
        let (x420, x421) = cm::add(x365, x401, x419);
        let (x422, x423) = cm::add(x367, x403, x421);
        let (x424, x425) = cm::add(x369, x405, x423);
        let (x426, x427) = cm::add(x371, x407, x425);
        let (x428, x429) = cm::add(x373, x409, x427);
        let (x430, x431) = cm::add(x375, x411, x429);
        let x432 = x431 + x376;
        let (x434, x433) = cm::mul(e.0[4], e.0[8]);
        let (x436, x435) = cm::mul(e.0[4], e.0[7]);
        let (x438, x437) = cm::mul(e.0[4], e.0[6]);
        let (x440, x439) = cm::mul(e.0[4], e.0[5]);
        let (x442, x441) = cm::mul(e.0[4], e.0[4]);
        let (x444, x443) = cm::mul(e.0[4], e.0[3]);
        let (x446, x445) = cm::mul(e.0[4], e.0[2]);
        let (x448, x447) = cm::mul(e.0[4], e.0[1]);
        let (x450, x449) = cm::mul(e.0[4], e.0[0]);
        let (x451, x452) = cm::add(x450, x447, 0);
        let (x453, x454) = cm::add(x448, x445, x452);
        let (x455, x456) = cm::add(x446, x443, x454);
        let (x457, x458) = cm::add(x444, x441, x456);
        let (x459, x460) = cm::add(x442, x439, x458);
        let (x461, x462) = cm::add(x440, x437, x460);
        let (x463, x464) = cm::add(x438, x435, x462);
        let (x465, x466) = cm::add(x436, x433, x464);
        let x467 = x466 + x434;
        let (x468, x469) = cm::add(x414, x449, 0);
        let (x470, x471) = cm::add(x416, x451, x469);
        let (x472, x473) = cm::add(x418, x453, x471);
        let (x474, x475) = cm::add(x420, x455, x473);
        let (x476, x477) = cm::add(x422, x457, x475);
        let (x478, x479) = cm::add(x424, x459, x477);
        let (x480, x481) = cm::add(x426, x461, x479);
        let (x482, x483) = cm::add(x428, x463, x481);
        let (x484, x485) = cm::add(x430, x465, x483);
        let (x486, x487) = cm::add(x432, x467, x485);
        let (x489, x488) = cm::mul(x468, 0x1FF);
        let (x491, x490) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x493, x492) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x495, x494) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x497, x496) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x499, x498) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x501, x500) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x503, x502) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x505, x504) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x506, x507) = cm::add(x505, x502, 0);
        let (x508, x509) = cm::add(x503, x500, x507);
        let (x510, x511) = cm::add(x501, x498, x509);
        let (x512, x513) = cm::add(x499, x496, x511);
        let (x514, x515) = cm::add(x497, x494, x513);
        let (x516, x517) = cm::add(x495, x492, x515);
        let (x518, x519) = cm::add(x493, x490, x517);
        let (x520, x521) = cm::add(x491, x488, x519);
        let x522 = x521 + x489;
        let (_, x524) = cm::add(x468, x504, 0);
        let (x525, x526) = cm::add(x470, x506, x524);
        let (x527, x528) = cm::add(x472, x508, x526);
        let (x529, x530) = cm::add(x474, x510, x528);
        let (x531, x532) = cm::add(x476, x512, x530);
        let (x533, x534) = cm::add(x478, x514, x532);
        let (x535, x536) = cm::add(x480, x516, x534);
        let (x537, x538) = cm::add(x482, x518, x536);
        let (x539, x540) = cm::add(x484, x520, x538);
        let (x541, x542) = cm::add(x486, x522, x540);
        let x543 = x542 + x487;
        let (x545, x544) = cm::mul(e.0[5], e.0[8]);
        let (x547, x546) = cm::mul(e.0[5], e.0[7]);
        let (x549, x548) = cm::mul(e.0[5], e.0[6]);
        let (x551, x550) = cm::mul(e.0[5], e.0[5]);
        let (x553, x552) = cm::mul(e.0[5], e.0[4]);
        let (x555, x554) = cm::mul(e.0[5], e.0[3]);
        let (x557, x556) = cm::mul(e.0[5], e.0[2]);
        let (x559, x558) = cm::mul(e.0[5], e.0[1]);
        let (x561, x560) = cm::mul(e.0[5], e.0[0]);
        let (x562, x563) = cm::add(x561, x558, 0);
        let (x564, x565) = cm::add(x559, x556, x563);
        let (x566, x567) = cm::add(x557, x554, x565);
        let (x568, x569) = cm::add(x555, x552, x567);
        let (x570, x571) = cm::add(x553, x550, x569);
        let (x572, x573) = cm::add(x551, x548, x571);
        let (x574, x575) = cm::add(x549, x546, x573);
        let (x576, x577) = cm::add(x547, x544, x575);
        let x578 = x577 + x545;
        let (x579, x580) = cm::add(x525, x560, 0);
        let (x581, x582) = cm::add(x527, x562, x580);
        let (x583, x584) = cm::add(x529, x564, x582);
        let (x585, x586) = cm::add(x531, x566, x584);
        let (x587, x588) = cm::add(x533, x568, x586);
        let (x589, x590) = cm::add(x535, x570, x588);
        let (x591, x592) = cm::add(x537, x572, x590);
        let (x593, x594) = cm::add(x539, x574, x592);
        let (x595, x596) = cm::add(x541, x576, x594);
        let (x597, x598) = cm::add(x543, x578, x596);
        let (x600, x599) = cm::mul(x579, 0x1FF);
        let (x602, x601) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x604, x603) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x606, x605) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x608, x607) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x610, x609) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x612, x611) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x614, x613) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x616, x615) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x617, x618) = cm::add(x616, x613, 0);
        let (x619, x620) = cm::add(x614, x611, x618);
        let (x621, x622) = cm::add(x612, x609, x620);
        let (x623, x624) = cm::add(x610, x607, x622);
        let (x625, x626) = cm::add(x608, x605, x624);
        let (x627, x628) = cm::add(x606, x603, x626);
        let (x629, x630) = cm::add(x604, x601, x628);
        let (x631, x632) = cm::add(x602, x599, x630);
        let x633 = x632 + x600;
        let (_, x635) = cm::add(x579, x615, 0);
        let (x636, x637) = cm::add(x581, x617, x635);
        let (x638, x639) = cm::add(x583, x619, x637);
        let (x640, x641) = cm::add(x585, x621, x639);
        let (x642, x643) = cm::add(x587, x623, x641);
        let (x644, x645) = cm::add(x589, x625, x643);
        let (x646, x647) = cm::add(x591, x627, x645);
        let (x648, x649) = cm::add(x593, x629, x647);
        let (x650, x651) = cm::add(x595, x631, x649);
        let (x652, x653) = cm::add(x597, x633, x651);
        let x654 = x653 + x598;
        let (x656, x655) = cm::mul(e.0[6], e.0[8]);
        let (x658, x657) = cm::mul(e.0[6], e.0[7]);
        let (x660, x659) = cm::mul(e.0[6], e.0[6]);
        let (x662, x661) = cm::mul(e.0[6], e.0[5]);
        let (x664, x663) = cm::mul(e.0[6], e.0[4]);
        let (x666, x665) = cm::mul(e.0[6], e.0[3]);
        let (x668, x667) = cm::mul(e.0[6], e.0[2]);
        let (x670, x669) = cm::mul(e.0[6], e.0[1]);
        let (x672, x671) = cm::mul(e.0[6], e.0[0]);
        let (x673, x674) = cm::add(x672, x669, 0);
        let (x675, x676) = cm::add(x670, x667, x674);
        let (x677, x678) = cm::add(x668, x665, x676);
        let (x679, x680) = cm::add(x666, x663, x678);
        let (x681, x682) = cm::add(x664, x661, x680);
        let (x683, x684) = cm::add(x662, x659, x682);
        let (x685, x686) = cm::add(x660, x657, x684);
        let (x687, x688) = cm::add(x658, x655, x686);
        let x689 = x688 + x656;
        let (x690, x691) = cm::add(x636, x671, 0);
        let (x692, x693) = cm::add(x638, x673, x691);
        let (x694, x695) = cm::add(x640, x675, x693);
        let (x696, x697) = cm::add(x642, x677, x695);
        let (x698, x699) = cm::add(x644, x679, x697);
        let (x700, x701) = cm::add(x646, x681, x699);
        let (x702, x703) = cm::add(x648, x683, x701);
        let (x704, x705) = cm::add(x650, x685, x703);
        let (x706, x707) = cm::add(x652, x687, x705);
        let (x708, x709) = cm::add(x654, x689, x707);
        let (x711, x710) = cm::mul(x690, 0x1FF);
        let (x713, x712) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x715, x714) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x717, x716) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x719, x718) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x721, x720) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x723, x722) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x725, x724) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x727, x726) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x728, x729) = cm::add(x727, x724, 0);
        let (x730, x731) = cm::add(x725, x722, x729);
        let (x732, x733) = cm::add(x723, x720, x731);
        let (x734, x735) = cm::add(x721, x718, x733);
        let (x736, x737) = cm::add(x719, x716, x735);
        let (x738, x739) = cm::add(x717, x714, x737);
        let (x740, x741) = cm::add(x715, x712, x739);
        let (x742, x743) = cm::add(x713, x710, x741);
        let x744 = x743 + x711;
        let (_, x746) = cm::add(x690, x726, 0);
        let (x747, x748) = cm::add(x692, x728, x746);
        let (x749, x750) = cm::add(x694, x730, x748);
        let (x751, x752) = cm::add(x696, x732, x750);
        let (x753, x754) = cm::add(x698, x734, x752);
        let (x755, x756) = cm::add(x700, x736, x754);
        let (x757, x758) = cm::add(x702, x738, x756);
        let (x759, x760) = cm::add(x704, x740, x758);
        let (x761, x762) = cm::add(x706, x742, x760);
        let (x763, x764) = cm::add(x708, x744, x762);
        let x765 = x764 + x709;
        let (x767, x766) = cm::mul(e.0[7], e.0[8]);
        let (x769, x768) = cm::mul(e.0[7], e.0[7]);
        let (x771, x770) = cm::mul(e.0[7], e.0[6]);
        let (x773, x772) = cm::mul(e.0[7], e.0[5]);
        let (x775, x774) = cm::mul(e.0[7], e.0[4]);
        let (x777, x776) = cm::mul(e.0[7], e.0[3]);
        let (x779, x778) = cm::mul(e.0[7], e.0[2]);
        let (x781, x780) = cm::mul(e.0[7], e.0[1]);
        let (x783, x782) = cm::mul(e.0[7], e.0[0]);
        let (x784, x785) = cm::add(x783, x780, 0);
        let (x786, x787) = cm::add(x781, x778, x785);
        let (x788, x789) = cm::add(x779, x776, x787);
        let (x790, x791) = cm::add(x777, x774, x789);
        let (x792, x793) = cm::add(x775, x772, x791);
        let (x794, x795) = cm::add(x773, x770, x793);
        let (x796, x797) = cm::add(x771, x768, x795);
        let (x798, x799) = cm::add(x769, x766, x797);
        let x800 = x799 + x767;
        let (x801, x802) = cm::add(x747, x782, 0);
        let (x803, x804) = cm::add(x749, x784, x802);
        let (x805, x806) = cm::add(x751, x786, x804);
        let (x807, x808) = cm::add(x753, x788, x806);
        let (x809, x810) = cm::add(x755, x790, x808);
        let (x811, x812) = cm::add(x757, x792, x810);
        let (x813, x814) = cm::add(x759, x794, x812);
        let (x815, x816) = cm::add(x761, x796, x814);
        let (x817, x818) = cm::add(x763, x798, x816);
        let (x819, x820) = cm::add(x765, x800, x818);
        let (x822, x821) = cm::mul(x801, 0x1FF);
        let (x824, x823) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x826, x825) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x828, x827) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x830, x829) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x832, x831) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x834, x833) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x836, x835) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x838, x837) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x839, x840) = cm::add(x838, x835, 0);
        let (x841, x842) = cm::add(x836, x833, x840);
        let (x843, x844) = cm::add(x834, x831, x842);
        let (x845, x846) = cm::add(x832, x829, x844);
        let (x847, x848) = cm::add(x830, x827, x846);
        let (x849, x850) = cm::add(x828, x825, x848);
        let (x851, x852) = cm::add(x826, x823, x850);
        let (x853, x854) = cm::add(x824, x821, x852);
        let x855 = x854 + x822;
        let (_, x857) = cm::add(x801, x837, 0);
        let (x858, x859) = cm::add(x803, x839, x857);
        let (x860, x861) = cm::add(x805, x841, x859);
        let (x862, x863) = cm::add(x807, x843, x861);
        let (x864, x865) = cm::add(x809, x845, x863);
        let (x866, x867) = cm::add(x811, x847, x865);
        let (x868, x869) = cm::add(x813, x849, x867);
        let (x870, x871) = cm::add(x815, x851, x869);
        let (x872, x873) = cm::add(x817, x853, x871);
        let (x874, x875) = cm::add(x819, x855, x873);
        let x876 = x875 + x820;
        let (x878, x877) = cm::mul(e.0[8], e.0[8]);
        let (x880, x879) = cm::mul(e.0[8], e.0[7]);
        let (x882, x881) = cm::mul(e.0[8], e.0[6]);
        let (x884, x883) = cm::mul(e.0[8], e.0[5]);
        let (x886, x885) = cm::mul(e.0[8], e.0[4]);
        let (x888, x887) = cm::mul(e.0[8], e.0[3]);
        let (x890, x889) = cm::mul(e.0[8], e.0[2]);
        let (x892, x891) = cm::mul(e.0[8], e.0[1]);
        let (x894, x893) = cm::mul(e.0[8], e.0[0]);
        let (x895, x896) = cm::add(x894, x891, 0);
        let (x897, x898) = cm::add(x892, x889, x896);
        let (x899, x900) = cm::add(x890, x887, x898);
        let (x901, x902) = cm::add(x888, x885, x900);
        let (x903, x904) = cm::add(x886, x883, x902);
        let (x905, x906) = cm::add(x884, x881, x904);
        let (x907, x908) = cm::add(x882, x879, x906);
        let (x909, x910) = cm::add(x880, x877, x908);
        let x911 = x910 + x878;
        let (x912, x913) = cm::add(x858, x893, 0);
        let (x914, x915) = cm::add(x860, x895, x913);
        let (x916, x917) = cm::add(x862, x897, x915);
        let (x918, x919) = cm::add(x864, x899, x917);
        let (x920, x921) = cm::add(x866, x901, x919);
        let (x922, x923) = cm::add(x868, x903, x921);
        let (x924, x925) = cm::add(x870, x905, x923);
        let (x926, x927) = cm::add(x872, x907, x925);
        let (x928, x929) = cm::add(x874, x909, x927);
        let (x930, x931) = cm::add(x876, x911, x929);
        let (x933, x932) = cm::mul(x912, 0x1FF);
        let (x935, x934) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x937, x936) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x939, x938) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x941, x940) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x943, x942) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x945, x944) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x947, x946) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x949, x948) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x950, x951) = cm::add(x949, x946, 0);
        let (x952, x953) = cm::add(x947, x944, x951);
        let (x954, x955) = cm::add(x945, x942, x953);
        let (x956, x957) = cm::add(x943, x940, x955);
        let (x958, x959) = cm::add(x941, x938, x957);
        let (x960, x961) = cm::add(x939, x936, x959);
        let (x962, x963) = cm::add(x937, x934, x961);
        let (x964, x965) = cm::add(x935, x932, x963);
        let x966 = x965 + x933;
        let (_, x968) = cm::add(x912, x948, 0);
        let (x969, x970) = cm::add(x914, x950, x968);
        let (x971, x972) = cm::add(x916, x952, x970);
        let (x973, x974) = cm::add(x918, x954, x972);
        let (x975, x976) = cm::add(x920, x956, x974);
        let (x977, x978) = cm::add(x922, x958, x976);
        let (x979, x980) = cm::add(x924, x960, x978);
        let (x981, x982) = cm::add(x926, x962, x980);
        let (x983, x984) = cm::add(x928, x964, x982);
        let (x985, x986) = cm::add(x930, x966, x984);
        let x987 = x986 + x931;
        let (x988, x989) = cm::sub(x969, 0xFFFFFFFFFFFFFFFF, 0);
        let (x990, x991) = cm::sub(x971, 0xFFFFFFFFFFFFFFFF, x989);
        let (x992, x993) = cm::sub(x973, 0xFFFFFFFFFFFFFFFF, x991);
        let (x994, x995) = cm::sub(x975, 0xFFFFFFFFFFFFFFFF, x993);
        let (x996, x997) = cm::sub(x977, 0xFFFFFFFFFFFFFFFF, x995);
        let (x998, x999) = cm::sub(x979, 0xFFFFFFFFFFFFFFFF, x997);
        let (x1000, x1001) = cm::sub(x981, 0xFFFFFFFFFFFFFFFF, x999);
        let (x1002, x1003) = cm::sub(x983, 0xFFFFFFFFFFFFFFFF, x1001);
        let (x1004, x1005) = cm::sub(x985, 0x1FF, x1003);
        let (_, x1007) = cm::sub(x987, 0x0, x1005);
        let x1008 = cm::mov(x1007, x988, x969);
        let x1009 = cm::mov(x1007, x990, x971);
        let x1010 = cm::mov(x1007, x992, x973);
        let x1011 = cm::mov(x1007, x994, x975);
        let x1012 = cm::mov(x1007, x996, x977);
        let x1013 = cm::mov(x1007, x998, x979);
        let x1014 = cm::mov(x1007, x1000, x981);
        let x1015 = cm::mov(x1007, x1002, x983);
        let x1016 = cm::mov(x1007, x1004, x985);
        Element([x1008, x1009, x1010, x1011, x1012, x1013, x1014, x1015, x1016])
    }
    pub(super) fn from_bytes(b: &[u8]) -> Element {
        let x1 = (b[65] as u64) << 8;
        let x2 = b[64];
        let x3 = (b[63] as u64) << 56;
        let x4 = (b[62] as u64) << 48;
        let x5 = (b[61] as u64) << 40;
        let x6 = (b[60] as u64) << 32;
        let x7 = (b[59] as u64) << 24;
        let x8 = (b[58] as u64) << 16;
        let x9 = (b[57] as u64) << 8;
        let x10 = b[56];
        let x11 = (b[55] as u64) << 56;
        let x12 = (b[54] as u64) << 48;
        let x13 = (b[53] as u64) << 40;
        let x14 = (b[52] as u64) << 32;
        let x15 = (b[51] as u64) << 24;
        let x16 = (b[50] as u64) << 16;
        let x17 = (b[49] as u64) << 8;
        let x18 = b[48];
        let x19 = (b[47] as u64) << 56;
        let x20 = (b[46] as u64) << 48;
        let x21 = (b[45] as u64) << 40;
        let x22 = (b[44] as u64) << 32;
        let x23 = (b[43] as u64) << 24;
        let x24 = (b[42] as u64) << 16;
        let x25 = (b[41] as u64) << 8;
        let x26 = b[40];
        let x27 = (b[39] as u64) << 56;
        let x28 = (b[38] as u64) << 48;
        let x29 = (b[37] as u64) << 40;
        let x30 = (b[36] as u64) << 32;
        let x31 = (b[35] as u64) << 24;
        let x32 = (b[34] as u64) << 16;
        let x33 = (b[33] as u64) << 8;
        let x34 = b[32];
        let x35 = (b[31] as u64) << 56;
        let x36 = (b[30] as u64) << 48;
        let x37 = (b[29] as u64) << 40;
        let x38 = (b[28] as u64) << 32;
        let x39 = (b[27] as u64) << 24;
        let x40 = (b[26] as u64) << 16;
        let x41 = (b[25] as u64) << 8;
        let x42 = b[24];
        let x43 = (b[23] as u64) << 56;
        let x44 = (b[22] as u64) << 48;
        let x45 = (b[21] as u64) << 40;
        let x46 = (b[20] as u64) << 32;
        let x47 = (b[19] as u64) << 24;
        let x48 = (b[18] as u64) << 16;
        let x49 = (b[17] as u64) << 8;
        let x50 = b[16];
        let x51 = (b[15] as u64) << 56;
        let x52 = (b[14] as u64) << 48;
        let x53 = (b[13] as u64) << 40;
        let x54 = (b[12] as u64) << 32;
        let x55 = (b[11] as u64) << 24;
        let x56 = (b[10] as u64) << 16;
        let x57 = (b[9] as u64) << 8;
        let x58 = b[8];
        let x59 = (b[7] as u64) << 56;
        let x60 = (b[6] as u64) << 48;
        let x61 = (b[5] as u64) << 40;
        let x62 = (b[4] as u64) << 32;
        let x63 = (b[3] as u64) << 24;
        let x64 = (b[2] as u64) << 16;
        let x65 = (b[1] as u64) << 8;
        let x66 = b[0] as u64;
        let x67 = x65 + x66;
        let x68 = x64 + x67;
        let x69 = x63 + x68;
        let x70 = x62 + x69;
        let x71 = x61 + x70;
        let x72 = x60 + x71;
        let x73 = x59 + x72;
        let x74 = x57 + x58 as u64;
        let x75 = x56 + x74;
        let x76 = x55 + x75;
        let x77 = x54 + x76;
        let x78 = x53 + x77;
        let x79 = x52 + x78;
        let x80 = x51 + x79;
        let x81 = x49 + x50 as u64;
        let x82 = x48 + x81;
        let x83 = x47 + x82;
        let x84 = x46 + x83;
        let x85 = x45 + x84;
        let x86 = x44 + x85;
        let x87 = x43 + x86;
        let x88 = x41 + x42 as u64;
        let x89 = x40 + x88;
        let x90 = x39 + x89;
        let x91 = x38 + x90;
        let x92 = x37 + x91;
        let x93 = x36 + x92;
        let x94 = x35 + x93;
        let x95 = x33 + x34 as u64;
        let x96 = x32 + x95;
        let x97 = x31 + x96;
        let x98 = x30 + x97;
        let x99 = x29 + x98;
        let x100 = x28 + x99;
        let x101 = x27 + x100;
        let x102 = x25 + x26 as u64;
        let x103 = x24 + x102;
        let x104 = x23 + x103;
        let x105 = x22 + x104;
        let x106 = x21 + x105;
        let x107 = x20 + x106;
        let x108 = x19 + x107;
        let x109 = x17 + x18 as u64;
        let x110 = x16 + x109;
        let x111 = x15 + x110;
        let x112 = x14 + x111;
        let x113 = x13 + x112;
        let x114 = x12 + x113;
        let x115 = x11 + x114;
        let x116 = x9 + x10 as u64;
        let x117 = x8 + x116;
        let x118 = x7 + x117;
        let x119 = x6 + x118;
        let x120 = x5 + x119;
        let x121 = x4 + x120;
        let x122 = x3 + x121;
        let x123 = x1 + x2 as u64;
        Element([x73, x80, x87, x94, x101, x108, x115, x122, x123])
    }
    pub(super) fn write_bytes(e: &Element, buf: &mut [u8]) {
        let x10 = e.0[0] as u8 & 0xFF;
        let x11 = e.0[0] >> 8;
        let x12 = (x11 as u8) & 0xFF;
        let x13 = x11 >> 8;
        let x14 = (x13 as u8) & 0xFF;
        let x15 = x13 >> 8;
        let x16 = (x15 as u8) & 0xFF;
        let x17 = x15 >> 8;
        let x18 = (x17 as u8) & 0xFF;
        let x19 = x17 >> 8;
        let x20 = (x19 as u8) & 0xFF;
        let x21 = x19 >> 8;
        let x22 = (x21 as u8) & 0xFF;
        let x23 = (x21 >> 8) as u8;
        let x24 = (e.0[1] as u8) & 0xFF;
        let x25 = e.0[1] >> 8;
        let x26 = (x25 as u8) & 0xFF;
        let x27 = x25 >> 8;
        let x28 = (x27 as u8) & 0xFF;
        let x29 = x27 >> 8;
        let x30 = (x29 as u8) & 0xFF;
        let x31 = x29 >> 8;
        let x32 = (x31 as u8) & 0xFF;
        let x33 = x31 >> 8;
        let x34 = (x33 as u8) & 0xFF;
        let x35 = x33 >> 8;
        let x36 = (x35 as u8) & 0xFF;
        let x37 = (x35 >> 8) as u8;
        let x38 = (e.0[2] as u8) & 0xFF;
        let x39 = e.0[2] >> 8;
        let x40 = (x39 as u8) & 0xFF;
        let x41 = x39 >> 8;
        let x42 = (x41 as u8) & 0xFF;
        let x43 = x41 >> 8;
        let x44 = (x43 as u8) & 0xFF;
        let x45 = x43 >> 8;
        let x46 = (x45 as u8) & 0xFF;
        let x47 = x45 >> 8;
        let x48 = (x47 as u8) & 0xFF;
        let x49 = x47 >> 8;
        let x50 = (x49 as u8) & 0xFF;
        let x51 = (x49 >> 8) as u8;
        let x52 = (e.0[3] as u8) & 0xFF;
        let x53 = e.0[3] >> 8;
        let x54 = (x53 as u8) & 0xFF;
        let x55 = x53 >> 8;
        let x56 = (x55 as u8) & 0xFF;
        let x57 = x55 >> 8;
        let x58 = (x57 as u8) & 0xFF;
        let x59 = x57 >> 8;
        let x60 = (x59 as u8) & 0xFF;
        let x61 = x59 >> 8;
        let x62 = (x61 as u8) & 0xFF;
        let x63 = x61 >> 8;
        let x64 = (x63 as u8) & 0xFF;
        let x65 = (x63 >> 8) as u8;
        let x66 = (e.0[4] as u8) & 0xFF;
        let x67 = e.0[4] >> 8;
        let x68 = (x67 as u8) & 0xFF;
        let x69 = x67 >> 8;
        let x70 = (x69 as u8) & 0xFF;
        let x71 = x69 >> 8;
        let x72 = (x71 as u8) & 0xFF;
        let x73 = x71 >> 8;
        let x74 = (x73 as u8) & 0xFF;
        let x75 = x73 >> 8;
        let x76 = (x75 as u8) & 0xFF;
        let x77 = x75 >> 8;
        let x78 = (x77 as u8) & 0xFF;
        let x79 = (x77 >> 8) as u8;
        let x80 = (e.0[5] as u8) & 0xFF;
        let x81 = e.0[5] >> 8;
        let x82 = (x81 & 0xFF) as u8;
        let x83 = x81 >> 8;
        let x84 = (x83 & 0xFF) as u8;
        let x85 = x83 >> 8;
        let x86 = (x85 as u8) & 0xFF;
        let x87 = x85 >> 8;
        let x88 = (x87 as u8) & 0xFF;
        let x89 = x87 >> 8;
        let x90 = (x89 as u8) & 0xFF;
        let x91 = x89 >> 8;
        let x92 = (x91 as u8) & 0xFF;
        let x93 = (x91 >> 8) as u8;
        let x94 = (e.0[6] as u8) & 0xFF;
        let x95 = e.0[6] >> 8;
        let x96 = (x95 & 0xFF) as u8;
        let x97 = x95 >> 8;
        let x98 = (x97 as u8) & 0xFF;
        let x99 = x97 >> 8;
        let x100 = (x99 as u8) & 0xFF;
        let x101 = x99 >> 8;
        let x102 = (x101 as u8) & 0xFF;
        let x103 = x101 >> 8;
        let x104 = (x103 as u8) & 0xFF;
        let x105 = x103 >> 8;
        let x106 = (x105 as u8) & 0xFF;
        let x107 = (x105 >> 8) as u8;
        let x108 = (e.0[7] as u8) & 0xFF;
        let x109 = e.0[7] >> 8;
        let x110 = (x109 as u8) & 0xFF;
        let x111 = x109 >> 8;
        let x112 = (x111 as u8) & 0xFF;
        let x113 = x111 >> 8;
        let x114 = (x113 as u8) & 0xFF;
        let x115 = x113 >> 8;
        let x116 = (x115 as u8) & 0xFF;
        let x117 = x115 >> 8;
        let x118 = (x117 as u8) & 0xFF;
        let x119 = x117 >> 8;
        let x120 = (x119 as u8) & 0xFF;
        let x121 = (x119 >> 8) as u8;
        let x122 = (e.0[8] as u8) & 0xFF;
        let x123 = (e.0[8] >> 8) as u8;
        buf[0] = x10;
        buf[1] = x12;
        buf[2] = x14;
        buf[3] = x16;
        buf[4] = x18;
        buf[5] = x20;
        buf[6] = x22;
        buf[7] = x23;
        buf[8] = x24;
        buf[9] = x26;
        buf[10] = x28;
        buf[11] = x30;
        buf[12] = x32;
        buf[13] = x34;
        buf[14] = x36;
        buf[15] = x37;
        buf[16] = x38;
        buf[17] = x40;
        buf[18] = x42;
        buf[19] = x44;
        buf[20] = x46;
        buf[21] = x48;
        buf[22] = x50;
        buf[23] = x51;
        buf[24] = x52;
        buf[25] = x54;
        buf[26] = x56;
        buf[27] = x58;
        buf[28] = x60;
        buf[29] = x62;
        buf[30] = x64;
        buf[31] = x65;
        buf[32] = x66;
        buf[33] = x68;
        buf[34] = x70;
        buf[35] = x72;
        buf[36] = x74;
        buf[37] = x76;
        buf[38] = x78;
        buf[39] = x79;
        buf[40] = x80;
        buf[41] = x82;
        buf[42] = x84;
        buf[43] = x86;
        buf[44] = x88;
        buf[45] = x90;
        buf[46] = x92;
        buf[47] = x93;
        buf[48] = x94;
        buf[49] = x96;
        buf[50] = x98;
        buf[51] = x100;
        buf[52] = x102;
        buf[53] = x104;
        buf[54] = x106;
        buf[55] = x107;
        buf[56] = x108;
        buf[57] = x110;
        buf[58] = x112;
        buf[59] = x114;
        buf[60] = x116;
        buf[61] = x118;
        buf[62] = x120;
        buf[63] = x121;
        buf[64] = x122;
        buf[65] = x123;
    }
    pub(super) fn add(a1: &Element, a2: &Element) -> Element {
        let (x1, x2) = cm::add(a1.0[0], a2.0[0], 0);
        let (x3, x4) = cm::add(a1.0[1], a2.0[1], x2);
        let (x5, x6) = cm::add(a1.0[2], a2.0[2], x4);
        let (x7, x8) = cm::add(a1.0[3], a2.0[3], x6);
        let (x9, x10) = cm::add(a1.0[4], a2.0[4], x8);
        let (x11, x12) = cm::add(a1.0[5], a2.0[5], x10);
        let (x13, x14) = cm::add(a1.0[6], a2.0[6], x12);
        let (x15, x16) = cm::add(a1.0[7], a2.0[7], x14);
        let (x17, x18) = cm::add(a1.0[8], a2.0[8], x16);
        let (x19, x20) = cm::sub(x1, 0xFFFFFFFFFFFFFFFF, 0);
        let (x21, x22) = cm::sub(x3, 0xFFFFFFFFFFFFFFFF, x20);
        let (x23, x24) = cm::sub(x5, 0xFFFFFFFFFFFFFFFF, x22);
        let (x25, x26) = cm::sub(x7, 0xFFFFFFFFFFFFFFFF, x24);
        let (x27, x28) = cm::sub(x9, 0xFFFFFFFFFFFFFFFF, x26);
        let (x29, x30) = cm::sub(x11, 0xFFFFFFFFFFFFFFFF, x28);
        let (x31, x32) = cm::sub(x13, 0xFFFFFFFFFFFFFFFF, x30);
        let (x33, x34) = cm::sub(x15, 0xFFFFFFFFFFFFFFFF, x32);
        let (x35, x36) = cm::sub(x17, 0x1FF, x34);
        let (_, x38) = cm::sub(x18, 0, x36);
        let x39 = cm::mov(x38, x19, x1);
        let x40 = cm::mov(x38, x21, x3);
        let x41 = cm::mov(x38, x23, x5);
        let x42 = cm::mov(x38, x25, x7);
        let x43 = cm::mov(x38, x27, x9);
        let x44 = cm::mov(x38, x29, x11);
        let x45 = cm::mov(x38, x31, x13);
        let x46 = cm::mov(x38, x33, x15);
        let x47 = cm::mov(x38, x35, x17);
        Element([x39, x40, x41, x42, x43, x44, x45, x46, x47])
    }
    pub(super) fn sub(a1: &Element, a2: &Element) -> Element {
        let (x1, x2) = cm::sub(a1.0[0], a2.0[0], 0);
        let (x3, x4) = cm::sub(a1.0[1], a2.0[1], x2);
        let (x5, x6) = cm::sub(a1.0[2], a2.0[2], x4);
        let (x7, x8) = cm::sub(a1.0[3], a2.0[3], x6);
        let (x9, x10) = cm::sub(a1.0[4], a2.0[4], x8);
        let (x11, x12) = cm::sub(a1.0[5], a2.0[5], x10);
        let (x13, x14) = cm::sub(a1.0[6], a2.0[6], x12);
        let (x15, x16) = cm::sub(a1.0[7], a2.0[7], x14);
        let (x17, x18) = cm::sub(a1.0[8], a2.0[8], x16);
        let x19 = cm::mov(x18, 0x0, 0xFFFFFFFFFFFFFFFF);
        let (x20, x21) = cm::add(x1, x19, 0);
        let (x22, x23) = cm::add(x3, x19, x21);
        let (x24, x25) = cm::add(x5, x19, x23);
        let (x26, x27) = cm::add(x7, x19, x25);
        let (x28, x29) = cm::add(x9, x19, x27);
        let (x30, x31) = cm::add(x11, x19, x29);
        let (x32, x33) = cm::add(x13, x19, x31);
        let (x34, x35) = cm::add(x15, x19, x33);
        let (x36, _) = cm::add(x17, x19 & 0x1FF, x35);
        Element([x20, x22, x24, x26, x28, x30, x32, x34, x36])
    }
    pub(super) fn mul(a1: &Element, a2: &Element) -> Element {
        let (x11, x10) = cm::mul(a1.0[0], a2.0[8]);
        let (x13, x12) = cm::mul(a1.0[0], a2.0[7]);
        let (x15, x14) = cm::mul(a1.0[0], a2.0[6]);
        let (x17, x16) = cm::mul(a1.0[0], a2.0[5]);
        let (x19, x18) = cm::mul(a1.0[0], a2.0[4]);
        let (x21, x20) = cm::mul(a1.0[0], a2.0[3]);
        let (x23, x22) = cm::mul(a1.0[0], a2.0[2]);
        let (x25, x24) = cm::mul(a1.0[0], a2.0[1]);
        let (x27, x26) = cm::mul(a1.0[0], a2.0[0]);
        let (x28, x29) = cm::add(x27, x24, 0);
        let (x30, x31) = cm::add(x25, x22, x29);
        let (x32, x33) = cm::add(x23, x20, x31);
        let (x34, x35) = cm::add(x21, x18, x33);
        let (x36, x37) = cm::add(x19, x16, x35);
        let (x38, x39) = cm::add(x17, x14, x37);
        let (x40, x41) = cm::add(x15, x12, x39);
        let (x42, x43) = cm::add(x13, x10, x41);
        let x44 = x43 + x11;
        let (x46, x45) = cm::mul(x26, 0x1FF);
        let (x48, x47) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x50, x49) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x52, x51) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x54, x53) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x56, x55) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x58, x57) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x60, x59) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x62, x61) = cm::mul(x26, 0xFFFFFFFFFFFFFFFF);
        let (x63, x64) = cm::add(x62, x59, 0);
        let (x65, x66) = cm::add(x60, x57, x64);
        let (x67, x68) = cm::add(x58, x55, x66);
        let (x69, x70) = cm::add(x56, x53, x68);
        let (x71, x72) = cm::add(x54, x51, x70);
        let (x73, x74) = cm::add(x52, x49, x72);
        let (x75, x76) = cm::add(x50, x47, x74);
        let (x77, x78) = cm::add(x48, x45, x76);
        let x79 = x78 + x46;
        let (_, x81) = cm::add(x26, x61, 0);
        let (x82, x83) = cm::add(x28, x63, x81);
        let (x84, x85) = cm::add(x30, x65, x83);
        let (x86, x87) = cm::add(x32, x67, x85);
        let (x88, x89) = cm::add(x34, x69, x87);
        let (x90, x91) = cm::add(x36, x71, x89);
        let (x92, x93) = cm::add(x38, x73, x91);
        let (x94, x95) = cm::add(x40, x75, x93);
        let (x96, x97) = cm::add(x42, x77, x95);
        let (x98, x99) = cm::add(x44, x79, x97);
        let (x101, x100) = cm::mul(a1.0[1], a2.0[8]);
        let (x103, x102) = cm::mul(a1.0[1], a2.0[7]);
        let (x105, x104) = cm::mul(a1.0[1], a2.0[6]);
        let (x107, x106) = cm::mul(a1.0[1], a2.0[5]);
        let (x109, x108) = cm::mul(a1.0[1], a2.0[4]);
        let (x111, x110) = cm::mul(a1.0[1], a2.0[3]);
        let (x113, x112) = cm::mul(a1.0[1], a2.0[2]);
        let (x115, x114) = cm::mul(a1.0[1], a2.0[1]);
        let (x117, x116) = cm::mul(a1.0[1], a2.0[0]);
        let (x118, x119) = cm::add(x117, x114, 0);
        let (x120, x121) = cm::add(x115, x112, x119);
        let (x122, x123) = cm::add(x113, x110, x121);
        let (x124, x125) = cm::add(x111, x108, x123);
        let (x126, x127) = cm::add(x109, x106, x125);
        let (x128, x129) = cm::add(x107, x104, x127);
        let (x130, x131) = cm::add(x105, x102, x129);
        let (x132, x133) = cm::add(x103, x100, x131);
        let x134 = x133 + x101;
        let (x135, x136) = cm::add(x82, x116, 0);
        let (x137, x138) = cm::add(x84, x118, x136);
        let (x139, x140) = cm::add(x86, x120, x138);
        let (x141, x142) = cm::add(x88, x122, x140);
        let (x143, x144) = cm::add(x90, x124, x142);
        let (x145, x146) = cm::add(x92, x126, x144);
        let (x147, x148) = cm::add(x94, x128, x146);
        let (x149, x150) = cm::add(x96, x130, x148);
        let (x151, x152) = cm::add(x98, x132, x150);
        let (x153, x154) = cm::add(x99, x134, x152);
        let (x156, x155) = cm::mul(x135, 0x1FF);
        let (x158, x157) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x160, x159) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x162, x161) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x164, x163) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x166, x165) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x168, x167) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x170, x169) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x172, x171) = cm::mul(x135, 0xFFFFFFFFFFFFFFFF);
        let (x173, x174) = cm::add(x172, x169, 0);
        let (x175, x176) = cm::add(x170, x167, x174);
        let (x177, x178) = cm::add(x168, x165, x176);
        let (x179, x180) = cm::add(x166, x163, x178);
        let (x181, x182) = cm::add(x164, x161, x180);
        let (x183, x184) = cm::add(x162, x159, x182);
        let (x185, x186) = cm::add(x160, x157, x184);
        let (x187, x188) = cm::add(x158, x155, x186);
        let x189 = x188 + x156;
        let (_, x191) = cm::add(x135, x171, 0);
        let (x192, x193) = cm::add(x137, x173, x191);
        let (x194, x195) = cm::add(x139, x175, x193);
        let (x196, x197) = cm::add(x141, x177, x195);
        let (x198, x199) = cm::add(x143, x179, x197);
        let (x200, x201) = cm::add(x145, x181, x199);
        let (x202, x203) = cm::add(x147, x183, x201);
        let (x204, x205) = cm::add(x149, x185, x203);
        let (x206, x207) = cm::add(x151, x187, x205);
        let (x208, x209) = cm::add(x153, x189, x207);
        let x210 = x209 + x154;
        let (x212, x211) = cm::mul(a1.0[2], a2.0[8]);
        let (x214, x213) = cm::mul(a1.0[2], a2.0[7]);
        let (x216, x215) = cm::mul(a1.0[2], a2.0[6]);
        let (x218, x217) = cm::mul(a1.0[2], a2.0[5]);
        let (x220, x219) = cm::mul(a1.0[2], a2.0[4]);
        let (x222, x221) = cm::mul(a1.0[2], a2.0[3]);
        let (x224, x223) = cm::mul(a1.0[2], a2.0[2]);
        let (x226, x225) = cm::mul(a1.0[2], a2.0[1]);
        let (x228, x227) = cm::mul(a1.0[2], a2.0[0]);
        let (x229, x230) = cm::add(x228, x225, 0);
        let (x231, x232) = cm::add(x226, x223, x230);
        let (x233, x234) = cm::add(x224, x221, x232);
        let (x235, x236) = cm::add(x222, x219, x234);
        let (x237, x238) = cm::add(x220, x217, x236);
        let (x239, x240) = cm::add(x218, x215, x238);
        let (x241, x242) = cm::add(x216, x213, x240);
        let (x243, x244) = cm::add(x214, x211, x242);
        let x245 = x244 + x212;
        let (x246, x247) = cm::add(x192, x227, 0);
        let (x248, x249) = cm::add(x194, x229, x247);
        let (x250, x251) = cm::add(x196, x231, x249);
        let (x252, x253) = cm::add(x198, x233, x251);
        let (x254, x255) = cm::add(x200, x235, x253);
        let (x256, x257) = cm::add(x202, x237, x255);
        let (x258, x259) = cm::add(x204, x239, x257);
        let (x260, x261) = cm::add(x206, x241, x259);
        let (x262, x263) = cm::add(x208, x243, x261);
        let (x264, x265) = cm::add(x210, x245, x263);
        let (x267, x266) = cm::mul(x246, 0x1FF);
        let (x269, x268) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x271, x270) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x273, x272) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x275, x274) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x277, x276) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x279, x278) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x281, x280) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x283, x282) = cm::mul(x246, 0xFFFFFFFFFFFFFFFF);
        let (x284, x285) = cm::add(x283, x280, 0);
        let (x286, x287) = cm::add(x281, x278, x285);
        let (x288, x289) = cm::add(x279, x276, x287);
        let (x290, x291) = cm::add(x277, x274, x289);
        let (x292, x293) = cm::add(x275, x272, x291);
        let (x294, x295) = cm::add(x273, x270, x293);
        let (x296, x297) = cm::add(x271, x268, x295);
        let (x298, x299) = cm::add(x269, x266, x297);
        let x300 = x299 + x267;
        let (_, x302) = cm::add(x246, x282, 0);
        let (x303, x304) = cm::add(x248, x284, x302);
        let (x305, x306) = cm::add(x250, x286, x304);
        let (x307, x308) = cm::add(x252, x288, x306);
        let (x309, x310) = cm::add(x254, x290, x308);
        let (x311, x312) = cm::add(x256, x292, x310);
        let (x313, x314) = cm::add(x258, x294, x312);
        let (x315, x316) = cm::add(x260, x296, x314);
        let (x317, x318) = cm::add(x262, x298, x316);
        let (x319, x320) = cm::add(x264, x300, x318);
        let x321 = x320 + x265;
        let (x323, x322) = cm::mul(a1.0[3], a2.0[8]);
        let (x325, x324) = cm::mul(a1.0[3], a2.0[7]);
        let (x327, x326) = cm::mul(a1.0[3], a2.0[6]);
        let (x329, x328) = cm::mul(a1.0[3], a2.0[5]);
        let (x331, x330) = cm::mul(a1.0[3], a2.0[4]);
        let (x333, x332) = cm::mul(a1.0[3], a2.0[3]);
        let (x335, x334) = cm::mul(a1.0[3], a2.0[2]);
        let (x337, x336) = cm::mul(a1.0[3], a2.0[1]);
        let (x339, x338) = cm::mul(a1.0[3], a2.0[0]);
        let (x340, x341) = cm::add(x339, x336, 0);
        let (x342, x343) = cm::add(x337, x334, x341);
        let (x344, x345) = cm::add(x335, x332, x343);
        let (x346, x347) = cm::add(x333, x330, x345);
        let (x348, x349) = cm::add(x331, x328, x347);
        let (x350, x351) = cm::add(x329, x326, x349);
        let (x352, x353) = cm::add(x327, x324, x351);
        let (x354, x355) = cm::add(x325, x322, x353);
        let x356 = x355 + x323;
        let (x357, x358) = cm::add(x303, x338, 0);
        let (x359, x360) = cm::add(x305, x340, x358);
        let (x361, x362) = cm::add(x307, x342, x360);
        let (x363, x364) = cm::add(x309, x344, x362);
        let (x365, x366) = cm::add(x311, x346, x364);
        let (x367, x368) = cm::add(x313, x348, x366);
        let (x369, x370) = cm::add(x315, x350, x368);
        let (x371, x372) = cm::add(x317, x352, x370);
        let (x373, x374) = cm::add(x319, x354, x372);
        let (x375, x376) = cm::add(x321, x356, x374);
        let (x378, x377) = cm::mul(x357, 0x1FF);
        let (x380, x379) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x382, x381) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x384, x383) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x386, x385) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x388, x387) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x390, x389) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x392, x391) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x394, x393) = cm::mul(x357, 0xFFFFFFFFFFFFFFFF);
        let (x395, x396) = cm::add(x394, x391, 0);
        let (x397, x398) = cm::add(x392, x389, x396);
        let (x399, x400) = cm::add(x390, x387, x398);
        let (x401, x402) = cm::add(x388, x385, x400);
        let (x403, x404) = cm::add(x386, x383, x402);
        let (x405, x406) = cm::add(x384, x381, x404);
        let (x407, x408) = cm::add(x382, x379, x406);
        let (x409, x410) = cm::add(x380, x377, x408);
        let x411 = x410 + x378;
        let (_, x413) = cm::add(x357, x393, 0);
        let (x414, x415) = cm::add(x359, x395, x413);
        let (x416, x417) = cm::add(x361, x397, x415);
        let (x418, x419) = cm::add(x363, x399, x417);
        let (x420, x421) = cm::add(x365, x401, x419);
        let (x422, x423) = cm::add(x367, x403, x421);
        let (x424, x425) = cm::add(x369, x405, x423);
        let (x426, x427) = cm::add(x371, x407, x425);
        let (x428, x429) = cm::add(x373, x409, x427);
        let (x430, x431) = cm::add(x375, x411, x429);
        let x432 = x431 + x376;
        let (x434, x433) = cm::mul(a1.0[4], a2.0[8]);
        let (x436, x435) = cm::mul(a1.0[4], a2.0[7]);
        let (x438, x437) = cm::mul(a1.0[4], a2.0[6]);
        let (x440, x439) = cm::mul(a1.0[4], a2.0[5]);
        let (x442, x441) = cm::mul(a1.0[4], a2.0[4]);
        let (x444, x443) = cm::mul(a1.0[4], a2.0[3]);
        let (x446, x445) = cm::mul(a1.0[4], a2.0[2]);
        let (x448, x447) = cm::mul(a1.0[4], a2.0[1]);
        let (x450, x449) = cm::mul(a1.0[4], a2.0[0]);
        let (x451, x452) = cm::add(x450, x447, 0);
        let (x453, x454) = cm::add(x448, x445, x452);
        let (x455, x456) = cm::add(x446, x443, x454);
        let (x457, x458) = cm::add(x444, x441, x456);
        let (x459, x460) = cm::add(x442, x439, x458);
        let (x461, x462) = cm::add(x440, x437, x460);
        let (x463, x464) = cm::add(x438, x435, x462);
        let (x465, x466) = cm::add(x436, x433, x464);
        let x467 = x466 + x434;
        let (x468, x469) = cm::add(x414, x449, 0);
        let (x470, x471) = cm::add(x416, x451, x469);
        let (x472, x473) = cm::add(x418, x453, x471);
        let (x474, x475) = cm::add(x420, x455, x473);
        let (x476, x477) = cm::add(x422, x457, x475);
        let (x478, x479) = cm::add(x424, x459, x477);
        let (x480, x481) = cm::add(x426, x461, x479);
        let (x482, x483) = cm::add(x428, x463, x481);
        let (x484, x485) = cm::add(x430, x465, x483);
        let (x486, x487) = cm::add(x432, x467, x485);
        let (x489, x488) = cm::mul(x468, 0x1FF);
        let (x491, x490) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x493, x492) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x495, x494) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x497, x496) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x499, x498) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x501, x500) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x503, x502) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x505, x504) = cm::mul(x468, 0xFFFFFFFFFFFFFFFF);
        let (x506, x507) = cm::add(x505, x502, 0);
        let (x508, x509) = cm::add(x503, x500, x507);
        let (x510, x511) = cm::add(x501, x498, x509);
        let (x512, x513) = cm::add(x499, x496, x511);
        let (x514, x515) = cm::add(x497, x494, x513);
        let (x516, x517) = cm::add(x495, x492, x515);
        let (x518, x519) = cm::add(x493, x490, x517);
        let (x520, x521) = cm::add(x491, x488, x519);
        let x522 = x521 + x489;
        let (_, x524) = cm::add(x468, x504, 0);
        let (x525, x526) = cm::add(x470, x506, x524);
        let (x527, x528) = cm::add(x472, x508, x526);
        let (x529, x530) = cm::add(x474, x510, x528);
        let (x531, x532) = cm::add(x476, x512, x530);
        let (x533, x534) = cm::add(x478, x514, x532);
        let (x535, x536) = cm::add(x480, x516, x534);
        let (x537, x538) = cm::add(x482, x518, x536);
        let (x539, x540) = cm::add(x484, x520, x538);
        let (x541, x542) = cm::add(x486, x522, x540);
        let x543 = x542 + x487;
        let (x545, x544) = cm::mul(a1.0[5], a2.0[8]);
        let (x547, x546) = cm::mul(a1.0[5], a2.0[7]);
        let (x549, x548) = cm::mul(a1.0[5], a2.0[6]);
        let (x551, x550) = cm::mul(a1.0[5], a2.0[5]);
        let (x553, x552) = cm::mul(a1.0[5], a2.0[4]);
        let (x555, x554) = cm::mul(a1.0[5], a2.0[3]);
        let (x557, x556) = cm::mul(a1.0[5], a2.0[2]);
        let (x559, x558) = cm::mul(a1.0[5], a2.0[1]);
        let (x561, x560) = cm::mul(a1.0[5], a2.0[0]);
        let (x562, x563) = cm::add(x561, x558, 0);
        let (x564, x565) = cm::add(x559, x556, x563);
        let (x566, x567) = cm::add(x557, x554, x565);
        let (x568, x569) = cm::add(x555, x552, x567);
        let (x570, x571) = cm::add(x553, x550, x569);
        let (x572, x573) = cm::add(x551, x548, x571);
        let (x574, x575) = cm::add(x549, x546, x573);
        let (x576, x577) = cm::add(x547, x544, x575);
        let x578 = x577 + x545;
        let (x579, x580) = cm::add(x525, x560, 0);
        let (x581, x582) = cm::add(x527, x562, x580);
        let (x583, x584) = cm::add(x529, x564, x582);
        let (x585, x586) = cm::add(x531, x566, x584);
        let (x587, x588) = cm::add(x533, x568, x586);
        let (x589, x590) = cm::add(x535, x570, x588);
        let (x591, x592) = cm::add(x537, x572, x590);
        let (x593, x594) = cm::add(x539, x574, x592);
        let (x595, x596) = cm::add(x541, x576, x594);
        let (x597, x598) = cm::add(x543, x578, x596);
        let (x600, x599) = cm::mul(x579, 0x1FF);
        let (x602, x601) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x604, x603) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x606, x605) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x608, x607) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x610, x609) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x612, x611) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x614, x613) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x616, x615) = cm::mul(x579, 0xFFFFFFFFFFFFFFFF);
        let (x617, x618) = cm::add(x616, x613, 0);
        let (x619, x620) = cm::add(x614, x611, x618);
        let (x621, x622) = cm::add(x612, x609, x620);
        let (x623, x624) = cm::add(x610, x607, x622);
        let (x625, x626) = cm::add(x608, x605, x624);
        let (x627, x628) = cm::add(x606, x603, x626);
        let (x629, x630) = cm::add(x604, x601, x628);
        let (x631, x632) = cm::add(x602, x599, x630);
        let x633 = x632 + x600;
        let (_, x635) = cm::add(x579, x615, 0);
        let (x636, x637) = cm::add(x581, x617, x635);
        let (x638, x639) = cm::add(x583, x619, x637);
        let (x640, x641) = cm::add(x585, x621, x639);
        let (x642, x643) = cm::add(x587, x623, x641);
        let (x644, x645) = cm::add(x589, x625, x643);
        let (x646, x647) = cm::add(x591, x627, x645);
        let (x648, x649) = cm::add(x593, x629, x647);
        let (x650, x651) = cm::add(x595, x631, x649);
        let (x652, x653) = cm::add(x597, x633, x651);
        let x654 = x653 + x598;
        let (x656, x655) = cm::mul(a1.0[6], a2.0[8]);
        let (x658, x657) = cm::mul(a1.0[6], a2.0[7]);
        let (x660, x659) = cm::mul(a1.0[6], a2.0[6]);
        let (x662, x661) = cm::mul(a1.0[6], a2.0[5]);
        let (x664, x663) = cm::mul(a1.0[6], a2.0[4]);
        let (x666, x665) = cm::mul(a1.0[6], a2.0[3]);
        let (x668, x667) = cm::mul(a1.0[6], a2.0[2]);
        let (x670, x669) = cm::mul(a1.0[6], a2.0[1]);
        let (x672, x671) = cm::mul(a1.0[6], a2.0[0]);
        let (x673, x674) = cm::add(x672, x669, 0);
        let (x675, x676) = cm::add(x670, x667, x674);
        let (x677, x678) = cm::add(x668, x665, x676);
        let (x679, x680) = cm::add(x666, x663, x678);
        let (x681, x682) = cm::add(x664, x661, x680);
        let (x683, x684) = cm::add(x662, x659, x682);
        let (x685, x686) = cm::add(x660, x657, x684);
        let (x687, x688) = cm::add(x658, x655, x686);
        let x689 = x688 + x656;
        let (x690, x691) = cm::add(x636, x671, 0);
        let (x692, x693) = cm::add(x638, x673, x691);
        let (x694, x695) = cm::add(x640, x675, x693);
        let (x696, x697) = cm::add(x642, x677, x695);
        let (x698, x699) = cm::add(x644, x679, x697);
        let (x700, x701) = cm::add(x646, x681, x699);
        let (x702, x703) = cm::add(x648, x683, x701);
        let (x704, x705) = cm::add(x650, x685, x703);
        let (x706, x707) = cm::add(x652, x687, x705);
        let (x708, x709) = cm::add(x654, x689, x707);
        let (x711, x710) = cm::mul(x690, 0x1FF);
        let (x713, x712) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x715, x714) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x717, x716) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x719, x718) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x721, x720) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x723, x722) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x725, x724) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x727, x726) = cm::mul(x690, 0xFFFFFFFFFFFFFFFF);
        let (x728, x729) = cm::add(x727, x724, 0);
        let (x730, x731) = cm::add(x725, x722, x729);
        let (x732, x733) = cm::add(x723, x720, x731);
        let (x734, x735) = cm::add(x721, x718, x733);
        let (x736, x737) = cm::add(x719, x716, x735);
        let (x738, x739) = cm::add(x717, x714, x737);
        let (x740, x741) = cm::add(x715, x712, x739);
        let (x742, x743) = cm::add(x713, x710, x741);
        let x744 = x743 + x711;
        let (_, x746) = cm::add(x690, x726, 0);
        let (x747, x748) = cm::add(x692, x728, x746);
        let (x749, x750) = cm::add(x694, x730, x748);
        let (x751, x752) = cm::add(x696, x732, x750);
        let (x753, x754) = cm::add(x698, x734, x752);
        let (x755, x756) = cm::add(x700, x736, x754);
        let (x757, x758) = cm::add(x702, x738, x756);
        let (x759, x760) = cm::add(x704, x740, x758);
        let (x761, x762) = cm::add(x706, x742, x760);
        let (x763, x764) = cm::add(x708, x744, x762);
        let x765 = x764 + x709;
        let (x767, x766) = cm::mul(a1.0[7], a2.0[8]);
        let (x769, x768) = cm::mul(a1.0[7], a2.0[7]);
        let (x771, x770) = cm::mul(a1.0[7], a2.0[6]);
        let (x773, x772) = cm::mul(a1.0[7], a2.0[5]);
        let (x775, x774) = cm::mul(a1.0[7], a2.0[4]);
        let (x777, x776) = cm::mul(a1.0[7], a2.0[3]);
        let (x779, x778) = cm::mul(a1.0[7], a2.0[2]);
        let (x781, x780) = cm::mul(a1.0[7], a2.0[1]);
        let (x783, x782) = cm::mul(a1.0[7], a2.0[0]);
        let (x784, x785) = cm::add(x783, x780, 0);
        let (x786, x787) = cm::add(x781, x778, x785);
        let (x788, x789) = cm::add(x779, x776, x787);
        let (x790, x791) = cm::add(x777, x774, x789);
        let (x792, x793) = cm::add(x775, x772, x791);
        let (x794, x795) = cm::add(x773, x770, x793);
        let (x796, x797) = cm::add(x771, x768, x795);
        let (x798, x799) = cm::add(x769, x766, x797);
        let x800 = x799 + x767;
        let (x801, x802) = cm::add(x747, x782, 0);
        let (x803, x804) = cm::add(x749, x784, x802);
        let (x805, x806) = cm::add(x751, x786, x804);
        let (x807, x808) = cm::add(x753, x788, x806);
        let (x809, x810) = cm::add(x755, x790, x808);
        let (x811, x812) = cm::add(x757, x792, x810);
        let (x813, x814) = cm::add(x759, x794, x812);
        let (x815, x816) = cm::add(x761, x796, x814);
        let (x817, x818) = cm::add(x763, x798, x816);
        let (x819, x820) = cm::add(x765, x800, x818);
        let (x822, x821) = cm::mul(x801, 0x1FF);
        let (x824, x823) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x826, x825) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x828, x827) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x830, x829) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x832, x831) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x834, x833) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x836, x835) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x838, x837) = cm::mul(x801, 0xFFFFFFFFFFFFFFFF);
        let (x839, x840) = cm::add(x838, x835, 0);
        let (x841, x842) = cm::add(x836, x833, x840);
        let (x843, x844) = cm::add(x834, x831, x842);
        let (x845, x846) = cm::add(x832, x829, x844);
        let (x847, x848) = cm::add(x830, x827, x846);
        let (x849, x850) = cm::add(x828, x825, x848);
        let (x851, x852) = cm::add(x826, x823, x850);
        let (x853, x854) = cm::add(x824, x821, x852);
        let x855 = x854 + x822;
        let (_, x857) = cm::add(x801, x837, 0);
        let (x858, x859) = cm::add(x803, x839, x857);
        let (x860, x861) = cm::add(x805, x841, x859);
        let (x862, x863) = cm::add(x807, x843, x861);
        let (x864, x865) = cm::add(x809, x845, x863);
        let (x866, x867) = cm::add(x811, x847, x865);
        let (x868, x869) = cm::add(x813, x849, x867);
        let (x870, x871) = cm::add(x815, x851, x869);
        let (x872, x873) = cm::add(x817, x853, x871);
        let (x874, x875) = cm::add(x819, x855, x873);
        let x876 = x875 + x820;
        let (x878, x877) = cm::mul(a1.0[8], a2.0[8]);
        let (x880, x879) = cm::mul(a1.0[8], a2.0[7]);
        let (x882, x881) = cm::mul(a1.0[8], a2.0[6]);
        let (x884, x883) = cm::mul(a1.0[8], a2.0[5]);
        let (x886, x885) = cm::mul(a1.0[8], a2.0[4]);
        let (x888, x887) = cm::mul(a1.0[8], a2.0[3]);
        let (x890, x889) = cm::mul(a1.0[8], a2.0[2]);
        let (x892, x891) = cm::mul(a1.0[8], a2.0[1]);
        let (x894, x893) = cm::mul(a1.0[8], a2.0[0]);
        let (x895, x896) = cm::add(x894, x891, 0);
        let (x897, x898) = cm::add(x892, x889, x896);
        let (x899, x900) = cm::add(x890, x887, x898);
        let (x901, x902) = cm::add(x888, x885, x900);
        let (x903, x904) = cm::add(x886, x883, x902);
        let (x905, x906) = cm::add(x884, x881, x904);
        let (x907, x908) = cm::add(x882, x879, x906);
        let (x909, x910) = cm::add(x880, x877, x908);
        let x911 = x910 + x878;
        let (x912, x913) = cm::add(x858, x893, 0);
        let (x914, x915) = cm::add(x860, x895, x913);
        let (x916, x917) = cm::add(x862, x897, x915);
        let (x918, x919) = cm::add(x864, x899, x917);
        let (x920, x921) = cm::add(x866, x901, x919);
        let (x922, x923) = cm::add(x868, x903, x921);
        let (x924, x925) = cm::add(x870, x905, x923);
        let (x926, x927) = cm::add(x872, x907, x925);
        let (x928, x929) = cm::add(x874, x909, x927);
        let (x930, x931) = cm::add(x876, x911, x929);
        let (x933, x932) = cm::mul(x912, 0x1FF);
        let (x935, x934) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x937, x936) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x939, x938) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x941, x940) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x943, x942) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x945, x944) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x947, x946) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x949, x948) = cm::mul(x912, 0xFFFFFFFFFFFFFFFF);
        let (x950, x951) = cm::add(x949, x946, 0);
        let (x952, x953) = cm::add(x947, x944, x951);
        let (x954, x955) = cm::add(x945, x942, x953);
        let (x956, x957) = cm::add(x943, x940, x955);
        let (x958, x959) = cm::add(x941, x938, x957);
        let (x960, x961) = cm::add(x939, x936, x959);
        let (x962, x963) = cm::add(x937, x934, x961);
        let (x964, x965) = cm::add(x935, x932, x963);
        let x966 = x965 + x933;
        let (_, x968) = cm::add(x912, x948, 0);
        let (x969, x970) = cm::add(x914, x950, x968);
        let (x971, x972) = cm::add(x916, x952, x970);
        let (x973, x974) = cm::add(x918, x954, x972);
        let (x975, x976) = cm::add(x920, x956, x974);
        let (x977, x978) = cm::add(x922, x958, x976);
        let (x979, x980) = cm::add(x924, x960, x978);
        let (x981, x982) = cm::add(x926, x962, x980);
        let (x983, x984) = cm::add(x928, x964, x982);
        let (x985, x986) = cm::add(x930, x966, x984);
        let x987 = x986 + x931;
        let (x988, x989) = cm::sub(x969, 0xFFFFFFFFFFFFFFFF, 0);
        let (x990, x991) = cm::sub(x971, 0xFFFFFFFFFFFFFFFF, x989);
        let (x992, x993) = cm::sub(x973, 0xFFFFFFFFFFFFFFFF, x991);
        let (x994, x995) = cm::sub(x975, 0xFFFFFFFFFFFFFFFF, x993);
        let (x996, x997) = cm::sub(x977, 0xFFFFFFFFFFFFFFFF, x995);
        let (x998, x999) = cm::sub(x979, 0xFFFFFFFFFFFFFFFF, x997);
        let (x1000, x1001) = cm::sub(x981, 0xFFFFFFFFFFFFFFFF, x999);
        let (x1002, x1003) = cm::sub(x983, 0xFFFFFFFFFFFFFFFF, x1001);
        let (x1004, x1005) = cm::sub(x985, 0x1FF, x1003);
        let (_, x1007) = cm::sub(x987, 0x0, x1005);
        let x1008 = cm::mov(x1007, x988, x969);
        let x1009 = cm::mov(x1007, x990, x971);
        let x1010 = cm::mov(x1007, x992, x973);
        let x1011 = cm::mov(x1007, x994, x975);
        let x1012 = cm::mov(x1007, x996, x977);
        let x1013 = cm::mov(x1007, x998, x979);
        let x1014 = cm::mov(x1007, x1000, x981);
        let x1015 = cm::mov(x1007, x1002, x983);
        let x1016 = cm::mov(x1007, x1004, x985);
        Element([x1008, x1009, x1010, x1011, x1012, x1013, x1014, x1015, x1016])
    }
    #[inline]
    pub(super) fn select(a1: u64, a2: &Element, a3: &Element) -> Element {
        Element([
            cm::mov(a1, a2.0[0], a3.0[0]),
            cm::mov(a1, a2.0[1], a3.0[1]),
            cm::mov(a1, a2.0[2], a3.0[2]),
            cm::mov(a1, a2.0[3], a3.0[3]),
            cm::mov(a1, a2.0[4], a3.0[4]),
            cm::mov(a1, a2.0[5], a3.0[5]),
            cm::mov(a1, a2.0[6], a3.0[6]),
            cm::mov(a1, a2.0[7], a3.0[7]),
            cm::mov(a1, a2.0[8], a3.0[8]),
        ])
    }
}

#[path = "table.rs"]
mod table;

#[inline]
pub fn generate_secret(public: &[u8], private: &[u8], secret: &mut [u8]) -> io::Result<()> {
    if secret.len() < 65 {
        return Err(ErrorKind::InvalidInput.into());
    }
    Point::try_from(public)?.scalar_mul(&private).write_secret(secret)
}
#[inline]
pub fn generate_pair(src: &mut impl Read, public: &mut [u8], private: &mut [u8]) -> io::Result<()> {
    if private.len() < 66 {
        return Err(ErrorKind::InvalidInput.into());
    }
    loop {
        src.read_exact(private)?;
        private[0] &= 0x01;
        private[1] ^= 0x42;
        if Num::from_bytes(&private).less_than(&Point::N) {
            break;
        }
    }
    Point::scalar_base_mul(&private).write(public)
}
