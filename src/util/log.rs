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

pub use inner::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Level {
    Trace   = 0,
    Debug   = 1,
    Info    = 2,
    Warning = 3,
    Error   = 4,
    Fatal   = 5,
}

pub mod prelude {
    pub use crate::{debug, error, fatal, info, trace, warning};
}

#[cfg(feature = "implant")]
mod inner {
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

    pub struct Log {}
}
#[cfg(not(feature = "implant"))]
mod inner {
    pub(super) const NEWLINE: [u8; 1] = [b'\n'];

    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use core::cell::UnsafeCell;
    use core::fmt::{self, Arguments, Debug, Display};
    use core::mem::MaybeUninit;

    use super::Level;
    use crate::data::time::Time;
    use crate::device::fs::{File, OpenOptions};
    use crate::util::stx::ffi::Path;
    use crate::util::stx::io::{self, Error, ErrorKind, Stderr, Write};
    use crate::util::stx::prelude::*;

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
        Console(Stderr),
        File(File),
        Writer(Box<dyn Write>),
        Multiple(Box<MultiLog>),
    }

    pub struct Log {
        level:  Level,
        prefix: Option<String>,
        w:      UnsafeCell<Logger>,
    }
    pub struct MultiLog {
        count: u8,
        inner: [MaybeUninit<UnsafeCell<Logger>>; 5],
    }

    pub trait MaybeLog {
        fn info(&self, v: &str);
        fn debug(&self, v: &str);
        fn trace(&self, v: &str);
        fn error(&self, v: &str);
        fn fatal(&self, v: &str);
        fn warning(&self, v: &str);
        fn info_fmt(&self, args: Arguments<'_>);
        fn debug_fmt(&self, args: Arguments<'_>);
        fn trace_fmt(&self, args: Arguments<'_>);
        fn error_fmt(&self, args: Arguments<'_>);
        fn fatal_fmt(&self, args: Arguments<'_>);
        fn warning_fmt(&self, args: Arguments<'_>);
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
        pub fn info(&self, v: &str) {
            self.log(Level::Info, v)
        }
        #[inline]
        pub fn debug(&self, v: &str) {
            self.log(Level::Debug, v)
        }
        #[inline]
        pub fn trace(&self, v: &str) {
            self.log(Level::Trace, v)
        }
        #[inline]
        pub fn error(&self, v: &str) {
            self.log(Level::Error, v)
        }
        #[inline]
        pub fn fatal(&self, v: &str) {
            self.log(Level::Fatal, v)
        }
        #[inline]
        pub fn level(&self) -> Level {
            self.level
        }
        #[inline]
        pub fn warning(&self, v: &str) {
            self.log(Level::Warning, v)
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
        pub fn info_fmt(&self, args: Arguments<'_>) {
            self.log_fmt(Level::Info, args)
        }
        #[inline]
        pub fn debug_fmt(&self, args: Arguments<'_>) {
            self.log_fmt(Level::Debug, args)
        }
        #[inline]
        pub fn trace_fmt(&self, args: Arguments<'_>) {
            self.log_fmt(Level::Trace, args)
        }
        #[inline]
        pub fn error_fmt(&self, args: Arguments<'_>) {
            self.log_fmt(Level::Error, args)
        }
        #[inline]
        pub fn fatal_fmt(&self, args: Arguments<'_>) {
            self.log_fmt(Level::Fatal, args)
        }
        #[inline]
        pub fn warning_fmt(&self, args: Arguments<'_>) {
            self.log_fmt(Level::Warning, args)
        }
        #[inline]
        pub fn set_prefix(&mut self, prefix: impl MaybePrefix) {
            self.prefix = prefix.into_prefix()
        }

        fn log(&self, level: Level, v: &str) {
            if self.level > level {
                return;
            }
            let w = unsafe { &mut *self.w.get() };
            let s = Time::now();
            let d = s.date();
            let t = s.clock();
            let _ = write!(
                w,
                "{}/{:02}/{:02} {:02}:{:02}:{:02} ",
                d.0, d.1 as u8, d.2, t.0, t.1, t.2
            );
            let _ = match self.prefix.as_ref() {
                Some(p) => write!(w, "[{}] {}: {}\n", p, level, v),
                None => write!(w, "{}: {}\n", level, v),
            };
            let _ = w.flush();
        }
        fn log_fmt(&self, level: Level, args: Arguments<'_>) {
            if self.level > level {
                return;
            }
            let w = unsafe { &mut *self.w.get() };
            let s = Time::now();
            let d = s.date();
            let t = s.clock();
            let _ = write!(
                w,
                "{}/{:02}/{:02} {:02}:{:02}:{:02} ",
                d.0, d.1 as u8, d.2, t.0, t.1, t.2
            );
            let _ = match self.prefix.as_ref() {
                Some(p) => write!(w, "[{}] {}: ", p, level),
                None => write!(w, "{}: ", level),
            };
            let _ = w.write_fmt(args);
            let _ = w.write(&NEWLINE);
            let _ = w.flush();
        }
    }
    impl MultiLog {
        #[inline]
        pub fn new() -> MultiLog {
            MultiLog {
                count: 0,
                inner: [
                    MaybeUninit::uninit(),
                    MaybeUninit::uninit(),
                    MaybeUninit::uninit(),
                    MaybeUninit::uninit(),
                    MaybeUninit::uninit(),
                ],
            }
        }

        #[inline]
        pub fn len(&self) -> usize {
            self.count as usize
        }
        #[inline]
        pub fn add(&mut self, w: Logger) -> io::Result<()> {
            if self.count > 4 {
                return Err(ErrorKind::TooManyLinks.into());
            }
            self.inner[self.count as usize].write(UnsafeCell::new(w));
            self.count += 1;
            Ok(())
        }
    }

    impl MaybeLog for Log {
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
    impl MaybeLog for Option<Log> {
        #[inline]
        fn info(&self, v: &str) {
            if let Some(l) = self {
                l.log(Level::Info, v)
            }
        }
        #[inline]
        fn debug(&self, v: &str) {
            if let Some(l) = self {
                l.log(Level::Debug, v)
            }
        }
        #[inline]
        fn trace(&self, v: &str) {
            if let Some(l) = self {
                l.log(Level::Trace, v)
            }
        }
        #[inline]
        fn error(&self, v: &str) {
            if let Some(l) = self {
                l.log(Level::Error, v)
            }
        }
        #[inline]
        fn fatal(&self, v: &str) {
            if let Some(l) = self {
                l.log(Level::Fatal, v)
            }
        }
        #[inline]
        fn warning(&self, v: &str) {
            if let Some(l) = self {
                l.log(Level::Warning, v)
            }
        }
        #[inline]
        fn info_fmt(&self, args: Arguments<'_>) {
            if let Some(l) = self {
                l.log_fmt(Level::Info, args)
            }
        }
        #[inline]
        fn debug_fmt(&self, args: Arguments<'_>) {
            if let Some(l) = self {
                l.log_fmt(Level::Debug, args)
            }
        }
        #[inline]
        fn trace_fmt(&self, args: Arguments<'_>) {
            if let Some(l) = self {
                l.log_fmt(Level::Trace, args)
            }
        }
        #[inline]
        fn error_fmt(&self, args: Arguments<'_>) {
            if let Some(l) = self {
                l.log_fmt(Level::Error, args)
            }
        }
        #[inline]
        fn fatal_fmt(&self, args: Arguments<'_>) {
            if let Some(l) = self {
                l.log_fmt(Level::Fatal, args)
            }
        }
        #[inline]
        fn warning_fmt(&self, args: Arguments<'_>) {
            if let Some(l) = self {
                l.log_fmt(Level::Warning, args)
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
        fn flush(&mut self) -> io::Result<()> {
            match self {
                Logger::Console(c) => c.flush(),
                Logger::File(f) => f.flush(),
                Logger::Writer(w) => w.flush(),
                Logger::Multiple(m) => m.flush(),
            }
        }
        #[inline]
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            match self {
                Logger::Console(c) => c.write(buf),
                Logger::File(f) => f.write(buf),
                Logger::Writer(w) => w.write(buf),
                Logger::Multiple(m) => m.write(buf),
            }
        }
        #[inline]
        fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
            match self {
                Logger::Console(c) => c.write_all(buf),
                Logger::File(f) => f.write_all(buf),
                Logger::Writer(w) => w.write_all(buf),
                Logger::Multiple(m) => m.write_all(buf),
            }
        }
        #[inline]
        fn write_fmt(&mut self, args: Arguments<'_>) -> io::Result<()> {
            match self {
                Logger::Console(c) => c.write_fmt(args),
                Logger::File(f) => f.write_fmt(args),
                Logger::Writer(w) => w.write_fmt(args),
                Logger::Multiple(m) => m.write_fmt(args),
            }
        }
    }
    impl Write for MultiLog {
        #[inline]
        fn flush(&mut self) -> io::Result<()> {
            for i in 0..self.count {
                unsafe { &mut *(&*(&self.inner[i as usize]).as_ptr()).get() }.flush()?;
            }
            Ok(())
        }
        #[inline]
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            for i in 0..self.count {
                unsafe { &mut *(&*(&self.inner[i as usize]).as_ptr()).get() }.write(buf)?;
            }
            Ok(buf.len())
        }
        #[inline]
        fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
            for i in 0..self.count {
                unsafe { &mut *(&*(&self.inner[i as usize]).as_ptr()).get() }.write_all(buf)?;
            }
            Ok(())
        }
        #[inline]
        fn write_fmt(&mut self, args: Arguments<'_>) -> io::Result<()> {
            for i in 0..self.count {
                unsafe { &mut *(&*(&self.inner[i as usize]).as_ptr()).get() }.write_fmt(args)?;
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
        fn try_from(v: &str) -> io::Result<Logger> {
            file(v)
        }
    }
    impl TryFrom<String> for Logger {
        type Error = Error;

        #[inline]
        fn try_from(v: String) -> io::Result<Logger> {
            file(v)
        }
    }

    unsafe impl Sync for Log {}
    unsafe impl Send for Log {}

    #[inline]
    pub fn console() -> Logger {
        Logger::Console(io::stderr())
    }
    #[inline]
    pub fn file(path: impl AsRef<Path>) -> io::Result<Logger> {
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

#[cfg(all(feature = "bugs", not(feature = "implant")))]
pub(crate) mod bugs {
    use core::fmt::Arguments;

    use crate::device::env;
    use crate::process;
    use crate::sync::LazyLock;
    use crate::util::log::{self, Level, Log, MultiLog};
    use crate::util::stx::io::{self, Write};
    use crate::util::stx::prelude::*;

    static BUGLOG: LazyLock<Log> = LazyLock::new(|| {
        let c = log::console();
        let t = env::temp_dir().join(format!("bugtrack-{}.log", process::id()));
        let (o, z) = match log::file(&t) {
            Err(_) => (c, false),
            Ok(f) => {
                let mut m = MultiLog::new();
                let _ = m.add(c);
                let _ = m.add(f);
                (m.into(), true)
            },
        };
        let l = Log::new(Level::Trace, "BUGTRACK", o);
        if z {
            crate::info!(
                l,
                "Bugtrack log init complete! Log file located at \"{}\"",
                t.to_string_lossy()
            );
        } else {
            crate::info!(l, "Bugtrack log init complete!");
        }
        l
    });

    #[macro_export]
    macro_rules! bugtrack {
        ($($arg:tt)*) => {
            crate::util::log::bugs::_bugtrack(format_args!($($arg)*))
        };
    }
    #[macro_export]
    macro_rules! bugprint {
        ($($arg:tt)*) => {
            crate::util::log::bugs::_bugprint(format_args!($($arg)*))
        };
    }

    #[inline]
    pub(crate) fn _bugprint(args: Arguments<'_>) {
        let _ = io::stderr().write_fmt(args); // IGNORE ERROR
        let _ = io::stderr().write(&super::inner::NEWLINE); // IGNORE ERROR
    }
    #[inline]
    pub(crate) fn _bugtrack(args: Arguments<'_>) {
        BUGLOG.trace_fmt(args)
    }
}
#[cfg(any(not(feature = "bugs"), feature = "implant"))]
pub(crate) mod bugs {
    #[macro_export]
    macro_rules! bugtrack {
        ($($args:tt)*) => {{}};
    }
    #[macro_export]
    macro_rules! bugprint {
        ($($arg:tt)*) => {{}};
    }
}
