#!/usr/bin/env python3
"""A clang wrapper for the Android cross-build.

Problems it solves, all from `cmake-rs`, Rust and the NDK behaving differently on a Windows host:

1. `cmake-rs` adds `--target=<rust-triple>` to `CMAKE_C_FLAGS`. The NDK's CMake toolchain file
   already passes `--target=<triple><api>`, which carries the API level and the sysroot. Clang
   keeps the *last* `--target`, so the bare one wins and the sysroot is lost
   (`crtbegin_dynamic.o: No such file or directory`). This drops the duplicates and keeps the
   NDK's.

2. Rust invokes the linker as `clang.exe <objects> -Wl,...` with no `--target`, so clang picks
   the host target and the linker rejects the Android flags (`lld: error: unknown argument:
   --version-script=...`). This adds `--target=<triple><api>` when none is present.

3. SDL3 has C++ sources, so the link needs the C++ runtime. The NDK's clang ignores
   `-static-libstdc++` ("argument unused during compilation"), so the static libc++ has to be
   named explicitly: `-lc++_static -lc++abi -lunwind`. Without it `librustyzoo.so` keeps
   `__gxx_personality_v0` undefined and Android's `dlopen` fails with
   `cannot locate symbol "__gxx_personality_v0"`. The flags are added only when linking.

Set `RUSTYZOO_REAL_CLANG` to the real compiler, `RUSTYZOO_TARGET` to the triple with API level,
and `RUSTYZOO_SYSROOT_LIB` to the NDK's `sysroot/usr/lib/<triple>` directory. Used as
`CC_<target>`, `CXX_<target>` and the cargo linker.
"""

import os
import subprocess
import sys

REAL = os.environ["RUSTYZOO_REAL_CLANG"]
TARGET = os.environ.get("RUSTYZOO_TARGET", "aarch64-linux-android21")
SYSROOT_LIB = os.environ.get("RUSTYZOO_SYSROOT_LIB")

# The static C++ runtime, in link order. `-lunwind` supplies the unwinder libc++abi needs.
CXX_RUNTIME = ["-lc++_static", "-lc++abi", "-lunwind"]


def main() -> int:
    args = sys.argv[1:]

    targets = [i for i, a in enumerate(args) if a.startswith("--target=")]
    if len(targets) > 1:
        # Keep only the last --target=... (the NDK's, which has the API level and sysroot).
        drop = set(targets[:-1])
        args = [a for i, a in enumerate(args) if i not in drop]
    elif not targets:
        # No target at all (Rust's linker invocation): add it.
        args = [f"--target={TARGET}", *args]

    # A link is a `-o` with an input; a compile is `-c`. Only links need the C++ runtime.
    linking = "-o" in args and "-c" not in args
    if linking and "-c++_static" not in " ".join(args):
        extra = []
        if SYSROOT_LIB:
            extra.append(f"-L{SYSROOT_LIB}")
        extra += CXX_RUNTIME
        args = [*args, *extra]

    return subprocess.call([REAL, *args])


if __name__ == "__main__":
    sys.exit(main())
