//! The startup project menu: shown instead of `AppShell` until a project
//! is opened or created, so a session always knows which `data/projects/
//! <name>` folder it's reading/writing (`project::projects_root`) instead
//! of a single hardcoded project name. `AppRoot` is the window's actual
//! root view (swapped in for `AppShell` in `main.rs`); it owns either
//! this menu or the shell and switches once a project is chosen.

use std::sync::Arc;

use gpui::assets::IconName;
use gpui::component::button::{Button, ButtonVariants};
use gpui::component::input::{InputEvent, InputState};
use gpui::component::{ActiveTheme, Icon, Sizable};
use gpui::{ClickEvent, Context, Entity, EventEmitter, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px};

use crate::audio::AudioEngine;
use crate::project;
use crate::shell::AppShell;
use crate::theme;

pub enum ProjectMenuEvent {
    Open(String),
}

impl EventEmitter<ProjectMenuEvent> for ProjectMenu {}

pub struct ProjectMenu {
    projects: Vec<String>,
    new_name_input: Entity<InputState>,
    error: Option<String>,
}

impl ProjectMenu {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let new_name_input = cx.new(|cx| InputState::new(window, cx).placeholder("New project name…"));
        cx.subscribe(&new_name_input, |this: &mut Self, _input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.create_project(cx);
            }
        })
        .detach();
        Self { projects: project::list_projects().unwrap_or_default(), new_name_input, error: None }
    }

    fn refresh(&mut self) {
        self.projects = project::list_projects().unwrap_or_default();
    }

    fn open_project(&mut self, name: String, cx: &mut Context<Self>) {
        cx.emit(ProjectMenuEvent::Open(name));
    }

    fn delete_project(&mut self, name: &str, cx: &mut Context<Self>) {
        if let Err(e) = project::delete_project(name) {
            self.error = Some(e);
        }
        self.refresh();
        cx.notify();
    }

    fn create_project(&mut self, cx: &mut Context<Self>) {
        let name = self.new_name_input.read(cx).value().trim().to_string();
        if name.is_empty() {
            return;
        }
        match project::create_project(&name) {
            Ok(()) => cx.emit(ProjectMenuEvent::Open(name)),
            Err(e) => {
                self.error = Some(e);
                self.refresh();
                cx.notify();
            }
        }
    }
}

fn project_row(ix: usize, name: &str, cx: &mut Context<ProjectMenu>) -> impl IntoElement + use<> {
    let open_name = name.to_string();
    let delete_name = name.to_string();
    div()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .rounded(theme::RADIUS_SM)
        .bg(theme::BG_PILL)
        .child(Icon::new(IconName::Music).small().text_color(theme::accent_dim()))
        .child(div().flex_1().child(name.to_string()))
        .child(Button::new(("open-project", ix)).ghost().small().icon(IconName::FolderOpen).label("Open").on_click(
            cx.listener(move |this, _: &ClickEvent, _, cx| this.open_project(open_name.clone(), cx)),
        ))
        .child(
            Button::new(("delete-project", ix))
                .ghost()
                .danger()
                .small()
                .icon(IconName::Trash)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.delete_project(&delete_name, cx))),
        )
}

impl Render for ProjectMenu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let names = self.projects.clone();
        let mut rows = Vec::new();
        for (ix, name) in names.iter().enumerate() {
            rows.push(project_row(ix, name, cx));
        }
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::BG_APP)
            .font_family(theme::FONT_UI)
            .child(
                div()
                    .w(px(420.0))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .rounded(theme::RADIUS_MD)
                    .bg(theme::BG_PANEL)
                    .border_1()
                    .border_color(theme::BORDER)
                    .child(div().text_size(px(16.0)).font_weight(gpui::FontWeight::BOLD).text_color(theme::MAIN_TEXT).child("Allodium"))
                    .child(div().text_size(px(11.0)).text_color(cx.theme().muted_foreground).child("Open an existing project, or create a new one."))
                    .child(div().flex().flex_col().gap_2().when(rows.is_empty(), |d| {
                        d.child(div().text_size(px(11.0)).text_color(cx.theme().muted_foreground).child("No projects yet in data/projects/."))
                    }).children(rows))
                    .when_some(self.error.clone(), |d, err| {
                        d.child(div().text_size(px(11.0)).text_color(theme::destructive()).child(err))
                    })
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().flex_1().child(gpui::component::input::Input::new(&self.new_name_input)))
                            .child(
                                Button::new("create-project")
                                    .primary()
                                    .small()
                                    .icon(IconName::Plus)
                                    .label("Create")
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.create_project(cx))),
                            ),
                    ),
            )
    }
}

/// The window's actual root view (see `main.rs`): either the project
/// menu above or the fully-opened `AppShell`, switching exactly once per
/// session when `ProjectMenuEvent::Open` fires.
pub struct AppRoot {
    engine: Arc<AudioEngine>,
    state: RootState,
}

enum RootState {
    Menu(Entity<ProjectMenu>),
    Shell(Entity<AppShell>),
}

impl AppRoot {
    pub fn new(engine: Arc<AudioEngine>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Skips the menu entirely when a last-opened project is on
        // record (and still exists) — a session only ever sees the menu
        // on its first run, or after that project's folder is deleted
        // out from under it. See `project::last_opened_project`'s own
        // doc comment.
        if let Some(name) = project::last_opened_project() {
            let state = RootState::Shell(cx.new(|cx| AppShell::new(engine.clone(), name, window, cx)));
            return Self { engine, state };
        }
        let menu = cx.new(|cx| ProjectMenu::new(window, cx));
        cx.subscribe_in(&menu, window, |this: &mut Self, _menu, event: &ProjectMenuEvent, window, cx| match event {
            ProjectMenuEvent::Open(name) => this.open_project(name.clone(), window, cx),
        })
        .detach();
        Self { engine, state: RootState::Menu(menu) }
    }

    fn open_project(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        project::record_last_opened(&name);
        let engine = self.engine.clone();
        let shell = cx.new(|cx| AppShell::new(engine, name, window, cx));
        self.state = RootState::Shell(shell);
        cx.notify();
    }
}

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        match &self.state {
            RootState::Menu(menu) => menu.clone().into_any_element(),
            RootState::Shell(shell) => shell.clone().into_any_element(),
        }
    }
}
