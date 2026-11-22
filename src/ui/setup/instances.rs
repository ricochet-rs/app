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

/// The fields of the Instances page while it is open.
#[derive(Default)]
pub(super) struct InstanceInputs {
    /// Each instance's name, by instance id.
    names: HashMap<String, Entity<InputState>>,
    /// Each group's name, by the name saved for it.
    groups: HashMap<String, Entity<InputState>>,
}

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
        let busy = self.server.read(cx).activity.is_some();
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
            .disabled(busy)
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
        self.instance_subscriptions.clear();
        let mut inputs = InstanceInputs::default();
        for (instance, placement) in self.arranged(cx) {
            let id = instance.id();
            let name = text_field(
                placement.name.unwrap_or_default(),
                instance.default_name(),
                window,
                cx,
            );
            let target = id.clone();
            self.instance_subscriptions.push(cx.subscribe_in(
                &name,
                window,
                move |this, input, event: &InputEvent, _, cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }
                    let change = InstanceChange::Name(input.read(cx).value().to_string());
                    let arranged = this.arranged(cx);
                    this.dashboard.update(cx, |dashboard, cx| {
                        dashboard.arrange_instance(&arranged, &target, change, cx)
                    });
                },
            ));
            inputs.names.insert(id, name);
        }
        self.instance_inputs = Some(inputs);
        self.add_group_fields(window, cx);
        cx.notify();
    }

    /// Gives each group without a name field one, which renames the group when you press Enter or leave it.
    fn add_group_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let groups: Vec<String> = self
            .arranged(cx)
            .into_iter()
            .filter_map(|(_, placement)| placement.group)
            .collect();
        for group in groups {
            let Some(inputs) = &self.instance_inputs else {
                return;
            };
            if inputs.groups.contains_key(&group) {
                continue;
            }
            let field = text_field(group.clone(), "Group name".into(), window, cx);
            self.instance_subscriptions.push(cx.subscribe_in(
                &field,
                window,
                |this, field, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                        this.rename_group(field, window, cx);
                    }
                },
            ));
            if let Some(inputs) = &mut self.instance_inputs {
                inputs.groups.insert(group, field);
            }
        }
    }

    /// Renames the group whose name `field` holds to what you typed there, or puts its name back when you cleared it.
    fn rename_group(
        &mut self,
        field: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(inputs) = &mut self.instance_inputs else {
            return;
        };
        let Some(from) = inputs
            .groups
            .iter()
            .find(|(_, input)| *input == field)
            .map(|(group, _)| group.clone())
        else {
            return;
        };
        let typed = field.read(cx).value().to_string();
        let to = typed.trim().to_owned();
        if to.is_empty() || to == from {
            if typed != from {
                field.update(cx, |input, cx| input.set_value(from, window, cx));
            }
            return;
        }
        inputs.groups.remove(&from);
        inputs
            .groups
            .entry(to.clone())
            .or_insert_with(|| field.clone());
        let arranged = self.arranged(cx);
        self.dashboard.update(cx, |dashboard, cx| {
            dashboard.rename_group(&arranged, &from, &to, cx)
        });
        cx.notify();
    }

    /// Puts an instance into `group`, or into no group.
    fn move_to_group(
        &mut self,
        id: &str,
        group: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let change = InstanceChange::Group(group.unwrap_or_default());
        let arranged = self.arranged(cx);
        self.dashboard.update(cx, |dashboard, cx| {
            dashboard.arrange_instance(&arranged, id, change, cx)
        });
        self.add_group_fields(window, cx);
        cx.notify();
    }

    /// Puts an instance into a new group and focuses the group's name so you can type one.
    fn move_to_new_group(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let taken: Vec<String> = self
            .arranged(cx)
            .into_iter()
            .filter_map(|(_, placement)| placement.group)
            .collect();
        let group = std::iter::once("New group".to_owned())
            .chain((2..).map(|n| format!("New group {n}")))
            .find(|name| !taken.contains(name))
            .unwrap_or_default();
        self.move_to_group(id, Some(group.clone()), window, cx);
        let field = self
            .instance_inputs
            .as_ref()
            .and_then(|inputs| inputs.groups.get(&group).cloned());
        if let Some(field) = field {
            field.update(cx, |input, cx| input.focus(window, cx));
        }
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
        let shown = self.shown_instance(cx);
        let sections = Servers::sections(self.arranged(cx));
        let groups: Vec<String> = sections
            .iter()
            .filter_map(|(group, _)| group.clone())
            .collect();
        let view = cx.entity();
        let mut index: usize = 0;
        let sections = sections.into_iter().map(|(group, entries)| {
            let header = match &group {
                Some(group) => Some(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            ph("folder")
                                .size_4()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(match inputs.groups.get(group) {
                            Some(field) => div().w(px(240.0)).child(Input::new(field).small()),
                            None => div().text_sm().child(group.clone()),
                        }),
                ),
                None if !groups.is_empty() => Some(
                    h_flex().h_8().items_center().child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Not in a group"),
                    ),
                ),
                None => None,
            };
            let count = entries.len();
            let rows = entries
                .into_iter()
                .enumerate()
                .map(|(position, (instance, placement))| {
                    let row = index;
                    index += 1;
                    let id = instance.id();
                    let removable = !matches!(
                        instance,
                        Instance::Machine(crate::connections::Connection::Local)
                    ) && instance != shown;
                    let shift = |offset: isize| {
                        let id = id.clone();
                        cx.listener(move |this: &mut Self, _: &gpui::ClickEvent, _, cx| {
                            let arranged = this.arranged(cx);
                            this.dashboard.update(cx, |dashboard, cx| {
                                dashboard.arrange_instance(
                                    &arranged,
                                    &id,
                                    InstanceChange::Move(offset),
                                    cx,
                                )
                            });
                            cx.notify();
                        })
                    };
                    let group_menu = {
                        let (view, id, groups, current) = (
                            view.clone(),
                            id.clone(),
                            groups.clone(),
                            placement.group.clone(),
                        );
                        Button::new(("instance-group", row))
                            .label(current.clone().unwrap_or_else(|| "No group".into()))
                            .tooltip("Choose the heading it shows under in the server menu")
                            .outline()
                            .small()
                            .dropdown_caret(true)
                            .w_full()
                            .dropdown_menu(move |menu, _, _| {
                                let choice = |label: String, group: Option<String>| {
                                    let (view, id) = (view.clone(), id.clone());
                                    PopupMenuItem::new(label)
                                        .checked(current == group)
                                        .on_click(move |_, window, cx| {
                                            let group = group.clone();
                                            view.update(cx, |this, cx| {
                                                this.move_to_group(&id, group, window, cx)
                                            });
                                        })
                                };
                                let mut menu = menu.item(choice("No group".into(), None));
                                for group in &groups {
                                    menu = menu.item(choice(group.clone(), Some(group.clone())));
                                }
                                let (view, id) = (view.clone(), id.clone());
                                menu.separator().item(
                                    PopupMenuItem::new("New group").icon(ph("plus")).on_click(
                                        move |_, window, cx| {
                                            view.update(cx, |this, cx| {
                                                this.move_to_new_group(&id, window, cx)
                                            });
                                        },
                                    ),
                                )
                            })
                    };
                    h_flex()
                        .gap_3()
                        .py_2()
                        .items_center()
                        .when(position > 0, |row| {
                            row.border_t_1().border_color(cx.theme().border)
                        })
                        .child(
                            ph(instance.icon())
                                .size_4()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .children(
                            inputs
                                .names
                                .get(&id)
                                .map(|name| div().flex_1().min_w_0().child(Input::new(name))),
                        )
                        .child(div().w(px(180.0)).child(group_menu))
                        .child(
                            Button::new(("instance-up", row))
                                .icon(ph("arrow-up"))
                                .tooltip("Move up")
                                .ghost()
                                .small()
                                .disabled(position == 0)
                                .on_click(shift(-1)),
                        )
                        .child(
                            Button::new(("instance-down", row))
                                .icon(ph("arrow-down"))
                                .tooltip("Move down")
                                .ghost()
                                .small()
                                .disabled(position + 1 == count)
                                .on_click(shift(1)),
                        )
                        .child(
                            Button::new(("instance-remove", row))
                                .icon(ph("trash"))
                                .tooltip("Remove from the list")
                                .ghost()
                                .small()
                                .disabled(!removable)
                                .when(!removable, |button| button.invisible())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.remove_instance(instance.clone(), cx)
                                })),
                        )
                })
                .collect::<Vec<_>>();
            v_flex()
                .gap_2()
                .children(header)
                .child(card(cx).py_2().gap_0().children(rows))
        });
        let sections = sections.collect::<Vec<_>>();
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
                        Some("Name, order and group the ricochet instances in the server menu. The menu lists them the way this page does, under their group names."),
                        cx,
                    ))
                    .children(sections)
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(prose(
                                "You can remove any instance except this computer and the one that's showing. Removing a machine only takes it off the list, and ricochet keeps running there.",
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

/// A one-line text field holding `value`, which shows `placeholder` while empty.
fn text_field(
    value: String,
    placeholder: String,
    window: &mut Window,
    cx: &mut Context<SetupView>,
) -> Entity<InputState> {
    cx.new(|cx| {
        let mut input = InputState::new(window, cx).placeholder(placeholder);
        input.set_value(value, window, cx);
        input
    })
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
