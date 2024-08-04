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

use core::cmp::Ordering;
use core::error::Error;
use core::fmt::{self, Debug, Display, Formatter};
use core::hash::Hasher;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::time::Duration;

use crate::data::time::Time;
use crate::prelude::*;

pub struct SystemTime(Time);
pub struct SystemTimeError(Duration);

impl SystemTime {
    #[inline]
    pub fn now() -> SystemTime {
        SystemTime(Time::now())
    }

    #[inline]
    pub fn elapsed(&self) -> Result<Duration, SystemTimeError> {
        Ok(self.0.sub(Time::now()))
    }
    #[inline]
    pub fn checked_add(&self, duration: Duration) -> Option<SystemTime> {
        Some(SystemTime(self.0.add(duration)))
    }
    #[inline]
    pub fn checked_sub(&self, duration: Duration) -> Option<SystemTime> {
        Some(SystemTime(self.0.sub(duration)))
    }
    #[inline]
    pub fn duration_since(&self, earlier: SystemTime) -> Result<Duration, SystemTimeError> {
        if earlier.0.is_before(self.0) {
            Err(SystemTimeError(earlier.0.sub(self.0)))
        } else {
            Ok(self.0.sub(earlier.0))
        }
    }
}
impl SystemTimeError {
    #[inline]
    pub fn duration(&self) -> Duration {
        self.0
    }
}
impl Clone for SystemTimeError {
    #[inline]
    fn clone(&self) -> SystemTimeError {
        SystemTimeError(self.0)
    }
}
impl Error for SystemTimeError {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Debug for SystemTimeError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SystemTimeError").field(&self.0).finish()
    }
}
impl Display for SystemTimeError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

impl Eq for SystemTime {}
impl Ord for SystemTime {
    #[inline]
    fn cmp(&self, other: &SystemTime) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl Hash for SystemTime {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
impl Copy for SystemTime {}
impl Debug for SystemTime {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SystemTime").field(&self.0).finish()
    }
}
impl Clone for SystemTime {
    #[inline]
    fn clone(&self) -> SystemTime {
        SystemTime(self.0.clone())
    }
}
impl PartialEq for SystemTime {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}
impl PartialOrd for SystemTime {
    #[inline]
    fn partial_cmp(&self, other: &SystemTime) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl From<Time> for SystemTime {
    #[inline]
    fn from(v: Time) -> SystemTime {
        SystemTime(v)
    }
}
impl Add<Duration> for SystemTime {
    type Output = SystemTime;

    #[inline]
    fn add(self, rhs: Duration) -> SystemTime {
        SystemTime(self.0.add(rhs))
    }
}
impl Sub<Duration> for SystemTime {
    type Output = SystemTime;

    #[inline]
    fn sub(self, rhs: Duration) -> SystemTime {
        SystemTime(self.0.sub(rhs))
    }
}
impl AddAssign<Duration> for SystemTime {
    #[inline]
    fn add_assign(&mut self, rhs: Duration) {
        self.0.add_assign(rhs)
    }
}
impl SubAssign<Duration> for SystemTime {
    #[inline]
    fn sub_assign(&mut self, rhs: Duration) {
        self.0.sub_assign(rhs)
    }
}

impl From<SystemTime> for Time {
    #[inline]
    fn from(v: SystemTime) -> Time {
        v.0
    }
}
