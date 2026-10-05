#!/bin/sh
set -e

if [ "$1" != "configure" ]; then
    exit 0
fi

if command -v modprobe >/dev/null 2>&1; then
    modprobe uinput 2>/dev/null || true
fi

if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules 2>/dev/null || true
    udevadm trigger --subsystem-match=misc --sysname-match=uinput 2>/dev/null || true
    udevadm trigger --subsystem-match=input 2>/dev/null || true
fi

exit 0
