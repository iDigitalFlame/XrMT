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

pub use self::inner::*;

#[cfg(all(target_family = "windows", not(feature = "std")))]
#[path = "device/winapi/std"]
mod inner {
    mod fs;
    pub use self::fs::*;
}
#[cfg(any(not(target_family = "windows"), feature = "std"))]
mod inner {
    extern crate std;

    pub use std::fs::*;

    use crate::path::Path;
    use crate::prelude::*;

    #[cfg(target_family = "windows")]
    impl crate::device::winapi::AsHandle for File {
        #[inline]
        fn as_handle(&self) -> crate::device::winapi::Handle {
            crate::device::winapi::Handle(std::os::windows::io::AsRawHandle::as_raw_handle(self) as usize)
        }
    }

    #[inline]
    pub fn exists(path: impl AsRef<Path>) -> bool {
        crate::fs::File::open(path).is_ok()
    }
}
