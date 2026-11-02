package org.rustyzoo;

import org.libsdl.app.SDLActivity;

/**
 * The RustyZoo activity.
 *
 * SDL3 is linked statically into `librustyzoo.so` (the Rust build uses `build-from-source-static`),
 * so there is no separate `libSDL3.so` to load: this overrides the library list to load only the
 * game. The entry point stays `SDL_main`, which `android/src/main.rs` exports.
 */
public class RustyZooActivity extends SDLActivity {
    @Override
    protected String[] getLibraries() {
        return new String[] { "rustyzoo" };
    }
}
