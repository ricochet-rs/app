//! One content item's page: its overview, logs, runtime, settings and environment variables.

use super::log::LogSource;
use super::{DashboardView, Loadable, when};
use crate::api::{self, ActiveRun, ContentSettings, Instance, Item};
use crate::ui::components::{card, card_title, danger_button, field, pill, prose, tab_bar};
use crate::ui::icons::ph;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Div, Entity, FontWeight, Window, div, px};
use gpui_component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_component::dialog::DialogButtonProps;
use gpui_component::input::{Input, InputState};
use gpui_component::spinner::Spinner;
use gpui_component::{ActiveTheme, Disableable, Sizable, WindowExt as _, h_flex, v_flex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Logs,
    Runtime,
    Settings,
    Environment,
}

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Logs => "Logs",
            Self::Runtime => "Runtime",
            Self::Settings => "Settings",
            Self::Environment => "Environment variables",
        }
    }

    /// The tabs you can open on `item`, given your role on it.
    fn for_item(item: &Item) -> Vec<Self> {
        let mut tabs = vec![Self::Overview, Self::Logs, Self::Runtime];
        if item.can_edit() {
            tabs.extend([Self::Settings, Self::Environment]);
        }
        tabs
    }
}

/// The text fields of an item's tabs and of the Deploy page.
pub struct Inputs {
    pub name: Entity<InputState>,
    pub summary: Entity<InputState>,
    pub tags: Entity<InputState>,
    pub schedule: Entity<InputState>,
    pub env_name: Entity<InputState>,
    pub env_value: Entity<InputState>,
    pub git_url: Entity<InputState>,
    pub git_branch: Entity<InputState>,
    pub git_path: Entity<InputState>,
}

impl Inputs {
    pub fn new(window: &mut Window, cx: &mut Context<DashboardView>) -> Self {
        let mut input = |placeholder: &'static str| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let inputs = Self {
            name: input("Sales dashboard"),
            summary: input("What the item shows or does"),
            tags: input("sales, finance"),
            schedule: input("0 6 * * 1-5"),
            env_name: input("API_TOKEN"),
            env_value: input("Value"),
            git_url: input("https://github.com/you/project.git"),
            git_branch: input("main"),
            git_path: input("Folder inside the repository, if not the root"),
        };
        inputs
            .env_value
            .update(cx, |input, cx| input.set_masked(true, window, cx));
        inputs
    }
}

/// What runs of an item right now: an app's instances, or a task's unfinished runs.
pub struct Runtime {
    pub instances: Vec<Instance>,
    pub runs: Vec<ActiveRun>,
}

/// The settings the Settings tab edits, as read from the item's `_ricochet.toml`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    pub name: String,
    pub summary: String,
    pub access: String,
    pub tags: Vec<String>,
    pub schedule: Option<String>,
}

impl Settings {
    pub fn parse(text: &str) -> Option<Self> {
        let toml: toml_edit::DocumentMut = text.parse().ok()?;
        let content = &toml["content"];
        let text = |key: &str| {
            content
                .get(key)
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_owned()
        };
        Some(Self {
            name: text("name"),
            summary: text("summary"),
            access: text("access_type"),
            tags: content
                .get("tags")
                .and_then(|tags| tags.as_array())
                .map(|tags| {
                    tags.iter()
                        .filter_map(|tag| tag.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            schedule: toml
                .get("schedule")
                .and_then(|schedule| schedule.get("cron"))
                .and_then(|cron| cron.as_str())
                .map(str::to_owned),
        })
    }
}

const ACCESS: [(&str, &str); 3] = [
    ("private", "Private"),
    ("internal", "Internal"),
    ("external", "External"),
];

impl DashboardView {
    // MARK: Loading

    pub(super) fn show_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        let Some(item) = self.selected_item() else {
            return;
        };
        self.tab = if Tab::for_item(&item).contains(&tab) {
            tab
        } else {
            Tab::Overview
        };
        match self.tab {
            Tab::Overview => {
                if matches!(self.deployments, Loadable::Idle) {
                    self.load_deployments(&item, cx);
                }
            }
            Tab::Logs => {
                if matches!(self.runtime, Loadable::Idle) {
                    self.load_runtime(&item, cx);
                }
                if matches!(self.deployments, Loadable::Idle) {
                    self.load_deployments(&item, cx);
                }
                if self.log.source.is_none()
                    && let Some(deployment) = item.deployment_id.clone()
                {
                    self.follow_log(LogSource::Build { deployment }, cx);
                }
            }
            Tab::Runtime => self.load_runtime(&item, cx),
            Tab::Settings => self.load_settings(&item, cx),
            Tab::Environment => self.load_env_vars(&item, cx),
        }
        cx.notify();
    }

    fn load_deployments(&mut self, item: &Item, cx: &mut Context<Self>) {
        let id = item.id.clone();
        self.deployments = Loadable::Loading;
        self.fetch(
            cx,
            move |client| client.deployments(&id),
            |this, result, _| this.deployments = Loadable::from_result(result),
        );
    }

    fn load_runtime(&mut self, item: &Item, cx: &mut Context<Self>) {
        let (id, task) = (item.id.clone(), item.is_task());
        if matches!(self.runtime, Loadable::Idle) {
            self.runtime = Loadable::Loading;
        }
        self.fetch(
            cx,
            move |client| {
                Ok(if task {
                    Runtime {
                        instances: Vec::new(),
                        runs: client.active_runs(&id)?,
                    }
                } else {
                    Runtime {
                        instances: client.instances(&id)?,
                        runs: Vec::new(),
                    }
                })
            },
            |this, result, _| this.runtime = Loadable::from_result(result),
        );
    }

    fn load_settings(&mut self, item: &Item, cx: &mut Context<Self>) {
        let id = item.id.clone();
        self.settings = Loadable::Loading;
        self.fetch(
            cx,
            move |client| {
                let text = client.ricochet_toml(&id)?;
                Settings::parse(&text).ok_or_else(|| {
                    api::ApiError::Failed("ricochet sent settings this app can't read.".into())
                })
            },
            |this, result, _| {
                if let Ok(settings) = &result {
                    this.access = settings.access.clone();
                    this.pending_inputs.extend([
                        (this.inputs.name.clone(), settings.name.clone()),
                        (this.inputs.summary.clone(), settings.summary.clone()),
                        (this.inputs.tags.clone(), settings.tags.join(", ")),
                        (
                            this.inputs.schedule.clone(),
                            settings.schedule.clone().unwrap_or_default(),
                        ),
                    ]);
                }
                this.settings = Loadable::from_result(result);
            },
        );
    }

    fn load_env_vars(&mut self, item: &Item, cx: &mut Context<Self>) {
        let id = item.id.clone();
        self.env_vars = Loadable::Loading;
        self.fetch(
            cx,
            move |client| client.env_vars(&id),
            |this, result, _| this.env_vars = Loadable::from_result(result),
        );
    }

    // MARK: Actions

    /// Runs `work` on the open item while `busy` shows, then shows `notice` or the error.
    fn act<T: Send + 'static>(
        &mut self,
        busy: &'static str,
        cx: &mut Context<Self>,
        work: impl FnOnce(&api::Client, &str) -> api::Result<T> + Send + 'static,
        done: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
    ) {
        let Some(item) = self.selected_item() else {
            return;
        };
        self.busy = Some(busy);
        self.notice = None;
        cx.notify();
        let id = item.id.clone();
        self.fetch(
            cx,
            move |client| work(client, &id),
            |this, result, cx| {
                this.busy = None;
                match result {
                    Ok(value) => done(this, value, cx),
                    Err(error) => this.notice = Some(error.to_string()),
                }
            },
        );
    }

    fn run_now(&mut self, cx: &mut Context<Self>) {
        self.act(
            "Starting",
            cx,
            |client, id| client.run(id),
            |this, run, cx| {
                this.notice = Some("Started a run.".into());
                this.runtime = Loadable::Idle;
                this.follow_log(LogSource::Log { id: run }, cx);
                this.show_tab(Tab::Logs, cx);
            },
        );
    }

    fn stop_run(&mut self, run: String, cx: &mut Context<Self>) {
        self.act(
            "Stopping",
            cx,
            move |client, id| client.stop_run(id, &run),
            |this, (), cx| {
                this.notice = Some("Stopped the run.".into());
                this.show_tab(Tab::Runtime, cx);
            },
        );
    }

    fn stop_instance(&mut self, instance: String, cx: &mut Context<Self>) {
        self.act(
            "Stopping",
            cx,
            move |client, id| client.stop_instance(id, &instance),
            |this, (), cx| {
                this.notice = Some("Stopped the instance.".into());
                this.show_tab(Tab::Runtime, cx);
            },
        );
    }

    fn save_settings(&mut self, cx: &mut Context<Self>) {
        let value = |input: &Entity<InputState>| input.read(cx).value().trim().to_owned();
        let name = value(&self.inputs.name);
        let changes = ContentSettings {
            name: (!name.is_empty()).then_some(name),
            summary: Some(value(&self.inputs.summary)),
            access_type: (!self.access.is_empty()).then(|| self.access.clone()),
            tags: Some(
                value(&self.inputs.tags)
                    .split(',')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(str::to_owned)
                    .collect(),
            ),
        };
        self.act(
            "Saving",
            cx,
            move |client, id| client.update_settings(id, &changes),
            |this, (), cx| {
                this.notice = Some("Saved.".into());
                this.load(cx);
            },
        );
    }

    fn save_schedule(&mut self, cx: &mut Context<Self>) {
        let cron = self.inputs.schedule.read(cx).value().trim().to_owned();
        if cron.is_empty() {
            self.notice = Some("Enter a cron schedule, such as `0 6 * * 1-5`.".into());
            cx.notify();
            return;
        }
        self.act(
            "Saving schedule",
            cx,
            move |client, id| client.set_schedule(id, &cron),
            |this, (), _| this.notice = Some("Saved the schedule.".into()),
        );
    }

    fn add_env_var(&mut self, cx: &mut Context<Self>) {
        let name = self.inputs.env_name.read(cx).value().trim().to_owned();
        let value = self.inputs.env_value.read(cx).value().to_string();
        if name.is_empty() {
            self.notice = Some("Enter a name for the variable.".into());
            cx.notify();
            return;
        }
        self.act(
            "Adding",
            cx,
            move |client, id| client.set_env_var(id, &name, &value),
            |this, names, _| {
                this.env_vars = Loadable::Loaded(names);
                this.pending_inputs.extend([
                    (this.inputs.env_name.clone(), String::new()),
                    (this.inputs.env_value.clone(), String::new()),
                ]);
            },
        );
    }

    fn delete_env_var(&mut self, name: String, cx: &mut Context<Self>) {
        self.act(
            "Removing",
            cx,
            move |client, id| client.delete_env_var(id, &name),
            |this, names, _| this.env_vars = Loadable::Loaded(names),
        );
    }

    fn confirm_delete(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let view = view.clone();
            dialog
                .title(prose(format!("Delete {}?", item.name)))
                .child(prose(
                    "ricochet stops it and deletes its deployments, logs and settings. This can't be undone.",
                ))
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Delete")
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text("Cancel"),
                )
                .on_ok(move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        this.act(
                            "Deleting",
                            cx,
                            |client, id| client.delete_item(id),
                            |this, (), cx| {
                                this.close_item();
                                this.load(cx);
                            },
                        )
                    });
                    true
                })
        });
    }

    // MARK: Rendering

    pub(super) fn item_detail(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let _ = window;
        let Some(item) = self.selected_item() else {
            return v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("Choose an item to see its details.")
                .into_any_element();
        };
        let server_url = self
            .active_server(cx)
            .map(|server| server.url)
            .unwrap_or_default();
        let open_url = format!("{server_url}/app/{}/", item.id);
        let tabs = Tab::for_item(&item);
        let view = cx.entity();
        let tab_list = tabs.clone();
        let bar = tab_bar(
            "item-tabs",
            tabs.iter()
                .map(|tab| (tab.title(), *tab == self.tab))
                .collect(),
            move |index, _, cx| {
                let tab = tab_list[index];
                view.update(cx, |this, cx| this.show_tab(tab, cx));
            },
        );
        let body = match self.tab {
            Tab::Overview => self.overview_tab(&item, cx),
            Tab::Logs => self.logs_tab(&item, cx),
            Tab::Runtime => self.runtime_tab(&item, cx),
            Tab::Settings => self.settings_tab(&item, cx),
            Tab::Environment => self.env_tab(cx),
        };
        div()
            .id("item-detail")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .gap_5()
                    .p_6()
                    .child(
                        h_flex().child(
                            Button::new("back-to-list")
                                .icon(ph("arrow-left"))
                                .label(if item.is_task() { "Tasks" } else { "Apps" })
                                .ghost()
                                .small()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.close_item();
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_start()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(item.name.clone()),
                                    )
                                    .children(item.summary.clone().filter(|s| !s.is_empty()).map(
                                        |summary| {
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(prose(summary))
                                        },
                                    )),
                            )
                            .when(item.is_task(), |row| {
                                row.child(
                                    Button::new("run-item")
                                        .icon(ph("play"))
                                        .label("Run now")
                                        .outline()
                                        .loading(self.busy == Some("Starting"))
                                        .disabled(self.busy.is_some())
                                        .on_click(cx.listener(|this, _, _, cx| this.run_now(cx))),
                                )
                            })
                            .child(
                                Button::new("open-item")
                                    .icon(ph("arrow-square-out"))
                                    .label("View live")
                                    .primary()
                                    .on_click(move |_, _, _| {
                                        let _ = open::that(&open_url);
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(pill("Content type", item.kind(), cx))
                            .child(pill("Language", item.language_name().to_owned(), cx))
                            .child(pill("Visibility", api::humanize(&item.access_type), cx))
                            .child(pill("Updated", super::ago(item.modified_at), cx)),
                    )
                    .children(self.notice.clone().map(|notice| {
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(prose(notice))
                    }))
                    .child(bar)
                    .child(body),
            )
            .into_any_element()
    }

    fn overview_tab(&self, item: &Item, cx: &mut Context<Self>) -> AnyElement {
        let status = item
            .deployment_status
            .as_deref()
            .map(api::humanize)
            .unwrap_or_else(|| "Not deployed".into());
        let role = item
            .role
            .as_deref()
            .map(api::humanize)
            .unwrap_or_else(|| "Admin".into());
        v_flex()
            .gap_5()
            .child(
                card(cx).child(card_title("Status", cx)).child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .child(pill("Deployment", status, cx))
                        .child(pill("Last updated", when(item.modified_at), cx))
                        .child(pill("Your role", role, cx)),
                ),
            )
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
            .child(self.deployment_history(cx))
            .into_any_element()
    }

    fn deployment_history(&self, cx: &mut Context<Self>) -> Div {
        let body = match &self.deployments {
            Loadable::Idle | Loadable::Loading => {
                h_flex().child(Spinner::new().small()).into_any_element()
            }
            Loadable::Failed(message) => failed(message, cx),
            Loadable::Loaded(deployments) if deployments.is_empty() => {
                muted("No deployments yet.", cx)
            }
            Loadable::Loaded(deployments) => v_flex()
                .children(deployments.iter().take(20).enumerate().map(|(index, d)| {
                    let deployment = d.id.clone();
                    deployment_row(index, d, cx)
                        .id(("deployment", index))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.follow_log(
                                LogSource::Build {
                                    deployment: deployment.clone(),
                                },
                                cx,
                            );
                            this.show_tab(Tab::Logs, cx);
                        }))
                }))
                .into_any_element(),
        };
        card(cx)
            .child(card_title("Deployments", cx))
            .child(muted("Select a deployment to read its build log.", cx))
            .child(body)
    }

    fn logs_tab(&self, item: &Item, cx: &mut Context<Self>) -> AnyElement {
        let build_mode =
            matches!(self.log.source, Some(LogSource::Build { .. }) | None) && !self.log_runtime;
        // Each entry: the log to follow, its title, a status word, and the id ricochet knows it by.
        let mut entries: Vec<(LogSource, String, Option<String>, String)> = Vec::new();
        let list_state = if build_mode {
            match &self.deployments {
                Loadable::Loaded(deployments) => {
                    for deployment in deployments.iter().take(30) {
                        entries.push((
                            LogSource::Build {
                                deployment: deployment.id.clone(),
                            },
                            when(deployment.deployed_at),
                            Some(api::humanize(&deployment.status)),
                            deployment.id.clone(),
                        ));
                    }
                    None
                }
                Loadable::Failed(message) => Some(failed(message, cx)),
                _ => Some(h_flex().child(Spinner::new().small()).into_any_element()),
            }
        } else {
            match &self.runtime {
                Loadable::Loaded(runtime) => {
                    for (index, instance) in runtime.instances.iter().enumerate() {
                        entries.push((
                            LogSource::Log {
                                id: instance.instance_id.clone(),
                            },
                            format!("Instance {}", index + 1),
                            Some(started(&instance.created_at)),
                            instance.instance_id.clone(),
                        ));
                    }
                    for run in &runtime.runs {
                        entries.push((
                            LogSource::Log { id: run.id.clone() },
                            format!("Run {}", short(&run.id)),
                            Some(started(&run.started_at)),
                            run.id.clone(),
                        ));
                    }
                    None
                }
                Loadable::Failed(message) => Some(failed(message, cx)),
                _ => Some(h_flex().child(Spinner::new().small()).into_any_element()),
            }
        };
        if let Some(source @ LogSource::Log { id }) = &self.log.source
            && !build_mode
            && !entries.iter().any(|(known, ..)| known == source)
        {
            entries.insert(
                0,
                (
                    source.clone(),
                    format!("Run {}", short(id)),
                    None,
                    id.clone(),
                ),
            );
        }
        let empty_text = match (build_mode, item.is_task()) {
            (true, _) => "No deployments yet.",
            (false, true) => {
                "No runs are going on. Start one with Run now, and its log shows here."
            }
            (false, false) => {
                "No instances are running. ricochet starts one when someone opens the app."
            }
        };
        let loading = if build_mode {
            matches!(self.deployments, Loadable::Idle | Loadable::Loading)
        } else {
            matches!(self.runtime, Loadable::Idle | Loadable::Loading)
        };
        let chosen = self.log.source.clone();
        let accent = cx.theme().ring;
        let muted_bg = cx.theme().muted;
        let list = v_flex()
            .gap_1()
            .children(list_state)
            .when(entries.is_empty() && !loading, |list| {
                list.child(muted(empty_text, cx))
            })
            .children(entries.iter().cloned().enumerate().map(
                |(index, (source, title, status, id))| {
                    let selected = chosen.as_ref() == Some(&source);
                    v_flex()
                        .id(("log-entry", index))
                        .px_3()
                        .py_2()
                        .gap_1()
                        .cursor_pointer()
                        .border_l_2()
                        .border_color(if selected {
                            accent
                        } else {
                            gpui::transparent_black()
                        })
                        .when(selected, |entry| entry.bg(muted_bg))
                        .hover(move |entry| entry.bg(muted_bg))
                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(title))
                        .children(status.map(|status| {
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(status)
                        }))
                        .child(
                            div()
                                .text_xs()
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_color(cx.theme().muted_foreground)
                                .truncate()
                                .child(id),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.follow_log(source.clone(), cx);
                            cx.notify();
                        }))
                },
            ));
        let toggle = |label: &'static str, active: bool, runtime: bool| {
            div()
                .id(label)
                .px_3()
                .py_1()
                .text_sm()
                .cursor_pointer()
                .when(active, |tab| {
                    tab.bg(cx.theme().background)
                        .font_weight(FontWeight::MEDIUM)
                })
                .when(!active, |tab| tab.text_color(cx.theme().muted_foreground))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.log_runtime = runtime;
                    cx.notify();
                }))
        };
        let selected_title = entries
            .iter()
            .find(|(source, ..)| Some(source) == chosen.as_ref())
            .map(|(_, title, ..)| title.clone())
            .unwrap_or_else(|| "Log".into());
        v_flex()
            .gap_4()
            .child(
                h_flex().child(
                    h_flex()
                        .p_1()
                        .bg(cx.theme().muted)
                        .child(toggle(
                            if item.is_task() { "Runs" } else { "Runtime" },
                            !build_mode,
                            true,
                        ))
                        .child(toggle("Deployment", build_mode, false)),
                ),
            )
            .child(
                h_flex()
                    .gap_4()
                    .items_start()
                    .child(
                        card(cx)
                            .w(px(260.0))
                            .flex_shrink_0()
                            .gap_3()
                            .child(
                                v_flex()
                                    .child(card_title(
                                        if build_mode {
                                            "Deployment logs"
                                        } else if item.is_task() {
                                            "Task runs"
                                        } else {
                                            "Runtime logs"
                                        },
                                        cx,
                                    ))
                                    .child(muted("Newest first", cx)),
                            )
                            .child(list),
                    )
                    .child(
                        card(cx)
                            .flex_1()
                            .min_w_0()
                            .gap_3()
                            .child(card_title(&selected_title, cx))
                            .child(self.log_panel(cx)),
                    ),
            )
            .into_any_element()
    }

    fn runtime_tab(&self, item: &Item, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy.is_some();
        let rows = match &self.runtime {
            Loadable::Idle | Loadable::Loading => {
                h_flex().child(Spinner::new().small()).into_any_element()
            }
            Loadable::Failed(message) => failed(message, cx),
            Loadable::Loaded(runtime) if item.is_task() && runtime.runs.is_empty() => {
                muted("Nothing is running right now.", cx)
            }
            Loadable::Loaded(runtime) if !item.is_task() && runtime.instances.is_empty() => muted(
                "No instances are running. ricochet starts one when someone opens the app.",
                cx,
            ),
            Loadable::Loaded(runtime) => {
                let rows: Vec<(String, String, String)> = if item.is_task() {
                    runtime
                        .runs
                        .iter()
                        .map(|run| {
                            (
                                run.id.clone(),
                                format!("Run {}", short(&run.id)),
                                started(&run.started_at),
                            )
                        })
                        .collect()
                } else {
                    runtime
                        .instances
                        .iter()
                        .enumerate()
                        .map(|(index, instance)| {
                            (
                                instance.instance_id.clone(),
                                format!("Instance {}", index + 1),
                                format!(
                                    "{} · {} connections · {} MiB peak",
                                    started(&instance.created_at),
                                    instance.connections,
                                    instance.peak_memory_bytes / (1024 * 1024)
                                ),
                            )
                        })
                        .collect()
                };
                let task = item.is_task();
                v_flex()
                    .children(
                        rows.into_iter()
                            .enumerate()
                            .map(|(index, (id, title, detail))| {
                                let (log_id, stop_id) = (id.clone(), id);
                                h_flex()
                                    .gap_3()
                                    .py_2()
                                    .items_center()
                                    .when(index > 0, |row| {
                                        row.border_t_1().border_color(cx.theme().border)
                                    })
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(title),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(detail),
                                            ),
                                    )
                                    .child(
                                        Button::new(("runtime-log", index))
                                            .icon(ph("file-text"))
                                            .label("Log")
                                            .ghost()
                                            .small()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.follow_log(
                                                    LogSource::Log { id: log_id.clone() },
                                                    cx,
                                                );
                                                this.show_tab(Tab::Logs, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new(("runtime-stop", index))
                                            .icon(ph("stop"))
                                            .label("Stop")
                                            .outline()
                                            .small()
                                            .disabled(busy)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if task {
                                                    this.stop_run(stop_id.clone(), cx)
                                                } else {
                                                    this.stop_instance(stop_id.clone(), cx)
                                                }
                                            })),
                                    )
                            }),
                    )
                    .into_any_element()
            }
        };
        let id = item.id.clone();
        v_flex()
            .gap_5()
            .when(item.is_task() && item.can_edit(), |pane| {
                pane.child(
                    card(cx).child(card_title("Schedule", cx)).child(field(
                        "Cron schedule",
                        Some(
                            "ricochet runs the task on this schedule, in the server's time zone."
                                .into(),
                        ),
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&self.inputs.schedule)))
                            .child(
                                Button::new("save-schedule")
                                    .label("Save")
                                    .outline()
                                    .small()
                                    .loading(self.busy == Some("Saving schedule"))
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, _, cx| this.save_schedule(cx))),
                            ),
                        cx,
                    )),
                )
            })
            .child(
                card(cx)
                    .child(
                        h_flex()
                            .child(div().flex_1().child(card_title(
                                if item.is_task() {
                                    "Running now"
                                } else {
                                    "Instances"
                                },
                                cx,
                            )))
                            .child(
                                Button::new("refresh-runtime")
                                    .icon(ph("arrow-clockwise"))
                                    .tooltip("Refresh")
                                    .ghost()
                                    .small()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(item) =
                                            this.items().iter().find(|i| i.id == id).cloned()
                                        {
                                            this.load_runtime(&item, cx);
                                        }
                                    })),
                            ),
                    )
                    .child(rows),
            )
            .into_any_element()
    }

    fn settings_tab(&self, item: &Item, cx: &mut Context<Self>) -> AnyElement {
        match &self.settings {
            Loadable::Idle | Loadable::Loading => {
                return h_flex().child(Spinner::new().small()).into_any_element();
            }
            Loadable::Failed(message) => return failed(message, cx),
            Loadable::Loaded(_) => {}
        }
        let busy = self.busy.is_some();
        let access = h_flex()
            .gap_1()
            .children(ACCESS.into_iter().map(|(value, label)| {
                let button = Button::new(value).label(label).small();
                let button = if self.access == value {
                    button.primary()
                } else {
                    button.outline()
                };
                button.on_click(cx.listener(move |this, _, _, cx| {
                    this.access = value.to_owned();
                    cx.notify();
                }))
            }));
        let item_for_delete = item.clone();
        v_flex()
            .gap_5()
            .child(
                card(cx)
                    .child(field("Name", None, Input::new(&self.inputs.name), cx))
                    .child(field("Summary", None, Input::new(&self.inputs.summary), cx))
                    .child(field(
                        "Tags",
                        Some("Separate tags with commas.".into()),
                        Input::new(&self.inputs.tags),
                        cx,
                    ))
                    .child(field(
                        "Access",
                        Some("Private items are open to their collaborators, internal ones to everyone who can sign in, and external ones to anyone with the link.".into()),
                        access,
                        cx,
                    ))
                    .child(
                        h_flex().justify_end().child(
                            Button::new("save-settings")
                                .label("Save")
                                .primary()
                                .small()
                                .loading(self.busy == Some("Saving"))
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| this.save_settings(cx))),
                        ),
                    ),
            )
            .when(item.can_delete(), |pane| {
                pane.child(
                    card(cx)
                        .child(card_title("Delete", cx))
                        .child(muted("Stop the item and delete its deployments, logs and settings.", cx))
                        .child(h_flex().child(
                            danger_button("delete-item", "Delete item", cx).on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.confirm_delete(item_for_delete.clone(), window, cx)
                                },
                            )),
                        )),
                )
            })
            .into_any_element()
    }

    fn env_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy.is_some();
        let names = match &self.env_vars {
            Loadable::Idle | Loadable::Loading => {
                h_flex().child(Spinner::new().small()).into_any_element()
            }
            Loadable::Failed(message) => failed(message, cx),
            Loadable::Loaded(names) if names.is_empty() => {
                muted("No environment variables yet.", cx)
            }
            Loadable::Loaded(names) => v_flex()
                .children(names.iter().enumerate().map(|(index, name)| {
                    let target = name.clone();
                    h_flex()
                        .gap_3()
                        .py_1p5()
                        .items_center()
                        .when(index > 0, |row| {
                            row.border_t_1().border_color(cx.theme().border)
                        })
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .font_family(cx.theme().mono_font_family.clone())
                                .child(name.clone()),
                        )
                        .child(
                            Button::new(("delete-env", index))
                                .icon(ph("trash"))
                                .tooltip("Remove")
                                .ghost()
                                .small()
                                .disabled(busy)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.delete_env_var(target.clone(), cx)
                                })),
                        )
                }))
                .into_any_element(),
        };
        card(cx)
            .child(card_title("Environment variables", cx))
            .child(muted(
                "ricochet stores values encrypted and never shows them again. Add a variable with the same name to change its value.",
                cx,
            ))
            .child(names)
            .child(
                h_flex()
                    .gap_2()
                    .child(div().w(px(200.0)).child(Input::new(&self.inputs.env_name)))
                    .child(div().flex_1().child(Input::new(&self.inputs.env_value).mask_toggle()))
                    .child(
                        Button::new("add-env")
                            .icon(ph("plus"))
                            .label("Add")
                            .outline()
                            .small()
                            .loading(self.busy == Some("Adding"))
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| this.add_env_var(cx))),
                    ),
            )
            .into_any_element()
    }
}

/// A deployment's status, time and version as one row.
pub(super) fn deployment_row(index: usize, d: &api::Deployment, cx: &App) -> Div {
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
        .child(super::status_icon(Some(&d.status), cx))
        .child(div().w(px(110.0)).text_sm().child(api::humanize(&d.status)))
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
}

pub(super) fn muted(text: &str, cx: &App) -> AnyElement {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(prose(text.to_owned()))
        .into_any_element()
}

pub(super) fn failed(message: &str, cx: &App) -> AnyElement {
    div()
        .text_sm()
        .text_color(cx.theme().danger)
        .child(prose(message.to_owned()))
        .into_any_element()
}

/// The last characters of a ULID, which differ even between runs started together.
fn short(id: &str) -> String {
    id.chars()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

/// An RFC 3339 time as "Started 14:05", or the text as given if it does not parse.
fn started(time: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(time)
        .map(|time| {
            format!(
                "Started {}",
                time.with_timezone(&chrono::Local).format("%b %-d at %H:%M")
            )
        })
        .unwrap_or_else(|_| time.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_come_from_ricochet_toml() {
        let text = "[content]\nname = \"Sales\"\naccess_type = \"internal\"\ntags = [\"a\", \"b\"]\n\n[schedule]\ncron = \"0 0 6 * * *\"\n";
        let settings = Settings::parse(text).unwrap();
        assert_eq!(settings.name, "Sales");
        assert_eq!(settings.access, "internal");
        assert_eq!(settings.tags, ["a", "b"]);
        assert_eq!(settings.schedule.as_deref(), Some("0 0 6 * * *"));
        assert_eq!(short("01JABCDEFGHIJK"), "FGHIJK");
    }
}
