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

extern crate core;

use core::clone::Clone;
use core::convert::From;
use core::fmt::{Debug, Display, Formatter, Result};
use core::iter::{IntoIterator, Iterator};
use core::marker::Copy;
use core::ops::{Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, DerefMut, Sub, SubAssign};
use core::option::Option::{self, None, Some};

#[repr(u32)]
pub enum Signal {
    Invalid      = 0x0,
    Hangup       = 0x1,       // SIGHUP
    Interrupt    = 0x2,       // SIGINT
    Quit         = 0x4,       // SIGQUIT
    Illegal      = 0x8,       // SIGSEGV
    Breakpoint   = 0x10,      // SIGTRAP
    Abort        = 0x20,      // SIGIOT
    Bus          = 0x40,      // SIGBUS
    FloatError   = 0x80,      // SIGFPE
    Kill         = 0x100,     // SIGKILL
    User1        = 0x200,     // SIGUSR1
    SegFault     = 0x400,     // SIGSEGV
    User2        = 0x800,     // SIGUSR2
    Pipe         = 0x1000,    // SIGPIPE
    Alarm        = 0x2000,    // SIGALRM
    Terminate    = 0x4000,    // SIGTERM
    Child        = 0x8000,    // SIGCHLD
    Continue     = 0x10000,   // SIGCONT
    Stop         = 0x20000,   // SIGSTOP
    Urgent       = 0x40000,   // SIGURG
    VirtualAlarm = 0x80000,   // SIGVTALRM
    Profile      = 0x100000,  // SIGPROF
    EmulatorTrap = 0x200000,  // SIGEMT
    Poll         = 0x400000,  // SIGPOLL
    Power        = 0x800000,  // SIGPWR
    BadSyscall   = 0x1000000, // SIGSYS
}

pub struct SignalIter {
    v:   SignalMask,
    pos: u8,
}
pub struct SignalMask(u32);

impl Signal {
    #[inline]
    pub fn from_mask(v: u32) -> Option<Signal> {
        match v {
            0x1 => Some(Signal::Hangup),
            0x2 => Some(Signal::Interrupt),
            0x4 => Some(Signal::Quit),
            0x8 => Some(Signal::Illegal),
            0x10 => Some(Signal::Breakpoint),
            0x20 => Some(Signal::Abort),
            0x40 => Some(Signal::Bus),
            0x80 => Some(Signal::FloatError),
            0x100 => Some(Signal::Kill),
            0x200 => Some(Signal::User1),
            0x400 => Some(Signal::SegFault),
            0x800 => Some(Signal::User2),
            0x1000 => Some(Signal::Pipe),
            0x2000 => Some(Signal::Alarm),
            0x4000 => Some(Signal::Terminate),
            0x8000 => Some(Signal::Child),
            0x10000 => Some(Signal::Continue),
            0x20000 => Some(Signal::Stop),
            0x40000 => Some(Signal::Urgent),
            0x80000 => Some(Signal::VirtualAlarm),
            0x100000 => Some(Signal::Profile),
            0x200000 => Some(Signal::EmulatorTrap),
            0x400000 => Some(Signal::Poll),
            0x800000 => Some(Signal::Power),
            0x1000000 => Some(Signal::BadSyscall),
            _ => None,
        }
    }

    #[inline]
    pub fn mask(&self) -> u32 {
        *self as u32
    }
    #[inline]
    pub fn is_invalid(&self) -> bool {
        match self {
            Signal::Invalid => true,
            _ => false,
        }
    }
}
impl SignalIter {
    #[inline]
    pub const fn new(v: SignalMask) -> SignalIter {
        SignalIter { v, pos: 0u8 }
    }
}
impl SignalMask {
    #[inline]
    pub const fn empty() -> SignalMask {
        SignalMask(0u32)
    }
    #[inline]
    pub const fn new(v: u32) -> SignalMask {
        SignalMask(v)
    }
    #[inline]
    pub const fn from_signal(v: Signal) -> SignalMask {
        SignalMask(v as u32)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
    #[inline]
    pub fn add(&mut self, s: Signal) {
        self.0 |= s.mask()
    }
    #[inline]
    pub fn iter(&self) -> SignalIter {
        SignalIter::new(*self)
    }
    #[inline]
    pub fn remove(&mut self, s: Signal) {
        self.0 &= !s.mask()
    }
    #[inline]
    pub fn add_signo(&mut self, s: i32) {
        if let Some(v) = Signal::from_signo(s) {
            self.0 |= v.mask()
        }
    }
    #[inline]
    pub fn contains(&self, s: &Signal) -> bool {
        if s.is_invalid() {
            return false;
        }
        self.0 & s.mask() != 0
    }
    #[inline]
    pub fn contains_any(&self, s: &SignalMask) -> bool {
        self.0 & s.0 != 0
    }
    /// Updates the current SignalMask set and returns the difference
    /// between the old and new masks.
    ///
    /// This can be used to remove or add new Signals to mask.
    pub fn update(&mut self, add: bool, s: Signal) -> SignalMask {
        let n = if add { self.0 | s.mask() } else { self.0 & !s.mask() };
        let d = SignalMask(self.0 ^ n);
        self.0 = n;
        d
    }
    /// Updates the current SignalMask set and returns the difference
    /// between the old and new masks.
    ///
    /// This can be used to remove or add new Signals to mask.
    ///
    /// This adds all of the ones in the mask at a single time.
    pub fn update_set(&mut self, add: bool, s: &SignalMask) -> SignalMask {
        let mut n = self.0;
        for i in s.iter() {
            n = if add { n | i.mask() } else { n & !i.mask() };
        }
        let d = SignalMask(self.0 ^ n);
        self.0 = n;
        d
    }
}

impl Copy for Signal {}
impl Clone for Signal {
    #[inline]
    fn clone(&self) -> Signal {
        *self
    }
}
impl Debug for Signal {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Signal::Bus => f.write_str("Bus"),
            Signal::Kill => f.write_str("Kill"),
            Signal::Pipe => f.write_str("Pipe"),
            Signal::Poll => f.write_str("Poll"),
            Signal::Quit => f.write_str("Quit"),
            Signal::Stop => f.write_str("Stop"),
            Signal::Abort => f.write_str("Abort"),
            Signal::Alarm => f.write_str("Alarm"),
            Signal::Child => f.write_str("Child"),
            Signal::Power => f.write_str("Power"),
            Signal::User1 => f.write_str("User1"),
            Signal::User2 => f.write_str("User2"),
            Signal::Hangup => f.write_str("Hangup"),
            Signal::Urgent => f.write_str("Urgent"),
            Signal::Illegal => f.write_str("Illegal"),
            Signal::Invalid => f.write_str("Invalid"),
            Signal::Profile => f.write_str("Profile"),
            Signal::Continue => f.write_str("Continue"),
            Signal::SegFault => f.write_str("SegFault"),
            Signal::Interrupt => f.write_str("Interrupt"),
            Signal::Terminate => f.write_str("Terminate"),
            Signal::BadSyscall => f.write_str("BadSyscall"),
            Signal::Breakpoint => f.write_str("Breakpoint"),
            Signal::FloatError => f.write_str("FloatError"),
            Signal::VirtualAlarm => f.write_str("VirtualAlarm"),
            Signal::EmulatorTrap => f.write_str("EmulatorTrap"),
        }
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> Result {
        core::result::Result::Ok(())
    }
}
impl Display for Signal {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Debug::fmt(self, f)
    }
}
impl From<i32> for Signal {
    #[inline]
    fn from(v: i32) -> Signal {
        Signal::from_signo(v).unwrap_or(Signal::Invalid)
    }
}
impl From<u32> for Signal {
    #[inline]
    fn from(v: u32) -> Signal {
        Signal::from_signo(v as i32).unwrap_or(Signal::Invalid)
    }
}
impl From<Signal> for Option<i32> {
    #[inline]
    fn from(v: Signal) -> Option<i32> {
        v.signo()
    }
}

impl BitOr<Signal> for Signal {
    type Output = u32;

    #[inline]
    fn bitor(self, rhs: Signal) -> u32 {
        self.mask() | rhs.mask()
    }
}
impl BitAnd<Signal> for Signal {
    type Output = u32;

    #[inline]
    fn bitand(self, rhs: Signal) -> u32 {
        self.mask() & rhs.mask()
    }
}
impl BitXor<Signal> for Signal {
    type Output = u32;

    #[inline]
    fn bitxor(self, rhs: Signal) -> u32 {
        self.mask() ^ rhs.mask()
    }
}

impl Copy for SignalMask {}
impl Clone for SignalMask {
    #[inline]
    fn clone(&self) -> SignalMask {
        *self
    }
}
impl Deref for SignalMask {
    type Target = u32;

    #[inline]
    fn deref(&self) -> &u32 {
        &self.0
    }
}
impl DerefMut for SignalMask {
    #[inline]
    fn deref_mut(&mut self) -> &mut u32 {
        &mut self.0
    }
}
impl From<u32> for SignalMask {
    #[inline]
    fn from(v: u32) -> SignalMask {
        SignalMask(v)
    }
}
impl From<Signal> for SignalMask {
    #[inline]
    fn from(v: Signal) -> SignalMask {
        SignalMask(v.mask())
    }
}
impl IntoIterator for SignalMask {
    type Item = Signal;
    type IntoIter = SignalIter;

    #[inline]
    fn into_iter(self) -> SignalIter {
        SignalIter::new(self)
    }
}
impl IntoIterator for &SignalMask {
    type Item = Signal;
    type IntoIter = SignalIter;

    #[inline]
    fn into_iter(self) -> SignalIter {
        SignalIter::new(*self)
    }
}

impl BitOr for SignalMask {
    type Output = SignalMask;

    #[inline]
    fn bitor(self, rhs: SignalMask) -> SignalMask {
        SignalMask(self.0 | rhs.0)
    }
}
impl BitAnd for SignalMask {
    type Output = SignalMask;

    #[inline]
    fn bitand(self, rhs: SignalMask) -> SignalMask {
        SignalMask(self.0 & rhs.0)
    }
}
impl BitXor for SignalMask {
    type Output = SignalMask;

    #[inline]
    fn bitxor(self, rhs: SignalMask) -> SignalMask {
        SignalMask(self.0 ^ rhs.0)
    }
}
impl BitOrAssign for SignalMask {
    #[inline]
    fn bitor_assign(&mut self, rhs: SignalMask) {
        self.0 |= rhs.0
    }
}
impl BitAndAssign for SignalMask {
    #[inline]
    fn bitand_assign(&mut self, rhs: SignalMask) {
        self.0 &= rhs.0
    }
}
impl BitXorAssign for SignalMask {
    #[inline]
    fn bitxor_assign(&mut self, rhs: SignalMask) {
        self.0 ^= rhs.0
    }
}

impl Add<Signal> for SignalMask {
    type Output = SignalMask;

    #[inline]
    fn add(self, rhs: Signal) -> SignalMask {
        SignalMask(self.0 | rhs.mask())
    }
}
impl Sub<Signal> for SignalMask {
    type Output = SignalMask;

    #[inline]
    fn sub(self, rhs: Signal) -> SignalMask {
        SignalMask(self.0 & !rhs.mask())
    }
}
impl BitOr<Signal> for SignalMask {
    type Output = SignalMask;

    #[inline]
    fn bitor(self, rhs: Signal) -> SignalMask {
        SignalMask(self.0 | rhs.mask())
    }
}
impl BitAnd<Signal> for SignalMask {
    type Output = SignalMask;

    #[inline]

    fn bitand(self, rhs: Signal) -> SignalMask {
        SignalMask(self.0 & rhs.mask())
    }
}
impl BitXor<Signal> for SignalMask {
    type Output = SignalMask;

    #[inline]
    fn bitxor(self, rhs: Signal) -> SignalMask {
        SignalMask(self.0 ^ rhs.mask())
    }
}
impl AddAssign<Signal> for SignalMask {
    #[inline]
    fn add_assign(&mut self, rhs: Signal) {
        self.0 |= rhs.mask()
    }
}
impl SubAssign<Signal> for SignalMask {
    #[inline]
    fn sub_assign(&mut self, rhs: Signal) {
        self.0 &= !rhs.mask()
    }
}
impl BitOrAssign<Signal> for SignalMask {
    #[inline]
    fn bitor_assign(&mut self, rhs: Signal) {
        self.0 |= rhs.mask()
    }
}
impl BitAndAssign<Signal> for SignalMask {
    #[inline]
    fn bitand_assign(&mut self, rhs: Signal) {
        self.0 &= rhs.mask()
    }
}
impl BitXorAssign<Signal> for SignalMask {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Signal) {
        self.0 ^= rhs.mask()
    }
}

impl BitOr<u32> for SignalMask {
    type Output = u32;

    #[inline]
    fn bitor(self, rhs: u32) -> u32 {
        self.0 | rhs
    }
}
impl BitAnd<u32> for SignalMask {
    type Output = u32;

    #[inline]
    fn bitand(self, rhs: u32) -> u32 {
        self.0 & rhs
    }
}
impl BitXor<u32> for SignalMask {
    type Output = u32;

    #[inline]
    fn bitxor(self, rhs: u32) -> u32 {
        self.0 ^ rhs
    }
}
impl BitOrAssign<u32> for SignalMask {
    #[inline]
    fn bitor_assign(&mut self, rhs: u32) {
        self.0 |= rhs
    }
}

impl BitOr<Signal> for u32 {
    type Output = u32;

    #[inline]
    fn bitor(self, rhs: Signal) -> u32 {
        self | rhs.mask()
    }
}
impl BitAnd<Signal> for u32 {
    type Output = u32;

    #[inline]
    fn bitand(self, rhs: Signal) -> u32 {
        self & rhs.mask()
    }
}
impl BitXor<Signal> for u32 {
    type Output = u32;

    #[inline]
    fn bitxor(self, rhs: Signal) -> u32 {
        self ^ rhs.mask()
    }
}
impl BitOrAssign<Signal> for u32 {
    #[inline]
    fn bitor_assign(&mut self, rhs: Signal) {
        *self |= rhs.mask()
    }
}
impl BitAndAssign<Signal> for u32 {
    #[inline]
    fn bitand_assign(&mut self, rhs: Signal) {
        *self &= rhs.mask()
    }
}
impl BitXorAssign<Signal> for u32 {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Signal) {
        *self ^= rhs.mask()
    }
}

impl Iterator for SignalIter {
    type Item = Signal;

    #[inline]
    fn next(&mut self) -> Option<Signal> {
        if self.pos > 31 {
            return None;
        }
        let n = unsafe { 1u32.unchecked_shl(self.pos as u32) };
        if n > self.v.0 {
            return None;
        }
        let r = Signal::from_mask(self.v.0 & n).unwrap_or(Signal::Invalid);
        self.pos += 1;
        Some(r)
    }
}

#[cfg(target_family = "windows")]
mod os {
    extern crate core;

    use core::option::Option::{self, None, Some};

    use crate::signals::Signal;

    impl Signal {
        #[inline]
        pub fn from_signo(v: i32) -> Option<Signal> {
            match v {
                0 => Some(Signal::Interrupt),
                1 => Some(Signal::Breakpoint),
                2 => Some(Signal::Abort),
                5 => Some(Signal::User1),
                6 => Some(Signal::Stop),
                _ => None,
            }
        }

        #[inline]
        pub fn signo(&self) -> Option<i32> {
            match self {
                Signal::Interrupt => Some(0),
                Signal::Breakpoint => Some(1),
                Signal::Abort => Some(2),
                Signal::User1 => Some(5),
                Signal::Stop => Some(6),
                _ => None,
            }
        }
    }
}
#[cfg(not(target_family = "windows"))]
mod os {
    extern crate core;

    extern crate libc;

    use core::option::Option::{self, None, Some};

    use crate::signals::Signal;

    impl Signal {
        #[inline]
        pub fn from_signo(v: i32) -> Option<Signal> {
            match v {
                libc::SIGHUP => Some(Signal::Hangup),
                libc::SIGINT => Some(Signal::Interrupt),
                libc::SIGQUIT => Some(Signal::Quit),
                libc::SIGILL => Some(Signal::Illegal),
                libc::SIGTRAP => Some(Signal::Breakpoint),
                libc::SIGABRT => Some(Signal::Abort),
                libc::SIGBUS => Some(Signal::Bus),
                libc::SIGFPE => Some(Signal::FloatError),
                libc::SIGKILL => Some(Signal::Kill),
                libc::SIGUSR1 => Some(Signal::User1),
                libc::SIGSEGV => Some(Signal::SegFault),
                libc::SIGUSR2 => Some(Signal::User2),
                libc::SIGPIPE => Some(Signal::Pipe),
                libc::SIGALRM => Some(Signal::Alarm),
                libc::SIGTERM => Some(Signal::Terminate),
                libc::SIGCHLD => Some(Signal::Child),
                libc::SIGCONT => Some(Signal::Continue),
                libc::SIGSTOP => Some(Signal::Stop),
                libc::SIGURG => Some(Signal::Urgent),
                libc::SIGVTALRM => Some(Signal::VirtualAlarm),
                libc::SIGPROF => Some(Signal::Profile),
                libc::SIGSYS => Some(Signal::BadSyscall),
                // BSD/Apple does not have these signals
                // ------------------------------------- //
                #[cfg(all(
                    not(target_os = "netbsd"),
                    not(target_os = "freebsd"),
                    not(target_vendor = "apple")
                ))]
                libc::SIGPOLL => Some(Signal::Poll),
                #[cfg(all(
                    not(target_os = "netbsd"),
                    not(target_os = "freebsd"),
                    not(target_vendor = "apple")
                ))]
                libc::SIGPWR => Some(Signal::Power),
                // ------------------------------------- //
                _ => None,
            }
        }

        #[inline]
        pub fn signo(&self) -> Option<i32> {
            match self {
                Signal::Hangup => Some(libc::SIGHUP),
                Signal::Interrupt => Some(libc::SIGINT),
                Signal::Quit => Some(libc::SIGQUIT),
                Signal::Illegal => Some(libc::SIGILL),
                Signal::Breakpoint => Some(libc::SIGTRAP),
                Signal::Abort => Some(libc::SIGABRT),
                Signal::Bus => Some(libc::SIGBUS),
                Signal::FloatError => Some(libc::SIGFPE),
                Signal::Kill => Some(libc::SIGKILL),
                Signal::User1 => Some(libc::SIGUSR1),
                Signal::SegFault => Some(libc::SIGSEGV),
                Signal::User2 => Some(libc::SIGUSR2),
                Signal::Pipe => Some(libc::SIGPIPE),
                Signal::Alarm => Some(libc::SIGALRM),
                Signal::Terminate => Some(libc::SIGTERM),
                Signal::Child => Some(libc::SIGCHLD),
                Signal::Continue => Some(libc::SIGCONT),
                Signal::Stop => Some(libc::SIGSTOP),
                Signal::Urgent => Some(libc::SIGURG),
                Signal::VirtualAlarm => Some(libc::SIGVTALRM),
                Signal::Profile => Some(libc::SIGPROF),
                Signal::BadSyscall => Some(libc::SIGSYS),
                // BSD/Apple does not have these signals
                // ------------------------------------- //
                #[cfg(all(
                    not(target_os = "netbsd"),
                    not(target_os = "freebsd"),
                    not(target_vendor = "apple")
                ))]
                Signal::Poll => Some(libc::SIGPOLL),
                #[cfg(all(
                    not(target_os = "netbsd"),
                    not(target_os = "freebsd"),
                    not(target_vendor = "apple")
                ))]
                Signal::Power => Some(libc::SIGPWR),
                // ------------------------------------- //
                _ => None,
            }
        }
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Formatter, Result};
    use core::write;

    use crate::signals::SignalMask;

    impl Debug for SignalMask {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "SignalMask({:X})", self.0)
        }
    }
}
