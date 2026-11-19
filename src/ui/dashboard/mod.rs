//! The content on a ricochet server, shown in the app instead of a browser.

use super::components::{card, error_alert, heading, prose, rule};
use super::icons::ph;
use crate::api::{self, ApiError, BrowserSignIn, Client, Credential, Deployment, Item};
use item::Tab;

mod item;
mod log;
mod pages;
use crate::controller::{DEFAULT_PORT, ServerController, ServerStatus};
use crate::servers::{Server, Servers};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Div, Entity, FontWeight, Hsla, Subscription, Window, div, px,
    uniform_list,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::spinner::Spinner;
use gpui_component::{ActiveTheme, Disableable, Sizable, h_flex, v_flex};
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const ITEM_HEIGHT: f32 = 52.0;

/// What the dashboard knows about the server it shows.
enum Content {
    /// No server to show yet.
    NoServer,
    /// The managed server is installed but not running.
    NotRunning,
    SignedOut,
    Loading,
    Loaded(Vec<Item>),
    Failed(String),
}

/// Something the dashboard reads from the server.
enum Loadable<T> {
    Idle,
    Loading,
    Loaded(T),
    Failed(String),
}

impl<T> Loadable<T> {
    fn from_result(result: api::Result<T>) -> Self {
        match result {
            Ok(value) => Self::Loaded(value),
            Err(error) => Self::Failed(error.to_string()),
        }
    }
}

/// A page of the Content section in the sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Apps,
    Tasks,
    Deployments,
    Deploy,
    Credentials,
}

impl Page {
    pub const ALL: [Self; 5] = [
        Self::Apps,
        Self::Tasks,
        Self::Deployments,
        Self::Deploy,
        Self::Credentials,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Apps => "Apps",
            Self::Tasks => "Tasks",
            Self::Deployments => "Deployments",
            Self::Deploy => "Deploy",
            Self::Credentials => "Credentials",
        }
    }

    /// The sidebar row's element id, apart from the settings pane of the same name.
    pub fn id(self) -> &'static str {
        match self {
            Self::Apps => "page-apps",
            Self::Tasks => "page-tasks",
            Self::Deployments => "page-deployments",
            Self::Deploy => "page-deploy",
            Self::Credentials => "page-credentials",
        }
    }

    /// The Phosphor icon beside the page in the sidebar.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Apps => "app-window",
            Self::Tasks => "timer",
            Self::Deployments => "clock-counter-clockwise",
            Self::Deploy => "rocket-launch",
            Self::Credentials => "key",
        }
    }
}

pub struct DashboardView {
    server: Entity<ServerController>,
    store: Servers,
    content: Content,
    /// Counts loads, so an answer from a server you already left is dropped.
    load_generation: u64,
    page: Page,
    selected: Option<String>,
    /// Counts the items opened, so an answer about one you already left is dropped.
    item_generation: u64,
    tab: Tab,
    deployments: Loadable<Vec<Deployment>>,
    runtime: Loadable<item::Runtime>,
    env_vars: Loadable<Vec<String>>,
    settings: Loadable<item::Settings>,
    access: String,
    log: log::LogView,
    /// Values for text fields that wait for a window to set them.
    pending_inputs: Vec<(Entity<InputState>, String)>,
    all_deployments: Loadable<Vec<(String, Deployment)>>,
    credentials: Loadable<Vec<Credential>>,
    deploy_credential: Option<String>,
    deploy_folder: Option<std::path::PathBuf>,
    deploying: bool,
    inputs: item::Inputs,
    search: Entity<InputState>,
    url_input: Entity<InputState>,
    key_input: Entity<InputState>,
    adding_server: bool,
    add_problem: Option<String>,
    /// Stops the browser sign-in while it waits.
    signing_in: Option<Arc<AtomicBool>>,
    sign_in_problem: Option<String>,
    /// The action running on the open item, such as "Saving".
    busy: Option<&'static str>,
    notice: Option<String>,
    /// The managed server and its status at the last load, to load again when either changes.
    seen_managed: (Option<Server>, ServerStatus),
    _subscriptions: Vec<Subscription>,
}

impl DashboardView {
    pub fn new(
        server: Entity<ServerController>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search content"));
        let url_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("ricochet.example.com"));
        let key_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("rico_…")
                .masked(true)
        });
        let subscriptions = vec![
            cx.observe(&server, |this, _, cx| {
                let seen = this.managed_state(cx);
                if seen != this.seen_managed {
                    let showing_managed = this.active_server(cx).is_none_or(|s| s.managed);
                    this.seen_managed = seen;
                    if showing_managed {
                        this.load(cx);
                    }
                }
            }),
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe(&url_input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.add_server(cx);
                }
            }),
            cx.subscribe(&key_input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.save_pasted_key(cx);
                }
            }),
        ];
        let mut view = Self {
            server,
            store: Servers::load(),
            content: Content::NoServer,
            load_generation: 0,
            page: Page::Apps,
            selected: None,
            item_generation: 0,
            tab: Tab::Overview,
            deployments: Loadable::Idle,
            runtime: Loadable::Idle,
            env_vars: Loadable::Idle,
            settings: Loadable::Idle,
            access: String::new(),
            log: log::LogView::default(),
            pending_inputs: Vec::new(),
            all_deployments: Loadable::Idle,
            credentials: Loadable::Idle,
            deploy_credential: None,
            deploy_folder: None,
            deploying: false,
            inputs: item::Inputs::new(window, cx),
            search,
            url_input,
            key_input,
            adding_server: false,
            add_problem: None,
            signing_in: None,
            sign_in_problem: None,
            busy: None,
            notice: None,
            seen_managed: (None, ServerStatus::Stopped),
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
            .then(|| Server::managed(&controller.connection, controller.dashboard()))
    }

    fn managed_state(&self, cx: &App) -> (Option<Server>, ServerStatus) {
        (self.managed_server(cx), self.server.read(cx).status)
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

    pub fn is_signed_in(&self, server: &Server) -> bool {
        self.store.key(server).is_some()
    }

    fn reset_selection(&mut self) {
        self.close_item();
        self.adding_server = false;
        self.notice = None;
    }

    /// Forgets what was read about the open item, and stops following its log.
    fn reset_item_state(&mut self) {
        self.deployments = Loadable::Idle;
        self.runtime = Loadable::Idle;
        self.env_vars = Loadable::Idle;
        self.settings = Loadable::Idle;
        self.log.stop();
        self.log = log::LogView::default();
        self.busy = None;
        self.notice = None;
    }

    /// Sets text fields that waited for a window.
    fn apply_pending_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (input, value) in std::mem::take(&mut self.pending_inputs) {
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }
    }

    pub fn show_server(&mut self, server: &Server, cx: &mut Context<Self>) {
        self.store.active = Some(server.id.clone());
        self.store.save();
        self.reset_selection();
        self.load(cx);
    }

    /// Shows the content of the server this app manages.
    pub fn show_managed(&mut self, cx: &mut Context<Self>) {
        self.store.active = None;
        self.store.save();
        self.reset_selection();
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
        let Some(url) = api::normalize_url(&text) else {
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
        self.reset_selection();
        self.load(cx);
    }

    pub fn sign_out(&mut self, server: &Server, cx: &mut Context<Self>) {
        self.store.set_key(server, None);
        self.store.save();
        self.reset_selection();
        self.load(cx);
    }

    // MARK: Loading

    fn client(&self, server: &Server) -> Option<Client> {
        Some(Client::new(&server.url, self.store.key(server)?))
    }

    /// Runs `work` against the server on a background thread, after opening the SSH tunnel for a managed one.
    fn reach<T: Send + 'static>(
        &self,
        server: &Server,
        cx: &App,
        work: impl FnOnce() -> api::Result<T> + Send + 'static,
    ) -> gpui::Task<api::Result<T>> {
        let backend = server.managed.then(|| self.server.read(cx).backend());
        let port = self.server.read(cx).proxy_port.unwrap_or(DEFAULT_PORT);
        cx.background_executor().spawn(async move {
            if let Some(backend) = backend {
                backend
                    .reach_dashboard(port)
                    .map_err(|error| ApiError::Failed(error.to_string()))?;
            }
            work()
        })
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.load_generation += 1;
        self.notice = None;
        let Some(server) = self.active_server(cx) else {
            self.content = Content::NoServer;
            cx.notify();
            return;
        };
        if server.managed && self.server.read(cx).status != ServerStatus::Running {
            self.content = Content::NotRunning;
            cx.notify();
            return;
        }
        let Some(client) = self.client(&server) else {
            self.content = Content::SignedOut;
            cx.notify();
            return;
        };
        self.content = Content::Loading;
        cx.notify();
        let generation = self.load_generation;
        let task = self.reach(&server, cx, move || client.items());
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.load_generation != generation {
                    return;
                }
                this.content = match result {
                    Ok(mut items) => {
                        items.sort_by_key(|item| item.name.to_lowercase());
                        let open = this
                            .selected
                            .clone()
                            .filter(|id| items.iter().any(|item| &item.id == id));
                        this.content = Content::Loaded(items);
                        match open {
                            Some(id) => this.open_item(id, this.tab, cx),
                            None => this.close_item(),
                        }
                        return cx.notify();
                    }
                    Err(ApiError::SignedOut) => {
                        this.store.set_key(&server, None);
                        this.store.save();
                        this.sign_in_problem = Some(ApiError::SignedOut.to_string());
                        Content::SignedOut
                    }
                    Err(ApiError::Failed(message)) => Content::Failed(message),
                };
                cx.notify();
            });
        })
        .detach();
    }

    /// Runs `work` with a client for the server shown, then hands its answer to `done`,
    /// unless you moved to another server or item in the meantime.
    fn fetch<T: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        work: impl FnOnce(&Client) -> api::Result<T> + Send + 'static,
        done: impl FnOnce(&mut Self, api::Result<T>, &mut Context<Self>) + 'static,
    ) {
        let Some(server) = self.active_server(cx) else {
            return;
        };
        let Some(client) = self.client(&server) else {
            return;
        };
        let generation = (self.load_generation, self.item_generation);
        let task = self.reach(&server, cx, move || work(&client));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if (this.load_generation, this.item_generation) != generation {
                    return;
                }
                if matches!(result, Err(ApiError::SignedOut)) {
                    this.store.set_key(&server, None);
                    this.store.save();
                    this.sign_in_problem = Some(ApiError::SignedOut.to_string());
                    this.content = Content::SignedOut;
                } else {
                    done(this, result, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    // MARK: Pages

    pub fn page(&self) -> Page {
        self.page
    }

    pub fn show_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.adding_server = false;
        match page {
            Page::Apps | Page::Tasks => {
                let keep = self
                    .selected_item()
                    .is_some_and(|item| item.is_task() == (page == Page::Tasks));
                if !keep {
                    self.close_item();
                }
            }
            Page::Deployments => self.load_all_deployments(cx),
            Page::Deploy | Page::Credentials => self.load_credentials(cx),
        }
        cx.notify();
    }

    fn items(&self) -> &[Item] {
        match &self.content {
            Content::Loaded(items) => items,
            _ => &[],
        }
    }

    fn selected_item(&self) -> Option<Item> {
        let id = self.selected.as_ref()?;
        self.items().iter().find(|item| &item.id == id).cloned()
    }

    fn close_item(&mut self) {
        self.selected = None;
        self.item_generation += 1;
        self.reset_item_state();
    }

    /// Opens an item on its own page, at `tab`.
    fn open_item(&mut self, id: String, tab: Tab, cx: &mut Context<Self>) {
        if let Some(item) = self.items().iter().find(|item| item.id == id) {
            self.page = if item.is_task() {
                Page::Tasks
            } else {
                Page::Apps
            };
        }
        if self.selected.as_deref() != Some(&id) {
            self.selected = Some(id);
            self.item_generation += 1;
            self.reset_item_state();
        }
        self.show_tab(tab, cx);
    }

    // MARK: Signing in

    fn sign_in_with_browser(&mut self, cx: &mut Context<Self>) {
        let Some(server) = self.active_server(cx) else {
            return;
        };
        let sign_in = match BrowserSignIn::start(&server.url) {
            Ok(sign_in) => sign_in,
            Err(error) => {
                self.sign_in_problem = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        self.signing_in = Some(sign_in.canceller());
        self.sign_in_problem = None;
        cx.notify();
        let task = self.reach(&server, cx, move || {
            let _ = open::that(&sign_in.url);
            sign_in.wait()
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.signing_in = None;
                match result {
                    Ok(key) => {
                        this.store.set_key(&server, Some(key));
                        this.store.save();
                        this.load(cx);
                    }
                    Err(error) => {
                        this.sign_in_problem = Some(error.to_string());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn cancel_sign_in(&mut self, cx: &mut Context<Self>) {
        if let Some(cancelled) = self.signing_in.take() {
            cancelled.store(true, Ordering::Relaxed);
        }
        cx.notify();
    }

    fn save_pasted_key(&mut self, cx: &mut Context<Self>) {
        let key = self.key_input.read(cx).value().trim().to_owned();
        let Some(server) = self.active_server(cx) else {
            return;
        };
        if key.is_empty() {
            return;
        }
        self.store.set_key(&server, Some(key));
        self.store.save();
        self.sign_in_problem = None;
        self.load(cx);
    }

    // MARK: States

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
                "Add a server",
                Some("Show the content on any ricochet server you can sign in to."),
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
                            .icon(ph("plus"))
                            .label("Add server")
                            .primary()
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| this.add_server(cx))),
                    ),
            )
    }

    fn sign_in_form(&self, server: &Server, cx: &mut Context<Self>) -> Div {
        let waiting = self.signing_in.is_some();
        card(cx)
            .child(heading(
                &format!("Sign in to {}", server.name),
                Some("ricochet creates an API key for this app when you sign in. It lasts eight hours."),
                cx,
            ))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("sign-in")
                            .icon(ph("sign-in"))
                            .label(if waiting {
                                "Waiting for your browser…"
                            } else {
                                "Sign in with your browser"
                            })
                            .primary()
                            .small()
                            .loading(waiting)
                            .disabled(waiting)
                            .on_click(cx.listener(|this, _, _, cx| this.sign_in_with_browser(cx))),
                    )
                    .when(waiting, |row| {
                        row.child(
                            Button::new("cancel-sign-in")
                                .label("Cancel")
                                .outline()
                                .small()
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_sign_in(cx))),
                        )
                    }),
            )
            .child(rule(cx))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(prose(
                                "Or paste an API key you created under Credentials in ricochet.",
                            )),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&self.key_input)))
                            .child(
                                Button::new("save-key")
                                    .label("Use key")
                                    .outline()
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| this.save_pasted_key(cx))),
                            ),
                    ),
            )
            .children(self.sign_in_problem.clone().map(|problem| {
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(prose(problem))
            }))
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

    // MARK: Content

    fn visible_items(&self, cx: &App) -> Vec<Item> {
        let query = self.search.read(cx).value().to_lowercase();
        let tasks = self.page == Page::Tasks;
        self.items()
            .iter()
            .filter(|item| item.is_task() == tasks)
            .filter(|item| {
                query.is_empty()
                    || item.name.to_lowercase().contains(&query)
                    || item.kind().to_lowercase().contains(&query)
                    || item
                        .tags
                        .iter()
                        .flatten()
                        .any(|tag| tag.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
    }

    fn item_rows(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let items = self.visible_items(cx);
        let accent = cx.theme().sidebar_accent;
        let muted = cx.theme().muted_foreground;
        items
            .into_iter()
            .enumerate()
            .skip(range.start)
            .take(range.len())
            .map(|(index, item)| {
                let selected = self.selected.as_deref() == Some(&item.id);
                let color = status_color(item.deployment_status.as_deref().unwrap_or(""), cx);
                let id = item.id.clone();
                h_flex()
                    .id(("item", index))
                    .h(px(ITEM_HEIGHT))
                    .px_3()
                    .gap_3()
                    .items_center()
                    .cursor_pointer()
                    .map(|row| {
                        if selected {
                            row.bg(accent)
                        } else {
                            row.hover(move |row| row.bg(accent.opacity(0.8)))
                        }
                    })
                    .child(ph(kind_icon(&item)).size_4().text_color(muted))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(item.name.clone()),
                            )
                            .child(div().text_xs().text_color(muted).truncate().child(format!(
                                "{} · {}",
                                item.kind(),
                                item.language_name()
                            ))),
                    )
                    .child(div().size(px(6.0)).rounded_full().bg(color))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let tab = this.tab;
                        this.open_item(id.clone(), tab, cx)
                    }))
                    .into_any_element()
            })
            .collect()
    }

    fn content_list(&self, cx: &mut Context<Self>) -> Div {
        let count = self.visible_items(cx).len();
        let empty = self
            .items()
            .iter()
            .all(|item| item.is_task() != (self.page == Page::Tasks));
        v_flex()
            .w(px(260.0))
            .flex_shrink_0()
            .h_full()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .gap_2()
                    .p_3()
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.search).cleanable(true)),
                    )
                    .child(
                        Button::new("refresh")
                            .icon(ph("arrow-clockwise"))
                            .tooltip("Refresh")
                            .ghost()
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| this.load(cx))),
                    ),
            )
            .child(if empty {
                div()
                    .p_4()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(prose(if self.page == Page::Tasks {
                        "No tasks are deployed to this server yet."
                    } else {
                        "No apps are deployed to this server yet."
                    }))
                    .into_any_element()
            } else {
                uniform_list(
                    "content-list",
                    count,
                    cx.processor(|this, range, _, cx| this.item_rows(range, cx)),
                )
                .flex_1()
                .min_h_0()
                .into_any_element()
            })
    }

    fn body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.adding_server || matches!(self.content, Content::NoServer) {
            return self
                .centered(self.add_server_form(cx), cx)
                .into_any_element();
        }
        match &self.content {
            Content::NoServer => unreachable!("handled above"),
            Content::NotRunning => self.centered(self.not_running(cx), cx).into_any_element(),
            Content::SignedOut => {
                let Some(server) = self.active_server(cx) else {
                    return div().into_any_element();
                };
                self.centered(self.sign_in_form(&server, cx), cx)
                    .into_any_element()
            }
            Content::Loading => self
                .centered(h_flex().justify_center().child(Spinner::new()), cx)
                .into_any_element(),
            Content::Failed(message) => {
                let alert = error_alert(
                    "Couldn't load content",
                    message,
                    cx.listener(|this, _, _, cx| this.load(cx)),
                    cx,
                );
                self.centered(alert, cx).into_any_element()
            }
            Content::Loaded(_) => match self.page {
                Page::Apps | Page::Tasks => h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.content_list(cx))
                    .child(self.item_detail(window, cx))
                    .into_any_element(),
                Page::Deployments => self.deployments_page(cx),
                Page::Deploy => self.deploy_page(cx),
                Page::Credentials => self.credentials_page(cx),
            },
        }
    }
}

impl Render for DashboardView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply_pending_inputs(window, cx);
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.body(window, cx))
    }
}

fn status_color(status: &str, cx: &App) -> Hsla {
    match status {
        "success" => cx.theme().success,
        "failure" | "timeout" => cx.theme().danger,
        "pending" => cx.theme().warning,
        _ => cx.theme().muted_foreground,
    }
}

fn kind_icon(item: &Item) -> &'static str {
    match item.kind().as_str() {
        "API" => "code-simple",
        "Script" => "timer",
        kind if kind.ends_with("document") => "book-open",
        _ => "app-window",
    }
}

/// A Unix timestamp in this computer's time zone, such as "Nov 19, 2026 at 14:05".
fn when(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%b %-d, %Y at %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_read_as_words() {
        assert_eq!(api::humanize("success"), "Success");
        assert_eq!(api::humanize("dependency_mismatch"), "Dependency mismatch");
        assert_eq!(api::humanize(""), "");
    }
}
