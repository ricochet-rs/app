//! The server menu, and the All instances page where you open, name, group and drag what it lists.

use super::{SetupView, Step};
use crate::connections::Connection;
use crate::servers::{Instance, Placement, Servers};
use crate::ui::components::heading;
use crate::ui::dashboard::InstanceChange;
use crate::ui::icons::ph;
use crate::ui::theme::Palette;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, DismissEvent, DragMoveEvent, Entity, FontWeight, MouseButton, Window, div,
    px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::{ActiveTheme, Sizable, h_flex, v_flex};

const ROW_HEIGHT: f32 = 60.0;

/// What the All instances page holds while it is open.
#[derive(Default)]
pub(super) struct InstanceInputs {
    /// The name being edited, and the field holding what you typed.
    editing: Option<(Editing, Entity<InputState>)>,
    /// The instance being dragged, whose row fades while it moves.
    dragging: Option<String>,
    /// Where the dragged instance lands when you let go.
    landing: Option<Landing>,
}

/// A name being edited on the All instances page.
#[derive(Clone, PartialEq)]
enum Editing {
    Instance(String),
    Group(String),
}

/// A group, or no group, and the instance to go before, or `None` for the group's end.
#[derive(Clone, PartialEq)]
struct Landing {
    group: Option<String>,
    before: Option<String>,
}

/// An instance being dragged on the All instances page, drawn under the pointer.
#[derive(Clone)]
struct DraggedInstance {
    id: String,
    name: String,
    icon: &'static str,
}

impl Render for DraggedInstance {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w(px(420.0))
            .h(px(ROW_HEIGHT))
            .gap_3()
            .pl_2()
            .pr_3()
            .items_center()
            .bg(Palette::card(cx))
            .border_1()
            .border_color(cx.theme().ring)
            .shadow_lg()
            .child(
                ph("dots-six-vertical")
                    .size_4()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(icon_box(self.icon, cx))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .child(self.name.clone()),
            )
    }
}

impl SetupView {
    fn arranged(&self, cx: &gpui::App) -> Vec<(Instance, Placement)> {
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
    fn open_machine_settings(&mut self, connection: Connection, cx: &mut Context<Self>) {
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
                    .item(
                        PopupMenuItem::new("Connect to a server")
                            .icon(ph("link"))
                            .on_click(move |_, _, cx| {
                                connect.update(cx, |this, cx| this.connect_another(cx))
                            }),
                    )
                    .item(
                        PopupMenuItem::new("Install ricochet")
                            .icon(ph("download-simple"))
                            .on_click(move |_, _, cx| {
                                install.update(cx, |this, cx| this.install_instead(cx))
                            }),
                    )
                    .item(
                        PopupMenuItem::new("All instances")
                            .icon(ph("squares-four"))
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
                    PopupMenuItem::new("App settings")
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

    pub(super) fn open_instances(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.instance_subscriptions.clear();
        self.instance_inputs = Some(InstanceInputs::default());
        cx.notify();
    }

    /// Turns a name on the page into a field holding `value`, which saves when you press Enter or click away.
    fn start_editing(
        &mut self,
        editing: Editing,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let field = cx.new(|cx| {
            let mut input = InputState::new(window, cx);
            input.set_value(value, window, cx);
            input
        });
        self.instance_subscriptions.clear();
        self.instance_subscriptions.push(cx.subscribe_in(
            &field,
            window,
            |this, field, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    this.finish_editing(field, cx);
                }
            },
        ));
        field.update(cx, |input, cx| input.focus(window, cx));
        if let Some(page) = &mut self.instance_inputs {
            page.editing = Some((editing, field));
        }
        cx.notify();
    }

    /// Saves what you typed in `field`, where a blank instance name goes back to its own and a blank group name is ignored.
    fn finish_editing(&mut self, field: &Entity<InputState>, cx: &mut Context<Self>) {
        let Some((editing, _)) = self
            .instance_inputs
            .as_mut()
            .and_then(|page| page.editing.take_if(|(_, open)| open == field))
        else {
            return;
        };
        let typed = field.read(cx).value().trim().to_owned();
        let arranged = self.arranged(cx);
        self.dashboard.update(cx, |dashboard, cx| match editing {
            Editing::Instance(id) => {
                dashboard.arrange_instance(&arranged, &id, InstanceChange::Name(typed), cx)
            }
            Editing::Group(from) if !typed.is_empty() && typed != from => {
                dashboard.rename_group(&arranged, &from, &typed, cx)
            }
            Editing::Group(_) => {}
        });
        cx.notify();
    }

    /// Adds an empty group and opens its name so you can type one.
    fn add_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let arranged = self.arranged(cx);
        let name = self
            .dashboard
            .update(cx, |dashboard, cx| dashboard.add_group(&arranged, cx));
        self.start_editing(Editing::Group(name.clone()), name, window, cx);
    }

    /// Remembers where the dragged instance would land, redrawing only when that changes.
    fn aim(&mut self, landing: Landing, cx: &mut Context<Self>) {
        if let Some(page) = &mut self.instance_inputs
            && page.landing.as_ref() != Some(&landing)
        {
            page.landing = Some(landing);
            cx.notify();
        }
    }

    /// Moves the dragged instance to where it was aimed.
    fn drop_instance(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(landing) = self.instance_inputs.as_mut().and_then(|page| {
            page.dragging = None;
            page.landing.take()
        }) else {
            return;
        };
        let arranged = self.arranged(cx);
        let change = InstanceChange::Move {
            group: landing.group,
            before: landing.before,
        };
        self.dashboard.update(cx, |dashboard, cx| {
            dashboard.arrange_instance(&arranged, id, change, cx)
        });
        cx.notify();
    }

    fn remove_instance(&mut self, instance: Instance, cx: &mut Context<Self>) {
        let id = instance.id();
        match instance {
            Instance::Machine(Connection::Ssh { host }) => {
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
        let Some(page) = &self.instance_inputs else {
            return div().into_any_element();
        };
        let arranged = self.arranged(cx);
        let empty_groups = self.dashboard.read(cx).empty_groups(&arranged);
        let mut sections = Servers::sections(arranged);
        sections.extend(
            empty_groups
                .into_iter()
                .map(|group| (Some(group), Vec::new())),
        );
        // Once groups exist, "Not in a group" stays as a place to drag instances back to.
        let grouped = sections.iter().any(|(group, _)| group.is_some());
        if grouped && !sections.iter().any(|(group, _)| group.is_none()) {
            sections.insert(0, (None, Vec::new()));
        }
        let landing = page.landing.clone().filter(|_| cx.has_active_drag());
        let mut row_index: usize = 0;
        let sections = sections
            .into_iter()
            .map(|(group, entries)| {
                let header = (grouped || group.is_some())
                    .then(|| self.group_header(group.as_deref(), entries.len(), cx));
                let body = if entries.is_empty() {
                    self.drop_zone(group.clone(), landing.as_ref(), cx)
                } else {
                    let ids: Vec<String> = entries.iter().map(|(i, _)| i.id()).collect();
                    let rows = entries
                        .into_iter()
                        .enumerate()
                        .map(|(position, (instance, placement))| {
                            let next = ids.get(position + 1).cloned();
                            let row = row_index;
                            row_index += 1;
                            self.instance_row(
                                row,
                                instance,
                                placement,
                                &group,
                                next,
                                position == 0,
                                landing.as_ref(),
                                cx,
                            )
                        })
                        .collect::<Vec<_>>();
                    v_flex()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(Palette::card(cx))
                        .children(rows)
                        .into_any_element()
                };
                v_flex().gap_2().children(header).child(body)
            })
            .collect::<Vec<_>>();
        div()
            .id("instances-page")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .gap_6()
                    .px_12()
                    .py_8()
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            .child(div().flex_1().child(heading(
                                "All instances",
                                Some("Drag an instance to reorder it or move it to another group."),
                                cx,
                            )))
                            .child(
                                Button::new("new-group")
                                    .icon(ph("plus"))
                                    .label("New group")
                                    .outline()
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.add_group(window, cx)
                                    })),
                            ),
                    )
                    .children(sections),
            )
            .into_any_element()
    }

    /// A group's name, which you click to rename, its count, and a button that deletes it while it is empty.
    fn group_header(
        &self,
        group: Option<&str>,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let editing = self.instance_inputs.as_ref().and_then(|page| {
            page.editing.as_ref().and_then(|(editing, field)| {
                (Some(editing) == group.map(|g| Editing::Group(g.to_owned())).as_ref())
                    .then(|| field.clone())
            })
        });
        let muted = cx.theme().muted_foreground;
        let title: AnyElement = match (group, editing) {
            (_, Some(field)) => div()
                .w(px(240.0))
                .child(Input::new(&field).small())
                .into_any_element(),
            (Some(group), None) => {
                let name = group.to_owned();
                div()
                    .id(gpui::SharedString::from(format!("group-{group}")))
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .hover(move |text| text.text_color(muted))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.start_editing(Editing::Group(name.clone()), name.clone(), window, cx)
                    }))
                    .child(group.to_owned())
                    .into_any_element()
            }
            (None, None) => div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(muted)
                .child("Not in a group")
                .into_any_element(),
        };
        h_flex()
            .h_7()
            .gap_2()
            .items_center()
            .child(title)
            .child(div().text_sm().text_color(muted).child(count.to_string()))
            .children(group.filter(|_| count == 0).map(|group| {
                let name = group.to_owned();
                h_flex().flex_1().justify_end().child(
                    Button::new(gpui::SharedString::from(format!("delete-group-{group}")))
                        .icon(ph("trash"))
                        .tooltip("Delete group")
                        .ghost()
                        .xsmall()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.dashboard
                                .update(cx, |dashboard, cx| dashboard.remove_group(&name, cx));
                            cx.notify();
                        })),
                )
            }))
            .into_any_element()
    }

    /// The outline an empty group shows, which a dragged instance drops into.
    fn drop_zone(
        &self,
        group: Option<String>,
        landing: Option<&Landing>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let here = Landing {
            group: group.clone(),
            before: None,
        };
        let aimed = landing == Some(&here);
        h_flex()
            .id(gpui::SharedString::from(format!(
                "drop-{}",
                group.as_deref().unwrap_or("")
            )))
            .h(px(56.0))
            .justify_center()
            .items_center()
            .border_1()
            .border_dashed()
            .border_color(if aimed {
                cx.theme().ring
            } else {
                cx.theme().input
            })
            .when(aimed, |zone| zone.bg(cx.theme().ring.opacity(0.08)))
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child("Drag an instance here")
            .on_drag_move::<DraggedInstance>(cx.listener(
                move |this, event: &DragMoveEvent<DraggedInstance>, _, cx| {
                    if event.bounds.contains(&event.event.position) {
                        this.aim(here.clone(), cx);
                    }
                },
            ))
            .on_drop(cx.listener(|this, dragged: &DraggedInstance, _, cx| {
                this.drop_instance(&dragged.id, cx)
            }))
            .into_any_element()
    }

    /// One instance: a handle to drag it by, its name and kind, and buttons to open it in the browser, open its settings, rename it or remove it.
    #[allow(clippy::too_many_arguments)]
    fn instance_row(
        &self,
        row: usize,
        instance: Instance,
        placement: Placement,
        group: &Option<String>,
        next: Option<String>,
        first: bool,
        landing: Option<&Landing>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page = self.instance_inputs.as_ref();
        let id = instance.id();
        let name = placement
            .name
            .clone()
            .unwrap_or_else(|| instance.default_name());
        let kind = match &instance {
            Instance::Machine(Connection::Local) => "On this computer",
            Instance::Machine(Connection::Ssh { .. }) => "Linux over SSH",
            Instance::Added(_) => "Added by address",
        };
        let detail = match &placement.name {
            Some(_) => format!("{} · {kind}", instance.default_name()),
            None => kind.to_owned(),
        };
        let editing = page.and_then(|page| {
            page.editing.as_ref().and_then(|(editing, field)| {
                (*editing == Editing::Instance(id.clone())).then(|| field.clone())
            })
        });
        let faded = cx.has_active_drag() && page.and_then(|p| p.dragging.as_deref()) == Some(&id);
        let mark_above =
            landing.is_some_and(|l| l.group == *group && l.before.as_deref() == Some(&id));
        let mark_below =
            next.is_none() && landing.is_some_and(|l| l.group == *group && l.before.is_none());
        let shown = self.shown_instance(cx);
        let url = match &instance {
            Instance::Added(server) => Some(server.url.clone()),
            Instance::Machine(_) if instance == shown => self.browser_url(cx),
            Instance::Machine(_) => None,
        };
        let removable =
            !matches!(instance, Instance::Machine(Connection::Local)) && instance != shown;
        let muted = cx.theme().muted_foreground;
        let ring = cx.theme().ring;
        let hover = cx.theme().muted;
        let marker = |row: gpui::Stateful<gpui::Div>, top: bool| {
            row.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .h(px(2.0))
                    .bg(ring)
                    .map(|line| if top { line.top_0() } else { line.bottom_0() }),
            )
        };
        let dragged = DraggedInstance {
            id: id.clone(),
            name: name.clone(),
            icon: instance.icon(),
        };
        let view = cx.entity();
        let aim_group = group.clone();
        let aim_id = id.clone();
        let title: AnyElement = match editing {
            Some(field) => div()
                .w(px(260.0))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(Input::new(&field).small())
                .into_any_element(),
            None => div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .truncate()
                .child(name.clone())
                .into_any_element(),
        };
        let actions = {
            let more = {
                let (view, id, name, instance) =
                    (view.clone(), id.clone(), name.clone(), instance.clone());
                Button::new(("instance-more", row))
                    .icon(ph("dots-three"))
                    .tooltip("More")
                    .ghost()
                    .small()
                    .text_color(muted)
                    .dropdown_menu(move |menu, _, _| {
                        let (rename_view, id, name) = (view.clone(), id.clone(), name.clone());
                        let menu = menu.item(PopupMenuItem::new("Rename").on_click(
                            move |_, window, cx| {
                                let (id, name) = (id.clone(), name.clone());
                                rename_view.update(cx, |this, cx| {
                                    this.start_editing(Editing::Instance(id), name, window, cx)
                                });
                            },
                        ));
                        if !removable {
                            return menu;
                        }
                        let (view, instance) = (view.clone(), instance.clone());
                        menu.item(PopupMenuItem::new("Remove").on_click(move |_, _, cx| {
                            let instance = instance.clone();
                            view.update(cx, |this, cx| this.remove_instance(instance, cx));
                        }))
                    })
            };
            h_flex()
                .gap_0p5()
                .flex_shrink_0()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .children(url.map(|url| {
                    Button::new(("instance-open", row))
                        .icon(ph("arrow-square-out"))
                        .tooltip("Open in browser")
                        .ghost()
                        .small()
                        .text_color(muted)
                        .on_click(move |_, _, _| {
                            let _ = open::that(&url);
                        })
                }))
                .children(match &instance {
                    Instance::Machine(connection) => {
                        let connection = connection.clone();
                        Some(
                            Button::new(("instance-settings", row))
                                .icon(ph("gear-six"))
                                .tooltip("Settings")
                                .ghost()
                                .small()
                                .text_color(muted)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_machine_settings(connection.clone(), cx)
                                })),
                        )
                    }
                    Instance::Added(_) => None,
                })
                .child(more)
        };
        let row_element = h_flex()
            .id(("instance-row", row))
            .relative()
            .h(px(ROW_HEIGHT))
            .gap_3()
            .pl_2()
            .pr_3()
            .items_center()
            .when(!first, |row| {
                row.border_t_1().border_color(cx.theme().border)
            })
            .when(faded, |row| row.opacity(0.35))
            .cursor_pointer()
            .hover(move |row| row.bg(hover))
            .on_click(cx.listener({
                let instance = instance.clone();
                move |this, _, _, cx| this.show_instance(instance.clone(), cx)
            }))
            .on_drag(dragged, move |dragged, _, _, cx| {
                view.update(cx, |this, cx| {
                    if let Some(page) = &mut this.instance_inputs {
                        page.dragging = Some(dragged.id.clone());
                        page.landing = None;
                    }
                    cx.notify();
                });
                cx.new(|_| dragged.clone())
            })
            .on_drag_move::<DraggedInstance>(cx.listener(
                move |this, event: &DragMoveEvent<DraggedInstance>, _, cx| {
                    if !event.bounds.contains(&event.event.position) {
                        return;
                    }
                    let before = if event.event.position.y < event.bounds.center().y {
                        Some(aim_id.clone())
                    } else {
                        next.clone()
                    };
                    this.aim(
                        Landing {
                            group: aim_group.clone(),
                            before,
                        },
                        cx,
                    );
                },
            ))
            .on_drop(cx.listener(|this, dragged: &DraggedInstance, _, cx| {
                this.drop_instance(&dragged.id, cx)
            }))
            .child(ph("dots-six-vertical").size_4().text_color(muted))
            .child(icon_box(instance.icon(), cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(title)
                    .child(div().text_xs().text_color(muted).truncate().child(detail)),
            )
            .child(actions);
        let row_element = if mark_above {
            marker(row_element, true)
        } else if mark_below {
            marker(row_element, false)
        } else {
            row_element
        };
        row_element.into_any_element()
    }
}

/// An instance's icon on a muted square.
fn icon_box(icon: &'static str, cx: &gpui::App) -> gpui::Div {
    div()
        .size(px(32.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(cx.theme().muted)
        .child(ph(icon).size_4())
}
