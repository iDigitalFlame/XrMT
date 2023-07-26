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

use core::ops::Deref;

use crate::data;
use crate::data::crypto::p521;
use crate::util::stx::io::{self, Read, Write};
use crate::util::stx::prelude::*;

pub struct KeyPair {
    pub public:  PublicKey,
    pub private: PrivateKey,
    shared:      SharedKeys,
}
pub struct PublicKey([u8; 133]);
pub struct PrivateKey([u8; 66]);
pub struct SharedKeys([u8; 65]);

impl KeyPair {
    #[inline]
    pub const fn empty() -> KeyPair {
        KeyPair {
            public:  PublicKey([0u8; 133]),
            shared:  SharedKeys([0u8; 65]),
            private: PrivateKey([0u8; 66]),
        }
    }

    #[inline]
    pub fn new(src: &mut impl Read) -> KeyPair {
        let mut k = KeyPair::empty();
        k.fill(src);
        k
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.public.is_empty()
    }
    #[inline]
    pub fn is_synced(&self) -> bool {
        self.shared.0.iter().any(|v| *v > 0)
    }
    #[inline]
    pub fn shared(&self) -> &SharedKeys {
        &self.shared
    }
    #[inline]
    pub fn sync(&mut self) -> io::Result<()> {
        p521::generate_secret(&self.public.0, &self.public.0, &mut self.shared.0)
    }
    #[inline]
    pub fn fill(&mut self, src: &mut impl Read) {
        let _ = p521::generate_pair(src, &mut self.public.0, &mut self.private.0); // IGNORE ERROR
        self.shared.0.fill(0);
    }
    #[inline]
    pub fn write(&self, w: &mut impl Write) -> io::Result<()> {
        data::write_full(w, &self.public.0)
    }
    #[inline]
    pub fn read(&mut self, r: &mut impl Read) -> io::Result<()> {
        data::read_full(r, &mut self.public.0)
    }
    #[inline]
    pub fn write_all(&self, w: &mut impl Write) -> io::Result<()> {
        data::write_full(w, &self.public.0)?;
        data::write_full(w, &self.private.0)?;
        data::write_full(w, &self.shared.0)
    }
    #[inline]
    pub fn read_all(&mut self, r: &mut impl Read) -> io::Result<()> {
        data::read_full(r, &mut self.public.0)?;
        data::read_full(r, &mut self.private.0)?;
        data::read_full(r, &mut self.shared.0)
    }
    #[inline]
    pub fn fill_public(&mut self, public: &PublicKey) -> io::Result<()> {
        p521::generate_secret(&public.0, &self.private.0, &mut self.shared.0)?;
        self.public.0.copy_from_slice(&public.0);
        Ok(())
    }
    #[inline]
    pub fn fill_private(&mut self, private: &PrivateKey) -> io::Result<()> {
        p521::generate_secret(&self.public.0, &private.0, &mut self.shared.0)?;
        self.private.0.copy_from_slice(&private.0);
        Ok(())
    }
}
impl PublicKey {
    #[inline]
    pub fn hash(&self) -> u32 {
        let mut h: u32 = 0x811C9DC5;
        for i in self.0 {
            h = h.wrapping_mul(0x1000193);
            h ^= i as u32;
        }
        h
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.iter().any(|v| *v > 0)
    }
}

impl Eq for KeyPair {}
impl Clone for KeyPair {
    #[inline]
    fn clone(&self) -> KeyPair {
        KeyPair {
            shared:  self.shared.clone(),
            public:  self.public.clone(),
            private: self.private.clone(),
        }
    }
}
impl Default for KeyPair {
    #[inline]
    fn default() -> KeyPair {
        KeyPair {
            public:  PublicKey([0u8; 133]),
            shared:  SharedKeys([0u8; 65]),
            private: PrivateKey([0u8; 66]),
        }
    }
}
impl PartialEq for KeyPair {
    #[inline]
    fn eq(&self, other: &KeyPair) -> bool {
        self.public == other.public && self.private == other.private && self.shared == other.shared
    }
}

impl Eq for PublicKey {}
impl Clone for PublicKey {
    #[inline]
    fn clone(&self) -> PublicKey {
        PublicKey(self.0.clone())
    }
}
impl Deref for PublicKey {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.0
    }
}
impl Default for PublicKey {
    #[inline]
    fn default() -> PublicKey {
        PublicKey([0u8; 133])
    }
}
impl PartialEq for PublicKey {
    #[inline]
    fn eq(&self, other: &PublicKey) -> bool {
        self.0 == other.0
    }
}

impl Eq for PrivateKey {}
impl Clone for PrivateKey {
    #[inline]
    fn clone(&self) -> PrivateKey {
        PrivateKey(self.0.clone())
    }
}
impl Deref for PrivateKey {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.0
    }
}
impl Default for PrivateKey {
    #[inline]
    fn default() -> PrivateKey {
        PrivateKey([0u8; 66])
    }
}
impl PartialEq for PrivateKey {
    #[inline]
    fn eq(&self, other: &PrivateKey) -> bool {
        self.0 == other.0
    }
}

impl Eq for SharedKeys {}
impl Clone for SharedKeys {
    #[inline]
    fn clone(&self) -> SharedKeys {
        SharedKeys(self.0.clone())
    }
}
impl Deref for SharedKeys {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.0
    }
}
impl Default for SharedKeys {
    #[inline]
    fn default() -> SharedKeys {
        SharedKeys([0u8; 65])
    }
}
impl PartialEq for SharedKeys {
    #[inline]
    fn eq(&self, other: &SharedKeys) -> bool {
        self.0 == other.0
    }
}

#[cfg(not(feature = "implant"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter};

    use super::{PrivateKey, PublicKey};
    use crate::data::crypto::SharedKeys;
    use crate::util;
    use crate::util::stx::prelude::*;

    impl Debug for SharedKeys {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Debug::fmt(&self.0, f)
        }
    }
    impl Display for PublicKey {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let mut b = [0u8; 389];
            make_string(&self.0, &mut b);
            f.write_str(unsafe { core::str::from_utf8_unchecked(&b) })
        }
    }
    impl Display for PrivateKey {
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
