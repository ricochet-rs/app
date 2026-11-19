//! The window: ricochet's content, and the setup that installs and manages a server.

use super::components::{rule, tab_bar};
use super::dashboard::DashboardView;
use super::setup::{OpenDashboard, SetupView};
use crate::controller::ServerController;
use gpui::prelude::*;
use gpui::{Context, Entity, Subscription, Window};
use gpui_component::{ActiveTheme, h_flex, v_flex};

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Content,
    Manage,
}

pub struct Workspace {
    mode: Mode,
    setup: Entity<SetupView>,
    dashboard: Entity<DashboardView>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(
        server: Entity<ServerController>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let setup = cx.new(|cx| SetupView::new(server.clone(), window, cx));
        let dashboard = cx.new(|cx| DashboardView::new(server, window, cx));
        let mode = if dashboard.read(cx).has_servers(cx) {
            Mode::Content
        } else {
            Mode::Manage
        };
        let subscriptions = vec![cx.subscribe(&setup, |this, _, _: &OpenDashboard, cx| {
            this.mode = Mode::Content;
            cx.notify();
        })];
        Self {
            mode,
            setup,
            dashboard,
            _subscriptions: subscriptions,
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = tab_bar(
            "workspace-tabs",
            vec![
                ("Content", self.mode == Mode::Content),
                ("Manage", self.mode == Mode::Manage),
            ],
            {
                let workspace = cx.entity();
                move |index, _, cx| {
                    workspace.update(cx, |this, cx| {
                        this.mode = if index == 0 {
                            Mode::Content
                        } else {
                            Mode::Manage
                        };
                        cx.notify();
                    })
                }
            },
        );
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(h_flex().px_4().pt_1().child(tabs))
            .child(rule(cx))
            .child(v_flex().flex_1().min_h_0().map(|body| match self.mode {
                Mode::Content => body.child(self.dashboard.clone()),
                Mode::Manage => body.child(self.setup.clone()),
            }))
    }
}
