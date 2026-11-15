#!/usr/bin/env bash
# Linker shim for the Windows cross-build.
#
# Ubuntu 20.04 ships mingw-w64 7.0, whose libkernel32.a exports
# WaitOnAddress/WakeByAddress* from kernel32.dll. Real Windows only has those
# in api-ms-win-core-synch-l1-2-0.dll, so an executable linked that way dies at
# startup with "entry point not found". Pulling the symbols from
# libsynchronization.a first fixes it.
#
# Newer mingw-w64 releases don't need the workaround, and the library layout
# differs between distributions, so fall back to a plain link when the extra
# library is absent.
set -euo pipefail

SYNC_LIB="/usr/x86_64-w64-mingw32/lib/libsynchronization.a"
ALT_LIB="$(x86_64-w64-mingw32-gcc -print-file-name=libsynchronization.a 2>/dev/null || true)"

if [ ! -f "$SYNC_LIB" ] && [ -n "$ALT_LIB" ] && [ -f "$ALT_LIB" ]; then
  SYNC_LIB="$ALT_LIB"
fi

if [ -f "$SYNC_LIB" ]; then
  exec x86_64-w64-mingw32-gcc \
    -Wl,-u,WaitOnAddress -Wl,-u,WakeByAddressAll -Wl,-u,WakeByAddressSingle \
    "$SYNC_LIB" "$@"
fi

exec x86_64-w64-mingw32-gcc "$@"
