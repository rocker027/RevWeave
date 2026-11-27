//! The commit message form: submit gating, submission, and the commit box.

use super::*;
use crate::localization::{Language, text};
use crate::view::pending_commit::PendingCommit;
use gitcomet_core::services::GitHooks;

pub(super) fn commit_allowed(is_merge_active: bool, staged_count: usize) -> bool {
    staged_count > 0 || is_merge_active
}

/// What a commit would record and where, for the commit buttons' tooltips:
/// "3 staged files on main", or "3 changed files on main" when the commit
/// stages them first. `None` when there are none.
pub(super) fn commit_target_summary(
    count: usize,
    kind: &str,
    branch: Option<&str>,
) -> Option<String> {
    if count == 0 {
        return None;
    }
    let files = if count == 1 {
        format!("1 {kind} file")
    } else {
        format!("{count} {kind} files")
    };
    Some(match branch {
        Some("HEAD" | "") => format!("{files} on a detached HEAD"),
        Some(branch) => format!("{files} on {branch}"),
        None => format!("{files} · branch unavailable"),
    })
}

/// The commit options a submit carries.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CommitOptions {
    pub(super) amend: bool,
    pub(super) push_after: bool,
    pub(super) skip_hooks: bool,
}

impl CommitOptions {
    /// The commit message; `stage_paths` are staged first in the same run.
    pub(super) fn msg(
        self,
        repo_id: RepoId,
        message: String,
        stage_paths: Vec<std::path::PathBuf>,
    ) -> Msg {
        let hooks = GitHooks::from_skip(self.skip_hooks);
        Msg::Commit {
            repo_id,
            request: gitcomet_state::model::CommitRequest {
                message,
                amend: self.amend,
                push_after_commit: self.push_after,
                hooks,
                stage_paths,
                fixup_target: None,
            },
        }
    }

    /// The Commit button's label and tooltip when it stages every change
    /// first. Commit & Push never does: it stays with what is staged.
    pub(super) fn stage_all_button_text(self, language: Language) -> (&'static str, &'static str) {
        if language == Language::TraditionalChinese {
            if self.skip_hooks {
                (
                    "暫存全部並提交（略過掛鉤）",
                    "暫存全部變更（包含未追蹤檔案）並提交，略過 pre-commit 和 commit-msg 掛鉤。",
                )
            } else {
                ("暫存全部並提交", "暫存全部變更（包含未追蹤檔案）並提交。")
            }
        } else if self.skip_hooks {
            (
                "Stage All & Commit without Hooks",
                "Stage all changes, untracked files included, and commit them without running the pre-commit and commit-msg hooks",
            )
        } else {
            (
                "Stage All & Commit",
                "Stage all changes, untracked files included, and commit them",
            )
        }
    }

    /// The commit button's label and tooltip. Skipped hooks show on the button
    /// itself, since the choice outlasts the commit it was made for.
    pub(super) fn button_text(self, language: Language) -> (&'static str, &'static str) {
        let (english, traditional_chinese) = match (self.amend, self.push_after, self.skip_hooks) {
            (false, false, false) => (
                ("Commit", "Commit staged changes"),
                ("提交", "提交已暫存的變更"),
            ),
            (false, true, false) => (
                ("Commit & Push", "Commit staged changes and push"),
                ("提交並推送", "提交已暫存的變更並推送"),
            ),
            (true, false, false) => (
                ("Amend", "Amend the previous commit"),
                ("修訂", "修訂上一筆提交"),
            ),
            (true, true, false) => (
                (
                    "Amend & Push",
                    "Amend the previous commit; published amends require explicit force push with lease",
                ),
                (
                    "修訂並推送",
                    "修訂上一筆提交。若原提交已推送，須另外執行有租約的強制推送（force push with lease）。",
                ),
            ),
            (false, false, true) => (
                (
                    "Commit without Hooks",
                    "Commit staged changes without running the pre-commit and commit-msg hooks",
                ),
                (
                    "提交（略過掛鉤）",
                    "提交已暫存的變更，略過 pre-commit 和 commit-msg 掛鉤。",
                ),
            ),
            (false, true, true) => (
                (
                    "Commit & Push without Hooks",
                    "Commit staged changes and push without running the pre-commit, commit-msg and pre-push hooks",
                ),
                (
                    "提交並推送（略過掛鉤）",
                    "提交已暫存的變更並推送，略過 pre-commit、commit-msg 和 pre-push 掛鉤。",
                ),
            ),
            (true, false, true) => (
                (
                    "Amend without Hooks",
                    "Amend the previous commit without running the pre-commit and commit-msg hooks",
                ),
                (
                    "修訂（略過掛鉤）",
                    "修訂上一筆提交，略過 pre-commit 和 commit-msg 掛鉤。",
                ),
            ),
            (true, true, true) => (
                (
                    "Amend & Push without Hooks",
                    "Amend the previous commit without running the pre-commit, commit-msg and pre-push hooks; published amends require explicit force push with lease",
                ),
                (
                    "修訂並推送（略過掛鉤）",
                    "修訂上一筆提交，略過 pre-commit、commit-msg 和 pre-push 掛鉤。若原提交已推送，須另外執行有租約的強制推送（force push with lease）。",
                ),
            ),
        };
        match language {
            Language::English => english,
            Language::TraditionalChinese => traditional_chinese,
        }
    }
}

impl DetailsPaneView {
    /// The scope shown by both commit entry points, even before typing a message.
    pub(in crate::view) fn commit_scope_summary(
        repo: &RepoState,
        amend: bool,
        language: Language,
    ) -> String {
        let target = match &repo.head_branch {
            Loadable::Ready(branch) if !branch.is_empty() && branch != "HEAD" => branch.as_str(),
            Loadable::Ready(_) => language.text("Detached HEAD", "分離 HEAD"),
            _ => language.text("Branch unavailable", "分支尚未載入"),
        };
        let Some(staged) = repo.staged_status_entries() else {
            return format!(
                "{target} · {}",
                language.text("File status unavailable", "檔案狀態尚未載入")
            );
        };
        let count = staged.len();
        let scope = if amend {
            match language {
                Language::English => format!("Amend previous commit · {count} staged file(s)"),
                Language::TraditionalChinese => {
                    format!("修訂上一筆提交 · 納入 {count} 個已暫存檔案")
                }
            }
        } else if merge_in_progress(repo) {
            match language {
                Language::English => format!("Complete merge · {count} staged file(s)"),
                Language::TraditionalChinese => format!("完成合併提交 · 納入 {count} 個已暫存檔案"),
            }
        } else if let Ok(paths) = stage_all_commit_paths(repo, false) {
            let count = paths.len();
            match language {
                Language::English => {
                    format!("Stage all & commit {count} changed file(s), including untracked files")
                }
                Language::TraditionalChinese => {
                    format!("暫存全部並提交 {count} 個變更檔案，包含未追蹤檔案")
                }
            }
        } else {
            match language {
                Language::English => format!("Commit only {count} staged file(s)"),
                Language::TraditionalChinese => format!("僅提交 {count} 個已暫存檔案"),
            }
        };
        format!("{target} · {scope}")
    }

    /// The active repository, when a commit with `message` may start at all.
    fn commit_submit_repo(&self, message: &str) -> Option<&RepoState> {
        let repo = self.active_repo()?;
        let blocked = repo.is_bare()
            || self.pending_commits.contains_key(&repo.id)
            || repo.commit_in_flight > 0
            || message.trim().is_empty();
        (!blocked).then_some(repo)
    }

    /// Whether `message` can commit what is staged (or amend).
    pub(super) fn can_submit_commit(&self, message: &str) -> bool {
        let Some(repo) = self.commit_submit_repo(message) else {
            return false;
        };
        if self.commit_amend_enabled {
            return amend_allowed(repo);
        }
        let staged_count = repo
            .staged_status_entries()
            .map_or(0, |entries| entries.len());
        let is_merge_active = merge_in_progress(repo);
        commit_allowed(is_merge_active, staged_count)
    }

    /// What the Commit button stages before committing `message`: every
    /// change, when nothing is staged. `None` when it commits the index as is.
    pub(super) fn stage_all_submit_paths(&self, message: &str) -> Option<Vec<std::path::PathBuf>> {
        let repo = self.commit_submit_repo(message)?;
        stage_all_commit_paths(repo, self.commit_amend_enabled).ok()
    }

    fn submit_commit(&mut self, push_after_commit: bool, cx: &mut gpui::Context<Self>) -> bool {
        let Some(repo_id) = self.active_repo_id() else {
            return false;
        };
        let message = self
            .commit_message_input
            .read_with(cx, |input, _| input.text().to_string());
        let options = self.commit_options();
        let stage_paths = if self.can_submit_commit(&message) {
            Vec::new()
        } else if push_after_commit {
            return false;
        } else {
            let Some(paths) = self.stage_all_submit_paths(&message) else {
                return false;
            };
            paths
        };

        self.pending_commits.insert(
            repo_id,
            PendingCommit::new(
                &self.state,
                repo_id,
                options.amend,
                message.trim().to_string().into(),
            ),
        );
        self.store.dispatch(
            CommitOptions {
                push_after: push_after_commit,
                ..options
            }
            .msg(repo_id, message.trim().to_string(), stage_paths),
        );
        cx.notify();
        true
    }

    /// Asks an Orca agent for a message describing the staged changes and
    /// fills the commit box with it; a newer request replaces this one.
    fn generate_ai_commit_message(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(repo) = self.active_repo() else {
            return;
        };
        let (repo_id, lifetime, workdir) = (repo.id, repo.lifetime(), repo.spec.workdir.clone());
        let language = Language::current(cx);
        let task = cx.spawn(async move |this, cx| {
            let result = smol::unblock(move || {
                let template = gitcomet_state::session::load()
                    .commit_message_prompt_template
                    .unwrap_or_default();
                gitcomet_state::store::orca_commit_message::generate_commit_message(
                    &workdir, &template,
                )
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                this.commit_message_generation = None;
                match result {
                    Ok(message) => {
                        this.set_extension_commit_message(repo_id, lifetime, message, cx);
                    }
                    Err(error) => {
                        let message = format!(
                            "{} {error}",
                            language
                                .text("Could not generate a commit message.", "無法產生提交訊息。")
                        );
                        let _ = this.root_view.update(cx, |root, cx| {
                            root.push_toast(components::ToastKind::Error, message, cx);
                        });
                    }
                }
                cx.notify();
            });
        });
        self.commit_message_generation = Some((repo_id, task));
        cx.notify();
    }

    fn ai_commit_message_button(
        &self,
        staged_count: usize,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        let scaled_px = crate::ui_scale::scaler(crate::ui_scale::current(cx).percent);
        let repo_key = self.active_repo_id().map(|id| id.0).unwrap_or(0);
        let busy = self.commit_message_generation.is_some();
        let generating = matches!(
            self.commit_message_generation,
            Some((id, _)) if Some(id) == self.active_repo_id()
        );
        let icon_color = theme.colors.foreground.secondary;
        let start_slot: components::ButtonSlot = if generating {
            components::ButtonSlot::spinner(
                ("ai_commit_message_spinner", repo_key),
                icon_color,
                scaled_px(12.0),
            )
        } else {
            svg_icon("icons/lightning.svg", icon_color, scaled_px(12.0)).into()
        };
        let tooltip = if busy {
            text(cx, "Generating commit message…", "正在產生提交訊息…")
        } else if staged_count == 0 {
            text(
                cx,
                "Stage changes to generate a commit message",
                "暫存變更後才能產生提交訊息",
            )
        } else {
            text(
                cx,
                "Generate a commit message from the staged changes with AI (Orca)",
                "依已暫存的變更，用 AI（Orca）產生提交訊息",
            )
        };
        components::Button::new("ai_commit_message", "")
            .start_slot(start_slot)
            .style(components::ButtonStyle::Transparent)
            .disabled(busy || staged_count == 0)
            .on_click(theme, cx, |this, _e, _w, cx| {
                this.generate_ai_commit_message(cx);
            })
            .debug_selector(|| "ai_commit_message_button".to_string())
            .gitcomet_tooltip(theme, tooltip.into())
    }

    pub(super) fn commit_options(&self) -> CommitOptions {
        CommitOptions {
            amend: self.commit_amend_enabled,
            push_after: self.commit_push_after_enabled,
            skip_hooks: self.commit_skip_hooks_enabled,
        }
    }

    pub(in crate::view) fn handle_commit_submit_shortcut(
        &mut self,
        push_after_commit: bool,
        window: &Window,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        if !push_after_commit && self.handle_commit_reword_submit_shortcut(window, cx) {
            return true;
        }
        if !self
            .commit_message_input
            .read(cx)
            .focus_handle()
            .is_focused(window)
        {
            return false;
        }

        let _ = self.submit_commit(push_after_commit, cx);
        true
    }

    pub(in crate::view) fn commit_box(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let ui_scale_percent = crate::ui_scale::current(cx).percent;
        let scaled_px = crate::ui_scale::scaler(ui_scale_percent);
        let commit_in_flight = self
            .active_repo()
            .is_some_and(|repo| repo.commit_in_flight > 0);
        let commit_message_text = self.commit_message_input.read(cx).text().to_string();
        let can_submit_commit = self.can_submit_commit(&commit_message_text);
        // With nothing staged, the Commit button stages every change first.
        let stage_all_count = if can_submit_commit {
            None
        } else {
            self.stage_all_submit_paths(&commit_message_text)
                .map(|paths| paths.len())
        };
        let repo_key = self.active_repo_id().map(|id| id.0).unwrap_or(0);
        let icon_color = theme.colors.accent.foreground;
        let icon = |path: &'static str| svg_icon(path, icon_color, scaled_px(14.0));
        let spinner = |id: (&'static str, u64)| {
            components::ButtonSlot::spinner(id, icon_color, scaled_px(14.0))
        };
        let options = self.commit_options();
        let amend = options.amend;
        let language = Language::current(cx);
        let scope_summary = self
            .active_repo()
            .map(|repo| Self::commit_scope_summary(repo, amend, language));
        let commit_options = CommitOptions {
            push_after: false,
            ..options
        };
        let (commit_label, commit_tooltip) = if stage_all_count.is_some() {
            commit_options.stage_all_button_text(language)
        } else {
            commit_options.button_text(language)
        };
        let (push_label, push_tooltip) = CommitOptions {
            push_after: true,
            ..options
        }
        .button_text(language);
        // Like GitKraken's "Commit changes to N files", the buttons show how
        // many files the commit takes and the tooltips name the branch.
        let staged_count = self
            .active_repo()
            .and_then(|repo| repo.staged_status_entries().map(|entries| entries.len()))
            .unwrap_or(0);
        let head_branch = self
            .active_repo()
            .and_then(|repo| repo.head_branch.ready().cloned());
        let with_target = |tooltip: &'static str, count: usize, kind: &str| -> SharedString {
            match commit_target_summary(count, kind, head_branch.as_deref()) {
                Some(summary) => format!("{tooltip} · {summary}").into(),
                None => tooltip.into(),
            }
        };
        let commit_tooltip = match stage_all_count {
            Some(count) => with_target(commit_tooltip, count, "changed"),
            None => with_target(commit_tooltip, staged_count, "staged"),
        };
        let push_tooltip = with_target(push_tooltip, staged_count, "staged");
        let badge = move |count: usize| -> Option<components::ButtonSlot> {
            (count > 0).then(|| {
                components::ButtonSlot::custom(move |disabled| {
                    div()
                        .text_size(theme.ui_text(12.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.disabled_color_if(disabled, icon_color))
                        .child(count.to_string())
                })
            })
        };
        let previous_messages_invoker: SharedString = "previous_commit_messages".into();
        let previous_messages_active = self
            .active_context_menu_invoker
            .as_ref()
            .is_some_and(|id| id.as_ref() == previous_messages_invoker.as_ref());
        let menu_selected_bg = components::control_open_background(theme);
        let previous_messages_icon_color = if previous_messages_active {
            theme.colors.accent.foreground
        } else {
            theme.colors.foreground.secondary
        };
        let commit_message = div()
            .id(("commit_message_container", repo_key))
            .w_full()
            .min_w(px(0.0))
            .child(self.commit_message_input.clone());
        let start_slot =
            |id: &'static str, icon_selector: &'static str| -> components::ButtonSlot {
                if commit_in_flight {
                    spinner((id, repo_key))
                } else {
                    icon("icons/check.svg")
                        .debug_selector(move || icon_selector.to_string())
                        .into()
                }
            };
        let mut commit = components::Button::new("commit", commit_label)
            .start_slot(start_slot("commit_spinner", "commit_button_icon"))
            .style(components::ButtonStyle::Subtle);
        if let Some(badge) = badge(stage_all_count.unwrap_or(staged_count)) {
            commit = commit.end_slot(badge);
        }
        let commit = commit
            .disabled(!can_submit_commit && stage_all_count.is_none())
            .on_click(theme, cx, |this, _e, _w, cx| {
                let _ = this.submit_commit(false, cx);
            })
            .debug_selector(|| "commit_button".to_string())
            .gitcomet_tooltip_keyed(
                theme,
                commit_tooltip,
                vec![components::secondary_shortcut("Enter").into()],
            );
        let mut commit_and_push = components::Button::new("commit_and_push", push_label)
            .start_slot(start_slot(
                "commit_push_spinner",
                "commit_and_push_button_icon",
            ))
            .style(components::ButtonStyle::Subtle);
        if let Some(badge) = badge(staged_count) {
            commit_and_push = commit_and_push.end_slot(badge);
        }
        let commit_and_push = commit_and_push
            .disabled(!can_submit_commit)
            .on_click(theme, cx, |this, _e, _w, cx| {
                let _ = this.submit_commit(true, cx);
            })
            .debug_selector(|| "commit_and_push_button".to_string())
            .gitcomet_tooltip_keyed(
                theme,
                push_tooltip,
                vec![components::secondary_shortcut("Shift+Enter").into()],
            );
        // The frame and raised fill the old split Commit button used.
        let framed = |frame_selector: &'static str, button: gpui::Stateful<gpui::Div>| {
            components::filled_action_frame(
                theme,
                crate::ui_scale::UiScale::from_percent(ui_scale_percent)
                    .with_appearance(theme.metrics),
            )
            .flex_1()
            .debug_selector(move || frame_selector.to_string())
            .child(
                div()
                    .flex()
                    .items_center()
                    .h_full()
                    .w_full()
                    .rounded(px(theme.radii.control))
                    .bg(theme.colors.surface.raised)
                    .overflow_hidden()
                    // The whole frame is the click target, not just the label.
                    .child(button.flex_1()),
            )
        };
        let checkbox = |id: &'static str, label: &'static str, on: bool| {
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .flex()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .child(
                    div()
                        .size(scaled_px(14.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(theme.radii.control.min(4.0)))
                        .border_1()
                        .border_color(if on {
                            theme.colors.accent.foreground
                        } else {
                            theme.colors.stroke.default
                        })
                        .when(on, |d| d.bg(theme.colors.accent.foreground))
                        .when(on, |d| {
                            d.child(svg_icon(
                                "icons/check.svg",
                                theme.colors.surface.canvas,
                                scaled_px(10.0),
                            ))
                        }),
                )
                .child(
                    div()
                        .text_size(theme.ui_text(13.0))
                        .text_color(theme.colors.foreground.primary)
                        .child(label),
                )
        };
        let amend_toggle = checkbox(
            "commit_amend_toggle",
            text(cx, "Amend previous commit", "修訂上一筆提交"),
            amend,
        )
        .on_activate(
            false,
            components::ControlActivation::Action,
            cx.listener(|this, e: &ClickEvent, _w, cx| {
                if e.standard_click() {
                    this.toggle_commit_amend(cx);
                }
            }),
        );
        let skip_hooks_toggle = checkbox(
            "commit_skip_hooks_toggle",
            text(cx, "Skip hooks", "略過掛鉤"),
            options.skip_hooks,
        )
        .on_activate(
            false,
            components::ControlActivation::Action,
            cx.listener(|this, e: &ClickEvent, _w, cx| {
                if e.standard_click() {
                    this.toggle_commit_skip_hooks(cx);
                }
            }),
        )
        .gitcomet_tooltip(
            theme,
            text(
                cx,
                "Skip the pre-commit, commit-msg and pre-push hooks until this window closes",
                "關閉此視窗前，略過 pre-commit、commit-msg 和 pre-push 掛鉤。",
            )
            .into(),
        );
        let previous_messages_menu = components::Button::new("previous_commit_messages", "")
            .start_slot(
                svg_icon(
                    "icons/history.svg",
                    previous_messages_icon_color,
                    scaled_px(14.0),
                )
                .debug_selector(|| "previous_commit_messages_icon".to_string()),
            )
            .style(components::ButtonStyle::Subtle)
            .open(previous_messages_active)
            .selected_bg(menu_selected_bg)
            .disabled(self.active_repo_id().is_none())
            .on_click(theme, cx, move |this, e, window, cx| {
                let Some(repo_id) = this.active_repo_id() else {
                    return;
                };

                this.open_popover_at(
                    (PopoverKind::PreviousCommitMessagesMenu { repo_id })
                        .invoked_by(previous_messages_invoker.clone()),
                    e.position(),
                    window,
                    cx,
                );
            })
            .debug_selector(|| "previous_commit_messages_button".to_string())
            .gitcomet_tooltip(
                theme,
                text(cx, "Previous commit messages", "先前的提交訊息").into(),
            );
        let message_card = div()
            .flex()
            .flex_col()
            .w_full()
            .border_1()
            .border_color(theme.colors.stroke.default)
            .rounded(px(theme.radii.panel))
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .py_1p5()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(theme.colors.stroke.subtle)
                    .text_size(theme.ui_text(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.colors.foreground.primary)
                    .child(text(cx, "Commit Message", "提交訊息"))
                    .child(self.ai_commit_message_button(staged_count, cx)),
            )
            .child(div().px_3().py_2().child(commit_message));
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(message_card)
            .when_some(scope_summary, |view, summary| {
                view.child(
                    div()
                        .id("commit_scope_summary")
                        .debug_selector(|| "commit_scope_summary".to_string())
                        .text_size(theme.ui_text(11.0))
                        .text_color(theme.colors.foreground.secondary)
                        .child(summary),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_x_3()
                            .gap_y_1()
                            .child(amend_toggle)
                            .child(skip_hooks_toggle),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .children(self.extension_commit_items.iter().cloned())
                            .child(previous_messages_menu),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(framed("commit_button_frame", commit))
                    .child(framed("commit_and_push_button_frame", commit_and_push)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::commit_target_summary;
    use super::*;
    use gitcomet_core::domain::{FileStatus, FileStatusKind, RepoSpec, RepoStatus};

    fn repo_with_changes(staged: &[&str], unstaged: &[&str]) -> RepoState {
        let mut repo = RepoState::new_opening(
            RepoId(1),
            RepoSpec {
                workdir: "/tmp/commit-scope".into(),
            },
        );
        repo.repository_mode = Some(gitcomet_core::domain::RepositoryMode::WorkingTree);
        repo.head_branch = Loadable::Ready("main".into());
        let entries = |paths: &[&str]| {
            Arc::new(
                paths
                    .iter()
                    .map(|path| FileStatus {
                        path: (*path).into(),
                        kind: if *path == "untracked" {
                            FileStatusKind::Untracked
                        } else {
                            FileStatusKind::Modified
                        },
                        conflict: None,
                    })
                    .collect(),
            )
        };
        repo.status = Loadable::Ready(
            RepoStatus {
                staged: entries(staged),
                unstaged: entries(unstaged),
            }
            .into(),
        );
        repo
    }

    #[test]
    fn commit_scope_distinguishes_partial_staging_from_stage_all() {
        let staged = repo_with_changes(&["a"], &["b", "untracked"]);
        assert_eq!(
            DetailsPaneView::commit_scope_summary(&staged, false, Language::TraditionalChinese),
            "main · 僅提交 1 個已暫存檔案"
        );
        let unstaged = repo_with_changes(&[], &["b", "untracked"]);
        assert_eq!(
            DetailsPaneView::commit_scope_summary(&unstaged, false, Language::TraditionalChinese),
            "main · 暫存全部並提交 2 個變更檔案，包含未追蹤檔案"
        );
    }

    #[test]
    fn commit_scope_names_merge_amend_detached_and_unknown_states() {
        let mut repo = repo_with_changes(&[], &["unstaged"]);
        assert_eq!(
            DetailsPaneView::commit_scope_summary(&repo, true, Language::English),
            "main · Amend previous commit · 0 staged file(s)"
        );
        repo.merge_commit_message = Loadable::Ready(Some("merge".into()));
        assert_eq!(
            DetailsPaneView::commit_scope_summary(&repo, false, Language::English),
            "main · Complete merge · 0 staged file(s)"
        );
        repo.head_branch = Loadable::Ready("HEAD".into());
        assert!(
            DetailsPaneView::commit_scope_summary(&repo, false, Language::English)
                .starts_with("Detached HEAD ·")
        );
        repo.head_branch = Loadable::Loading;
        assert!(
            DetailsPaneView::commit_scope_summary(&repo, false, Language::English)
                .starts_with("Branch unavailable ·")
        );
        repo.status = Loadable::NotLoaded;
        assert_eq!(
            DetailsPaneView::commit_scope_summary(&repo, false, Language::English),
            "Branch unavailable · File status unavailable"
        );
    }

    #[test]
    fn commit_target_summary_counts_files_and_names_the_branch() {
        assert_eq!(commit_target_summary(0, "staged", Some("main")), None);
        assert_eq!(
            commit_target_summary(1, "staged", Some("main")).as_deref(),
            Some("1 staged file on main")
        );
        assert_eq!(
            commit_target_summary(3, "staged", Some("feature/x")).as_deref(),
            Some("3 staged files on feature/x")
        );
        assert_eq!(
            commit_target_summary(2, "changed", Some("HEAD")).as_deref(),
            Some("2 changed files on a detached HEAD")
        );
        assert_eq!(
            commit_target_summary(2, "staged", None).as_deref(),
            Some("2 staged files · branch unavailable")
        );
    }
}
