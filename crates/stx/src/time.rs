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

//! Temporal quantification.
//!
//! # Examples
//!
//! There are multiple ways to create a new [`Duration`]:
//!
//! ```
//! # use xrmt_stx::time::Duration;
//! let five_seconds = Duration::from_secs(5);
//! assert_eq!(five_seconds, Duration::from_millis(5_000));
//! assert_eq!(five_seconds, Duration::from_micros(5_000_000));
//! assert_eq!(five_seconds, Duration::from_nanos(5_000_000_000));
//!
//! let ten_seconds = Duration::from_secs(10);
//! let seven_nanos = Duration::from_nanos(7);
//! let total = ten_seconds + seven_nanos;
//! assert_eq!(total, Duration::new(10, 7));
//! ```
//!
//! Using [`Instant`] to calculate how long a function took to run:
//!
//! ```ignore (incomplete)
//! let now = Instant::now();
//!
//! // Calling a slow function, it may take a while
//! slow_function();
//!
//! let elapsed_time = now.elapsed();
//! println!("Running slow_function() took {} seconds.", elapsed_time.as_secs());
//! ```

#![no_implicit_prelude]

pub use self::inner::*;

#[cfg(all(target_family = "windows", not(feature = "std")))]
mod inner {
    extern crate core;

    extern crate xrmt_time;

    use core::clone::Clone;
    use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
    use core::convert::From;
    use core::error::Error;
    use core::fmt::{Debug, Display, Formatter};
    use core::hash::{Hash, Hasher};
    use core::marker::Copy;
    use core::ops::{Add, AddAssign, Sub, SubAssign};
    use core::option::Option::{self, None, Some};
    use core::result::Result::{self, Err, Ok};

    use xrmt_time::Time;

    use crate::io::FmtResult;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub use core::time::*;

    /// A measurement of a monotonically nondecreasing clock.
    /// Opaque and useful only with [`Duration`].
    ///
    /// Instants are always guaranteed, barring [platform bugs], to be no less
    /// than any previously measured instant when created, and are often
    /// useful for tasks such as measuring benchmarks or timing how long an
    /// operation takes.
    ///
    /// Note, however, that instants are **not** guaranteed to be **steady**. In
    /// other words, each tick of the underlying clock might not be the same
    /// length (e.g. some seconds may be longer than others). An instant may
    /// jump forwards or experience time dilation (slow down or speed up),
    /// but it will never go backwards.
    /// As part of this non-guarantee it is also not specified whether system
    /// suspends count as elapsed time or not. The behavior varies across
    /// platforms and Rust versions.
    ///
    /// Instants are opaque types that can only be compared to one another.
    /// There is no method to get "the number of seconds" from an instant.
    /// Instead, it only allows measuring the duration between two instants
    /// (or comparing two instants).
    ///
    /// The size of an `Instant` struct may vary depending on the target
    /// operating system.
    ///
    /// Example:
    ///
    /// ```no_run
    /// use xrmt_stx::time::{Duration, Instant};
    /// use xrmt_stx::thread::sleep;
    ///
    /// fn main() {
    ///    let now = Instant::now();
    ///
    ///    // we sleep for 2 seconds
    ///    sleep(Duration::new(2, 0));
    ///    // it prints '2'
    ///    println!("{}", now.elapsed().as_secs());
    /// }
    /// ```
    ///
    /// [platform bugs]: Instant#monotonicity
    ///
    /// # OS-specific behaviors
    ///
    /// An `Instant` is a wrapper around system-specific types and it may behave
    /// differently depending on the underlying operating system. For example,
    /// the following snippet is fine on Linux but panics on macOS:
    ///
    /// ```no_run
    /// use xrmt_stx::time::{Instant, Duration};
    ///
    /// let now = Instant::now();
    /// let days_per_10_millennia = 365_2425;
    /// let solar_seconds_per_day = 60 * 60 * 24;
    /// let millenium_in_solar_seconds = 31_556_952_000;
    /// assert_eq!(millenium_in_solar_seconds, days_per_10_millennia * solar_seconds_per_day / 10);
    ///
    /// let duration = Duration::new(millenium_in_solar_seconds, 0);
    /// println!("{:?}", now + duration);
    /// ```
    ///
    /// For cross-platform code, you can comfortably use durations of up to
    /// around one hundred years.
    ///
    /// # Underlying System calls
    ///
    /// The following system calls are [currently] being used by `now()` to find
    /// out the current time:
    ///
    /// |  Platform |               System call                                            |
    /// |-----------|----------------------------------------------------------------------|
    /// | SGX       | [`insecure_time` usercall]. More information on [timekeeping in SGX] |
    /// | UNIX      | [clock_gettime (Monotonic Clock)]                                    |
    /// | Darwin    | [clock_gettime (Monotonic Clock)]                                    |
    /// | VXWorks   | [clock_gettime (Monotonic Clock)]                                    |
    /// | SOLID     | `get_tim`                                                            |
    /// | WASI      | [__wasi_clock_time_get (Monotonic Clock)]                            |
    /// | Windows   | [QueryPerformanceCounter]                                            |
    ///
    /// [currently]: crate::io#platform-specific-behavior
    /// [QueryPerformanceCounter]: https://docs.microsoft.com/en-us/windows/win32/api/profileapi/nf-profileapi-queryperformancecounter
    /// [`insecure_time` usercall]: https://edp.fortanix.com/docs/api/fortanix_sgx_abi/struct.Usercalls.html#method.insecure_time
    /// [timekeeping in SGX]: https://edp.fortanix.com/docs/concepts/rust-std/#codestdtimecode
    /// [__wasi_clock_time_get (Monotonic Clock)]: https://github.com/WebAssembly/WASI/blob/main/legacy/preview1/docs.md#clock_time_get
    /// [clock_gettime (Monotonic Clock)]: https://linux.die.net/man/3/clock_gettime
    ///
    /// **Disclaimer:** These system calls might change over time.
    ///
    /// > Note: mathematical operations like [`add`] may panic if the underlying
    /// > structure cannot represent the new point in time.
    ///
    /// [`add`]: Instant::add
    ///
    /// ## Monotonicity
    ///
    /// On all platforms `Instant` will try to use an OS API that guarantees
    /// monotonic behavior if available, which is the case for all [tier 1]
    /// platforms. In practice such guarantees are – under rare circumstances –
    /// broken by hardware, virtualization or operating system bugs. To work
    /// around these bugs and platforms not offering monotonic clocks
    /// [`duration_since`], [`elapsed`] and [`sub`] saturate to zero. In
    /// older Rust versions this lead to a panic instead.
    /// [`checked_duration_since`] can be used to detect and handle
    /// situations where monotonicity is violated, or `Instant`s are
    /// subtracted in the wrong order.
    ///
    /// This workaround obscures programming errors where earlier and later
    /// instants are accidentally swapped. For this reason future Rust
    /// versions may reintroduce panics.
    ///
    /// [tier 1]: https://doc.rust-lang.org/rustc/platform-support.html
    /// [`duration_since`]: Instant::duration_since
    /// [`elapsed`]: Instant::elapsed
    /// [`sub`]: Instant::sub
    /// [`checked_duration_since`]: Instant::checked_duration_since
    pub struct Instant(Time);
    /// A measurement of the system clock, useful for talking to
    /// external entities like the file system or other processes.
    ///
    /// Distinct from the [`Instant`] type, this time measurement **is not
    /// monotonic**. This means that you can save a file to the file system,
    /// then save another file to the file system, **and the second file has
    /// a `SystemTime` measurement earlier than the first**. In other words,
    /// an operation that happens after another operation in real time may
    /// have an earlier `SystemTime`!
    ///
    /// Consequently, comparing two `SystemTime` instances to learn about the
    /// duration between them returns a [`Result`] instead of an infallible
    /// [`Duration`] to indicate that this sort of time drift may happen and
    /// needs to be handled.
    ///
    /// Although a `SystemTime` cannot be directly inspected, the [`UNIX_EPOCH`]
    /// constant is provided in this module as an anchor in time to learn
    /// information about a `SystemTime`. By calculating the duration from this
    /// fixed point in time, a `SystemTime` can be converted to a human-readable
    /// time, or perhaps some other string representation.
    ///
    /// The size of a `SystemTime` struct may vary depending on the target
    /// operating system.
    ///
    /// A `SystemTime` does not count leap seconds.
    /// `SystemTime::now()`'s behavior around a leap second
    /// is the same as the operating system's wall clock.
    /// The precise behavior near a leap second
    /// (e.g. whether the clock appears to run slow or fast, or stop, or jump)
    /// depends on platform and configuration,
    /// so should not be relied on.
    ///
    /// Example:
    ///
    /// ```no_run
    /// use xrmt_stx::time::{Duration, SystemTime};
    /// use xrmt_stx::thread::sleep;
    ///
    /// fn main() {
    ///    let now = SystemTime::now();
    ///
    ///    // we sleep for 2 seconds
    ///    sleep(Duration::new(2, 0));
    ///    match now.elapsed() {
    ///        Ok(elapsed) => {
    ///            // it prints '2'
    ///            println!("{}", elapsed.as_secs());
    ///        }
    ///        Err(e) => {
    ///            // an error occurred!
    ///            println!("Error: {e:?}");
    ///        }
    ///    }
    /// }
    /// ```
    ///
    /// # Platform-specific behavior
    ///
    /// The precision of `SystemTime` can depend on the underlying OS-specific
    /// time format. For example, on Windows the time is represented in 100
    /// nanosecond intervals whereas Linux can represent nanosecond
    /// intervals.
    ///
    /// The following system calls are [currently] being used by `now()` to find
    /// out the current time:
    ///
    /// |  Platform |               System call                                            |
    /// |-----------|----------------------------------------------------------------------|
    /// | SGX       | [`insecure_time` usercall]. More information on [timekeeping in SGX] |
    /// | UNIX      | [clock_gettime (Realtime Clock)]                                     |
    /// | Darwin    | [clock_gettime (Realtime Clock)]                                     |
    /// | VXWorks   | [clock_gettime (Realtime Clock)]                                     |
    /// | SOLID     | `SOLID_RTC_ReadTime`                                                 |
    /// | WASI      | [__wasi_clock_time_get (Realtime Clock)]                             |
    /// | Windows   | [GetSystemTimePreciseAsFileTime] / [GetSystemTimeAsFileTime]         |
    ///
    /// [currently]: crate::io#platform-specific-behavior
    /// [`insecure_time` usercall]: https://edp.fortanix.com/docs/api/fortanix_sgx_abi/struct.Usercalls.html#method.insecure_time
    /// [timekeeping in SGX]: https://edp.fortanix.com/docs/concepts/rust-std/#codestdtimecode
    /// [clock_gettime (Realtime Clock)]: https://linux.die.net/man/3/clock_gettime
    /// [__wasi_clock_time_get (Realtime Clock)]: https://github.com/WebAssembly/WASI/blob/main/legacy/preview1/docs.md#clock_time_get
    /// [GetSystemTimePreciseAsFileTime]: https://docs.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemtimepreciseasfiletime
    /// [GetSystemTimeAsFileTime]: https://docs.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemtimeasfiletime
    ///
    /// **Disclaimer:** These system calls might change over time.
    ///
    /// > Note: mathematical operations like [`add`] may panic if the underlying
    /// > structure cannot represent the new point in time.
    ///
    /// [`add`]: SystemTime::add
    pub struct SystemTime(Time);
    /// An error returned from the `duration_since` and `elapsed` methods on
    /// `SystemTime`, used to learn how far in the opposite direction a system
    /// time lies.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::thread::sleep;
    /// use xrmt_stx::time::{Duration, SystemTime};
    ///
    /// let sys_time = SystemTime::now();
    /// sleep(Duration::from_secs(1));
    /// let new_sys_time = SystemTime::now();
    /// match sys_time.duration_since(new_sys_time) {
    ///     Ok(_) => {}
    ///     Err(e) => println!("SystemTimeError difference: {:?}", e.duration()),
    /// }
    /// ```
    pub struct SystemTimeError(Duration);

    /// An anchor in time which can be used to create new `SystemTime` instances
    /// or learn about where in time a `SystemTime` lies.
    ///
    /// This constant is defined to be "1970-01-01 00:00:00 UTC" on all systems
    /// with respect to the system clock. Using `duration_since` on an
    /// existing [`SystemTime`] instance can tell how far away from this
    /// point in time a measurement lies, and using `UNIX_EPOCH + duration`
    /// can be used to create a [`SystemTime`] instance to represent another
    /// fixed point in time.
    ///
    /// `duration_since(UNIX_EPOCH).unwrap().as_secs()` returns
    /// the number of non-leap seconds since the start of 1970 UTC.
    /// This is a POSIX `time_t` (as a `u64`),
    /// and is the same time representation as used in many Internet protocols.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::time::{SystemTime, UNIX_EPOCH};
    ///
    /// match SystemTime::now().duration_since(UNIX_EPOCH) {
    ///     Ok(n) => println!("1970-01-01 00:00:00 UTC was {} seconds ago!", n.as_secs()),
    ///     Err(_) => panic!("SystemTime before UNIX EPOCH!"),
    /// }
    /// ```
    pub const UNIX_EPOCH: SystemTime = SystemTime::UNIX_EPOCH;

    impl Instant {
        /// Returns an instant corresponding to "now".
        ///
        /// # Examples
        ///
        /// ```
        /// use xrmt_stx::time::Instant;
        ///
        /// let now = Instant::now();
        /// ```
        #[inline]
        pub fn now() -> Instant {
            Instant(Time::now())
        }

        /// Returns the amount of time elapsed since this instant.
        ///
        /// # Panics
        ///
        /// Previous Rust versions panicked when the current time was earlier
        /// than self. Currently this method returns a Duration of zero
        /// in that case. Future versions may reintroduce the panic. See
        /// [Monotonicity].
        ///
        /// [Monotonicity]: Instant#monotonicity
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use xrmt_stx::thread::sleep;
        /// use xrmt_stx::time::{Duration, Instant};
        ///
        /// let instant = Instant::now();
        /// let three_secs = Duration::from_secs(3);
        /// sleep(three_secs);
        /// assert!(instant.elapsed() >= three_secs);
        /// ```
        #[inline]
        pub fn elapsed(&self) -> Duration {
            Time::now().sub(self.0)
        }
        /// Returns the amount of time elapsed from another instant to this one,
        /// or zero duration if that instant is later than this one.
        ///
        /// # Panics
        ///
        /// Previous Rust versions panicked when `earlier` was later than
        /// `self`. Currently this method saturates. Future versions may
        /// reintroduce the panic in some circumstances. See
        /// [Monotonicity].
        ///
        /// [Monotonicity]: Instant#monotonicity
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use xrmt_stx::time::{Duration, Instant};
        /// use xrmt_stx::thread::sleep;
        ///
        /// let now = Instant::now();
        /// sleep(Duration::new(1, 0));
        /// let new_now = Instant::now();
        /// println!("{:?}", new_now.duration_since(now));
        /// println!("{:?}", now.duration_since(new_now)); // 0ns
        /// ```
        #[inline]
        pub fn duration_since(&self, earlier: Instant) -> Duration {
            self.0.subtract(&earlier.0)
        }
        /// Returns `Some(t)` where `t` is the time `self + duration` if `t` can
        /// be represented as `Instant` (which means it's inside the
        /// bounds of the underlying data structure), `None` otherwise.
        #[inline]
        pub fn checked_add(&self, duration: Duration) -> Option<Instant> {
            Some(Instant(self.0.add(duration)))
        }
        /// Returns `Some(t)` where `t` is the time `self - duration` if `t` can
        /// be represented as `Instant` (which means it's inside the
        /// bounds of the underlying data structure), `None` otherwise.
        #[inline]
        pub fn checked_sub(&self, duration: Duration) -> Option<Instant> {
            Some(Instant(self.0.add(duration)))
        }
        /// Returns the amount of time elapsed from another instant to this one,
        /// or zero duration if that instant is later than this one.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use xrmt_stx::time::{Duration, Instant};
        /// use xrmt_stx::thread::sleep;
        ///
        /// let now = Instant::now();
        /// sleep(Duration::new(1, 0));
        /// let new_now = Instant::now();
        /// println!("{:?}", new_now.saturating_duration_since(now));
        /// println!("{:?}", now.saturating_duration_since(new_now)); // 0ns
        /// ```
        #[inline]
        pub fn saturating_duration_since(&self, earlier: Instant) -> Duration {
            self.0.subtract(&earlier.0)
        }
        /// Returns the amount of time elapsed from another instant to this one,
        /// or None if that instant is later than this one.
        ///
        /// Due to [monotonicity bugs], even under correct logical ordering of
        /// the passed `Instant`s, this method can return `None`.
        ///
        /// [monotonicity bugs]: Instant#monotonicity
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use xrmt_stx::time::{Duration, Instant};
        /// use xrmt_stx::thread::sleep;
        ///
        /// let now = Instant::now();
        /// sleep(Duration::new(1, 0));
        /// let new_now = Instant::now();
        /// println!("{:?}", new_now.checked_duration_since(now));
        /// println!("{:?}", now.checked_duration_since(new_now)); // None
        /// ```
        #[inline]
        pub fn checked_duration_since(&self, earlier: Instant) -> Option<Duration> {
            if self.0.is_before(&earlier.0) {
                None
            } else {
                Some(self.0.subtract(&earlier.0))
            }
        }
    }
    impl SystemTime {
        /// An anchor in time which can be used to create new `SystemTime`
        /// instances or learn about where in time a `SystemTime` lies.
        ///
        /// This constant is defined to be "1970-01-01 00:00:00 UTC" on all
        /// systems with respect to the system clock. Using
        /// `duration_since` on an existing `SystemTime` instance can
        /// tell how far away from this point in time a measurement
        /// lies, and using `UNIX_EPOCH + duration` can be
        /// used to create a `SystemTime` instance to represent another fixed
        /// point in time.
        ///
        /// `duration_since(UNIX_EPOCH).unwrap().as_secs()` returns
        /// the number of non-leap seconds since the start of 1970 UTC.
        /// This is a POSIX `time_t` (as a `u64`),
        /// and is the same time representation as used in many Internet
        /// protocols.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use xrmt_stx::time::SystemTime;
        ///
        /// match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        ///     Ok(n) => println!("1970-01-01 00:00:00 UTC was {} seconds ago!", n.as_secs()),
        ///     Err(_) => panic!("SystemTime before UNIX EPOCH!"),
        /// }
        /// ```
        pub const UNIX_EPOCH: SystemTime = SystemTime(Time::ZERO);

        /// Returns the system time corresponding to "now".
        ///
        /// # Examples
        ///
        /// ```
        /// use xrmt_stx::time::SystemTime;
        ///
        /// let sys_time = SystemTime::now();
        /// ```
        #[inline]
        pub fn now() -> SystemTime {
            SystemTime(Time::now())
        }

        /// Returns the difference from this system time to the
        /// current clock time.
        ///
        /// This function may fail as the underlying system clock is susceptible
        /// to drift and updates (e.g., the system clock could go
        /// backwards), so this function might not always succeed. If
        /// successful, <code>[Ok]\([Duration])</code> is returned where
        /// the duration represents the amount of time elapsed from this
        /// time measurement to the current time.
        ///
        /// To measure elapsed time reliably, use [`Instant`] instead.
        ///
        /// Returns an [`Err`] if `self` is later than the current system time,
        /// and the error contains how far from the current system time
        /// `self` is.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use xrmt_stx::thread::sleep;
        /// use xrmt_stx::time::{Duration, SystemTime};
        ///
        /// let sys_time = SystemTime::now();
        /// let one_sec = Duration::from_secs(1);
        /// sleep(one_sec);
        /// assert!(sys_time.elapsed().unwrap() >= one_sec);
        /// ```
        #[inline]
        pub fn elapsed(&self) -> Result<Duration, SystemTimeError> {
            Ok(self.0.sub(Time::now()))
        }
        /// Returns `Some(t)` where `t` is the time `self + duration` if `t` can
        /// be represented as `SystemTime` (which means it's inside the
        /// bounds of the underlying data structure), `None` otherwise.
        #[inline]
        pub fn checked_add(&self, duration: Duration) -> Option<SystemTime> {
            Some(SystemTime(self.0.add(duration)))
        }
        /// Returns `Some(t)` where `t` is the time `self - duration` if `t` can
        /// be represented as `SystemTime` (which means it's inside the
        /// bounds of the underlying data structure), `None` otherwise.
        #[inline]
        pub fn checked_sub(&self, duration: Duration) -> Option<SystemTime> {
            Some(SystemTime(self.0.sub(duration)))
        }
        /// Returns the amount of time elapsed from an earlier point in time.
        ///
        /// This function may fail because measurements taken earlier are not
        /// guaranteed to always be before later measurements (due to anomalies
        /// such as the system clock being adjusted either forwards or
        /// backwards). [`Instant`] can be used to measure elapsed time
        /// without this risk of failure.
        ///
        /// If successful, <code>[Ok]\([Duration])</code> is returned where the
        /// duration represents the amount of time elapsed from the specified
        /// measurement to this one.
        ///
        /// Returns an [`Err`] if `earlier` is later than `self`, and the error
        /// contains how far from `self` the time is.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use xrmt_stx::time::SystemTime;
        ///
        /// let sys_time = SystemTime::now();
        /// let new_sys_time = SystemTime::now();
        /// let difference = new_sys_time.duration_since(sys_time)
        ///     .expect("Clock may have gone backwards");
        /// println!("{difference:?}");
        /// ```
        #[inline]
        pub fn duration_since(&self, earlier: SystemTime) -> Result<Duration, SystemTimeError> {
            if earlier.0.is_before(&self.0) {
                Err(SystemTimeError(earlier.0.sub(self.0)))
            } else {
                Ok(self.0.sub(earlier.0))
            }
        }
    }
    impl SystemTimeError {
        /// Returns the positive duration which represents how far forward the
        /// second system time was from the first.
        ///
        /// A `SystemTimeError` is returned from the
        /// [`SystemTime::duration_since`] and [`SystemTime::elapsed`]
        /// methods whenever the second system time represents a point
        /// later in time than the `self` of the method call.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use xrmt_stx::thread::sleep;
        /// use xrmt_stx::time::{Duration, SystemTime};
        ///
        /// let sys_time = SystemTime::now();
        /// sleep(Duration::from_secs(1));
        /// let new_sys_time = SystemTime::now();
        /// match sys_time.duration_since(new_sys_time) {
        ///     Ok(_) => {}
        ///     Err(e) => println!("SystemTimeError difference: {:?}", e.duration()),
        /// }
        /// ```
        #[inline]
        pub fn duration(&self) -> Duration {
            self.0
        }
    }

    impl Eq for Instant {}
    impl Ord for Instant {
        #[inline]
        fn cmp(&self, other: &Instant) -> Ordering {
            self.0.cmp(&other.0)
        }
    }
    impl Hash for Instant {
        #[inline]
        fn hash<H: Hasher>(&self, h: &mut H) {
            self.0.hash(h);
        }
    }
    impl Copy for Instant {}
    impl Clone for Instant {
        #[inline]
        fn clone(&self) -> Instant {
            Instant(self.0)
        }
    }
    impl PartialEq for Instant {
        #[inline]
        fn eq(&self, other: &Instant) -> bool {
            self.0.eq(&other.0)
        }
    }
    impl PartialOrd for Instant {
        #[inline]
        fn partial_cmp(&self, other: &Instant) -> Option<Ordering> {
            self.0.partial_cmp(&other.0)
        }
    }
    impl Sub<Instant> for Instant {
        type Output = Duration;

        #[inline]
        fn sub(self, rhs: Instant) -> Duration {
            self.0.subtract(&rhs.0)
        }
    }
    impl Add<Duration> for Instant {
        type Output = Instant;

        #[inline]
        fn add(self, rhs: Duration) -> Instant {
            Instant(self.0.add(rhs))
        }
    }
    impl Sub<Duration> for Instant {
        type Output = Instant;

        #[inline]
        fn sub(self, rhs: Duration) -> Instant {
            Instant(self.0.sub(rhs))
        }
    }
    impl AddAssign<Duration> for Instant {
        #[inline]
        fn add_assign(&mut self, rhs: Duration) {
            self.0.add_assign(rhs);
        }
    }
    impl SubAssign<Duration> for Instant {
        #[inline]
        fn sub_assign(&mut self, other: Duration) {
            *self = *self - other;
        }
    }

    impl Clone for SystemTimeError {
        #[inline]
        fn clone(&self) -> SystemTimeError {
            SystemTimeError(self.0)
        }
    }
    impl Debug for SystemTimeError {
        #[cfg(not(feature = "strip"))]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            f.write_str("SystemTimeError(")?;
            Debug::fmt(&self.0, f)?;
            f.write_str(")")
        }
        #[cfg(feature = "strip")]
        #[inline]
        fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
            Ok(())
        }
    }
    impl Error for SystemTimeError {
        #[inline]
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            None
        }
    }
    impl Display for SystemTimeError {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            Debug::fmt(self, f)
        }
    }

    impl Eq for SystemTime {}
    impl Ord for SystemTime {
        #[inline]
        fn cmp(&self, other: &SystemTime) -> Ordering {
            self.0.cmp(&other.0)
        }
    }
    impl Hash for SystemTime {
        #[inline]
        fn hash<H: Hasher>(&self, h: &mut H) {
            self.0.hash(h);
        }
    }
    impl Copy for SystemTime {}
    impl Clone for SystemTime {
        #[inline]
        fn clone(&self) -> SystemTime {
            SystemTime(self.0.clone())
        }
    }
    impl PartialEq for SystemTime {
        #[inline]
        fn eq(&self, other: &Self) -> bool {
            self.0.eq(&other.0)
        }
    }
    impl PartialOrd for SystemTime {
        #[inline]
        fn partial_cmp(&self, other: &SystemTime) -> Option<Ordering> {
            self.0.partial_cmp(&other.0)
        }
    }
    impl From<Time> for SystemTime {
        #[inline]
        fn from(v: Time) -> SystemTime {
            SystemTime(v)
        }
    }
    impl Add<Duration> for SystemTime {
        type Output = SystemTime;

        #[inline]
        fn add(self, rhs: Duration) -> SystemTime {
            SystemTime(self.0.add(rhs))
        }
    }
    impl Sub<Duration> for SystemTime {
        type Output = SystemTime;

        #[inline]
        fn sub(self, rhs: Duration) -> SystemTime {
            SystemTime(self.0.sub(rhs))
        }
    }
    impl AddAssign<Duration> for SystemTime {
        #[inline]
        fn add_assign(&mut self, rhs: Duration) {
            self.0.add_assign(rhs)
        }
    }
    impl SubAssign<Duration> for SystemTime {
        #[inline]
        fn sub_assign(&mut self, rhs: Duration) {
            self.0.sub_assign(rhs)
        }
    }

    impl From<Instant> for Time {
        #[inline]
        fn from(v: Instant) -> Time {
            v.0
        }
    }
    impl From<SystemTime> for Time {
        #[inline]
        fn from(v: SystemTime) -> Time {
            v.0
        }
    }
}
#[cfg(any(not(target_family = "windows"), feature = "std"))]
mod inner {
    extern crate std;

    pub use std::time::*;
}

pub mod extra {
    extern crate core;

    extern crate xrmt_time;

    pub use xrmt_time::*;

    #[cfg(all(target_family = "windows", not(feature = "std")))]
    #[inline]
    pub fn from_instant(v: crate::time::Instant) -> Time {
        core::convert::Into::into(v)
    }
    #[cfg(any(not(target_family = "windows"), feature = "std"))]
    #[inline]
    pub fn from_instant(v: crate::time::Instant) -> Time {
        let (t, i) = (Time::now(), crate::time::Instant::now());
        // Try with Instant(s) in the future.
        if let core::option::Option::Some(d) = v.checked_duration_since(i) {
            return t + d;
        }
        // Try with Instant(s) in the past.
        if let core::option::Option::Some(d) = i.checked_duration_since(v) {
            return t - d;
        }
        // *shrug* Just return now.
        t
    }
}
