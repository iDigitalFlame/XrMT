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

#[cfg(all(feature = "bugs", feature = "strip"))]
compile_error!("Cannot use 'bugs' and 'strip' at the same time!");

extern crate core;

use core::clone::Clone;
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::marker::Copy;
use core::option::Option::{self, Some};

pub use inner::*;

#[repr(u8)]
pub enum Level {
    Trace   = 0,
    Debug   = 1,
    Info    = 2,
    Warning = 3,
    Error   = 4,
    Fatal   = 5,
}

impl Eq for Level {}
impl Ord for Level {
    #[inline]
    fn cmp(&self, other: &Level) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}
impl Copy for Level {}
impl Clone for Level {
    #[inline]
    fn clone(&self) -> Level {
        *self
    }
}
impl PartialEq for Level {
    #[inline]
    fn eq(&self, other: &Level) -> bool {
        *self as u8 == *other as u8
    }
}
impl PartialOrd for Level {
    #[inline]
    fn partial_cmp(&self, other: &Level) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub mod prelude {
    pub use crate::{debug, error, fatal, info, trace, warning};
}

#[cfg(feature = "strip")]
mod inner {
    extern crate core;

    use core::clone::Clone;
    use core::convert::From;
    use core::marker::{Send, Sync};

    #[macro_export]
    macro_rules! info {
        ($dst:expr, $($arg:tt)*) => {{}};
    }
    #[macro_export]
    macro_rules! debug {
        ($dst:expr, $($arg:tt)*) => {{}};
    }
    #[macro_export]
    macro_rules! trace {
        ($dst:expr, $($arg:tt)*) => {{}};
    }
    #[macro_export]
    macro_rules! error {
        ($dst:expr, $($arg:tt)*) => {{}};
    }
    #[macro_export]
    macro_rules! fatal {
        ($dst:expr, $($arg:tt)*) => {{}};
    }
    #[macro_export]
    macro_rules! warning {
        ($dst:expr, $($arg:tt)*) => {{}};
    }

    pub enum Logger {
        None,
    }

    pub struct Log;
    pub struct RefLog;
    pub struct ThreadLog;

    impl ThreadLog {
        #[inline]
        pub fn new_ref(&self) -> RefLog {
            RefLog
        }
    }

    impl Clone for RefLog {
        #[inline]
        fn clone(&self) -> RefLog {
            RefLog
        }
    }

    impl Clone for ThreadLog {
        #[inline]
        fn clone(&self) -> ThreadLog {
            ThreadLog
        }
    }
    impl From<Log> for ThreadLog {
        #[inline]
        fn from(_v: Log) -> ThreadLog {
            ThreadLog
        }
    }

    unsafe impl Sync for Log {}
    unsafe impl Send for Log {}

    #[inline]
    pub fn none() -> Logger {
        Logger::None
    }
}
#[cfg(not(feature = "strip"))]
mod inner {
    extern crate alloc;
    extern crate core;

    extern crate xrmt_io;

    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::{String, ToString};
    use alloc::sync::{Arc, Weak};
    use core::cell::UnsafeCell;
    use core::clone::Clone;
    use core::convert::{AsMut, AsRef, From, Into, TryFrom, TryInto};
    use core::fmt::{self, Arguments, Debug, Display};
    use core::iter::{Extend, FromIterator, IntoIterator, Iterator};
    use core::marker::{Copy, Send, Sized, Sync, Unpin};
    use core::matches;
    use core::ops::{Drop, Fn, FnMut, FnOnce};
    use core::option::Option::{self, None, Some};
    use core::result::Result::{self, Err, Ok};

    use crate::Level;

    const NEWLINE: [u8; 1] = [b'\n'];

    #[macro_export]
    macro_rules! info {
        ($dst:expr, $($arg:tt)*) => {
            $dst.info_fmt(core::format_args!($($arg)*))
        };
    }
    #[macro_export]
    macro_rules! debug {
        ($dst:expr, $($arg:tt)*) => {
            $dst.debug_fmt(core::format_args!($($arg)*))
        };
    }
    #[macro_export]
    macro_rules! trace {
        ($dst:expr, $($arg:tt)*) => {
            $dst.trace_fmt(core::format_args!($($arg)*))
        };
    }
    #[macro_export]
    macro_rules! error {
        ($dst:expr, $($arg:tt)*) => {
            $dst.error_fmt(core::format_args!($($arg)*))
        };
    }
    #[macro_export]
    macro_rules! fatal {
        ($dst:expr, $($arg:tt)*) => {
            $dst.fatal_fmt(core::format_args!($($arg)*))
        };
    }
    #[macro_export]
    macro_rules! warning {
        ($dst:expr, $($arg:tt)*) => {
            $dst.warning_fmt(core::format_args!($($arg)*))
        };
    }

    pub enum Logger {
        None,
        File(File),
        Console(Stderr),
        Writer(Box<dyn Write>),
        Multiple(Box<MultiLog>),
    }

    pub struct Log {
        level:  Level,
        prefix: Option<String>,
        w:      UnsafeCell<Logger>,
    }
    pub struct MultiLog([Logger; 5]);
    pub struct RefLog(Weak<Mutex<Log>>);
    pub struct ThreadLog(Arc<Mutex<Log>>);

    pub trait MaybeLog {
        fn log(&self, level: Level, v: &str);
        fn log_fmt(&self, level: Level, args: Arguments<'_>);

        #[inline]
        fn info(&self, v: &str) {
            self.log(Level::Info, v)
        }
        #[inline]
        fn debug(&self, v: &str) {
            self.log(Level::Debug, v)
        }
        #[inline]
        fn trace(&self, v: &str) {
            self.log(Level::Trace, v)
        }
        #[inline]
        fn error(&self, v: &str) {
            self.log(Level::Error, v)
        }
        #[inline]
        fn fatal(&self, v: &str) {
            self.log(Level::Fatal, v)
        }
        #[inline]
        fn warning(&self, v: &str) {
            self.log(Level::Warning, v)
        }
        #[inline]
        fn info_fmt(&self, args: Arguments<'_>) {
            self.log_fmt(Level::Info, args)
        }
        #[inline]
        fn debug_fmt(&self, args: Arguments<'_>) {
            self.log_fmt(Level::Debug, args)
        }
        #[inline]
        fn trace_fmt(&self, args: Arguments<'_>) {
            self.log_fmt(Level::Trace, args)
        }
        #[inline]
        fn error_fmt(&self, args: Arguments<'_>) {
            self.log_fmt(Level::Error, args)
        }
        #[inline]
        fn fatal_fmt(&self, args: Arguments<'_>) {
            self.log_fmt(Level::Fatal, args)
        }
        #[inline]
        fn warning_fmt(&self, args: Arguments<'_>) {
            self.log_fmt(Level::Warning, args)
        }
    }
    pub trait MaybePrefix {
        fn into_prefix(self) -> Option<String>;
    }

    impl Log {
        #[inline]
        pub fn new(level: Level, prefix: impl MaybePrefix, w: Logger) -> Log {
            Log {
                level,
                w: UnsafeCell::new(w),
                prefix: prefix.into_prefix(),
            }
        }

        #[inline]
        pub fn level(&self) -> Level {
            self.level
        }
        #[inline]
        pub fn prefix(&self) -> Option<&str> {
            self.prefix.as_deref()
        }
        #[inline]
        pub fn set_level(&mut self, level: Level) {
            self.level = level
        }
        #[inline]
        pub fn set_prefix(&mut self, prefix: impl MaybePrefix) {
            self.prefix = prefix.into_prefix()
        }
    }
    impl Logger {
        #[inline]
        fn is_none(&self) -> bool {
            matches!(self, Logger::None)
        }
    }
    impl MultiLog {
        #[inline]
        pub fn new() -> MultiLog {
            MultiLog([Logger::None, Logger::None, Logger::None, Logger::None, Logger::None])
        }

        #[inline]
        pub fn len(&self) -> usize {
            // FIX
            self.0.iter().position(|v| v.is_none()).unwrap_or_else(|| self.0.len())
        }
        #[inline]
        pub fn add(&mut self, w: Logger) -> IoResult<()> {
            for i in 0..self.0.len() {
                if self.0[i].is_none() {
                    self.0[i] = w;
                    return Ok(());
                }
            }
            return Err(ErrorKind::TooManyLinks.into());
        }
    }
    impl ThreadLog {
        #[inline]
        pub fn new_ref(&self) -> RefLog {
            RefLog(Arc::downgrade(&self.0))
        }
    }

    impl MaybeLog for Log {
        fn log(&self, level: Level, v: &str) {
            if self.level > level {
                return;
            }
            let w = unsafe { &mut *self.w.get() };
            let s = Time::now();
            let d = s.date();
            let t = s.clock();
            ignore_error!(write!(
                w,
                "{}/{:02}/{:02} {:02}:{:02}:{:02} ",
                d.0, d.1 as u8, d.2, t.0, t.1, t.2
            ));
            ignore_error!(match self.prefix.as_ref() {
                Some(p) => write!(w, "[{}] {}: {}\n", p, level, v),
                None => write!(w, "{}: {}\n", level, v),
            });
            ignore_error!(w.flush());
        }
        fn log_fmt(&self, level: Level, args: Arguments<'_>) {
            if self.level > level {
                return;
            }
            let w = unsafe { &mut *self.w.get() };
            let s = Time::now();
            let d = s.date();
            let t = s.clock();
            ignore_error!(write!(
                w,
                "{}/{:02}/{:02} {:02}:{:02}:{:02} ",
                d.0, d.1 as u8, d.2, t.0, t.1, t.2
            ));
            ignore_error!(match self.prefix.as_ref() {
                Some(p) => write!(w, "[{}] {}: ", p, level),
                None => write!(w, "{}: ", level),
            });
            ignore_error!(w.write_fmt(args));
            ignore_error!(w.write(&NEWLINE));
            ignore_error!(w.flush());
        }
    }
    impl MaybeLog for RefLog {
        #[inline]
        fn log(&self, level: Level, v: &str) {
            let p = self.0.as_ptr();
            if !p.is_null() {
                if let Ok(l) = unsafe { &*p }.lock() {
                    l.log(level, v)
                }
            }
        }
        #[inline]
        fn log_fmt(&self, level: Level, args: Arguments<'_>) {
            let p = self.0.as_ptr();
            if !p.is_null() {
                if let Ok(l) = unsafe { &*p }.lock() {
                    l.log_fmt(level, args)
                }
            }
        }
    }
    impl MaybeLog for ThreadLog {
        #[inline]
        fn log(&self, level: Level, v: &str) {
            if let Ok(l) = self.0.lock() {
                l.log(level, v)
            }
        }
        #[inline]
        fn log_fmt(&self, level: Level, args: Arguments<'_>) {
            if let Ok(l) = self.0.lock() {
                l.log_fmt(level, args)
            }
        }
    }
    impl MaybeLog for Option<Log> {
        #[inline]
        fn log(&self, level: Level, v: &str) {
            if let Some(l) = self {
                l.log(level, v)
            }
        }
        #[inline]
        fn log_fmt(&self, level: Level, args: Arguments<'_>) {
            if let Some(l) = self {
                l.log_fmt(level, args)
            }
        }
    }

    impl Debug for Level {
        #[inline]
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Level {
        #[inline]
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(match self {
                Level::Trace => "TRACE",
                Level::Debug => "DEBUG",
                Level::Info => " INFO",
                Level::Warning => "WARN",
                Level::Error => "ERROR",
                Level::Fatal => "FATAL",
            })
        }
    }

    impl Write for Logger {
        #[inline]
        fn flush(&mut self) -> IoResult<()> {
            match self {
                Logger::None => Ok(()),
                Logger::File(f) => f.flush(),
                Logger::Console(c) => c.flush(),
                Logger::Writer(w) => w.flush(),
                Logger::Multiple(m) => m.flush(),
            }
        }
        #[inline]
        fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
            match self {
                Logger::None => Ok(buf.len()),
                Logger::File(f) => f.write(buf),
                Logger::Console(c) => c.write(buf),
                Logger::Writer(w) => w.write(buf),
                Logger::Multiple(m) => m.write(buf),
            }
        }
        #[inline]
        fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
            match self {
                Logger::None => Ok(()),
                Logger::File(f) => f.write_all(buf),
                Logger::Console(c) => c.write_all(buf),
                Logger::Writer(w) => w.write_all(buf),
                Logger::Multiple(m) => m.write_all(buf),
            }
        }
        #[inline]
        fn write_fmt(&mut self, args: Arguments<'_>) -> IoResult<()> {
            match self {
                Logger::None => Ok(()),
                Logger::File(f) => f.write_fmt(args),
                Logger::Console(c) => c.write_fmt(args),
                Logger::Writer(w) => w.write_fmt(args),
                Logger::Multiple(m) => m.write_fmt(args),
            }
        }
    }
    impl Write for MultiLog {
        #[inline]
        fn flush(&mut self) -> IoResult<()> {
            for i in self.0.iter_mut() {
                if i.is_none() {
                    break;
                }
                i.flush()?;
            }
            Ok(())
        }
        #[inline]
        fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
            for i in self.0.iter_mut() {
                if i.is_none() {
                    break;
                }
                i.write(buf)?;
            }
            Ok(buf.len())
        }
        #[inline]
        fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
            for i in self.0.iter_mut() {
                if i.is_none() {
                    break;
                }
                i.write_all(buf)?;
            }
            Ok(())
        }
        #[inline]
        fn write_fmt(&mut self, args: Arguments<'_>) -> IoResult<()> {
            for i in self.0.iter_mut() {
                if i.is_none() {
                    break;
                }
                i.write_fmt(args)?;
            }
            Ok(())
        }
    }

    impl From<File> for Logger {
        #[inline]
        fn from(v: File) -> Logger {
            Logger::File(v)
        }
    }
    impl From<Stderr> for Logger {
        #[inline]
        fn from(v: Stderr) -> Logger {
            Logger::Console(v)
        }
    }
    impl From<MultiLog> for Logger {
        #[inline]
        fn from(v: MultiLog) -> Logger {
            Logger::Multiple(Box::new(v))
        }
    }

    impl Clone for RefLog {
        #[inline]
        fn clone(&self) -> RefLog {
            RefLog(self.0.clone())
        }
    }

    impl Clone for ThreadLog {
        #[inline]
        fn clone(&self) -> ThreadLog {
            ThreadLog(self.0.clone())
        }
    }
    impl From<Log> for ThreadLog {
        #[inline]
        fn from(v: Log) -> ThreadLog {
            ThreadLog(Arc::new(Mutex::new(v)))
        }
    }

    impl From<[Logger; 1]> for MultiLog {
        #[inline]
        fn from(v: [Logger; 1]) -> MultiLog {
            let mut m = MultiLog::new();
            match v {
                // Hack to TAKE elements from array and destructure it.
                [a] => {
                    // We ignore the errors as they can't happen.
                    ignore_error!(m.add(a));
                },
            }
            m
        }
    }
    impl From<[Logger; 2]> for MultiLog {
        #[inline]
        fn from(v: [Logger; 2]) -> MultiLog {
            let mut m = MultiLog::new();
            match v {
                // Hack to TAKE elements from array and destructure it.
                [a, b] => {
                    // We ignore the errors as they can't happen.
                    ignore_error!(m.add(a));
                    ignore_error!(m.add(b));
                },
            }
            m
        }
    }
    impl From<[Logger; 3]> for MultiLog {
        #[inline]
        fn from(v: [Logger; 3]) -> MultiLog {
            let mut m = MultiLog::new();
            match v {
                // Hack to TAKE elements from array and destructure it.
                [a, b, c] => {
                    // We ignore the errors as they can't happen.
                    ignore_error!(m.add(a));
                    ignore_error!(m.add(b));
                    ignore_error!(m.add(c));
                },
            }
            m
        }
    }
    impl From<[Logger; 4]> for MultiLog {
        #[inline]
        fn from(v: [Logger; 4]) -> MultiLog {
            let mut m = MultiLog::new();
            match v {
                // Hack to TAKE elements from array and destructure it.
                [a, b, c, d] => {
                    // We ignore the errors as they can't happen.
                    ignore_error!(m.add(a));
                    ignore_error!(m.add(b));
                    ignore_error!(m.add(c));
                    ignore_error!(m.add(d));
                },
            }
            m
        }
    }
    impl From<[Logger; 5]> for MultiLog {
        #[inline]
        fn from(v: [Logger; 5]) -> MultiLog {
            let mut m = MultiLog::new();
            match v {
                // Hack to TAKE elements from array and destructure it.
                [a, b, c, d, e] => {
                    // We ignore the errors as they can't happen.
                    ignore_error!(m.add(a));
                    ignore_error!(m.add(b));
                    ignore_error!(m.add(c));
                    ignore_error!(m.add(d));
                    ignore_error!(m.add(e));
                },
            }
            m
        }
    }

    impl MaybePrefix for &str {
        #[inline]
        fn into_prefix(self) -> Option<String> {
            Option::Some(self.to_string())
        }
    }
    impl MaybePrefix for String {
        #[inline]
        fn into_prefix(self) -> Option<String> {
            Some(self)
        }
    }
    impl MaybePrefix for Option<&str> {
        #[inline]
        fn into_prefix(self) -> Option<String> {
            self.map(|s| s.to_string())
        }
    }
    impl MaybePrefix for Cow<'_, str> {
        #[inline]
        fn into_prefix(self) -> Option<String> {
            Some(self.to_string())
        }
    }

    impl TryFrom<&str> for Logger {
        type Error = Error;

        #[inline]
        fn try_from(v: &str) -> IoResult<Logger> {
            file(v)
        }
    }
    impl TryFrom<String> for Logger {
        type Error = Error;

        #[inline]
        fn try_from(v: String) -> IoResult<Logger> {
            file(v)
        }
    }

    unsafe impl Sync for Log {}
    unsafe impl Send for Log {}

    #[inline]
    pub fn none() -> Logger {
        Logger::None
    }
    #[inline]
    pub fn console() -> Logger {
        Logger::Console(io::stderr())
    }
    #[inline]
    pub fn file(path: impl AsRef<Path>) -> IoResult<Logger> {
        OpenOptions::new()
            .write(true)
            .append(true)
            .create(true)
            .open(path)
            .map(|f| f.into())
    }
    #[inline]
    pub fn stderr(level: Level, prefix: impl MaybePrefix) -> Log {
        Log::new(level, prefix, Logger::Console(io::stderr()))
    }
}
