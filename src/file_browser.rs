//! The file browser panel: `data/`'s direct subdirectories (`samples/`,
//! `projects/`, `plugins/`) become horizontal tabs along the top; the
//! selected tab's contents render below as a VS Code-style folder tree —
//! expand/collapse per folder, no "go up" affordance, since each tab's
//! directory *is* the tree's root rather than something navigated into.
//! See `daw-rewrite-starter/02-feature-scope.md` item 8 ("a minimal file
//! browser").
//!
//! Docked to the left edge of the shell (`shell.rs`), not floating like
//! Channel Rack/Piano Roll/Playlist/Mixer — every ordinary IDE pins its
//! file tree to one side rather than letting it float, and this panel's
//! whole job (drag a sample into a channel) only makes sense if it's
//! reliably sitting right next to the thing it's dragged onto.
//!
//! Audio files are real drag sources (`SampleDragPayload`, picked up by
//! `channel_rack::row`'s sample box and its "+ Channel" button) — the
//! feature-scope brief's own "drag/drop (or at least pick) a sample file
//! onto a channel," now actually drag/drop, not just the `rfd` file
//! picker that was the only way in before this panel existed.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::assets::IconName;
use gpui::component::list::ListItem;
use gpui::component::tab::{Tab, TabBar};
use gpui::component::tree::{TreeItem, TreeState, tree};
use gpui::component::{Icon, Sizable};
use gpui::{
    App, Context, CursorStyle, Entity, Hsla, IntoElement, ParentElement, Pixels, Point, Render,
    Styled, Window, div, prelude::*, px,
};

use crate::audio::{self, AudioEngine, SampleBuffer};
use crate::sample_downloader::{DownloadState, SampleDownloader};
use crate::theme;

/// Where the browser's tabs come from — the original app's `data/`
/// directory, one level down. Relative, since `nix-shell --run "cargo
/// run"` (this repo's only supported way to launch — see `CLAUDE.md`)
/// always runs from the repo root.
const DATA_ROOT: &str = "data";

/// The subfolders every checkout is expected to have under `data/`
/// (samples/projects/plugins tabs) — all three are gitignored, so a
/// fresh clone or a different machine won't have them yet. Created on
/// startup if missing so the tabs always show up rather than silently
/// disappearing.
const DATA_SUBFOLDERS: &[&str] = &["samples", "projects", "plugins"];

fn ensure_data_folders() {
    for name in DATA_SUBFOLDERS {
        let _ = fs::create_dir_all(Path::new(DATA_ROOT).join(name));
    }
}

/// This panel's own identity color (gold) — carried over from when it was
/// still a `PanelKind` variant in `floating_panel.rs`; kept here now that
/// it's docked instead, since the toolbar's show/hide pill and this
/// panel's own header still want one consistent tint.
pub const IDENTITY_COLOR: Hsla = Hsla { h: 45.0 / 360.0, s: 0.55, l: 0.66, a: 1.0 };
pub const ICON: IconName = IconName::Folder;

const AUDIO_EXTENSIONS: &[&str] = &["wav", "ogg", "mp3", "flac"];

fn is_audio_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).map(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str())).unwrap_or(false)
}

/// Carried by `on_drag`/`on_drop` for dragging a sample file out of this
/// tree onto a Channel Rack row (or its "+ Channel" button). `label` is
/// just for the drag-preview ghost; `path` is what actually gets assigned.
#[derive(Clone)]
pub struct SampleDragPayload {
    pub path: String,
    pub label: String,
}

/// The floating ghost shown under the cursor while dragging a sample —
/// GPUI's drag API requires a real view, not just a style, to render this.
pub struct SampleDragPreview {
    label: String,
}

impl Render for SampleDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded(theme::RADIUS_SM)
            .bg(theme::BG_PANEL.alpha(0.95))
            .border_1()
            .border_color(IDENTITY_COLOR)
            .text_size(px(11.0))
            .text_color(theme::MAIN_TEXT)
            .child(self.label.clone())
    }
}

fn sample_drag_preview(payload: &SampleDragPayload, _offset: Point<Pixels>, _window: &mut Window, cx: &mut App) -> Entity<SampleDragPreview> {
    cx.new(|_| SampleDragPreview { label: payload.label.clone() })
}

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
    engine: Arc<AudioEngine>,
    /// Decoded samples previewed by clicking an audio-file leaf, keyed by
    /// absolute path — kept around so re-clicking the same file doesn't
    /// re-decode it, same caching shape `ChannelRack::cache_sample` uses.
    samples: HashMap<String, Arc<SampleBuffer>>,
    /// (tab label, absolute directory it roots) — one per direct child
    /// directory of `data/`, in alphabetical order.
    tabs: Vec<(String, PathBuf)>,
    selected_tab: usize,
    tree: Entity<TreeState>,
    downloader: Option<SampleDownloader>,
    /// A one-line status left over from the last download, shown next to
    /// the button until the next tab switch or download attempt clears it.
    download_message: Option<(String, bool)>, // (text, is_error)
}

impl FileBrowser {
    pub fn new(engine: Arc<AudioEngine>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        ensure_data_folders();
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

        Self { engine, samples: HashMap::new(), tabs, selected_tab: 0, tree, downloader: None, download_message: None }
    }

    /// Plays an audio-file leaf's sample straight through, once, at the
    /// same default volume/pan a freshly added Channel Rack channel gets
    /// (`Channel::volume`/`pan`'s own defaults) — there's no channel here
    /// to read real levels from, just a quick "what does this sound
    /// like" audition. Decoding failures are silently ignored, matching
    /// `ChannelRack::audition`'s own "no sample, no sound" shape rather
    /// than surfacing a dialog for what's just a preview click.
    fn play_sample(&mut self, path: &Path, cx: &mut Context<Self>) {
        let path_str = path.to_string_lossy().to_string();
        let sample = match self.samples.get(&path_str) {
            Some(sample) => sample.clone(),
            None => match audio::decode_sample(path) {
                Ok(sample) => {
                    self.samples.insert(path_str, sample.clone());
                    sample
                }
                Err(e) => {
                    eprintln!("[file browser] couldn't decode {}: {e}", path.display());
                    return;
                }
            },
        };
        self.engine.preview(sample, 0.8, 0.5);
        cx.notify();
    }

    fn select_tab(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix == self.selected_tab || ix >= self.tabs.len() {
            return;
        }
        self.selected_tab = ix;
        self.download_message = None;
        let items = scan_dir(&self.tabs[ix].1);
        self.tree.update(cx, |state, cx| state.set_items(items, cx));
        cx.notify();
    }

    fn is_samples_tab(&self) -> bool {
        self.tabs.get(self.selected_tab).map(|(name, _)| name.eq_ignore_ascii_case("samples")).unwrap_or(false)
    }

    fn rescan_current_tab(&mut self, cx: &mut Context<Self>) {
        if let Some((_, path)) = self.tabs.get(self.selected_tab) {
            let items = scan_dir(path);
            self.tree.update(cx, |state, cx| state.set_items(items, cx));
        }
    }

    fn start_default_sample_download(&mut self, cx: &mut Context<Self>) {
        if self.downloader.is_some() {
            return;
        }
        let Some((_, samples_root)) = self.tabs.get(self.selected_tab) else { return };
        let dest = samples_root.join("Default Samples");
        self.download_message = None;
        self.downloader = Some(SampleDownloader::start(dest));
        cx.notify();
    }

    /// Polls the background downloader (if any) once per render, applying
    /// a terminal state exactly once — same "mutate from render" shape
    /// `toolbar.rs`'s own meter/autoscale state already uses, not a new
    /// pattern for this codebase. Returns `true` while a download is
    /// still running, so `render` knows whether to keep animating.
    fn poll_downloader(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(downloader) = &self.downloader else { return false };
        match downloader.state() {
            DownloadState::Running { .. } => true,
            DownloadState::Done { count } => {
                self.rescan_current_tab(cx);
                self.download_message =
                    Some((format!("Downloaded {count} sample{} into \"Default Samples\".", if count == 1 { "" } else { "s" }), false));
                self.downloader = None;
                false
            }
            DownloadState::Failed(err) => {
                self.download_message = Some((format!("Download failed: {err}"), true));
                self.downloader = None;
                false
            }
        }
    }

    fn render_download_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let running = matches!(self.downloader.as_ref().map(|d| d.state()), Some(DownloadState::Running { .. }));
        let progress = match self.downloader.as_ref().map(|d| d.state()) {
            Some(DownloadState::Running { done, total }) => Some((done, total)),
            _ => None,
        };

        let mut button = div()
            .id("download-default-samples")
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .h(px(22.0))
            .rounded(theme::RADIUS_XS)
            .border_1()
            .border_color(theme::BTN_BORDER)
            .text_size(px(10.0))
            .text_color(theme::BTN_TEXT)
            .child(Icon::new(IconName::Download).xsmall())
            .child(match progress {
                Some((done, total)) => format!("Downloading… {done}/{total}"),
                None => "Download Default Samples".to_string(),
            });
        button = if running {
            button.cursor_default().opacity(0.6)
        } else {
            button.cursor_pointer().hover(|s| s.bg(theme::BTN_HOVER).text_color(theme::MAIN_TEXT)).on_click(
                cx.listener(|this, _: &gpui::ClickEvent, _, cx| this.start_default_sample_download(cx)),
            )
        };

        div()
            .flex()
            .flex_col()
            .gap_1()
            .px_2()
            .pb_2()
            .child(button)
            .when_some(self.download_message.clone(), |d, (msg, is_error)| {
                d.child(
                    div()
                        .text_size(px(10.0))
                        .text_color(if is_error { theme::destructive() } else { theme::METER_GREEN })
                        .child(msg),
                )
            })
    }
}

/// `depth` indents nested entries; the folder/file glyph swaps to an open
/// folder once expanded, matching VS Code's own explorer affordance. An
/// audio-file leaf gets a waveform glyph instead of the plain file icon,
/// is a drag source (see `SampleDragPayload`) so it can be dropped
/// straight onto a Channel Rack row, and a click plays it through
/// `browser.play_sample` as a quick audition.
fn render_entry(
    ix: usize,
    entry: &gpui::component::tree::TreeEntry,
    selected: bool,
    browser: &Entity<FileBrowser>,
    _window: &mut Window,
    _cx: &mut App,
) -> ListItem {
    let depth = entry.depth();
    let is_folder = entry.is_folder();
    let path = PathBuf::from(entry.item().id.as_ref());
    let is_audio = !is_folder && is_audio_file(&path);
    let icon = if is_folder {
        if entry.is_expanded() { IconName::FolderOpen } else { IconName::Folder }
    } else if is_audio {
        IconName::AudioWaveform
    } else {
        IconName::File
    };
    let icon_color = if is_folder { theme::accent_dim() } else if is_audio { IDENTITY_COLOR } else { theme::BTN_TEXT };
    let label = entry.item().label.clone();

    let mut item = ListItem::new(("file-browser-entry", ix))
        .selected(selected)
        .pl(px(8.0 + depth as f32 * 16.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .py(px(2.0))
                .child(Icon::new(icon).xsmall().text_color(icon_color))
                .child(div().text_size(px(12.0)).text_color(theme::MAIN_TEXT).child(label.clone())),
        );

    if is_audio {
        let path_str = path.to_string_lossy().to_string();
        item = item
            .cursor(CursorStyle::OpenHand)
            .on_drag(SampleDragPayload { path: path_str, label: label.to_string() }, sample_drag_preview);

        let browser = browser.clone();
        item = item.on_click(move |_, _window, cx| {
            browser.update(cx, |this, cx| this.play_sample(&path, cx));
        });
    }

    item
}

impl Render for FileBrowser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let still_running = self.poll_downloader(cx);
        if still_running {
            window.request_animation_frame();
            cx.notify();
        }

        let selected = self.selected_tab;
        let on_samples_tab = self.is_samples_tab();
        let tab_bar = TabBar::new("file-browser-tabs")
            .underline()
            .small()
            .selected_index(selected)
            .children(self.tabs.iter().map(|(name, _)| Tab::new().label(name.clone())))
            .on_click(cx.listener(|this, ix: &usize, _, cx| this.select_tab(*ix, cx)));

        let mut root = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::BG_PANEL)
            .border_r_1()
            .border_color(theme::BORDER)
            .child(
                div()
                    .px_2()
                    .pt_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Icon::new(IconName::Folder).small().text_color(IDENTITY_COLOR))
                    .child(div().text_size(px(11.0)).font_weight(gpui::FontWeight::SEMIBOLD).text_color(theme::MAIN_TEXT).child("Explorer")),
            )
            .child(div().px_2().child(tab_bar));

        if on_samples_tab {
            root = root.child(self.render_download_bar(cx));
        }

        let browser = cx.entity();
        root.child(
            div()
                .flex_1()
                .w_full()
                .overflow_hidden()
                .child(tree(&self.tree, move |ix, entry, selected, window, cx| render_entry(ix, entry, selected, &browser, window, cx))),
        )
    }
}
