use super::*;
use crate::localization::Language;
use crate::view::panes::history::{HistoryClickTarget, HistoryPointerClick};
use crate::view::rows::{HistoryBranchActivation, history_double_click_branch};
use std::time::Duration;

const HISTORY_REF_EXPANSION_DELAY: Duration = Duration::from_millis(500);

impl PopoverHost {
    /// Handle the graph's ref chooser only. An ordinary click still expands its
    /// actions; the second click must land on the same named branch.
    pub(super) fn handle_history_ref_click(
        &mut self,
        action: &ContextMenuAction,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        self.history_ref_expansion_task = None;
        let first = self.first_history_ref_click.take();
        let (
            ClickEvent::Mouse(click),
            Some(PopoverKind::CommitMenu {
                repo_id,
                commit_id,
                refs: CommitMenuRefs::Chooser,
            }),
        ) = (event, self.popover.as_ref())
        else {
            return false;
        };
        if !event.standard_click() {
            return false;
        }
        let target = match action {
            ContextMenuAction::ToggleHistoryRefGroup {
                target: HistoryMenuRef::Branch(target),
            } => Some(target),
            _ => None,
        };
        let repeated_click = click.up.click_count > 1;
        if target.is_none() && !repeated_click {
            return false;
        }
        // An expansion can put a child action under the pointer. Consume a
        // repeated click from the original branch row even if its target changed.
        if !matches!(click.up.click_count, 1 | 2)
            || click.down.click_count != click.up.click_count
            || click.down.modifiers.number_of_modifiers() != 0
            || click.up.modifiers.number_of_modifiers() != 0
        {
            return repeated_click;
        }

        let repo_id = *repo_id;
        let eligible = target.and_then(|target| {
            let repo = self.active_repo().filter(|repo| repo.id == repo_id)?;
            (crate::view::rows::sidebar::branch_commit_id(repo, target).as_ref() == Some(commit_id))
                .then_some((repo, target))
        });
        let Some((repo, target)) = eligible else {
            return repeated_click;
        };
        let clicked = HistoryPointerClick {
            repo_id,
            lifetime: repo.lifetime(),
            target: HistoryClickTarget::Branch {
                commit_id: commit_id.clone(),
                target: target.clone(),
            },
        };
        if !repeated_click {
            self.latch_context_menu_placement();
            if self.expanded_history_ref.is_some() {
                self.defer_history_ref_expansion(clicked.clone(), target.clone(), window, cx);
                self.first_history_ref_click = Some(clicked);
                return true;
            }
            self.first_history_ref_click = Some(clicked);
            return false;
        }
        if first == Some(clicked)
            && let Some(activation) = history_double_click_branch(repo, target)
        {
            self.activate_history_ref_branch(repo_id, activation, window, cx);
        }
        true
    }

    /// Closing an expanded group above the clicked row, or shrinking the
    /// scrollable menu, can move the row before a second click arrives.
    fn defer_history_ref_expansion(
        &mut self,
        clicked: HistoryPointerClick,
        target: BranchMenuTarget,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.history_ref_expansion_task = Some(cx.spawn_in(window, async move |host, cx| {
            cx.background_executor()
                .timer(HISTORY_REF_EXPANSION_DELAY)
                .await;
            let _ = host.update_in(cx, |host, window, cx| {
                let Some(PopoverKind::CommitMenu {
                    commit_id,
                    refs: CommitMenuRefs::Chooser,
                    ..
                }) = host.popover.as_ref()
                else {
                    return;
                };
                if host.first_history_ref_click.as_ref() == Some(&clicked)
                    && host.active_repo().is_some_and(|repo| {
                        repo.id == clicked.repo_id
                            && repo.lifetime() == clicked.lifetime
                            && crate::view::rows::sidebar::branch_commit_id(repo, &target).as_ref()
                                == Some(commit_id)
                    })
                {
                    host.context_menu_activate_action(
                        ContextMenuAction::ToggleHistoryRefGroup {
                            target: HistoryMenuRef::Branch(target),
                        },
                        window,
                        cx,
                    );
                }
            });
        }));
    }

    fn activate_history_ref_branch(
        &mut self,
        repo_id: RepoId,
        activation: HistoryBranchActivation,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if !matches!(activation, HistoryBranchActivation::Open(_))
            && self.state.worktree_finish_blocks_workdir_mutation(repo_id)
        {
            return;
        }
        let action = match activation {
            HistoryBranchActivation::Open(path) => ContextMenuAction::OpenRepo { path },
            HistoryBranchActivation::Checkout(name) => {
                if self.state.worktree_finish_protects_branch(repo_id, &name) {
                    return;
                }
                ContextMenuAction::CheckoutBranch { repo_id, name }
            }
            HistoryBranchActivation::PromptForTrackingName { remote, branch } => {
                ContextMenuAction::OpenPopover {
                    kind: PopoverKind::CheckoutRemoteBranchPrompt {
                        repo_id,
                        remote,
                        branch,
                    },
                }
            }
            HistoryBranchActivation::CreateTracking {
                remote,
                branch,
                local_branch,
            } => {
                if !self
                    .state
                    .worktree_finish_protects_branch(repo_id, &local_branch)
                {
                    self.store.dispatch(Msg::CheckoutRemoteBranch {
                        repo_id,
                        remote,
                        branch,
                        local_branch,
                        mode: gitcomet_core::services::CheckoutRemoteBranchMode::Create,
                    });
                    self.close_popover_and_restore_focus(window, cx);
                }
                return;
            }
        };
        self.context_menu_activate_action(action, window, cx);
    }
}

/// The row context, listing the commit's refs that `refs` admits.
pub(super) fn model(
    this: &PopoverHost,
    repo_id: RepoId,
    commit_id: &CommitId,
    refs: &CommitMenuRefs,
    language: Language,
) -> ContextMenuModel {
    let graph_chooser = matches!(refs, CommitMenuRefs::Chooser);
    let include_hidden_remotes = matches!(refs, CommitMenuRefs::DetailsChooser);
    let chooser = include_hidden_remotes || graph_chooser;
    let commit = (!chooser).then(|| commit::model(this, repo_id, commit_id, refs, language));
    let sha = commit_id.as_ref();
    let mut model = ContextMenuModel::new(vec![ContextMenuItem::Header(
        format!(
            "{}{}{}",
            if chooser {
                language.text("Branches and tags", "分支與標籤")
            } else {
                language.text("Commit", "提交")
            },
            if chooser { " · " } else { " " },
            sha.get(..8).unwrap_or(sha)
        )
        .into(),
    )]);
    if let Some(ContextMenuItem::Label(summary)) =
        commit.as_ref().and_then(|menu| menu.items.get(1))
    {
        model.items.push(ContextMenuItem::Label(summary.clone()));
    }
    model.items.push(ContextMenuItem::Separator);

    let Some(repo) = this.state.repos.iter().find(|repo| repo.id == repo_id) else {
        if chooser {
            model.items.push(ContextMenuItem::Label(
                language
                    .text("Repository unavailable", "儲存庫無法使用")
                    .into(),
            ));
        }
        return model;
    };
    let mut targets = Vec::new();
    if let Some(branches) = repo.branches.ready() {
        targets.extend(
            branches
                .iter()
                .filter(|branch| branch.target == *commit_id)
                .map(|branch| HistoryMenuRef::Branch(BranchMenuTarget::local(&branch.name))),
        );
    }
    let hidden_remotes = repo.history_hidden_remotes();
    targets.extend(
        repo.remote_branches
            .ready()
            .into_iter()
            .flat_map(|branches| branches.iter())
            .filter(|branch| {
                branch.target == *commit_id
                    && (include_hidden_remotes || !hidden_remotes.contains(&branch.remote))
            })
            .map(|branch| {
                HistoryMenuRef::Branch(BranchMenuTarget::remote(&branch.remote, &branch.name))
            }),
    );
    if let Some(tags) = repo.tags.ready() {
        targets.extend(
            tags.iter()
                .filter(|tag| tag.target == *commit_id)
                .map(|tag| HistoryMenuRef::Tag(tag.name.clone())),
        );
    }
    targets.retain(|target| refs.admits(target));
    targets.sort();
    targets.dedup();
    // The checked-out branch leads, as in the graph's chips and refs card.
    if let Some(head) = repo.head_branch.ready()
        && let Some(ix) = targets.iter().position(|target| {
            matches!(target, HistoryMenuRef::Branch(BranchMenuTarget::Local { name }) if name == head)
        })
    {
        targets[..=ix].rotate_right(1);
    }
    for target in targets {
        let expanded = this.expanded_history_ref.as_ref() == Some(&target);
        let mut label = match &target {
            HistoryMenuRef::Branch(BranchMenuTarget::Local { name }) if chooser => {
                format!("{name} · {}", language.text("Local branch", "本機分支"))
            }
            HistoryMenuRef::Branch(BranchMenuTarget::Remote { remote, branch }) if chooser => {
                format!(
                    "{remote}/{branch} · {}",
                    language.text("Remote branch", "遠端分支")
                )
            }
            HistoryMenuRef::Tag(name) if chooser => {
                format!("{name} · {}", language.text("Tag", "標籤"))
            }
            HistoryMenuRef::Branch(BranchMenuTarget::Local { name }) => {
                format!("{} {name}", language.text("Local branch", "本機分支"))
            }
            HistoryMenuRef::Branch(BranchMenuTarget::Remote { remote, branch }) => {
                format!(
                    "{} {remote}/{branch}",
                    language.text("Remote branch", "遠端分支")
                )
            }
            HistoryMenuRef::Tag(name) => format!("{} {name}", language.text("Tag", "標籤")),
        };
        let linked_worktree = if chooser {
            match &target {
                HistoryMenuRef::Branch(BranchMenuTarget::Local { name }) => repo
                    .worktrees
                    .ready()
                    .into_iter()
                    .flat_map(|worktrees| worktrees.iter())
                    .find(|worktree| {
                        !worktree.bare
                            && !worktree.detached
                            && worktree.path != repo.spec.workdir
                            && worktree.branch.as_deref() == Some(name)
                            && worktree.head.as_ref() == Some(commit_id)
                    }),
                _ => None,
            }
        } else {
            None
        };
        if linked_worktree.is_some() {
            label.push_str(language.text(" · Linked worktree", " · 連結工作樹"));
        }
        let mut tooltip = linked_worktree.map_or_else(
            || label.clone(),
            |worktree| format!("{label}\n{}", worktree.path.display()),
        );
        let hint = if graph_chooser
            && !repo.is_bare()
            && let HistoryMenuRef::Branch(branch) = &target
        {
            let checkout_available = !this.state.worktree_finish_blocks_workdir_mutation(repo_id)
                && !matches!(branch, BranchMenuTarget::Local { name }
                    if repo.head_branch.ready() == Some(name)
                        || this.state.worktree_finish_protects_branch(repo_id, name));
            if linked_worktree.is_some() {
                Some(language.text("Double-click to open worktree", "點兩下開啟工作樹"))
            } else if checkout_available {
                Some(language.text("Double-click to check out branch", "點兩下切換分支"))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(hint) = hint {
            tooltip.push('\n');
            tooltip.push_str(hint);
        }
        let index = model.items.len();
        model
            .entry_debug_selectors
            .insert(index, format!("history_ref_group_{target:?}").into());
        model.entry_tooltips.insert(index, tooltip.into());
        model.items.push(ContextMenuItem::Entry {
            label: label.into(),
            icon: Some(
                if expanded {
                    "icons/chevron_down.svg"
                } else {
                    "icons/chevron_right.svg"
                }
                .into(),
            ),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::ToggleHistoryRefGroup {
                target: target.clone(),
            }),
        });
        if expanded {
            let actions = match &target {
                // Commit context menus already list the actions for this tip.
                HistoryMenuRef::Branch(target) => {
                    branch::model(this, repo_id, target, chooser, language)
                }
                HistoryMenuRef::Tag(name) => {
                    tag::model_for_tag(this, repo_id, commit_id, name, language)
                }
            };
            append_actions(&mut model, actions, true);
        }
        model.groups.push(index..model.items.len());
        if expanded {
            model.items.push(ContextMenuItem::Separator);
        }
    }
    if chooser {
        append_chooser_load_state(
            &mut model,
            &repo.branches,
            language,
            language.text("branches", "本機分支"),
        );
        append_chooser_load_state(
            &mut model,
            &repo.remote_branches,
            language,
            language.text("remote branches", "遠端分支"),
        );
        append_chooser_load_state(
            &mut model,
            &repo.tags,
            language,
            language.text("tags", "標籤"),
        );
        if model.groups.is_empty()
            && matches!(&repo.branches, Loadable::Ready(_))
            && matches!(&repo.remote_branches, Loadable::Ready(_))
            && matches!(&repo.tags, Loadable::Ready(_))
        {
            model.items.push(ContextMenuItem::Label(
                language
                    .text(
                        "No branches or tags point to this commit",
                        "沒有分支或標籤指向此提交",
                    )
                    .into(),
            ));
        }
        return model;
    }
    // Until tags load, a picked tag has no group yet; say why.
    let tags_wanted = matches!(
        refs,
        CommitMenuRefs::All | CommitMenuRefs::Only(HistoryMenuRef::Tag(_))
    );
    if tags_wanted && !matches!(&repo.tags, Loadable::Ready(_)) {
        model.items.push(ContextMenuItem::Label(match &repo.tags {
            Loadable::Error(error) => match language {
                Language::English => format!("Tags unavailable: {error}"),
                Language::TraditionalChinese => format!("無法載入標籤：{error}"),
            }
            .into(),
            _ => language.text("Loading tags…", "正在載入標籤…").into(),
        }));
    }
    if model.items.len() > 3 {
        model.items.push(ContextMenuItem::Separator);
    }
    if let Some(commit) = commit {
        append_actions(&mut model, commit, false);
    }
    model
}

fn append_chooser_load_state<T>(
    model: &mut ContextMenuModel,
    state: &Loadable<T>,
    language: Language,
    name: &str,
) {
    if matches!(state, Loadable::Ready(_)) {
        return;
    }
    let label = match (language, state) {
        (Language::English, Loadable::Error(error)) => format!("Cannot load {name}: {error}"),
        (Language::TraditionalChinese, Loadable::Error(error)) => {
            format!("無法載入{name}：{error}")
        }
        (Language::English, _) => format!("Loading {name}…"),
        (Language::TraditionalChinese, _) => format!("正在載入{name}…"),
    };
    model.items.push(ContextMenuItem::Label(label.into()));
}

/// Keep presentation metadata attached to its action while composing menus.
fn append_actions(model: &mut ContextMenuModel, source: ContextMenuModel, clear_shortcuts: bool) {
    let first = source
        .items
        .iter()
        .position(|item| matches!(item, ContextMenuItem::Entry { .. }))
        .unwrap_or(source.items.len());
    for (old_index, mut item) in source.items.into_iter().enumerate().skip(first) {
        if clear_shortcuts && let ContextMenuItem::Entry { shortcut, .. } = &mut item {
            *shortcut = None;
        }
        let index = model.items.len();
        if let Some(tooltip) = source.entry_tooltips.get(&old_index) {
            model.entry_tooltips.insert(index, tooltip.clone());
        }
        if let Some(selector) = source.entry_debug_selectors.get(&old_index) {
            model.entry_debug_selectors.insert(index, selector.clone());
        }
        if let Some(label) = source.entry_source_labels.get(&old_index) {
            model.entry_source_labels.insert(index, label.clone());
        }
        model.items.push(item);
    }
}
