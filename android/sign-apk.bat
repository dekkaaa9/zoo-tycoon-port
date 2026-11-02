@echo off
REM Sign the release APK so it can be sideloaded.
REM
REM `gradlew assembleRelease` produces app-release-unsigned.apk, which Android refuses to
REM install. This aligns it and signs it. Order matters: zipalign first, then apksigner --
REM signing after aligning is required, and aligning after signing breaks the signature.
REM
REM The default keystore is the one the Android tools generate at %USERPROFILE%\.android\
REM debug.keystore (alias androiddebugkey, password android, valid until 2056). That is fine
REM for sideloading from a GitHub release, but it is not an upload key: if it is ever
REM regenerated, the next release will not upgrade over this one in place. For anything beyond
REM that, generate a keystore and pass it in:
REM
REM   keytool -genkeypair -v -keystore zoo.keystore -alias zoo -keyalg RSA -keysize 4096 ^
REM           -validity 10000 -storepass <pass> -keypass <pass> ^
REM           -dname "CN=Zoo Tycoon, O=RustyZoo"
REM
REM   android\sign-apk.bat app\build\outputs\apk\release\app-release-unsigned.apk ^
REM                        ..\..\release\zoo-tycoon-0.1-android.apk ^
REM                        path\to\zoo.keystore zoo <pass>
setlocal
if "%ANDROID_HOME%"=="" set ANDROID_HOME=D:\port2andro\android-sdk
if "%ANDROID_SDK_ROOT%"=="" set ANDROID_SDK_ROOT=%ANDROID_HOME%
set HERE=%~dp0

set IN=%~1
set OUT=%~2
set KS=%~3
if "%KS%"=="" set KS=%USERPROFILE%\.android\debug.keystore
set ALIAS=%~4
if "%ALIAS%"=="" set ALIAS=androiddebugkey
set PASS=%~5
if "%PASS%"=="" set PASS=android

if "%IN%"=="" (
  echo usage: sign-apk.bat ^<unsigned.apk^> ^<signed.apk^> [keystore] [alias] [password]
  exit /b 1
)
if "%OUT%"=="" (
  echo usage: sign-apk.bat ^<unsigned.apk^> ^<signed.apk^> [keystore] [alias] [password]
  exit /b 1
)

REM apksigner and zipalign ship together; take whichever build-tools is newest.
set BT=
for /f "delims=" %%d in ('dir /b /ad /o-n "%ANDROID_HOME%\build-tools"') do (
  if not defined BT set BT=%ANDROID_HOME%\build-tools\%%d
)
if not exist "%BT%\apksigner.bat" (
  echo no apksigner under %ANDROID_HOME%\build-tools
  exit /b 1
)

set TMP=%TEMP%\rustyzoo-align.apk
if exist "%TMP%" del "%TMP%"

echo === align ===
"%BT%\zipalign.exe" -p -f 4 "%IN%" "%TMP%"
if errorlevel 1 exit /b 1

echo === sign ===
call "%BT%\apksigner.bat" sign --ks "%KS%" --ks-pass pass:%PASS% ^
    --ks-key-alias %ALIAS% --key-pass pass:%PASS% --out "%OUT%" "%TMP%"
if errorlevel 1 exit /b 1

echo === verify ===
"%BT%\apksigner.bat" verify --print-certs "%OUT%"
if errorlevel 1 exit /b 1

del "%TMP%"
echo signed: %OUT%
