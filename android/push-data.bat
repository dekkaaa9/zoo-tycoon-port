@echo off
REM Push the game data and install the app on a connected device.
REM
REM Usage:
REM   push-data.bat "D:\Zoo Tycoon"          push the game data, then install the APK
REM   push-data.bat "D:\Zoo Tycoon" --no-apk push only
REM
REM The data goes to the app's external files dir, which is where the native shell reads it
REM (`/sdcard/Android/data/org.rustyzoo/files/Zoo Tycoon`).

setlocal

set ADB=C:\adb\adb.exe
if not exist "%ADB%" set ADB=adb

set SRC=%~1
if "%SRC%"=="" (
  echo usage: push-data.bat "D:\path\to\Zoo Tycoon" [--no-apk]
  exit /b 1
)

set DEST=/sdcard/Android/data/org.rustyzoo/files

echo === install APK ===
if not "%2"=="--no-apk" (
  if exist "%~dp0app\build\outputs\apk\debug\app-debug.apk" (
    "%ADB%" install -r "%~dp0app\build\outputs\apk\debug\app-debug.apk"
  ) else (
    echo no APK yet: run build-apk.bat first
  )
)

echo === push game data (this takes a while) ===
"%ADB%" shell mkdir -p "%DEST%/Zoo Tycoon"
"%ADB%" push "%SRC%"/. "%DEST%/Zoo Tycoon/"

echo done. Launch RustyZoo on the device.
