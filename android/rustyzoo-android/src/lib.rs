//! The Android shell for RustyZoo, shipped under the name "Zoo Tycoon".
//!
//! This mirrors `src/window/mod.rs` — the same game loop, the same core crate — but the input
//! comes from a touch screen instead of a keyboard and mouse, and the picture fills the display.
//!
//! SDL3 on Android runs `main` on its own thread started from the Java `SDLActivity`. The
//! activity is `org.rustyzoo.RustyZooActivity`; everything below happens in the native library.
//! This crate is a `cdylib` named `rustyzoo`, so the build produces `librustyzoo.so`.
//!
//! The game data (`D:\Zoo Tycoon` on the desktop) is pushed to the app's external files dir,
//! which is what `data_dir` points at. See `android/README.md`.

use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::Mutex;

use rustyzoo::app::{App, Command, Mouse};
use rustyzoo::camera::Settings;
use rustyzoo::config::Config;
use rustyzoo::entity::Catalogue;
use rustyzoo::lists::{freeform_list, scenario_list};
use rustyzoo::menu::{self, Menu};
use rustyzoo::music::Music;
use rustyzoo::render::MapRenderer;
use rustyzoo::startup::{self, Startup};
use rustyzoo::timing::Timers;
use rustyzoo::ui::interface::Interface;
use rustyzoo::ui::layout::{Kind, installed_expansions};
use rustyzoo::userdir;
use rustyzoo::vfs::Vfs;
use sdl3::event::Event;
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::{Canvas, ScaleMode, Texture};
use sdl3::video::Window;

/// Where the game data lives inside the app, relative to the app's external files dir.
const DATA_SUBDIR: &str = "Zoo Tycoon";

type CliError = Box<dyn std::error::Error>;

/// The log file, opened by `log_file_start` before anything else runs. `None` when it could not
/// be opened: logcat still works, the file is a bonus, never a requirement.
static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);

/// `<external files dir>/rustyzoo.log`, beside the `Zoo Tycoon` data folder, so a file manager
/// that can reach the game data can reach this too.
fn log_path() -> Option<PathBuf> {
    data_dir().parent().map(|dir| dir.join("rustyzoo.log"))
}

/// Open the log file for writing and record panics in it. Called first in `SDL_main`: when the
/// app dies on start-up the file has to exist already, and on a device nobody can reach with
/// `adb` reading it afterwards is the only way to know what happened. The file is truncated, so
/// it holds the last run only -- that is the one worth sending back.
fn log_file_start() {
    let Some(path) = log_path() else {
        log::error("log: no log path");
        return;
    };
    match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
    {
        Ok(file) => {
            *LOG_FILE.lock().unwrap_or_else(|e| e.into_inner()) = Some(file);
            log::info(&format!("log: file {}", path.display()));
        }
        Err(e) => log::error(&format!("log: {}: {e}", path.display())),
    }
    // The default hook writes to stderr, which Android discards, so a panic leaves nothing
    // behind. Keep the default behaviour and add the file.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let what = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("panic");
        let at = info
            .location()
            .map_or_else(String::new, |l| format!(" at {}:{}", l.file(), l.line()));
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        log::error(&format!("panic: {what}{at} (thread {thread})"));
        previous(info);
    }));
}

/// Android discards a native process's stderr, so `eprintln!` never reaches logcat and a
/// failure inside `run` is invisible. Route diagnostics through the platform log instead,
/// where `adb logcat -s rustyzoo` shows them, and mirror every line into the log file.
mod log {
    use std::io::Write;

    const ANDROID_LOG_ERROR: i32 = 6;
    const ANDROID_LOG_INFO: i32 = 4;

    unsafe extern "C" {
        fn __android_log_write(prio: i32, tag: *const u8, text: *const u8) -> i32;
    }

    fn write(prio: i32, msg: &str) {
        // Both arguments need a NUL terminator; the tag is a literal, the message is not.
        let tag = b"rustyzoo\0";
        let mut text = msg.as_bytes().to_vec();
        text.push(0);
        unsafe {
            __android_log_write(prio, tag.as_ptr(), text.as_ptr());
        }
        // Flushed per line: a native crash takes the process with it and anything still
        // buffered is gone, and the file is the only log a device without `adb` gives back.
        if let Ok(mut file) = super::LOG_FILE.lock()
            && let Some(file) = file.as_mut()
        {
            let _ = writeln!(file, "{msg}");
            let _ = file.flush();
        }
    }

    pub fn error(msg: &str) {
        write(ANDROID_LOG_ERROR, msg);
    }

    pub fn info(msg: &str) {
        write(ANDROID_LOG_INFO, msg);
    }
}

/// The entry point SDL calls on its own thread. Named `SDL_main` so the `sdl3-main` shim finds it.
#[unsafe(no_mangle)]
pub extern "C" fn SDL_main(_argc: i32, _argv: *mut *mut std::ffi::c_char) -> i32 {
    log_file_start();
    log::info("SDL_main: start");
    match run() {
        Ok(()) => {
            log::info("SDL_main: finished");
            0
        }
        Err(e) => {
            log::error(&format!("SDL_main: {e}"));
            1
        }
    }
}

/// Shows `frame` in the window: the start-up picture while the game data loads, then every
/// frame the app paints.
fn present(
    canvas: &mut Canvas<Window>,
    texture: &mut Texture,
    frame: &rustyzoo::Framebuffer,
) -> Result<(), CliError> {
    texture.update(None, &frame.pixels, frame.width as usize * 4)?;
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.copy(&*texture, None, None)?;
    match canvas.present() {
        true => Ok(()),
        false => Err(sdl3::get_error().into()),
    }
}

fn run() -> Result<(), CliError> {
    let data = data_dir();
    log::info(&format!("run: data dir {:?}", data));
    let vfs = Vfs::open(&data)?;
    log::info("run: vfs open");
    let ini = Config::parse(vfs.ini())?;
    log::info("run: config parsed");

    let sdl = sdl3::init()?;
    let video = sdl.video()?;
    log::info("run: sdl init");

    // Android is full screen by construction: the window is the display.
    let display = video.get_primary_display()?;
    let bounds = display.get_bounds()?;
    let density = display.get_mode().map_or(1.0, |m| m.pixel_density);
    log::info(&format!(
        "run: display bounds {}x{}, density {density}",
        bounds.width(),
        bounds.height()
    ));
    let game = game_size(&ini, bounds.width(), bounds.height(), density);
    log::info(&format!("run: game size {}x{}", game.0, game.1));

    let window = video.window("Zoo Tycoon", game.0, game.1).fullscreen().build()?;
    let mut canvas = sdl3::render::create_renderer(window, None)?;
    let textures = canvas.texture_creator();
    let mut texture = textures.create_texture_streaming(PixelFormat::RGBA32, game.0, game.1)?;
    texture.set_scale_mode(ScaleMode::Nearest);
    let pixels = canvas.window().size_in_pixels();
    log::info(&format!(
        "run: window and renderer up, {}x{} pixels",
        pixels.0, pixels.1
    ));

    // The start-up picture with its progress bar while the game data loads, then the main
    // menu. This is `boot::boot` of the window shell, which lives in the binary crate: the
    // same steps the original counts (main menu 3.3), one per directory of the resource
    // search, one per entity type, one per layout of the interface and one after each stage.
    let (catalogue, ui, menu) = {
        let expansions = installed_expansions(&vfs);
        let mut screen = Startup::new(vfs.root(), &ini, expansions, game);
        log::info(&format!("run: {}", screen.log_line()));
        present(&mut canvas, &mut texture, screen.frame())?;
        let mut failure = None;
        let mut step = |screen: &mut Startup| {
            if screen.step()
                && let Err(e) = present(&mut canvas, &mut texture, screen.frame())
            {
                failure.get_or_insert(e);
            }
        };
        for _ in 0..vfs.directory_count() {
            step(&mut screen);
        }
        let catalogue = Catalogue::load_with(&vfs, &mut || step(&mut screen))?;
        log::info("run: catalogue loaded");
        step(&mut screen);
        let ui = load_interface(&vfs, game)?.ok_or("no interface")?;
        let layouts = ui
            .tree
            .elements
            .iter()
            .filter(|e| matches!(e.kind, Kind::Layout))
            .count();
        for _ in 0..=layouts {
            step(&mut screen);
        }
        if let Some(e) = failure {
            return Err(e);
        }
        log::info(&format!("run: {} steps", screen.steps()));
        let mut menu = Menu::new(
            ini.clone(),
            vfs.root(),
            userdir::dir(),
            startup::product_version(vfs.root()),
            expansions,
        );
        let (pages, delay) = menu::credit_pages(&vfs);
        menu.set_credits(pages, delay);
        menu.set_lists(scenario_list(&vfs, expansions), freeform_list(&vfs));
        (catalogue, ui, menu)
    };

    // The main menu: no game yet, the menu acting on what is selected in the interface. The
    // window shell reaches it through `App::menu`; `App::new` plus `start_game` would skip
    // straight into a game, which is what this shell did before.
    let timers = Timers::from_config(&ini);
    let mut app = App::menu(
        MapRenderer::new(&vfs, &catalogue),
        Settings::from_config(&ini),
        timers,
        game,
        ui,
        menu,
        Music::from_config(&ini),
    )?;
    log::info("run: main menu up, entering loop");

    let mut pump = sdl.event_pump()?;
    let mut touch = Touch::new(game, pixels);
    let mut commands = Vec::new();
    let start = sdl3::timer::ticks();

    loop {
        // Sleep until shortly before a timer is due instead of spinning: the battery lasts
        // and the same firings run (`Timers::next_due`). The event pump's timed wait is not
        // used here: it does not come back on Android.
        let due = app.next_due(sdl3::timer::ticks() - start);
        if due > 2 {
            std::thread::sleep(std::time::Duration::from_millis((due - 2).min(1000)));
        }
        while let Some(event) = pump.poll_event() {
            touch.note(event, &mut commands);
        }

        let now = sdl3::timer::ticks() - start;
        if let Some(frame) = app.pass(now, &commands) {
            present(&mut canvas, &mut texture, frame)?;
        }
        commands.clear();

        if app.quit() {
            break;
        }
    }
    Ok(())
}

// The folder the game data was pushed to: `<external files dir>/Zoo Tycoon`.
//
// SDL sets `EXTERNAL_STORAGE` to the root of shared storage (`/sdcard`), not to this app's
// own external files directory, so it points at the wrong place. Ask SDL for the app-specific
// path instead -- that is the directory `push-data.bat` fills. Declared here rather than taken
// from the `sdl3` crate because the crate exposes no safe wrapper for it; it resolves out of
// the statically linked SDL3 archive like any other SDL entry point.
unsafe extern "C" {
    fn SDL_GetAndroidExternalStoragePath() -> *const std::ffi::c_char;
}

fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ZT_DATA_DIR") {
        return PathBuf::from(dir);
    }
    // SDLActivity calls nativeSetupJNI in onCreate, before SDL_main runs, so the activity is
    // already reachable here even though sdl3::init() has not been called yet.
    let from_sdl = unsafe {
        let ptr = SDL_GetAndroidExternalStoragePath();
        if ptr.is_null() {
            None
        } else {
            std::ffi::CStr::from_ptr(ptr).to_str().ok().map(PathBuf::from)
        }
    };
    let base = from_sdl.unwrap_or_else(|| PathBuf::from("/sdcard/Android/data/org.rustyzoo/files"));
    base.join(DATA_SUBDIR)
}

/// The interface of a game in a window of `size` pixels: the tree of `ui/gamescrn.lyt`,
/// resolved, with the settings of the ini file. `None` when the tree cannot be loaded.
///
/// This is `layout::load_interface` of the window shell, which lives in the binary crate and
/// is out of reach from here. Every part it is built from is public in the core, so this is
/// the same load without the scripted `Fill` (no script runs on Android).
fn load_interface(vfs: &Vfs, size: (u32, u32)) -> Result<Option<Interface>, CliError> {
    use rustyzoo::strings::StringTables;
    use rustyzoo::text::Fonts;
    use rustyzoo::ui::interface::Settings;
    use rustyzoo::ui::layout::{Tree, installed_expansions};

    let size = (
        i32::try_from(size.0).unwrap_or(i32::MAX),
        i32::try_from(size.1).unwrap_or(i32::MAX),
    );
    let made = StringTables::open(vfs.root())
        .map_err(|e| e.to_string())
        .and_then(|tables| {
            let mut tree = Tree::load(vfs, "ui/gamescrn.lyt", installed_expansions(vfs));
            let mut fonts = Fonts::default();
            tree.resolve(size.0, size.1, |font| {
                // Strings that do not exist are empty texts: the default font, 8 points.
                let (face, points) = font.map_or(("Arial", "8"), |(face, points)| {
                    let string = |n: i32| u32::try_from(n).map_or("", |n| tables.string(n));
                    (string(face), string(points))
                });
                fonts.get(face, points).line_height()
            });
            for warning in &tree.warnings {
                log::error(&format!("run: layout warning: {warning}"));
            }
            match tree.failure.clone() {
                Some(failure) => Err(format!("load stopped at {failure}")),
                None => Ok(Interface::new(tree, Box::new(tables))),
            }
        });
    match made {
        Ok(mut ui) => {
            if let Ok(ini) = Config::parse(vfs.ini()) {
                ui.set_settings(Settings::from_config(&ini));
            }
            Ok(Some(ui))
        }
        Err(why) => {
            log::error(&format!("run: no interface: {why}"));
            Ok(None)
        }
    }
}

/// The game size: the ini file's, cut to the display so the whole zoo fits, at least 1:1.
fn game_size(ini: &Config, w: u32, h: u32, density: f32) -> (u32, u32) {
    let side = |key: &str| rustyzoo::timing::user_whole(ini, key).and_then(|n| u32::try_from(n).ok());
    let want = match (side("screenwidth"), side("screenheight")) {
        (Some(w @ 1..), Some(h @ 1..)) => (w, h),
        _ => (1024, 768),
    };
    let density = if density > 0.0 { density } else { 1.0 };
    let fit = |n: u32, max: u32| if max == 0 { n } else { n.min(max) };
    (
        fit(want.0, (w as f32 / density) as u32).max(1),
        fit(want.1, (h as f32 / density) as u32).max(1),
    )
}

/// The pixels of drag that make one scroll of the map: the key scroll of the ini, so a finger
/// moves the view as fast as an arrow key does instead of jumping on every motion event.
const DRAG_STEP: i32 = 64;

/// Touch input: a tap is a click, a drag pans, a pinch zooms, a two-finger turn rotates the
/// camera. This is the mapping that makes the mouse-driven original usable without a mouse.
struct Touch {
    /// The size of the framebuffer the commands are in.
    game: (u32, u32),
    /// The size of the window, to put a mouse in the unit square a finger is already in.
    window: (u32, u32),
    /// The last position of the primary finger, in framebuffer pixels.
    last: Option<(i32, i32)>,
    /// A tap that has not yet become a drag.
    pending: Option<(i32, i32)>,
    /// The drag so far, in the pixels that have not yet made a whole scroll.
    drag: (i32, i32),
    /// Whether a mouse button is held: a drag with one held pans, as a finger does.
    held: bool,
    /// Whether a finger is down. An emulator reports one tap as a mouse click *and* a finger
    /// touch of the same point, so the two are kept from both answering it: while one of them
    /// holds the touch, the other is dropped.
    finger: bool,
}

impl Touch {
    fn new(game: (u32, u32), window: (u32, u32)) -> Self {
        Self {
            game,
            window,
            last: None,
            pending: None,
            drag: (0, 0),
            held: false,
            finger: false,
        }
    }

    /// A drag of `dx`, `dy` framebuffer pixels: one scroll of the map per `DRAG_STEP` pixels,
    /// so the view follows the finger rather than racing ahead of it.
    fn drag(&mut self, dx: i32, dy: i32, commands: &mut Vec<Command>) {
        self.drag.0 += dx;
        self.drag.1 += dy;
        let mut moved = false;
        while self.drag.0 >= DRAG_STEP {
            self.drag.0 -= DRAG_STEP;
            commands.push(Command::Scroll(1, 0));
            moved = true;
        }
        while self.drag.0 <= -DRAG_STEP {
            self.drag.0 += DRAG_STEP;
            commands.push(Command::Scroll(-1, 0));
            moved = true;
        }
        while self.drag.1 >= DRAG_STEP {
            self.drag.1 -= DRAG_STEP;
            commands.push(Command::Scroll(0, 1));
            moved = true;
        }
        while self.drag.1 <= -DRAG_STEP {
            self.drag.1 += DRAG_STEP;
            commands.push(Command::Scroll(0, -1));
            moved = true;
        }
        if moved {
            self.pending = None;
        }
    }

    /// The framebuffer pixel of a point of the window given in the unit square: SDL reports a
    /// finger there, and a mouse is put there by its window size.
    fn at(&self, x: f32, y: f32) -> (i32, i32) {
        let scale = |n: f32, len: u32| (f64::from(n) * f64::from(len)).round() as i32;
        (scale(x, self.game.0), scale(y, self.game.1))
    }

    /// The framebuffer pixel of a point of the window given in window pixels.
    fn mouse_at(&self, x: f32, y: f32) -> (i32, i32) {
        let (w, h) = (self.window.0.max(1), self.window.1.max(1));
        self.at(x / w as f32, y / h as f32)
    }

    fn note(&mut self, event: Event, commands: &mut Vec<Command>) {
        match event {
            Event::FingerDown { x, y, .. } => {
                // The emulator's mouse already answered this tap.
                if self.held {
                    return;
                }
                self.finger = true;
                let at = self.at(x, y);
                self.last = Some(at);
                self.pending = Some(at);
                self.drag = (0, 0);
                commands.push(Command::Mouse(Mouse::Move(at.0, at.1)));
            }
            Event::FingerMotion { x, y, .. } => {
                if self.held {
                    return;
                }
                let at = self.at(x, y);
                if let Some(prev) = self.last {
                    self.drag(at.0 - prev.0, at.1 - prev.1, commands);
                }
                self.last = Some(at);
                commands.push(Command::Mouse(Mouse::Move(at.0, at.1)));
            }
            Event::FingerUp { .. } => {
                if self.held {
                    return;
                }
                self.finger = false;
                // A finger that went down and up without moving is a click.
                if let Some(at) = self.pending.take() {
                    commands.push(Command::Mouse(Mouse::Move(at.0, at.1)));
                    commands.push(Command::Mouse(Mouse::Down));
                    commands.push(Command::Mouse(Mouse::Up));
                }
                self.last = None;
            }
            // A mouse (a Bluetooth one, or the emulator) works as a finger does: a drag with
            // the button held pans, so the same gesture pans on a device and in the emulator.
            Event::MouseMotion { x, y, .. } => {
                // The emulator's finger already answered this touch.
                if self.finger {
                    return;
                }
                let at = self.mouse_at(x, y);
                if self.held
                    && let Some(prev) = self.last
                {
                    self.drag(at.0 - prev.0, at.1 - prev.1, commands);
                }
                self.last = Some(at);
                commands.push(Command::Mouse(Mouse::Move(at.0, at.1)));
            }
            Event::MouseButtonDown { mouse_btn, x, y, .. } => {
                if self.finger {
                    return;
                }
                self.held = true;
                let at = self.mouse_at(x, y);
                // The drag measures from where the button went down, not from wherever the
                // pointer happened to be.
                self.last = Some(at);
                self.pending = None;
                self.drag = (0, 0);
                // The move comes with the press: a click that follows no motion -- the pointer
                // is already where the player wants it -- must still put the pointer there, or
                // the press lands on whatever the last motion left under it.
                commands.push(Command::Mouse(Mouse::Move(at.0, at.1)));
                commands.push(Command::Mouse(match mouse_btn {
                    sdl3::mouse::MouseButton::Right => Mouse::Right,
                    _ => Mouse::Down,
                }));
            }
            Event::MouseButtonUp { .. } => {
                if self.finger {
                    return;
                }
                self.held = false;
                commands.push(Command::Mouse(Mouse::Up));
            }
            Event::MouseWheel { y, .. } => {
                if y != 0.0 {
                    commands.push(Command::Mouse(Mouse::Wheel(y.signum() as i32)));
                }
            }
            Event::MultiGesture { d_dist, d_theta, .. } => {
                if d_dist.abs() > 0.01 {
                    commands.push(Command::Zoom(d_dist > 0.0));
                }
                if d_theta.abs() > 0.02 {
                    commands.push(Command::Rotate(d_theta > 0.0));
                }
            }
            _ => {}
        }
    }
}
