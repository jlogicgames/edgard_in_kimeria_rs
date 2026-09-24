//! UI message catalog and the runtime-selectable [`Language`].
//!
//! Implements jlogicgames/edgard_in_kimeria_rs#7 (add Ukrainian language
//! support). Every user-facing string lives here exactly once per language,
//! looked up by [`Msg`] key, instead of scattered as literals through the
//! menu-spawning code — adding a language means filling in one more match
//! arm per key, not hunting through `ui.rs`.

/// The game's selectable display language. Stored on
/// [`crate::GameSettings`] and changed from the main menu's Options page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    English,
    Ukrainian,
}

impl Language {
    /// The language's own name, in its own language — shown as the label of
    /// the button that selects it, so it reads correctly even to a player
    /// who can't read the language currently active.
    pub fn native_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Ukrainian => "Українська",
        }
    }
}

/// A localizable piece of UI text. One variant per distinct string; look up
/// the copy for the active language with [`Msg::t`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Msg {
    Title,
    Start,
    Play,
    About,
    Options,
    Exit,
    Back,
    Resume,
    ExitToMenu,
    PlayAgain,
    PauseMenu,
    GameOver,
    LanguageLabel,
    ControlsHelp,
    MenuHint,
    AboutBody,
}

impl Msg {
    /// This message's text in `lang`.
    pub fn t(self, lang: Language) -> &'static str {
        use Language::{English, Ukrainian};
        use Msg::*;
        match (self, lang) {
            (Title, English) => "Edgard in Kimeria",
            (Title, Ukrainian) => "Едгард у Кімерії",

            (Start, English) => "Start",
            (Start, Ukrainian) => "Почати",

            (Play, English) => "Play",
            (Play, Ukrainian) => "Грати",

            (About, English) => "About",
            (About, Ukrainian) => "Про гру",

            (Options, English) => "Options",
            (Options, Ukrainian) => "Налаштування",

            (Exit, English) => "Exit",
            (Exit, Ukrainian) => "Вихід",

            (Back, English) => "Back",
            (Back, Ukrainian) => "Назад",

            (Resume, English) => "Resume",
            (Resume, Ukrainian) => "Продовжити",

            (ExitToMenu, English) => "Exit to Menu",
            (ExitToMenu, Ukrainian) => "Вийти в меню",

            (PlayAgain, English) => "Play Again",
            (PlayAgain, Ukrainian) => "Грати знову",

            (PauseMenu, English) => "Pause Menu",
            (PauseMenu, Ukrainian) => "Меню паузи",

            (GameOver, English) => "Game Over",
            (GameOver, Ukrainian) => "Гру закінчено",

            (LanguageLabel, English) => "Language",
            (LanguageLabel, Ukrainian) => "Мова",

            (ControlsHelp, English) => {
                "Use WASD or Arrow Keys for movement.\n\
                J/Z to jump. K/X to attack. L/C to interact.\n\
                Collect as many stars as you can and avoid enemies!"
            }
            (ControlsHelp, Ukrainian) => {
                "Використовуйте WASD або стрілки для руху.\n\
                J/Z — стрибок. K/X — атака. L/C — взаємодія.\n\
                Зберіть якомога більше зірок і уникайте ворогів!"
            }

            (MenuHint, English) => {
                "Arrows/Tab to move - Enter/Space/A to confirm - Esc/B to go back"
            }
            (MenuHint, Ukrainian) => {
                "Стрілки/Tab — рух - Enter/Пробіл/A — підтвердити - Esc/B — назад"
            }

            (AboutBody, English) => {
                "Edgard in Kimeria\n\n\
                Use WASD or Arrow Keys for movement.\n\
                J/Z to jump. K/X to attack. L/C to interact.\n\
                Escape to pause.\n\
                Collect as many stars as you can and avoid enemies!"
            }
            (AboutBody, Ukrainian) => {
                "Едгард у Кімерії\n\n\
                Використовуйте WASD або стрілки для руху.\n\
                J/Z — стрибок. K/X — атака. L/C — взаємодія.\n\
                Escape — пауза.\n\
                Зберіть якомога більше зірок і уникайте ворогів!"
            }
        }
    }
}
