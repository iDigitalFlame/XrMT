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

use core::alloc::Allocator;
use core::cmp::Ordering;
use core::ops::Not;
use core::time::Duration;

use crate::c2::cfg::{Setting, EMPTY, SYS_WORK_HOURS};
use crate::data::time::Time;
use crate::data::{Readable, Reader, Writable, Writer};
use crate::prelude::*;
use crate::util::BinaryIter;
use crate::{io, number_like};

number_like!(Day, u8);

pub struct Day(u8);
#[derive(Debug)]
pub struct WorkHours {
    pub days:       u8,
    pub start_hour: u8,
    pub start_min:  u8,
    pub end_hour:   u8,
    pub end_min:    u8,
}

impl Day {
    pub const SUNDAY: Day = Day(0x1);
    pub const MONDAY: Day = Day(0x2);
    pub const TUESDAY: Day = Day(0x4);
    pub const WEDNESDAY: Day = Day(0x8);
    pub const THURSDAY: Day = Day(0x10);
    pub const FRIDAY: Day = Day(0x20);
    pub const SATURDAY: Day = Day(0x40);
    pub const EVERYDAY: Day = Day(0x00);
}
impl WorkHours {
    #[inline]
    pub const fn new() -> WorkHours {
        WorkHours {
            days:       0u8,
            start_min:  0u8,
            start_hour: 0u8,
            end_min:    0u8,
            end_hour:   0u8,
        }
    }
    #[inline]
    pub const fn with(days: Day, start_hour: u8, start_min: u8, end_hour: u8, end_min: u8) -> WorkHours {
        WorkHours {
            days: days.0,
            start_hour,
            start_min,
            end_hour,
            end_min,
        }
    }

    #[inline]
    pub fn from_stream(r: &mut impl Reader) -> io::Result<WorkHours> {
        let mut w = WorkHours::new();
        w.read_stream(r)?;
        Ok(w)
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        !(self.end_min > 59 || self.end_hour > 23 || self.start_min > 59 || self.start_hour > 23)
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start_hour == 0 && self.start_min == 0 && self.end_hour == 0 && self.end_min == 0 && (self.days == 0 || self.days > 126)
    }
    pub fn work(&self) -> Option<Duration> {
        if self.is_empty() {
            return None;
        }
        // TODO(dij): Go Divergence! This function has been updated to work correctly.
        //            THE GO VERSION IS CURRENTLY BUGGED.
        let n = Time::now();
        // Check out day, if it's non-zero and we're not in the requested day,
        // quick bail, wait until the next day.
        if self.days > 0 && self.days < 127 && (self.days & (1 << n.weekday() as u8)) == 0 {
            // Figure out how much time until the next day.
            return WorkHours::next_day(n);
        }
        // Check if we have any start or end hours. If they're both empty, then
        // quick bail and let us run.
        if self.start_hour == 0 && self.start_min == 0 && self.end_hour == 0 && self.end_min == 0 {
            return None;
        }
        // Now here we have a [potentially] valid start or end values.
        // Check the start date. If it's invalid, treat it as empty. Check if it's
        // empty.
        let (y, m, d) = n.date();
        let s = if (self.start_hour == 0 && self.start_min == 0) || self.start_hour > 23 || self.start_min > 60 {
            // If start is empty, use the start of today as the start time (00:00).
            Time::new(y, m, d, 0, 0, 0)
        } else {
            // Otherwise, create it
            Time::new(y, m, d, self.start_hour, self.start_min, 0)
        };
        // Check if the start time is after, if not wait until it is.
        if n.is_before(s) {
            // Wait until the time needed.
            return Some(s - n);
        }
        // We now have a valid start time, that we know we are after, so we /could/
        // be able to run, unless the end time prevents us.
        //
        // Check the end date. If it's invalid, treat it as empty. Check if it's
        // empty.
        if (self.end_hour == 0 && self.end_min == 0) || self.end_hour > 23 || self.end_min > 60 {
            // If there's no end time, we can run.
            return None;
        }
        // Create our end time.
        let e = Time::new(y, m, d, self.end_hour, self.end_min, 0);
        if n.is_after(e) {
            // If the end time is after, wait until the next day.
            WorkHours::next_day(n)
        } else {
            // Now we should be able to run as we're before the end time.
            None
        }
    }

    #[inline]
    fn next_day(now: Time) -> Option<Duration> {
        let (y, m, d) = now.date();
        Some(Time::new(y, m, d + 1, 0, 0, 0) - now)
    }
}

impl Eq for Day {}
impl Ord for Day {
    #[inline]
    fn cmp(&self, other: &Day) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl Not for Day {
    type Output = Day;

    fn not(self) -> Day {
        if self.0 >= 127 {
            return Day::EVERYDAY;
        }
        match self.0.count_ones() {
            0 => return Day::EVERYDAY,
            1 | 2 | 3 | 4 | 5 | 6 => (),
            _ => return Day(!self.0),
        }
        // Flip and custom inverse the value.
        let v: usize = BinaryIter::new(self.0 as usize, 6usize)
            .map(|(p, e)| if e { 0 } else { 1 << p })
            .sum();
        Day(v as u8)
    }
}
impl Default for Day {
    #[inline]
    fn default() -> Day {
        Day::EVERYDAY
    }
}
impl From<u16> for Day {
    #[inline]
    fn from(v: u16) -> Day {
        Day(v as u8)
    }
}
impl From<u32> for Day {
    #[inline]
    fn from(v: u32) -> Day {
        Day(v as u8)
    }
}
impl From<u64> for Day {
    #[inline]
    fn from(v: u64) -> Day {
        Day(v as u8)
    }
}
impl PartialOrd for Day {
    #[inline]
    fn partial_cmp(&self, other: &Day) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl From<usize> for Day {
    #[inline]
    fn from(v: usize) -> Day {
        Day(v as u8)
    }
}

impl Copy for WorkHours {}
impl Clone for WorkHours {
    #[inline]
    fn clone(&self) -> WorkHours {
        WorkHours {
            days:       self.days,
            end_min:    self.end_min,
            end_hour:   self.end_hour,
            start_min:  self.start_min,
            start_hour: self.start_hour,
        }
    }
}
impl Default for WorkHours {
    #[inline]
    fn default() -> WorkHours {
        WorkHours::new()
    }
}
impl Writable for WorkHours {
    #[inline]
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        w.write_u8(self.days)?;
        w.write_u8(self.start_hour)?;
        w.write_u8(self.start_min)?;
        w.write_u8(self.end_hour)?;
        w.write_u8(self.end_min)
    }
}
impl Readable for WorkHours {
    #[inline]
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        r.read_into_u8(&mut self.days)?;
        r.read_into_u8(&mut self.start_hour)?;
        r.read_into_u8(&mut self.start_min)?;
        r.read_into_u8(&mut self.end_hour)?;
        r.read_into_u8(&mut self.end_min)
    }
}
impl<A: Allocator> Setting<A> for WorkHours {
    #[inline]
    fn len(&self) -> usize {
        6
    }
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        &EMPTY
    }
    #[inline]
    fn write(&self, buf: &mut Vec<u8, A>) {
        buf.push(SYS_WORK_HOURS);
        buf.push(self.days);
        buf.push(self.start_hour);
        buf.push(self.start_min);
        buf.push(self.end_hour);
        buf.push(self.end_min);
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter};

    use crate::c2::cfg::workhours::Day;
    use crate::prelude::*;
    use crate::util::BinaryIter;

    impl Debug for Day {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_tuple("Day").field(&self.0).finish()
        }
    }
    impl Display for Day {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let mut w = false;
            if *self == Day::EVERYDAY || *self >= 127 {
                return f.write_str("Everyday");
            }
            for (i, e) in BinaryIter::new(self.0 as usize, 6usize) {
                if !e || i > 6 {
                    continue;
                }
                if !w {
                    w = true
                } else {
                    f.write_str(", ")?;
                }
                match i {
                    0 => f.write_str("Sunday"),
                    1 => f.write_str("Monday"),
                    2 => f.write_str("Tuesday"),
                    3 => f.write_str("Wednesday"),
                    4 => f.write_str("Thursday"),
                    5 => f.write_str("Friday"),
                    6 => f.write_str("Saturday"),
                    _ => Ok(()),
                }?;
            }
            Ok(())
        }
    }
}
