//! The app's menu in the macOS menu bar: Bench → Settings… (⌘,), New Window (⌘⇧N), Quit (⌘Q).
//! The gear and a new window are there, as in any Mac app, not icons in the window.

use gpui_kit::*;

use super::{BenchApp, Center};

actions!(bench, [OpenSettings, NewWindow, Quit]);

/// The menu bar and its keys, once at start.
pub fn install(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-,", OpenSettings, None),
        KeyBinding::new("cmd-shift-n", NewWindow, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.set_menus(vec![Menu::new("Bench").items([
        MenuItem::action("Settings…", OpenSettings),
        MenuItem::separator(),
        MenuItem::action("New Window", NewWindow),
        MenuItem::separator(),
        MenuItem::action("Quit Bench", Quit),
    ])]);
}

impl BenchApp {
    /// Bench → New Window: another window (`marks.rs`).
    pub fn on_new_window(&mut self, _: &NewWindow, _: &mut Window, cx: &mut Context<Self>) {
        self.open_window(None, cx);
    }

    /// Bench → Settings…: the gear's page in this window's center (again: back to what it was).
    pub fn on_open_settings(
        &mut self,
        _: &OpenSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.center == Center::Settings {
            self.center = Center::Chat;
            cx.notify();
        } else {
            self.open_center(Center::Settings, window, cx);
        }
    }
}
