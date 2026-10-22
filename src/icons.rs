//! One icon set for the whole app: Lucide (ISC, see assets/icons/LICENSE), embedded.

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

            /// The icon at a size; the loader spins, the recording waveform breathes.
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
                    Ico::AudioLines => div()
                        .child(icon)
                        .with_animation(
                            "breathe",
                            Animation::new(Duration::from_millis(1400)).repeat().with_easing(pulsating_between(0.45, 1.)),
                            |d, t| d.opacity(t),
                        )
                        .into_any_element(),
                    _ => icon.into_any_element(),
                }
            }
        }
    };
}

icons! {
    ArrowUp => "arrow-up",
    AudioLines => "audio-lines",
    Bot => "bot",
    Brain => "brain",
    ChevronDown => "chevron-down",
    ChevronRight => "chevron-right",
    CircleAlert => "circle-alert",
    Copy => "copy",
    Cpu => "cpu",
    Gauge => "gauge",
    GitBranch => "git-branch",
    Globe => "globe",
    ExternalLink => "external-link",
    File => "file",
    FileDiff => "file-diff",
    Folder => "folder",
    FolderOpen => "folder-open",
    FolderTree => "folder-tree",
    Save => "save",
    Loader => "loader-circle",
    MemoryStick => "memory-stick",
    Mic => "mic",
    PanelRight => "panel-right",
    Pause => "pause",
    Play => "play",
    Plus => "plus",
    RefreshCw => "refresh-cw",
    RotateCcw => "rotate-ccw",
    Server => "server",
    Settings => "settings",
    Shield => "shield",
    Square => "square",
    Terminal => "square-terminal",
    Trash => "trash",
    UserRound => "user-round",
    Volume => "volume-2",
    Search => "search",
    X => "x",
}
