//! One icon set for the whole app: Lucide (ISC, see assets/icons/LICENSE), embedded.
// The template ships icons a new app may not use yet.
#![allow(dead_code)]

use std::time::Duration;

use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::*;

macro_rules! icons {
    ($($name:ident => $file:literal),* $(,)?) => {
        #[derive(Clone, Copy)]
        pub enum Ico { $($name),* }

        impl Ico {
            fn bytes(self) -> &'static [u8] {
                match self { $(Ico::$name => include_bytes!(concat!("../assets/icons/", $file, ".svg"))),* }
            }

            pub fn icon(self) -> Icon {
                Icon::default().data(self.bytes())
            }

            /// The icon at a size; the loader spins.
            pub fn live(self, small: bool) -> AnyElement {
                let icon = if small { self.icon().small() } else { self.icon().xsmall() };
                match self {
                    Ico::Loader => icon
                        .with_animation(
                            "spin",
                            Animation::new(Duration::from_millis(900)).repeat(),
                            |i, t| i.transform(Transformation::rotate(percentage(t))),
                        )
                        .into_any_element(),
                    _ => icon.into_any_element(),
                }
            }
        }
    };
}

icons! {
    AppWindow => "app-window",
    ChevronDown => "chevron-down",
    ChevronRight => "chevron-right",
    CircleAlert => "circle-alert",
    Copy => "copy",
    Ellipsis => "ellipsis",
    ExternalLink => "external-link",
    File => "file",
    FileText => "file-text",
    Folder => "folder",
    Loader => "loader-circle",
    Pin => "pin",
    Plus => "plus",
    RefreshCw => "refresh-cw",
    Search => "search",
    Settings => "settings",
    Trash => "trash",
    X => "x",
}
