//! The server-wide pages: every deployment, deploying new content, and your Git credentials.

use super::item::{Tab, deployment_row, failed, muted};
use super::log::LogSource;
use super::{DashboardView, Loadable};
use crate::api::{self, GitSource};
use crate::ui::components::{card, card_title, field, page_title, prose};
use crate::ui::icons::ph;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Div, FontWeight, PathPromptOptions, div};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::spinner::Spinner;
use gpui_component::{ActiveTheme, Disableable, Sizable, h_flex, v_flex};

/// How many deployments the Deployments page lists.
const RECENT_DEPLOYMENTS: usize = 100;

impl DashboardView {
    // MARK: Loading

    pub(super) fn load_all_deployments(&mut self, cx: &mut Context<Self>) {
        let items: Vec<(String, String)> = self
            .items()
            .iter()
            .map(|item| (item.id.clone(), item.name.clone()))
            .collect();
        self.all_deployments = Loadable::Loading;
        self.fetch(
            cx,
            move |client| {
                let mut all = Vec::new();
                for (id, name) in items {
                    for deployment in client.deployments(&id)? {
                        all.push((name.clone(), deployment));
                    }
                }
                all.sort_by_key(|(_, deployment)| std::cmp::Reverse(deployment.deployed_at));
                all.truncate(RECENT_DEPLOYMENTS);
                Ok(all)
            },
            |this, result, _| this.all_deployments = Loadable::from_result(result),
        );
    }

    pub(super) fn load_credentials(&mut self, cx: &mut Context<Self>) {
        if matches!(self.credentials, Loadable::Loading) {
            return;
        }
        self.credentials = Loadable::Loading;
        self.fetch(
            cx,
            |client| client.credentials(),
            |this, result, _| this.credentials = Loadable::from_result(result),
        );
    }

    /// Opens the item a deploy created or updated, following its build log.
    fn show_deployed(&mut self, deployed: api::Deployed, cx: &mut Context<Self>) {
        self.deploying = false;
        self.selected = Some(deployed.id.clone());
        self.item_generation += 1;
        self.reset_item_state();
        self.tab = Tab::Logs;
        self.page = super::Page::Apps;
        self.load(cx);
        self.follow_log(
            LogSource::Build {
                deployment: deployed.deployment_id,
            },
            cx,
        );
    }

    fn deploy_from_git(&mut self, cx: &mut Context<Self>) {
        let value = |input: &gpui::Entity<gpui_component::input::InputState>| {
            let text = input.read(cx).value().trim().to_owned();
            (!text.is_empty()).then_some(text)
        };
        let Some(url) = value(&self.inputs.git_url) else {
            self.notice = Some("Enter the address of the Git repository.".into());
            cx.notify();
            return;
        };
        let source = GitSource {
            url,
            branch: value(&self.inputs.git_branch),
            path: value(&self.inputs.git_path),
            credential_id: self.deploy_credential.clone(),
        };
        self.start_deploy(cx, move |client| client.deploy_git(&source));
    }

    fn choose_folder(&mut self, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Deploy".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = chosen.await
                && let Some(folder) = paths.pop()
            {
                let _ = this.update(cx, |this, cx| {
                    this.deploy_folder = Some(folder);
                    this.notice = None;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn deploy_from_folder(&mut self, cx: &mut Context<Self>) {
        let Some(folder) = self.deploy_folder.clone() else {
            return;
        };
        self.start_deploy(cx, move |client| {
            let bundle = crate::bundle::pack(&folder).map_err(api::ApiError::Failed)?;
            client.upload(&bundle.bytes, &bundle.config, bundle.id.as_deref())
        });
    }

    fn start_deploy(
        &mut self,
        cx: &mut Context<Self>,
        work: impl FnOnce(&api::Client) -> api::Result<api::Deployed> + Send + 'static,
    ) {
        self.deploying = true;
        self.notice = None;
        cx.notify();
        self.fetch(cx, work, |this, result, cx| match result {
            Ok(deployed) => this.show_deployed(deployed, cx),
            Err(error) => {
                this.deploying = false;
                this.notice = Some(error.to_string());
            }
        });
    }

    // MARK: Rendering

    fn page_frame(
        &self,
        title: &str,
        summary: &str,
        content: impl IntoElement,
        cx: &Context<Self>,
    ) -> AnyElement {
        div()
            .id("dashboard-page")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .gap_5()
                    .p_6()
                    .child(
                        v_flex().gap_1().child(page_title(title, None)).child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(prose(summary.to_owned())),
                        ),
                    )
                    .child(content),
            )
            .into_any_element()
    }

    pub(super) fn deployments_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let body = match &self.all_deployments {
            Loadable::Idle | Loadable::Loading => {
                h_flex().child(Spinner::new().small()).into_any_element()
            }
            Loadable::Failed(message) => failed(message, cx),
            Loadable::Loaded(all) if all.is_empty() => {
                muted("Nothing is deployed to this server yet.", cx)
            }
            Loadable::Loaded(all) => v_flex()
                .children(all.iter().enumerate().map(|(index, (name, deployment))| {
                    let (item, id) = (deployment.content_id.clone(), deployment.id.clone());
                    v_flex()
                        .id(("all-deployments", index))
                        .cursor_pointer()
                        .child(
                            div()
                                .pt_2()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child(name.clone()),
                        )
                        .child(deployment_row(1, deployment, cx))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_item(item.clone(), Tab::Logs, cx);
                            this.follow_log(
                                LogSource::Build {
                                    deployment: id.clone(),
                                },
                                cx,
                            );
                        }))
                }))
                .into_any_element(),
        };
        self.page_frame(
            "Deployment history",
            "The latest deployments of every item you can see. Select one to read its build log.",
            card(cx).child(body),
            cx,
        )
    }

    pub(super) fn deploy_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let credentials = match &self.credentials {
            Loadable::Loaded(credentials) => credentials.clone(),
            _ => Vec::new(),
        };
        let chosen = self.deploy_credential.clone();
        let credential_buttons = h_flex().gap_1().flex_wrap().child({
            let button = Button::new("credential-none").label("None").small();
            let button = if chosen.is_none() {
                button.primary()
            } else {
                button.outline()
            };
            button.on_click(cx.listener(|this, _, _, cx| {
                this.deploy_credential = None;
                cx.notify();
            }))
        });
        let credential_buttons =
            credential_buttons.children(credentials.into_iter().map(|credential| {
                let selected = chosen.as_deref() == Some(&credential.id);
                let button = Button::new(gpui::SharedString::from(format!(
                    "credential-{}",
                    credential.id
                )))
                .label(credential.name.clone())
                .small();
                let button = if selected {
                    button.primary()
                } else {
                    button.outline()
                };
                button.on_click(cx.listener(move |this, _, _, cx| {
                    this.deploy_credential = Some(credential.id.clone());
                    cx.notify();
                }))
            }));
        let folder = self
            .deploy_folder
            .as_ref()
            .map(|folder| folder.display().to_string());
        let deploying = self.deploying;
        let content = v_flex()
            .gap_5()
            .children(self.notice.clone().map(|notice| {
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(prose(notice))
            }))
            .child(
                card(cx)
                    .child(card_title("From a Git repository", cx))
                    .child(field("Repository", None, Input::new(&self.inputs.git_url), cx))
                    .child(field(
                        "Branch",
                        Some("Leave it empty for the repository's default branch.".into()),
                        Input::new(&self.inputs.git_branch),
                        cx,
                    ))
                    .child(field("Folder", None, Input::new(&self.inputs.git_path), cx))
                    .child(field(
                        "Credential",
                        Some("Choose one for a private repository.".into()),
                        credential_buttons,
                        cx,
                    ))
                    .child(
                        h_flex().justify_end().child(
                            Button::new("deploy-git")
                                .icon(ph("rocket-launch"))
                                .label("Deploy")
                                .primary()
                                .small()
                                .loading(deploying)
                                .disabled(deploying)
                                .on_click(cx.listener(|this, _, _, cx| this.deploy_from_git(cx))),
                        ),
                    ),
            )
            .child(
                card(cx)
                    .child(card_title("From a folder", cx))
                    .child(muted(
                        "Upload a project folder with a `_ricochet.toml`, the way `ricochet deploy` does. If the file names an `id`, ricochet deploys that item again.",
                        cx,
                    ))
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                Button::new("choose-folder")
                                    .label("Choose folder")
                                    .outline()
                                    .small()
                                    .disabled(deploying)
                                    .on_click(cx.listener(|this, _, _, cx| this.choose_folder(cx))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .child(folder.clone().unwrap_or_default()),
                            )
                            .child(
                                Button::new("deploy-folder")
                                    .icon(ph("rocket-launch"))
                                    .label("Deploy")
                                    .primary()
                                    .small()
                                    .loading(deploying)
                                    .disabled(deploying || folder.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| this.deploy_from_folder(cx))),
                            ),
                    ),
            );
        self.page_frame(
            "Deploy",
            "Deploy new content, or a new version of an item, to this server.",
            content,
            cx,
        )
    }

    pub(super) fn credentials_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let body: Div = match &self.credentials {
            Loadable::Idle | Loadable::Loading => h_flex().child(Spinner::new().small()),
            Loadable::Failed(message) => div().child(failed(message, cx)),
            Loadable::Loaded(credentials) if credentials.is_empty() => {
                div().child(muted("You have no Git credentials yet.", cx))
            }
            Loadable::Loaded(credentials) => {
                v_flex().children(credentials.iter().enumerate().map(|(index, credential)| {
                    h_flex()
                        .gap_3()
                        .py_2()
                        .items_center()
                        .when(index > 0, |row| {
                            row.border_t_1().border_color(cx.theme().border)
                        })
                        .child(ph("key").size_4().text_color(cx.theme().muted_foreground))
                        .child(div().flex_1().text_sm().child(credential.name.clone()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(credential.kind.to_uppercase()),
                        )
                }))
            }
        };
        let url = self
            .active_server(cx)
            .map(|server| format!("{}/credentials", server.url))
            .unwrap_or_default();
        self.page_frame(
            "Credentials",
            "Git credentials let ricochet deploy from private repositories. ricochet never shows their secrets.",
            v_flex().gap_3().child(card(cx).child(body)).child(
                h_flex().child(
                    Button::new("manage-credentials")
                        .icon(ph("arrow-square-out"))
                        .label("Add or remove credentials in your browser")
                        .ghost()
                        .small()
                        .on_click(move |_, _, _| {
                            let _ = open::that(&url);
                        }),
                ),
            ),
            cx,
        )
    }
}
