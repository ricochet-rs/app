//! The window: server controls on top, then a sidebar of panes beside the pane being edited.

use super::components::{callout, error_alert, field, heading, rule, segmented};
use super::icons::ph;
use super::section::SetupSection;
use crate::config::section::Field as _;
use crate::config::{
    AuthenticationSettings, Encryption, LogLevel, LogRotation, PackageIndex, Provider, RRepository,
    Role, ServerConfig,
};
use crate::controller::{Failure, ServerController, ServerStatus};
use crate::person::Person;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, ClipboardItem, Context, Div, Entity, FontWeight, SharedString, Subscription,
    Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::switch::Switch;
use gpui_component::{ActiveTheme, Disableable, Sizable, h_flex, v_flex};
use std::collections::HashMap;

mod onboarding;

type Access = Box<dyn Fn(&mut ServerConfig) -> Option<&mut String>>;

/// A text field and the answer in the config it edits.
struct Binding {
    input: Entity<InputState>,
    access: Access,
}

/// A screen of first-run setup.
#[derive(Clone, Copy, PartialEq)]
enum Step {
    Welcome,
    Target,
    SignIn,
    Server,
    Review,
    Installing,
    Done,
}

#[derive(Clone, Copy, PartialEq)]
enum UsersTab {
    Settings,
    People,
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
    switcher_open: bool,
    host_input: Entity<InputState>,
    /// The first-run screen being shown, or `None` once setup is behind you.
    onboarding: Option<Step>,
    section: SetupSection,
    users_tab: UsersTab,
    config: ServerConfig,
    /// The issuer ricochet signs people in with now, or `None` before the first setup.
    saved_issuer: Option<String>,
    problem: Option<String>,
    chose_own_provider: bool,
    configuring_own_provider: bool,
    confirming_uninstall: bool,
    /// Whether a save is waiting to finish, so its issuer becomes the saved one on success.
    submitted: bool,
    inputs: HashMap<&'static str, Entity<InputState>>,
    bindings: Vec<Binding>,
    r_rows: Vec<[Entity<InputState>; 2]>,
    julia_rows: Vec<Entity<InputState>>,
    python_rows: Vec<Entity<InputState>>,
    row_bindings: Vec<Binding>,
    people: People,
    people_failure: Option<Failure>,
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
            switcher_open: false,
            host_input,
            section: SetupSection::Authentication,
            users_tab: UsersTab::Settings,
            config: saved.unwrap_or_default(),
            saved_issuer,
            problem: None,
            chose_own_provider: false,
            configuring_own_provider: false,
            confirming_uninstall: false,
            submitted: false,
            inputs: HashMap::new(),
            bindings: Vec::new(),
            r_rows: Vec::new(),
            julia_rows: Vec::new(),
            python_rows: Vec::new(),
            row_bindings: Vec::new(),
            people: People::Idle,
            people_failure: None,
            changing_role: false,
            subscriptions: vec![cx.observe_in(&server, window, |this, server, window, cx| {
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
        bind!("server.license_key", "", false, server.license_key);
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
            "Days",
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
            "Minutes",
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
        bind!("logging.max_age_days", "Days", false, logging.max_age_days);
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
        self.switcher_open = false;
        self.problem = None;
        self.confirming_uninstall = false;
        self.rebuild_rows(window, cx);
        self.sync_inputs(window, cx);
        self.onboarding = match (configured, self.onboarding) {
            (true, _) => None,
            (false, Some(Step::Installing | Step::Done | Step::Review | Step::Server)) => {
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
                            title: "Ricochet couldn't list people".into(),
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
                    title: format!("Ricochet couldn't change {name}'s role"),
                    message: error.to_string(),
                });
                this.load_people(cx);
            });
        })
        .detach();
    }

    /// Starts the install, or opens the first pane that still needs an answer.
    fn submit(&mut self, cx: &mut Context<Self>) {
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
            ServerStatus::NotInstalled => ("Not set up", cx.theme().muted_foreground),
        };
        let version = server
            .installed
            .as_ref()
            .map_or("Not installed".to_owned(), |release| {
                format!("Version {}", release.version)
            });
        let (running, stopped, configured) = (
            server.status == ServerStatus::Running,
            server.status == ServerStatus::Stopped,
            server.is_configured,
        );
        let update = server.available_update();
        let can_logs = server.can_open_logs();
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
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Ricochet"))
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
                    .child(div().size(px(6.0)).rounded_full().bg(color))
                    .child(label),
            )
            .child(
                Button::new("connection")
                    .icon(ph(connection_icon))
                    .label(connection_name)
                    .outline()
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.switcher_open = !this.switcher_open;
                        cx.notify();
                    })),
            )
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
                        .label("Open ricochet")
                        .primary()
                        .small()
                        .disabled(!running || busy)
                        .on_click(act(|server, cx| server.open_dashboard(cx))),
                )
                .child(if stopped {
                    Button::new("start")
                        .icon(ph("play"))
                        .label("Start")
                        .outline()
                        .small()
                        .disabled(busy)
                        .on_click(act(|s, cx| s.start(cx)))
                } else {
                    Button::new("stop")
                        .icon(ph("stop"))
                        .label("Stop")
                        .outline()
                        .small()
                        .disabled(busy)
                        .on_click(act(|s, cx| s.stop(cx)))
                })
                .child(
                    Button::new("restart")
                        .icon(ph("arrow-clockwise"))
                        .label("Restart")
                        .outline()
                        .small()
                        .disabled(stopped || busy)
                        .on_click(act(|s, cx| s.restart(cx))),
                )
                .when(can_logs, |row| {
                    row.child(
                        Button::new("logs")
                            .icon(ph("file-text"))
                            .label("Show logs")
                            .ghost()
                            .small()
                            .on_click(act(|s, _| s.open_logs())),
                    )
                })
            })
    }

    /// The computers you can switch to, shown under the bar while it is open.
    fn switcher_panel(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.switcher_open {
            return None;
        }
        let server = self.server.read(cx);
        let active = server.connection.clone();
        let busy = server.activity.is_some();
        let hover = cx.theme().accent;
        let rows = server
            .choices()
            .into_iter()
            .enumerate()
            .map(|(index, choice)| {
                let selected = choice == active;
                let target = choice.clone();
                h_flex()
                    .id(("switch", index))
                    .gap_2()
                    .px_3()
                    .py_1p5()
                    .items_center()
                    .text_sm()
                    .when(!busy, |row| {
                        row.cursor_pointer().hover(move |row| row.bg(hover))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.server.read(cx).activity.is_some() {
                            let target = target.clone();
                            this.server
                                .update(cx, |server, cx| server.switch(target, cx));
                        }
                    }))
                    .child(
                        ph(if choice.is_remote() {
                            "hard-drives"
                        } else {
                            "desktop"
                        })
                        .size_4(),
                    )
                    .child(div().flex_1().child(choice.name()))
                    .when(selected, |row| row.child(ph("check").size_4()))
            });
        Some(
            v_flex()
                .p_2()
                .bg(cx.theme().sidebar)
                .border_b_1()
                .border_color(cx.theme().border)
                .children(rows)
                .child(
                    Button::new("add-computer")
                        .icon(ph("plus"))
                        .label("Add computer")
                        .ghost()
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.switcher_open = false;
                            this.onboarding = Some(Step::Target);
                            cx.notify();
                        })),
                ),
        )
    }

    // MARK: Sidebar

    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        let mut list = v_flex()
            .w(px(200.0))
            .flex_shrink_0()
            .h_full()
            .p_2()
            .gap_0p5()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border);
        for required in [true, false] {
            list = list.child(
                div()
                    .px_3()
                    .pt(px(if required { 4.0 } else { 16.0 }))
                    .pb_1()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(cx.theme().muted_foreground)
                    .child(if required { "Required" } else { "Optional" }),
            );
            for item in SetupSection::ALL
                .into_iter()
                .filter(|item| item.is_required() == required)
            {
                let selected = self.section == item;
                let needs_answer = item.problem(&self.config).is_some();
                let accent = cx.theme().accent;
                list = list.child(
                    h_flex()
                        .id(SharedString::from(item.title()))
                        .px_3()
                        .py_1p5()
                        .gap_2()
                        .items_center()
                        .text_sm()
                        .cursor_pointer()
                        .when(selected, |row| row.bg(accent))
                        .hover(move |row| row.bg(accent))
                        .on_click(cx.listener(move |this, _, _, cx| this.select(item, cx)))
                        .child(
                            ph(item.icon())
                                .size_4()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(div().flex_1().child(item.title()))
                        .when(needs_answer, |row| {
                            row.child(div().size(px(6.0)).rounded_full().bg(cx.theme().danger))
                        }),
                );
            }
        }
        list
    }

    // MARK: Detail

    fn detail(&self, cx: &mut Context<Self>) -> Div {
        let server = self.server.read(cx);
        let activity = server.activity.clone();
        let failure = server.failure.clone();
        let busy = activity.is_some();
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
        };

        let alert = if let Some(problem) = &self.problem {
            Some(error_alert(
                "Check your answers",
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
                            .child(heading(self.section.title(), self.section.summary(), cx))
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
        field(label, description.map(str::to_owned), self.input(key), cx)
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

    #[allow(clippy::too_many_arguments)]
    fn choice<T: Copy + PartialEq + 'static>(
        &self,
        id: &'static str,
        label: &'static str,
        description: Option<&str>,
        all: &'static [T],
        current: T,
        title: fn(T) -> &'static str,
        cx: &mut Context<Self>,
        set: fn(&mut SetupView, T, &mut Window, &mut Context<SetupView>),
    ) -> Div {
        let view = cx.entity();
        let options = all
            .iter()
            .map(|option| (title(*option), *option == current))
            .collect();
        field(
            label,
            description.map(str::to_owned),
            segmented(id, options, move |index, window, app| {
                view.update(app, |this, cx| set(this, all[index], window, cx));
            }),
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
                "You lose access to your current accounts",
                "After signing in, change your role in the Users tab.",
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
            let card = |icon: &'static str,
                        id: &'static str,
                        title: &'static str,
                        message: &'static str,
                        selected: bool,
                        actions: Vec<AnyElement>,
                        cx: &App| {
                v_flex()
                    .gap_2p5()
                    .p_5()
                    .items_center()
                    .min_h(px(150.0))
                    .bg(super::theme::Palette::card(cx))
                    .border(px(if selected { 2.0 } else { 1.0 }))
                    .border_color(if selected {
                        cx.theme().ring
                    } else {
                        cx.theme().border
                    })
                    .id(id)
                    .child(ph(icon).size_8().text_color(if selected {
                        cx.theme().ring
                    } else {
                        cx.theme().foreground
                    }))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                    .child(
                        div()
                            .text_sm()
                            .text_center()
                            .text_color(cx.theme().muted_foreground)
                            .child(message),
                    )
                    .child(h_flex().gap_3().pt_1p5().children(actions))
            };
            let agree = Button::new("agree")
                .label(if auth.managed { "Agreed" } else { "I agree" })
                .small()
                .on_click(cx.listener(|this, _, window, cx| {
                    this.config.authentication.managed = true;
                    this.config.server.port = AuthenticationSettings::MANAGED_PORT.to_owned();
                    this.sync_inputs(window, cx);
                    this.edited(cx);
                }));
            let agree = if auth.managed {
                agree.outline()
            } else {
                agree.primary()
            };
            v_flex()
                .gap_3()
                .max_w(px(760.0))
                .child(card(
                    "shield-check",
                    "managed",
                    "Managed auth",
                    "Sign in on this computer with a ricochet account. You share your name and email address with ricochet.",
                    auth.managed,
                    vec![
                        agree.into_any_element(),
                        Button::new("privacy")
                            .label("Privacy Policy")
                            .link()
                            .small()
                            .on_click(|_, _, _| {
                                let _ = open::that(AuthenticationSettings::PRIVACY_POLICY);
                            })
                            .into_any_element(),
                    ],
                    cx,
                ))
                .child(card(
                    "key",
                    "own",
                    "Your own IdP",
                    "Sign in through Okta, Entra ID, Keycloak or another OpenID Connect provider.",
                    uses_own_provider,
                    vec![Button::new("configure")
                        .label("Configure")
                        .outline()
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.config.authentication.managed = false;
                            this.chose_own_provider = true;
                            this.configuring_own_provider = true;
                            this.edited(cx);
                        }))
                        .into_any_element()],
                    cx,
                ))
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
        v_flex()
            .gap_4()
            .max_w(px(760.0))
            .child(
                Button::new("back").icon(ph("caret-left"))
                    .label("Back")
                    .link()
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.configuring_own_provider = false;
                        cx.notify();
                    })),
            )
            .child(segmented(
                "provider",
                Provider::ALL.iter().map(|p| (p.title(), *p == provider)).collect(),
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
            ))
            .child(self.text_field("Issuer URL", "auth.issuer_url", None, cx))
            .child(self.text_field("Client ID", "auth.client_id", None, cx))
            .child(self.text_field(
                "Client secret",
                "auth.client_secret",
                (provider == Provider::Oidc)
                    .then_some("Leave this empty if you registered the app without a secret."),
                cx,
            ))
            .child(field(
                "Redirect URL",
                Some("Add this URL to the app you registered. It changes when you change the port.".into()),
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_1p5()
                            .border_1()
                            .border_color(cx.theme().input)
                            .text_sm()
                            .font_family(cx.theme().mono_font_family.clone())
                            .child(redirect_url),
                    )
                    .child(Button::new("copy-redirect").icon(ph("copy")).label("Copy").outline().small().on_click(
                        move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copied.clone())),
                    )),
                cx,
            ))
            .child(self.text_field(
                "Display name claim",
                "auth.display_name_claim",
                Some("Ricochet shows this claim as each person's name."),
                cx,
            ))
            .child(self.text_field(
                "Groups claim",
                "auth.groups_claim",
                Some("Leave this empty and ricochet ignores groups from your identity provider."),
                cx,
            ))
            .child(self.text_field(
                "Group sync interval",
                "auth.sync_interval",
                Some("How often ricochet refreshes groups, in minutes. Leave this empty and ricochet refreshes them only when someone signs in."),
                cx,
            ))
    }

    fn server_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let managed = self.config.authentication.managed;
        let installed = self.server.read(cx).installed.is_some();
        let local_url = self.config.local_url();
        let port_description = if managed {
            "Switch to your own IdP in Authentication to change the port.".to_owned()
        } else {
            format!("On this computer, ricochet answers at {local_url}.")
        };
        let server = self.server.clone();
        let install_or_uninstall = if installed {
            Button::new("uninstall")
                .icon(ph("trash"))
                .label("Uninstall ricochet")
                .danger()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.confirming_uninstall = true;
                    cx.notify();
                }))
        } else {
            Button::new("install")
                .icon(ph("download-simple"))
                .label("Install ricochet")
                .primary()
                .on_click(move |_, _, cx| server.update(cx, |server, cx| server.install(cx)))
        };
        let confirm = self.confirming_uninstall.then(|| {
            let server = self.server.clone();
            callout(
                "Uninstall ricochet?",
                "This app stops ricochet and moves it, with its content, database, caches and encryption keys, to the Trash. You can restore them until you empty the Trash.",
                cx.theme().danger,
                vec![
                    Button::new("confirm-uninstall").icon(ph("trash"))
                        .label("Uninstall")
                        .danger()
                        .small()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.confirming_uninstall = false;
                            server.update(cx, |server, cx| server.uninstall(cx));
                            cx.notify();
                        }))
                        .into_any_element(),
                    Button::new("cancel-uninstall")
                        .label("Cancel")
                        .outline()
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.confirming_uninstall = false;
                            cx.notify();
                        }))
                        .into_any_element(),
                ],
                cx,
            )
        });
        v_flex()
            .gap_4()
            .max_w(px(760.0))
            .child(field(
                "Port",
                Some(port_description),
                Input::new(&self.inputs["server.port"]).disabled(managed),
                cx,
            ))
            .child(self.text_field(
                "License key",
                "server.license_key",
                Some("Leave this empty to use the Community Edition."),
                cx,
            ))
            .child(div().pt_4().child(install_or_uninstall))
            .children(confirm)
            .into_any_element()
    }

    fn users_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let tab = self.users_tab;
        let view = cx.entity();
        let tabs = segmented(
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
            UsersTab::Settings => v_flex()
                .gap_4()
                .child(self.choice(
                    "default-role",
                    "Role for new people",
                    Some("The first person to sign in still becomes the admin."),
                    Role::ALL,
                    users.default_role,
                    Role::title,
                    cx,
                    |this, role, _, cx| {
                        this.config.users.default_role = role;
                        this.edited(cx);
                    },
                ))
                .child(self.toggle(
                    "require-sign-in",
                    "Require sign-in for every page",
                    Some("When this is off, people can browse without signing in."),
                    users.require_sign_in,
                    cx,
                    |config, on| config.users.require_sign_in = on,
                ))
                .child(self.toggle(
                    "avatars",
                    "Allow profile pictures",
                    Some("People can upload their own profile picture."),
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
                        .child(segmented(
                            format!("role-{index}"),
                            Role::ALL
                                .iter()
                                .map(|role| (role.title(), *role == person.role))
                                .collect(),
                            move |role_index, _, app| {
                                if changing {
                                    return;
                                }
                                let person = person_for_click.clone();
                                view.update(app, |this, cx| {
                                    this.change_role(person, Role::ALL[role_index], cx)
                                });
                            },
                        ))
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
                    "Ricochet isn't set up yet, so no one has signed in."
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
            People::Loading => div().text_sm().text_color(muted).child("Loading people…"),
        };
        v_flex().gap_3().children(failure).child(body)
    }

    fn email_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let email = &self.config.email;
        let enabled = email.enabled;
        let default_port = match email.encryption {
            Encryption::StartTls => 587,
            Encryption::Tls => 465,
            Encryption::None => 25,
        };
        let mut pane = v_flex().gap_4().max_w(px(760.0)).child(self.toggle(
            "email-enabled",
            "Send email",
            Some("Ricochet sends no email until you turn this on."),
            enabled,
            cx,
            |config, on| config.email.enabled = on,
        ));
        if enabled {
            pane = pane
                .child(self.text_field("Host", "email.host", None, cx))
                .child(self.choice(
                    "encryption",
                    "Encryption",
                    None,
                    Encryption::ALL,
                    email.encryption,
                    Encryption::title,
                    cx,
                    |this, encryption, window, cx| {
                        this.config.email.encryption = encryption;
                        this.sync_email_port_hint(window, cx);
                        this.edited(cx);
                    },
                ))
                .child(self.text_field(
                    "Port",
                    "email.port",
                    Some(&format!("Leave this empty to use {default_port}.")),
                    cx,
                ))
                .child(self.text_field("Username", "email.username", None, cx))
                .child(self.text_field("Password", "email.password", None, cx))
                .child(self.text_field("From", "email.from", None, cx))
                .child(self.text_field(
                    "Reply-to",
                    "email.reply_to",
                    Some("Leave this empty and people reply to the sender."),
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
                    .label("Remove")
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
        let add = |id: &'static str,
                   label: &'static str,
                   cx: &mut Context<Self>,
                   act: fn(&mut SetupView)| {
            Button::new(id)
                .icon(ph("plus"))
                .label(label)
                .outline()
                .small()
                .on_click(cx.listener(move |this, _, window, cx| {
                    act(this);
                    this.rebuild_rows(window, cx);
                    this.sync_inputs(window, cx);
                    this.edited(cx);
                }))
        };
        v_flex()
            .gap_4()
            .max_w(px(760.0))
            .child(field(
                "R repositories",
                Some("Ricochet tries them in name order.".into()),
                v_flex().gap_2().children(r_rows).child(add(
                    "add-r",
                    "Add R repository",
                    cx,
                    |this| this.config.packages.r.push(RRepository::default()),
                )),
                cx,
            ))
            .child(field(
                "Julia registries",
                None,
                v_flex().gap_2().children(julia_rows).child(add(
                    "add-julia",
                    "Add Julia registry",
                    cx,
                    |this| this.config.packages.julia.push(PackageIndex::default()),
                )),
                cx,
            ))
            .child(field(
                "Python indexes",
                None,
                v_flex().gap_2().children(python_rows).child(add(
                    "add-python",
                    "Add Python index",
                    cx,
                    |this| this.config.packages.python.push(PackageIndex::default()),
                )),
                cx,
            ))
            .child(self.toggle(
                "allow-overrides",
                "Let developers choose their own",
                Some("Developers can add or replace repositories in an item's _ricochet.toml."),
                self.config.packages.allow_overrides,
                cx,
                |config, on| config.packages.allow_overrides = on,
            ))
            .into_any_element()
    }

    fn deployments_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap_4()
            .max_w(px(760.0))
            .child(self.text_field(
                "Deployments to keep",
                "deployments.max_deployments",
                Some("Ricochet keeps this many deployments of each item. Leave this empty for 5."),
                cx,
            ))
            .child(self.text_field(
                "Delete deployments after",
                "deployments.max_age_days",
                Some("In days. Leave this empty and ricochet keeps deployments however old they get."),
                cx,
            ))
            .child(self.text_field(
                "Largest bundle",
                "deployments.max_bundle_size",
                Some("Ricochet turns away bigger bundles. Leave this empty for 100MB."),
                cx,
            ))
            .into_any_element()
    }

    fn tasks_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap_4()
            .max_w(px(760.0))
            .child(self.text_field(
                "Longest task run",
                "tasks.max_duration",
                Some("In minutes. Ricochet stops a run that takes longer. Leave this empty for 60."),
                cx,
            ))
            .child(self.text_field(
                "Longest package install",
                "tasks.restore_timeout",
                Some("In minutes. Ricochet stops an install that takes longer. Leave this empty for ricochet's default."),
                cx,
            ))
            .child(self.text_field(
                "Package installs at once",
                "tasks.restore_concurrency",
                Some("Ricochet runs this many installs at the same time. Leave this empty for 4."),
                cx,
            ))
            .into_any_element()
    }

    fn logging_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let logging = &self.config.logging;
        v_flex()
            .gap_4()
            .max_w(px(760.0))
            .child(self.choice(
                "log-level",
                "Level",
                Some("Ricochet logs far more at Debug and Trace than at Info, the default."),
                LogLevel::ALL,
                logging.level,
                LogLevel::title,
                cx,
                |this, level, _, cx| {
                    this.config.logging.level = level;
                    this.edited(cx);
                },
            ))
            .child(self.choice(
                "log-rotation",
                "Start a new file",
                None,
                LogRotation::ALL,
                logging.rotation,
                LogRotation::title,
                cx,
                |this, rotation, _, cx| {
                    this.config.logging.rotation = rotation;
                    this.edited(cx);
                },
            ))
            .child(self.toggle(
                "log-compress",
                "Compress old files",
                None,
                logging.compress,
                cx,
                |config, on| config.logging.compress = on,
            ))
            .child(self.text_field(
                "Files to keep",
                "logging.max_files",
                Some("Leave this empty and ricochet keeps every file."),
                cx,
            ))
            .child(self.text_field(
                "Delete files after",
                "logging.max_age_days",
                Some("In days. Leave this empty and ricochet keeps files however old they get."),
                cx,
            ))
            .into_any_element()
    }

    fn audit_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let audit = &self.config.audit_log;
        v_flex()
            .gap_4()
            .max_w(px(760.0))
            .child(self.toggle(
                "audit-forever",
                "Keep records forever",
                None,
                audit.keep_forever,
                cx,
                |config, on| config.audit_log.keep_forever = on,
            ))
            .when(!audit.keep_forever, |pane| {
                pane.child(self.text_field(
                    "Keep records for",
                    "audit.retention_days",
                    Some("In days. Leave this empty for 365."),
                    cx,
                ))
            })
            .child(self.toggle(
                "audit-file",
                "Also write a log file",
                Some("Ricochet copies each record to logs/audit/audit.log."),
                audit.write_file,
                cx,
                |config, on| config.audit_log.write_file = on,
            ))
            .into_any_element()
    }

    fn telemetry_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let telemetry = &self.config.telemetry;
        v_flex()
            .gap_4()
            .max_w(px(760.0))
            .child(self.toggle(
                "otel-enabled",
                "Send traces",
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(step) = self.onboarding {
            return self.onboarding_view(step, cx);
        }
        if self.server.read(cx).loading {
            return self.connecting_view(cx);
        }
        let panel = self.switcher_panel(cx);
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.controls(cx))
            .child(rule(cx))
            .children(panel)
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(cx))
                    .child(self.detail(cx)),
            )
            .into_any_element()
    }
}
