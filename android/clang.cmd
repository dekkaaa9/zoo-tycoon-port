@echo off
REM clang shim for the Android cross-build. See clang-wrapper.py.
set RUSTYZOO_REAL_CLANG=D:\port2andro\android-sdk\ndk\30.0.16248370\toolchains\llvm\prebuilt\windows-x86_64\bin\clang.exe
set RUSTYZOO_TARGET=aarch64-linux-android21
set RUSTYZOO_SYSROOT_LIB=D:\port2andro\android-sdk\ndk\30.0.16248370\toolchains\llvm\prebuilt\windows-x86_64\sysroot\usr\lib\aarch64-linux-android
python "%~dp0clang-wrapper.py" %*
