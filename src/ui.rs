//! HUD and menus.
//!
//! Built with Bevy's own `bevy_ui`: the menus are four small trees of nodes,
//! nothing here wants immediate-mode or a docking/inspector toolkit, and staying
//! native means one render path and no extra dependency. Each menu is tagged
//! `DespawnOnExit(state)`, so the state machine cleans it up automatically,
//! rather than requiring a manually paired add/remove in a different file.

use bevy::color::Mix;
use bevy::prelude::*;
use rand::Rng;

use crate::assets::GameAssets;
use crate::audio::PlaySound;
use crate::camera::MainCamera;
use crate::effects::{Firefly, FogEffect, MenuFirefly};
use crate::level::{LevelEntity, LevelMap, LoadLevel};
use crate::localization::{Language, Msg};
use crate::{AppState, GameProgress, GameSettings, LOGICAL_RESOLUTION};

const PANEL_BG: Color = Color::srgb(0.0, 0.0, 0.0);
/// The main menu leaves its fog backdrop visible, so its panel is a tint
/// rather than the other menus' solid card.
const MENU_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.35);
const TEXT_COLOR: Color = Color::WHITE;
const BUTTON_BG: Color = Color::WHITE;
const BUTTON_BG_HOVER: Color = Color::srgb(0.85, 0.85, 0.85);
const BUTTON_TEXT: Color = Color::BLACK;
/// How quickly hover/focus/press feedback eases toward its target each
/// second; the exact curve of `1 - exp(-EASE_RATE * dt)` doesn't matter, only
/// that it's fast enough to feel responsive but not instant.
const EASE_RATE: f32 = 14.0;
/// Seconds between one menu element appearing and the next, so a menu's
/// heading, then its buttons, rise in one after another instead of all at
/// once.
const APPEAR_STAGGER: f32 = 0.08;
/// How long each element's own fade/rise takes once its turn comes.
const APPEAR_DURATION: f32 = 0.35;
/// Vertical distance an element slides up over `APPEAR_DURATION`.
const APPEAR_RISE_PX: f32 = 14.0;
/// Fraction of scale the main menu title pulses by — subtle, just enough to
/// read as "alive" rather than a static label.
const BREATHE_AMPLITUDE: f32 = 0.035;
/// Radians per second of the title's breathing sine wave.
const BREATHE_SPEED: f32 = 2.0;

/// Which menu button an entity is, so one handler can serve every menu.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    /// Only spawned on the web start screen (`AppState::StartScreen`);
    /// native starts directly in `AppState::Loading` and never spawns the
    /// button that constructs this.
    Start,
    Play,
    About,
    Options,
    /// Never constructed on wasm32 — no button spawns it there, see
    /// `spawn_main_menu_buttons`.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Exit,
    Back,
    Resume,
    ExitToMenu,
    PlayAgain,
    SetLanguage(Language),
}

#[derive(Component)]
struct CoinCounter;

/// Tags the HUD root (coin icon + counter), which survives a pause on
/// purpose and so isn't `DespawnOnExit`-scoped like the menus — see
/// `despawn_hud_for_menu`.
#[derive(Component)]
struct Hud;

/// Tags the fog backdrop shared by the main menu and its About screen, so one
/// system can keep exactly one alive across both states.
#[derive(Component)]
struct MenuFog;

/// Keyboard/gamepad-navigable focus on a menu button, independent of mouse
/// hover — highlighted with [`FOCUS_RING`] so it's visible without a cursor.
#[derive(Component)]
struct Focused;

/// A button's position in its menu's up/down navigation order.
#[derive(Component)]
struct MenuButtonIndex(u32);

/// Eased scale for the hover/focus/press "punch", so size and colour glide
/// toward their target instead of snapping every frame.
#[derive(Component)]
struct ButtonAnim {
    scale: f32,
}

impl Default for ButtonAnim {
    fn default() -> Self {
        Self { scale: 1.0 }
    }
}

/// A menu element's entrance: fades and slides up into place `delay` seconds
/// after `spawn_time`, so a whole menu doesn't pop in on a single frame.
#[derive(Component, Clone, Copy)]
struct AppearAnim {
    spawn_time: f32,
    delay: f32,
}

impl AppearAnim {
    /// `order` is the element's position within its menu (0 first), which
    /// both staggers the entrance and gives keyboard/gamepad nav its order.
    fn new(order: u32, now: f32) -> Self {
        Self {
            spawn_time: now,
            delay: order as f32 * APPEAR_STAGGER,
        }
    }

    /// Eased 0..1 progress through the entrance, given the current time.
    fn progress(&self, now: f32) -> f32 {
        let t = ((now - self.spawn_time - self.delay) / APPEAR_DURATION).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t) // smoothstep: eases out instead of stopping abruptly
    }
}

/// Marks the main menu's title for a continuous idle pulse, on top of (and
/// independent from) its one-shot entrance in [`AppearAnim`].
#[derive(Component)]
struct BreathingTitle;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::StartScreen), spawn_start_screen)
            .add_systems(
                OnEnter(AppState::MainMenu),
                (reset_camera_for_menu, despawn_hud_for_menu, spawn_main_menu).chain(),
            )
            .add_systems(
                OnEnter(AppState::About),
                (
                    reset_camera_for_menu,
                    despawn_hud_for_menu,
                    spawn_about_menu,
                )
                    .chain(),
            )
            .add_systems(
                OnEnter(AppState::Options),
                (
                    reset_camera_for_menu,
                    despawn_hud_for_menu,
                    spawn_options_menu,
                )
                    .chain(),
            )
            .add_systems(OnEnter(AppState::Paused), spawn_pause_menu)
            .add_systems(OnEnter(AppState::GameOver), spawn_game_over)
            .add_systems(OnEnter(AppState::Playing), spawn_hud)
            .add_systems(
                Update,
                (
                    focus_on_hover,
                    handle_menu_navigation,
                    ensure_default_focus,
                    activate_focused_button,
                    handle_buttons,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    update_coin_counter,
                    resume_on_escape,
                    sync_menu_fog,
                    sync_menu_fireflies,
                    animate_buttons,
                    play_hover_sound,
                    advance_appear,
                    animate_breathing_title,
                ),
            );
    }
}

// --- shared building blocks -------------------------------------------------

fn panel(full_screen: bool) -> impl Bundle {
    panel_with_bg(full_screen, PANEL_BG)
}

fn panel_with_bg(full_screen: bool, bg: Color) -> impl Bundle {
    let (width, height) = if full_screen {
        (Val::Percent(100.0), Val::Percent(100.0))
    } else {
        (Val::Px(400.0), Val::Px(300.0))
    };
    panel_sized(width, height, bg)
}

fn panel_sized(width: Val, height: Val, bg: Color) -> impl Bundle {
    (
        Node {
            width,
            height,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::all(Val::Px(10.0)),
            row_gap: Val::Px(20.0),
            border_radius: BorderRadius::all(Val::Px(20.0)),
            ..default()
        },
        BackgroundColor(bg),
    )
}

/// Full-viewport centring wrapper, so a menu sits in the middle of the window.
fn overlay_root(name: &'static str) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        Name::new(name),
    )
}

/// `index` is the button's position in its menu for up/down keyboard
/// navigation — 0 for the first button, counting up from there. `appear` is
/// its entrance timing; shared with the label's own `AppearAnim` below so
/// the background and its text fade in together.
fn button(
    action: MenuAction,
    label: &str,
    font_size: f32,
    index: u32,
    font: Handle<Font>,
    appear: AppearAnim,
) -> impl Bundle {
    // Wide enough for "Exit to Menu", the longest label, so no button wraps
    // its text onto a second, left-aligned line.
    (
        Button,
        action,
        MenuButtonIndex(index),
        ButtonAnim::default(),
        appear,
        Node {
            width: Val::Px(220.0),
            height: Val::Px(75.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::horizontal(Val::Px(8.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(BUTTON_BG),
        children![(
            Text::new(label.to_string()),
            TextFont::from_font_size(font_size).with_font(font),
            TextColor(BUTTON_TEXT),
            TextLayout::justify(Justify::Center),
            appear,
        )],
    )
}

fn heading(text: &str, font_size: f32, font: Handle<Font>, appear: AppearAnim) -> impl Bundle {
    (
        Text::new(text.to_string()),
        TextFont::from_font_size(font_size).with_font(font),
        TextColor(TEXT_COLOR),
        appear,
    )
}

fn help_text(font: Handle<Font>, appear: AppearAnim, lang: Language) -> impl Bundle {
    (
        Text::new(Msg::ControlsHelp.t(lang)),
        TextFont::from_font_size(14.0).with_font(font),
        TextColor(TEXT_COLOR),
        TextLayout::justify(Justify::Center),
        appear,
    )
}

fn menu_hint(font: Handle<Font>, appear: AppearAnim, lang: Language) -> impl Bundle {
    (
        Text::new(Msg::MenuHint.t(lang)),
        TextFont::from_font_size(14.0).with_font(font),
        TextColor(TEXT_COLOR),
        TextLayout::justify(Justify::Center),
        appear,
    )
}

// --- menus ------------------------------------------------------------------

/// Keeps exactly one [`MenuFog`] entity alive while the main menu or About
/// screen is up, and despawns it once neither is — the forest fog shader
/// drawn behind the panel, at the camera's fog layer, in place of the sky.
fn sync_menu_fog(
    mut commands: Commands,
    state: Res<State<AppState>>,
    existing: Query<Entity, With<MenuFog>>,
) {
    let want_fog = matches!(
        state.get(),
        AppState::MainMenu | AppState::About | AppState::Options
    );
    match (want_fog, existing.iter().next()) {
        (true, None) => {
            commands.spawn((FogEffect::default(), MenuFog, Name::new("MenuFog")));
        }
        (false, Some(entity)) => {
            commands.entity(entity).try_despawn();
        }
        _ => {}
    }
}

/// Fireflies are positioned in world space, not parented to the camera like
/// the fog, so the menu needs the camera sitting at a known spot for
/// [`LOGICAL_RESOLUTION`]-sized firefly coordinates to land on screen.
/// Harmless to reset: nothing else is visible in world space on a menu
/// screen, and `Playing` re-snaps the camera to the player regardless.
fn reset_camera_for_menu(mut camera: Query<&mut Transform, With<MainCamera>>) {
    if let Ok(mut transform) = camera.single_mut() {
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
    }
}

/// `spawn_hud` only ever adds the HUD, never removes it — deliberately, so
/// it survives a pause — which otherwise left it on screen forever, coins
/// and all, once a run had started even after backing out to the menu. This
/// is the actual removal, run on the way back to either menu screen.
fn despawn_hud_for_menu(mut commands: Commands, hud: Query<Entity, With<Hud>>) {
    for entity in &hud {
        commands.entity(entity).try_despawn();
    }
}

const MENU_FIREFLY_COUNT: u32 = 24;

/// Mirrors [`sync_menu_fog`] for the ambient fireflies: keeps a fixed batch
/// alive behind the main menu/About screen and despawns every entity tagged
/// [`MenuFirefly`] — both the emitters and any mid-flight particle — once
/// neither screen is up.
fn sync_menu_fireflies(
    mut commands: Commands,
    state: Res<State<AppState>>,
    existing: Query<Entity, With<MenuFirefly>>,
) {
    let want = matches!(
        state.get(),
        AppState::MainMenu | AppState::About | AppState::Options
    );
    if want {
        if existing.iter().next().is_none() {
            let mut rng = rand::rng();
            for _ in 0..MENU_FIREFLY_COUNT {
                commands.spawn((
                    // The camera sits at `GamePos::ZERO` for the menu (see
                    // `reset_camera_for_menu`), centred in the viewport —
                    // shift the wander rectangle's corner to match, or it
                    // would only ever cover one screen quadrant.
                    Firefly::new(LOGICAL_RESOLUTION, rng.random::<f32>() * 2.0)
                        .with_origin(-LOGICAL_RESOLUTION / 2.0),
                    MenuFirefly,
                    Name::new("MenuFireflyEmitter"),
                ));
            }
        }
    } else {
        for entity in &existing {
            commands.entity(entity).try_despawn();
        }
    }
}

/// The temporary camera the web start screen renders to. `spawn_camera` (the
/// real, `MainCamera`-tagged one, in `camera.rs`) needs `GameAssets` for its
/// backdrop sprite and doesn't spawn until `OnExit(AppState::Loading)`, which
/// is still ahead of this screen — so it gets its own bare camera here,
/// `DespawnOnExit`-cleaned the moment the click sends the state past it.
#[derive(Component)]
struct StartScreenCamera;

/// Web only in practice: `AppState::StartScreen` is the default state on
/// `wasm32` and otherwise unreachable (see `AppState`). One button stands
/// between app boot and `AppState::Loading`; clicking it is the user gesture
/// browsers require before audio can play, satisfied before `AudioPlugin`
/// or asset loading ever starts.
fn spawn_start_screen(mut commands: Commands, settings: Res<GameSettings>, time: Res<Time<Real>>) {
    let now = time.elapsed_secs();
    let lang = settings.language;
    commands.spawn((
        Camera2d,
        StartScreenCamera,
        DespawnOnExit(AppState::StartScreen),
        Name::new("StartScreenCamera"),
    ));
    commands.spawn((
        overlay_root("StartScreen"),
        DespawnOnExit(AppState::StartScreen),
        BackgroundColor(Color::BLACK),
        children![button(
            MenuAction::Start,
            Msg::Start.t(lang),
            40.0,
            0,
            Handle::<Font>::default(),
            AppearAnim::new(0, now),
        )],
    ));
}

fn spawn_main_menu(
    mut commands: Commands,
    assets: Res<GameAssets>,
    settings: Res<GameSettings>,
    time: Res<Time<Real>>,
) {
    let now = time.elapsed_secs();
    let lang = settings.language;
    commands.spawn((
        overlay_root("MainMenu"),
        DespawnOnExit(AppState::MainMenu),
        children![(
            panel_with_bg(true, MENU_BG),
            // Not the `children!` macro: the button row's length depends on
            // the target (native has an Exit button, web doesn't — see
            // `spawn_main_menu_buttons`), so it needs ordinary control flow
            // rather than a fixed list of expressions. `SpawnWith` is the
            // `SpawnableList` for exactly that: an `FnOnce` given a spawner
            // to call `.spawn()` on directly, mixed here with the
            // `Spawn`-wrapped single bundles either side of it.
            Children::spawn((
                Spawn((
                    Text::new(Msg::Title.t(lang)),
                    // QuestSquare reads small at its nominal size (see the
                    // About screen's note on this); the game's own title is
                    // the most prominent text on screen, so it gets by far
                    // the biggest number of all of them. Sized as a fraction
                    // of viewport width, not a fixed pixel count — a fixed
                    // 150px doesn't fit an 18-character string on a narrow
                    // (e.g. portrait phone) window and wraps across three
                    // lines instead of shrinking to stay on one.
                    TextFont::from_font_size(FontSize::Vw(11.0))
                        .with_font(assets.font_text.clone()),
                    TextColor(TEXT_COLOR),
                    AppearAnim::new(0, now),
                    BreathingTitle,
                )),
                {
                    let font_button = assets.font_button.clone();
                    SpawnWith(move |parent: &mut ChildSpawner| {
                        spawn_main_menu_buttons(parent, font_button, lang, now);
                    })
                },
                Spawn(menu_hint(
                    assets.font_text.clone(),
                    AppearAnim::new(MAIN_MENU_BUTTON_COUNT + 1, now),
                    lang,
                )),
            )),
        )],
    ));
}

/// Native only: quitting a browser tab from inside the page isn't something
/// `AppExit` can do, and closing it out from under the player without so much
/// as a confirmation is worse than not offering the button — so the web
/// build simply doesn't spawn it. `MenuAction::Exit` and its handler stay for
/// native.
#[cfg(not(target_arch = "wasm32"))]
const MAIN_MENU_BUTTON_COUNT: u32 = 4;
#[cfg(target_arch = "wasm32")]
const MAIN_MENU_BUTTON_COUNT: u32 = 3;

fn spawn_main_menu_buttons(
    parent: &mut ChildSpawner,
    font_button: Handle<Font>,
    lang: Language,
    now: f32,
) {
    parent.spawn(button(
        MenuAction::Play,
        Msg::Play.t(lang),
        40.0,
        0,
        font_button.clone(),
        AppearAnim::new(1, now),
    ));
    parent.spawn(button(
        MenuAction::About,
        Msg::About.t(lang),
        28.0,
        1,
        font_button.clone(),
        AppearAnim::new(2, now),
    ));
    parent.spawn(button(
        MenuAction::Options,
        Msg::Options.t(lang),
        28.0,
        2,
        font_button.clone(),
        AppearAnim::new(3, now),
    ));
    #[cfg(not(target_arch = "wasm32"))]
    parent.spawn(button(
        MenuAction::Exit,
        Msg::Exit.t(lang),
        28.0,
        3,
        font_button,
        AppearAnim::new(4, now),
    ));
}

fn spawn_about_menu(
    mut commands: Commands,
    assets: Res<GameAssets>,
    settings: Res<GameSettings>,
    time: Res<Time<Real>>,
) {
    let now = time.elapsed_secs();
    let lang = settings.language;
    commands.spawn((
        overlay_root("About"),
        DespawnOnExit(AppState::About),
        children![(
            // Bigger than the other menus' fixed 400x300: at that size the
            // body text was lost in mostly-empty black space.
            panel_sized(Val::Px(580.0), Val::Px(460.0), PANEL_BG),
            children![
                heading(
                    Msg::About.t(lang),
                    42.0,
                    assets.font_text.clone(),
                    AppearAnim::new(0, now)
                ),
                (
                    Text::new(Msg::AboutBody.t(lang)),
                    // QuestSquare's glyphs sit small in their em-box — at the
                    // same nominal size it reads noticeably smaller than
                    // NanoPlus on the buttons, so it needs a bigger number to
                    // match.
                    TextFont::from_font_size(30.0).with_font(assets.font_text.clone()),
                    TextColor(TEXT_COLOR),
                    TextLayout::justify(Justify::Center),
                    AppearAnim::new(1, now),
                ),
                button(
                    MenuAction::Back,
                    Msg::Back.t(lang),
                    28.0,
                    0,
                    assets.font_button.clone(),
                    AppearAnim::new(2, now),
                ),
            ],
        )],
    ));
}

/// The main menu's language page: one button per [`Language`], each of which
/// sets [`GameSettings::language`] and returns to the main menu — which then
/// re-spawns (via `OnEnter(AppState::MainMenu)`) with every label in the
/// newly chosen language.
fn spawn_options_menu(
    mut commands: Commands,
    assets: Res<GameAssets>,
    settings: Res<GameSettings>,
    time: Res<Time<Real>>,
) {
    let now = time.elapsed_secs();
    let lang = settings.language;
    let english_label = if lang == Language::English {
        format!("> {}", Language::English.native_name())
    } else {
        Language::English.native_name().to_string()
    };
    let ukrainian_label = if lang == Language::Ukrainian {
        format!("> {}", Language::Ukrainian.native_name())
    } else {
        Language::Ukrainian.native_name().to_string()
    };
    commands.spawn((
        overlay_root("Options"),
        DespawnOnExit(AppState::Options),
        children![(
            panel(false),
            children![
                heading(
                    Msg::Options.t(lang),
                    24.0,
                    assets.font_text.clone(),
                    AppearAnim::new(0, now)
                ),
                heading(
                    Msg::LanguageLabel.t(lang),
                    16.0,
                    assets.font_text.clone(),
                    AppearAnim::new(1, now)
                ),
                button(
                    MenuAction::SetLanguage(Language::English),
                    &english_label,
                    24.0,
                    0,
                    assets.font_button.clone(),
                    AppearAnim::new(2, now),
                ),
                button(
                    MenuAction::SetLanguage(Language::Ukrainian),
                    &ukrainian_label,
                    24.0,
                    1,
                    assets.font_button.clone(),
                    AppearAnim::new(3, now),
                ),
                button(
                    MenuAction::Back,
                    Msg::Back.t(lang),
                    28.0,
                    2,
                    assets.font_button.clone(),
                    AppearAnim::new(4, now),
                ),
                menu_hint(assets.font_text.clone(), AppearAnim::new(5, now), lang),
            ],
        )],
    ));
}

fn spawn_pause_menu(
    mut commands: Commands,
    assets: Res<GameAssets>,
    settings: Res<GameSettings>,
    time: Res<Time<Real>>,
) {
    let now = time.elapsed_secs();
    let lang = settings.language;
    commands.spawn((
        overlay_root("PauseMenu"),
        DespawnOnExit(AppState::Paused),
        children![(
            panel(false),
            children![
                heading(
                    Msg::PauseMenu.t(lang),
                    24.0,
                    assets.font_text.clone(),
                    AppearAnim::new(0, now)
                ),
                button(
                    MenuAction::Resume,
                    Msg::Resume.t(lang),
                    28.0,
                    0,
                    assets.font_button.clone(),
                    AppearAnim::new(1, now),
                ),
                button(
                    MenuAction::ExitToMenu,
                    Msg::ExitToMenu.t(lang),
                    28.0,
                    1,
                    assets.font_button.clone(),
                    AppearAnim::new(2, now),
                ),
                menu_hint(assets.font_text.clone(), AppearAnim::new(3, now), lang),
                help_text(assets.font_text.clone(), AppearAnim::new(4, now), lang),
            ],
        )],
    ));
}

fn spawn_game_over(
    mut commands: Commands,
    assets: Res<GameAssets>,
    settings: Res<GameSettings>,
    time: Res<Time<Real>>,
) {
    let now = time.elapsed_secs();
    let lang = settings.language;
    commands.spawn((
        overlay_root("GameOver"),
        DespawnOnExit(AppState::GameOver),
        children![(
            panel(false),
            children![
                heading(
                    Msg::GameOver.t(lang),
                    24.0,
                    assets.font_text.clone(),
                    AppearAnim::new(0, now)
                ),
                button(
                    MenuAction::PlayAgain,
                    Msg::PlayAgain.t(lang),
                    28.0,
                    0,
                    assets.font_button.clone(),
                    AppearAnim::new(1, now),
                ),
                menu_hint(assets.font_text.clone(), AppearAnim::new(2, now), lang),
            ],
        )],
    ));
}

/// The coin readout. Unlike the menus this must survive a pause, so it isn't
/// `DespawnOnExit`-scoped to any single state; `despawn_hud_for_menu`
/// removes it explicitly once back at the main menu/About instead.
fn spawn_hud(
    mut commands: Commands,
    assets: Res<GameAssets>,
    existing: Query<Entity, With<CoinCounter>>,
) {
    if !existing.is_empty() {
        return;
    }
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(10.0),
            top: Val::Px(10.0),
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        },
        Hud,
        Name::new("Hud"),
        children![
            (
                ImageNode {
                    image: assets.image("images/Items.png"),
                    rect: Some(Rect::from_corners(Vec2::ZERO, Vec2::splat(16.0))),
                    ..default()
                },
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(32.0),
                    ..default()
                },
            ),
            (
                CoinCounter,
                Text::new("0"),
                TextFont::from_font_size(24.0).with_font(assets.font_text.clone()),
                TextColor(TEXT_COLOR),
            ),
        ],
    ));
}

fn update_coin_counter(
    progress: Res<GameProgress>,
    mut query: Query<&mut Text, With<CoinCounter>>,
) {
    if !progress.is_changed() {
        return;
    }
    for mut text in &mut query {
        **text = progress.coins_collected.to_string();
    }
}

/// Mouse hover claims keyboard/gamepad focus too, so Enter/gamepad-A always
/// activates whichever button the pointer last landed on, exactly like a
/// click would — hover and focus are one and the same, not two systems that
/// happen to look alike.
fn focus_on_hover(
    mut commands: Commands,
    entered_hover: Query<(Entity, &Interaction), (With<MenuButtonIndex>, Changed<Interaction>)>,
    focused: Query<Entity, With<Focused>>,
) {
    for (entity, interaction) in &entered_hover {
        if *interaction != Interaction::Hovered {
            continue;
        }
        for old in &focused {
            if old != entity {
                commands.entity(old).remove::<Focused>();
            }
        }
        commands.entity(entity).insert(Focused);
    }
}

/// Moves [`Focused`] between a menu's buttons with the arrow keys,
/// Tab/Shift+Tab, or a gamepad's D-pad, so a screen can be worked without a
/// mouse.
fn handle_menu_navigation(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    buttons: Query<(Entity, &MenuButtonIndex, Has<Focused>)>,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let gamepad_down = gamepads
        .iter()
        .any(|g| g.just_pressed(GamepadButton::DPadDown));
    let gamepad_up = gamepads
        .iter()
        .any(|g| g.just_pressed(GamepadButton::DPadUp));
    let down = keys.just_pressed(KeyCode::ArrowDown)
        || (keys.just_pressed(KeyCode::Tab) && !shift)
        || gamepad_down;
    let up = keys.just_pressed(KeyCode::ArrowUp)
        || (keys.just_pressed(KeyCode::Tab) && shift)
        || gamepad_up;
    if !down && !up {
        return;
    }

    let mut ordered: Vec<(Entity, u32, bool)> = buttons
        .iter()
        .map(|(e, index, has)| (e, index.0, has))
        .collect();
    if ordered.is_empty() {
        return;
    }
    ordered.sort_by_key(|(_, index, _)| *index);

    let current = ordered.iter().position(|(_, _, has)| *has).unwrap_or(0);
    let next = if down {
        (current + 1) % ordered.len()
    } else {
        (current + ordered.len() - 1) % ordered.len()
    };
    if current != next {
        commands.entity(ordered[current].0).remove::<Focused>();
        commands.entity(ordered[next].0).insert(Focused);
    }
}

/// Auto-focuses a menu's first button whenever none is focused, e.g. right
/// after the menu spawns.
fn ensure_default_focus(
    mut commands: Commands,
    buttons: Query<(Entity, &MenuButtonIndex), Without<Focused>>,
    focused: Query<Entity, With<Focused>>,
) {
    if !focused.is_empty() {
        return;
    }
    if let Some((entity, _)) = buttons.iter().min_by_key(|(_, index)| index.0) {
        commands.entity(entity).insert(Focused);
    }
}

/// Enter/Space/gamepad-South "clicks" the focused button — the
/// keyboard/gamepad equivalent of a mouse press, feeding the same
/// [`Interaction`] `handle_buttons` reads.
fn activate_focused_button(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut focused: Query<&mut Interaction, With<Focused>>,
) {
    let confirm = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::NumpadEnter)
        || keys.just_pressed(KeyCode::Space)
        || gamepads
            .iter()
            .any(|g| g.just_pressed(GamepadButton::South) || g.just_pressed(GamepadButton::Start));
    if !confirm {
        return;
    }
    for mut interaction in &mut focused {
        *interaction = Interaction::Pressed;
    }
}

/// Eases hover/focus/press feedback instead of snapping instantly: a gentle
/// scale "punch" plus background colour, driven by [`ButtonAnim`]. Also
/// folds in the button's own [`AppearAnim`] fade, so a still-entering button
/// doesn't flash to full opacity before its turn.
fn animate_buttons(
    time: Res<Time<Real>>,
    mut query: Query<(
        &Interaction,
        Has<Focused>,
        &mut BackgroundColor,
        &mut UiTransform,
        &mut ButtonAnim,
        Option<&AppearAnim>,
    )>,
) {
    let t = (time.delta_secs() * EASE_RATE).min(1.0);
    let now = time.elapsed_secs();
    for (interaction, focused, mut background, mut transform, mut anim, appear) in &mut query {
        let pressed = *interaction == Interaction::Pressed;
        let active = pressed || *interaction == Interaction::Hovered || focused;

        let target_scale = if pressed {
            0.94
        } else if active {
            1.08
        } else {
            1.0
        };
        anim.scale += (target_scale - anim.scale) * t;
        transform.scale = Vec2::splat(anim.scale);

        let target_bg = if active { BUTTON_BG_HOVER } else { BUTTON_BG };
        let mixed = background.0.mix(&target_bg, t);
        let appear_alpha = appear.map_or(1.0, |a| a.progress(now));
        background.0 = mixed.with_alpha(mixed.alpha() * appear_alpha);
    }
}

/// Advances every menu element's entrance: fades its text in and slides it
/// up into place. Buttons' own background/scale are handled separately in
/// `animate_buttons`, which reads the same [`AppearAnim`] for its alpha.
fn advance_appear(
    time: Res<Time<Real>>,
    mut query: Query<(&AppearAnim, &mut UiTransform, Option<&mut TextColor>)>,
) {
    let now = time.elapsed_secs();
    for (appear, mut transform, text_color) in &mut query {
        let progress = appear.progress(now);
        transform.translation = Val2::new(Val::ZERO, Val::Px((1.0 - progress) * APPEAR_RISE_PX));
        if let Some(mut color) = text_color {
            color.0 = color.0.with_alpha(progress);
        }
    }
}

/// A slow, subtle scale pulse on the main menu title, independent of its
/// one-shot entrance — just enough to read as "alive".
fn animate_breathing_title(
    time: Res<Time<Real>>,
    mut query: Query<&mut UiTransform, With<BreathingTitle>>,
) {
    let scale = 1.0 + BREATHE_AMPLITUDE * (time.elapsed_secs() * BREATHE_SPEED).sin();
    for mut transform in &mut query {
        transform.scale = Vec2::splat(scale);
    }
}

/// A quiet blip on hover or when keyboard/gamepad focus lands on a button —
/// distinct from `button_click.wav`'s louder press sound in `handle_buttons`,
/// so moving between buttons gives feedback even before one is pressed.
fn play_hover_sound(
    mut sounds: MessageWriter<PlaySound>,
    hovered: Query<&Interaction, Changed<Interaction>>,
    newly_focused: Query<Entity, Added<Focused>>,
) {
    let entered_hover = hovered.iter().any(|i| *i == Interaction::Hovered);
    if entered_hover || !newly_focused.is_empty() {
        sounds.write(PlaySound {
            name: "button_click.wav".to_string(),
            volume: 0.35,
        });
    }
}

fn handle_buttons(
    mut commands: Commands,
    interactions: Query<(&Interaction, &MenuAction), Changed<Interaction>>,
    level_entities: Query<Entity, Or<(With<LevelEntity>, With<LevelMap>)>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut loads: MessageWriter<LoadLevel>,
    mut exit: MessageWriter<AppExit>,
    mut sounds: MessageWriter<PlaySound>,
    mut progress: ResMut<GameProgress>,
    mut settings: ResMut<GameSettings>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        sounds.write(PlaySound::new("button_click.wav"));
        match action {
            MenuAction::Start => next_state.set(AppState::Loading),
            MenuAction::Play => {
                loads.write(LoadLevel(0));
                next_state.set(AppState::Playing);
            }
            MenuAction::About => next_state.set(AppState::About),
            MenuAction::Options => next_state.set(AppState::Options),
            MenuAction::SetLanguage(lang) => {
                settings.language = *lang;
                next_state.set(AppState::MainMenu);
            }
            MenuAction::Exit => {
                exit.write(AppExit::Success);
            }
            MenuAction::Back => next_state.set(AppState::MainMenu),
            MenuAction::Resume => next_state.set(AppState::Playing),
            MenuAction::ExitToMenu => {
                // Same cleanup `handle_load_level` does before a level swap,
                // but landing on the main menu instead of a map.
                for entity in &level_entities {
                    commands.entity(entity).try_despawn();
                }
                next_state.set(AppState::MainMenu);
            }
            MenuAction::PlayAgain => {
                // `game.reset()`: coins and level index both go back.
                progress.coins_collected = 0;
                loads.write(LoadLevel(0));
                next_state.set(AppState::Playing);
            }
        }
    }
}

/// Escape (or a gamepad's East/Select button) resumes as well as pauses,
/// matching `game.pause()`'s toggle.
fn resume_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let back = keys.just_pressed(KeyCode::Escape)
        || gamepads
            .iter()
            .any(|g| g.just_pressed(GamepadButton::East) || g.just_pressed(GamepadButton::Select));
    if *state.get() == AppState::Paused && back {
        next_state.set(AppState::Playing);
    }
}
