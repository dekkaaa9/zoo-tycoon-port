# RustyZoo on Android

This directory holds the Android port of RustyZoo, shipped under the name **Zoo Tycoon**: a
Gradle app that packages a Rust library which runs the same core as the desktop build. The
core (`src/lib.rs`) has no platform code, so the port is a new shell —
`android/rustyzoo-android/src/lib.rs` — in place of `src/window/`, plus the Java activity SDL
needs.

## What works, and what does not

| Part | State |
| --- | --- |
| Core library cross-compiles for `aarch64-linux-android` | ✅ |
| SDL3 built from source for Android | ✅ (static, inside `librustyzoo.so`) |
| Gradle app, manifest, activity | ✅ |
| APK build + data push scripts | ✅ |
| Runs on MuMu Player (900×1600, landscape) | ✅ verified |
| Start-up picture and progress bar while the data loads | ✅ |
| Main menu, scenario and freeform lists, credits | ✅ |
| Map, interface, catalogue, simulation | ✅ |
| Tap → click, drag → pan (1:1 with the finger) | ✅ |
| Zoom, rotate and pause buttons of the left bar | ✅ verified on device |
| Construction, adopt, objects and hire panels | ✅ |
| Pinch → zoom, two-finger turn → rotate | not implemented |
| Keyboard | not mapped (no `Key` commands from the shell) |

The desktop build is unaffected: everything Android lives under `android/` and is built with a
separate target directory.

## Prerequisites

- **Android SDK** with `platform-tools`, `platforms;android-35`, `build-tools`, and **NDK 30**
  (`ndk;30.0.16248370`). The scripts assume `D:\port2andro\android-sdk`.
- **JDK 21** for Gradle. The scripts assume Temurin at
  `C:\Program Files\Eclipse Adoptium\jdk-21.0.12.101-hotspot`.
- **Rust** with the Android targets installed *for the pinned toolchain* (the repo pins 1.99.0):

  ```
  rustup target add --toolchain 1.99.0-x86_64-pc-windows-msvc aarch64-linux-android
  ```

  Installing the target for the default toolchain is not enough: `rust-toolchain.toml` selects
  the pinned one, and it has its own target list.

- **Python** on `PATH` (the four helper scripts are Python), with Pillow and numpy for
  `make-launcher-icons.py`.
- **CMake 4.3.1 or older** (the one that ships with Visual Studio), see fix 4 below.

## Building

```
android\build-apk.bat
```

That runs `build-core.bat` (Cargo), copies `librustyzoo.so` into
`app/src/main/jniLibs/arm64-v8a/`, and runs `gradlew assembleDebug`. The APK lands in
`android/app/build/outputs/apk/debug/`.

Build with `-j 2`. The machine this was developed on has 13.75 GB of RAM: building SDL3 and
RustyZoo in parallel at a higher job count exhausts the page file (`os error 1455`). Under memory
pressure rustc dies with `STATUS_STACK_BUFFER_OVERRUN` or `rustc-LLVM ERROR: out of memory` — stop
MuMu Player (it holds ~2 GB) and rebuild, or drop to `-j 1`.

## Shipping a release

`assembleDebug` produces a debuggable APK, which is fine for the device but not for handing out.
For a release:

```
cd android
gradlew.bat assembleRelease
sign-apk.bat app\build\outputs\apk\release\app-release-unsigned.apk ..\..\release\zoo-tycoon-0.1-android.apk
```

`gradlew assembleRelease` needs the same environment as `build-apk.bat` (`ANDROID_HOME`,
`JAVA_HOME`), and the `.so` already sitting in `jniLibs` from the last debug build — it does not
re-run Cargo. The unsigned APK it emits is 3.8 MB against the debug build's 6.9 MB.

`sign-apk.bat` aligns and then signs, in that order: `zipalign` must come first because signing
after aligning is required, and aligning after signing invalidates the signature. Its default
keystore is the one the Android tools generate at `%USERPROFILE%\.android\debug.keystore` (alias
`androiddebugkey`, password `android`, valid until 2056), which is enough to sideload. Pass a
real keystore in for anything beyond that — the header of the script has the `keytool` line.

The game data is not part of any of this. It is not ours to ship, so no build bundles it; see
`push-data.bat` for getting a copy onto the device.

## The name and the icon

The app is shipped as **Zoo Tycoon**: `app_name` in `res/values/strings.xml` is the launcher
label and the SDL window title, and the activity label points at the same string. The package
id stays `org.rustyzoo` — it is what `data_dir` resolves against and what every `adb` command
names, and it is not shown to a player.

The launcher icon is the original game's own, pulled out of `zoo.exe`:

```
python android\make-launcher-icons.py "D:\Zoo Tycoon"
```

`zoo.exe` is the only place the game ships an icon, and it is a single 32×32, 8-bit image held
as PE resources (`RT_GROUP_ICON` for the directory, `RT_ICON` for the pixels). The script reads
those, decodes the DIB (palette, bottom-up XOR bitmap, bottom-up AND mask) and writes
`ic_launcher.png` for all five densities. Because 32 px is all there is, each density is a
supersample — nearest neighbour to four times the target, then Lanczos back down — which keeps
the "ZOO" lettering crisp where a plain upscale turns to mush. Re-run it after touching the
game install; the checked-in PNGs are the output, not the source.

## Running

```
android\push-data.bat "D:\Zoo Tycoon"
```

That installs the APK and pushes the game data to
`/sdcard/Android/data/org.rustyzoo/files/Zoo Tycoon`, which is where the shell reads it
(`data_dir` in `rustyzoo-android/src/lib.rs`). Then launch Zoo Tycoon on the device.

`push-data.bat` mangles paths with spaces; push by hand if it fails:

```
& $adb push "D:\Zoo Tycoon" "/sdcard/Android/data/org.rustyzoo/files/"
```

The data is not bundled in the APK: it is the user's own copy of the game, and the GPL build
ships no assets.

**MuMu Player restarts wipe `/sdcard/Android/data/org.rustyzoo/`.** Re-push the data after every
emulator relaunch.

A restart can also leave the `adb` server holding a dead connection, so `adb devices` lists
nothing even though the emulator is up. Kill the stray server and connect again:

```
& "D:\Program Files\Netease\MuMuPlayer\nx_main\adb.exe" connect 127.0.0.1:16384
```

MuMu 12 keeps 16384 for the first instance (16384, 16416, 16448, … for the rest); `netstat -ano`
filtered on the `MuMuNxDevice` pid shows the ports it actually listens on if that changes.

Native `stderr` is discarded on Android, so `eprintln!` never reaches logcat. The shell has a
`log` module that writes through `__android_log_write`; read it with:

```
adb logcat -s rustyzoo
```

The startup log names each milestone (`run: data dir`, `run: picture 505 at 400,150`,
`run: catalogue loaded`, `run: 1107 steps`, `run: main menu up, entering loop`, …) and
`SDL_main` reports any error, so a failure is visible without a debugger.

### Tapping the menu

`adb shell input tap x y` lands on the framebuffer point `(x, y)` — the coordinates are 1:1, in
the 1600×900 space the game renders. The main menu buttons, from `dump-layout --size 1600x900`:

| Button | Rectangle | Tap |
| --- | --- | --- |
| Play Freeform Game | 625, 410 – 975, 435 | `800 422` |
| Play Tutorial & Scenario Game | 666, 450 – 934, 475 | `800 462` |
| Load Saved Game | 666, 490 – 934, 515 | `800 502` |
| Continue Saved Game | 666, 530 – 934, 555 | `800 542` |
| View Game Credits | 682, 620 – 918, 641 | `800 630` |
| Exit Zoo Tycoon | 682, 660 – 918, 681 | `800 670` |

On the freeform screen the map list is the box at 416, 190 – 756, 664 and Play is at
1060, 710 – 1179, 737 (`1120 723`). Tap a row first: Play stays disabled until one is selected.

Buttons are transparent where their artwork is, so a tap on the very edge of a button can miss —
aim at the middle of the rectangle.

### Tapping in a game

The camera buttons of the left bar, from `dump-layout --size 1600x900` on the real game data
(`ui/main.lyt`). The tutorial scenario (`scn01a`) disables them in its `[disableUI]` list, so test
them in a freeform game:

| Button | Id | Rectangle | Tap |
| --- | --- | --- | --- |
| Zoom in | 1007 | 14, 795 – 50, 817 | `32 806` |
| Zoom out | 1023 | 5, 802 – 26, 827 | `15 814` |
| Rotate left (ccw) | 1008 | 6, 818 – 37, 850 | `21 834` |
| Rotate right (cw) | 1009 | 26, 805 – 72, 835 | `49 820` |
| Pause / Play | 1071 / 1072 | 170, 866 – 202, 898 | `186 882` |
| Construction | 1002 | 4, 13 – 47, 56 | `25 35` |

Zoom steps are `-2`, `0` and `2`, so one tap on zoom in goes straight from 0 to 2. Rotate left and
right are a quarter turn each. The pause and play buttons share one rectangle: one of them is
always hidden, and which one shows tells which way the tap goes.

These five were **not wired in the core at all** when the port started — see the core fixes below.

## The Windows/NDK 30 fixes

Cross-compiling SDL3 for Android from Windows hits four problems, all handled by
`build-core.bat` and its helpers. They are worth knowing because they are not obvious:

1. **The NDK's `aarch64-linux-android21-clang` is a `.cmd` wrapper**, which Rust cannot exec
   (`%1 is not a valid Win32 application`). `build-core.bat` points the linker at `clang.exe`.

2. **`cmake-rs` appends `--target=<rust-triple>` to `CMAKE_C_FLAGS`**, and the NDK toolchain file
   also passes `--target=<triple><api>`. Clang keeps the last one, so the bare one wins and the
   sysroot is lost (`crtbegin_dynamic.o: No such file or directory`). `clang.cmd` /
   `clang++.cmd` run `clang-wrapper.py`, which drops the duplicate.

3. **`sdl3-sys` passes `-DCMAKE_SYSTEM_NAME=Android`.** CMake caches a `-D` value as
   `UNINITIALIZED`, and the NDK toolchain file then does not recognize the build as Android:
   `WIN32` stays true from the host and SDL compiles `src/process/windows/SDL_windowsprocess.c`,
   which fails on `#include <windows.h>`. `patch-sdl3-sys.py` removes those two defines; the
   toolchain file sets them correctly on its own. The script runs before every build because
   cargo re-extracts the crate if the registry cache is cleaned.

4. **CMake 4.5.0-rc1 sets `WIN32` even when `CMAKE_SYSTEM_NAME` is `Android`.** SDL's
   `SDL_DetectCMakePlatform()` tests `WIN32` first, so it sets `WINDOWS` and compiles the
   Windows sources — the same failure as fix 3, from a different cause. `build-core.bat` points
   `CMAKE` at the CMake that ships with Visual Studio (4.3.1), which sets `WIN32` correctly.

If a build fails after a `cargo clean` or a registry change, delete
`target/aarch64-linux-android` and the `sdl3-sys` entries under `target/release/build` and
`target/release/.fingerprint`, then rebuild: cargo caches the build script and will not re-run
it just because the crate source changed.

## The link fixes

The link is where this port spent most of its time. All of these live in
`rustyzoo-android/.cargo/config.toml`, which `write-cargo-config.py` generates — **edit the
script, never the generated file**. The flags go through `rustflags`, not
`CARGO_TARGET_<TRIPLE>_RUSTFLAGS`.

1. **Static libc++.** `__gxx_personality_v0` is undefined at `dlopen` unless the C++ runtime is
   linked in: `-lc++_static -lc++abi -lunwind`.

2. **`clang_rt.builtins`.** `__arm_za_disable` is undefined at `dlopen` unless the compiler-rt
   builtins archive is linked. It is passed as a *file*, not with `-l`, because the archive's
   name does not start with `lib`.

3. **Dynamic libc.** NDK 30 puts the `libc.so` stubs in API-level subdirectories
   (`sysroot/usr/lib/aarch64-linux-android/21/libc.so`) and only static `.a` files in the
   top-level triple directory. lld resolves `-l` *per directory* — `lib<name>.so` then
   `lib<name>.a` within each directory before moving on — so a `-L` for the static directory
   makes it pick `libc.a` and the build links a static bionic, which then crashes in
   `getauxval` (a null deref in `init_have_lse_atomics`, because the static `libc.a` has no
   `__libc_shared_globals`). The fix is to **not** pass `-L` for the static directory: the
   clang driver adds it itself, *after* the API-level directory, so `libc.so` wins. Verified by
   `malloc`, `getauxval`, `dlopen`, `memcpy`, `pthread_create`, `sin` and `clone` being UND
   dynamic symbols with `libc.so` / `libm.so` / `libdl.so` in `DT_NEEDED`.

4. **`-Wl,--allow-multiple-definition`.** NDK 30's `libc.a` embeds a prebuilt Rust std that
   already defines `rust_eh_personality`.

5. **JNI symbol export.** This one took three attempts, and the order matters:

   - `--gc-sections` (passed by rustc) removed the JNI functions, because SDL3 is built with
     `-ffunction-sections` and nothing in Rust references them. Fixed with `-Wl,--no-gc-sections`.
   - rustc's `--version-script` then marked all 67 `Java_*` symbols `local`.
     `-Wl,--export-dynamic-symbol=Java_*` does **not** override a version script's `local: *`
     — verified with a minimal lld test.
   - lld applies the **last** `--version-script` it is given. `write-cargo-config.py` therefore
     generates `jni-exports.map` with `{ global: Java_*; JNI_*; SDL_main; local: *; };` and
     passes it as another `--version-script`. Only globs and certainly-defined symbols may be
     listed: lld **fails the link** with `version script assignment of 'global' to symbol X
     failed: symbol not defined` for undefined names.

   `librustyzoo.so` now exports exactly 69 defined symbols — all 67
   `Java_org_libsdl_app_*` plus `JNI_OnLoad` and `SDL_main` — and `UnsatisfiedLinkError` is gone.

## The port-specific fixes

These are in `rustyzoo-android/src/lib.rs` and have no desktop counterpart.

1. **The boot flow.** The first version called `App::new()` + `start_game(None)`, which drops the
   player straight into a game with no start-up picture and no menu. The desktop shell goes
   through `boot::boot` — the start-up picture with its progress bar while the data loads, then
   `App::menu` — but `boot` is a **bin-crate-only** module. The shell therefore has its own
   equivalent: `Startup::new` presented while `Catalogue::load_with` steps (one step per resource
   directory, per entity type, per interface layout and one after each stage — 1107 in total),
   then `Menu::new` + `set_credits` + `set_lists`, then `App::menu`. Everything the menu needs
   (`menu`, `startup`, `lists`, `userdir`, `music`) is public in the core; only `layout` is not,
   and that is fix 2 below. Verified end to end: picture → progress → main menu → freeform list →
   game.

2. **The interface.** `App::new()` → `App::empty()` leaves `ui: None` — the "bare" map-only mode
   — and `App::paint()` then only redraws when the camera moves, so the picture froze after the
   first frame. The Windows shell calls `crate::layout::load_interface()` +
   `app.set_interface(ui)`, but `layout` is a **bin-crate-only** module (`main.rs` declares it)
   and is unreachable from the Android shell. The shell therefore has its own `load_interface()`
   that reproduces it from the public core API (`StringTables::open`, `Tree::load`,
   `installed_expansions`, `Fonts`, `Interface::new`, `Settings::from_config`), without the
   scripted `Fill`. `App::menu` takes the interface as an argument, so it is built before the app
   rather than installed after it.

3. **The data directory.** SDL sets `EXTERNAL_STORAGE=/sdcard` (the root of shared storage), not
   the app's external files directory, so the obvious `data_dir()` resolved to
   `/sdcard/Zoo Tycoon`, which does not exist. The shell calls
   `SDL_GetAndroidExternalStoragePath()` instead, declared as a raw `unsafe extern "C"` because
   the `sdl3` crate 0.20.0 exposes no wrapper; it resolves out of the statically linked SDL3
   archive like any other SDL entry point.

4. **Touch coordinates are normalized.** SDL reports `FingerDown` / `FingerMotion` / `FingerUp`
   `x` and `y` in the *unit square*, not in pixels. Casting them to `i32` gives `(0, 0)` for
   every event, so touch did nothing at all on a real device. `Touch::at` scales them by the
   framebuffer size; `Touch::mouse_at` first divides the mouse's window pixels by the window
   size, so both paths land in the same space. The window size comes from
   `canvas.window().size_in_pixels()`.

5. **A drag pans one scroll per 64 pixels.** `Command::Scroll` takes a *direction*, and each one
   moves the camera by the ini's `keyScrollX` (64). Emitting one per motion event made a 400 px
   drag move the view ~4.5× too far — straight off the edge of the map, leaving 60 % of the
   screen black. `Touch::drag` accumulates the movement and emits one `Scroll` per
   `DRAG_STEP` pixels, so the view follows the finger. Verified reversible: 4.1 % dark → 13.1 %
   after a 400 px left drag → 4.1 % again after the matching right drag.

6. **The loop sleeps.** The first version spun on `poll_event` at ~280 k passes a second and
   burned a core. `Timers::next_due` gives the milliseconds to the next firing, so the loop
   sleeps that long minus a margin and then polls. `EventPump::wait_event_timeout_ms` is *not*
   used: on Android it does not come back, and the app hangs at 0 % CPU after the first frame.

7. **A held mouse button drags.** MuMu's `input swipe` generates *mouse* events, not finger
   events, so the drag path never ran in the emulator. `Touch` tracks whether a button is held
   and treats a drag with one held exactly like a finger, which also makes a Bluetooth mouse
   work. The same gesture pans on a device and in the emulator.

8. **MuMu reports one tap twice.** The emulator answers a single `input tap` with *both* a mouse
   click and a finger touch of the same point, so every tap ran the command twice — a construction
   button opened its panel and closed it again in the same pass. `Touch` carries a `finger` flag:
   while a mouse button is held the finger events are dropped, and while a finger is down the
   mouse events are dropped, so exactly one of the two paths answers. `MouseButtonDown` also
   pushes a `Mouse::Move` first, so the widget under the press is hovered before it is pressed
   (a hover drives tooltips and the cursor).

The engine only repaints when the camera moves or the interface is dirty, so an idle game paints
at 1–2 fps and a pan at the full 60 fps. That is the engine's design (`App::paint`), not a port
bug — the desktop shell behaves the same way.

## The core fixes the port found

Three of these are bugs in the **core library**, not in the Android shell: they affect the desktop
build too, and were found here because the port is the first thing that drives the interface from
a touchscreen. They are fixed in `src/`, with regression tests.

1. **Buttons below their layout were unclickable.** `ui/confirml.lyt` (layout 45100, the question
   dialog) declares `dy=300` but places its buttons at `y=315` — outside its own rectangle.
   `Interface::hit_in` gated hit-testing on the layout rect, so the OK button of every question
   could not be pressed: the tutorial stopped dead at its first dialog. The fix drops the
   layout-rect gate and keeps only the `HIDDEN | NEVER_SHOWN` one. Regression test:
   `a_question_answers_a_button_that_draws_below_its_layout` in
   `src/ui/interface/dialog/tests.rs`. The same change also replaced an `e.children.clone()` in
   the hit loop with index iteration.

2. **The camera buttons did nothing.** The zoom, rotate and pause buttons of `ui/main.lyt` carry
   no `action=` and no `target=` — only `animation=` — so the core had no path from a press on
   them to a command. `src/app/camera.rs` (new) maps the six ids
   (`ZOOM_IN=1007`, `ZOOM_OUT=1023`, `ROTATE_LEFT=1008`, `ROTATE_RIGHT=1009`, `PAUSE=1071`,
   `PLAY=1072`) to the `Command::Zoom` / `Command::Rotate` / `Command::Pause` their keys already
   ran, and `camera_sync()` keeps the pause/play pair in line with `Game::paused`. Four
   regression tests in `src/app/camera_tests.rs`.

3. **A tap double-fired in the emulator.** Fix 8 above; the `finger` flag lives in the shell, but
   the *symptom* — a press and a release in one pass, so a toggle button ended where it started —
   is what made the construction panel appear to do nothing.

Verified on device, MuMu Player 900×1600, freeform map at 1600×900:

| Button | Evidence |
| --- | --- |
| Zoom in | map repainted, 804 922 of 998 400 map pixels changed |
| Rotate ccw | 1 289 636 pixels changed; ccw then cw returns the view to within 987 pixels |
| Pause | two screenshots 12 s apart are **byte-identical** (0 pixels changed; a running game changes 27 247 over the same span) |

The core was checked the same way without a device, by replaying the same five taps as a headless
script (`out/cam.txt`): `ui select 1007 → zoom in → zoom=2`, `ui select 1008 → rotate ccw →
rotation=3`, `ui select 1009 → rotate cw → rotation=0`, `ui select 1071 → pause`. Note that a
rotate *does* move the view as well as the rotation — `Camera::change` re-locates the remembered
tile and may nudge the scroll by ±64 px to keep it on screen — so a rotate left followed by a
rotate right returns the camera exactly, but a rotate of a *running* game shows a world that has
moved on in between.

## Files

| File | Role |
| --- | --- |
| `rustyzoo-android/src/lib.rs` | The Android shell: SDL init, game loop, touch input, `SDL_main`, `load_interface` shim |
| `../src/app/camera.rs` | Core: the zoom, rotate and pause buttons (core fix 2) |
| `../src/app/camera_tests.rs` | Its regression tests |
| `make-launcher-icons.py` | Builds `ic_launcher.png` for every density out of `zoo.exe` |
| `build-core.bat` | Cross-compile the Rust library |
| `build-apk.bat` | Library, `.so`, APK |
| `sign-apk.bat` | Align and sign the release APK |
| `push-data.bat` | Install APK and push game data |
| `write-cargo-config.py` | Generates `.cargo/config.toml` **and** `jni-exports.map` |
| `patch-sdl3-sys.py` | Fix 3 above |
| `clang-wrapper.py`, `clang.cmd`, `clang++.cmd` | Fix 2 above |
| `app/` | The Gradle module: manifest, activity, resources |
| `app/src/main/java/org/rustyzoo/RustyZooActivity.java` | Loads `librustyzoo.so` only |

## Adding another ABI

`build-core.bat` builds `aarch64-linux-android` only. For `armv7` / `x86_64` / `x86`, add the
target, add a `[target.<triple>]` block to `.cargo/config-android.toml`, set the matching
`CC_<triple>` / `AR_<triple>` in the script, and add the ABI to `abiFilters` in
`app/build.gradle`. The `clang-wrapper.py` fix applies to all of them.
