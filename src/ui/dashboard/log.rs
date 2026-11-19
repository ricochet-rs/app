//! Follows one of ricochet's logs as it is written: a deployment's build, an app instance, or a task run.

use super::DashboardView;
use crate::api::ApiError;
use futures::StreamExt;
use gpui::prelude::*;
use gpui::{App, Context, Div, ListAlignment, ListState, SharedString, div, list, px};
use gpui_component::{ActiveTheme, h_flex, scroll::ScrollableElement as _, v_flex};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The log to follow.
#[derive(Clone, Debug, PartialEq)]
pub enum LogSource {
    /// A deployment's build log, whose id ricochet looks up once the build starts.
    Build { deployment: String },
    /// A log ricochet already named: an app instance's or a task run's.
    Log { id: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogState {
    Idle,
    Waiting,
    Following,
    Ended,
    Failed(String),
}

/// One log line split into the parts ricochet writes: `[<time> <stream>] text`.
#[derive(Clone, Debug, PartialEq)]
pub struct LogLine {
    pub time: Option<SharedString>,
    pub stderr: bool,
    pub text: SharedString,
}

impl LogLine {
    pub fn parse(raw: &str) -> Self {
        let plain = || Self {
            time: None,
            stderr: false,
            text: raw.to_owned().into(),
        };
        let Some((prefix, text)) = raw.strip_prefix('[').and_then(|rest| rest.split_once("] "))
        else {
            return plain();
        };
        let mut parts = prefix.split_whitespace();
        let Some(stamp) = parts
            .next()
            .filter(|stamp| stamp.len() >= 19 && stamp.as_bytes()[10] == b'T')
        else {
            return plain();
        };
        Self {
            time: Some(stamp[11..19].to_owned().into()),
            stderr: parts.next() == Some("stderr"),
            text: text.to_owned().into(),
        }
    }
}

pub struct LogView {
    pub source: Option<LogSource>,
    pub state: LogState,
    lines: Arc<Vec<LogLine>>,
    list: ListState,
    stop: Arc<AtomicBool>,
    _task: Option<gpui::Task<()>>,
}

impl Default for LogView {
    fn default() -> Self {
        Self {
            source: None,
            state: LogState::Idle,
            lines: Arc::default(),
            list: ListState::new(0, ListAlignment::Bottom, px(400.0)),
            stop: Arc::default(),
            _task: None,
        }
    }
}

impl LogView {
    /// Stops reading the log; the reader notices at its next line.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    fn push(&mut self, raw: Vec<String>) {
        let start = self.lines.len();
        let lines = Arc::make_mut(&mut self.lines);
        lines.extend(raw.iter().map(|line| LogLine::parse(line)));
        self.list.splice(start..start, raw.len());
    }
}

impl DashboardView {
    pub(super) fn follow_log(&mut self, source: LogSource, cx: &mut Context<Self>) {
        self.log.stop();
        self.log = LogView {
            source: Some(source.clone()),
            state: LogState::Waiting,
            ..LogView::default()
        };
        let stop = self.log.stop.clone();
        let (sender, receiver) = futures::channel::mpsc::unbounded::<String>();
        self.log._task = Some(cx.spawn(async move |this, cx| {
            let mut chunks = receiver.ready_chunks(512);
            while let Some(lines) = chunks.next().await {
                let updated = this.update(cx, |this, cx| {
                    this.log.state = LogState::Following;
                    this.log.push(lines);
                    cx.notify();
                });
                if updated.is_err() {
                    break;
                }
            }
        }));
        self.fetch(
            cx,
            move |client| {
                let id = match source {
                    LogSource::Log { id } => id,
                    LogSource::Build { deployment } => client
                        .deployment_logs(&deployment)?
                        .into_iter()
                        .next()
                        .ok_or_else(|| {
                            ApiError::Failed("This deployment has no build log.".into())
                        })?,
                };
                client.follow_log(&id, &stop, |line| {
                    let _ = sender.unbounded_send(line);
                })
            },
            |this, result, _| {
                this.log.state = match result {
                    Ok(()) => LogState::Ended,
                    Err(error) => LogState::Failed(error.to_string()),
                };
            },
        );
    }

    pub(super) fn log_panel(&self, cx: &App) -> Div {
        let lines = self.log.lines.clone();
        let status = match &self.log.state {
            LogState::Idle => Some("Choose a log above.".to_owned()),
            LogState::Waiting => Some("Waiting for the log…".to_owned()),
            LogState::Following => None,
            LogState::Ended if lines.is_empty() => Some("The log is empty.".to_owned()),
            LogState::Ended => None,
            LogState::Failed(message) => Some(message.clone()),
        };
        let muted = cx.theme().muted_foreground;
        let danger = cx.theme().danger;
        let mono = cx.theme().mono_font_family.clone();
        v_flex()
            .gap_2()
            .children(status.map(|status| {
                div()
                    .text_sm()
                    .text_color(if matches!(self.log.state, LogState::Failed(_)) {
                        danger
                    } else {
                        muted
                    })
                    .child(status)
            }))
            .child(
                div()
                    .id("log-lines")
                    .h(px(420.0))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().muted.opacity(0.4))
                    .font_family(mono)
                    .text_xs()
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                    .child(
                        list(self.log.list.clone(), move |index, _, _| {
                            let line = &lines[index];
                            h_flex()
                                .px_2()
                                .gap_3()
                                .items_start()
                                .children(line.time.clone().map(|time| {
                                    div().flex_shrink_0().text_color(muted).child(time)
                                }))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .when(line.stderr, |text| text.text_color(danger))
                                        .child(line.text.clone()),
                                )
                                .into_any_element()
                        })
                        .size_full(),
                    )
                    .vertical_scrollbar(&self.log.list),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_split_into_time_stream_and_text() {
        let line = LogLine::parse("[2026-11-19T14:05:09.123Z stderr] Error: boom");
        assert_eq!(
            line.time.as_ref().map(|time| time.as_ref()),
            Some("14:05:09")
        );
        assert!(line.stderr);
        assert_eq!(line.text.as_ref(), "Error: boom");
        let line = LogLine::parse("[2026-11-19T14:05:09Z] Listening");
        assert!(!line.stderr);
        assert_eq!(line.text.as_ref(), "Listening");
        assert_eq!(LogLine::parse("plain [text]").text.as_ref(), "plain [text]");
    }
}
