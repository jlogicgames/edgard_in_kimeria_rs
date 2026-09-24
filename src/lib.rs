//! Edgard in Kimeria — a Bevy game.
//!
//! # Coordinate convention
//!
//! Gameplay runs in **Tiled space**: origin at the map's top-left, `+y` pointing
//! *down*, entity positions anchored at their top-left corner. Keeping this
//! convention throughout the physics avoids a sign-flipped rewrite that would
//! have to be re-derived (and re-debugged) from scratch.
//!
//! Bevy renders y-up, so [`core::GamePos`] is the source of truth and
//! [`core::sync_transforms`] projects it onto [`Transform`] once per frame:
//! `translation = (pos.x, -pos.y, z)`, with sprites anchored `TOP_LEFT`.
//! Nothing outside that one system should touch `Transform.translation` for
//! gameplay entities.

// Bevy system signatures are wide by construction: a system that needs six
// resources and a filtered query is idiomatic, not a smell. Both lints fire on
// almost every system here and neither points at anything worth changing.
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use bevy::prelude::*;

pub mod animation;
pub mod assets;
pub mod audio;
pub mod camera;
pub mod core;
pub mod dev;
pub mod effects;
pub mod enemy;
pub mod items;
pub mod level;
pub mod localization;
pub mod objects;
pub mod player;
pub mod ui;
#[cfg(not(target_arch = "wasm32"))]
pub mod window_prefs;

/// Logical resolution the game is authored against; the window letterboxes to it.
pub const LOGICAL_RESOLUTION: Vec2 = Vec2::new(640.0, 360.0);

/// Top-level app state.
///
/// Menu entities are tagged `DespawnOnExit(state)`, so leaving a state cleans up
/// its UI automatically.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppState {
    /// Web only: a single-button screen standing between app boot and asset
    /// loading. Browsers block audio until a user gesture, and `AudioPlugin`
    /// otherwise has no guarantee one happened before it tries to play
    /// anything, so this state is the default on `wasm32` — the click that
    /// leaves it is that gesture. Native has no such restriction and starts
    /// straight in `Loading`; the enum's single `#[default]` swaps by target.
    #[cfg_attr(target_arch = "wasm32", default)]
    StartScreen,
    /// Waits for every sprite sheet and sound before anything can spawn.
    #[cfg_attr(not(target_arch = "wasm32"), default)]
    Loading,
    MainMenu,
    About,
    Options,
    Playing,
    Paused,
    GameOver,
}

/// Runtime toggles for the game.
#[derive(Resource, Debug, Clone)]
pub struct GameSettings {
    pub play_sounds: bool,
    pub sound_volume: f32,
    /// Draw hitbox/collision-block gizmos.
    pub debug_draw: bool,
    /// Enables the chromatic-aberration glitch. Off by default.
    pub chroma_glitch: bool,
    /// Debug aid: ignores lethal damage so a level can be walked end to end.
    /// Toggled with F2.
    pub invulnerable: bool,
    /// UI display language, changed from the main menu's Options page.
    pub language: crate::localization::Language,
    /// Native only: whether the player opted out of the fullscreen default
    /// (jlogicgames/edgard_in_kimeria_rs#22). Unused on web, which always
    /// runs windowed regardless of this field.
    pub windowed: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            play_sounds: true,
            sound_volume: 1.0,
            debug_draw: false,
            chroma_glitch: false,
            invulnerable: false,
            language: crate::localization::Language::default(),
            windowed: false,
        }
    }
}

/// Score and progression carried across level loads.
#[derive(Resource, Debug, Default)]
pub struct GameProgress {
    pub coins_collected: u32,
    pub current_level: usize,
}

/// The level rotation, in play order.
pub const LEVEL_NAMES: [&str; 2] = ["forest-1", "forest"];

/// Everything the game needs, as one plugin so `main.rs` stays a launcher.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
        let mut settings = GameSettings::default();
        #[cfg(not(target_arch = "wasm32"))]
        {
            settings.windowed = crate::window_prefs::load_windowed();
        }
        app.init_state::<AppState>()
            .insert_resource(settings)
            .init_resource::<GameProgress>()
            .add_plugins((
                assets::AssetsPlugin,
                core::CorePlugin,
                animation::AnimationPlugin,
                camera::CameraPlugin,
                level::LevelPlugin,
                player::PlayerPlugin,
                enemy::EnemyPlugin,
                items::ItemsPlugin,
                objects::ObjectsPlugin,
                effects::EffectsPlugin,
                ui::UiPlugin,
                audio::AudioPlugin,
                dev::DevPlugin,
            ));
    }
}
