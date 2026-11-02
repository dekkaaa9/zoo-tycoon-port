@echo off
REM Build the RustyZoo Android app: the Rust library, then the APK.
REM
REM Steps:
REM   1. cargo build --target aarch64-linux-android  -> librustyzoo.so
REM   2. copy it into app/src/main/jniLibs/arm64-v8a/
REM   3. gradlew assembleDebug                       -> app/build/outputs/apk/debug/app-debug.apk
REM
REM The game data is NOT bundled (it is not ours to ship). Push it with push-data.bat.

setlocal

set ANDROID_HOME=D:\port2andro\android-sdk
set ANDROID_SDK_ROOT=%ANDROID_HOME%
set ANDROID_NDK_HOME=%ANDROID_HOME%\ndk\30.0.16248370
set JAVA_HOME=C:\Program Files\Eclipse Adoptium\jdk-21.0.12.101-hotspot

set HERE=%~dp0
set ABI=arm64-v8a
set RUST_TARGET=aarch64-linux-android

echo === 1/3 Rust library ===
call "%HERE%build-core.bat"
if errorlevel 1 exit /b 1

echo === 2/3 copy librustyzoo.so ===
REM The Android crate is its own package, so it has its own target directory.
set SO=%HERE%rustyzoo-android\target\%RUST_TARGET%\release\librustyzoo.so
if not exist "%SO%" (
  echo could not find %SO%
  exit /b 1
)
set DEST=%HERE%app\src\main\jniLibs\%ABI%
if not exist "%DEST%" mkdir "%DEST%"
copy /y "%SO%" "%DEST%\librustyzoo.so" >nul
if errorlevel 1 exit /b 1
echo copied to %DEST%\librustyzoo.so

echo === 3/3 APK ===
cd /d "%HERE%"
call gradlew.bat assembleDebug
if errorlevel 1 exit /b 1

echo.
echo APK: %HERE%app\build\outputs\apk\debug\app-debug.apk
