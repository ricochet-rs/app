#![cfg_attr(windows, windows_subsystem = "windows")]

mod archive;
mod assets;
mod backend;
mod command;
mod config;
mod connections;
mod controller;
mod error;
mod net;
mod paths;
mod person;
mod ports;
mod release;
mod servers;
mod service;
mod settings;
mod shell;
mod ui;
mod updater;

use controller::ServerController;
use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, Menu, MenuItem, WindowBounds, WindowOptions,
    actions, px, size,
};
use gpui_component::Root;
use ui::setup::SetupView;

actions!(ricochet, [Quit, ToggleSettings]);

fn main() {
    // First, while the app has a single thread, because it sets environment variables.
    #[cfg(target_os = "linux")]
    ui::dashboard::linux_webview::prepare();
    updater::clean_up();
    Application::new()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            gpui_component::init(cx);
            ui::theme::apply(None, cx);

            cx.on_action(|_: &Quit, cx| cx.quit());
            let (quit, settings) = if cfg!(target_os = "macos") {
                ("cmd-q", "cmd-,")
            } else {
                ("ctrl-q", "ctrl-,")
            };
            cx.bind_keys([
                KeyBinding::new(quit, Quit, None),
                KeyBinding::new(settings, ToggleSettings, None),
            ]);
            cx.set_menus(vec![Menu {
                name: "ricochet".into(),
                items: vec![
                    MenuItem::action("Settings…", ToggleSettings),
                    MenuItem::separator(),
                    MenuItem::action("Quit ricochet", Quit),
                ],
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
                // The shortcut and the menu item reach the window wherever the focus is.
                // GPUI runs this while it holds the window, so the window is updated once it lets go.
                let (settings, handle) = (view.downgrade(), window.window_handle());
                cx.on_action(move |_: &ToggleSettings, cx| {
                    let settings = settings.clone();
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, cx| {
                            settings.update(cx, |view, cx| view.toggle_settings(window, cx))
                        });
                    });
                });
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("ricochet could not open its window");
            cx.activate(true);
        });
}
