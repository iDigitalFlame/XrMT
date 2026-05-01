# XrMT Builtins Crate

The XrMT Builtins Crate is a very low-level crate used for running a `no_std`
crate in a Windows environment. This crate provides functions that are used
instead of compiling against `MSVCRT`.

The `std` feature can be enabled to disable this crate entirely, even if on
Windows. If running this crate on embedded devices *(why?)*, it does nothing.

By default, this crate is enabled (if on Windows).

## Features

- __std__: Disables this crate.

## Default Features

- __None__

## Dependencies

- __None__
