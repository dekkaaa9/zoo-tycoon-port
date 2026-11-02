#!/usr/bin/env python3
"""Patch `sdl3-sys` in the cargo registry for the Android cross-build.

`build-common.rs` defines `CMAKE_SYSTEM_NAME=Android` and `CMAKE_SYSTEM_PROCESSOR` with
`-D`. CMake caches a `-D` value as `UNINITIALIZED`, and the NDK's toolchain file then does
not recognize the build as Android: `WINDOWS` stays true from the host, so SDL compiles
`src/process/windows/SDL_windowsprocess.c` and fails on `#include <windows.h>`.

The fix is to stop passing those two variables: the NDK toolchain file sets both correctly.
This script is idempotent and is run by `build-core.bat` before every build, because cargo
re-extracts the crate (undoing the patch) whenever the registry cache is cleaned.

Usage: python patch-sdl3-sys.py
"""

import glob
import os
import sys

OLD = """                let (abi, processor) = match target_arch.as_str() {
                    "aarch64" => ("arm64-v8a", "arm64"),
                    "arm" => ("armeabi-v7a", "arm"),
                    "x86_64" => ("x86_64", "x86_64"),
                    "x86" => ("x86", "x86"),
                    other => (other, other),
                };
                build_config.define("ANDROID_ABI", abi);
                build_config.define("CMAKE_SYSTEM_NAME", "Android");
                build_config.define("CMAKE_SYSTEM_PROCESSOR", processor);"""

NEW = """                let abi = match target_arch.as_str() {
                    "aarch64" => "arm64-v8a",
                    "arm" => "armeabi-v7a",
                    "x86_64" => "x86_64",
                    "x86" => "x86",
                    other => other,
                };
                build_config.define("ANDROID_ABI", abi);
                // CMAKE_SYSTEM_NAME and CMAKE_SYSTEM_PROCESSOR are left to the NDK's
                // toolchain file: a -D value is cached UNINITIALIZED and stops the
                // toolchain file from recognizing the Android build.""" 

MARK = "RustyZoo: CMAKE_SYSTEM_NAME and CMAKE_SYSTEM_PROCESSOR are NOT defined here."

def main() -> int:
    cargo_home = os.environ.get("CARGO_HOME", os.path.expanduser("~/.cargo"))
    pattern = os.path.join(
        cargo_home, "registry", "src", "*", "sdl3-sys-*", "build-common.rs"
    )
    files = glob.glob(pattern)
    if not files:
        print("patch-sdl3-sys: no sdl3-sys build-common.rs found (nothing to do)")
        return 0
    for path in files:
        with open(path, encoding="utf-8") as f:
            text = f.read()
        if MARK in text:
            print(f"patch-sdl3-sys: already patched: {path}")
            continue
        if OLD not in text:
            print(f"patch-sdl3-sys: WARNING: expected code not found in {path}")
            continue
        with open(path, "w", encoding="utf-8", newline="") as f:
            f.write(text.replace(OLD, NEW))
        print(f"patch-sdl3-sys: patched: {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
