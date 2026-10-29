//! The app's menu in the macOS menu bar, laid out as a Mac app's: Bench (Settings…, Hide, Quit),
//! File (a new thread, project, window; a file opened, saved, closed), Edit (the text's own and
//! the searches), View (Back and Forward, Chat or Code, the docks, the text's size, full screen), Thread (stop,
//! dictate, delete), Window, Help. Each item is an action with its key here — the menu shows the
//! key beside it — and the window acts on it (`listen`, on the window's root in
//! `workspace/layout.rs`). Keys that belong to a field too (⌘⌫ deletes a line in it) stay in
//! `keys.rs`, which gives them to the field when they are not the window's.

use gpui_kit::component::input as text;
use gpui_kit::*;

use super::{BenchApp, Center};

actions!(
    bench,
    [
        OpenSettings,
        Hide,
        HideOthers,
        ShowAll,
        Quit,
        NewThread,
        NewProject,
        NewWindow,
        OpenFile,
        FindInProject,
        Save,
        CloseTab,
        FindThreads,
        ToggleCode,
        GoBack,
        GoForward,
        ToggleSidebar,
        ThreadBeside,
        ToggleTerminal,
        ShowProblems,
        BiggerText,
        SmallerText,
        FullScreen,
        StopThread,
        Dictate,
        SendSelection,
        CommentSelection,
        DeleteThread,
        Minimize,
        Zoom,
        BringAllToFront,
        OpenRepository,
    ]
);

/// Where Bench's sources and its issues are.
const REPOSITORY: &str = "https://github.com/krutovvv/bench";

/// The menu bar and its keys, once at start.
pub fn install(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-,", OpenSettings, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-n", NewThread, None),
        KeyBinding::new("cmd-shift-n", NewWindow, None),
        KeyBinding::new("cmd-p", OpenFile, None),
        KeyBinding::new("cmd-shift-f", FindInProject, None),
        KeyBinding::new("cmd-s", Save, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("cmd-k", FindThreads, None),
        KeyBinding::new("cmd-f", FindThreads, None),
        KeyBinding::new("cmd-e", ToggleCode, None),
        KeyBinding::new("cmd-[", GoBack, None),
        KeyBinding::new("cmd-]", GoForward, None),
        KeyBinding::new("cmd-\\", ToggleSidebar, None),
        KeyBinding::new("cmd-j", ThreadBeside, None),
        KeyBinding::new("ctrl-`", ToggleTerminal, None),
        KeyBinding::new("cmd-shift-m", ShowProblems, None),
        KeyBinding::new("cmd-=", BiggerText, None),
        KeyBinding::new("cmd-+", BiggerText, None),
        KeyBinding::new("cmd--", SmallerText, None),
        KeyBinding::new("ctrl-cmd-f", FullScreen, None),
        KeyBinding::new("cmd-.", StopThread, None),
        KeyBinding::new("cmd-shift-d", Dictate, None),
        KeyBinding::new("cmd-l", SendSelection, None),
        KeyBinding::new("cmd-shift-l", CommentSelection, None),
        KeyBinding::new("cmd-m", Minimize, None),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &BringAllToFront, cx| cx.activate(true));
    cx.on_action(|_: &OpenRepository, cx| cx.open_url(REPOSITORY));
    cx.set_menus(vec![
        Menu::new("Bench").items([
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Hide Bench", Hide),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
            MenuItem::separator(),
            MenuItem::action("Quit Bench", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Thread", NewThread),
            MenuItem::action("New Project…", NewProject),
            MenuItem::action("New Window", NewWindow),
            MenuItem::separator(),
            MenuItem::action("Open File…", OpenFile),
            MenuItem::action("Find in Project…", FindInProject),
            MenuItem::separator(),
            MenuItem::action("Save", Save),
            MenuItem::action("Close Tab", CloseTab),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Undo", text::Undo, OsAction::Undo),
            MenuItem::os_action("Redo", text::Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("Cut", text::Cut, OsAction::Cut),
            MenuItem::os_action("Copy", text::Copy, OsAction::Copy),
            MenuItem::os_action("Paste", text::Paste, OsAction::Paste),
            MenuItem::os_action("Select All", text::SelectAll, OsAction::SelectAll),
            MenuItem::separator(),
            MenuItem::action("Search…", FindThreads),
        ]),
        Menu::new("View").items([
            MenuItem::action("Back", GoBack),
            MenuItem::action("Forward", GoForward),
            MenuItem::separator(),
            MenuItem::action("Chat or Code", ToggleCode),
            MenuItem::action("Sidebar", ToggleSidebar),
            MenuItem::action("Thread Beside", ThreadBeside),
            MenuItem::action("Terminal", ToggleTerminal),
            MenuItem::action("Problems", ShowProblems),
            MenuItem::separator(),
            MenuItem::action("Bigger Text", BiggerText),
            MenuItem::action("Smaller Text", SmallerText),
            MenuItem::separator(),
            MenuItem::action("Enter Full Screen", FullScreen),
        ]),
        Menu::new("Thread").items([
            MenuItem::action("Stop", StopThread),
            MenuItem::action("Dictate", Dictate),
            MenuItem::action("Send Selection to Thread", SendSelection),
            MenuItem::action("Comment on Selection…", CommentSelection),
            MenuItem::separator(),
            MenuItem::action("Delete Thread…", DeleteThread),
        ]),
        Menu::new("Window").items([
            MenuItem::action("Minimize", Minimize),
            MenuItem::action("Zoom", Zoom),
            MenuItem::separator(),
            MenuItem::action("Bring All to Front", BringAllToFront),
        ]),
        Menu::new("Help").items([MenuItem::action("Bench on GitHub", OpenRepository)]),
    ]);
}

/// The window's answers to the menu: its root listens for every item that acts on it.
pub fn listen(root: Div, cx: &mut Context<BenchApp>) -> Div {
    root.on_action(cx.listener(BenchApp::on_open_settings))
        .on_action(cx.listener(BenchApp::on_new_window))
        .on_action(cx.listener(|this, _: &NewThread, window, cx| {
            // The new thread appears where a thread lives: beside a document, in the dock;
            // otherwise the center shows it (`new_thread_here`).
            this.new_thread_here(cx);
            this.focus_composer(window, cx);
        }))
        .on_action(cx.listener(|this, _: &NewProject, window, cx| {
            this.new_project_form(window, cx)
        }))
        .on_action(cx.listener(|this, _: &OpenFile, window, cx| this.open_quick(window, cx)))
        .on_action(cx.listener(|this, _: &FindInProject, window, cx| this.open_find(window, cx)))
        .on_action(cx.listener(|this, _: &Save, window, cx| {
            this.save_active_doc(window, cx);
        }))
        .on_action(cx.listener(|this, _: &CloseTab, _, cx| {
            if this.center == Center::Doc {
                if let Some(key) = this.explorer.active.clone() {
                    this.close_doc(key, cx);
                }
            }
        }))
        .on_action(cx.listener(|this, _: &FindThreads, window, cx| this.open_search(window, cx)))
        .on_action(cx.listener(|this, _: &ToggleCode, window, cx| this.toggle_side(window, cx)))
        .on_action(cx.listener(|this, _: &GoBack, window, cx| this.go_back(window, cx)))
        .on_action(cx.listener(|this, _: &GoForward, window, cx| this.go_forward(window, cx)))
        .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| this.toggle_sidebar(cx)))
        .on_action(cx.listener(|this, _: &ThreadBeside, window, cx| {
            this.toggle_thread_beside(window, cx)
        }))
        .on_action(cx.listener(|this, _: &ToggleTerminal, window, cx| {
            this.toggle_terminal_panel(window, cx)
        }))
        .on_action(cx.listener(|this, _: &ShowProblems, window, cx| {
            this.open_problems(window, cx)
        }))
        .on_action(cx.listener(|this, _: &BiggerText, window, cx| this.step_size(1, window, cx)))
        .on_action(cx.listener(|this, _: &SmallerText, window, cx| this.step_size(-1, window, cx)))
        .on_action(cx.listener(|_, _: &FullScreen, window, _| window.toggle_fullscreen()))
        .on_action(cx.listener(|this, _: &StopThread, _, cx| {
            if this.thread(cx).live.is_some() {
                this.stop(cx);
            }
        }))
        .on_action(cx.listener(|this, _: &Dictate, window, cx| this.toggle_mic(window, cx)))
        .on_action(cx.listener(|this, _: &SendSelection, window, cx| {
            if this.focused_doc().is_some() {
                this.send_selection_to_thread(window, cx)
            }
        }))
        .on_action(cx.listener(|this, _: &CommentSelection, window, cx| {
            if this.focused_doc().is_some() {
                this.start_comment(window, cx)
            }
        }))
        // Asked once, as ⌫ ⌫ in the sidebar (`sidebar/items.rs`): the item arms it, a second
        // time deletes.
        .on_action(cx.listener(|this, _: &DeleteThread, _, cx| this.delete_open_thread(cx)))
        .on_action(cx.listener(|_, _: &Minimize, window, _| window.minimize_window()))
        .on_action(cx.listener(|_, _: &Zoom, window, _| window.zoom_window()))
}

impl BenchApp {
    /// File → New Window: another window (`marks.rs`).
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

    /// Thread → Delete Thread… and ⌘⌫: the open thread deleted, asked once.
    pub(crate) fn delete_open_thread(&mut self, cx: &mut Context<Self>) {
        if self.center != Center::Chat {
            return;
        }
        let at = self.thread_ref(cx);
        let Some(id) = at.id.clone() else {
            return;
        };
        if self.armed.is(&crate::app::armed::thread_key(id.as_str())) {
            self.delete_thread(at.host, id, cx);
        } else {
            self.arm_delete(id.to_string(), cx);
        }
    }
}
