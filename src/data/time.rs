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

use core::cmp::Ordering;
use core::hash::Hasher;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::time::Duration;

use crate::data::read_u64;
use crate::prelude::*;

const DAYS_BEFORE: [u16; 13] = [
    0,
    31,
    31 + 28,
    31 + 28 + 31,
    31 + 28 + 31 + 30,
    31 + 28 + 31 + 30 + 31,
    31 + 28 + 31 + 30 + 31 + 30,
    31 + 28 + 31 + 30 + 31 + 30 + 31,
    31 + 28 + 31 + 30 + 31 + 30 + 31 + 31,
    31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30,
    31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31,
    31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31 + 30,
    31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31 + 30 + 31,
];
const TIME_OFFSET: i64 = 0xE7791F700i64;

#[repr(u8)]
pub enum Month {
    Invalid   = 0,
    January   = 1,
    February  = 2,
    March     = 3,
    April     = 4,
    May       = 5,
    June      = 6,
    July      = 7,
    August    = 8,
    September = 9,
    October   = 10,
    November  = 11,
    December  = 12,
}
#[repr(u8)]
pub enum Weekday {
    Sunday    = 0,
    Monday    = 1,
    Tuesday   = 2,
    Wednesday = 3,
    Thursday  = 4,
    Friday    = 5,
    Saturday  = 6,
    Invalid   = 7,
}

pub struct Time(i64);

impl Time {
    pub const ZERO: Time = Time(0);

    #[inline]
    pub const fn zero() -> Time {
        Time(0)
    }
    #[inline]
    pub const fn from_nano(sec: i64) -> Time {
        Time(sec + TIME_OFFSET)
    }
    #[inline]
    pub const fn from_unix(sec: i64, nano_sec: i64) -> Time {
        let mut s = sec;
        if nano_sec < 0 || nano_sec >= 0x3B9ACA00 {
            s += nano_sec / 0x3B9ACA00;
            if nano_sec.wrapping_sub(nano_sec.wrapping_mul(0x3B9ACA00)) < 0 {
                s -= 1;
            }
        }
        Time(s + TIME_OFFSET)
    }
    pub const fn new(year: u16, month: Month, day: u8, hour: u8, min: u8, sec: u8) -> Time {
        let (y, v) = Time::norm(year as i32, month as i32 - 1, 12);
        let v = v as usize + 1;
        let (s, _) = Time::norm(sec as i32, 0, 1000000000);
        let (m, s) = Time::norm(min as i32, s, 60);
        let (h, m) = Time::norm(hour as i32, m, 60);
        let (d, h) = Time::norm(day as i32, h, 24);
        let mut e = Time::days_since_epoch(y) + DAYS_BEFORE[v - 1] as i64;
        if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) && v >= 3 {
            e += 1;
        }
        Time((((e + d as i64 - 1) * 0x15180) + (h * 0xE10 + m * 0x3C + s) as i64).wrapping_sub(0x7FFFFFEFA347D200))
    }

    #[inline]
    pub fn now() -> Time {
        Time::from(sys::now())
    }

    #[inline]
    pub fn day(&self) -> u8 {
        let (_, _, d, _) = self.make_date(true);
        d
    }
    #[inline]
    pub fn hour(&self) -> u8 {
        ((self.abs() % 0x15180) / 0xE10) as u8
    }
    #[inline]
    pub fn unix(&self) -> i64 {
        self.0 - TIME_OFFSET
    }
    #[inline]
    pub fn year(&self) -> u16 {
        let (y, ..) = self.make_date(false);
        y
    }
    #[inline]
    pub fn minute(&self) -> u8 {
        ((self.abs() % 0xE10) / 0x3C) as u8
    }
    #[inline]
    pub fn second(&self) -> u8 {
        (self.abs() % 0x3C) as u8
    }
    #[inline]
    pub fn month(&self) -> Month {
        let (_, m, ..) = self.make_date(true);
        m
    }
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
    #[inline]
    pub fn year_day(&self) -> u16 {
        let (_, _, _, d) = self.make_date(true);
        d + 1
    }
    #[inline]
    pub fn weekday(&self) -> Weekday {
        Weekday::from((((self.abs().wrapping_add(0x15180)) % 0x93A80) / 0x15180) as u8)
    }
    #[inline]
    pub fn clock(&self) -> (u8, u8, u8) {
        let mut s = self.abs() % 0x15180;
        let h = s / 0xE10;
        s -= h * 0xE10;
        let m = s / 0x3C;
        (h as u8, m as u8, (s - (m * 0x3C)) as u8)
    }
    #[inline]
    pub fn add(self, d: Duration) -> Time {
        Time(self.0.wrapping_add(d.as_secs() as i64))
    }
    #[inline]
    pub fn date(&self) -> (u16, Month, u8) {
        let (y, m, d, _) = self.make_date(true);
        (y, m, d)
    }
    #[inline]
    pub fn is_equal(&self, u: Time) -> bool {
        self.0 == u.0
    }
    #[inline]
    pub fn is_after(&self, u: Time) -> bool {
        self.0 > u.0
    }
    #[inline]
    pub fn is_before(&self, u: Time) -> bool {
        self.0 < u.0
    }
    #[inline]
    pub fn add_seconds(self, d: i64) -> Time {
        Time(self.0.wrapping_add(d))
    }
    #[inline]
    pub fn subtract(&self, u: Time) -> Duration {
        if self.is_before(u) {
            return Duration::ZERO;
        }
        Duration::from_secs((self.unix() - u.unix()) as u64)
    }
    #[inline]
    pub fn to_local(self, hours_offset: i8) -> Time {
        Time(self.0.wrapping_add(hours_offset as i64 * (0xE10)))
    }
    #[inline]
    pub fn add_date(self, years: u16, months: u8, days: u8) -> Time {
        let (y, v, d) = self.date();
        let (h, m, s) = self.clock();
        Time::new(y + years, Month::from(v as u8 + months), d + days, h, m, s)
    }

    #[inline]
    fn abs(&self) -> u64 {
        (self.0 as u64).wrapping_add(0x7FFFFFEFA347D200)
    }
    fn make_date(&self, full: bool) -> (u16, Month, u8, u16) {
        let mut d = self.abs() / 0x15180;
        let mut y = 0x190 * (d / 0x23AB1);
        d -= 0x23AB1 * (d / 0x23AB1);
        let mut n = d / 0x8EAC;
        n -= n >> 2;
        y += 0x64 * n;
        d -= 0x8EAC * n;
        y += 0x4 * (d / 0x5B5);
        d -= 0x5B5 * (d / 0x5B5);
        let mut n = d / 0x16D;
        n -= n >> 2;
        y += n;
        d -= 0x16D * n;
        let year = ((y as i64).wrapping_sub(0x440D116EBF)) as u16;
        let year_day = d as u16;
        if !full {
            return (year, Month::Invalid, 0, year_day);
        }
        let mut day = year_day;
        if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            if day == 0x3B {
                return (year as u16, Month::February, 29, year_day);
            } else if day > 0x3B {
                day -= 1
            }
        }
        let m = (day / 31) as usize;
        let e = DAYS_BEFORE[m + 1];
        if day >= e {
            (
                year,
                Month::from((m + 2) as u8),
                (day - e + 1) as u8,
                year_day,
            )
        } else {
            (
                year,
                Month::from((m + 1) as u8),
                (day - DAYS_BEFORE[m] + 1) as u8,
                year_day,
            )
        }
    }

    #[inline]
    const fn days_since_epoch(year: i32) -> i64 {
        let mut y = year as i64 + 0x440D116EBF;
        let mut d = 0x23AB1 * (y / 0x190);
        y -= 0x190 * (y / 0x190);
        d += 0x8EAC * (y / 0x64);
        y -= 0x64 * (y / 0x64);
        d += 0x5B5 * (y / 0x4);
        y -= 0x4 * (y / 0x4);
        (d + (0x16D * y)) as i64
    }
    #[inline]
    const fn norm(hi: i32, low: i32, base: i32) -> (i32, i32) {
        let (mut x, mut y) = (hi, low);
        if y < 0 {
            let n = (-y - 1) / base + 1;
            x -= n;
            y += n * base;
        }
        if y >= base {
            let n = y / base;
            x += n;
            y -= n * base;
        }
        (x, y)
    }
}

impl Eq for Time {}
impl Ord for Time {
    #[inline]
    fn cmp(&self, other: &Time) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl Hash for Time {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
impl Copy for Time {}
impl Clone for Time {
    #[inline]
    fn clone(&self) -> Time {
        Time(self.0)
    }
}
impl Default for Time {
    #[inline]
    fn default() -> Time {
        Time::ZERO
    }
}
impl From<i64> for Time {
    #[inline]
    fn from(v: i64) -> Time {
        Time(v)
    }
}
impl From<u64> for Time {
    #[inline]
    fn from(v: u64) -> Time {
        Time(v as i64)
    }
}
impl Sub<Time> for Time {
    type Output = Duration;

    #[inline]
    fn sub(self, d: Time) -> Duration {
        self.subtract(d)
    }
}
impl PartialEq for Time {
    #[inline]
    fn eq(&self, other: &Time) -> bool {
        self.0 == other.0
    }
}
impl PartialOrd for Time {
    #[inline]
    fn partial_cmp(&self, other: &Time) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl From<[u8; 8]> for Time {
    #[inline]
    fn from(v: [u8; 8]) -> Time {
        Time(read_u64(&v) as i64)
    }
}
impl Add<Duration> for Time {
    type Output = Time;

    #[inline]
    fn add(self, d: Duration) -> Time {
        self.add(d)
    }
}
impl Sub<Duration> for Time {
    type Output = Time;

    #[inline]
    fn sub(self, d: Duration) -> Time {
        self.add_seconds(d.as_secs() as i64 * -1)
    }
}
impl AddAssign<Duration> for Time {
    #[inline]
    fn add_assign(&mut self, d: Duration) {
        *self = self.add(d)
    }
}
impl SubAssign<Duration> for Time {
    #[inline]
    fn sub_assign(&mut self, d: Duration) {
        *self = self.add_seconds(d.as_secs() as i64 * -1)
    }
}

impl Eq for Month {}
impl Ord for Month {
    #[inline]
    fn cmp(&self, other: &Month) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}
impl Copy for Month {}
impl Clone for Month {
    #[inline]
    fn clone(&self) -> Month {
        *self
    }
}
impl From<u8> for Month {
    #[inline]
    fn from(v: u8) -> Month {
        match v {
            0x1 => Month::January,
            0x2 => Month::February,
            0x3 => Month::March,
            0x4 => Month::April,
            0x5 => Month::May,
            0x6 => Month::June,
            0x7 => Month::July,
            0x8 => Month::August,
            0x9 => Month::September,
            0xA => Month::October,
            0xB => Month::November,
            0xC => Month::December,
            _ => Month::Invalid,
        }
    }
}
impl PartialEq for Month {
    #[inline]
    fn eq(&self, other: &Month) -> bool {
        *self as u8 == *other as u8
    }
}
impl PartialOrd for Month {
    #[inline]
    fn partial_cmp(&self, other: &Month) -> Option<Ordering> {
        (*self as u8).partial_cmp(&(*other as u8))
    }
}

impl Eq for Weekday {}
impl Ord for Weekday {
    #[inline]
    fn cmp(&self, other: &Weekday) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}
impl Copy for Weekday {}
impl Clone for Weekday {
    #[inline]
    fn clone(&self) -> Weekday {
        *self
    }
}
impl From<u8> for Weekday {
    #[inline]
    fn from(v: u8) -> Weekday {
        match v {
            0 => Weekday::Sunday,
            1 => Weekday::Monday,
            2 => Weekday::Tuesday,
            3 => Weekday::Wednesday,
            4 => Weekday::Thursday,
            5 => Weekday::Friday,
            6 => Weekday::Saturday,
            _ => Weekday::Invalid,
        }
    }
}
impl PartialEq for Weekday {
    #[inline]
    fn eq(&self, other: &Weekday) -> bool {
        *self as u8 == *other as u8
    }
}
impl PartialOrd for Weekday {
    #[inline]
    fn partial_cmp(&self, other: &Weekday) -> Option<Ordering> {
        (*self as u8).partial_cmp(&(*other as u8))
    }
}

#[cfg(target_family = "windows")]
mod sys {
    use crate::data::time::Time;
    use crate::device::winapi;

    #[inline]
    pub fn now() -> Time {
        // This outputs the time in UTC.
        Time::from_unix(0, winapi::kernel_nano_sec_time())
    }
}
#[cfg(target_vendor = "fortanix")]
mod sys {
    extern crate std;

    use std::time::SystemTime;

    use crate::data::time::Time;

    #[inline]
    pub fn now() -> Time {
        Time::from_nano(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        )
    }
}
#[cfg(all(not(target_family = "windows"), not(target_vendor = "fortanix")))]
mod sys {
    extern crate core;
    extern crate libc;

    use core::ptr;

    use crate::data::time::Time;

    #[inline]
    pub fn now() -> Time {
        // This outputs the time in UTC.
        Time::from_nano(unsafe { libc::time(ptr::null_mut()) } as i64)
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter};

    use crate::data::time::{Month, Time, Weekday};
    use crate::prelude::*;

    impl Debug for Time {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_tuple("Time").field(&self.0).finish()
        }
    }
    impl Display for Time {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let (h, n, s) = self.clock();
            let (y, m, d) = self.date();
            write!(f, "{y:04}/{:02}/{d:02}: {h:02}:{n:02};{s:02}", m as u8)
        }
    }

    impl Debug for Month {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Month {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            match self {
                Month::January => f.write_str("January"),
                Month::February => f.write_str("February"),
                Month::March => f.write_str("March"),
                Month::April => f.write_str("April"),
                Month::May => f.write_str("May"),
                Month::June => f.write_str("June"),
                Month::July => f.write_str("July"),
                Month::August => f.write_str("August"),
                Month::September => f.write_str("September"),
                Month::October => f.write_str("October"),
                Month::November => f.write_str("November"),
                Month::December => f.write_str("December"),
                Month::Invalid => f.write_str("Invalid"),
            }
        }
    }

    impl Debug for Weekday {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Weekday {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            match self {
                Weekday::Sunday => f.write_str("Sunday"),
                Weekday::Monday => f.write_str("Monday"),
                Weekday::Tuesday => f.write_str("Tuesday"),
                Weekday::Wednesday => f.write_str("Wednesday"),
                Weekday::Thursday => f.write_str("Thursday"),
                Weekday::Friday => f.write_str("Friday"),
                Weekday::Saturday => f.write_str("Saturday"),
                Weekday::Invalid => f.write_str("Invalid"),
            }
        }
    }
}
