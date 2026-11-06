//! The Logging pane's Live tab: the end of ricochet's newest log file, read again every two seconds.

use super::SetupView;
use crate::backend::Backend;
use crate::ui::icons::ph;
use gpui::prelude::*;
use gpui::{ClipboardItem, Context, Div, Hsla, div, px};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{ActiveTheme, Sizable, h_flex, v_flex};
use serde_json::Value;
use std::time::Duration;

const TAIL_LINES: &str = "500";
/// The window height the rest of the pane takes, so the log fills what is left.
const CHROME_HEIGHT: f32 = 320.0;
const MIN_LOG_HEIGHT: f32 = 240.0;

/// The levels ricochet logs at, most severe first.
pub(super) const LEVELS: [&str; 5] = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];

/// One line of ricochet's JSON log.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct LogEntry {
    time: String,
    level: String,
    message: String,
    target: String,
    fields: Vec<(String, String)>,
    /// The line as ricochet wrote it, for copying.
    raw: String,
}

impl LogEntry {
    /// Reads a line ricochet wrote as JSON, keeping any other line whole as the message.
    fn parse(line: &str) -> Self {
        let Ok(Value::Object(mut entry)) = serde_json::from_str::<Value>(line) else {
            return Self {
                time: String::new(),
                level: String::new(),
                message: line.to_owned(),
                target: String::new(),
                fields: Vec::new(),
                raw: line.to_owned(),
            };
        };
        let text = |value: &Value| match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        let time = entry
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|stamp| chrono::DateTime::parse_from_rfc3339(stamp).ok())
            .map(|stamp| {
                stamp
                    .with_timezone(&chrono::Local)
                    .format("%H:%M:%S")
                    .to_string()
            })
            .unwrap_or_default();
        let level = entry
            .get("level")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_uppercase();
        let target = entry
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let mut message = String::new();
        let mut fields = Vec::new();
        if let Some(Value::Object(values)) = entry.remove("fields") {
            for (key, value) in values {
                if key == "message" {
                    message = text(&value);
                } else {
                    fields.push((key, text(&value)));
                }
            }
        }
        if let Some(span) = entry
            .get("span")
            .and_then(|span| span.get("name"))
            .and_then(Value::as_str)
        {
            fields.push(("span".to_owned(), span.to_owned()));
        }
        Self {
            time,
            level,
            message,
            target,
            fields,
            raw: line.to_owned(),
        }
    }
}

/// What the Live tab last read.
#[derive(Default)]
pub(super) struct LogTail {
    file: Option<String>,
    entries: Vec<LogEntry>,
    error: Option<String>,
}

/// The newest log file's name and its last lines.
fn read_tail(backend: &dyn Backend) -> Result<(String, Vec<LogEntry>), String> {
    let logs = backend.paths().logs.clone();
    let entries = backend.list_dir(&logs).map_err(|error| error.to_string())?;
    let newest = entries
        .iter()
        .filter(|entry| !entry.is_dir && !entry.name.ends_with(".gz"))
        .max_by_key(|entry| entry.modified.unwrap_or(0))
        .ok_or_else(|| "ricochet hasn't written a log file yet.".to_owned())?;
    let path = logs.join(&newest.name);
    let result = backend
        .shell()
        .run("tail", &["-n", TAIL_LINES, &path.to_string_lossy()])
        .map_err(|error| error.to_string())?;
    if !result.success {
        return Err(result.output());
    }
    let parsed = result
        .stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(LogEntry::parse)
        .collect();
    Ok((newest.name.clone(), parsed))
}

impl SetupView {
    /// Starts reading the log every two seconds, unless that is already running.
    pub(super) fn follow_logs(&mut self, cx: &mut Context<Self>) {
        if self.log_task.is_some() {
            return;
        }
        let backend = self.server.read(cx).backend();
        self.log_task = Some(cx.spawn(async move |this, cx| {
            loop {
                let reader = backend.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move { read_tail(reader.as_ref()) })
                    .await;
                let updated = this.update(cx, |this, cx| {
                    let offset = this.log_scroll.offset();
                    let at_bottom = -offset.y >= this.log_scroll.max_offset().height - px(8.0);
                    match result {
                        Ok((file, entries)) => {
                            let changed = this.log_tail.entries != entries;
                            this.log_tail = LogTail {
                                file: Some(file),
                                entries,
                                error: None,
                            };
                            if changed && at_bottom {
                                this.log_scroll.scroll_to_bottom();
                            }
                        }
                        Err(error) => this.log_tail.error = Some(error),
                    }
                    cx.notify();
                });
                if updated.is_err() {
                    break;
                }
                cx.background_executor().timer(Duration::from_secs(2)).await;
            }
        }));
    }

    pub(super) fn stop_following_logs(&mut self) {
        self.log_task = None;
    }

    fn level_color(&self, level: &str, cx: &Context<Self>) -> Hsla {
        match level {
            "ERROR" => cx.theme().danger,
            "WARN" => cx.theme().warning,
            "INFO" => cx.theme().ring,
            _ => cx.theme().muted_foreground,
        }
    }

    pub(super) fn live_logs(&self, cx: &mut Context<Self>) -> Div {
        let tail = &self.log_tail;
        let muted = cx.theme().muted_foreground;
        let mono = cx.theme().mono_font_family.clone();
        let status = match (&tail.error, &tail.file) {
            (Some(error), _) => div().text_color(cx.theme().danger).child(error.clone()),
            (None, Some(file)) => h_flex()
                .gap_1p5()
                .items_center()
                .child(div().size(px(6.0)).rounded_full().bg(cx.theme().success))
                .child(div().font_family(mono.clone()).child(file.clone())),
            (None, None) => div().child("Reading the log…"),
        };
        let filters = h_flex()
            .gap_1()
            .children(LEVELS.iter().enumerate().map(|(index, level)| {
                let shown = !self.hidden_levels.contains(level);
                let color = self.level_color(level, cx);
                let level = *level;
                div()
                    .id(("log-level", index))
                    .px_2()
                    .py_0p5()
                    .border_1()
                    .cursor_pointer()
                    .font_family(mono.clone())
                    .map(|chip| {
                        if shown {
                            chip.border_color(color)
                                .text_color(color)
                                .bg(color.opacity(0.1))
                        } else {
                            chip.border_color(cx.theme().border).text_color(muted)
                        }
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(position) = this.hidden_levels.iter().position(|l| *l == level)
                        {
                            this.hidden_levels.remove(position);
                        } else {
                            this.hidden_levels.push(level);
                        }
                        cx.notify();
                    }))
                    .child(level)
            }));
        let divider = cx.theme().border;
        let (stripe_even, stripe_odd) = (cx.theme().background, cx.theme().muted);
        let visible_lines = tail
            .entries
            .iter()
            .filter(|entry| !self.hidden_levels.contains(&entry.level.as_str()))
            .map(|entry| entry.raw.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let copy_all = Button::new("copy-logs")
            .icon(ph("copy"))
            .label("Copy all")
            .ghost()
            .xsmall()
            .on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(visible_lines.clone()))
            });
        let rows = tail
            .entries
            .iter()
            .filter(|entry| !self.hidden_levels.contains(&entry.level.as_str()))
            .enumerate()
            .map(|(index, entry)| {
                let details: Vec<String> = std::iter::once(entry.target.clone())
                    .filter(|target| !target.is_empty())
                    .chain(
                        entry
                            .fields
                            .iter()
                            .map(|(key, value)| format!("{key}={value}")),
                    )
                    .collect();
                let accent = match entry.level.as_str() {
                    "ERROR" | "WARN" => self.level_color(&entry.level, cx),
                    _ => gpui::transparent_black(),
                };
                let raw = entry.raw.clone();
                h_flex()
                    .id(("log-row", index))
                    .group("log-row")
                    .gap_3()
                    .pl_3()
                    .pr_2()
                    .py_2()
                    .items_start()
                    .relative()
                    .bg(if index % 2 == 0 {
                        stripe_even
                    } else {
                        stripe_odd
                    })
                    .border_b_1()
                    .border_color(divider)
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .bottom_0()
                            .w(px(3.0))
                            .bg(accent),
                    )
                    .child(
                        div()
                            .w(px(60.0))
                            .flex_shrink_0()
                            .text_color(muted)
                            .child(entry.time.clone()),
                    )
                    .child(
                        div()
                            .w(px(44.0))
                            .flex_shrink_0()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(self.level_color(&entry.level, cx))
                            .child(entry.level.clone()),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_color(cx.theme().foreground)
                                    .child(entry.message.clone()),
                            )
                            .when(!details.is_empty(), |text| {
                                text.child(div().text_color(muted).child(details.join("  ")))
                            }),
                    )
                    .child(
                        div()
                            .invisible()
                            .group_hover("log-row", |copy| copy.visible())
                            .child(
                                Button::new(("copy-log", index))
                                    .icon(ph("copy"))
                                    .tooltip("Copy line")
                                    .ghost()
                                    .xsmall()
                                    .on_click(move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            raw.clone(),
                                        ))
                                    }),
                            ),
                    )
            });
        let height = (self.viewport_height - px(CHROME_HEIGHT)).max(px(MIN_LOG_HEIGHT));
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .justify_between()
                    .gap_4()
                    .text_xs()
                    .text_color(muted)
                    .child(status)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(filters)
                            .child(copy_all),
                    ),
            )
            .child(
                div()
                    .relative()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().muted)
                    .child(
                        v_flex()
                            .id("log-lines")
                            .h(height)
                            .overflow_y_scroll()
                            .track_scroll(&self.log_scroll)
                            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                            .text_xs()
                            .font_family(mono)
                            .children(rows),
                    )
                    .vertical_scrollbar(&self.log_scroll),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ricochets_json_lines() {
        let entry = LogEntry::parse(
            r#"{"timestamp":"2026-11-06T18:56:10.349454Z","level":"WARN","fields":{"message":"vendored `juliaup` not found","path":"/home/josi/jl"},"target":"ricochet_core::jl","span":{"name":"setup"}}"#,
        );
        assert_eq!(entry.level, "WARN");
        assert_eq!(entry.message, "vendored `juliaup` not found");
        assert_eq!(entry.target, "ricochet_core::jl");
        assert!(
            entry
                .fields
                .contains(&("path".to_owned(), "/home/josi/jl".to_owned()))
        );
        assert!(
            entry
                .fields
                .contains(&("span".to_owned(), "setup".to_owned()))
        );
        assert!(!entry.time.is_empty());
    }

    #[test]
    fn keeps_other_lines_whole() {
        let entry = LogEntry::parse("thread 'main' panicked");
        assert_eq!(entry.message, "thread 'main' panicked");
        assert!(entry.level.is_empty());
    }
}
