//! The app's whole state and its actions. Views (`ui/`) read it and call these methods; I/O
//! goes in its own modules. Replace `Item` with what the app is about.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;
use serde::{Deserialize, Serialize};

use crate::state::state_dir;
use crate::theme::Appearance;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Item {
    pub title: String,
    pub body: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Main,
    Settings,
}

pub struct MainApp {
    pub appearance: Appearance,
    pub panel: Panel,
    pub items: Vec<Item>,
    pub selected: Option<usize>,
    pub search: Entity<InputState>,
    pub query: String,
    _subscriptions: Vec<Subscription>,
}

fn items_path() -> std::path::PathBuf {
    state_dir().join("items.json")
}

impl MainApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let subscriptions = vec![
            // "System" follows macOS light/dark live.
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.appearance.mode == crate::theme::Mode::System {
                    crate::theme::apply(this.appearance, Some(window), cx);
                }
            }),
            cx.subscribe_in(&search, window, |this, state, ev: &InputEvent, _, cx| {
                if let InputEvent::Change = ev {
                    this.query = state.read(cx).value().trim().to_lowercase();
                    cx.notify();
                }
            }),
        ];
        let items: Vec<Item> = std::fs::read(items_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_else(|| {
                vec![Item {
                    title: "Welcome".into(),
                    body: "# __TITLE__\n\nA new app in the house style. Replace `Item` in \
                           `src/app.rs` with what this app is about."
                        .into(),
                }]
            });
        Self {
            appearance: Appearance::load(),
            panel: Panel::Main,
            selected: (!items.is_empty()).then_some(0),
            items,
            search,
            query: String::new(),
            _subscriptions: subscriptions,
        }
    }

    fn save_items(&self) {
        let _ = std::fs::create_dir_all(state_dir());
        let _ = std::fs::write(
            items_path(),
            serde_json::to_vec_pretty(&self.items).unwrap_or_default(),
        );
    }

    /// Rows matching the search, by index.
    pub fn visible(&self) -> Vec<usize> {
        (0..self.items.len())
            .filter(|i| {
                self.query.is_empty() || self.items[*i].title.to_lowercase().contains(&self.query)
            })
            .collect()
    }

    pub fn select(&mut self, i: usize, cx: &mut Context<Self>) {
        self.selected = Some(i);
        self.panel = Panel::Main;
        cx.notify();
    }

    pub fn new_item(&mut self, cx: &mut Context<Self>) {
        self.items.insert(
            0,
            Item {
                title: format!("Item {}", self.items.len() + 1),
                body: String::new(),
            },
        );
        self.save_items();
        self.select(0, cx);
    }

    pub fn delete_item(&mut self, i: usize, cx: &mut Context<Self>) {
        if i < self.items.len() {
            self.items.remove(i);
            self.save_items();
            self.selected = (!self.items.is_empty()).then(|| i.min(self.items.len() - 1));
            cx.notify();
        }
    }

    pub fn toggle_settings(&mut self, cx: &mut Context<Self>) {
        self.panel = if self.panel == Panel::Settings {
            Panel::Main
        } else {
            Panel::Settings
        };
        cx.notify();
    }

    pub fn set_appearance(&mut self, a: Appearance, window: &mut Window, cx: &mut Context<Self>) {
        self.appearance = a;
        a.save();
        crate::theme::apply(a, Some(window), cx);
        cx.notify();
    }
}
