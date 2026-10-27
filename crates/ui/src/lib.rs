//! Bench's elements. The only crate that touches `gpui-kit` styling and pixel values.
//!
//! Gives: `theme` (palette, fonts, accent), `icons` (`Ico`), `metrics` (every size and `Gap`),
//! `text`, `button`, `field`, `block`, `layout` (`row`, `stack`), `form`, `list`, `bars`, `fmt`.
//! Takes: nothing from Bench. Every element is stateless: data and handlers in, an element out.
//!
//! The storybook (`cargo run -p storybook`) shows each of them in every state.

pub mod bars;
pub mod block;
pub mod button;
pub mod field;
pub mod fmt;
pub mod form;
pub mod icons;
pub mod ide;
pub mod layout;
pub mod list;
pub mod metrics;
pub mod swatch;
pub mod text;
pub mod theme;

pub use bars::*;
pub use block::*;
pub use button::*;
pub use field::*;
pub use fmt::*;
pub use form::*;
pub use icons::Ico;
pub use ide::*;
pub use layout::*;
pub use list::*;
pub use metrics::*;
pub use swatch::*;
pub use text::*;

/// A click handler, as `cx.listener(...)` makes it.
pub type OnClick =
    Box<dyn Fn(&gpui_kit::ClickEvent, &mut gpui_kit::Window, &mut gpui_kit::App) + 'static>;
/// A handler that gets the index of what was clicked (segmented cells, tabs).
pub type OnPick = std::rc::Rc<dyn Fn(usize, &mut gpui_kit::Window, &mut gpui_kit::App) + 'static>;
