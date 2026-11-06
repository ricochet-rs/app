//! First-run setup: one decision per screen, then a live install checklist.

use super::{SetupView, Step};
use crate::config::section::{Field as _, SettingsSection as _};
use crate::ui::components::{error_alert, field};
use crate::ui::icons::ph;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Div, FontWeight, div, px};
use gpui_component::Disableable as _;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::spinner::Spinner;
use gpui_component::{ActiveTheme, h_flex, v_flex};

const INSTALL_STAGES: [&str; 4] = [
    "Checking your settings",
    "Downloading ricochet",
    "Writing your configuration",
    "Starting ricochet",
];

/// Which stage of the install the controller's activity belongs to.
fn stage_of(activity: &str) -> usize {
    match activity.split_whitespace().next() {
        Some("Checking") | Some("Stopping") => 0,
        Some("Downloading") => 1,
        Some("Writing") => 2,
        _ => 3,
    }
}

impl Step {
    /// The position among the screens that count toward progress, or `None` for the ones that don't.
    fn number(self) -> Option<usize> {
        match self {
            Self::Target => Some(1),
            Self::SignIn => Some(2),
            Self::Server => Some(3),
            Self::Review => Some(4),
            _ => None,
        }
    }
}

impl SetupView {
    pub(super) fn onboarding_view(&mut self, step: Step, cx: &mut Context<Self>) -> AnyElement {
        let (title, subtitle, body) = match step {
            Step::Welcome => self.welcome(cx),
            Step::Target => self.target_step(cx),
            Step::SignIn => (
                "How will people sign in?".into(),
                "Pick the easiest option now. You can switch later.".into(),
                self.authentication_pane(cx),
            ),
            Step::Server => self.server_step(cx),
            Step::Review => self.review_step(cx),
            Step::Installing => self.installing_step(cx),
            Step::Done => self.done_step(cx),
        };
        let wide = step == Step::SignIn;
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .id("onboarding")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        h_flex().w_full().justify_center().px_6().py_10().child(
                            v_flex()
                                .w_full()
                                .max_w(px(if wide { 640.0 } else { 480.0 }))
                                .gap_8()
                                .child(self.progress_dots(step, cx))
                                .child(
                                    v_flex()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_2xl()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(title),
                                        )
                                        .child(
                                            div()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(subtitle),
                                        ),
                                )
                                .child(body),
                        ),
                    ),
            )
            .child(self.onboarding_footer(step, cx))
            .into_any_element()
    }

    /// Three bars that fill as the person moves through the choices.
    fn progress_dots(&self, step: Step, cx: &Context<Self>) -> Div {
        let reached = step.number().unwrap_or(match step {
            Step::Welcome => 0,
            _ => 4,
        });
        h_flex().gap_1p5().children((1..=4).map(|n| {
            div().h(px(4.0)).w(px(32.0)).bg(if n <= reached {
                cx.theme().primary
            } else {
                cx.theme().border
            })
        }))
    }

    /// Shown while the app logs in to the computer it managed last time.
    pub(super) fn connecting_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let name = self.server.read(cx).connection.name();
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(Spinner::new())
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Connecting to {name}…")),
            )
            .into_any_element()
    }

    fn welcome(&self, cx: &mut Context<Self>) -> (String, String, AnyElement) {
        let row = |icon: &'static str, title: &'static str, text: &'static str| {
            h_flex()
                .gap_4()
                .items_start()
                .child(
                    div()
                        .size(px(40.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(cx.theme().muted)
                        .child(ph(icon).size_5()),
                )
                .child(
                    v_flex()
                        .gap_0p5()
                        .child(div().font_weight(FontWeight::MEDIUM).child(title))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(text),
                        ),
                )
        };
        (
            "Welcome to ricochet".into(),
            "Set up a ricochet server in about a minute, or manage one you already have.".into(),
            v_flex()
                .gap_5()
                .child(row(
                    "desktop",
                    "Runs on this computer",
                    "Your content and data stay here.",
                ))
                .child(row(
                    "power",
                    "Starts when you log in",
                    "It keeps running in the background.",
                ))
                .child(row(
                    "sliders-horizontal",
                    "Change anything later",
                    "Email, packages, logging and more are one click away in Settings.",
                ))
                .into_any_element(),
        )
    }

    fn target_step(&mut self, cx: &mut Context<Self>) -> (String, String, AnyElement) {
        let server = self.server.read(cx);
        let busy = server.activity.is_some();
        let active = server.connection.clone();
        let connecting = server.activity.clone();
        let choices = server.choices();
        let failure = server.failure.clone();
        let rows = choices.into_iter().enumerate().map(|(index, choice)| {
            let selected = choice == active && !busy;
            let this_one = connecting
                .as_deref()
                .is_some_and(|a| a.ends_with(&choice.name()));
            let (icon, detail) = if choice.is_remote() {
                ("hard-drives", "Linux over SSH")
            } else {
                ("desktop", "Runs here")
            };
            let marker: AnyElement = if this_one {
                Spinner::new().into_any_element()
            } else if selected {
                ph("check-circle")
                    .size_5()
                    .text_color(cx.theme().success)
                    .into_any_element()
            } else {
                div().into_any_element()
            };
            let hover = cx.theme().muted;
            let target = choice.clone();
            h_flex()
                .id(("target", index))
                .gap_3()
                .px_4()
                .py_3()
                .items_center()
                .border_1()
                .border_color(if selected {
                    cx.theme().ring
                } else {
                    cx.theme().border
                })
                .when(!busy, |row| {
                    row.cursor_pointer().hover(move |row| row.bg(hover))
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.server.read(cx).activity.is_some() {
                        let target = target.clone();
                        this.server
                            .update(cx, |server, cx| server.switch(target, cx));
                    }
                }))
                .child(ph(icon).size_5())
                .child(
                    v_flex()
                        .flex_1()
                        .child(div().font_weight(FontWeight::MEDIUM).child(choice.name()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(detail),
                        ),
                )
                .child(marker)
        });
        let alert = failure.map(|failure| {
            let server = self.server.clone();
            error_alert(
                &failure.title,
                &failure.message,
                move |_, _, cx| server.update(cx, |server, cx| server.dismiss_failure(cx)),
                cx,
            )
        });
        let connect = Button::new("connect-host")
            .label("Connect")
            .icon(ph("arrow-right"))
            .primary()
            .disabled(busy)
            .on_click(cx.listener(|this, _, window, cx| {
                let host = this.host_input.read(cx).value().trim().to_owned();
                if !crate::backend::is_valid_host(&host) {
                    this.problem =
                        Some("Enter an SSH host from your SSH config, or user@host.".into());
                    cx.notify();
                    return;
                }
                this.problem = None;
                this.host_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                this.server.update(cx, |server, cx| {
                    server.switch(crate::connections::Connection::Ssh { host }, cx)
                });
            }));
        (
            "Where should ricochet run?".into(),
            "Pick a computer. Ricochet is managed over SSH on other computers, using the keys you already have.".into(),
            v_flex()
                .gap_6()
                .children(alert)
                .child(v_flex().gap_2().children(rows))
                .child(field(
                    "Add another computer",
                    Some("An alias from your SSH config, or user@host. It must log in without a password prompt.".into()),
                    h_flex().gap_2().child(v_flex().flex_1().min_w_0().child(gpui_component::input::Input::new(&self.host_input))).child(connect),
                    cx,
                ))
                .into_any_element(),
        )
    }

    fn server_step(&self, cx: &mut Context<Self>) -> (String, String, AnyElement) {
        let managed = self.config.authentication.managed;
        let description = if managed {
            "Managed sign-in needs this port. Choose your own identity provider to change it."
                .to_owned()
        } else {
            format!("Ricochet will answer at {}.", self.config.local_url())
        };
        (
            "Where should ricochet listen?".into(),
            "The defaults work for most people.".into(),
            v_flex()
                .gap_5()
                .child(field(
                    "Port",
                    Some(description),
                    gpui_component::input::Input::new(&self.inputs["server.port"])
                        .disabled(managed),
                    cx,
                ))
                .child(self.text_field(
                    "License key",
                    "server.license_key",
                    Some("Optional. Leave this empty to use the Community Edition."),
                    cx,
                ))
                .into_any_element(),
        )
    }

    fn review_step(&self, cx: &mut Context<Self>) -> (String, String, AnyElement) {
        let auth = &self.config.authentication;
        let signin = if auth.managed {
            "Managed auth".to_owned()
        } else {
            auth.issuer()
        };
        let license = if self.config.server.license_key.non_empty().is_some() {
            "Your license key"
        } else {
            "Community Edition"
        };
        let rows = [
            ("Computer", self.server.read(cx).connection.name()),
            ("Sign-in", signin),
            ("Address", self.config.local_url()),
            ("License", license.to_owned()),
        ];
        let count = rows.len();
        (
            "Ready to install".into(),
            "Ricochet downloads the server, then starts it in the background.".into(),
            v_flex()
                .border_1()
                .border_color(cx.theme().border)
                .children(rows.into_iter().enumerate().map(|(i, (label, value))| {
                    h_flex()
                        .justify_between()
                        .gap_4()
                        .px_4()
                        .py_3()
                        .when(i + 1 < count, |row| {
                            row.border_b_1().border_color(cx.theme().border)
                        })
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(label),
                        )
                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(value))
                }))
                .child(
                    h_flex()
                        .when(!self.server.read(cx).connection.is_remote(), |row| {
                            row.hidden()
                        })
                        .gap_4()
                        .items_start()
                        .justify_between()
                        .pt_4()
                        .child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("Keep running after I log out"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(
                                            "Turns on lingering for your account on that computer.",
                                        ),
                                ),
                        )
                        .child(
                            gpui_component::switch::Switch::new("keep-running")
                                .checked(self.keep_running)
                                .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                    this.keep_running = *checked;
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element(),
        )
    }

    fn installing_step(&self, cx: &mut Context<Self>) -> (String, String, AnyElement) {
        let server = self.server.read(cx);
        let activity = server.activity.clone();
        let failure = server.failure.clone();
        let current = activity.as_deref().map(stage_of);
        let list = v_flex()
            .gap_3()
            .children(INSTALL_STAGES.iter().enumerate().map(|(i, label)| {
                let (marker, muted) = match (current, &failure) {
                    (Some(c), _) if i < c => (
                        ph("check-circle")
                            .size_5()
                            .text_color(cx.theme().success)
                            .into_any_element(),
                        false,
                    ),
                    (Some(c), _) if i == c => (Spinner::new().into_any_element(), false),
                    _ => (
                        div()
                            .size(px(10.0))
                            .m(px(5.0))
                            .bg(cx.theme().border)
                            .into_any_element(),
                        true,
                    ),
                };
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .size(px(20.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(marker),
                    )
                    .child(
                        div()
                            .text_color(if muted {
                                cx.theme().muted_foreground
                            } else {
                                cx.theme().foreground
                            })
                            .child(*label),
                    )
            }));
        let alert = failure.map(|failure| {
            let server = self.server.clone();
            error_alert(
                &failure.title,
                &failure.message,
                move |_, _, cx| server.update(cx, |server, cx| server.dismiss_failure(cx)),
                cx,
            )
        });
        (
            if alert.is_some() {
                "That didn't work".to_owned()
            } else {
                "Installing ricochet".to_owned()
            },
            if alert.is_some() {
                "Fix what the message says, then try again.".to_owned()
            } else {
                "This usually takes under a minute.".to_owned()
            },
            v_flex()
                .gap_6()
                .child(list)
                .children(alert)
                .into_any_element(),
        )
    }

    fn done_step(&self, cx: &mut Context<Self>) -> (String, String, AnyElement) {
        (
            "Ricochet is running".into(),
            format!("Open it at {}.", self.server.read(cx).dashboard()),
            h_flex()
                .gap_3()
                .items_center()
                .p_4()
                .border_1()
                .border_color(cx.theme().success)
                .child(ph("check-circle").size_6().text_color(cx.theme().success))
                .child(
                    div()
                        .text_sm()
                        .child("Everything is set up. It starts again whenever you log in."),
                )
                .into_any_element(),
        )
    }

    fn onboarding_footer(&self, step: Step, cx: &mut Context<Self>) -> Div {
        let busy = self.server.read(cx).activity.is_some();
        let back = |to: Step, cx: &mut Context<Self>| {
            Button::new("wizard-back")
                .icon(ph("arrow-left"))
                .label("Back")
                .ghost()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.onboarding = Some(to);
                    this.problem = None;
                    cx.notify();
                }))
        };
        let next = |label: &'static str,
                    icon: &'static str,
                    to: Step,
                    problem: Option<String>,
                    cx: &mut Context<Self>| {
            Button::new("wizard-next")
                .label(label)
                .icon(ph(icon))
                .primary()
                .on_click(cx.listener(move |this, _, _, cx| match &problem {
                    Some(problem) => {
                        this.problem = Some(problem.clone());
                        cx.notify();
                    }
                    None => {
                        this.onboarding = Some(to);
                        this.problem = None;
                        cx.notify();
                    }
                }))
        };
        let c = &self.config;
        let (left, right): (Option<AnyElement>, Option<AnyElement>) = match step {
            Step::Welcome => (
                Some(
                    Button::new("advanced")
                        .label("Advanced setup")
                        .ghost()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.onboarding = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                ),
                Some(next("Get started", "arrow-right", Step::Target, None, cx).into_any_element()),
            ),
            Step::Target => (Some(back(Step::Welcome, cx).into_any_element()), None),
            Step::SignIn => (
                Some(back(Step::Target, cx).into_any_element()),
                Some(
                    next(
                        "Continue",
                        "arrow-right",
                        Step::Server,
                        c.authentication.problem(),
                        cx,
                    )
                    .into_any_element(),
                ),
            ),
            Step::Server => (
                Some(back(Step::SignIn, cx).into_any_element()),
                Some(
                    next(
                        "Continue",
                        "arrow-right",
                        Step::Review,
                        c.server.problem(),
                        cx,
                    )
                    .into_any_element(),
                ),
            ),
            Step::Review => (
                Some(back(Step::Server, cx).into_any_element()),
                Some(
                    Button::new("wizard-install")
                        .label(if self.server.read(cx).installed.is_some() {
                            "Save and start"
                        } else {
                            "Install and start"
                        })
                        .icon(ph("rocket-launch"))
                        .primary()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.onboarding = Some(Step::Installing);
                            this.submit(cx);
                        }))
                        .into_any_element(),
                ),
            ),
            Step::Installing => (
                (!busy).then(|| back(Step::Review, cx).into_any_element()),
                (!busy).then(|| {
                    Button::new("wizard-retry")
                        .label("Try again")
                        .icon(ph("arrow-clockwise"))
                        .primary()
                        .on_click(cx.listener(|this, _, _, cx| this.submit(cx)))
                        .into_any_element()
                }),
            ),
            Step::Done => {
                let server = self.server.clone();
                (
                    Some(
                        Button::new("wizard-finish")
                            .label("Go to settings")
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.onboarding = None;
                                cx.notify();
                            }))
                            .into_any_element(),
                    ),
                    Some(
                        Button::new("wizard-open")
                            .label("Open ricochet")
                            .icon(ph("arrow-square-out"))
                            .primary()
                            .on_click(move |_, _, cx| {
                                server.update(cx, |server, cx| server.open_dashboard(cx))
                            })
                            .into_any_element(),
                    ),
                )
            }
        };
        let problem = self.problem.as_ref().map(|problem| {
            error_alert(
                "Check your answers",
                problem,
                cx.listener(|this, _, _, cx| {
                    this.problem = None;
                    cx.notify();
                }),
                cx,
            )
        });
        v_flex().border_t_1().border_color(cx.theme().border).child(
            h_flex().w_full().justify_center().px_6().py_4().child(
                v_flex()
                    .w_full()
                    .max_w(px(if step == Step::SignIn { 640.0 } else { 480.0 }))
                    .gap_3()
                    .children(problem)
                    .child(
                        h_flex()
                            .justify_between()
                            .child(left.unwrap_or_else(|| div().into_any_element()))
                            .child(right.unwrap_or_else(|| div().into_any_element())),
                    ),
            ),
        )
    }
}
