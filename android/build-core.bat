@echo off
REM Build the RustyZoo Android library (librustyzoo.so) for aarch64.
REM
REM The Windows + NDK 30 + cmake-rs combination needs four fixes, all handled here:
REM   1. The NDK ships aarch64-linux-android21-clang as a .cmd wrapper, which Rust cannot exec
REM      ("%1 is not a valid Win32 application"). We use clang.exe instead.
REM   2. cmake-rs appends --target=<rust-triple> to CMAKE_C_FLAGS; the NDK toolchain file also
REM      passes --target=<triple><api>. Clang keeps the last one, so the bare one wins and the
REM      sysroot is lost. `clang.cmd` / `clang++.cmd` strip the duplicate. See clang-wrapper.py.
REM   3. sdl3-sys passes -DCMAKE_SYSTEM_NAME=Android, which CMake caches as UNINITIALIZED and
REM      stops the NDK toolchain file from recognizing the build. patch-sdl3-sys.py removes it.
REM   4. CMake 4.5.0-rc1 sets WIN32 even for Android, so SDL compiles its Windows sources.
REM      We use the CMake from Visual Studio (4.3.1) instead.

setlocal

if "%ANDROID_NDK_HOME%"=="" set ANDROID_NDK_HOME=D:\port2andro\android-sdk\ndk\30.0.16248370
if "%ANDROID_HOME%"=="" set ANDROID_HOME=D:\port2andro\android-sdk
if "%ANDROID_SDK_ROOT%"=="" set ANDROID_SDK_ROOT=%ANDROID_HOME%

set TOOLCHAIN=%ANDROID_NDK_HOME%\toolchains\llvm\prebuilt\windows-x86_64
set PATH=%TOOLCHAIN%\bin;%USERPROFILE%\.cargo\bin;%PATH%

set API=21
set HERE=%~dp0
set REPO=%HERE%..
set RUST_TARGET=aarch64-linux-android

REM The Rust linker is the real clang.exe, not a .cmd shim: Rust uses CreateProcess, which
REM cannot run a batch file ("%1 is not a valid Win32 application"). Everything Rust's linker
REM needs is passed as flags instead, through RUSTFLAGS below.
set CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=%TOOLCHAIN%\bin\clang.exe

REM CMake and the cc crate can run a .cmd, so they go through the wrapper, which strips the
REM duplicate --target and adds the static C++ runtime. See clang-wrapper.py.
set CC_aarch64_linux_android=%HERE%clang.cmd
set CXX_aarch64_linux_android=%HERE%clang++.cmd
set AR_aarch64_linux_android=%TOOLCHAIN%\bin\llvm-ar.exe
set RANLIB_aarch64_linux_android=%TOOLCHAIN%\bin\llvm-ranlib.exe

REM What the linker needs lives in rustyzoo-android/.cargo/config.toml: RUSTFLAGS here would
REM *replace* cargo's own flags (breaking `std` resolution) instead of adding to them. That file
REM is regenerated from ANDROID_NDK_HOME below.
set SYSROOT_LIB=%TOOLCHAIN%\sysroot\usr\lib\aarch64-linux-android
python "%HERE%write-cargo-config.py" "%ANDROID_NDK_HOME%" "%API%" || exit /b 1

REM CMake: the NDK toolchain file, Ninja, the API level, and the Visual Studio CMake (see fix 4).
set CMAKE=D:\Visual Studio\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe
set CMAKE_TOOLCHAIN_FILE=%ANDROID_NDK_HOME%\build\cmake\android.toolchain.cmake
set CMAKE_GENERATOR=Ninja
set ANDROID_PLATFORM=android-%API%

echo NDK: %ANDROID_NDK_HOME%
echo API: %API%

REM sdl3-sys needs a small patch; cargo re-extracts the crate if the cache is cleaned.
python "%HERE%patch-sdl3-sys.py" || exit /b 1

cd /d "%HERE%rustyzoo-android"
REM -j 2: SDL3 + RustyZoo in parallel can exhaust this machine's page file
REM ("The paging file is too small for this operation to complete", os error 1455).
cargo build --release --target %RUST_TARGET% -j 2
if errorlevel 1 (
  echo BUILD FAILED
  exit /b 1
)
echo BUILD OK
echo library: %REPO%\target\%RUST_TARGET%\release\librustyzoo.so
