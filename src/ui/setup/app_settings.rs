//! The App settings dialog, for settings of the app itself rather than of an instance: the proxy it reaches servers and downloads through.

use super::SetupView;
use crate::config::section::{Field as _, SettingsSection as _};
use crate::config::{ProxyMode, ProxySettings};
use crate::ui::components::{card_title, field, prose, segmented};
use crate::ui::icons::ph;
use gpui::prelude::*;
use gpui::{Context, Entity, Subscription, Window, div, px};
use gpui_component::WindowExt as _;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::dialog::{DialogAction, DialogFooter};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme, h_flex, v_flex};

/// The fields of the App settings dialog while it is open.
pub struct AppSettingsInputs {
    _subscriptions: Vec<Subscription>,
    url: Entity<InputState>,
    bypass: Entity<InputState>,
}

impl SetupView {
    pub(super) fn open_app_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.app_settings.is_some() {
            return;
        }
        let saved = crate::settings::proxy_settings();
        let input =
            |value: String, placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
                let placeholder = placeholder.to_owned();
                cx.new(|cx| {
                    let mut input = InputState::new(window, cx).placeholder(placeholder);
                    input.set_value(value, window, cx);
                    input
                })
            };
        let url = input(saved.url, "http://proxy.example.com:8080", window, cx);
        let bypass = input(saved.bypass, ".corp.example.com, intranet", window, cx);
        let subscriptions = [url.clone(), bypass.clone()]
            .into_iter()
            .map(|field| {
                cx.subscribe_in(&field, window, |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.save_app_proxy(None, cx);
                    }
                })
            })
            .collect();
        self.app_settings = Some(AppSettingsInputs {
            _subscriptions: subscriptions,
            url: url.clone(),
            bypass: bypass.clone(),
        });
        let view = cx.entity();
        let closed = view.downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let closed = closed.clone();
            dialog
                .title(prose("App settings"))
                .w(px(560.0))
                .child(proxy_settings(&view, &url, &bypass, cx))
                .footer(
                    DialogFooter::new().child(
                        DialogAction::new().child(Button::new("done").primary().label("Done")),
                    ),
                )
                .on_ok(|_, _, _| true)
                .on_close(move |_, _, cx| {
                    if let Some(view) = closed.upgrade() {
                        view.update(cx, |this, cx| {
                            this.app_settings = None;
                            cx.notify();
                        });
                    }
                })
        });
        cx.notify();
    }

    pub(super) fn close_app_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.app_settings = None;
        window.close_dialog(cx);
        cx.notify();
    }

    /// Saves what the dialog shows, with `mode` when you chose a new one.
    fn save_app_proxy(&mut self, mode: Option<ProxyMode>, cx: &mut Context<Self>) {
        let Some(inputs) = &self.app_settings else {
            return;
        };
        let settings = ProxySettings {
            mode: mode.unwrap_or_else(|| crate::settings::proxy_settings().mode),
            url: inputs.url.read(cx).value().to_string(),
            bypass: inputs.bypass.read(cx).value().to_string(),
        };
        // The web UI picks up the new proxy the next time it shows.
        crate::settings::set_proxy_settings(settings);
        cx.notify();
    }
}

/// The Network proxy section of the dialog, which reads the saved settings because the view is mid-render.
fn proxy_settings(
    view: &Entity<SetupView>,
    url: &Entity<InputState>,
    bypass: &Entity<InputState>,
    cx: &mut gpui::App,
) -> impl IntoElement {
    let settings = crate::settings::proxy_settings();
    let mode = settings.mode;
    let system_hint = if cfg!(target_os = "macos") {
        "The app follows the proxy in System Settings under Network, including a proxy configuration file (PAC) for the web UI."
    } else if cfg!(windows) {
        "The app follows the proxy in Windows Settings under Network & internet, including a setup script (PAC) for the web UI."
    } else {
        "The app follows your desktop's proxy settings for the web UI, and `HTTPS_PROXY` for downloads."
    };
    let modes = segmented(
        "app-proxy-mode",
        ProxyMode::ALL
            .iter()
            .map(|choice| (choice.title(), *choice == mode))
            .collect(),
        {
            let view = view.clone();
            move |index, _, cx| {
                view.update(cx, |this, cx| {
                    this.save_app_proxy(Some(ProxyMode::ALL[index]), cx)
                })
            }
        },
    );
    let problem = settings
        .problem()
        .filter(|_| mode == ProxyMode::Manual && !settings.url.trimmed().is_empty());
    let muted = cx.theme().muted_foreground;
    v_flex()
        .gap_5()
        .child(
            v_flex()
                .gap_1p5()
                .child(card_title("Network proxy", cx))
                .child(div().text_sm().text_color(muted).child(prose(
                    "Only needed when your network sends internet traffic through a proxy, as many company networks do. The app uses it to download ricochet and to reach servers you connect to by address.",
                ))),
        )
        .child(field(
            "Proxy",
            Some(
                match mode {
                    ProxyMode::System => system_hint,
                    ProxyMode::Manual => "The app reaches servers and downloads ricochet through the proxy below.",
                    ProxyMode::Off => "The app downloads ricochet without a proxy. The web UI still follows your system's proxy settings.",
                }
                .to_owned(),
            ),
            modes,
            cx,
        ))
        .when(mode == ProxyMode::Manual, |section| {
            section
                .child(field(
                    "Proxy address",
                    Some("The proxy's URL with its port. A user name and password reach downloads only, because the web UI can't sign in to a proxy.".into()),
                    Input::new(url),
                    cx,
                ))
                .child(field(
                    "Reach these hosts directly",
                    Some("Separate hosts with commas. Servers on this computer never go through the proxy.".into()),
                    Input::new(bypass),
                    cx,
                ))
        })
        .children(problem.map(|problem| {
            div()
                .text_sm()
                .text_color(cx.theme().danger)
                .child(prose(problem))
        }))
        .child(
            h_flex()
                .gap_2()
                .p_3()
                .items_start()
                .bg(cx.theme().muted)
                .child(ph("info").size_4().text_color(muted))
                .child(div().flex_1().text_xs().text_color(muted).child(prose(
                    "Each machine the app manages has a proxy of its own for ricochet, under Server in its settings.",
                ))),
        )
}
