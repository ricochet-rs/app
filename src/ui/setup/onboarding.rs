//! First-run setup: one decision per screen, then a live install checklist.

use super::{SetupView, Step};
use crate::config::section::{Field as _, SettingsSection as _};
use crate::ui::components::{error_alert, field, prose};
use crate::ui::icons::ph;
use crate::ui::theme::Palette;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Div, FontWeight, div, px};
use gpui_component::Disableable as _;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::spinner::Spinner;
use gpui_component::{ActiveTheme, h_flex, v_flex};

const INSTALL_STAGES: [&str; 4] = [
    "Checking your settings",
    "Downloading ricochet",
    "Writing your configuration",
    "Starting ricochet",
];

const STEPS: usize = 4;
const COMMUNITY_EDITION_DOCS: &str =
    "https://docs.ricochet.rs/latest/admin/pricing/5-community-edition/";
const MACHINE_ROW_HEIGHT: f32 = 60.0;
const VISIBLE_MACHINES: usize = 5;

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
            Self::License => Some(3),
            Self::Review => Some(4),
            _ => None,
        }
    }

    fn width(self) -> f32 {
        if self == Self::SignIn { 680.0 } else { 540.0 }
    }
}

impl SetupView {
    pub(super) fn onboarding_view(&mut self, step: Step, cx: &mut Context<Self>) -> AnyElement {
        let (title, subtitle, body) = match step {
            Step::Welcome => self.welcome(cx),
            Step::Target => self.target_step(cx),
            Step::SignIn => (
                "Configure authentication".into(),
                "You can change your identity provider later.".into(),
                self.authentication_pane(cx),
            ),
            Step::License => self.license_step(cx),
            Step::Review => self.review_step(cx),
            Step::Installing => self.installing_step(cx),
            Step::Done => self.done_step(cx),
        };
        let subtitle_text = if step == Step::License {
            prose(subtitle)
                .link("Community Edition", COMMUNITY_EDITION_DOCS)
                .link_color(cx.theme().foreground)
        } else {
            prose(subtitle)
        };
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                        div()
                            .id("onboarding")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.page_scroll)
                            .child(
                                h_flex()
                                    .w_full()
                                    .justify_center()
                                    .px_8()
                                    .pt_12()
                                    .pb_10()
                                    .child(
                                        v_flex()
                                            .w_full()
                                            .max_w(px(step.width()))
                                            .gap_8()
                                            .child(self.progress(step, cx))
                                            .child(
                                                v_flex()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .text_2xl()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .child(prose(title)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(subtitle_text),
                                                    ),
                                            )
                                            .child(body),
                                    ),
                            ),
                    )
                    .vertical_scrollbar(&self.page_scroll),
            )
            .child(self.onboarding_footer(step, cx))
            .into_any_element()
    }

    /// A step count over four bars that fill as the person moves through the choices.
    fn progress(&self, step: Step, cx: &Context<Self>) -> Div {
        let reached = step.number().unwrap_or(match step {
            Step::Welcome => 0,
            _ => STEPS,
        });
        v_flex()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(cx.theme().muted_foreground)
                    .child(match step.number() {
                        Some(n) => format!("Step {n} of {STEPS}"),
                        None if reached == 0 => "Setup".to_owned(),
                        None => "Install".to_owned(),
                    }),
            )
            .child(h_flex().gap_1p5().children((1..=STEPS).map(|n| {
                div().flex_1().h(px(3.0)).bg(if n <= reached {
                    cx.theme().ring
                } else {
                    cx.theme().border
                })
            })))
    }

    /// Shown while the app logs in to the machine it managed last time.
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
                                .child(prose(text)),
                        ),
                )
        };
        (
            "Welcome to ricochet".into(),
            "Install ricochet or manage an existing server.".into(),
            v_flex()
                .gap_5()
                .child(row(
                    "desktop",
                    "Your machine or a server",
                    "Run ricochet locally or on any Linux server you can SSH into.",
                ))
                .child(row(
                    "power",
                    "Always running",
                    "ricochet runs in the background and starts back up on its own.",
                ))
                .child(row(
                    "sliders-horizontal",
                    "Everything in one place",
                    "Manage authentication, packages, logging, and users right from this app.",
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
        let count = choices.len();
        let ring = cx.theme().ring;
        let hover = cx.theme().muted;
        let rows = choices.into_iter().enumerate().map(|(index, choice)| {
            let selected = choice == active && !busy;
            let this_one = connecting
                .as_deref()
                .is_some_and(|a| a.ends_with(&choice.name()));
            let remote = choice.is_remote();
            let (icon, detail) = if remote {
                ("hard-drives", "Linux over SSH")
            } else {
                ("desktop", "Local")
            };
            let marker: AnyElement = if this_one {
                Spinner::new().into_any_element()
            } else if selected {
                ph("check-circle")
                    .size_5()
                    .text_color(ring)
                    .into_any_element()
            } else {
                ph("arrow-right")
                    .size_4()
                    .text_color(cx.theme().muted_foreground)
                    .into_any_element()
            };
            let target = choice.clone();
            h_flex()
                .id(("target", index))
                .w_full()
                .h(px(MACHINE_ROW_HEIGHT))
                .flex_shrink_0()
                .gap_3()
                .px_4()
                .items_center()
                .when(index + 1 < count, |row| {
                    row.border_b_1().border_color(cx.theme().border)
                })
                .when(selected, |row| row.bg(ring.opacity(0.07)))
                .when(!busy && !selected, |row| {
                    row.cursor_pointer().hover(move |row| row.bg(hover))
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.server.read(cx).activity.is_none() {
                        let target = target.clone();
                        this.server
                            .update(cx, |server, cx| server.switch(target, cx));
                    }
                }))
                .child(
                    div()
                        .size(px(32.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(if selected {
                            ring.opacity(0.16)
                        } else {
                            cx.theme().muted
                        })
                        .child(ph(icon).size_4().text_color(if selected {
                            ring
                        } else {
                            cx.theme().foreground
                        })),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .when(remote, |name| {
                                    name.font_family(cx.theme().mono_font_family.clone())
                                })
                                .truncate()
                                .child(choice.name()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(detail),
                        ),
                )
                .child(marker)
        });
        let list_height = MACHINE_ROW_HEIGHT * count.min(VISIBLE_MACHINES) as f32;
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
            "Install location".into(),
            "Choose where to install ricochet. Either on this machine or one accessible via SSH."
                .into(),
            v_flex()
                .gap_6()
                .children(alert)
                .child(
                    v_flex()
                        .relative()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(Palette::card(cx))
                        .child(
                            v_flex()
                                .id("machines")
                                .h(px(list_height))
                                .overflow_y_scroll()
                                .track_scroll(&self.machine_scroll)
                                .children(rows),
                        )
                        .vertical_scrollbar(&self.machine_scroll),
                )
                .child(field(
                    "Add machine",
                    Some("Enter a host from your SSH config or user@host. You'll need key-based SSH access.".into()),
                    h_flex()
                        .gap_2()
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(gpui_component::input::Input::new(&self.host_input)),
                        )
                        .child(connect),
                    cx,
                ))
                .into_any_element(),
        )
    }

    fn license_step(&self, cx: &mut Context<Self>) -> (String, String, AnyElement) {
        (
            "Have a license?".into(),
            "No worries if not. The `Community Edition` is a great place to start.".into(),
            field(
                "License key",
                None,
                gpui_component::input::Input::new(&self.inputs["server.license_key"]),
                cx,
            )
            .into_any_element(),
        )
    }

    fn review_step(&self, cx: &mut Context<Self>) -> (String, String, AnyElement) {
        let auth = &self.config.authentication;
        let server = self.server.read(cx);
        let remote = server.connection.is_remote();
        let machine = server.connection.name();
        let (auth_icon, signin) = if auth.managed {
            ("shield-check", "Managed".to_owned())
        } else {
            ("key", auth.issuer())
        };
        let license = if self.config.server.license_key.non_empty().is_some() {
            "License key added"
        } else {
            "Community Edition"
        };
        let rows = [
            (
                if remote { "hard-drives" } else { "desktop" },
                "Machine",
                machine.clone(),
            ),
            (auth_icon, "Authentication", signin),
            ("file-text", "License", license.to_owned()),
        ];
        let summary = v_flex()
            .border_1()
            .border_color(cx.theme().border)
            .bg(Palette::card(cx))
            .children(
                rows.into_iter()
                    .enumerate()
                    .map(|(i, (icon, label, value))| {
                        h_flex()
                            .gap_3()
                            .px_4()
                            .py_3()
                            .items_center()
                            .when(i > 0, |row| {
                                row.border_t_1().border_color(cx.theme().border)
                            })
                            .child(ph(icon).size_4().text_color(cx.theme().muted_foreground))
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(label),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(value),
                            )
                    }),
            );
        let keep_running = remote.then(|| {
            h_flex()
                .gap_4()
                .px_4()
                .py_3()
                .items_center()
                .border_1()
                .border_color(cx.theme().border)
                .bg(Palette::card(cx))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child("Keep running after you log out"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(prose(format!(
                                    "Otherwise ricochet stops when you log out of {machine}."
                                ))),
                        ),
                )
                .child(
                    gpui_component::switch::Switch::new("keep-running")
                        .checked(self.keep_running)
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.keep_running = *checked;
                            cx.notify();
                        })),
                )
        });
        (
            "Ready to install".into(),
            "We'll download ricochet and get it running in the background.".into(),
            v_flex()
                .gap_3()
                .child(summary)
                .children(keep_running)
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
                            .child(prose(*label)),
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
                "Something went wrong".to_owned()
            } else {
                "Installing ricochet".to_owned()
            },
            if alert.is_some() {
                "Check the error below and try again.".to_owned()
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
                .pl_4()
                .pr_4()
                .py_3()
                .bg(cx.theme().success.opacity(0.08))
                .border_l_2()
                .border_color(cx.theme().success)
                .child(ph("check-circle").size_5().text_color(cx.theme().success))
                .child(div().text_sm().child(prose(
                    "You're all set. ricochet will keep running in the background.",
                )))
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
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("advanced")
                                .label("Skip to settings")
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.onboarding = None;
                                    this.showing_content = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("connect-to-server")
                                .icon(ph("globe"))
                                .label("Connect to a server")
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| this.add_server(cx))),
                        )
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
                        Step::License,
                        self.sign_in_problem(),
                        cx,
                    )
                    .into_any_element(),
                ),
            ),
            Step::License => (
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
                Some(back(Step::License, cx).into_any_element()),
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
                            .label("Open settings")
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.onboarding = None;
                                cx.notify();
                            }))
                            .into_any_element(),
                    ),
                    Some(
                        Button::new("wizard-open")
                            .icon(ph("arrow-square-out"))
                            .child(prose("Open ricochet"))
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
                "A few settings need attention",
                problem,
                cx.listener(|this, _, _, cx| {
                    this.problem = None;
                    cx.notify();
                }),
                cx,
            )
        });
        v_flex().border_t_1().border_color(cx.theme().border).child(
            h_flex().w_full().justify_center().px_8().py_4().child(
                v_flex()
                    .w_full()
                    .max_w(px(step.width()))
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
