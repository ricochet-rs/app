//! The Apps and Tasks pages: a table of items, laid out like ricochet's own.

use super::item::Tab;
use super::{DashboardView, Page, ago, status_icon};
use crate::api::{self, Item};
use crate::ui::components::{chip, page_title, prose};
use crate::ui::icons::ph;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Div, FontWeight, div, px, uniform_list};
use gpui_component::button::Button;
use gpui_component::input::Input;
use gpui_component::{ActiveTheme, Sizable, h_flex, v_flex};
use std::ops::Range;

const ROW_HEIGHT: f32 = 64.0;
const STATUS_WIDTH: f32 = 96.0;
const TYPE_WIDTH: f32 = 170.0;
const ACCESS_WIDTH: f32 = 110.0;
const UPDATED_WIDTH: f32 = 96.0;
/// How many tags a row shows before it counts the rest.
const SHOWN_TAGS: usize = 2;

impl DashboardView {
    pub(super) fn content_table(&self, cx: &mut Context<Self>) -> AnyElement {
        let tasks = self.page == Page::Tasks;
        let count = self.visible_items(cx).len();
        let any = self.items().iter().any(|item| item.is_task() == tasks);
        let controls = h_flex()
            .gap_2()
            .child(
                div().w(px(280.0)).child(
                    Input::new(&self.search)
                        .prefix(ph("magnifying-glass").size_4())
                        .cleanable(true),
                ),
            )
            .child(
                Button::new("refresh")
                    .icon(ph("arrow-clockwise"))
                    .tooltip("Refresh")
                    .outline()
                    .on_click(cx.listener(|this, _, _, cx| this.load(cx))),
            );
        let header = h_flex()
            .h(px(44.0))
            .px_4()
            .gap_4()
            .items_center()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().w(px(36.0)))
            .child(div().flex_1().child("Name"))
            .child(div().w(px(STATUS_WIDTH)).child("Status"))
            .child(div().w(px(TYPE_WIDTH)).child("Type"))
            .child(div().w(px(ACCESS_WIDTH)).child("Visibility"))
            .child(div().w(px(UPDATED_WIDTH)).child("Updated"));
        let body = if !any {
            empty(
                if tasks {
                    "No tasks are deployed to this server yet."
                } else {
                    "No apps are deployed to this server yet."
                },
                cx,
            )
        } else if count == 0 {
            empty("Nothing matches your search.", cx)
        } else {
            uniform_list(
                "content-table",
                count,
                cx.processor(|this, range, _, cx| this.table_rows(range, cx)),
            )
            .flex_1()
            .min_h_0()
            .into_any_element()
        };
        v_flex()
            .flex_1()
            .min_h_0()
            .p_6()
            .gap_5()
            .child(page_title(
                if tasks { "Tasks" } else { "Apps" },
                Some(controls.into_any_element()),
            ))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(crate::ui::theme::Palette::card(cx))
                    .child(header)
                    .child(body),
            )
            .into_any_element()
    }

    fn table_rows(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let server_url = self
            .active_server(cx)
            .map(|server| server.url)
            .unwrap_or_default();
        self.visible_items(cx)
            .into_iter()
            .enumerate()
            .skip(range.start)
            .take(range.len())
            .map(|(index, item)| {
                let id = item.id.clone();
                let open_url = format!("{server_url}/app/{}/", item.id);
                let hover = cx.theme().muted.opacity(0.5);
                let deployed = item.deployment_status.as_deref() == Some("success");
                h_flex()
                    .id(("row", index))
                    .h(px(ROW_HEIGHT))
                    .px_4()
                    .gap_4()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .cursor_pointer()
                    .hover(move |row| row.bg(hover))
                    .child(language_tile(&item, cx))
                    .child(name_cell(&item, cx))
                    .child(
                        h_flex()
                            .w(px(STATUS_WIDTH))
                            .gap_2()
                            .items_center()
                            .child(status_icon(item.deployment_status.as_deref(), cx))
                            .when(deployed && !item.is_task(), |cell| {
                                cell.child(
                                    Button::new(("open-row", index))
                                        .icon(ph("arrow-square-out"))
                                        .label("Open")
                                        .outline()
                                        .xsmall()
                                        .on_click(move |_, _, _| {
                                            let _ = open::that(&open_url);
                                        }),
                                )
                            }),
                    )
                    .child(
                        div()
                            .w(px(TYPE_WIDTH))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(item.kind()),
                    )
                    .child(
                        h_flex()
                            .w(px(ACCESS_WIDTH))
                            .gap_1p5()
                            .items_center()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(ph(access_icon(&item.access_type)).size_4())
                            .child(api::humanize(&item.access_type)),
                    )
                    .child(
                        div()
                            .w(px(UPDATED_WIDTH))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(ago(item.modified_at)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_item(id.clone(), Tab::Overview, cx)
                    }))
                    .into_any_element()
            })
            .collect()
    }
}

/// A square with the item's language, standing in for ricochet's language logos.
pub(super) fn language_tile(item: &Item, cx: &App) -> Div {
    let short = match item.language.as_str() {
        "r" => "R",
        "python" => "Py",
        "julia" => "Jl",
        _ => "·",
    };
    div()
        .size(px(36.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(cx.theme().muted)
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .font_family(cx.theme().mono_font_family.clone())
        .text_color(cx.theme().muted_foreground)
        .child(short)
}

fn name_cell(item: &Item, cx: &App) -> Div {
    let tags = item.tags.clone().unwrap_or_default();
    let hidden = tags.len().saturating_sub(SHOWN_TAGS);
    let palette = [
        cx.theme().info,
        cx.theme().danger,
        cx.theme().success,
        cx.theme().warning,
    ];
    v_flex()
        .flex_1()
        .min_w_0()
        .gap_0p5()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .min_w_0()
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(item.name.clone()),
                )
                .children(
                    tags.into_iter()
                        .take(SHOWN_TAGS)
                        .enumerate()
                        .map(|(index, tag)| chip(tag, palette[index % palette.len()])),
                )
                .when(hidden > 0, |row| {
                    row.child(chip(format!("+{hidden}"), cx.theme().muted_foreground))
                }),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .truncate()
                .child(
                    item.summary
                        .clone()
                        .filter(|summary| !summary.is_empty())
                        .unwrap_or_else(|| "No summary".into()),
                ),
        )
}

pub(super) fn access_icon(access: &str) -> &'static str {
    match access {
        "external" => "globe",
        "internal" => "users-three",
        _ => "lock",
    }
}

fn empty(text: &str, cx: &App) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .py_12()
        .text_color(cx.theme().muted_foreground)
        .child(prose(text.to_owned()))
        .into_any_element()
}
