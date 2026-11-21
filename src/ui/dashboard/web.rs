//! Shows a server's own web UI in the system's webview: WKWebView on macOS and WebView2 on Windows.
//! You sign in there with a browser session, so every page of the web UI works without an API key.

use super::{Content, DashboardView};
use crate::servers::Server;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, Window, div};
use gpui_component::webview::WebView;
use gpui_component::wry::{NewWindowResponse, WebContext, WebViewBuilder};

pub struct Web {
    /// Keeps cookies and storage in the app's folder, because WebView2 can't write next to the program.
    context: WebContext,
    view: Option<Entity<WebView>>,
    /// The server address the webview loaded last.
    loaded: Option<String>,
}

impl Default for Web {
    fn default() -> Self {
        let folder = dirs::data_local_dir().map(|dir| dir.join("ricochet-app/webview"));
        Self {
            context: WebContext::new(folder),
            view: None,
            loaded: None,
        }
    }
}

impl Web {
    pub fn hide(&self, cx: &mut gpui::App) {
        if let Some(view) = &self.view
            && view.read(cx).visible()
        {
            view.update(cx, |view, _| view.hide());
        }
    }
}

impl DashboardView {
    /// Opens the tunnel to a managed server if it needs one, then shows the server's web UI.
    pub(super) fn show_web(&mut self, server: Server, cx: &mut Context<Self>) {
        self.content = Content::Loading;
        cx.notify();
        let generation = self.load_generation;
        let task = self.reach(&server, cx, || Ok(()));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.load_generation != generation {
                    return;
                }
                this.content = match result {
                    Ok(()) => Content::Web(server.url),
                    Err(error) => Content::Failed(error.to_string()),
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn web_page(
        &mut self,
        url: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view = match &self.web.view {
            Some(view) => {
                if self.web.loaded.as_ref() != Some(&url) {
                    view.update(cx, |view, _| view.load_url(&url));
                }
                view.clone()
            }
            None => {
                let built = WebViewBuilder::new_with_web_context(&mut self.web.context)
                    .with_url(&url)
                    .with_new_window_req_handler(|url, _| {
                        // Content on the server can ask for any URL, so only web pages reach the browser.
                        let lower = url.to_ascii_lowercase();
                        if lower.starts_with("https://") || lower.starts_with("http://") {
                            let _ = open::that(&url);
                        }
                        NewWindowResponse::Deny
                    })
                    .build_as_child(window);
                match built {
                    Ok(built) => {
                        let view = cx.new(|cx| WebView::new(built, window, cx));
                        self.web.view = Some(view.clone());
                        view
                    }
                    Err(error) => {
                        let message = format!("The web view didn't start: {error}");
                        return self.centered(div().child(message), cx).into_any_element();
                    }
                }
            }
        };
        self.web.loaded = Some(url);
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
