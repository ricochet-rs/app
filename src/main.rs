#![cfg_attr(windows, windows_subsystem = "windows")]

mod api;
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

            let bounds = Bounds::centered(None, size(px(900.0), px(680.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(680.0), px(480.0))),
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
