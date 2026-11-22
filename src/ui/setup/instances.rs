//! The server menu, and the Instances page that names, groups and orders what it lists.

use super::{SetupView, Step};
use crate::servers::{Instance, Servers};
use crate::ui::components::{card, heading, prose};
use crate::ui::dashboard::InstanceChange;
use crate::ui::icons::ph;
use gpui::prelude::*;
use gpui::{AnyElement, Context, DismissEvent, Entity, Window, div, px};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::{ActiveTheme, Disableable, Sizable, h_flex, v_flex};
use std::collections::HashMap;

/// The name and group fields of each instance while the Instances page is open, by instance id.
pub(super) type InstanceInputs = HashMap<String, [Entity<InputState>; 2]>;

impl SetupView {
    fn arranged(&self, cx: &gpui::App) -> Vec<(Instance, crate::servers::Placement)> {
        let machines = self.server.read(cx).connected();
        self.dashboard.read(cx).instances(machines)
    }

    /// The instance whose content and settings show.
    fn shown_instance(&self, cx: &gpui::App) -> Instance {
        match self.dashboard.read(cx).added_server() {
            Some(server) => Instance::Added(server),
            None => Instance::Machine(self.server.read(cx).connection.clone()),
        }
    }

    /// Shows the settings of a machine this app manages, connecting to it first when it is not the current one.
    fn open_machine_settings(
        &mut self,
        connection: crate::connections::Connection,
        cx: &mut Context<Self>,
    ) {
        let current = self.server.read(cx).connection == connection;
        self.show_instance(Instance::Machine(connection), cx);
        if current {
            self.select(self.section, cx);
        } else {
            // The pane loads what it needs once the machine has connected.
            self.showing_content = false;
            cx.notify();
        }
    }

    fn show_instance(&mut self, instance: Instance, cx: &mut Context<Self>) {
        self.instance_inputs = None;
        match instance {
            Instance::Machine(connection) => self.show_machine(connection, cx),
            Instance::Added(server) => self.show_added(server, cx),
        }
    }

    /// The server button, which drops a menu of your instances in their groups and ways to add or arrange them.
    pub(super) fn machine_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // The controller runs one operation on one machine, so only switching machines waits for it.
        let busy = self.server.read(cx).activity.is_some();
        let managing = Instance::Machine(self.server.read(cx).connection.clone());
        let shown = self.shown_instance(cx);
        let arranged = self.arranged(cx);
        let name = arranged
            .iter()
            .find(|(instance, _)| *instance == shown)
            .and_then(|(_, placement)| placement.name.clone())
            .unwrap_or_else(|| shown.default_name());
        let sections = Servers::sections(arranged);
        let dashboard = self.dashboard.read(cx);
        let showing = dashboard.added_server();
        let in_browser = self.browser_url(cx);
        let dashboard = self.dashboard.clone();
        let cover = self.dashboard.clone();
        let view = cx.entity();
        let icon = shown.icon();
        Button::new("connection")
            .icon(ph(icon))
            .label(name)
            .ghost()
            .xsmall()
            .dropdown_caret(true)
            .dropdown_menu(move |menu, _, cx| {
                let mut menu = menu;
                // The system draws the web UI above the window, so it hides while the menu is open.
                cover.update(cx, |dashboard, cx| dashboard.set_covered(true, cx));
                let uncover = cover.clone();
                cx.subscribe_self(move |_, _: &DismissEvent, cx| {
                    uncover.update(cx, |dashboard, cx| dashboard.set_covered(false, cx));
                })
                .detach();
                let uncover = cover.clone();
                cx.on_release(move |_, cx| {
                    uncover.update(cx, |dashboard, cx| dashboard.set_covered(false, cx));
                })
                .detach();
                let mut opened: usize = 0;
                for (index, (group, entries)) in sections.iter().enumerate() {
                    if index > 0 {
                        menu = menu.separator();
                    }
                    if let Some(group) = group {
                        menu = menu.label(group.clone());
                    }
                    for (instance, placement) in entries {
                        let target = instance.clone();
                        let view = view.clone();
                        let label = placement
                            .name
                            .clone()
                            .unwrap_or_else(|| instance.default_name());
                        // An added server opens at its address; a machine only while it is the one shown and reachable.
                        let url = match instance {
                            Instance::Added(server) => Some(server.url.clone()),
                            Instance::Machine(_) if *instance == shown => in_browser.clone(),
                            Instance::Machine(_) => None,
                        };
                        let has_settings = matches!(instance, Instance::Machine(_));
                        let row = opened;
                        opened += 1;
                        // The Settings button lets its click reach the row, which closes the menu and opens the settings.
                        let wants_settings = std::rc::Rc::new(std::cell::Cell::new(false));
                        let pressed = wants_settings.clone();
                        menu = menu.item(
                            PopupMenuItem::element(move |_, cx| {
                                h_flex()
                                    .w_full()
                                    .gap_3()
                                    .items_center()
                                    .child(div().flex_1().truncate().child(label.clone()))
                                    .children(url.clone().map(|url| {
                                        Button::new(("open-instance", row))
                                            .icon(ph("arrow-square-out"))
                                            .tooltip("Open in browser")
                                            .ghost()
                                            .xsmall()
                                            .text_color(cx.theme().muted_foreground)
                                            .on_click(move |_, _, cx| {
                                                cx.stop_propagation();
                                                let _ = open::that(&url);
                                            })
                                    }))
                                    .children(has_settings.then(|| {
                                        let pressed = pressed.clone();
                                        Button::new(("instance-settings", row))
                                            .icon(ph("gear-six"))
                                            .label("Settings")
                                            .outline()
                                            .xsmall()
                                            .on_click(move |_, _, _| pressed.set(true))
                                    }))
                            })
                            .icon(ph(instance.icon()))
                            .checked(*instance == shown)
                            .disabled(
                                busy && matches!(instance, Instance::Machine(_))
                                    && *instance != managing,
                            )
                            .on_click(move |_, _, cx| {
                                let target = target.clone();
                                let settings = wants_settings.take();
                                view.update(cx, |this, cx| match (&target, settings) {
                                    (Instance::Machine(connection), true) => {
                                        this.open_machine_settings(connection.clone(), cx)
                                    }
                                    _ => this.show_instance(target, cx),
                                });
                            }),
                        );
                    }
                }
                let (connect, install, arrange, app) =
                    (view.clone(), view.clone(), view.clone(), view.clone());
                menu = menu
                    .separator()
                    .label("Connect")
                    .item(
                        described_item(
                            "Connect to a server…",
                            "Open a ricochet that runs somewhere else, by its address.",
                        )
                        .icon(ph("link"))
                        .on_click(move |_, _, cx| {
                            connect.update(cx, |this, cx| this.connect_another(cx))
                        }),
                    )
                    .label("Manage")
                    .item(
                        described_item(
                            "Install ricochet…",
                            "Set it up on this computer or on a Linux server over SSH.",
                        )
                        .icon(ph("download-simple"))
                        .on_click(move |_, _, cx| {
                            install.update(cx, |this, cx| this.install_instead(cx))
                        }),
                    )
                    .item(
                        PopupMenuItem::new("Arrange instances…")
                            .icon(ph("list-dashes"))
                            .on_click(move |_, window, cx| {
                                arrange.update(cx, |this, cx| this.open_instances(window, cx))
                            }),
                    );
                if let Some(server) = showing.clone() {
                    let dashboard = dashboard.clone();
                    menu = menu.item(
                        PopupMenuItem::new(format!("Remove {}", server.name))
                            .icon(ph("trash"))
                            .on_click(move |_, _, cx| {
                                dashboard.update(cx, |d, cx| d.remove_server(&server, cx))
                            }),
                    );
                }
                menu.separator().item(
                    PopupMenuItem::new("App settings…")
                        .icon(ph("gear-six"))
                        .on_click(move |_, window, cx| {
                            app.update(cx, |this, cx| this.open_app_settings(window, cx))
                        }),
                )
            })
    }

    /// Asks for the address of a running ricochet, which offers installing one instead.
    fn connect_another(&mut self, cx: &mut Context<Self>) {
        self.instance_inputs = None;
        self.add_server(cx);
    }

    /// Shows where ricochet can be installed: this machine or an SSH host.
    pub(super) fn install_instead(&mut self, cx: &mut Context<Self>) {
        self.instance_inputs = None;
        self.dashboard
            .update(cx, |dashboard, cx| dashboard.show_managed(cx));
        self.onboarding = Some(Step::Target);
        cx.notify();
    }

    pub(super) fn open_instances(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut inputs = InstanceInputs::new();
        self.instance_subscriptions.clear();
        for (instance, placement) in self.arranged(cx) {
            let id = instance.id();
            let field = |value: Option<String>,
                         placeholder: String,
                         window: &mut Window,
                         cx: &mut Context<Self>| {
                cx.new(|cx| {
                    let mut input = InputState::new(window, cx).placeholder(placeholder);
                    input.set_value(value.unwrap_or_default(), window, cx);
                    input
                })
            };
            let name = field(placement.name, instance.default_name(), window, cx);
            let group = field(placement.group, "No group".into(), window, cx);
            for (input, is_name) in [(name.clone(), true), (group.clone(), false)] {
                let id = id.clone();
                self.instance_subscriptions.push(cx.subscribe_in(
                    &input,
                    window,
                    move |this, input, event: &InputEvent, _, cx| {
                        if !matches!(event, InputEvent::Change) {
                            return;
                        }
                        let text = input.read(cx).value().to_string();
                        let change = if is_name {
                            InstanceChange::Name(text)
                        } else {
                            InstanceChange::Group(text)
                        };
                        let arranged = this.arranged(cx);
                        this.dashboard.update(cx, |dashboard, cx| {
                            dashboard.arrange_instance(&arranged, &id, change, cx)
                        });
                    },
                ));
            }
            inputs.insert(id, [name, group]);
        }
        self.instance_inputs = Some(inputs);
        cx.notify();
    }

    fn remove_instance(&mut self, instance: Instance, cx: &mut Context<Self>) {
        let id = instance.id();
        match instance {
            Instance::Machine(crate::connections::Connection::Ssh { host }) => {
                self.server
                    .update(cx, |server, cx| server.forget(&host, cx));
                self.dashboard
                    .update(cx, |dashboard, cx| dashboard.forget_placement(&id, cx));
            }
            Instance::Machine(_) => {}
            Instance::Added(server) => self
                .dashboard
                .update(cx, |dashboard, cx| dashboard.remove_server(&server, cx)),
        }
        cx.notify();
    }

    pub(super) fn instances_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(inputs) = &self.instance_inputs else {
            return div().into_any_element();
        };
        let arranged = self.arranged(cx);
        let shown = self.shown_instance(cx);
        let count = arranged.len();
        let rows = arranged.iter().enumerate().map(|(index, (instance, _))| {
            let id = instance.id();
            let fields = inputs.get(&id).cloned();
            let removable = !matches!(
                instance,
                Instance::Machine(crate::connections::Connection::Local)
            ) && *instance != shown;
            let shift = |offset: isize| {
                let id = id.clone();
                cx.listener(move |this: &mut Self, _: &gpui::ClickEvent, _, cx| {
                    let arranged = this.arranged(cx);
                    this.dashboard.update(cx, |dashboard, cx| {
                        dashboard.arrange_instance(&arranged, &id, InstanceChange::Move(offset), cx)
                    });
                    cx.notify();
                })
            };
            let target = instance.clone();
            h_flex()
                .gap_3()
                .py_2()
                .items_center()
                .when(index > 0, |row| {
                    row.border_t_1().border_color(cx.theme().border)
                })
                .child(
                    ph(instance.icon())
                        .size_4()
                        .text_color(cx.theme().muted_foreground),
                )
                .children(fields.map(|[name, group]| {
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_2()
                        .child(div().flex_1().min_w_0().child(Input::new(&name)))
                        .child(div().w(px(180.0)).child(Input::new(&group)))
                }))
                .child(
                    Button::new(("instance-up", index))
                        .icon(ph("arrow-up"))
                        .tooltip("Move up")
                        .ghost()
                        .small()
                        .disabled(index == 0)
                        .on_click(shift(-1)),
                )
                .child(
                    Button::new(("instance-down", index))
                        .icon(ph("arrow-down"))
                        .tooltip("Move down")
                        .ghost()
                        .small()
                        .disabled(index + 1 == count)
                        .on_click(shift(1)),
                )
                .child(
                    Button::new(("instance-remove", index))
                        .icon(ph("trash"))
                        .tooltip(if removable {
                            "Remove from the list"
                        } else {
                            "You can't remove the instance that shows, or this machine"
                        })
                        .ghost()
                        .small()
                        .disabled(!removable)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_instance(target.clone(), cx)
                        })),
                )
        });
        div()
            .id("instances-page")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .gap_5()
                    .p_6()
                    .child(heading(
                        "Instances",
                        Some("Name, group and order the ricochet instances in the server menu. Instances with the same group show together."),
                        cx,
                    ))
                    .child(card(cx).child(v_flex().children(rows)))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(prose(
                                "Removing a machine only takes it off the list. ricochet keeps running there.",
                            )),
                    )
                    .child(
                        h_flex().justify_end().child(
                            Button::new("instances-done")
                                .label("Done")
                                .primary()
                                .small()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.instance_inputs = None;
                                    this.instance_subscriptions.clear();
                                    cx.notify();
                                })),
                        ),
                    ),
            )
            .into_any_element()
    }
}

/// A menu item with a line below its title that says what it does.
fn described_item(title: &'static str, detail: &'static str) -> PopupMenuItem {
    PopupMenuItem::element(move |_, cx| {
        v_flex().gap_0p5().child(title).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(detail),
        )
    })
}
