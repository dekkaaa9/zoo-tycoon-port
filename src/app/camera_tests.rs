//! The camera buttons of the left bar: each one runs the command of the act it stands for, and the
//! play button takes the place of the pause button while the game is paused.

use super::*;
use crate::entity::Catalogue;
use crate::render::tests::{cache_install_with, flat_map, frame_file, tile_with};
use crate::sprite::tests::Install;
use crate::ui::draw::Strings as UiStrings;
use crate::ui::layout::Tree;
use crate::vfs::Vfs;
use crate::zoom::Zoom;

const SIZE: (u32, u32) = (64, 48);

struct Texts;

impl UiStrings for Texts {
    fn string(&self, _: i32) -> String {
        String::new()
    }

    fn text_entry(&self, _: &[u8]) -> Vec<u8> {
        vec![]
    }
}

fn map() -> Zoo {
    let mut zoo = tile_with(&[]);
    zoo.width = 4;
    zoo.height = 4;
    zoo.tiles = flat_map(4, 4);
    zoo
}

/// An interface with the six camera buttons, each 8x8 of its own, in a layout that starts hidden.
/// A button without a picture is transparent, so it is hit over its whole rectangle only when it
/// says it is not (ui/main.lyt gives each of them an animation instead).
fn interface() -> Interface {
    let root = "[LayoutInfo]\nid=1\nx=0\ny=0\ndx=whole\ndy=whole\n\
        [a]\ntype=ZTMapView\nid=7000\ndx=whole\ndy=whole\ntransparent=0\n\
        [bar]\ntype=UILayout\nlayout=ui/bar.lyt\nstate=1\n";
    // The two buttons of the pause pair are at the same place, as in ui/main.lyt.
    let bar = "[LayoutInfo]\nid=2\nlayer=5\nx=0\ny=0\ndx=64\ndy=48\n\
        [zoom in]\ntype=UIButton\nid=1007\nx=2\ny=2\ndx=8\ndy=8\ntransparent=0\n\
        [zoom out]\ntype=UIButton\nid=1023\nx=2\ny=12\ndx=8\ndy=8\ntransparent=0\n\
        [rotate left]\ntype=UIButton\nid=1008\nx=2\ny=22\ndx=8\ndy=8\ntransparent=0\n\
        [rotate right]\ntype=UIButton\nid=1009\nx=12\ny=2\ndx=8\ndy=8\ntransparent=0\n\
        [pause]\ntype=UIButton\nid=1071\nx=12\ny=12\ndx=8\ndy=8\ntransparent=0\n\
        [play]\ntype=UIButton\nid=1072\nx=12\ny=12\ndx=8\ndy=8\ntransparent=0\nstate=1\n";
    let files = Install::new(
        "app-camera-ui",
        &[
            ("ui/root.lyt", root.as_bytes().to_vec()),
            ("ui/bar.lyt", bar.as_bytes().to_vec()),
        ],
    );
    let mut tree = Tree::load(&Vfs::open(&files.0).unwrap(), "ui/root.lyt", 0);
    assert_eq!(tree.failure, None);
    tree.resolve(SIZE.0 as i32, SIZE.1 as i32, |_| 12);
    let mut ui = Interface::new(tree, Box::new(Texts));
    assert!(ui.show(2));
    ui
}

/// Runs `body` on a game whose interface has the camera buttons shown.
fn with_camera(body: impl FnOnce(&mut App)) {
    let install = cache_install_with("app-camera", frame_file(), &[]);
    let vfs = Vfs::open(&install.0).unwrap();
    let catalogue = Catalogue::load(&vfs).unwrap();
    let mut app = App::new(
        MapRenderer::new(&vfs, &catalogue),
        map(),
        Settings::default(),
        Timers::new(66, 16, 200),
        SIZE,
    )
    .unwrap();
    app.set_interface(interface());
    app.pass(0, &[]);
    body(&mut app);
}

/// A click at the middle of a button: the pointer moves there, the button goes down and comes up
/// inside it, as a mouse does. The command runs in the pass of the release.
fn click(app: &mut App, at: (i32, i32), now: u64) {
    let move_at = Command::Mouse(Mouse::Move(at.0, at.1));
    app.pass(now, &[move_at, Command::Mouse(Mouse::Down)]);
    app.pass(now + 10, &[Command::Mouse(Mouse::Up)]);
}

#[test]
fn the_zoom_buttons_change_the_zoom_of_the_camera() {
    with_camera(|app| {
        let zoom = |app: &App| Zoom::from_value(app.camera().unwrap().zoom);
        assert_eq!(zoom(app), Zoom::Normal);
        click(app, (6, 6), 20);
        assert_eq!(zoom(app), Zoom::In, "zoom in");
        click(app, (6, 16), 40);
        assert_eq!(zoom(app), Zoom::Normal, "zoom out");
    });
}

#[test]
fn the_rotate_buttons_turn_the_camera_the_other_way() {
    with_camera(|app| {
        assert_eq!(app.camera().unwrap().rotation, 0);
        click(app, (6, 26), 20);
        assert_eq!(app.camera().unwrap().rotation, 3, "left");
        click(app, (16, 6), 40);
        assert_eq!(app.camera().unwrap().rotation, 0, "right");
    });
}

#[test]
fn the_pause_button_pauses_and_the_play_button_that_takes_its_place_unpauses() {
    with_camera(|app| {
        assert!(!app.paused());
        click(app, (16, 16), 20);
        assert!(app.paused(), "the pause button pauses");
        // The play button is the one that shows now, at the same place.
        assert!(app.interface().unwrap().shown(1072));
        assert!(!app.interface().unwrap().shown(1071));
        click(app, (16, 16), 40);
        assert!(!app.paused(), "the play button unpauses");
        assert!(app.interface().unwrap().shown(1071));
        assert!(!app.interface().unwrap().shown(1072));
    });
}

#[test]
fn a_camera_button_tells_the_command_it_ran_in_the_log() {
    with_camera(|app| {
        click(app, (6, 6), 20);
        let lines = app.log_lines(2, 30);
        assert!(lines.contains(&"pass 2 t=30 zoom in".to_string()), "{lines:?}");
        click(app, (16, 16), 40);
        let lines = app.log_lines(4, 50);
        assert!(lines.contains(&"pass 4 t=50 pause".to_string()), "{lines:?}");
    });
}
