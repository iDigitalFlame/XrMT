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

#[cfg(all(target_family = "windows", not(feature = "std")))]
pub use self::inner::*;

#[cfg(all(target_family = "windows", not(feature = "std")))]
#[path = "device/winapi/std/time.rs"]
mod inner;
#[cfg(any(not(target_family = "windows"), feature = "std"))]
mod inner {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::data::time::Time;
    use crate::prelude::*;

    impl From<SystemTime> for Time {
        #[inline]
        fn from(v: SystemTime) -> Time {
            Time::from_unix(
                v.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
                0,
            )
        }
    }
}
