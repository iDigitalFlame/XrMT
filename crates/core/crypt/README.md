# XrMT Crypt Crate

The XrMT Crypt Crate is a special crate that can be used to obfuscate strings in
the resulting binary. The fully use this crate, an additional compiling tool must
be used.

This crate provides a macro that takes string argument. This string will be the
obfuscated value, replaced by the macro.

If this crate is disabled, the macro just returns the passed string without any
modifications.

By default, this crate is disabled.

## Features

- **crypt**: Enables this crate.

## Default Features

- **None**

## Dependencies

- **None**
