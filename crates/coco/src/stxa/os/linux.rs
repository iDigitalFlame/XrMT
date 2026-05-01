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
#![cfg(any(target_os = "linux", target_os = "android"))]

#[cfg(all(target_os = "linux", not(target_os = "android")))]
pub mod net {
    extern crate core;

    extern crate xrmt_stx;

    use core::time::Duration;

    use xrmt_stx::io::IoResult;
    use xrmt_stx::os::linux::net;

    use crate::stxa::net::TcpStream;

    pub trait TcpStreamExt {
        fn quickack(&self) -> IoResult<bool>;
        fn deferaccept(&self) -> IoResult<Duration>;
        fn set_quickack(&self, quickack: bool) -> IoResult<()>;
        fn set_deferaccept(&self, accept: Duration) -> IoResult<()>;
    }

    impl TcpStreamExt for TcpStream {
        #[inline]
        fn quickack(&self) -> IoResult<bool> {
            net::TcpStreamExt::quickack(&self.0)
        }
        #[inline]
        fn deferaccept(&self) -> IoResult<Duration> {
            net::TcpStreamExt::deferaccept(&self.0)
        }
        #[inline]
        fn set_quickack(&self, quickack: bool) -> IoResult<()> {
            net::TcpStreamExt::set_quickack(&self.0, quickack)
        }
        #[inline]
        fn set_deferaccept(&self, accept: Duration) -> IoResult<()> {
            net::TcpStreamExt::set_deferaccept(&self.0, accept)
        }
    }
}
#[cfg(all(not(target_os = "linux"), target_os = "android"))]
pub mod net {
    extern crate xrmt_stx;

    use xrmt_stx::io::IoResult;
    use xrmt_stx::os::android::net;

    use crate::stxa::net::TcpStream;

    pub trait TcpStreamExt {
        fn quickack(&self) -> IoResult<bool>;
        fn set_quickack(&self, quickack: bool) -> IoResult<()>;
    }

    impl TcpStreamExt for TcpStream {
        #[inline]
        fn quickack(&self) -> IoResult<bool> {
            net::TcpStreamExt::quickack(&self.0)
        }
        #[inline]
        fn set_quickack(&self, quickack: bool) -> IoResult<()> {
            net::TcpStreamExt::set_quickack(&self.0, quickack)
        }
    }
}
