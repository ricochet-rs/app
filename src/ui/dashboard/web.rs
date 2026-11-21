//! Shows a server's own web UI in the system's webview: WKWebView on macOS, WebView2 on Windows and WebKitGTK on Linux.
//! You sign in there with a browser session, so every page of the web UI works without an API key.

use super::DashboardView;
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
    /// Advances GTK's loop, which WebKitGTK needs and GPUI doesn't run.
    #[cfg(target_os = "linux")]
    gtk_loop: Option<gpui::Task<()>>,
}

impl Default for Web {
    fn default() -> Self {
        let folder = dirs::data_local_dir().map(|dir| dir.join("ricochet-app/webview"));
        Self {
            context: WebContext::new(folder),
            view: None,
            loaded: None,
            #[cfg(target_os = "linux")]
            gtk_loop: None,
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
                    .build_as_child(&parent(window));
                match built {
                    Ok(built) => {
                        #[cfg(target_os = "linux")]
                        {
                            self.web.gtk_loop = Some(linux::run_gtk(cx));
                        }
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

#[cfg(not(target_os = "linux"))]
fn parent(window: &Window) -> &Window {
    window
}

#[cfg(target_os = "linux")]
use linux::parent;

/// WebKitGTK on Linux: wry embeds it only in an X11 window, and only through GTK's own loop.
#[cfg(target_os = "linux")]
pub mod linux {
    use gpui::{Context, Task, Window};
    use raw_window_handle::{
        HandleError, HasWindowHandle, RawWindowHandle, WindowHandle, XlibWindowHandle,
    };
    use std::time::Duration;

    /// How often GTK's loop runs while the web UI exists: often enough to scroll smoothly at 60 frames a second.
    const GTK_TICK: Duration = Duration::from_millis(16);

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

    pub fn run_gtk<T: 'static>(cx: &mut Context<T>) -> Task<()> {
        cx.spawn(async move |_, cx| {
            loop {
                while gtk::events_pending() {
                    gtk::main_iteration_do(false);
                }
                cx.background_executor().timer(GTK_TICK).await;
            }
        })
    }

    /// GPUI hands out an XCB handle, and wry takes only an Xlib one; both name the same X window.
    pub struct X11Parent(Option<u32>);

    pub fn parent(window: &Window) -> X11Parent {
        let id = match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
            Ok(RawWindowHandle::Xcb(handle)) => Some(handle.window.get()),
            Ok(RawWindowHandle::Xlib(handle)) => u32::try_from(handle.window).ok(),
            _ => None,
        };
        X11Parent(id)
    }

    impl HasWindowHandle for X11Parent {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let id = self.0.ok_or(HandleError::NotSupported)?;
            let handle = XlibWindowHandle::new(id.into());
            // SAFETY: the X window belongs to GPUI's window, which outlives the webview inside it.
            Ok(unsafe { WindowHandle::borrow_raw(handle.into()) })
        }
    }
}
