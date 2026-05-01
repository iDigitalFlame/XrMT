#!/usr/bin/bash

TEST_DIR="../builds"

_run=0
_bugs=0
_release=0
_no_copy=0

while getopts "brnR" vv; do
    case $vv in
        "b")
        _bugs=1
        ;;
        "n")
        _no_copy=1
        ;;
        "r")
        _release=1
        ;;
        "R")
        _run=1
        ;;
    esac
    shift
done

_arg=("cargo" "+nightly")

if [ $_run -eq 1 ]; then
    _arg+=("run")
else
    _arg+=("build")
fi
if [ $_bugs -eq 1 ]; then
    _arg+=("--features")
    _arg+=("bugs")
fi
if [ $_release -eq 1 ]; then
    _arg+=("--release")
fi

_os=""

case $1 in
    "bsd")
    _os="x86_64-unknown-freebsd"
    ;;
    "mac")
    _os="x86_64-apple-darwin"
    ;;
    "win")
    _os="x86_64-pc-windows-msvc"
    ;;
    "win32")
    _os="i686-pc-windows-msvc"
    ;;
    "linux")
    _os="x86_64-unknown-linux-gnu"
    ;;
    "win-std")
    _os="x86_64-pc-windows-gnu"
    _arg+=("--features")
    _arg+=("std")
    ;;
    "win-rebuild")
    _os="x86_64-pc-windows-msvc"
    _arg+=("-Zbuild-std")
    _arg+=("-Zbuild-std-features=panic_immediate_abort")
    ;;
    "")
    echo "specify a target"
    exit 1
    ;;
esac

_arg+=("--target")
_arg+=($_os)

echo "Running \"${_arg[@]}\".."

if ! eval "${_arg[@]}"; then
    exit 1
fi

if [ $_no_copy -eq 0 ]; then
    _rd="debug"
    if [ $_release -eq 1 ]; then
        _rd="release"
    fi
    _rn=""
    if [[ $1 == "win"* ]]; then
        _rn=".exe"
    fi
    if [ -n "$2" ]; then
        cp "target/${_os}/${_rd}/xrmt-testing${_rn}" "${TEST_DIR}/${2}${_rn}"
    else
        cp "target/${_os}/${_rd}/xrmt-testing${_rn}" "${TEST_DIR}/"
    fi
fi


# cargo build --features bugs --release --target x86_64-pc-windows-msvc
# cargo build --target x86_64-pc-windows-msvc --features bugs
# cargo +nightly build -Zbuild-std -Zbuild-std-features=panic_immediate_abort --features strip,pie,print --release --target x86_64-pc-windows-msvc
