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

pub use self::client::*;
pub use self::connect::*;
pub use self::error::*;
pub use self::session::*;
pub use self::transform::*;
pub use self::wrapper::*;

#[cfg_attr(rustfmt, rustfmt_skip)]
use self::state::*;
use self::vars::*;

pub mod cfg;
mod client;
mod connect;
mod error;
pub mod event;
mod mux;
mod session;
mod state;
pub mod task;
mod transform;
mod vars;
mod wrapper;
