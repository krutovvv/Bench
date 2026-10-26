//! The look lives in `bench_ui::theme`; this is where Bench keeps the person's choice.

pub use bench_ui::theme::*;

pub fn load() -> Appearance {
    Appearance::load_from(&crate::connection::state_dir())
}

pub fn save(a: &Appearance) {
    a.save_to(&crate::connection::state_dir())
}
