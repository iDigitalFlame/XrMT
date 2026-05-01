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

#[doc(inline)]
pub extern crate alloc;
#[doc(inline)]
pub extern crate core;

pub use alloc::borrow::ToOwned;
pub use alloc::boxed::Box;
pub use alloc::string::{String, ToString};
pub use alloc::vec::Vec;
pub use core::clone::Clone;
pub use core::cmp::{Eq, Ord, PartialEq, PartialOrd};
pub use core::convert::{AsMut, AsRef, From, Into, TryFrom, TryInto};
pub use core::default::Default;
pub use core::fmt::Debug;
pub use core::future::{Future, IntoFuture};
pub use core::hash::Hash;
pub use core::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, FromIterator, IntoIterator, Iterator};
pub use core::marker::{Copy, Send, Sized, Sync, Unpin};
pub use core::mem::{align_of, align_of_val, drop, size_of, size_of_val};
pub use core::ops::{AsyncFn, AsyncFnMut, AsyncFnOnce, Drop, Fn, FnMut, FnOnce};
#[doc(inline)]
pub use core::option::Option::{self, None, Some};
#[doc(inline)]
pub use core::result::Result::{self, Err, Ok};
pub use core::{assert, cfg, column, compile_error, concat, env, file, format_args, format_args_nl, include, include_bytes, include_str, line, log_syntax, module_path, option_env, stringify, trace_macros};

pub use crate::{eprint, eprintln, print, println};
