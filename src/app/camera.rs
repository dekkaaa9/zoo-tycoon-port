//! The camera buttons of the interface: the zoom, the rotate and the pause buttons of the left
//! bar. Each one stands for the act of a key (input 3), so a press on it runs the command that key
//! would have run, and the play button of a paused game takes the place of its pause button.

use super::*;

/// The zoom in and zoom out buttons of the left bar (ui/main.lyt).
pub(super) const ZOOM_IN: i32 = 1007;
pub(super) const ZOOM_OUT: i32 = 1023;
/// The rotate left and rotate right buttons of the left bar: they turn the camera, not an item.
pub(super) const ROTATE_LEFT: i32 = 1008;
pub(super) const ROTATE_RIGHT: i32 = 1009;
/// The pause button, drawn while the game runs, and the play button that takes its place while the
/// game is paused. Both are at the same place, one of them always hidden.
pub(super) const PAUSE: i32 = 1071;
pub(super) const PLAY: i32 = 1072;

/// Whether `id` is one of the camera buttons: those run a command instead of a panel's.
pub(super) fn is_camera_button(id: i32) -> bool {
    matches!(
        id,
        ZOOM_IN | ZOOM_OUT | ROTATE_LEFT | ROTATE_RIGHT | PAUSE | PLAY
    )
}

impl App<'_> {
    /// A button of the interface was released as selected: when it is one of the camera buttons,
    /// the command of the same act runs, as its key would have run it.
    pub(super) fn camera_pressed(&mut self, id: i32) {
        let command = match id {
            ZOOM_IN => Command::Zoom(true),
            ZOOM_OUT => Command::Zoom(false),
            ROTATE_LEFT => Command::Rotate(false),
            ROTATE_RIGHT => Command::Rotate(true),
            // Either button of the pair pauses: the one that shows tells which way it goes.
            PAUSE | PLAY => Command::Pause,
            _ => return,
        };
        self.applied.push(command);
        self.apply(command);
    }

    /// The pause button shows while the game runs and the play button while it is paused, one of
    /// them always hidden. A game that opens, that is paused by a key or by a scenario, and that is
    /// unpaused again all keep the pair in line; a change makes the interface dirty.
    pub(super) fn camera_sync(&mut self) {
        if let Some(ui) = &mut self.ui {
            let paused = self.game.as_ref().is_some_and(|g| g.paused);
            ui.set_hidden(PAUSE, paused);
            ui.set_hidden(PLAY, !paused);
        }
    }
}
