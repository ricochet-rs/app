//! The content on a ricochet server, shown in the app instead of a browser.

use super::components::{card, card_title, error_alert, heading, prose, rule};
use super::icons::ph;
use crate::api::{self, ApiError, BrowserSignIn, Client, Deployment, Item};
use crate::controller::{DEFAULT_PORT, ServerController, ServerStatus};
use crate::servers::{Server, Servers};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Div, Entity, FontWeight, Hsla, Subscription, Window, div, px,
    uniform_list,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
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

enum Deployments {
    Loading,
    Loaded(Vec<Deployment>),
    Failed(String),
}

pub struct DashboardView {
    server: Entity<ServerController>,
    store: Servers,
    content: Content,
    /// Counts loads, so an answer from a server you already left is dropped.
    load_generation: u64,
    selected: Option<String>,
    deployments: Option<Deployments>,
    search: Entity<InputState>,
    url_input: Entity<InputState>,
    key_input: Entity<InputState>,
    adding_server: bool,
    add_problem: Option<String>,
    /// Stops the browser sign-in while it waits.
    signing_in: Option<Arc<AtomicBool>>,
    sign_in_problem: Option<String>,
    running: bool,
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
            selected: None,
            deployments: None,
            search,
            url_input,
            key_input,
            adding_server: false,
            add_problem: None,
            signing_in: None,
            sign_in_problem: None,
            running: false,
            notice: None,
            seen_managed: (None, ServerStatus::Stopped),
            _subscriptions: subscriptions,
        };
        view.seen_managed = view.managed_state(cx);
        view.load(cx);
        view
    }

    /// Whether there is any server to show, managed or added.
    pub fn has_servers(&self, cx: &App) -> bool {
        !self.choices(cx).is_empty()
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

    fn choices(&self, cx: &App) -> Vec<Server> {
        self.store.choices(self.managed_server(cx))
    }

    fn active_server(&self, cx: &App) -> Option<Server> {
        let choices = self.choices(cx);
        let active = self.store.active.as_deref();
        choices
            .iter()
            .find(|server| Some(server.id.as_str()) == active)
            .or(choices.first())
            .cloned()
    }

    fn show_server(&mut self, server: &Server, cx: &mut Context<Self>) {
        self.store.active = Some(server.id.clone());
        self.store.save();
        self.selected = None;
        self.deployments = None;
        self.adding_server = false;
        self.load(cx);
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

    fn remove_server(&mut self, server: &Server, cx: &mut Context<Self>) {
        self.store.remove(server);
        self.store.save();
        self.selected = None;
        self.deployments = None;
        self.load(cx);
    }

    fn sign_out(&mut self, server: &Server, cx: &mut Context<Self>) {
        self.store.set_key(server, None);
        self.store.save();
        self.selected = None;
        self.deployments = None;
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
                        Content::Loaded(items)
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

    fn select_item(&mut self, id: String, cx: &mut Context<Self>) {
        if self.selected.as_deref() == Some(&id) {
            return;
        }
        self.selected = Some(id.clone());
        self.notice = None;
        self.deployments = Some(Deployments::Loading);
        cx.notify();
        let Some(server) = self.active_server(cx) else {
            return;
        };
        let Some(client) = self.client(&server) else {
            return;
        };
        let task = self.reach(&server, cx, move || client.deployments(&id));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.deployments = Some(match result {
                    Ok(deployments) => Deployments::Loaded(deployments),
                    Err(error) => Deployments::Failed(error.to_string()),
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn run_task(&mut self, item: Item, cx: &mut Context<Self>) {
        let Some(server) = self.active_server(cx) else {
            return;
        };
        let Some(client) = self.client(&server) else {
            return;
        };
        self.running = true;
        self.notice = None;
        cx.notify();
        let id = item.id.clone();
        let task = self.reach(&server, cx, move || client.run(&id));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.running = false;
                this.notice = Some(match result {
                    Ok(()) => format!("Started a run of {}.", item.name),
                    Err(error) => error.to_string(),
                });
                cx.notify();
            });
        })
        .detach();
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

    // MARK: Header

    fn header(&self, cx: &mut Context<Self>) -> Div {
        let active = self.active_server(cx);
        let choices = self.choices(cx);
        let signed_in = active
            .as_ref()
            .is_some_and(|server| self.store.key(server).is_some());
        let view = cx.entity();
        let menu_server = active.clone();
        let menu = Button::new("server-menu")
            .icon(ph(match &active {
                Some(server) if server.managed => "desktop",
                _ => "globe",
            }))
            .label(
                active
                    .as_ref()
                    .map_or("Add a server".to_owned(), |s| s.name.clone()),
            )
            .outline()
            .small()
            .dropdown_menu(move |mut menu, _, _| {
                for choice in &choices {
                    let target = choice.clone();
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(choice.name.clone())
                            .icon(ph(if choice.managed { "desktop" } else { "globe" }))
                            .checked(Some(choice) == menu_server.as_ref())
                            .on_click(move |_, _, cx| {
                                view.update(cx, |this, cx| this.show_server(&target, cx))
                            }),
                    );
                }
                let add = view.clone();
                menu = menu.separator().item(
                    PopupMenuItem::new("Add server")
                        .icon(ph("plus"))
                        .on_click(move |_, _, cx| {
                            add.update(cx, |this, cx| {
                                this.adding_server = true;
                                cx.notify();
                            })
                        }),
                );
                if let Some(server) = menu_server.clone() {
                    if signed_in {
                        let view = view.clone();
                        let server = server.clone();
                        menu = menu.item(
                            PopupMenuItem::new(format!("Sign out of {}", server.name))
                                .icon(ph("sign-in"))
                                .on_click(move |_, _, cx| {
                                    view.update(cx, |this, cx| this.sign_out(&server, cx))
                                }),
                        );
                    }
                    if !server.managed {
                        let view = view.clone();
                        menu = menu.item(
                            PopupMenuItem::new(format!("Remove {}", server.name))
                                .icon(ph("trash"))
                                .on_click(move |_, _, cx| {
                                    view.update(cx, |this, cx| this.remove_server(&server, cx))
                                }),
                        );
                    }
                }
                menu
            });
        let loading = matches!(self.content, Content::Loading);
        h_flex()
            .gap_3()
            .px_4()
            .py_3()
            .items_center()
            .child(menu)
            .child(div().flex_1())
            .children(active.as_ref().map(|server| {
                let url = server.url.clone();
                Button::new("open-in-browser")
                    .icon(ph("arrow-square-out"))
                    .tooltip("Open in your browser")
                    .outline()
                    .small()
                    .on_click(move |_, _, _| {
                        let _ = open::that(&url);
                    })
            }))
            .child(
                Button::new("refresh")
                    .icon(ph("arrow-clockwise"))
                    .tooltip("Refresh")
                    .outline()
                    .small()
                    .loading(loading)
                    .disabled(loading || active.is_none())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.selected = None;
                        this.deployments = None;
                        this.load(cx);
                    })),
            )
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
        let cancellable = self.has_servers(cx);
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
        let Content::Loaded(items) = &self.content else {
            return Vec::new();
        };
        let query = self.search.read(cx).value().to_lowercase();
        items
            .iter()
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
                let color = status_color(&item.deployment_status, cx);
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
                    .on_click(cx.listener(move |this, _, _, cx| this.select_item(id.clone(), cx)))
                    .into_any_element()
            })
            .collect()
    }

    fn content_list(&self, cx: &mut Context<Self>) -> Div {
        let count = self.visible_items(cx).len();
        let empty = matches!(&self.content, Content::Loaded(items) if items.is_empty());
        v_flex()
            .w(px(300.0))
            .flex_shrink_0()
            .h_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .child(div().p_3().child(Input::new(&self.search).cleanable(true)))
            .child(if empty {
                div()
                    .p_4()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(prose("Nothing is deployed to this server yet."))
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

    fn item_detail(&self, cx: &mut Context<Self>) -> AnyElement {
        let Content::Loaded(items) = &self.content else {
            return div().into_any_element();
        };
        let Some(item) = self
            .selected
            .as_ref()
            .and_then(|id| items.iter().find(|item| &item.id == id))
            .cloned()
        else {
            return v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("Choose an item to see its details and deployments.")
                .into_any_element();
        };
        let server_url = self
            .active_server(cx)
            .map(|server| server.url)
            .unwrap_or_default();
        let open_url = format!("{server_url}/app/{}/", item.id);
        let facts = [
            ("Type", item.kind().to_owned()),
            ("Language", item.language_name().to_owned()),
            ("Access", capitalized(&item.access_type)),
            ("Updated", when(item.modified_at)),
        ];
        let run_item = item.clone();
        div()
            .id("item-detail")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .gap_6()
                    .p_6()
                    .child(heading(&item.name, item.summary.as_deref(), cx))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("open-item")
                                    .icon(ph("arrow-square-out"))
                                    .label("Open")
                                    .primary()
                                    .small()
                                    .on_click(move |_, _, _| {
                                        let _ = open::that(&open_url);
                                    }),
                            )
                            .when(item.is_task(), |row| {
                                row.child(
                                    Button::new("run-item")
                                        .icon(ph("play"))
                                        .label("Run now")
                                        .outline()
                                        .small()
                                        .loading(self.running)
                                        .disabled(self.running)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.run_task(run_item.clone(), cx)
                                        })),
                                )
                            })
                            .children(self.notice.clone().map(|notice| {
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(notice)
                            })),
                    )
                    .child(card(cx).child(h_flex().gap_6().flex_wrap().children(
                        facts.into_iter().map(|(label, value)| {
                            v_flex()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(label),
                                )
                                .child(div().text_sm().child(value))
                        }),
                    )))
                    .when_some(item.tags.clone().filter(|t| !t.is_empty()), |pane, tags| {
                        pane.child(
                            h_flex()
                                .gap_1p5()
                                .flex_wrap()
                                .children(tags.into_iter().map(|tag| {
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .text_xs()
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .child(tag)
                                })),
                        )
                    })
                    .child(self.deployment_history(cx)),
            )
            .into_any_element()
    }

    fn deployment_history(&self, cx: &App) -> Div {
        let body = match &self.deployments {
            None | Some(Deployments::Loading) => {
                h_flex().child(Spinner::new().small()).into_any_element()
            }
            Some(Deployments::Failed(message)) => div()
                .text_sm()
                .text_color(cx.theme().danger)
                .child(prose(message.clone()))
                .into_any_element(),
            Some(Deployments::Loaded(deployments)) if deployments.is_empty() => div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("No deployments yet.")
                .into_any_element(),
            Some(Deployments::Loaded(deployments)) => v_flex()
                .children(deployments.iter().take(20).enumerate().map(|(index, d)| {
                    let version = [
                        d.matched_ver.clone(),
                        d.git_hash
                            .as_ref()
                            .map(|hash| hash.chars().take(7).collect()),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<String>>()
                    .join(" · ");
                    h_flex()
                        .gap_3()
                        .py_2()
                        .items_center()
                        .when(index > 0, |row| {
                            row.border_t_1().border_color(cx.theme().border)
                        })
                        .child(
                            div()
                                .size(px(6.0))
                                .rounded_full()
                                .bg(status_color(&d.status, cx)),
                        )
                        .child(div().w(px(110.0)).text_sm().child(capitalized(&d.status)))
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(when(d.deployed_at)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_color(cx.theme().muted_foreground)
                                .child(version),
                        )
                }))
                .into_any_element(),
        };
        card(cx).child(card_title("Deployments", cx)).child(body)
    }

    fn body(&self, cx: &mut Context<Self>) -> AnyElement {
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
            Content::Loaded(_) => h_flex()
                .flex_1()
                .min_h_0()
                .child(self.content_list(cx))
                .child(self.item_detail(cx))
                .into_any_element(),
        }
    }
}

impl Render for DashboardView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.header(cx))
            .child(rule(cx))
            .child(self.body(cx))
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
    match item.kind() {
        "API" => "code-simple",
        "Script" => "timer",
        kind if kind.ends_with("document") => "book-open",
        _ => "app-window",
    }
}

fn capitalized(text: &str) -> String {
    let text = text.replace('_', " ");
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
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
        assert_eq!(capitalized("success"), "Success");
        assert_eq!(capitalized("dependency_mismatch"), "Dependency mismatch");
        assert_eq!(capitalized(""), "");
    }
}
