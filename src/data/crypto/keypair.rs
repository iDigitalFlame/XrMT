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

use alloc::alloc::Global;
use core::alloc::Allocator;
use core::ops::{Deref, Range};
use core::slice::from_raw_parts_mut;

use crate::data::crypto::p521;
use crate::data::{read_full, write_full};
use crate::ignore_error;
use crate::io::{self, Read, Write};
use crate::prelude::*;

const SIZE: usize = 0x108usize;

const SHARED: Range<usize> = Range {
    end:   0x108usize,
    start: 0xC7usize,
};
const PUBLIC: Range<usize> = Range {
    end:   0x85usize,
    start: 0x0usize,
};
const PRIVATE: Range<usize> = Range {
    end:   0xC7usize,
    start: 0x85usize,
};

pub struct PublicKey<'a>(&'a [u8]);
pub struct PrivateKey<'a>(&'a [u8]);
pub struct SharedKeys<'a>(&'a [u8]);
pub struct KeyPair<A: Allocator = Global> {
    // NOTE(dij): We use a single Vec as the backing store for the all the
    //            key slices. This also saves it on the heap and allows it
    //            to be encrypted in memory.
    inner: Vec<u8, A>,
}

impl KeyPair {
    #[inline]
    pub fn empty() -> KeyPair {
        let mut k = KeyPair { inner: Vec::with_capacity(SIZE) };
        unsafe { k.inner.set_len(SIZE) };
        k
    }
    #[inline]
    pub fn new(src: &mut impl Read) -> KeyPair {
        KeyPair::new_in(src, Global)
    }
}
impl PublicKey<'_> {
    #[inline]
    pub fn hash(&self) -> u32 {
        let mut h: u32 = 0x811C9DC5;
        for i in self.0.iter() {
            h = h.wrapping_mul(0x1000193);
            h ^= *i as u32;
        }
        h
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        !self.0.iter().any(|v| *v > 0)
    }
}
impl<A: Allocator> KeyPair<A> {
    #[inline]
    pub fn empty_in(alloc: A) -> KeyPair<A> {
        let mut k = KeyPair {
            inner: Vec::with_capacity_in(SIZE, alloc),
        };
        unsafe { k.inner.set_len(SIZE) };
        k
    }
    #[inline]
    pub fn new_in(src: &mut impl Read, alloc: A) -> KeyPair<A> {
        let mut k = KeyPair {
            inner: Vec::with_capacity_in(SIZE, alloc),
        };
        unsafe { k.inner.set_len(SIZE) };
        k.fill(src);
        k
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.public_key().is_empty()
    }
    #[inline]
    pub fn is_synced(&self) -> bool {
        self.shared_key().0.iter().any(|v| *v > 0)
    }
    #[inline]
    pub fn sync(&mut self) -> io::Result<()> {
        let (p, k, s) = self.split();
        p521::generate_secret(p, k, s)
    }
    #[inline]
    pub fn fill(&mut self, src: &mut impl Read) {
        let (p, k, s) = self.split();
        ignore_error!(p521::generate_pair(src, p, k));
        s.fill(0);
    }
    #[inline]
    pub fn public_key<'a>(&'a self) -> PublicKey<'a> {
        PublicKey(&self.inner[PUBLIC])
    }
    #[inline]
    pub fn shared_key<'a>(&'a self) -> SharedKeys<'a> {
        SharedKeys(&self.inner[SHARED])
    }
    #[inline]
    pub fn private_key<'a>(&'a self) -> PrivateKey<'a> {
        PrivateKey(&self.inner[PRIVATE])
    }
    #[inline]
    pub fn write(&self, w: &mut impl Write) -> io::Result<()> {
        write_full(w, &&self.inner[PUBLIC])
    }
    #[inline]
    pub fn read(&mut self, r: &mut impl Read) -> io::Result<()> {
        read_full(r, &mut self.inner[PUBLIC])
    }
    #[inline]
    pub fn write_all(&self, w: &mut impl Write) -> io::Result<()> {
        write_full(w, &self.inner[PUBLIC])?;
        write_full(w, &self.inner[PRIVATE])?;
        write_full(w, &self.inner[SHARED])
    }
    #[inline]
    pub fn read_all(&mut self, r: &mut impl Read) -> io::Result<()> {
        let (p, k, s) = self.split();
        read_full(r, p)?;
        read_full(r, k)?;
        read_full(r, s)
    }
    #[inline]
    pub fn fill_public(&mut self, public: &PublicKey) -> io::Result<()> {
        let (p, k, s) = self.split();
        p521::generate_secret(p, k, s)?;
        p.copy_from_slice(&public.0);
        Ok(())
    }
    #[inline]
    pub fn fill_private(&mut self, private: &PrivateKey) -> io::Result<()> {
        let (p, k, s) = self.split();
        p521::generate_secret(p, private.0, s)?;
        k.copy_from_slice(&private.0);
        Ok(())
    }

    #[inline]
    fn split(&mut self) -> (&mut [u8], &mut [u8], &mut [u8]) {
        unsafe {
            (
                from_raw_parts_mut(self.inner[PUBLIC].as_mut_ptr(), PUBLIC.len()),
                from_raw_parts_mut(self.inner[PRIVATE].as_mut_ptr(), PRIVATE.len()),
                from_raw_parts_mut(self.inner[SHARED].as_mut_ptr(), SHARED.len()),
            )
        }
    }
}
impl<A: Allocator + Clone> KeyPair<A> {
    #[inline]
    pub fn allocator(&self) -> A {
        self.inner.allocator().clone()
    }
}

impl<A: Allocator> Eq for KeyPair<A> {}
impl<A: Allocator> PartialEq for KeyPair<A> {
    #[inline]
    fn eq(&self, other: &KeyPair<A>) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl Eq for PublicKey<'_> {}
impl Deref for PublicKey<'_> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.0
    }
}
impl PartialEq for PublicKey<'_> {
    #[inline]
    fn eq(&self, other: &PublicKey<'_>) -> bool {
        self.0 == other.0
    }
}

impl Eq for PrivateKey<'_> {}
impl Deref for PrivateKey<'_> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.0
    }
}
impl PartialEq for PrivateKey<'_> {
    #[inline]
    fn eq(&self, other: &PrivateKey<'_>) -> bool {
        self.0 == other.0
    }
}

impl Eq for SharedKeys<'_> {}
impl Deref for SharedKeys<'_> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.0
    }
}
impl PartialEq for SharedKeys<'_> {
    #[inline]
    fn eq(&self, other: &SharedKeys<'_>) -> bool {
        self.0 == other.0
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter};

    use crate::data::crypto::{PrivateKey, PublicKey, SharedKeys};
    use crate::prelude::*;
    use crate::util;

    impl Debug for SharedKeys<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Debug::fmt(&self.0, f)
        }
    }
    impl Display for PublicKey<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let mut b = [0u8; 398];
            make_string(&self.0, &mut b);
            f.write_str(unsafe { core::str::from_utf8_unchecked(&b) })
        }
    }
    impl Display for PrivateKey<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let mut b = [0u8; 197];
            make_string(&self.0, &mut b);
            f.write_str(unsafe { core::str::from_utf8_unchecked(&b) })
        }
    }

    fn make_string(key: &[u8], out: &mut [u8]) {
        let (mut i, mut n) = (0, 0);
        while i < key.len() && n < out.len() {
            if n > 0 {
                out[n] = b':';
                n += 1;
            }
            if key[i] < 16 {
                out[n] = b'0';
            } else {
                out[n] = util::HEXTABLE[(key[i] >> 0x4) as usize];
            }
            out[n + 1] = util::HEXTABLE[(key[i] & 0xF) as usize];
            n += 2;
            i += 1;
        }
    }
}
