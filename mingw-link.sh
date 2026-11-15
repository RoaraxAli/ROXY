#!/usr/bin/env bash
# Ubuntu 20.04's mingw-w64 7.0 exports WaitOnAddress/WakeByAddress* from
# libkernel32.a, but on real Windows they live in api-ms-win-core-synch-l1-2-0.dll.
# Pull them from libsynchronization.a before kernel32 is scanned.
exec x86_64-w64-mingw32-gcc \
  -Wl,-u,WaitOnAddress -Wl,-u,WakeByAddressAll -Wl,-u,WakeByAddressSingle \
  /usr/x86_64-w64-mingw32/lib/libsynchronization.a "$@"
