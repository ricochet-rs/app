//! The small pieces every pane is built from.

use super::icons::ph;
use gpui::{
    AnyElement, App, ClickEvent, Div, Hsla, IntoElement, ParentElement, SharedString, Styled,
    Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Sizable, h_flex, v_flex};
use std::rc::Rc;

pub fn heading(title: &str, summary: Option<&str>, cx: &App) -> Div {
    v_flex()
        .gap_1p5()
        .child(
            div()
                .text_xl()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title.to_owned()),
        )
        .children(summary.map(|summary| {
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(summary.to_owned())
        }))
}

/// A label above `content`, with an optional description under it.
pub fn field(
    label: impl Into<SharedString>,
    description: Option<String>,
    content: impl IntoElement,
    cx: &App,
) -> Div {
    v_flex()
        .gap_1p5()
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(label.into()),
        )
        .child(content)
        .children(description.map(|description| {
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(description)
        }))
}

/// A row of buttons of which exactly one is selected.
pub fn segmented(
    id: impl Into<SharedString>,
    options: Vec<(&'static str, bool)>,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> Div {
    let on_select = Rc::new(on_select);
    let id: SharedString = id.into();
    h_flex().gap_1().children(
        options
            .into_iter()
            .enumerate()
            .map(|(index, (title, selected))| {
                let on_select = on_select.clone();
                let button = Button::new((id.clone(), index)).label(title).small();
                let button = if selected {
                    button.primary()
                } else {
                    button.outline()
                };
                button.on_click(move |_, window, cx| on_select(index, window, cx))
            }),
    )
}

/// A boxed notice with a title, a message, and optional buttons.
pub fn callout(title: &str, message: &str, tint: Hsla, actions: Vec<AnyElement>, cx: &App) -> Div {
    h_flex()
        .gap_3()
        .p_3()
        .items_start()
        .border_1()
        .border_color(tint)
        .child(ph("warning").size_5().text_color(tint).flex_shrink_0())
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(tint)
                        .child(title.to_owned()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .child(message.to_owned()),
                ),
        )
        .child(h_flex().flex_shrink_0().gap_2().children(actions))
}

pub fn error_alert(
    title: &str,
    message: &str,
    dismiss: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    callout(
        title,
        message,
        cx.theme().danger,
        vec![
            Button::new("dismiss-alert")
                .label("Dismiss")
                .ghost()
                .small()
                .on_click(dismiss)
                .into_any_element(),
        ],
        cx,
    )
}

pub fn rule(cx: &App) -> Div {
    div().h(px(1.0)).w_full().bg(cx.theme().border)
}
