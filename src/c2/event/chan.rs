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

use crate::c2::task::Beacon;
use crate::prelude::*;
use crate::sync::mpsc::{SendError, SyncSender};

pub struct Broadcaster<T> {
    chan:   SyncSender<T>,
    beacon: Beacon,
}

impl<T> Broadcaster<T> {
    #[inline]
    pub fn new(b: Beacon, s: SyncSender<T>) -> Broadcaster<T> {
        Broadcaster { beacon: b, chan: s }
    }

    #[inline]
    pub fn signal(&self) {
        self.beacon.set()
    }
    #[inline]
    pub fn send(&self, v: T) -> Result<(), SendError<T>> {
        self.chan.send(v)?;
        self.beacon.set();
        Ok(())
    }
}
