//! The app's own settings, which apply to every instance: the proxy it reaches servers and downloads through.

use super::SetupView;
use crate::config::section::{Field as _, SettingsSection as _};
use crate::config::{ProxyMode, ProxySettings};
use crate::ui::components::{card, card_title, field, heading, prose};
use crate::ui::icons::ph;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, Subscription, Window, div};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme, Sizable, h_flex, v_flex};

/// The fields of the app settings page while it is open.
pub struct AppSettingsInputs {
    url: Entity<InputState>,
    bypass: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl SetupView {
    pub(super) fn open_app_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        self.instance_inputs = None;
        self.instance_subscriptions.clear();
        self.app_settings = Some(AppSettingsInputs {
            url,
            bypass,
            _subscriptions: subscriptions,
        });
        cx.notify();
    }

    pub(super) fn close_app_settings(&mut self, cx: &mut Context<Self>) {
        self.app_settings = None;
        cx.notify();
    }

    /// Saves what the page shows, with `mode` when you picked a new one.
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

    pub(super) fn app_settings_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(inputs) = &self.app_settings else {
            return div().into_any_element();
        };
        let settings = crate::settings::proxy_settings();
        let mode = settings.mode;
        let system_hint = if cfg!(target_os = "macos") {
            "The app follows the proxy in System Settings under Network, including a proxy configuration file (PAC) for the web UI."
        } else if cfg!(windows) {
            "The app follows the proxy in Windows Settings under Network & internet, including a setup script (PAC) for the web UI."
        } else {
            "The app follows your desktop's proxy settings for the web UI, and `HTTPS_PROXY` for downloads."
        };
        let modes = h_flex()
            .gap_1()
            .children(ProxyMode::ALL.iter().map(|&choice| {
                let button = Button::new(choice.raw()).label(choice.title()).small();
                let button = if choice == mode {
                    button.primary()
                } else {
                    button.outline()
                };
                button.on_click(
                    cx.listener(move |this, _, _, cx| this.save_app_proxy(Some(choice), cx)),
                )
            }));
        let problem = settings.problem();
        let proxy = card(cx)
            .child(card_title("Network proxy", cx))
            .child(field(
                "Proxy",
                Some(
                    match mode {
                        ProxyMode::System => system_hint,
                        ProxyMode::Manual => "The app reaches servers and downloads ricochet through this proxy. Servers on `localhost` are always reached directly.",
                        ProxyMode::Off => "The app downloads ricochet without a proxy. The web UI still follows your system's proxy settings.",
                    }
                    .to_owned(),
                ),
                modes,
                cx,
            ))
            .when(mode == ProxyMode::Manual, |card| {
                card.child(field(
                    "Proxy address",
                    Some("The proxy's URL with its port, like http://proxy.example.com:8080. The web UI can't sign in to a proxy, so a user name and password only reach downloads.".into()),
                    Input::new(&inputs.url),
                    cx,
                ))
                .child(field(
                    "Bypass the proxy for",
                    Some("Hosts to reach directly, separated by commas.".into()),
                    Input::new(&inputs.bypass),
                    cx,
                ))
                .children(problem.filter(|_| !settings.url.trimmed().is_empty()).map(|problem| {
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(prose(problem))
                }))
            });
        div()
            .id("app-settings-page")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .gap_5()
                    .p_6()
                    .max_w(gpui::px(760.0))
                    .child(heading(
                        "App settings",
                        Some("Settings for the app itself, which apply to every instance. Each machine the app manages also has a proxy for ricochet under Server in its own settings."),
                        cx,
                    ))
                    .child(proxy)
                    .child(
                        card(cx).child(card_title("Instances", cx)).child(
                            h_flex()
                                .gap_3()
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(prose("Name, group and order the instances in the server menu.")),
                                )
                                .child(
                                    Button::new("app-manage-instances")
                                        .icon(ph("sliders-horizontal"))
                                        .label("Manage instances")
                                        .outline()
                                        .small()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.app_settings = None;
                                            this.open_instances(window, cx);
                                        })),
                                ),
                        ),
                    ),
            )
            .into_any_element()
    }
}
