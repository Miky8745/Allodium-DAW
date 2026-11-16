//! The file browser panel: `data/`'s direct subdirectories (`samples/`,
//! `projects/`, `plugins/`) become horizontal tabs along the top; the
//! selected tab's contents render below as a VS Code-style folder tree —
//! expand/collapse per folder, no "go up" affordance, since each tab's
//! directory *is* the tree's root rather than something navigated into.
//! See `daw-rewrite-starter/02-feature-scope.md` item 8 ("a minimal file
//! browser").

use std::fs;
use std::path::{Path, PathBuf};

use gpui::assets::IconName;
use gpui::component::list::ListItem;
use gpui::component::tab::{Tab, TabBar};
use gpui::component::tree::{TreeItem, TreeState, tree};
use gpui::component::{Icon, Sizable};
use gpui::{App, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px};

use crate::theme;

/// Where the browser's tabs come from — the original app's `data/`
/// directory, one level down. Relative, since `nix-shell --run "cargo
/// run"` (this repo's only supported way to launch — see `CLAUDE.md`)
/// always runs from the repo root.
const DATA_ROOT: &str = "data";

/// Folders sort before files, both alphabetically — the ordinary explorer
/// convention. An empty directory has no children, so it renders as a
/// plain (non-expandable) leaf; that's an acceptable corner case here.
fn scan_dir(path: &Path) -> Vec<TreeItem> {
    let Ok(read) = fs::read_dir(path) else { return Vec::new() };
    let mut entries: Vec<_> = read.filter_map(|e| e.ok()).collect();
    entries.sort_by(|a, b| {
        let a_dir = a.path().is_dir();
        let b_dir = b.path().is_dir();
        b_dir.cmp(&a_dir).then_with(|| a.file_name().to_ascii_lowercase().cmp(&b.file_name().to_ascii_lowercase()))
    });
    entries
        .into_iter()
        .map(|entry| {
            let entry_path = entry.path();
            let label = entry.file_name().to_string_lossy().to_string();
            let id = entry_path.to_string_lossy().to_string();
            if entry_path.is_dir() {
                TreeItem::new(id, label).children(scan_dir(&entry_path))
            } else {
                TreeItem::new(id, label)
            }
        })
        .collect()
}

pub struct FileBrowser {
    /// (tab label, absolute directory it roots) — one per direct child
    /// directory of `data/`, in alphabetical order.
    tabs: Vec<(String, PathBuf)>,
    selected_tab: usize,
    tree: Entity<TreeState>,
}

impl FileBrowser {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut tabs: Vec<(String, PathBuf)> = fs::read_dir(DATA_ROOT)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| (e.file_name().to_string_lossy().to_string(), e.path()))
            .collect();
        tabs.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));

        let tree = cx.new(|cx| TreeState::new(cx).items(tabs.first().map(|(_, path)| scan_dir(path)).unwrap_or_default()));

        Self { tabs, selected_tab: 0, tree }
    }

    fn select_tab(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix == self.selected_tab || ix >= self.tabs.len() {
            return;
        }
        self.selected_tab = ix;
        let items = scan_dir(&self.tabs[ix].1);
        self.tree.update(cx, |state, cx| state.set_items(items, cx));
        cx.notify();
    }
}

/// `depth` indents nested entries; the folder/file glyph swaps to an open
/// folder once expanded, matching VS Code's own explorer affordance.
fn render_entry(ix: usize, entry: &gpui::component::tree::TreeEntry, selected: bool, _window: &mut Window, _cx: &mut App) -> ListItem {
    let depth = entry.depth();
    let is_folder = entry.is_folder();
    let icon = if is_folder {
        if entry.is_expanded() { IconName::FolderOpen } else { IconName::Folder }
    } else {
        IconName::File
    };
    let icon_color = if is_folder { theme::accent_dim() } else { theme::BTN_TEXT };

    ListItem::new(("file-browser-entry", ix))
        .selected(selected)
        .pl(px(8.0 + depth as f32 * 16.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .py(px(2.0))
                .child(Icon::new(icon).xsmall().text_color(icon_color))
                .child(div().text_size(px(12.0)).text_color(theme::MAIN_TEXT).child(entry.item().label.clone())),
        )
}

impl Render for FileBrowser {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected_tab;
        let tab_bar = TabBar::new("file-browser-tabs")
            .underline()
            .small()
            .selected_index(selected)
            .children(self.tabs.iter().map(|(name, _)| Tab::new().label(name.clone())))
            .on_click(cx.listener(|this, ix: &usize, _, cx| this.select_tab(*ix, cx)));

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::BG_PANEL)
            .child(div().px_2().child(tab_bar))
            .child(div().flex_1().w_full().overflow_hidden().child(tree(&self.tree, render_entry)))
    }
}
