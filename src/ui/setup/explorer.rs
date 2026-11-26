//! ricochet's home folder in the Danger zone: a file browser, and deleting data while ricochet stays installed.

use super::SetupView;
use crate::backend::{Data, DirEntry};
use crate::ui::components::{danger_button, prose};
use crate::ui::icons::ph;
use gpui::prelude::*;
use gpui::{Context, Div, FontWeight, Hsla, Window, div, px};
use gpui_component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_component::dialog::DialogButtonProps;
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::spinner::Spinner;
use gpui_component::{
    ActiveTheme, Disableable as _, Icon, IconName, Sizable, WindowExt as _, h_flex, v_flex,
};

const SIZE_WIDTH: f32 = 72.0;
const MODIFIED_WIDTH: f32 = 160.0;

pub(super) enum Listing {
    Loading,
    Loaded(Vec<DirEntry>),
    Failed(String),
}

/// The folder being browsed, as names under ricochet's home, and what it holds.
pub(super) struct Explorer {
    folder: Vec<String>,
    listing: Listing,
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64 / 1024.0;
    let mut unit = 0;
    while size >= 1024.0 && unit + 1 < UNITS.len() {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.1} {}", UNITS[unit])
}

fn modified_text(seconds: i64) -> String {
    chrono::DateTime::from_timestamp(seconds, 0)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

/// What deleting `data` costs.
fn data_consequence(data: Data) -> &'static str {
    match data {
        Data::Cache => "All packages will need to be reinstalled.",
        Data::Database => "Logs, items, users, credentials, etc. will be lost.",
        Data::Content => "All deployment bundles will be deleted and cannot be viewed or invoked.",
    }
}

/// `items` in a sentence: "a", "a and b", or "a, b and c".
fn join_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

impl SetupView {
    /// Shows `folder`, given as names under ricochet's home, and lists it off the UI thread.
    pub(super) fn open_folder(&mut self, folder: Vec<String>, cx: &mut Context<Self>) {
        let backend = self.server.read(cx).backend();
        let path = folder
            .iter()
            .fold(backend.paths().home.clone(), |path, name| path.join(name));
        if self.explorer.as_ref().is_none_or(|e| e.folder != folder) {
            self.explorer_scroll.set_offset(gpui::Point::default());
        }
        self.explorer = Some(Explorer {
            folder: folder.clone(),
            listing: Listing::Loading,
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { backend.list_dir(&path) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(explorer) = this.explorer.as_mut().filter(|e| e.folder == folder) {
                    explorer.listing = match result {
                        Ok(entries) => Listing::Loaded(entries),
                        Err(error) => Listing::Failed(error.to_string()),
                    };
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn reload_explorer(&mut self, cx: &mut Context<Self>) {
        if let Some(folder) = self.explorer.as_ref().map(|e| e.folder.clone()) {
            self.open_folder(folder, cx);
        }
    }

    pub(super) fn toggle_explorer(&mut self, cx: &mut Context<Self>) {
        if self.explorer.is_some() {
            self.explorer = None;
            cx.notify();
        } else {
            self.open_folder(Vec::new(), cx);
        }
    }

    pub(super) fn explorer_panel(&self, edge: Hsla, cx: &mut Context<Self>) -> Option<Div> {
        let explorer = self.explorer.as_ref()?;
        let folder = explorer.folder.clone();
        let muted = cx.theme().muted_foreground;
        let mono = cx.theme().mono_font_family.clone();
        let mut crumbs = vec![("ricochet".to_owned(), Vec::new())];
        for depth in 0..folder.len() {
            crumbs.push((folder[depth].clone(), folder[..=depth].to_vec()));
        }
        let last = crumbs.len() - 1;
        let bar = h_flex()
            .gap_1()
            .pl_4()
            .pr_2()
            .py_1()
            .items_center()
            .border_t_1()
            .border_color(edge)
            .bg(cx.theme().muted)
            .text_xs()
            .font_family(mono.clone())
            .children(
                crumbs
                    .into_iter()
                    .enumerate()
                    .map(|(index, (name, target))| {
                        let crumb = div().id(("crumb", index)).child(name);
                        let crumb = if index == last {
                            crumb.text_color(cx.theme().foreground)
                        } else {
                            crumb.text_color(cx.theme().link).cursor_pointer().on_click(
                                cx.listener(move |this, _, _, cx| {
                                    this.open_folder(target.clone(), cx)
                                }),
                            )
                        };
                        h_flex()
                            .gap_1()
                            .when(index > 0, |path| {
                                path.child(div().text_color(muted).child("/"))
                            })
                            .child(crumb)
                    }),
            )
            .child(div().flex_1())
            .child(
                Button::new("refresh-explorer")
                    .icon(ph("arrow-clockwise"))
                    .tooltip("Refresh")
                    .ghost()
                    .xsmall()
                    .on_click(cx.listener(|this, _, _, cx| this.reload_explorer(cx))),
            );
        let notice = |text: String, color: Hsla| {
            div()
                .px_4()
                .py_3()
                .text_xs()
                .text_color(color)
                .child(text)
                .into_any_element()
        };
        let body = match &explorer.listing {
            Listing::Loading => h_flex()
                .gap_2()
                .px_4()
                .py_3()
                .text_xs()
                .text_color(muted)
                .child(Spinner::new())
                .child("Reading the folder…")
                .into_any_element(),
            Listing::Failed(message) => notice(message.clone(), cx.theme().danger),
            Listing::Loaded(entries) if entries.is_empty() => {
                notice("This folder is empty.".to_owned(), muted)
            }
            Listing::Loaded(entries) => {
                let hover = cx.theme().muted;
                let divider = cx.theme().border;
                let zone = chrono::Local::now().format("UTC%:z");
                let header = h_flex()
                    .gap_3()
                    .px_4()
                    .py_1p5()
                    .text_xs()
                    .text_color(muted)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().w(px(16.0)))
                    .child(div().flex_1().child("Name"))
                    .child(div().w(px(SIZE_WIDTH)).text_right().child("Size"))
                    .child(
                        div()
                            .w(px(MODIFIED_WIDTH))
                            .text_right()
                            .child(format!("Modified ({zone})")),
                    );
                let rows = entries.iter().enumerate().map(|(index, entry)| {
                    let icon = if entry.is_dir {
                        Icon::new(IconName::Folder)
                            .size_4()
                            .text_color(cx.theme().ring)
                    } else {
                        ph("file-text").size_4().text_color(muted)
                    };
                    let row = h_flex()
                        .id(("entry", index))
                        .h(px(30.0))
                        .flex_shrink_0()
                        .gap_3()
                        .px_4()
                        .items_center()
                        .text_xs()
                        .when(index > 0, |row| row.border_t_1().border_color(divider))
                        .child(icon)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_family(mono.clone())
                                .child(entry.name.clone()),
                        )
                        .child(
                            div()
                                .w(px(SIZE_WIDTH))
                                .text_right()
                                .text_color(muted)
                                .child(if entry.is_dir {
                                    String::new()
                                } else {
                                    human_size(entry.size)
                                }),
                        )
                        .child(
                            div()
                                .w(px(MODIFIED_WIDTH))
                                .text_right()
                                .text_color(muted)
                                .child(entry.modified.map(modified_text).unwrap_or_default()),
                        );
                    if entry.is_dir {
                        let target = [folder.clone(), vec![entry.name.clone()]].concat();
                        row.cursor_pointer()
                            .hover(move |row| row.bg(hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_folder(target.clone(), cx)
                            }))
                    } else {
                        row
                    }
                });
                v_flex()
                    .child(header)
                    .child(
                        div()
                            .relative()
                            .child(
                                v_flex()
                                    .id("explorer-entries")
                                    .max_h(px(300.0))
                                    .overflow_y_scroll()
                                    .track_scroll(&self.explorer_scroll)
                                    .on_scroll(|_, _, cx| cx.stop_propagation())
                                    .children(rows),
                            )
                            .vertical_scrollbar(&self.explorer_scroll),
                    )
                    .into_any_element()
            }
        };
        Some(v_flex().child(bar).child(body))
    }

    pub(super) fn delete_data_row(&self, edge: Hsla, cx: &mut Context<Self>) -> Div {
        let busy = self.server.read(cx).activity.is_some();
        let chosen = &self.delete_choices;
        let description = "Clear out data while keeping ricochet installed.".to_owned();
        let (primary, input, on_primary, muted, hover) = (
            cx.theme().primary,
            cx.theme().input,
            cx.theme().primary_foreground,
            cx.theme().muted_foreground,
            cx.theme().muted,
        );
        let options = Data::ALL.into_iter().enumerate().map(|(index, data)| {
            let checked = chosen.contains(&data);
            let mark = div()
                .mt(px(2.0))
                .size(px(14.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .map(|mark| {
                    if checked {
                        mark.bg(primary)
                            .child(ph("check").size(px(10.0)).text_color(on_primary))
                    } else {
                        mark.border_1().border_color(input)
                    }
                });
            h_flex()
                .id(("delete-choice", index))
                .gap_3()
                .px_2()
                .py_1p5()
                .items_start()
                .cursor_pointer()
                .hover(move |row| row.bg(hover))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(position) = this.delete_choices.iter().position(|d| *d == data) {
                        this.delete_choices.remove(position);
                    } else {
                        this.delete_choices.push(data);
                    }
                    cx.notify();
                }))
                .child(mark)
                .child(
                    v_flex()
                        .gap_0p5()
                        .child(div().text_sm().child(data.title()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(data_consequence(data)),
                        ),
                )
        });
        v_flex()
            .gap_3()
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(edge)
            .child(
                h_flex()
                    .gap_4()
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
                                    .child("Delete data"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(prose(description)),
                            ),
                    )
                    .child(
                        danger_button("delete-data", "Delete selected", cx)
                            .disabled(chosen.is_empty() || busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirm_delete_data(window, cx)
                            })),
                    ),
            )
            .child(v_flex().gap_0p5().children(options))
    }

    /// Asks in a dialog before deleting the chosen data.
    fn confirm_delete_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chosen: Vec<Data> = Data::ALL
            .into_iter()
            .filter(|data| self.delete_choices.contains(data))
            .collect();
        if chosen.is_empty() {
            return;
        }
        let names: Vec<String> = chosen
            .iter()
            .map(|data| data.title().to_lowercase())
            .collect();
        let list = join_list(&names);
        let controller = self.server.read(cx);
        let message = if controller.connection.is_remote() {
            format!(
                "ricochet will be stopped while the {list} is permanently deleted from {}, then started again.",
                controller.connection.name()
            )
        } else {
            let bin = if cfg!(windows) {
                "Recycle Bin"
            } else {
                "Trash"
            };
            format!(
                "ricochet will be stopped while the {list} is moved to the {bin}, then started again."
            )
        };
        let title = format!("Delete the {list}?");
        let view = cx.entity();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let view = view.clone();
            let chosen = chosen.clone();
            dialog
                .title(prose(title.clone()))
                .child(prose(message.clone()))
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Delete")
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text("Cancel"),
                )
                .on_ok(move |_, _, cx| {
                    let data = chosen.clone();
                    view.update(cx, |this, cx| {
                        this.reload_explorer_when_idle = true;
                        this.delete_choices.clear();
                        this.server
                            .update(cx, |server, cx| server.delete_data(data, cx));
                        cx.notify();
                    });
                    true
                })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_like_a_file_manager() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1843), "1.8 KB");
        assert_eq!(human_size(316 * 1024), "316.0 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn lists_read_as_a_sentence() {
        let names = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(join_list(&names(&["database"])), "database");
        assert_eq!(
            join_list(&names(&["database", "content"])),
            "database and content"
        );
        assert_eq!(
            join_list(&names(&["package cache", "database", "content"])),
            "package cache, database and content"
        );
    }
}
