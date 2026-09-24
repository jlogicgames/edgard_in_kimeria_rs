use bevy::prelude::*;
use bevy::window::WindowResolution;

use edgard_in_kimeria::{GamePlugin, LOGICAL_RESOLUTION};

/// Desktop and mobile builds start fullscreen by default
/// (jlogicgames/edgard_in_kimeria_rs#22); the Options menu can switch to
/// windowed, and that choice is what `window_prefs` persists. Web is out of
/// scope for this and always starts windowed — it has its own fullscreen
/// constraints (the browser, not the app, owns that decision).
#[cfg(not(target_arch = "wasm32"))]
fn initial_window_mode() -> bevy::window::WindowMode {
    if edgard_in_kimeria::window_prefs::load_windowed() {
        bevy::window::WindowMode::Windowed
    } else {
        bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current)
    }
}
#[cfg(target_arch = "wasm32")]
fn initial_window_mode() -> bevy::window::WindowMode {
    bevy::window::WindowMode::Windowed
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                // Pixel art: never filter the tileset or sprite sheets.
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Edgard in Kimeria".to_string(),
                        resolution: WindowResolution::new(
                            LOGICAL_RESOLUTION.x as u32 * 2,
                            LOGICAL_RESOLUTION.y as u32 * 2,
                        ),
                        mode: initial_window_mode(),
                        // Ignored on native (bevy_winit only reads it under
                        // `cfg(target_arch = "wasm32")`). On web, without it
                        // the canvas stays a fixed 1280x720 regardless of the
                        // browser window, so on any viewport shorter than
                        // that — i.e. almost any real browser window, once
                        // its chrome is subtracted — the bottom of the UI
                        // (menu buttons, panel content) is clipped with no
                        // way to scroll to it. This resizes the canvas to
                        // its parent element (`<body>`, which our index.html
                        // sizes to the viewport), which in turn resizes the
                        // window/camera so both the letterboxed game view and
                        // the menus lay out to whatever size the browser
                        // actually gives them.
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(GamePlugin)
        .run();
}
