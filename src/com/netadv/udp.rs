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

use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use core::cell::UnsafeCell;
use core::cmp;
use core::net::SocketAddr;
use core::time::Duration;

use crate::com::netadv::{Conn, Listener};
use crate::io::{self, ErrorKind, Read, Write};
use crate::net::UdpSocket;
use crate::prelude::*;
use crate::sync::mpsc::{channel, Receiver, Sender};
use crate::sync::{ArcMut, Event, Mutex, WeakMut};
use crate::thread::Builder;

pub struct Udp;
pub struct UdpListener {
    new:    Receiver<UdpConn>,
    thread: WeakMut<UdpThread>,
}
pub struct UdpSteam(UdpSocket);
pub struct UdpConn(Arc<UdpSock>);

struct UdpSock {
    buf:   Mutex<Vec<u8>>,
    addr:  SocketAddr,
    time:  UnsafeCell<Option<Duration>>,
    sock:  Weak<UdpSocket>,
    local: SocketAddr,
    event: Event,
}
struct UdpThread {
    new:  Sender<UdpConn>,
    sock: Arc<UdpSocket>,
    cons: BTreeMap<SocketAddr, Weak<UdpSock>>,
}

impl UdpThread {
    fn run(&mut self) {
        let mut b = [0u8; 4096];
        loop {
            match self.sock.recv_from(&mut b) {
                Err(_) => break,
                Ok((c, a)) => {
                    if !self.check(&b, c, a) {
                        break;
                    }
                },
            }
        }
    }
    fn check(&mut self, buf: &[u8], c: usize, addr: SocketAddr) -> bool {
        if let Some(x) = self.cons.get(&addr) {
            if let Some(v) = x.upgrade() {
                if let Ok(mut d) = v.buf.lock() {
                    d.extend_from_slice(&buf[0..c]);
                    v.event.set_ignore();
                }
                return true;
            }
            self.cons.remove(&addr);
        }
        let n = Arc::new(UdpSock {
            addr,
            buf: Mutex::new(buf[0..c].to_vec()),
            sock: Arc::downgrade(&self.sock),
            time: UnsafeCell::new(None),
            local: self.sock.local_addr().unwrap_or(addr),
            event: Event::new(),
        });
        n.event.set_ignore();
        self.cons.insert(addr, Arc::downgrade(&n));
        self.new.send(UdpConn(n)).is_ok()
    }
}

impl Conn for UdpConn {
    #[inline]
    fn peer_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.0.addr)
    }
    #[inline]
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.0.local)
    }
    #[inline]
    fn read_timeout(&self) -> io::Result<Option<Duration>> {
        Ok(unsafe { *self.0.time.get() })
    }
    #[inline]
    fn write_timeout(&self) -> io::Result<Option<Duration>> {
        Ok(None)
    }
    #[inline]
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        unsafe { *self.0.time.get() = dur };
        Ok(())
    }
    #[inline]
    fn set_write_timeout(&self, _dur: Option<Duration>) -> io::Result<()> {
        Ok(())
    }
}
impl Read for UdpConn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        match unsafe { *self.0.time.get() } {
            Some(d) => {
                if self.0.event.wait_for(d).is_err() {
                    return Err(ErrorKind::TimedOut.into());
                }
            },
            None => self.0.event.wait(),
        }
        let mut b = unwrap_unlikely(self.0.buf.lock());
        let n = cmp::min(b.len(), buf.len());
        for (i, v) in b.drain(0..n).enumerate() {
            buf[i] = v;
        }
        if b.is_empty() {
            self.0.event.reset_ignore()
        }
        Ok(n)
    }
}
impl Write for UdpConn {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.0.sock.upgrade() {
            Some(c) => c.send_to(buf, self.0.addr),
            None => Err(ErrorKind::ConnectionAborted.into()),
        }
    }
}

impl Conn for UdpSteam {
    #[inline]
    fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.peer_addr()
    }
    #[inline]
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.local_addr()
    }
    #[inline]
    fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.read_timeout()
    }
    #[inline]
    fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.write_timeout()
    }
    #[inline]
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(dur)
    }
    #[inline]
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.0.set_write_timeout(dur)
    }
}
impl Read for UdpSteam {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.recv(buf)
    }
}
impl Write for UdpSteam {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(buf)
    }
}

impl Listener for UdpListener {
    #[inline]
    fn local_addr(&self) -> io::Result<SocketAddr> {
        match self.thread.upgrade() {
            Some(x) => x.sock.local_addr(),
            None => Err(ErrorKind::AddrNotAvailable.into()),
        }
    }
    #[inline]
    fn accept(&self) -> io::Result<(Box<dyn Conn>, SocketAddr)> {
        match self.new.recv() {
            Ok(c) => {
                let a = c.0.addr;
                Ok((Box::new(c), a))
            },
            Err(_) => Err(ErrorKind::ConnectionAborted.into()),
        }
    }
}

pub fn udp_listen(addr: impl AsRef<str>) -> io::Result<UdpListener> {
    let l = UdpSocket::bind(addr.as_ref())?;
    let (s, r) = channel();
    let mut t = ArcMut::new(UdpThread {
        new:  s,
        sock: Arc::new(l),
        cons: BTreeMap::new(),
    });
    let n = UdpListener {
        new:    r,
        thread: ArcMut::downgrade(&t),
    };
    if Builder::new().spawn(move || t.run()).is_err() {
        Err(ErrorKind::ResourceBusy.into())
    } else {
        Ok(n)
    }
}
#[inline]
pub fn udp_connect(addr: impl AsRef<str>, dur: Option<Duration>) -> io::Result<UdpSteam> {
    let c = UdpSocket::bind("0.0.0.0:0")?;
    c.connect(addr.as_ref())?;
    if dur.is_some() {
        c.set_read_timeout(dur)?;
        c.set_write_timeout(dur)?;
    }
    Ok(UdpSteam(c))
}
