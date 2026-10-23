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
    ArrowUpBold => "arrow-up-bold",
    Bot => "bot",
    Brain => "brain",
    Chat => "message-square-text",
    ChevronDown => "chevron-down",
    ChevronRight => "chevron-right",
    CircleAlert => "circle-alert",
    CircleCheck => "circle-check",
    CircleDot => "circle-dot",
    Download => "download",
    CircleHelp => "circle-help",
    CircleEmpty => "circle",
    Clock => "clock",
    Copy => "copy",
    Ellipsis => "ellipsis",
    Cpu => "cpu",
    Gauge => "gauge",
    GitBranch => "git-branch",
    Globe => "globe",
    ExternalLink => "external-link",
    File => "file",
    FileText => "file-text",
    Film => "film",
    Image => "image",
    Music => "music",
    Folder => "folder",
    Database => "database",
    FileArchive => "file-archive",
    FileAudio => "file-audio",
    FileCode => "file-code",
    FileCog => "file-cog",
    FileImage => "file-image",
    FileJson => "file-json",
    FileLock => "file-lock",
    FileTerminal => "file-terminal",
    FileVideo => "file-video",
    FolderArchive => "folder-archive",
    FolderCode => "folder-code",
    FolderCog => "folder-cog",
    FolderGit => "folder-git-2",
    FolderLock => "folder-lock",
    FolderOpen => "folder-open",
    FolderTree => "folder-tree",
    Save => "save",
    ListChecks => "list-checks",
    Loader => "loader-circle",
    Maximize => "maximize-2",
    MemoryStick => "memory-stick",
    Minimize => "minimize-2",
    MicFill => "mic-fill",
    PanelRight => "panel-right",
    Paperclip => "paperclip",
    PaperclipBold => "paperclip-bold",
    Pause => "pause",
    Pencil => "pencil",
    Pin => "pin",
    Play => "play",
    Plus => "plus",
    RefreshCw => "refresh-cw",
    RotateCcw => "rotate-ccw",
    Server => "server",
    Send => "send",
    Settings => "settings",
    Shield => "shield",
    Square => "square",
    Terminal => "square-terminal",
    Trash => "trash",
    UserRound => "user-round",
    Volume => "volume-2",
    Wrench => "wrench",
    Search => "search",
    X => "x",
    // Not Lucide: a Z in Lucide's style, for "open in Zed".
    Zed => "zed",
}
