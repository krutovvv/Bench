//! The composer's files: the paperclip, drag and drop onto the chat, ⌘V of a screenshot.

use std::path::PathBuf;

use gpui_kit::*;

use super::BenchApp;
use crate::attach;

impl BenchApp {
    pub fn add_attachments(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        for p in paths {
            if p.is_file() && !self.attachments.contains(&p) {
                self.attachments.push(p);
            }
        }
        cx.notify();
    }

    /// The paperclip: files picked in the system's dialog, attached as a drop would.
    pub fn pick_attachments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Attach".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await {
                let _ = this.update(cx, |this, cx| this.add_attachments(paths, cx));
            }
        })
        .detach();
    }

    pub fn remove_attachment(&mut self, k: usize, cx: &mut Context<Self>) {
        if k < self.attachments.len() {
            self.attachments.remove(k);
            cx.notify();
        }
    }

    /// ⌘V with an image (a screenshot copied with ⌃⇧⌘4) or files copied in Finder attaches them;
    /// returns false when the clipboard holds only text (the text field pastes it).
    pub fn paste_attachments(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(item) = cx.read_from_clipboard() else {
            return false;
        };
        let mut paths = vec![];
        for entry in item.entries() {
            match entry {
                ClipboardEntry::ExternalPaths(p) => paths.extend(p.paths().iter().cloned()),
                ClipboardEntry::Image(img) => {
                    let ext = match img.format {
                        ImageFormat::Jpeg => "jpg",
                        ImageFormat::Gif => "gif",
                        ImageFormat::Webp => "webp",
                        ImageFormat::Svg => "svg",
                        ImageFormat::Bmp => "bmp",
                        ImageFormat::Tiff => "tiff",
                        _ => "png",
                    };
                    if let Ok(p) = attach::save_pasted(&img.bytes, ext) {
                        paths.push(p);
                    }
                }
                _ => {}
            }
        }
        if paths.is_empty() {
            return false;
        }
        self.add_attachments(paths, cx);
        true
    }
}
