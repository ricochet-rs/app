#![cfg_attr(windows, windows_subsystem = "windows")]

mod archive;
mod assets;
mod backend;
mod command;
mod config;
mod connections;
mod controller;
mod error;
mod paths;
mod person;
mod ports;
mod release;
mod servers;
mod service;
mod shell;
mod ui;

use controller::ServerController;
use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, Menu, MenuItem, WindowBounds, WindowOptions,
    actions, px, size,
};
use gpui_component::Root;
use ui::setup::SetupView;

actions!(ricochet, [Quit]);

fn main() {
    #[cfg(target_os = "linux")]
    ui::dashboard::linux_webview::prepare();
    Application::new()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            gpui_component::init(cx);
            ui::theme::apply(None, cx);

            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-q"
                } else {
                    "ctrl-q"
                },
                Quit,
                None,
            )]);
            cx.set_menus(vec![Menu {
                name: "ricochet".into(),
                items: vec![MenuItem::action("Quit ricochet", Quit)],
            }]);

            let bounds = Bounds::centered(None, size(px(1120.0), px(780.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(860.0), px(560.0))),
                // On macOS and Windows the app draws its own title bar, with its controls beside the window buttons.
                titlebar: if cfg!(any(target_os = "macos", target_os = "windows")) {
                    Some(gpui::TitlebarOptions {
                        title: Some("ricochet".into()),
                        ..gpui_component::TitleBar::title_bar_options()
                    })
                } else {
                    Some(gpui::TitlebarOptions {
                        title: Some("ricochet".into()),
                        ..Default::default()
                    })
                },
                ..Default::default()
            };
            cx.open_window(options, |window, cx| {
                ui::theme::apply(Some(window), cx);
                window
                    .observe_window_appearance(|window, cx| ui::theme::apply(Some(window), cx))
                    .detach();
                let server = cx.new(ServerController::new);
                let view = cx.new(|cx| SetupView::new(server, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("ricochet could not open its window");
            cx.activate(true);
        });
}
