//! The small pieces every pane is built from.

use super::icons::ph;
use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, Font, Hsla, InteractiveText, IntoElement,
    ParentElement, RenderOnce, SharedString, Styled, StyledText, UnderlineStyle, Window, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::tab::{Tab, TabBar};
use gpui_component::{ActiveTheme, IconName, Sizable, h_flex, v_flex};
use std::ops::Range;
use std::rc::Rc;

/// Text that sets `backticked` spans and each standalone `ricochet`, in lowercase, in the monospace font.
#[derive(IntoElement)]
pub struct Prose {
    text: SharedString,
    /// Words of the text that open a URL when clicked.
    link: Option<(&'static str, &'static str)>,
    /// The link's colour, or the theme's link colour when `None`.
    link_color: Option<Hsla>,
}

pub fn prose(text: impl Into<SharedString>) -> Prose {
    Prose {
        text: text.into(),
        link: None,
        link_color: None,
    }
}

impl Prose {
    /// Underlines the first `words` of the text and opens `url` when they are clicked.
    pub fn link(mut self, words: &'static str, url: &'static str) -> Self {
        self.link = Some((words, url));
        self
    }

    pub fn link_color(mut self, color: Hsla) -> Self {
        self.link_color = Some(color);
        self
    }
}

impl RenderOnce for Prose {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (text, mono_ranges) = mono_spans(&self.text);
        let link = self.link.and_then(|(words, url)| {
            text.find(words)
                .map(|start| (start..start + words.len(), url))
        });
        if mono_ranges.is_empty() && link.is_none() {
            return StyledText::new(text).into_any_element();
        }
        let style = window.text_style();
        let mono = Font {
            family: cx.theme().mono_font_family.clone(),
            ..style.font()
        };
        let link_color = self.link_color.unwrap_or(cx.theme().link);
        let mut cuts = vec![0, text.len()];
        for range in mono_ranges
            .iter()
            .chain(link.iter().map(|(range, _)| range))
        {
            cuts.extend([range.start, range.end]);
        }
        cuts.sort_unstable();
        cuts.dedup();
        let covers = |range: &Range<usize>, start: usize, end: usize| {
            range.start <= start && end <= range.end
        };
        let runs = cuts
            .windows(2)
            .map(|cut| {
                let (start, end) = (cut[0], cut[1]);
                let mut run = style.to_run(end - start);
                if mono_ranges.iter().any(|r| covers(r, start, end)) {
                    run.font = mono.clone();
                }
                if link.as_ref().is_some_and(|(r, _)| covers(r, start, end)) {
                    run.color = link_color;
                    run.underline = Some(UnderlineStyle {
                        thickness: px(1.0),
                        color: Some(link_color),
                        wavy: false,
                    });
                }
                run
            })
            .collect();
        let styled = StyledText::new(text).with_runs(runs);
        match link {
            Some((range, url)) => InteractiveText::new(SharedString::from(url), styled)
                .on_click(vec![range], move |_, _, _| {
                    let _ = open::that(url);
                })
                .into_any_element(),
            None => styled.into_any_element(),
        }
    }
}

const BRAND: &str = "ricochet";

/// The text with its backticks removed, and the ranges of it to set in the monospace font.
fn mono_spans(source: &str) -> (String, Vec<Range<usize>>) {
    let mut text = String::with_capacity(source.len());
    let mut ranges = Vec::new();
    for (index, part) in source.split('`').enumerate() {
        let start = text.len();
        if index % 2 == 1 {
            text.push_str(part);
            ranges.push(start..text.len());
            continue;
        }
        let mut part = part.to_owned();
        for range in brand_ranges(&part) {
            part.replace_range(range.clone(), BRAND);
            ranges.push(start + range.start..start + range.end);
        }
        text.push_str(&part);
    }
    (text, ranges)
}

/// Where `ricochet` stands as a word of its own, rather than inside a path, host, file or command name.
fn brand_ranges(text: &str) -> Vec<Range<usize>> {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let joins = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-');
    let links = |b: u8| matches!(b, b'.' | b'/' | b'@' | b':');
    lower
        .match_indices(BRAND)
        .map(|(start, _)| start..start + BRAND.len())
        .filter(|range| {
            let before = range.start.checked_sub(1).map(|i| bytes[i]);
            let after = bytes.get(range.end).copied();
            let next = bytes.get(range.end + 1).copied();
            !before.is_some_and(|b| joins(b) || links(b) || b == b'<')
                && !after.is_some_and(joins)
                && !(after.is_some_and(links) && next.is_some_and(joins))
        })
        .collect()
}

pub fn heading(title: &str, summary: Option<&str>, cx: &App) -> Div {
    v_flex()
        .gap_1p5()
        .child(
            div()
                .text_xl()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(prose(title.to_owned())),
        )
        .children(summary.map(|summary| {
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(prose(summary.to_owned()))
        }))
}

/// A label above `content`, with an optional description under it.
pub fn field(
    label: impl Into<SharedString>,
    description: Option<String>,
    content: impl IntoElement,
    cx: &App,
) -> Div {
    labelled(label, false, description, content, cx)
}

fn labelled(
    label: impl Into<SharedString>,
    optional: bool,
    description: Option<String>,
    content: impl IntoElement,
    cx: &App,
) -> Div {
    v_flex()
        .gap_1p5()
        .child(
            h_flex()
                .gap_1()
                .text_sm()
                .child(
                    div()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(prose(label)),
                )
                .when(optional, |label| {
                    label.child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("(optional)"),
                    )
                }),
        )
        .child(content)
        .children(description.map(|description| {
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(prose(description))
        }))
}

/// A solid red button for actions that delete something, brightening to full red on hover.
pub fn danger_button(id: &'static str, label: &'static str, cx: &App) -> Button {
    let danger = cx.theme().danger;
    Button::new(id)
        .icon(ph("trash"))
        .label(label)
        .small()
        .custom(
            ButtonCustomVariant::new(cx)
                .color(danger.opacity(0.85))
                .foreground(cx.theme().danger_foreground)
                .hover(danger)
                .active(danger),
        )
}

/// A panel that groups settings, in the style of gpui-component's setting groups.
pub fn card(cx: &App) -> Div {
    v_flex()
        .gap_5()
        .p_5()
        .border_1()
        .border_color(cx.theme().border)
        .bg(super::theme::Palette::card(cx))
}

/// A heading above a [`card`].
pub fn card_title(title: &str, cx: &App) -> Div {
    div()
        .text_sm()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(cx.theme().foreground)
        .child(prose(title.to_owned()))
}

/// A setting with its label and description on the left and its control on the right.
pub fn setting_row(
    label: impl Into<SharedString>,
    optional: bool,
    description: Option<String>,
    control: impl IntoElement,
    cx: &App,
) -> Div {
    h_flex()
        .gap_6()
        .items_start()
        .child(
            v_flex()
                .w(gpui::relative(0.42))
                .flex_shrink_0()
                .gap_1()
                .child(
                    h_flex()
                        .gap_1()
                        .flex_wrap()
                        .text_sm()
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(prose(label)),
                        )
                        .when(optional, |label| {
                            label.child(
                                div()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("(optional)"),
                            )
                        }),
                )
                .children(description.map(|description| {
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(prose(description))
                })),
        )
        .child(div().flex_1().min_w_0().child(control))
}

/// A row of buttons of which exactly one is selected.
pub fn segmented(
    id: impl Into<SharedString>,
    options: Vec<(&'static str, bool)>,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> Div {
    let on_select = Rc::new(on_select);
    let id: SharedString = id.into();
    h_flex()
        .gap_1()
        .flex_wrap()
        .children(
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

/// gpui-component's underlined tabs over a pane.
pub fn tab_bar(
    id: impl Into<SharedString>,
    tabs: Vec<(&'static str, bool)>,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> TabBar {
    let selected = tabs.iter().position(|(_, selected)| *selected).unwrap_or(0);
    TabBar::new(ElementId::Name(id.into()))
        .underline()
        .selected_index(selected)
        .on_click(move |index, window, cx| on_select(*index, window, cx))
        .children(tabs.into_iter().map(|(title, _)| Tab::new().label(title)))
}

/// A tinted notice with a title, a message, and optional buttons.
pub fn callout(title: &str, message: &str, tint: Hsla, actions: Vec<AnyElement>, cx: &App) -> Div {
    h_flex()
        .gap_3()
        .pl_4()
        .pr_2()
        .py_3()
        .items_start()
        .bg(tint.opacity(0.08))
        .border_l_2()
        .border_color(tint)
        .child(
            div()
                .pt(px(1.0))
                .child(ph("warning").size_5().text_color(tint).flex_shrink_0()),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(prose(title.to_owned())),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(prose(message.to_owned())),
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
    let copied = format!("{title}\n{message}");
    callout(
        title,
        message,
        cx.theme().danger,
        vec![
            Button::new("copy-alert")
                .icon(ph("copy"))
                .tooltip("Copy")
                .ghost()
                .xsmall()
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(copied.clone()))
                })
                .into_any_element(),
            Button::new("dismiss-alert")
                .icon(IconName::Close)
                .tooltip("Dismiss")
                .ghost()
                .xsmall()
                .on_click(dismiss)
                .into_any_element(),
        ],
        cx,
    )
}

/// A square that fills and shows a check once its option is selected.
pub fn selection_mark(selected: bool, cx: &App) -> Div {
    let mark = div()
        .size(px(18.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center();
    if selected {
        mark.bg(cx.theme().ring)
            .child(ph("check").size_3().text_color(cx.theme().background))
    } else {
        mark.border_1().border_color(cx.theme().input)
    }
}

pub fn rule(cx: &App) -> Div {
    div().h(px(1.0)).w_full().bg(cx.theme().border)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_ricochet_as_a_word_only() {
        fn found(text: &str) -> Vec<&str> {
            brand_ranges(text).into_iter().map(|r| &text[r]).collect()
        }
        assert_eq!(
            found("Ricochet keeps ricochet's files."),
            ["Ricochet", "ricochet"]
        );
        assert_eq!(found("Open ricochet"), ["ricochet"]);
        assert!(found("_ricochet.toml ricochet-server idp.ricochet.rs").is_empty());
        assert!(found("<ricochet@example.com>").is_empty());
        assert_eq!(found("Where should ricochet run?"), ["ricochet"]);
        assert_eq!(found("starts ricochet."), ["ricochet"]);
    }

    #[test]
    fn strips_backticks_and_marks_their_spans() {
        let (text, ranges) = mono_spans("Use `localhost` so Ricochet answers.");
        assert_eq!(text, "Use localhost so ricochet answers.");
        let marked: Vec<&str> = ranges.into_iter().map(|r| &text[r]).collect();
        assert_eq!(marked, ["localhost", "ricochet"]);
    }
}
