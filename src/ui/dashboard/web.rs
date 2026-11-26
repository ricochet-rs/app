//! Shows a server's own web UI in the system's webview: WKWebView on macOS, WebView2 on Windows and WebKitGTK on Linux.
//! You sign in there with a browser session, so every page of the web UI works without an API key.

use super::DashboardView;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, Window, div};
use gpui_webview::WebView;
use wry::{NewWindowResponse, ProxyConfig, ProxyEndpoint, WebContext, WebViewBuilder};

/// One webview, with the folder it keeps cookies and storage in.
struct Slot {
    /// Keeps cookies and storage in the app's folder, because WebView2 can't write next to the program.
    context: WebContext,
    view: Option<Entity<WebView>>,
    /// The server address the webview loaded last.
    loaded: Option<String>,
    /// The proxy host and port the webview was built with.
    proxy: Option<(String, String)>,
}

impl Slot {
    fn new(folder: &str) -> Self {
        let folder = dirs::data_local_dir().map(|dir| dir.join("ricochet-app").join(folder));
        Self {
            context: WebContext::new(folder),
            view: None,
            loaded: None,
            proxy: None,
        }
    }

    fn hide(&self, cx: &mut gpui::App) {
        if let Some(view) = &self.view
            && view.read(cx).visible()
        {
            view.update(cx, |view, _| view.hide());
        }
    }
}

/// The web UI's webviews: one that connects directly, and one that goes through the proxy you entered.
///
/// They are separate because WebView2 takes a proxy only when its environment starts,
/// and WKWebView and WebKitGTK keep it in the data store every webview in it shares,
/// while ricochet on `localhost` must never go through the proxy.
pub struct Web {
    direct: Slot,
    proxied: Slot,
}

impl Default for Web {
    fn default() -> Self {
        Self {
            direct: Slot::new("webview"),
            proxied: Slot::new("webview-proxy"),
        }
    }
}

impl Web {
    pub fn hide(&self, cx: &mut gpui::App) {
        self.direct.hide(cx);
        self.proxied.hide(cx);
    }
}

/// The data store the proxied webview keeps on macOS, apart from the default one the direct webview uses.
#[cfg(target_os = "macos")]
const PROXIED_STORE: [u8; 16] = *b"ricochet-proxied";

impl DashboardView {
    pub(super) fn web_page(
        &mut self,
        url: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let proxy = crate::settings::webview_proxy(&url)
            .and_then(|proxy| crate::settings::endpoint(&proxy));
        let (slot, other) = if proxy.is_some() {
            (&mut self.web.proxied, &self.web.direct)
        } else {
            (&mut self.web.direct, &self.web.proxied)
        };
        other.hide(cx);
        if slot.view.is_some() && slot.proxy != proxy {
            // The proxy changed, so the webview starts again with the new one.
            slot.view = None;
            slot.loaded = None;
        }
        let view = match &slot.view {
            Some(view) => {
                if slot.loaded.as_ref() != Some(&url) {
                    view.update(cx, |view, _| view.load_url(&url));
                }
                Ok(view.clone())
            }
            None => {
                let mut builder = WebViewBuilder::new_with_web_context(&mut slot.context)
                    .with_url(&url)
                    .with_new_window_req_handler(|url, _| {
                        // Content on the server can ask for any URL, so only web pages reach the browser.
                        let lower = url.to_ascii_lowercase();
                        if lower.starts_with("https://") || lower.starts_with("http://") {
                            let _ = open::that(&url);
                        }
                        NewWindowResponse::Deny
                    });
                if let Some((host, port)) = proxy.clone() {
                    builder =
                        builder.with_proxy_config(ProxyConfig::Http(ProxyEndpoint { host, port }));
                    #[cfg(target_os = "macos")]
                    {
                        use wry::WebViewBuilderExtDarwin;
                        builder = builder.with_data_store_identifier(PROXIED_STORE);
                    }
                }
                WebView::build(builder, window, cx).map(|built| {
                    let view = cx.new(|_| built);
                    slot.view = Some(view.clone());
                    slot.proxy = proxy.clone();
                    view
                })
            }
        };
        let view = match view {
            Ok(view) => view,
            Err(error) => {
                let message = format!("The web view didn't start: {error}");
                return self.centered(div().child(message), cx).into_any_element();
            }
        };
        if proxy.is_some() {
            self.web.proxied.loaded = Some(url);
        } else {
            self.web.direct.loaded = Some(url);
        }
        if self.covered {
            self.hide_web(cx);
        } else if !view.read(cx).visible() {
            view.update(cx, |view, _| view.show());
        }
        div()
            .flex_1()
            .min_h_0()
            .size_full()
            .child(view)
            .into_any_element()
    }
}

/// WebKitGTK on Linux, which wry embeds only in an X11 window.
#[cfg(target_os = "linux")]
pub mod linux {
    /// Runs the app on X11, through XWayland under Wayland, and starts GTK, before GPUI opens a window.
    /// Call it first in `main`, while the app has a single thread.
    pub fn prepare() {
        if std::env::var_os("DISPLAY").is_some() {
            // SAFETY: nothing else runs yet, so no other thread reads the environment.
            unsafe {
                std::env::remove_var("WAYLAND_DISPLAY");
                std::env::set_var("GDK_BACKEND", "x11");
            }
        }
        if let Err(error) = gtk::init() {
            eprintln!("GTK didn't start, so the web UI can't show: {error}");
        }
    }
}
