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

extern crate alloc;
extern crate core;

extern crate xrmt_stx;

use xrmt_stx::io::IoResult;
use xrmt_stx::process::{self, ExitStatus};

use crate::future::F;

pub struct Child(process::Child);

impl Child {
    pub fn new(v: process::Child) -> Child {
        Child(v)
    }

    /*#[inline]
    pub async fn wait(&mut self) -> IoResult<ExitStatus> {

        //F::new(ChildExitWait::new(&mut self.0)).await
    }*/
}
