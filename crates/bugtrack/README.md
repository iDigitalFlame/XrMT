# XrMT Bugtrack Crate

The XrMT Bugtrack Crate is a debugging library that is embedded into other XrMT
crates. If the `bugs` feature is enabled, this crate will create a logging file
in the current user's temp directory and will print messages to it and standard
error.

This crate is platform-independent and does nothing if not enabled.

By default, this crate is disabled.

## Features

- **bugs**: Enables this crate and starts bugtrack logging when running.

## Default Features

- **None**

## Dependencies

- **xrmt-time**
- **xrmt-text** (Windows only)
