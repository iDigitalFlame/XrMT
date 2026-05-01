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

extern crate alloc as alloc_crate;
extern crate core;

#[cfg(feature = "compat")]
pub use core::{
    any,
    arch,
    array,
    ascii,
    assert,
    assert_eq,
    assert_matches,
    assert_ne,
    assert_unsafe_precondition,
    cell,
    cfg,
    cfg_select,
    char,
    clone,
    cmp,
    column,
    compile_error,
    concat,
    concat_bytes,
    const_format_args,
    convert,
    debug_assert,
    debug_assert_eq,
    debug_assert_ne,
    default,
    env,
    error,
    f128,
    f16,
    f32,
    f64,
    file,
    format_args,
    format_args_nl,
    future,
    hash,
    hint,
    include,
    include_bytes,
    include_str,
    intrinsics,
    iter,
    line,
    log_syntax,
    marker,
    matches,
    mem,
    module_path,
    num,
    ops,
    option,
    option_env,
    panic,
    pat,
    pattern_type,
    pin,
    primitive,
    ptr,
    random,
    range,
    result,
    simd,
    stringify,
    todo,
    trace_macros,
    ub_checks,
    unimplemented,
    unreachable,
    write,
    writeln,
};
#[cfg(feature = "compat")]
#[cfg_attr(rustfmt, rustfmt_skip)]
pub use alloc_crate::{
    alloc,
    borrow,
    boxed,
    bstr,
    collections,
    fmt,
    format,
    rc,
    slice,
    str,
    string,
    vec
};

#[cfg(feature = "compat")]
pub mod task {
    //! Types and Traits for working with asynchronous tasks.

    extern crate alloc;
    extern crate core;

    pub use alloc::task::*;
    pub use core::task::*;
}

// Allow for printing without needing the "core" crate to be added
// when no prelude is present.
#[cfg(all(
    target_family = "windows",
    not(feature = "std"),
    not(feature = "compat"),
    any(not(feature = "strip"), feature = "print")
))]
pub use core::{write, writeln};

// Creates a warning if not-matching, since macros are already in scope?
#[cfg(all(not(feature = "strip"), any(not(target_family = "windows"), feature = "std")))]
pub use self::printing::*;

#[cfg(all(feature = "strip", not(feature = "print")))]
mod printing {
    extern crate alloc;

    /// Prints to the standard output.
    ///
    /// Equivalent to the [`println!`] macro except that a newline is not
    /// printed at the end of the message.
    ///
    /// Note that stdout is frequently line-buffered by default so it may be
    /// necessary to use [`io::stdout().flush()`][flush] to ensure the output is
    /// emitted immediately.
    ///
    /// The `print!` macro will lock the standard output on each call. If you
    /// call `print!` within a hot loop, this behavior may be the bottleneck
    /// of the loop. To avoid this, lock stdout with
    /// [`io::stdout().lock()`][lock]: ```
    /// use xrmt_stx::io::{stdout, Write};
    ///
    /// let mut lock = stdout().lock();
    /// write!(lock, "hello world").unwrap();
    /// ```
    /// 
    /// Use `print!` only for the primary output of your program. Use
    /// [`eprint!`] instead to print error and progress messages.
    ///
    /// See [the formatting documentation in `xrmt_stx::fmt`](../std/fmt/index.html)
    /// for details of the macro argument syntax.
    ///
    /// [flush]: crate::io::Write::flush
    /// [`println!`]: crate::println
    /// [`eprint!`]: crate::eprint
    /// [lock]: crate::io::Stdout
    ///
    /// # Panics
    ///
    /// Panics if writing to `io::stdout()` fails.
    ///
    /// Writing to non-blocking stdout can cause an error, which will lead
    /// this macro to panic.
    ///
    /// # Examples
    /// ```
    /// use xrmt_stx::io::{self, Write};
    ///
    /// print!("this ");
    /// print!("will ");
    /// print!("be ");
    /// print!("on ");
    /// print!("the ");
    /// print!("same ");
    /// print!("line ");
    ///
    /// io::stdout().flush().unwrap();
    ///
    /// print!("this string has a newline, why not choose println! instead?\n");
    ///
    /// io::stdout().flush().unwrap();
    /// ```
    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => {{}};
    }
    /// Prints to the standard error.
    ///
    /// Equivalent to the [`print!`] macro, except that output goes to
    /// [`io::stderr`] instead of [`io::stdout`]. See [`print!`] for
    /// example usage.
    ///
    /// Use `eprint!` only for error and progress messages. Use `print!`
    /// instead for the primary output of your program.
    ///
    /// [`io::stderr`]: crate::io::stderr
    /// [`io::stdout`]: crate::io::stdout
    ///
    /// See [the formatting documentation in
    /// `xrmt_stx::fmt`](../std/fmt/index.html) for details of the macro
    /// argument syntax.
    ///
    /// # Panics
    ///
    /// Panics if writing to `io::stderr` fails.
    ///
    /// Writing to non-blocking stderr can cause an error, which will lead
    /// this macro to panic.
    ///
    /// # Examples
    ///
    /// ```
    /// eprint!("Error: Could not complete task");
    /// ```
    #[macro_export]
    macro_rules! eprint {
        ($($arg:tt)*) => {{}};
    }
    /// Prints to the standard output, with a newline.
    ///
    /// On all platforms, the newline is the LINE FEED character (`\n`/`U+000A`)
    /// alone (no additional CARRIAGE RETURN (`\r`/`U+000D`)).
    ///
    /// This macro uses the same syntax as [`format!`], but writes to the
    /// standard output instead. See [`fmt`] for more information.
    ///
    /// The `println!` macro will lock the standard output on each call. If you
    /// call `println!` within a hot loop, this behavior may be the
    /// bottleneck of the loop. To avoid this, lock stdout with
    /// [`io::stdout().lock()`][lock]: ```
    /// use xrmt_stx::io::{stdout, Write};
    ///
    /// let mut lock = stdout().lock();
    /// writeln!(lock, "hello world").unwrap();
    /// ```
    /// 
    /// Use `println!` only for the primary output of your program. Use
    /// [`eprintln!`] instead to print error and progress messages.
    ///
    /// See [the formatting documentation in `xrmt_stx::fmt`](../std/fmt/index.html)
    /// for details of the macro argument syntax.
    ///
    /// [`fmt`]: alloc::fmt
    ///
    /// [`eprintln!`]: crate::eprintln
    /// [lock]: crate::io::Stdout
    ///
    /// # Panics
    ///
    /// Panics if writing to [`io::stdout`] fails.
    ///
    /// Writing to non-blocking stdout can cause an error, which will lead
    /// this macro to panic.
    ///
    /// [`format!`]: alloc::format!
    /// [`io::stdout`]: crate::io::stdout
    ///
    /// # Examples
    /// ```
    /// println!(); // prints just a newline
    /// println!("hello there!");
    /// println!("format {} arguments", "some");
    /// let local_variable = "some";
    /// println!("format {local_variable} arguments");
    /// ```
    #[macro_export]
    macro_rules! println {
        () => {};
        ($($arg:tt)*) => {{}};
    }
    /// Prints to the standard error, with a newline.
    ///
    /// Equivalent to the [`println!`] macro, except that output goes to
    /// [`io::stderr`] instead of [`io::stdout`]. See [`println!`] for
    /// example usage.
    ///
    /// Use `eprintln!` only for error and progress messages. Use `println!`
    /// instead for the primary output of your program.
    ///
    /// See [the formatting documentation in
    /// `xrmt_stx::fmt`](../std/fmt/index.html) for details of the macro
    /// argument syntax.
    ///
    /// [`io::stderr`]: crate::io::stderr
    /// [`io::stdout`]: crate::io::stdout
    /// [`println!`]: crate::println
    ///
    /// # Panics
    ///
    /// Panics if writing to `io::stderr` fails.
    ///
    /// Writing to non-blocking stderr can cause an error, which will lead
    /// this macro to panic.
    ///
    /// # Examples
    ///
    /// ```
    /// eprintln!("Error: Could not complete task");
    /// ```
    #[macro_export]
    macro_rules! eprintln {
        () => {};
        ($($arg:tt)*) => {{}};
    }
}
#[cfg(all(
    target_family = "windows",
    not(feature = "std"),
    any(not(feature = "strip"), feature = "print")
))]
mod printing {
    extern crate alloc;

    /// Prints to the standard output.
    ///
    /// Equivalent to the [`println!`] macro except that a newline is not
    /// printed at the end of the message.
    ///
    /// Note that stdout is frequently line-buffered by default so it may be
    /// necessary to use [`io::stdout().flush()`][flush] to ensure the output is
    /// emitted immediately.
    ///
    /// The `print!` macro will lock the standard output on each call. If you
    /// call `print!` within a hot loop, this behavior may be the bottleneck
    /// of the loop. To avoid this, lock stdout with
    /// [`io::stdout().lock()`][lock]:
    /// ```
    /// use xrmt_stx::io::{stdout, Write};
    ///
    /// let mut lock = stdout().lock();
    /// write!(lock, "hello world").unwrap();
    /// ```
    ///
    /// Use `print!` only for the primary output of your program. Use
    /// [`eprint!`] instead to print error and progress messages.
    ///
    /// See [the formatting documentation in
    /// `xrmt_stx::fmt`](../std/fmt/index.html) for details of the macro
    /// argument syntax.
    ///
    /// [flush]: crate::io::Write::flush
    /// [`println!`]: crate::println
    /// [`eprint!`]: crate::eprint
    /// [lock]: crate::io::Stdout
    ///
    /// # Panics
    ///
    /// Panics if writing to `io::stdout()` fails.
    ///
    /// Writing to non-blocking stdout can cause an error, which will lead
    /// this macro to panic.
    ///
    /// # Examples
    /// ```
    /// use xrmt_stx::io::{self, Write};
    ///
    /// print!("this ");
    /// print!("will ");
    /// print!("be ");
    /// print!("on ");
    /// print!("the ");
    /// print!("same ");
    /// print!("line ");
    ///
    /// io::stdout().flush().unwrap();
    ///
    /// print!("this string has a newline, why not choose println! instead?\n");
    ///
    /// io::stdout().flush().unwrap();
    /// ```
    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => {{
            let _ = $crate::write!($crate::io::extra::RawStdout::get(), $($arg)*);
        }};
    }
    /// Prints to the standard error.
    ///
    /// Equivalent to the [`print!`] macro, except that output goes to
    /// [`io::stderr`] instead of [`io::stdout`]. See [`print!`] for
    /// example usage.
    ///
    /// Use `eprint!` only for error and progress messages. Use `print!`
    /// instead for the primary output of your program.
    ///
    /// [`io::stderr`]: crate::io::stderr
    /// [`io::stdout`]: crate::io::stdout
    ///
    /// See [the formatting documentation in
    /// `xrmt_stx::fmt`](../std/fmt/index.html) for details of the macro
    /// argument syntax.
    ///
    /// # Panics
    ///
    /// Panics if writing to `io::stderr` fails.
    ///
    /// Writing to non-blocking stderr can cause an error, which will lead
    /// this macro to panic.
    ///
    /// # Examples
    ///
    /// ```
    /// eprint!("Error: Could not complete task");
    /// ```
    #[macro_export]
    macro_rules! eprint {
        ($($arg:tt)*) => {{
            let _ = $crate::write!($crate::io::extra::RawStderr::get(), $($arg)*);
        }};
    }
    /// Prints to the standard output, with a newline.
    ///
    /// On all platforms, the newline is the LINE FEED character (`\n`/`U+000A`)
    /// alone (no additional CARRIAGE RETURN (`\r`/`U+000D`)).
    ///
    /// This macro uses the same syntax as [`format!`], but writes to the
    /// standard output instead. See [`fmt`] for more information.
    ///
    /// The `println!` macro will lock the standard output on each call. If you
    /// call `println!` within a hot loop, this behavior may be the
    /// bottleneck of the loop. To avoid this, lock stdout with
    /// [`io::stdout().lock()`][lock]:
    /// ```
    /// use xrmt_stx::io::{stdout, Write};
    ///
    /// let mut lock = stdout().lock();
    /// writeln!(lock, "hello world").unwrap();
    /// ```
    ///
    /// Use `println!` only for the primary output of your program. Use
    /// [`eprintln!`] instead to print error and progress messages.
    ///
    /// See [the formatting documentation in `fmt`](../core/fmt/index.html)
    /// for details of the macro argument syntax.
    ///
    /// [`fmt`]: alloc::fmt
    /// [`eprintln!`]: crate::eprintln
    /// [lock]: crate::io::Stdout
    ///
    /// # Panics
    ///
    /// Panics if writing to [`io::stdout`] fails.
    ///
    /// Writing to non-blocking stdout can cause an error, which will lead
    /// this macro to panic.
    ///
    /// [`format!`]: alloc::format!
    /// [`io::stdout`]: crate::io::stdout
    ///
    /// # Examples
    /// ```
    /// println!(); // prints just a newline
    /// println!("hello there!");
    /// println!("format {} arguments", "some");
    /// let local_variable = "some";
    /// println!("format {local_variable} arguments");
    /// ```
    #[macro_export]
    macro_rules! println {
        () => {
            $crate::print!("\n")
        };
        ($($arg:tt)*) => {{
            let _ = $crate::writeln!($crate::io::extra::RawStdout::get(), $($arg)*);
        }};
    }
    /// Prints to the standard error, with a newline.
    ///
    /// Equivalent to the [`println!`] macro, except that output goes to
    /// [`io::stderr`] instead of [`io::stdout`]. See [`println!`] for
    /// example usage.
    ///
    /// Use `eprintln!` only for error and progress messages. Use `println!`
    /// instead for the primary output of your program.
    ///
    /// See [the formatting documentation in
    /// `xrmt_stx::fmt`](../std/fmt/index.html) for details of the macro
    /// argument syntax.
    ///
    /// [`io::stderr`]: crate::io::stderr
    /// [`io::stdout`]: crate::io::stdout
    /// [`println!`]: crate::println
    ///
    /// # Panics
    ///
    /// Panics if writing to `io::stderr` fails.
    ///
    /// Writing to non-blocking stderr can cause an error, which will lead
    /// this macro to panic.
    ///
    /// # Examples
    ///
    /// ```
    /// eprintln!("Error: Could not complete task");
    /// ```
    #[macro_export]
    macro_rules! eprintln {
        () => {
            $crate::eprint!("\n")
        };
        ($($arg:tt)*) => {{
            let _ = $crate::writeln!($crate::io::extra::RawStderr::get(), $($arg)*);
        }};
    }
}
#[cfg(all(
    any(not(target_family = "windows"), feature = "std"),
    any(not(feature = "strip"), feature = "print")
))]
mod printing {
    extern crate std;
    pub use std::{eprint, eprintln, print, println};
}

/// Prints and returns the value of a given expression for quick and dirty
/// debugging.
///
/// An example:
///
/// ```rust
/// let a = 2;
/// let b = dbg!(a * 2) + 1;
/// //      ^-- prints: [src/main.rs:2:9] a * 2 = 4
/// assert_eq!(b, 5);
/// ```
///
/// The macro works by using the `Debug` implementation of the type of
/// the given expression to print the value to [stderr] along with the
/// source location of the macro invocation as well as the source code
/// of the expression.
///
/// Invoking the macro on an expression moves and takes ownership of it
/// before returning the evaluated expression unchanged. If the type
/// of the expression does not implement `Copy` and you don't want
/// to give up ownership, you can instead borrow with `dbg!(&expr)`
/// for some expression `expr`.
///
/// The `dbg!` macro works exactly the same in release builds.
/// This is useful when debugging issues that only occur in release
/// builds or when debugging in release mode is significantly faster.
///
/// Note that the macro is intended as a debugging tool and therefore you
/// should avoid having uses of it in version control for long periods
/// (other than in tests and similar).
/// Debug output from production code is better done with other facilities
/// such as the [`debug!`] macro from the [`log`] crate.
///
/// # Stability
///
/// The exact output printed by this macro should not be relied upon
/// and is subject to future changes.
///
/// # Panics
///
/// Panics if writing to `io::stderr` fails.
///
/// # Further examples
///
/// With a method call:
///
/// ```rust
/// fn foo(n: usize) {
///     if let Some(_) = dbg!(n.checked_sub(4)) {
///         // ...
///     }
/// }
///
/// foo(3)
/// ```
///
/// This prints to [stderr]:
///
/// ```text,ignore
/// [src/main.rs:2:22] n.checked_sub(4) = None
/// ```
///
/// Naive factorial implementation:
///
/// ```rust
/// fn factorial(n: u32) -> u32 {
///     if dbg!(n <= 1) {
///         dbg!(1)
///     } else {
///         dbg!(n * factorial(n - 1))
///     }
/// }
///
/// dbg!(factorial(4));
/// ```
///
/// This prints to [stderr]:
///
/// ```text,ignore
/// [src/main.rs:2:8] n <= 1 = false
/// [src/main.rs:2:8] n <= 1 = false
/// [src/main.rs:2:8] n <= 1 = false
/// [src/main.rs:2:8] n <= 1 = true
/// [src/main.rs:3:9] 1 = 1
/// [src/main.rs:7:9] n * factorial(n - 1) = 2
/// [src/main.rs:7:9] n * factorial(n - 1) = 6
/// [src/main.rs:7:9] n * factorial(n - 1) = 24
/// [src/main.rs:9:1] factorial(4) = 24
/// ```
///
/// The `dbg!(..)` macro moves the input:
///
/// ```compile_fail
/// /// A wrapper around `usize` which importantly is not Copyable.
/// #[derive(Debug)]
/// struct NoCopy(usize);
///
/// let a = NoCopy(42);
/// let _ = dbg!(a); // <-- `a` is moved here.
/// let _ = dbg!(a); // <-- `a` is moved again; error!
/// ```
///
/// You can also use `dbg!()` without a value to just print the
/// file and line whenever it's reached.
///
/// Finally, if you want to `dbg!(..)` multiple values, it will treat them as
/// a tuple (and return it, too):
///
/// ```
/// assert_eq!(dbg!(1usize, 2u32), (1, 2));
/// ```
///
/// However, a single argument with a trailing comma will still not be treated
/// as a tuple, following the convention of ignoring trailing commas in macro
/// invocations. You can use a 1-tuple directly if you need one:
///
/// ```
/// assert_eq!(1, dbg!(1u32,)); // trailing comma ignored
/// assert_eq!((1,), dbg!((1u32,))); // 1-tuple
/// ```
///
/// [stderr]: https://en.wikipedia.org/wiki/Standard_streams#Standard_error_(stderr)
/// [`debug!`]: https://docs.rs/log/*/log/macro.debug.html
/// [`log`]: https://crates.io/crates/log
#[cfg(not(feature = "strip"))]
#[macro_export]
macro_rules! dbg {
    () => {
        $crate::eprintln!("[{}:{}:{}]", core::file!(), core::line!(), core::column!())
    };
    ($val:expr $(,)?) => {
        match $val {
            v => {
                $crate::eprintln!("[{}:{}:{}] {} = {:#?}",
                    core::file!(),
                    core::line!(),
                    core::column!(),
                    core::stringify!($v),
                    &&v as &dyn core::fmt::Debug,
                );
                v
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg!($val)),+,)
    };
}
