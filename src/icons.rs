//! One icon set for the whole app: Lucide (ISC, see assets/icons/LICENSE), embedded.

use gpui_kit::component::Icon;

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
        }
    };
}

icons! {
    ArrowUp => "arrow-up",
    Brain => "brain",
    ChevronDown => "chevron-down",
    ChevronRight => "chevron-right",
    CircleAlert => "circle-alert",
    Copy => "copy",
    Cpu => "cpu",
    Globe => "globe",
    Loader => "loader-circle",
    MemoryStick => "memory-stick",
    Mic => "mic",
    Plus => "plus",
    RefreshCw => "refresh-cw",
    RotateCcw => "rotate-ccw",
    Server => "server",
    Settings => "settings",
    Square => "square",
    Terminal => "square-terminal",
    Trash => "trash",
    UserRound => "user-round",
    Volume => "volume-2",
    X => "x",
}
