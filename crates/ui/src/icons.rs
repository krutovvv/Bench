//! One icon set for the whole app: Lucide (ISC, see assets/icons/LICENSE), embedded.

use std::time::Duration;

use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::*;

macro_rules! icons {
    ($($name:ident => $file:literal),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub enum Ico { $($name),* }

        impl Ico {
            /// Every icon, for the storybook.
            pub const ALL: &[Ico] = &[$(Ico::$name),*];

            fn bytes(self) -> &'static [u8] {
                match self { $(Ico::$name => include_bytes!(concat!("../../../assets/icons/", $file, ".svg"))),* }
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
    Activity => "activity",
    AppWindow => "app-window",
    Bell => "bell",
    Bot => "bot",
    Brain => "brain",
    ChevronDown => "chevron-down",
    ChevronRight => "chevron-right",
    CircleAlert => "circle-alert",
    CircleCheck => "circle-check",
    CircleDot => "circle-dot",
    CircleHelp => "circle-help",
    CircleEmpty => "circle",
    Clock => "clock",
    Copy => "copy",
    Ellipsis => "ellipsis",
    Gauge => "gauge",
    GitBranch => "git-branch",
    GitFork => "git-fork",
    Globe => "globe",
    ExternalLink => "external-link",
    File => "file",
    FileText => "file-text",
    Film => "film",
    Image => "image",
    Music => "music",
    Folder => "folder",
    FolderOpen => "folder-open",
    FileArchive => "file-archive",
    FileAudio => "file-audio",
    Code => "code",
    FileCode => "file-code",
    FileCog => "file-cog",
    FileImage => "file-image",
    FileJson => "file-json",
    FileLock => "file-lock",
    FileTerminal => "file-terminal",
    FileVideo => "file-video",
    FolderGit => "folder-git-2",
    FolderArchive => "folder-archive",
    FolderCode => "folder-code",
    FolderCog => "folder-cog",
    FolderLock => "folder-lock",
    Package => "package",
    Scale => "scale",
    Save => "save",
    ListChecks => "list-checks",
    Tag => "tag",
    Loader => "loader-circle",
    MemoryStick => "memory-stick",
    Mic => "mic",
    PanelBottom => "panel-bottom",
    PanelLeft => "panel-left",
    PanelRight => "panel-right",
    FolderTree => "folder-tree",
    FileDiff => "file-diff",
    MessageSquare => "message-square",
    Link => "link",
    Laptop => "laptop",
    Info => "info",
    MessagesSquare => "messages-square",
    Paperclip => "paperclip",
    Send => "send",
    Pencil => "pencil",
    Pin => "pin",
    Pause => "pause",
    Play => "play",
    Plus => "plus",
    Power => "power",
    RefreshCw => "refresh-cw",
    RotateCcw => "rotate-ccw",
    Server => "server",
    Settings => "settings",
    Square => "square",
    Sparkles => "sparkles",
    Terminal => "square-terminal",
    Trash => "trash",
    FilePlus => "file-plus",
    FolderPlus => "folder-plus",
    UserRound => "user-round",
    Volume => "volume-2",
    Search => "search",
    X => "x",
    BookCheck => "book-check",
    Wrench => "wrench",
    CalendarDays => "calendar-days",
    Check => "check",
    Flag => "flag",
    Layers => "layers",
    Moon => "moon",
    Star => "star",
}
