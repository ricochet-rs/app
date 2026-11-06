//! The window: server controls on top, then a sidebar of panes beside the pane being edited.

use super::components::{
    callout, card, card_title, danger_button, error_alert, heading, prose, rule, segmented,
    selection_mark, setting_row, tab_bar,
};
use super::icons::ph;
use super::section::SetupSection;
use crate::config::section::{Field as _, SettingsSection as _};
use crate::config::{
    AuthenticationSettings, Encryption, LogLevel, LogRotation, PackageIndex, Provider, RRepository,
    Role, ServerConfig,
};
use crate::controller::{Failure, ServerController, ServerStatus};
use crate::person::Person;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, ClipboardItem, Context, Div, Entity, FontWeight, ScrollHandle, Subscription,
    Window, div, px,
};
use gpui_component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_component::dialog::DialogButtonProps;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::select::{Select, SelectEvent, SelectState};
use gpui_component::switch::Switch;
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, IndexPath, Root, Sizable, WindowExt as _, h_flex,
    v_flex,
};
use std::collections::HashMap;

mod explorer;
mod logs;
mod onboarding;

type Access = Box<dyn Fn(&mut ServerConfig) -> Option<&mut String>>;

const UNINSTALL_WARNING: &str = "`ricochet` and all of its folders will be deleted.";

const REVERSE_PROXY_DOCS: &str =
    "https://docs.ricochet.rs/latest/admin/installation/4-reverse-proxy/";

/// A text field and the answer in the config it edits.
struct Binding {
    input: Entity<InputState>,
    access: Access,
}

/// A dropdown and where its answer sits among the options, read from the config.
struct SelectBinding {
    state: Entity<SelectState<Vec<&'static str>>>,
    position: Box<dyn Fn(&ServerConfig) -> usize>,
}

/// A screen of first-run setup.
#[derive(Clone, Copy, PartialEq)]
enum Step {
    Welcome,
    Target,
    SignIn,
    License,
    Review,
    Installing,
    Done,
}

#[derive(Clone, Copy, PartialEq)]
enum UsersTab {
    Settings,
    People,
}

#[derive(Clone, Copy, PartialEq)]
enum LoggingTab {
    Settings,
    Live,
}

enum People {
    Idle,
    Loading,
    Loaded(Vec<Person>),
}

pub struct SetupView {
    server: Entity<ServerController>,
    /// The count of config reads from the computer being managed that the fields already show.
    seen_generation: u64,
    /// Whether to keep a remote ricochet running after you log out of that computer.
    keep_running: bool,
    page_scroll: ScrollHandle,
    machine_scroll: ScrollHandle,
    explorer_scroll: ScrollHandle,
    host_input: Entity<InputState>,
    /// The first-run screen being shown, or `None` once setup is behind you.
    onboarding: Option<Step>,
    section: SetupSection,
    users_tab: UsersTab,
    logging_tab: LoggingTab,
    log_tail: logs::LogTail,
    /// Reads the log every two seconds while the Live tab shows, and stops when dropped.
    log_task: Option<gpui::Task<()>>,
    log_scroll: ScrollHandle,
    /// Log levels the Live tab hides, such as `"DEBUG"`.
    hidden_levels: Vec<&'static str>,
    /// The window's height at the last render, so the log view can fill what is left.
    viewport_height: gpui::Pixels,
    server_menu_open: bool,
    /// What had focus when the current click began, so a click that focuses nothing can clear it.
    focus_at_press: std::rc::Rc<std::cell::RefCell<Option<gpui::FocusHandle>>>,
    config: ServerConfig,
    /// The issuer ricochet signs people in with now, or `None` before the first setup.
    saved_issuer: Option<String>,
    problem: Option<String>,
    chose_own_provider: bool,
    /// Whether managed auth is selected but still waits for **I agree**.
    considering_managed: bool,
    configuring_own_provider: bool,
    /// Whether a save is waiting to finish, so its issuer becomes the saved one on success.
    submitted: bool,
    inputs: HashMap<&'static str, Entity<InputState>>,
    bindings: Vec<Binding>,
    selects: HashMap<&'static str, SelectBinding>,
    r_rows: Vec<[Entity<InputState>; 2]>,
    julia_rows: Vec<Entity<InputState>>,
    python_rows: Vec<Entity<InputState>>,
    row_bindings: Vec<Binding>,
    people: People,
    people_failure: Option<Failure>,
    /// The folder under ricochet's home being browsed in the Danger zone, or `None` while the browser is closed.
    explorer: Option<explorer::Explorer>,
    delete_choices: Vec<crate::backend::Data>,
    /// Whether to list the browsed folder again once the operation running now finishes.
    reload_explorer_when_idle: bool,
    changing_role: bool,
    subscriptions: Vec<Subscription>,
    row_subscriptions: Vec<Subscription>,
}

impl SetupView {
    pub fn new(
        server: Entity<ServerController>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let saved = server.read(cx).saved.clone();
        let saved_issuer = saved.as_ref().map(|config| config.authentication.issuer());
        let first_run = !server.read(cx).is_configured;
        let host_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("my-server or user@host"));
        let mut view = Self {
            onboarding: first_run.then_some(Step::Welcome),
            seen_generation: 0,
            keep_running: true,
            page_scroll: ScrollHandle::default(),
            machine_scroll: ScrollHandle::default(),
            explorer_scroll: ScrollHandle::default(),
            host_input,
            section: SetupSection::Authentication,
            users_tab: UsersTab::Settings,
            logging_tab: LoggingTab::Settings,
            log_tail: logs::LogTail::default(),
            log_task: None,
            log_scroll: ScrollHandle::default(),
            hidden_levels: Vec::new(),
            viewport_height: window.viewport_size().height,
            server_menu_open: true,
            focus_at_press: Default::default(),
            config: saved.unwrap_or_default(),
            saved_issuer,
            problem: None,
            chose_own_provider: false,
            considering_managed: false,
            configuring_own_provider: false,
            submitted: false,
            inputs: HashMap::new(),
            bindings: Vec::new(),
            selects: HashMap::new(),
            r_rows: Vec::new(),
            julia_rows: Vec::new(),
            python_rows: Vec::new(),
            row_bindings: Vec::new(),
            people: People::Idle,
            people_failure: None,
            explorer: None,
            delete_choices: Vec::new(),
            reload_explorer_when_idle: false,
            changing_role: false,
            subscriptions: vec![cx.observe_in(&server, window, |this, server, window, cx| {
                if this.reload_explorer_when_idle && server.read(cx).activity.is_none() {
                    this.reload_explorer_when_idle = false;
                    this.reload_explorer(cx);
                }
                if this.submitted && server.read(cx).activity.is_none() {
                    this.submitted = false;
                    if server.read(cx).failure.is_none() {
                        this.saved_issuer = Some(this.config.authentication.issuer());
                        if this.onboarding == Some(Step::Installing) {
                            this.onboarding = Some(Step::Done);
                        }
                    }
                }
                if server.read(cx).generation != this.seen_generation {
                    this.show_computer(window, cx);
                }
                cx.notify();
            })],
            row_subscriptions: Vec::new(),
            server,
        };
        view.register_inputs(window, cx);
        view.register_selects(window, cx);
        view.rebuild_rows(window, cx);
        view.sync_inputs(window, cx);
        view
    }

    fn register_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        macro_rules! bind {
            ($key:literal, $placeholder:literal, $secret:literal, $($path:tt)+) => {
                self.bind($key, $placeholder, $secret, Box::new(|c: &mut ServerConfig| Some(&mut c.$($path)+)), window, cx)
            };
        }
        bind!(
            "auth.issuer_url",
            "https://auth.example.com",
            false,
            authentication.issuer_url
        );
        bind!("auth.client_id", "", false, authentication.client_id);
        bind!("auth.client_secret", "", true, authentication.client_secret);
        bind!(
            "auth.display_name_claim",
            "preferred_username",
            false,
            authentication.display_name_claim
        );
        bind!(
            "auth.groups_claim",
            "groups",
            false,
            authentication.groups_claim
        );
        bind!(
            "auth.sync_interval",
            "Minutes",
            false,
            authentication.sync_interval
        );
        bind!("server.port", "6188", false, server.port);
        bind!(
            "server.license_key",
            "Paste your license key",
            false,
            server.license_key
        );
        bind!("email.host", "smtp.example.com", false, email.host);
        bind!("email.port", "587", false, email.port);
        bind!("email.username", "", false, email.username);
        bind!("email.password", "", true, email.password);
        bind!(
            "email.from",
            "ricochet <ricochet@example.com>",
            false,
            email.from
        );
        bind!("email.reply_to", "", false, email.reply_to);
        bind!(
            "deployments.max_deployments",
            "5",
            false,
            deployments.max_deployments
        );
        bind!(
            "deployments.max_age_days",
            "Forever",
            false,
            deployments.max_age_days
        );
        bind!(
            "deployments.max_bundle_size",
            "100MB",
            false,
            deployments.max_bundle_size
        );
        bind!("tasks.max_duration", "60", false, tasks.max_duration);
        bind!(
            "tasks.restore_timeout",
            "Default",
            false,
            tasks.restore_timeout
        );
        bind!(
            "tasks.restore_concurrency",
            "4",
            false,
            tasks.restore_concurrency
        );
        bind!("logging.max_files", "All", false, logging.max_files);
        bind!(
            "logging.max_age_days",
            "Forever",
            false,
            logging.max_age_days
        );
        bind!(
            "audit.retention_days",
            "365",
            false,
            audit_log.retention_days
        );
        bind!(
            "telemetry.endpoint",
            "http://localhost:4317",
            false,
            telemetry.endpoint
        );
    }

    fn new_input(
        &mut self,
        placeholder: &str,
        secret: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<InputState>, Subscription) {
        let placeholder = placeholder.to_owned();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder)
                .masked(secret)
        });
        let subscription = cx.subscribe(&input, |this, input, event: &InputEvent, cx| {
            this.on_input(&input, event, cx)
        });
        (input, subscription)
    }

    fn bind(
        &mut self,
        key: &'static str,
        placeholder: &str,
        secret: bool,
        access: Access,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (input, subscription) = self.new_input(placeholder, secret, window, cx);
        self.subscriptions.push(subscription);
        self.inputs.insert(key, input.clone());
        self.bindings.push(Binding { input, access });
    }

    fn row_input(
        &mut self,
        placeholder: &str,
        access: Access,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let (input, subscription) = self.new_input(placeholder, false, window, cx);
        self.row_subscriptions.push(subscription);
        self.row_bindings.push(Binding {
            input: input.clone(),
            access,
        });
        input
    }

    /// Makes one text field per package source in the config, after the list changes.
    fn rebuild_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.row_subscriptions.clear();
        self.row_bindings.clear();
        let (r, julia, python) = (
            self.config.packages.r.len(),
            self.config.packages.julia.len(),
            self.config.packages.python.len(),
        );
        self.r_rows = (0..r)
            .map(|i| {
                let name = self.row_input(
                    "CRAN",
                    Box::new(move |c| c.packages.r.get_mut(i).map(|r| &mut r.name)),
                    window,
                    cx,
                );
                let url = self.row_input(
                    "https://cloud.r-project.org",
                    Box::new(move |c| c.packages.r.get_mut(i).map(|r| &mut r.url)),
                    window,
                    cx,
                );
                [name, url]
            })
            .collect();
        self.julia_rows = (0..julia)
            .map(|i| {
                self.row_input(
                    "https://github.com/JuliaRegistries/General.git",
                    Box::new(move |c| c.packages.julia.get_mut(i).map(|j| &mut j.url)),
                    window,
                    cx,
                )
            })
            .collect();
        self.python_rows = (0..python)
            .map(|i| {
                self.row_input(
                    "https://pypi.org/simple",
                    Box::new(move |c| c.packages.python.get_mut(i).map(|p| &mut p.url)),
                    window,
                    cx,
                )
            })
            .collect();
    }

    /// Shows the answers in the config in their text fields.
    fn sync_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for binding in self.bindings.iter().chain(&self.row_bindings) {
            let value = (binding.access)(&mut self.config).map(|value| value.clone());
            if let Some(value) = value {
                binding
                    .input
                    .update(cx, |input, cx| input.set_value(value, window, cx));
            }
        }
        for select in self.selects.values() {
            let selected = Some(IndexPath::new((select.position)(&self.config)));
            select.state.update(cx, |state, cx| {
                state.set_selected_index(selected, window, cx)
            });
        }
        self.sync_email_port_hint(window, cx);
    }

    fn sync_email_port_hint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let hint = match self.config.email.encryption {
            Encryption::StartTls => "587",
            Encryption::Tls => "465",
            Encryption::None => "25",
        };
        if let Some(input) = self.inputs.get("email.port") {
            input.update(cx, |input, cx| input.set_placeholder(hint, window, cx));
        }
    }

    fn on_input(&mut self, input: &Entity<InputState>, event: &InputEvent, cx: &mut Context<Self>) {
        if !matches!(event, InputEvent::Change) {
            return;
        }
        let value = input.read(cx).value().to_string();
        let Some(binding) = self
            .bindings
            .iter()
            .chain(&self.row_bindings)
            .find(|b| &b.input == input)
        else {
            return;
        };
        if let Some(answer) = (binding.access)(&mut self.config)
            && *answer != value
        {
            *answer = value;
            self.edited(cx);
        }
    }

    /// Clears the notice about earlier answers, because the answers changed.
    fn edited(&mut self, cx: &mut Context<Self>) {
        self.problem = None;
        cx.notify();
    }

    fn select(&mut self, section: SetupSection, cx: &mut Context<Self>) {
        self.section = section;
        if section == SetupSection::Users && self.users_tab == UsersTab::People {
            self.load_people(cx);
        }
        if section == SetupSection::Logging && self.logging_tab == LoggingTab::Live {
            self.follow_logs(cx);
        } else {
            self.stop_following_logs();
        }
        cx.notify();
    }

    /// Shows the answers saved on the computer just connected to, and moves to whatever it needs next.
    fn show_computer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let server = self.server.read(cx);
        self.seen_generation = server.generation;
        let configured = server.is_configured;
        self.config = server.saved.clone().unwrap_or_default();
        self.saved_issuer = server.saved.as_ref().map(|c| c.authentication.issuer());
        self.people = People::Idle;
        self.people_failure = None;
        self.explorer = None;
        self.delete_choices.clear();
        self.stop_following_logs();
        self.log_tail = logs::LogTail::default();
        if self.section == SetupSection::Logging && self.logging_tab == LoggingTab::Live {
            self.follow_logs(cx);
        }
        self.problem = None;
        self.considering_managed = false;
        self.rebuild_rows(window, cx);
        self.sync_inputs(window, cx);
        self.onboarding = match (configured, self.onboarding) {
            (true, _) => None,
            (false, Some(Step::Installing | Step::Done | Step::Review | Step::License)) => {
                self.onboarding
            }
            (false, _) => Some(Step::SignIn),
        };
    }

    fn load_people(&mut self, cx: &mut Context<Self>) {
        if !self.server.read(cx).is_configured {
            return;
        }
        let backend = self.server.read(cx).backend();
        self.people = People::Loading;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { Person::all(backend.as_ref()) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(people) => this.people = People::Loaded(people),
                    Err(error) => {
                        this.people = People::Idle;
                        this.people_failure = Some(Failure {
                            title: "Couldn't load users".into(),
                            message: error.to_string(),
                        });
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn change_role(&mut self, person: Person, role: Role, cx: &mut Context<Self>) {
        if role == person.role {
            return;
        }
        self.changing_role = true;
        let backend = self.server.read(cx).backend();
        cx.spawn(async move |this, cx| {
            let name = person.name().to_owned();
            let result = cx
                .background_executor()
                .spawn(async move { person.set_role(backend.as_ref(), role) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.changing_role = false;
                this.people_failure = result.err().map(|error| Failure {
                    title: format!("Couldn't change {name}'s role"),
                    message: error.to_string(),
                });
                this.load_people(cx);
            });
        })
        .detach();
    }

    /// Starts the install, or opens the first pane that still needs an answer.
    /// What stops the authentication answers from being saved, counting a managed choice still waiting for **I agree**.
    fn sign_in_problem(&self) -> Option<String> {
        if self.considering_managed && !self.config.authentication.managed {
            return Some("Select I agree to use managed authentication.".into());
        }
        self.config.authentication.problem()
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if let Some(problem) = self.sign_in_problem() {
            self.select(SetupSection::Authentication, cx);
            self.problem = Some(problem);
            cx.notify();
            return;
        }
        if let Some((section, problem)) = SetupSection::first_problem(&self.config) {
            self.select(section, cx);
            self.problem = Some(problem);
            cx.notify();
            return;
        }
        self.submitted = true;
        let config = self.config.clone();
        let keep_running = self.keep_running && self.server.read(cx).connection.is_remote();
        self.server
            .update(cx, |server, cx| server.set_up(config, keep_running, cx));
    }

    fn save_title(&self, cx: &App) -> &'static str {
        let server = self.server.read(cx);
        if server.is_configured {
            "Save and restart"
        } else if server.installed.is_none() {
            "Install and start"
        } else {
            "Save and start"
        }
    }

    // MARK: Controls

    fn controls(&self, cx: &mut Context<Self>) -> Div {
        let server = self.server.read(cx);
        let busy = server.activity.is_some();
        let (label, color) = match server.status {
            ServerStatus::Running => ("Running", cx.theme().success),
            ServerStatus::Starting => ("Starting", cx.theme().warning),
            ServerStatus::Stopped => ("Stopped", cx.theme().muted_foreground),
            ServerStatus::NotInstalled => ("Not installed", cx.theme().muted_foreground),
        };
        let checking = server.checking;
        let (label, version) = if checking {
            ("Checking", "\u{a0}".to_owned())
        } else {
            let version = server
                .installed
                .as_ref()
                .map_or("\u{a0}".to_owned(), |release| {
                    format!("Version {}", release.version)
                });
            (label, version)
        };
        let marker = if checking || busy {
            gpui_component::spinner::Spinner::new()
                .xsmall()
                .into_any_element()
        } else {
            div()
                .size(px(6.0))
                .rounded_full()
                .bg(color)
                .into_any_element()
        };
        let (running, stopped, configured) = (
            server.status == ServerStatus::Running,
            server.status == ServerStatus::Stopped,
            server.is_configured,
        );
        let update = server.available_update();
        let connection_name = server.connection.name();
        let connection_icon = if server.connection.is_remote() {
            "hard-drives"
        } else {
            "desktop"
        };
        let server_entity = self.server.clone();
        let act = move |f: fn(&mut ServerController, &mut Context<ServerController>)| {
            let server = server_entity.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| server.update(cx, f)
        };

        h_flex()
            .gap_3()
            .px_4()
            .py_3()
            .items_center()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(prose("ricochet")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(version),
                    ),
            )
            .child(
                h_flex()
                    .gap_1p5()
                    .px_2()
                    .py_0p5()
                    .items_center()
                    .border_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .child(marker)
                    .child(label),
            )
            .child(self.machine_menu(connection_icon, connection_name, cx))
            .child(div().flex_1())
            .children(update.map(|release| {
                let server = self.server.clone();
                Button::new("update")
                    .icon(ph("arrow-circle-up"))
                    .label(format!("Update to {}", release.version))
                    .outline()
                    .small()
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        server.update(cx, |server, cx| server.update_to(release.clone(), cx))
                    })
            }))
            .when(configured, |row| {
                row.child(
                    Button::new("open")
                        .icon(ph("arrow-square-out"))
                        .label("Open")
                        .primary()
                        .small()
                        .disabled(!running || busy)
                        .on_click(act(|server, cx| server.open_dashboard(cx))),
                )
                .child(if stopped {
                    Button::new("start")
                        .icon(ph("play"))
                        .tooltip("Start")
                        .outline()
                        .small()
                        .disabled(busy)
                        .on_click(act(|s, cx| s.start(cx)))
                } else {
                    Button::new("stop")
                        .icon(ph("stop"))
                        .tooltip("Stop")
                        .outline()
                        .small()
                        .disabled(busy)
                        .on_click(act(|s, cx| s.stop(cx)))
                })
                .child(
                    Button::new("restart")
                        .icon(ph("arrow-clockwise"))
                        .tooltip("Restart")
                        .outline()
                        .small()
                        .disabled(stopped || busy)
                        .on_click(act(|s, cx| s.restart(cx))),
                )
            })
    }

    /// The computers you can switch to, shown under the bar while it is open.
    /// The machine button, which drops a menu of every machine, a check on the current one, and **Add machine**.
    fn machine_menu(
        &self,
        icon: &'static str,
        name: String,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let server = self.server.read(cx);
        let active = server.connection.clone();
        let busy = server.activity.is_some();
        let choices = server.choices();
        let view = cx.entity();
        Button::new("connection")
            .icon(ph(icon))
            .label(name)
            .outline()
            .small()
            .disabled(busy)
            .dropdown_menu(move |mut menu, _, _| {
                for choice in &choices {
                    let target = choice.clone();
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(choice.name())
                            .icon(ph(if choice.is_remote() {
                                "hard-drives"
                            } else {
                                "desktop"
                            }))
                            .checked(*choice == active)
                            .on_click(move |_, _, cx| {
                                let target = target.clone();
                                view.update(cx, |this, cx| {
                                    this.server
                                        .update(cx, |server, cx| server.switch(target, cx));
                                });
                            }),
                    );
                }
                let view = view.clone();
                menu.separator()
                    .item(PopupMenuItem::new("Add machine").icon(ph("plus")).on_click(
                        move |_, _, cx| {
                            view.update(cx, |this, cx| {
                                this.onboarding = Some(Step::Target);
                                cx.notify();
                            });
                        },
                    ))
            })
    }

    // MARK: Sidebar

    /// One sidebar entry that opens `section`, with a red dot while that pane needs an answer.
    fn sidebar_row(
        &self,
        id: &'static str,
        icon: &'static str,
        label: &'static str,
        active: bool,
        needs_answer: bool,
        cx: &Context<Self>,
    ) -> gpui::Stateful<Div> {
        let accent = cx.theme().sidebar_accent;
        h_flex()
            .id(id)
            .h_7()
            .px_2()
            .gap_2()
            .items_center()
            .text_sm()
            .cursor_pointer()
            .map(|row| {
                if active {
                    row.bg(accent).font_weight(FontWeight::MEDIUM)
                } else {
                    row.hover(move |row| row.bg(accent.opacity(0.8)))
                }
            })
            .child(ph(icon).size_4().text_color(cx.theme().muted_foreground))
            .child(div().flex_1().truncate().child(label))
            .when(needs_answer, |row| {
                row.child(div().size(px(6.0)).rounded_full().bg(cx.theme().danger))
            })
    }

    fn sidebar_item(
        &self,
        section: SetupSection,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        self.sidebar_row(
            label,
            section.icon(),
            label,
            self.section == section,
            section.problem(&self.config).is_some(),
            cx,
        )
        .on_click(cx.listener(move |this, _, _, cx| this.select(section, cx)))
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let group_label = |label: &'static str| {
            div()
                .px_2()
                .pt_4()
                .pb_1()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(muted)
                .child(label)
        };
        let open = self.server_menu_open;
        let on_server_page = SetupSection::SERVER_PAGES.contains(&self.section);
        let server_needs_answer = SetupSection::SERVER_PAGES
            .iter()
            .any(|section| section.problem(&self.config).is_some());
        let server = self
            .sidebar_row(
                "Server",
                SetupSection::Server.icon(),
                "Server",
                on_server_page && !open,
                server_needs_answer && !open,
                cx,
            )
            .child(
                Icon::new(if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .size_3p5()
                .text_color(muted),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.server_menu_open = !this.server_menu_open;
                if this.server_menu_open && !SetupSection::SERVER_PAGES.contains(&this.section) {
                    this.select(SetupSection::Server, cx);
                }
                cx.notify();
            }));
        let server_pages = open.then(|| {
            v_flex()
                .ml_3p5()
                .pl_2()
                .gap_0p5()
                .border_l_1()
                .border_color(cx.theme().sidebar_border)
                .children(SetupSection::SERVER_PAGES.into_iter().map(|section| {
                    let label = if section == SetupSection::Server {
                        "General"
                    } else {
                        section.title()
                    };
                    self.sidebar_item(section, label, cx)
                }))
        });
        v_flex()
            .w(px(220.0))
            .flex_shrink_0()
            .h_full()
            .px_3()
            .pb_3()
            .gap_0p5()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .child(group_label("Required"))
            .child(self.sidebar_item(
                SetupSection::Authentication,
                SetupSection::Authentication.title(),
                cx,
            ))
            .child(server)
            .children(server_pages)
            .child(group_label("Optional"))
            .children(
                SetupSection::OPTIONAL
                    .into_iter()
                    .map(|section| self.sidebar_item(section, section.title(), cx)),
            )
    }

    // MARK: Detail

    fn detail(&self, cx: &mut Context<Self>) -> Div {
        let server = self.server.read(cx);
        let activity = server.activity.clone();
        let failure = server.failure.clone();
        let busy = activity.is_some();
        let remote_host = server
            .connection
            .is_remote()
            .then(|| server.connection.name());
        let pane = match self.section {
            SetupSection::Authentication => self.authentication_pane(cx),
            SetupSection::Server => self.server_pane(cx),
            SetupSection::Users => self.users_pane(cx),
            SetupSection::Email => self.email_pane(cx),
            SetupSection::Packages => self.packages_pane(cx),
            SetupSection::Deployments => self.deployments_pane(cx),
            SetupSection::Tasks => self.tasks_pane(cx),
            SetupSection::Logging => self.logging_pane(cx),
            SetupSection::AuditLog => self.audit_pane(cx),
            SetupSection::Telemetry => self.telemetry_pane(cx),
            SetupSection::DangerZone => self.danger_zone_pane(cx),
        };

        let alert = if let Some(problem) = &self.problem {
            Some(error_alert(
                "Fix these settings",
                problem,
                cx.listener(|this, _, _, cx| {
                    this.problem = None;
                    cx.notify();
                }),
                cx,
            ))
        } else {
            failure.map(|failure| {
                let server = self.server.clone();
                error_alert(
                    &failure.title,
                    &failure.message,
                    move |_, _, cx| server.update(cx, |server, cx| server.dismiss_failure(cx)),
                    cx,
                )
            })
        };

        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                div()
                    .id("pane")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        v_flex()
                            .gap_6()
                            .p_6()
                            .child(heading(
                                self.section.title(),
                                self.section.summary(remote_host.as_deref()).as_deref(),
                                cx,
                            ))
                            .child(pane),
                    ),
            )
            .child(rule(cx))
            .child(
                v_flex().gap_3().p_4().children(alert).child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .children(activity.map(|activity| {
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(activity)
                        }))
                        .child(div().flex_1())
                        .child(
                            Button::new("save")
                                .icon(ph("rocket-launch"))
                                .label(self.save_title(cx))
                                .primary()
                                .loading(busy)
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                        ),
                ),
            )
    }

    // MARK: Field helpers

    fn input(&self, key: &'static str) -> Input {
        Input::new(&self.inputs[key])
    }

    fn text_field(
        &self,
        label: &'static str,
        key: &'static str,
        description: Option<&str>,
        cx: &App,
    ) -> Div {
        setting_row(
            label,
            false,
            description.map(str::to_owned),
            self.input(key),
            cx,
        )
    }

    /// A short field for a number or size, right-aligned, with its unit after the value.
    fn number_field(
        &self,
        label: &'static str,
        key: &'static str,
        unit: Option<&'static str>,
        description: Option<&str>,
        cx: &App,
    ) -> Div {
        let muted = cx.theme().muted_foreground;
        let input = self.input(key).when_some(unit, |input, unit| {
            input.suffix(div().text_xs().text_color(muted).child(unit))
        });
        setting_row(
            label,
            false,
            description.map(str::to_owned),
            h_flex()
                .justify_end()
                .child(div().w(px(140.0)).child(input)),
            cx,
        )
    }

    fn optional_text_field(
        &self,
        label: &'static str,
        key: &'static str,
        description: Option<&str>,
        cx: &App,
    ) -> Div {
        setting_row(
            label,
            true,
            description.map(str::to_owned),
            self.input(key),
            cx,
        )
    }

    fn toggle(
        &self,
        id: &'static str,
        label: &'static str,
        description: Option<&str>,
        checked: bool,
        cx: &mut Context<Self>,
        set: impl Fn(&mut ServerConfig, bool) + 'static,
    ) -> Div {
        h_flex()
            .gap_4()
            .items_start()
            .justify_between()
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(label))
                    .children(description.map(|description| {
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(description.to_owned())
                    })),
            )
            .child(Switch::new(id).checked(checked).on_click(cx.listener(
                move |this, checked: &bool, _, cx| {
                    set(&mut this.config, *checked);
                    this.edited(cx);
                },
            )))
    }

    /// Makes the dropdown for one of `all`, which reads its answer with `get` and saves a pick with `set`.
    #[allow(clippy::too_many_arguments)]
    fn bind_select<T: Copy + PartialEq + 'static>(
        &mut self,
        id: &'static str,
        all: &'static [T],
        title: fn(T) -> &'static str,
        get: fn(&ServerConfig) -> T,
        set: fn(&mut SetupView, T, &mut Window, &mut Context<SetupView>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let position = move |config: &ServerConfig| {
            all.iter()
                .position(|option| *option == get(config))
                .unwrap_or(0)
        };
        let titles: Vec<&'static str> = all.iter().map(|option| title(*option)).collect();
        let selected = Some(IndexPath::new(position(&self.config)));
        let state = cx.new(|cx| SelectState::new(titles, selected, window, cx));
        self.subscriptions.push(cx.subscribe_in(
            &state,
            window,
            move |this, _, event: &SelectEvent<Vec<&'static str>>, window, cx| {
                let SelectEvent::Confirm(Some(picked)) = event else {
                    return;
                };
                if let Some(option) = all.iter().find(|option| title(**option) == *picked) {
                    set(this, *option, window, cx);
                }
            },
        ));
        self.selects.insert(
            id,
            SelectBinding {
                state,
                position: Box::new(position),
            },
        );
    }

    fn register_selects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.bind_select(
            "default-role",
            Role::ALL,
            Role::title,
            |config| config.users.default_role,
            |this, role, _, cx| {
                this.config.users.default_role = role;
                this.edited(cx);
            },
            window,
            cx,
        );
        self.bind_select(
            "encryption",
            Encryption::ALL,
            Encryption::title,
            |config| config.email.encryption,
            |this, encryption, window, cx| {
                this.config.email.encryption = encryption;
                this.sync_email_port_hint(window, cx);
                this.edited(cx);
            },
            window,
            cx,
        );
        self.bind_select(
            "log-level",
            LogLevel::ALL,
            LogLevel::title,
            |config| config.logging.level,
            |this, level, _, cx| {
                this.config.logging.level = level;
                this.edited(cx);
            },
            window,
            cx,
        );
        self.bind_select(
            "log-rotation",
            LogRotation::ALL,
            LogRotation::title,
            |config| config.logging.rotation,
            |this, rotation, _, cx| {
                this.config.logging.rotation = rotation;
                this.edited(cx);
            },
            window,
            cx,
        );
    }

    /// A setting answered from a dropdown made by [`Self::bind_select`].
    fn choice(
        &self,
        id: &'static str,
        label: &'static str,
        description: Option<&str>,
        cx: &App,
    ) -> Div {
        setting_row(
            label,
            false,
            description.map(str::to_owned),
            h_flex().justify_end().child(
                div()
                    .w(px(200.0))
                    .child(Select::new(&self.selects[id].state)),
            ),
            cx,
        )
    }

    // MARK: Panes

    fn authentication_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let auth = &self.config.authentication;
        let changes_issuer = self
            .saved_issuer
            .as_ref()
            .is_some_and(|saved| *saved != auth.issuer());
        let uses_own_provider = !auth.managed
            && (self.chose_own_provider
                || auth.issuer_url.non_empty().is_some()
                || auth.client_id.non_empty().is_some());

        let warning = changes_issuer.then(|| {
            callout(
                "Existing users won't carry over",
                "Everyone who signs in through the new provider gets a new account with the default role. Assign roles under Users.",
                cx.theme().warning,
                vec![
                    Button::new("open-users")
                        .icon(ph("users"))
                        .label("Open Users")
                        .outline()
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.users_tab = UsersTab::People;
                            this.select(SetupSection::Users, cx);
                        }))
                        .into_any_element(),
                ],
                cx,
            )
        });

        let body = if self.configuring_own_provider {
            self.own_provider(cx).into_any_element()
        } else {
            let managed_selected = auth.managed || self.considering_managed;
            let card = |id: &'static str,
                        icon: &'static str,
                        title: &'static str,
                        message: &'static str,
                        selected: bool,
                        cx: &App| {
                let ring = cx.theme().ring;
                let hover = cx.theme().muted_foreground;
                v_flex()
                    .id(id)
                    .flex_1()
                    .min_w_0()
                    .gap_4()
                    .p_5()
                    .cursor_pointer()
                    .bg(if selected {
                        ring.opacity(0.07)
                    } else {
                        super::theme::Palette::card(cx)
                    })
                    .border_1()
                    .border_color(if selected { ring } else { cx.theme().border })
                    .when(!selected, |card| {
                        card.hover(move |card| card.border_color(hover))
                    })
                    .child(
                        h_flex()
                            .justify_between()
                            .items_start()
                            .child(
                                div()
                                    .size(px(40.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(if selected {
                                        ring.opacity(0.16)
                                    } else {
                                        cx.theme().muted
                                    })
                                    .child(ph(icon).size_5().text_color(if selected {
                                        ring
                                    } else {
                                        cx.theme().foreground
                                    })),
                            )
                            .child(selection_mark(selected, cx)),
                    )
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                            .child(
                                div()
                                    .w_full()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(prose(message)),
                            ),
                    )
            };
            let consent = managed_selected.then(|| {
                let privacy = Button::new("privacy")
                    .label("Privacy Policy")
                    .link()
                    .xsmall()
                    .on_click(|_, _, _| {
                        let _ = open::that(AuthenticationSettings::PRIVACY_POLICY);
                    });
                let answer = if auth.managed {
                    h_flex()
                        .gap_1p5()
                        .flex_shrink_0()
                        .text_sm()
                        .text_color(cx.theme().success)
                        .child(ph("check-circle").size_4())
                        .child("You agreed")
                        .into_any_element()
                } else {
                    Button::new("agree")
                        .label("I agree")
                        .primary()
                        .small()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.config.authentication.managed = true;
                            this.considering_managed = false;
                            this.config.server.port =
                                AuthenticationSettings::MANAGED_PORT.to_owned();
                            this.sync_inputs(window, cx);
                            if this.onboarding == Some(Step::SignIn) {
                                this.onboarding = Some(Step::License);
                            }
                            this.edited(cx);
                        }))
                        .into_any_element()
                };
                h_flex()
                    .gap_4()
                    .px_4()
                    .py_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(super::theme::Palette::card(cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .items_start()
                            .child(div().w_full().text_sm().child(prose(
                                "You share your name and email address with ricochet when you sign in.",
                            )))
                            .child(privacy),
                    )
                    .child(answer)
            });
            v_flex()
                .gap_3()
                .max_w(px(760.0))
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            card(
                                "managed",
                                "shield-check",
                                "Managed authentication",
                                "Get up and running with our identity provider. Intended for local development only.",
                                managed_selected,
                                cx,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                if !this.config.authentication.managed {
                                    this.considering_managed = true;
                                    this.problem = None;
                                    cx.notify();
                                }
                            })),
                        )
                        .child(
                            card(
                                "own",
                                "key",
                                "Configure provider",
                                "Bring your own identity provider. Sign in through any OpenID Connect provider, such as Auth0, Clerk, Okta or Microsoft Entra ID.",
                                uses_own_provider && !self.considering_managed,
                                cx,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.config.authentication.managed = false;
                                this.considering_managed = false;
                                this.chose_own_provider = true;
                                this.configuring_own_provider = true;
                                this.problem = None;
                                this.edited(cx);
                            })),
                        ),
                )
                .children(consent)
                .into_any_element()
        };

        v_flex()
            .gap_5()
            .children(warning)
            .child(body)
            .into_any_element()
    }

    fn own_provider(&self, cx: &mut Context<Self>) -> Div {
        let provider = self.config.authentication.provider;
        let redirect_url = self.config.redirect_url();
        let copied = redirect_url.clone();
        let ring = cx.theme().ring;
        let chosen = h_flex()
            .gap_3()
            .px_4()
            .py_3()
            .items_center()
            .border_1()
            .border_color(ring)
            .bg(ring.opacity(0.07))
            .child(
                div()
                    .size(px(32.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(ring.opacity(0.16))
                    .child(ph("key").size_4().text_color(ring)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Configure provider"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Bring your own identity provider."),
                    ),
            )
            .child(
                Button::new("change-authentication")
                    .label("Change")
                    .outline()
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.configuring_own_provider = false;
                        cx.notify();
                    })),
            );
        let provider_choice = segmented(
            "provider",
            Provider::ALL
                .iter()
                .map(|p| (p.title(), *p == provider))
                .collect(),
            {
                let view = cx.entity();
                move |index, window, app| {
                    view.update(app, |this, cx| {
                        this.config.authentication.provider = Provider::ALL[index];
                        let prompt = Provider::ALL[index].issuer_prompt();
                        if let Some(input) = this.inputs.get("auth.issuer_url") {
                            input.update(cx, |input, cx| input.set_placeholder(prompt, window, cx));
                        }
                        this.edited(cx);
                    });
                }
            },
        );
        let secret_field = if provider == Provider::Oidc {
            self.optional_text_field(
                "Client secret",
                "auth.client_secret",
                Some("Leave blank to use PKCE authentication."),
                cx,
            )
        } else {
            self.text_field(
                "Client secret",
                "auth.client_secret",
                Some("ricochet uses it to read groups and photos from Microsoft Entra ID."),
                cx,
            )
        };
        let redirect = v_flex()
            .gap_1p5()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child("Redirect URL"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .pl_3()
                    .pr_1()
                    .py_1()
                    .items_center()
                    .border_1()
                    .border_color(cx.theme().input)
                    .bg(cx.theme().muted)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .font_family(cx.theme().mono_font_family.clone())
                            .child(redirect_url),
                    )
                    .child(
                        Button::new("copy-redirect")
                            .icon(ph("copy"))
                            .label("Copy")
                            .ghost()
                            .small()
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))
                            }),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(prose("Add this URL to your identity provider's app registration.")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        prose(
                            "Use `localhost` for development only. In production, use an `https` URL behind a reverse proxy.",
                        )
                        .link("reverse proxy", REVERSE_PROXY_DOCS),
                    ),
            );
        v_flex()
            .gap_6()
            .max_w(px(760.0))
            .child(chosen)
            .child(
                v_flex().gap_3().child(card_title("Connection", cx)).child(
                    card(cx)
                        .child(setting_row("Provider", false, None, provider_choice, cx))
                        .child(self.text_field("Issuer URL", "auth.issuer_url", None, cx))
                        .child(self.text_field("Client ID", "auth.client_id", None, cx))
                        .child(secret_field)
                        .child(redirect),
                ),
            )
            .child(
                v_flex().gap_3().child(card_title("Claims", cx)).child(
                    card(cx)
                    .child(self.optional_text_field(
                        "Display name claim",
                        "auth.display_name_claim",
                        Some("The claim ricochet shows as each user's name."),
                        cx,
                    ))
                    .child(self.optional_text_field(
                        "Groups claim",
                        "auth.groups_claim",
                        Some("The claim that lists each user's groups."),
                        cx,
                    ))
                    .child(self.optional_text_field(
                        "Group sync interval",
                        "auth.sync_interval",
                        Some("Minutes between group refreshes. Leave empty to refresh only at sign-in."),
                        cx,
                    )),
                ),
            )
    }

    fn server_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let managed = self.config.authentication.managed;
        let installed = self.server.read(cx).installed.is_some();
        let local_url = self.config.local_url();
        let port_description = if managed {
            "Managed authentication requires port 6188.".to_owned()
        } else {
            format!("Open ricochet at {local_url}.")
        };
        let install = (!installed).then(|| {
            let server = self.server.clone();
            h_flex().child(
                Button::new("install")
                    .icon(ph("download-simple"))
                    .child(prose("Install ricochet"))
                    .primary()
                    .small()
                    .on_click(move |_, _, cx| server.update(cx, |server, cx| server.install(cx))),
            )
        });
        v_flex()
            .gap_4()
            .max_w(px(760.0))
            .child(
                card(cx)
                    .child(setting_row(
                        "Port",
                        false,
                        Some(port_description),
                        Input::new(&self.inputs["server.port"]).disabled(managed),
                        cx,
                    ))
                    .child(self.optional_text_field(
                        "License key",
                        "server.license_key",
                        Some("Leave empty for the Community Edition."),
                        cx,
                    )),
            )
            .children(install)
            .into_any_element()
    }

    fn danger_zone_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let installed = self.server.read(cx).installed.is_some();
        let controller = self.server.read(cx);
        let remote = controller.connection.is_remote();
        let home = controller.backend().paths().home.clone();
        let danger = cx.theme().danger;
        let edge = danger.opacity(0.35);
        let muted_foreground = cx.theme().muted_foreground;
        let row = move |title: &'static str, description: String, actions: Vec<AnyElement>| {
            h_flex()
                .gap_4()
                .px_4()
                .py_3()
                .items_center()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child(prose(title)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted_foreground)
                                .child(prose(description)),
                        ),
                )
                .child(h_flex().flex_shrink_0().gap_2().children(actions))
        };
        let home_text = home.display().to_string();
        let mut home_actions = vec![
            Button::new("copy-home")
                .icon(ph("copy"))
                .tooltip("Copy path")
                .ghost()
                .small()
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(home_text.clone()))
                })
                .into_any_element(),
            Button::new("browse-home")
                .label(if self.explorer.is_some() {
                    "Close"
                } else {
                    "Browse"
                })
                .outline()
                .small()
                .on_click(cx.listener(|this, _, _, cx| this.toggle_explorer(cx)))
                .into_any_element(),
        ];
        if !remote {
            let folder = home.clone();
            home_actions.push(
                Button::new("open-home")
                    .icon(ph("arrow-square-out"))
                    .label("Open folder")
                    .ghost()
                    .small()
                    .on_click(move |_, _, _| {
                        let _ = open::that(&folder);
                    })
                    .into_any_element(),
            );
        }
        let browser = self.explorer_panel(edge, cx);
        let delete_data = installed.then(|| self.delete_data_row(edge, cx));
        let uninstall_description = UNINSTALL_WARNING.to_owned();
        let uninstall_row = installed.then(|| {
            row(
                "Uninstall ricochet",
                uninstall_description.clone(),
                vec![
                    danger_button("uninstall", "Uninstall", cx)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.confirm_uninstall(window, cx)),
                        )
                        .into_any_element(),
                ],
            )
            .border_t_1()
            .border_color(edge)
        });
        v_flex()
            .max_w(px(760.0))
            .border_1()
            .border_color(edge)
            .bg(super::theme::Palette::card(cx))
            .child(row(
                "ricochet home",
                format!("`{}`", home.display()),
                home_actions,
            ))
            .children(browser)
            .children(delete_data)
            .children(uninstall_row)
            .into_any_element()
    }

    /// Asks in a dialog before uninstalling ricochet from the machine being managed.
    fn confirm_uninstall(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let message = UNINSTALL_WARNING.to_owned();
        let server = self.server.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let server = server.clone();
            dialog
                .title(prose("Uninstall ricochet?"))
                .child(prose(message.clone()))
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Uninstall")
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text("Cancel"),
                )
                .on_ok(move |_, _, cx| {
                    server.update(cx, |server, cx| server.uninstall(cx));
                    true
                })
        });
    }

    fn users_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let tab = self.users_tab;
        let view = cx.entity();
        let tabs = tab_bar(
            "users-tab",
            vec![
                ("Settings", tab == UsersTab::Settings),
                ("People", tab == UsersTab::People),
            ],
            move |index, _, app| {
                view.update(app, |this, cx| {
                    this.users_tab = if index == 0 {
                        UsersTab::Settings
                    } else {
                        UsersTab::People
                    };
                    if this.users_tab == UsersTab::People {
                        this.load_people(cx);
                    }
                    cx.notify();
                });
            },
        );
        let users = &self.config.users;
        let body = match tab {
            UsersTab::Settings => card(cx)
                .child(self.choice(
                    "default-role",
                    "Default role",
                    Some("The first person to sign in becomes an admin."),
                    cx,
                ))
                .child(self.toggle(
                    "require-sign-in",
                    "Require sign-in",
                    Some("Block anonymous visitors from every page."),
                    users.require_sign_in,
                    cx,
                    |config, on| config.users.require_sign_in = on,
                ))
                .child(self.toggle(
                    "avatars",
                    "Allow profile pictures",
                    Some("Users can upload a profile picture."),
                    users.allow_avatar_upload,
                    cx,
                    |config, on| config.users.allow_avatar_upload = on,
                )),
            UsersTab::People => self.people_list(cx),
        };
        v_flex()
            .gap_4()
            .max_w(px(760.0))
            .child(tabs)
            .child(body)
            .into_any_element()
    }

    fn people_list(&self, cx: &mut Context<Self>) -> Div {
        let configured = self.server.read(cx).is_configured;
        let failure = self.people_failure.as_ref().map(|failure| {
            error_alert(
                &failure.title,
                &failure.message,
                cx.listener(|this, _, _, cx| {
                    this.people_failure = None;
                    cx.notify();
                }),
                cx,
            )
        });
        let muted = cx.theme().muted_foreground;
        let body = match &self.people {
            People::Loaded(people) if !people.is_empty() => {
                let rows = people.iter().enumerate().map(|(index, person)| {
                    let person_for_click = person.clone();
                    let view = cx.entity();
                    let changing = self.changing_role;
                    h_flex()
                        .gap_3()
                        .px_4()
                        .py_3()
                        .items_center()
                        .when(index > 0, |row| {
                            row.border_t_1().border_color(cx.theme().border)
                        })
                        .child(
                            div()
                                .size(px(32.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(cx.theme().muted)
                                .text_color(muted)
                                .text_sm()
                                .child(
                                    person
                                        .name()
                                        .chars()
                                        .next()
                                        .map(|c| c.to_uppercase().to_string())
                                        .unwrap_or_default(),
                                ),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(person.name().to_owned()),
                                )
                                .children(
                                    person
                                        .email
                                        .as_ref()
                                        .filter(|email| email.as_str() != person.name())
                                        .map(|email| {
                                            div().text_xs().text_color(muted).child(email.clone())
                                        }),
                                ),
                        )
                        .child({
                            let current = person.role;
                            Button::new(("role", index))
                                .label(current.title())
                                .outline()
                                .small()
                                .dropdown_caret(true)
                                .disabled(changing)
                                .dropdown_menu_with_anchor(
                                    gpui::Corner::TopRight,
                                    move |mut menu, _, _| {
                                        for role in Role::ALL {
                                            let person = person_for_click.clone();
                                            let view = view.clone();
                                            let role = *role;
                                            menu = menu.item(
                                                PopupMenuItem::new(role.title())
                                                    .checked(role == current)
                                                    .on_click(move |_, _, cx| {
                                                        let person = person.clone();
                                                        view.update(cx, |this, cx| {
                                                            this.change_role(person, role, cx)
                                                        });
                                                    }),
                                            );
                                        }
                                        menu
                                    },
                                )
                        })
                });
                v_flex()
                    .border_1()
                    .border_color(cx.theme().border)
                    .children(rows)
            }
            People::Loaded(_) | People::Idle => h_flex()
                .gap_2()
                .text_sm()
                .text_color(muted)
                .child(if configured {
                    "No one has signed in yet."
                } else {
                    "Install ricochet to see users."
                })
                .when(configured, |row| {
                    let server = self.server.clone();
                    row.child(
                        Button::new("open-ricochet-people")
                            .icon(ph("arrow-square-out"))
                            .label("Open ricochet")
                            .link()
                            .small()
                            .on_click(move |_, _, cx| {
                                server.update(cx, |server, cx| server.open_dashboard(cx))
                            }),
                    )
                }),
            People::Loading => div().text_sm().text_color(muted).child("Loading users…"),
        };
        v_flex().gap_3().children(failure).child(body)
    }

    fn email_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let email = &self.config.email;
        let enabled = email.enabled;
        let mut pane = card(cx).max_w(px(760.0)).child(self.toggle(
            "email-enabled",
            "Enable email",
            Some("ricochet emails notices such as failed deployments."),
            enabled,
            cx,
            |config, on| config.email.enabled = on,
        ));
        if enabled {
            pane = pane
                .child(self.text_field("Host", "email.host", None, cx))
                .child(self.choice("encryption", "Encryption", None, cx))
                .child(self.number_field("Port", "email.port", None, None, cx))
                .child(self.text_field("Username", "email.username", None, cx))
                .child(self.text_field("Password", "email.password", None, cx))
                .child(self.text_field("From", "email.from", None, cx))
                .child(self.text_field(
                    "Reply-to",
                    "email.reply_to",
                    Some("Leave empty to reply to the From address."),
                    cx,
                ));
        }
        pane.into_any_element()
    }

    fn packages_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let remove =
            |id: (&'static str, usize),
             cx: &mut Context<Self>,
             act: fn(&mut SetupView, usize, &mut Window, &mut Context<SetupView>)| {
                Button::new(id)
                    .icon(ph("trash"))
                    .tooltip("Remove")
                    .ghost()
                    .small()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        act(this, id.1, window, cx);
                        this.rebuild_rows(window, cx);
                        this.sync_inputs(window, cx);
                        this.edited(cx);
                    }))
            };
        let r_rows = self
            .r_rows
            .iter()
            .enumerate()
            .map(|(i, [name, url])| {
                h_flex()
                    .gap_2()
                    .child(v_flex().w(px(140.0)).child(Input::new(name)))
                    .child(v_flex().flex_1().min_w_0().child(Input::new(url)))
                    .child(remove(("remove-r", i), cx, |this, i, _, _| {
                        this.config.packages.r.remove(i);
                    }))
            })
            .collect::<Vec<_>>();
        let julia_rows = self
            .julia_rows
            .iter()
            .enumerate()
            .map(|(i, url)| {
                h_flex()
                    .gap_2()
                    .child(v_flex().flex_1().min_w_0().child(Input::new(url)))
                    .child(remove(("remove-julia", i), cx, |this, i, _, _| {
                        this.config.packages.julia.remove(i);
                    }))
            })
            .collect::<Vec<_>>();
        let python_rows = self
            .python_rows
            .iter()
            .enumerate()
            .map(|(i, url)| {
                h_flex()
                    .gap_2()
                    .child(v_flex().flex_1().min_w_0().child(Input::new(url)))
                    .child(remove(("remove-python", i), cx, |this, i, _, _| {
                        this.config.packages.python.remove(i);
                    }))
            })
            .collect::<Vec<_>>();
        let list = |id: &'static str,
                    title: &'static str,
                    rows: Vec<Div>,
                    cx: &mut Context<Self>,
                    act: fn(&mut SetupView)| {
            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(title))
                        .child(
                            Button::new(id)
                                .icon(ph("plus"))
                                .label("Add")
                                .ghost()
                                .xsmall()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    act(this);
                                    this.rebuild_rows(window, cx);
                                    this.sync_inputs(window, cx);
                                    this.edited(cx);
                                })),
                        ),
                )
                .children(rows)
        };
        card(cx)
            .max_w(px(760.0))
            .child(list("add-r", "R repositories", r_rows, cx, |this| {
                this.config.packages.r.push(RRepository::default())
            }))
            .child(list(
                "add-julia",
                "Julia registries",
                julia_rows,
                cx,
                |this| this.config.packages.julia.push(PackageIndex::default()),
            ))
            .child(list(
                "add-python",
                "Python indexes",
                python_rows,
                cx,
                |this| this.config.packages.python.push(PackageIndex::default()),
            ))
            .child(self.toggle(
                "allow-overrides",
                "Enable repo overrides",
                Some("Developers can set custom package repository overrides."),
                self.config.packages.allow_overrides,
                cx,
                |config, on| config.packages.allow_overrides = on,
            ))
            .into_any_element()
    }

    fn deployments_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        card(cx)
            .max_w(px(760.0))
            .child(self.number_field(
                "Deployment history",
                "deployments.max_deployments",
                None,
                Some("Deployments to keep per item."),
                cx,
            ))
            .child(self.number_field(
                "Deployment retention",
                "deployments.max_age_days",
                Some("days"),
                None,
                cx,
            ))
            .child(self.number_field(
                "Max bundle size",
                "deployments.max_bundle_size",
                None,
                Some("Such as 100MB or 1GB."),
                cx,
            ))
            .into_any_element()
    }

    fn tasks_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        card(cx)
            .max_w(px(760.0))
            .child(self.number_field("Task timeout", "tasks.max_duration", Some("min"), None, cx))
            .child(self.number_field(
                "Package install timeout",
                "tasks.restore_timeout",
                Some("min"),
                None,
                cx,
            ))
            .child(self.number_field(
                "Concurrent package installs",
                "tasks.restore_concurrency",
                None,
                None,
                cx,
            ))
            .into_any_element()
    }

    fn logging_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let tab = self.logging_tab;
        let view = cx.entity();
        let tabs = tab_bar(
            "logging-tab",
            vec![
                ("Settings", tab == LoggingTab::Settings),
                ("Live", tab == LoggingTab::Live),
            ],
            move |index, _, app| {
                view.update(app, |this, cx| {
                    this.logging_tab = if index == 0 {
                        LoggingTab::Settings
                    } else {
                        LoggingTab::Live
                    };
                    if this.logging_tab == LoggingTab::Live {
                        this.follow_logs(cx);
                    } else {
                        this.stop_following_logs();
                    }
                    cx.notify();
                });
            },
        );
        let body = match tab {
            LoggingTab::Settings => self.logging_settings(cx),
            LoggingTab::Live => self.live_logs(cx),
        };
        v_flex()
            .gap_5()
            .max_w(px(760.0))
            .child(tabs)
            .child(body)
            .into_any_element()
    }

    fn logging_settings(&self, cx: &mut Context<Self>) -> Div {
        let logging = &self.config.logging;
        card(cx)
            .child(self.choice("log-level", "Log level", None, cx))
            .child(self.choice("log-rotation", "Log rotation", None, cx))
            .child(self.toggle(
                "log-compress",
                "Enable compression",
                Some("Compress old logs to save disk space."),
                logging.compress,
                cx,
                |config, on| config.logging.compress = on,
            ))
            .child(self.number_field(
                "File retention (days)",
                "logging.max_age_days",
                Some("days"),
                Some("Configure how long logs are kept before being deleted."),
                cx,
            ))
            .child(self.number_field(
                "Max log files",
                "logging.max_files",
                None,
                Some("Delete the oldest rotated logs past this count."),
                cx,
            ))
    }

    fn audit_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let audit = &self.config.audit_log;
        card(cx)
            .max_w(px(760.0))
            .child(self.toggle(
                "audit-forever",
                "Keep audit logs forever",
                None,
                audit.keep_forever,
                cx,
                |config, on| config.audit_log.keep_forever = on,
            ))
            .when(!audit.keep_forever, |pane| {
                pane.child(self.number_field(
                    "Log retention",
                    "audit.retention_days",
                    Some("days"),
                    None,
                    cx,
                ))
            })
            .child(self.toggle(
                "audit-file",
                "Enable file logging",
                Some("Audit logs are recorded to a file in addition to the database."),
                audit.write_file,
                cx,
                |config, on| config.audit_log.write_file = on,
            ))
            .into_any_element()
    }

    fn telemetry_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let telemetry = &self.config.telemetry;
        card(cx)
            .max_w(px(760.0))
            .child(self.toggle(
                "otel-enabled",
                "Enable tracing",
                None,
                telemetry.enabled,
                cx,
                |config, on| config.telemetry.enabled = on,
            ))
            .when(telemetry.enabled, |pane| {
                pane.child(self.text_field("Collector URL", "telemetry.endpoint", None, cx))
            })
            .into_any_element()
    }
}

impl Render for SetupView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(step) = self.onboarding {
            return self.onboarding_view(step, cx);
        }
        if self.server.read(cx).loading {
            return self.connecting_view(cx);
        }
        self.viewport_height = window.viewport_size().height;

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.controls(cx))
            .child(rule(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(cx))
                    .child(self.detail(cx)),
            )
            .capture_any_mouse_down({
                let pressed = self.focus_at_press.clone();
                move |_, window, cx| *pressed.borrow_mut() = window.focused(cx)
            })
            .on_any_mouse_down({
                let pressed = self.focus_at_press.clone();
                move |_, window, cx| {
                    if *pressed.borrow() == window.focused(cx) {
                        window.blur();
                    }
                }
            })
            .children(Root::render_dialog_layer(window, cx))
            .into_any_element()
    }
}
