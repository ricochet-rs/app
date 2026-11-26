//! The content of a ricochet server: its own web UI, shown inside the app.

use super::components::{card, heading, prose, rule};
use super::icons::ph;

mod web;
use crate::controller::{ServerController, ServerStatus};
use crate::servers::{Instance, Placement, Server, Servers, normalize_url};
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Div, Entity, Subscription, Window, div, px};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::spinner::Spinner;
use gpui_component::{ActiveTheme, Disableable, Sizable, h_flex, v_flex};
#[cfg(target_os = "linux")]
pub use web::linux as linux_webview;

/// Asks the window to show where ricochet can be installed, instead of connecting by address.
pub struct InstallInstead;

impl gpui::EventEmitter<InstallInstead> for DashboardView {}

/// An edit to an instance on the All instances page.
pub enum InstanceChange {
    Name(String),
    /// Into a group, or no group, just before another instance or at the group's end.
    Move {
        group: Option<String>,
        before: Option<String>,
    },
}

/// What the dashboard knows about the server it shows.
enum Content {
    /// No server to show yet.
    NoServer,
    /// The managed server is installed but not running.
    NotRunning,
    Loading,
    /// The server's web UI, at this address.
    Web(String),
}

pub struct DashboardView {
    server: Entity<ServerController>,
    store: Servers,
    content: Content,
    url_input: Entity<InputState>,
    adding_server: bool,
    add_problem: Option<String>,
    /// The managed server, its status and whether an operation runs on it, to load again when they change.
    seen_managed: (Option<Server>, ServerStatus, bool),
    web: web::Web,
    /// Whether a menu drops over the content, so the web UI must hide to let it show.
    covered: bool,
    _subscriptions: Vec<Subscription>,
}

impl DashboardView {
    pub fn new(
        server: Entity<ServerController>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let url_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("ricochet.example.com"));
        let subscriptions = vec![
            cx.observe(&server, |this, _, cx| {
                let seen = this.managed_state(cx);
                if seen != this.seen_managed {
                    let showing_managed = this.active_server(cx).is_none_or(|s| s.managed);
                    let (before, now) = (&this.seen_managed, &seen);
                    let changed = (&before.0, before.1) != (&now.0, now.1);
                    let waited = before.2 && !now.2 && matches!(this.content, Content::Loading);
                    this.seen_managed = seen;
                    if showing_managed && (changed || waited) {
                        this.load(cx);
                    }
                }
            }),
            cx.subscribe(&url_input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.add_server(cx);
                }
            }),
        ];
        let mut view = Self {
            server,
            store: Servers::load(),
            content: Content::NoServer,
            url_input,
            adding_server: false,
            add_problem: None,
            seen_managed: (None, ServerStatus::Stopped, false),
            web: web::Web::default(),
            covered: false,
            _subscriptions: subscriptions,
        };
        view.seen_managed = view.managed_state(cx);
        view.load(cx);
        view
    }

    // MARK: Servers

    fn managed_server(&self, cx: &App) -> Option<Server> {
        let controller = self.server.read(cx);
        controller
            .is_configured
            .then(|| Server::managed(controller.dashboard()))
    }

    fn managed_state(&self, cx: &App) -> (Option<Server>, ServerStatus, bool) {
        let controller = self.server.read(cx);
        (
            self.managed_server(cx),
            controller.status,
            controller.activity.is_some(),
        )
    }

    /// The servers you added by address.
    pub fn added_servers(&self) -> Vec<Server> {
        self.store
            .urls
            .iter()
            .map(|url| Server::remote(url))
            .collect()
    }

    /// The server added by address that is showing, or `None` while the managed server shows.
    pub fn added_server(&self) -> Option<Server> {
        let active = self.store.active.as_deref()?;
        self.store
            .urls
            .iter()
            .any(|url| url == active)
            .then(|| Server::remote(active))
    }

    /// The server whose content shows: the added one you picked, otherwise the managed one.
    pub fn active_server(&self, cx: &App) -> Option<Server> {
        self.added_server().or_else(|| self.managed_server(cx))
    }

    /// Hides the web UI while a menu drops over it, and shows it again once the menu closes.
    pub fn set_covered(&mut self, covered: bool, cx: &mut Context<Self>) {
        self.covered = covered;
        if covered {
            self.hide_web(cx);
        }
        cx.notify();
    }

    /// Hides the web UI, which the system draws above the window and would cover anything else shown there.
    pub fn hide_web(&mut self, cx: &mut App) {
        self.web.hide(cx);
    }

    pub fn show_server(&mut self, server: &Server, cx: &mut Context<Self>) {
        self.store.active = Some(server.id.clone());
        self.store.save();
        self.adding_server = false;
        self.load(cx);
    }

    /// Every instance, in the order and groups you gave them.
    pub fn instances(&self) -> Vec<(Instance, Placement)> {
        let instances = std::iter::once(Instance::Local)
            .chain(self.added_servers().into_iter().map(Instance::Added))
            .collect();
        self.store.arrange(instances)
    }

    /// Renames, regroups or moves an instance among `arranged`.
    pub fn arrange_instance(
        &mut self,
        arranged: &[(Instance, Placement)],
        id: &str,
        change: InstanceChange,
        cx: &mut Context<Self>,
    ) {
        let blank = |text: String| {
            let text = text.trim().to_owned();
            (!text.is_empty()).then_some(text)
        };
        match change {
            InstanceChange::Name(name) => self.store.place(arranged, id, |p| p.name = blank(name)),
            InstanceChange::Move { group, before } => {
                self.store
                    .move_to(arranged, id, group.and_then(blank), before.as_deref())
            }
        }
        self.store.save();
        cx.notify();
    }

    /// The groups you made that hold none of `arranged`.
    pub fn empty_groups(&self, arranged: &[(Instance, Placement)]) -> Vec<String> {
        self.store.empty_groups(arranged)
    }

    /// Adds an empty group and returns its name.
    pub fn add_group(
        &mut self,
        arranged: &[(Instance, Placement)],
        cx: &mut Context<Self>,
    ) -> String {
        let name = self.store.add_group(arranged);
        self.store.save();
        cx.notify();
        name
    }

    pub fn remove_group(&mut self, name: &str, cx: &mut Context<Self>) {
        self.store.remove_group(name);
        self.store.save();
        cx.notify();
    }

    /// Renames the group `from` to `to` for every instance among `arranged` in it.
    pub fn rename_group(
        &mut self,
        arranged: &[(Instance, Placement)],
        from: &str,
        to: &str,
        cx: &mut Context<Self>,
    ) {
        self.store.rename_group(arranged, from, to);
        self.store.save();
        cx.notify();
    }

    /// Shows the content of the server this app manages.
    pub fn show_managed(&mut self, cx: &mut Context<Self>) {
        self.store.active = None;
        self.store.save();
        self.adding_server = false;
        self.load(cx);
    }

    /// Shows the form that adds a server by address.
    pub fn start_adding(&mut self, cx: &mut Context<Self>) {
        self.adding_server = true;
        self.add_problem = None;
        cx.notify();
    }

    fn add_server(&mut self, cx: &mut Context<Self>) {
        let text = self.url_input.read(cx).value().to_string();
        let Some(url) = normalize_url(&text) else {
            self.add_problem = Some(
                "Enter the address you open ricochet at, such as `ricochet.example.com`.".into(),
            );
            cx.notify();
            return;
        };
        self.add_problem = None;
        let server = self.store.add(&url);
        self.show_server(&server, cx);
    }

    pub fn remove_server(&mut self, server: &Server, cx: &mut Context<Self>) {
        self.store.remove(server);
        self.store.save();
        self.adding_server = false;
        self.load(cx);
    }

    // MARK: Loading

    fn load(&mut self, cx: &mut Context<Self>) {
        let Some(server) = self.active_server(cx) else {
            self.content = Content::NoServer;
            cx.notify();
            return;
        };
        if server.managed {
            let controller = self.server.read(cx);
            // Coming back while ricochet restarts waits for it, which the observer then loads.
            if controller.activity.is_some() {
                self.content = Content::Loading;
                cx.notify();
                return;
            }
            if controller.status != ServerStatus::Running {
                self.content = Content::NotRunning;
                cx.notify();
                return;
            }
        }
        self.content = Content::Web(server.url);
        cx.notify();
    }

    // MARK: Forms

    fn centered(&self, content: impl IntoElement, cx: &App) -> Div {
        v_flex()
            .flex_1()
            .min_h_0()
            .items_center()
            .justify_center()
            .p_6()
            .bg(cx.theme().background)
            .child(div().w(px(440.0)).max_w_full().child(content))
    }

    fn add_server_form(&self, cx: &mut Context<Self>) -> Div {
        let cancellable = self.active_server(cx).is_some();
        card(cx)
            .child(heading(
                "Connect to ricochet",
                Some("Enter the address you open ricochet at in your browser."),
                cx,
            ))
            .child(Input::new(&self.url_input))
            .children(self.add_problem.clone().map(|problem| {
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(prose(problem))
            }))
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .when(cancellable, |row| {
                        row.child(
                            Button::new("cancel-add")
                                .label("Cancel")
                                .outline()
                                .small()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.adding_server = false;
                                    this.add_problem = None;
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        Button::new("add-server")
                            .icon(ph("arrow-right"))
                            .label("Connect")
                            .primary()
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| this.add_server(cx))),
                    ),
            )
            .child(rule(cx))
            .child(
                h_flex()
                    .gap_1()
                    .flex_wrap()
                    .items_center()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(prose("Want to run ricochet yourself?"))
                    .child(
                        Button::new("install-instead")
                            .label("Install it on a machine")
                            .link()
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.adding_server = false;
                                this.add_problem = None;
                                cx.emit(InstallInstead);
                            })),
                    ),
            )
    }

    fn not_running(&self, cx: &mut Context<Self>) -> Div {
        let server = self.server.clone();
        let busy = self.server.read(cx).activity.is_some();
        card(cx)
            .child(heading(
                "ricochet isn't running",
                Some("Start it to see its content here."),
                cx,
            ))
            .child(
                h_flex().child(
                    Button::new("start")
                        .icon(ph("play"))
                        .label("Start ricochet")
                        .primary()
                        .small()
                        .loading(busy)
                        .disabled(busy)
                        .on_click(move |_, _, cx| server.update(cx, |s, cx| s.start(cx))),
                ),
            )
    }

    fn body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if let Content::Web(url) = &self.content
            && !self.adding_server
        {
            let url = url.clone();
            return self.web_page(url, window, cx);
        }
        self.hide_web(cx);
        if self.adding_server || matches!(self.content, Content::NoServer) {
            return self
                .centered(self.add_server_form(cx), cx)
                .into_any_element();
        }
        match &self.content {
            Content::NoServer | Content::Web(_) => unreachable!("handled above"),
            Content::NotRunning => self.centered(self.not_running(cx), cx).into_any_element(),
            Content::Loading => self
                .centered(h_flex().justify_center().child(Spinner::new()), cx)
                .into_any_element(),
        }
    }
}

impl Render for DashboardView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.body(window, cx))
    }
}
