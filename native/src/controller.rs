//! Installs ricochet-server on a computer, starts and stops it, and reports whether it answers.

use crate::backend::{Backend, LocalBackend};
use crate::config::ServerConfig;
use crate::connections::{Connection, Store, discover_ssh_hosts};
use crate::error::{AppError, Result};
use crate::paths::Paths;
use crate::release::Release;
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::{AsyncApp, Context, WeakEntity};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 6188;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerStatus {
    NotInstalled,
    Stopped,
    Starting,
    Running,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub title: String,
    pub message: String,
}

/// What a finished operation changes in what the controller shows.
#[derive(Default)]
struct Update {
    configured: Option<bool>,
    proxy_port: Option<Option<u16>>,
    open_dashboard: bool,
    /// The computer now being managed, once connected.
    connection: Option<(Connection, Arc<dyn Backend>)>,
    /// The answers saved on that computer.
    saved: Option<Option<ServerConfig>>,
}

#[allow(clippy::large_enum_variant)]
enum Event {
    Activity(String),
    Done(Result<Update>),
}

/// Reports the step an operation is on while it runs on its own thread.
struct Progress(UnboundedSender<Event>);

impl Progress {
    fn activity(&self, activity: impl Into<String>) {
        let _ = self.0.unbounded_send(Event::Activity(activity.into()));
    }
}

pub struct ServerController {
    backend: Arc<dyn Backend>,
    pub connection: Connection,
    store: Store,
    /// SSH hosts found in your SSH config.
    pub discovered: Vec<String>,
    pub status: ServerStatus,
    pub installed: Option<Release>,
    latest: Option<Release>,
    pub activity: Option<String>,
    pub failure: Option<Failure>,
    pub is_configured: bool,
    /// The port ricochet serves on as last saved, or `None` before the first setup.
    pub proxy_port: Option<u16>,
    /// The answers saved on the computer being managed, and a count that grows each time they are read again.
    pub saved: Option<ServerConfig>,
    pub generation: u64,
    /// Whether the last-used SSH computer is still being reached on launch.
    pub loading: bool,
}

impl ServerController {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = Store::load();
        let connection = store.active.clone().unwrap_or(Connection::Local);
        let backend: Arc<dyn Backend> = Arc::new(LocalBackend::new());
        let reconnect = connection.is_remote();
        let saved = if reconnect {
            None
        } else {
            backend.load_config()
        };
        let this = Self {
            backend,
            connection,
            store,
            discovered: discover_ssh_hosts(),
            status: ServerStatus::Stopped,
            installed: None,
            latest: None,
            activity: None,
            failure: None,
            is_configured: saved.is_some(),
            proxy_port: saved.as_ref().and_then(|config| config.server.proxy_port()),
            saved,
            generation: 0,
            loading: reconnect,
        };
        cx.spawn(async move |this, cx| Self::watch(this, cx).await)
            .detach();
        this
    }

    pub fn backend(&self) -> Arc<dyn Backend> {
        self.backend.clone()
    }

    /// Every computer to offer: this one, hosts you saved, and hosts from your SSH config.
    pub fn choices(&self) -> Vec<Connection> {
        self.store.choices(&self.discovered)
    }

    /// Where you open ricochet, as seen from this computer.
    pub fn dashboard(&self) -> String {
        format!(
            "http://localhost:{}",
            self.proxy_port.unwrap_or(DEFAULT_PORT)
        )
    }

    pub fn open_dashboard(&mut self, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let port = self.proxy_port.unwrap_or(DEFAULT_PORT);
        cx.spawn(async move |this, cx| {
            let opened = cx
                .background_executor()
                .spawn(async move { backend.open_dashboard(port) })
                .await;
            if let Err(error) = opened {
                let _ = this.update(cx, |this, cx| {
                    this.failure = Some(Failure {
                        title: "Ricochet couldn't open".into(),
                        message: error.to_string(),
                    });
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn open_logs(&self) {
        self.backend.open_logs();
    }

    pub fn can_open_logs(&self) -> bool {
        !self.backend.is_remote()
    }

    pub fn available_update(&self) -> Option<Release> {
        let (latest, installed) = (self.latest.as_ref()?, self.installed.as_ref()?);
        latest.is_newer_than(installed).then(|| latest.clone())
    }

    pub fn dismiss_failure(&mut self, cx: &mut Context<Self>) {
        self.failure = None;
        cx.notify();
    }

    /// Logs in to `connection` and shows what ricochet looks like there.
    pub fn switch(&mut self, connection: Connection, cx: &mut Context<Self>) {
        let activity = format!("Connecting to {}", connection.name());
        self.perform(&activity, "Ricochet couldn't connect", cx, move |_, _| {
            let backend = connection.connect()?;
            let saved = backend.load_config();
            Ok(Update {
                configured: Some(saved.is_some()),
                proxy_port: Some(saved.as_ref().and_then(|c| c.server.proxy_port())),
                connection: Some((connection, backend)),
                saved: Some(saved),
                ..Update::default()
            })
        });
    }

    pub fn set_up(&mut self, config: ServerConfig, keep_running: bool, cx: &mut Context<Self>) {
        let saved_port = self.proxy_port;
        let installed = self.installed.is_some();
        self.perform(
            "Checking your settings",
            "Ricochet didn't finish setting up",
            cx,
            move |backend, progress| {
                let port = config.server.proxy_port().ok_or(AppError::PortInUse(0))?;
                // Ricochet itself holds the port it already uses, so only a new port can belong to another app.
                if Some(port) != saved_port && !backend.is_port_free(port) {
                    return Err(AppError::PortInUse(port));
                }
                progress.activity("Stopping ricochet");
                backend.service().stop()?;
                if !installed {
                    backend.install_latest(&|activity| progress.activity(activity))?;
                }
                progress.activity("Writing your configuration");
                backend.prepare()?;
                backend.save_config(&config, port)?;
                backend.service().write_job()?;
                if keep_running {
                    // Lingering is a convenience, so a refusal must not undo an install that works.
                    let _ = backend.keep_running_after_logout();
                }
                progress.activity("Starting ricochet");
                backend.service().start()?;
                backend.wait_until_running(port)?;
                Ok(Update {
                    configured: Some(true),
                    proxy_port: Some(Some(port)),
                    open_dashboard: true,
                    saved: Some(Some(config)),
                    ..Update::default()
                })
            },
        );
    }

    pub fn start(&mut self, cx: &mut Context<Self>) {
        let port = self.proxy_port.unwrap_or(DEFAULT_PORT);
        self.perform(
            "Starting ricochet",
            "Ricochet didn't start",
            cx,
            move |backend, _| {
                backend.service().start()?;
                backend.wait_until_running(port)?;
                Ok(Update::default())
            },
        );
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.perform(
            "Stopping ricochet",
            "Ricochet didn't stop",
            cx,
            |backend, _| {
                backend.service().stop()?;
                Ok(Update::default())
            },
        );
    }

    pub fn restart(&mut self, cx: &mut Context<Self>) {
        let port = self.proxy_port.unwrap_or(DEFAULT_PORT);
        self.perform(
            "Restarting ricochet",
            "Ricochet didn't restart",
            cx,
            move |backend, _| {
                backend.service().restart()?;
                backend.wait_until_running(port)?;
                Ok(Update::default())
            },
        );
    }

    pub fn update_to(&mut self, release: Release, cx: &mut Context<Self>) {
        let port = self.proxy_port.unwrap_or(DEFAULT_PORT);
        let activity = format!("Updating to ricochet {}", release.version);
        self.perform(
            &activity,
            "Ricochet didn't finish updating",
            cx,
            move |backend, _| {
                backend.install_release(&release)?;
                backend.prepare()?;
                if backend.service().is_loaded() {
                    backend.service().restart()?;
                    backend.wait_until_running(port)?;
                }
                Ok(Update::default())
            },
        );
    }

    /// Downloads the latest ricochet-server without configuring or starting it, which **Install and start** does later.
    pub fn install(&mut self, cx: &mut Context<Self>) {
        self.perform(
            "Checking for the latest ricochet",
            "This app couldn't install ricochet",
            cx,
            |backend, progress| {
                backend.install_latest(&|activity| progress.activity(activity))?;
                Ok(Update::default())
            },
        );
    }

    /// Stops everything the app started and moves ricochet and its data aside, keeping this app for a fresh setup.
    pub fn uninstall(&mut self, cx: &mut Context<Self>) {
        self.perform(
            "Uninstalling ricochet",
            "Ricochet didn't finish uninstalling",
            cx,
            |backend, _| {
                backend.service().remove()?;
                backend.uninstall()?;
                Ok(Update {
                    configured: Some(false),
                    proxy_port: Some(None),
                    ..Update::default()
                })
            },
        );
    }

    /// Runs `work` on its own thread, showing each step it reports, then shows its failure or applies its update.
    fn perform<W>(&mut self, activity: &str, failing: &'static str, cx: &mut Context<Self>, work: W)
    where
        W: FnOnce(&Arc<dyn Backend>, &Progress) -> Result<Update> + Send + 'static,
    {
        if self.activity.is_some() {
            return;
        }
        self.activity = Some(activity.to_owned());
        self.failure = None;
        cx.notify();

        let (sender, mut events) = unbounded();
        let backend = self.backend.clone();
        let progress = Progress(sender.clone());
        std::thread::spawn(move || {
            let result = work(&backend, &progress);
            let _ = sender.unbounded_send(Event::Done(result));
        });

        let mut step = activity.to_owned();
        cx.spawn(async move |this, cx| {
            while let Some(event) = events.next().await {
                match event {
                    Event::Activity(next) => {
                        step = next.clone();
                        let shown = this.update(cx, |this, cx| {
                            this.activity = Some(next);
                            cx.notify();
                        });
                        if shown.is_err() {
                            break;
                        }
                    }
                    Event::Done(result) => {
                        let step = step.clone();
                        let _ = this.update(cx, |this, cx| this.finish(result, failing, &step, cx));
                        break;
                    }
                }
            }
        })
        .detach();
    }

    fn finish(
        &mut self,
        result: Result<Update>,
        failing: &str,
        step: &str,
        cx: &mut Context<Self>,
    ) {
        self.activity = None;
        self.loading = false;
        match result {
            Ok(update) => {
                if let Some((connection, backend)) = update.connection {
                    self.backend = backend;
                    self.store.remember(&connection);
                    self.store.save();
                    self.connection = connection;
                    self.installed = None;
                    self.status = ServerStatus::Stopped;
                }
                if let Some(saved) = update.saved {
                    self.saved = saved;
                    self.generation += 1;
                }
                if let Some(configured) = update.configured {
                    self.is_configured = configured;
                }
                if let Some(port) = update.proxy_port {
                    self.proxy_port = port;
                }
                if update.open_dashboard {
                    self.open_dashboard(cx);
                }
            }
            Err(error) => {
                self.log_failure(failing, step, &error);
                self.failure = Some(Failure {
                    title: failing.to_owned(),
                    message: error.to_string(),
                });
            }
        }
        cx.notify();
        self.refresh(true, cx);
    }

    /// Reads whether ricochet answers, and with `full` what is installed, off the UI thread.
    fn refresh(&mut self, full: bool, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let port = self.proxy_port.unwrap_or(DEFAULT_PORT);
        let known = self.installed.clone();
        cx.spawn(async move |this, cx| {
            let (installed, status) = cx
                .background_executor()
                .spawn(async move {
                    let installed = if full { backend.installed() } else { known };
                    let status = backend.status(installed.is_some(), port);
                    (installed, status)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.activity.is_none() {
                    this.installed = installed;
                    this.status = status;
                }
                cx.notify();
            });
        })
        .detach();
    }

    async fn watch(this: WeakEntity<Self>, cx: &mut AsyncApp) {
        let started = this.update(cx, |this, cx| {
            if this.loading {
                let connection = this.connection.clone();
                this.switch(connection, cx);
            } else {
                this.refresh(true, cx);
                if this.is_configured {
                    this.restart_if_job_changed(cx);
                }
            }
        });
        if started.is_err() {
            return;
        }
        let mut next_release_check = std::time::Instant::now();
        loop {
            if std::time::Instant::now() >= next_release_check {
                next_release_check += Duration::from_secs(12 * 60 * 60);
                let latest = cx
                    .background_executor()
                    .spawn(async { Release::latest().ok() })
                    .await;
                let updated = this.update(cx, |this, cx| {
                    this.latest = latest;
                    cx.notify();
                });
                if updated.is_err() {
                    return;
                }
            }
            let polled = this.update(cx, |this, cx| {
                if this.activity.is_none() {
                    this.refresh(false, cx);
                }
            });
            if polled.is_err() {
                return;
            }
            cx.background_executor().timer(Duration::from_secs(3)).await;
        }
    }

    /// Rewrites the service job on launch and restarts ricochet only when the job changed.
    fn restart_if_job_changed(&mut self, cx: &mut Context<Self>) {
        let port = self.proxy_port.unwrap_or(DEFAULT_PORT);
        self.perform(
            "Restarting ricochet",
            "Ricochet didn't restart",
            cx,
            move |backend, _| {
                let _ = backend.prepare();
                if backend.service().write_job()? && backend.service().is_loaded() {
                    backend.service().stop()?;
                    backend.service().start()?;
                    backend.wait_until_running(port)?;
                }
                Ok(Update::default())
            },
        );
    }

    /// Appends the failure to the app's log, so a message cut off on screen can still be read.
    fn log_failure(&self, title: &str, step: &str, error: &AppError) {
        let local = Paths::new();
        let entry = format!(
            "{} {title} while {step} ({}): {error}\n",
            chrono::Utc::now().to_rfc3339(),
            self.connection.name()
        );
        let _ = std::fs::create_dir_all(&local.app_support);
        if let Ok(mut log) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&local.app_log)
        {
            let _ = log.write_all(entry.as_bytes());
        }
    }
}
