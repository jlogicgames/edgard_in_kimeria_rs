//! Persists the player's windowed/fullscreen choice across sessions.
//!
//! Native only (desktop and, if a mobile target is ever added, mobile too):
//! implements jlogicgames/edgard_in_kimeria_rs#22, which starts these builds
//! fullscreen by default with the Options menu offering windowed as an
//! opt-out. The web build has its own fullscreen constraints and is out of
//! scope, so this module isn't compiled for `wasm32`.
//!
//! No settings dependency exists yet in this crate (see `GameSettings`,
//! which is in-memory only), so rather than pull in a key-value store for
//! one bool, this writes a one-word file under the OS's standard config
//! directory.

use std::fs;
use std::path::PathBuf;

fn config_path() -> Option<PathBuf> {
    let dir = if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    }?;
    Some(dir.join("edgard_in_kimeria").join("window_mode.txt"))
}

/// Whether the player last chose windowed mode. Defaults to `false`
/// (fullscreen) when there's no saved preference yet, or no writable config
/// directory could be found.
pub fn load_windowed() -> bool {
    config_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .is_some_and(|contents| contents.trim() == "windowed")
}

/// Best-effort: a player toggling a display setting shouldn't be blocked or
/// warned by a read-only config directory, so failures are silently ignored.
pub fn save_windowed(windowed: bool) {
    let Some(path) = config_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, if windowed { "windowed" } else { "fullscreen" });
}
