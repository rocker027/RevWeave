use super::*;
use crate::localization::Language;
use gitcomet_core::domain::Upstream;
use gitcomet_extension_api::RefMenuTarget;

pub(super) fn model(
    this: &PopoverHost,
    repo_id: RepoId,
    target: &BranchMenuTarget,
    include_tip_actions: bool,
    language: Language,
) -> ContextMenuModel {
    let section = target.section();
    let name = target.display_name();
    let branch_kind = match section {
        BranchSection::Local => language.text("Local branch", "本機分支"),
        BranchSection::Remote => language.text("Remote branch", "遠端分支"),
    };
    let header = components::ContextMenuText::new(format!("{branch_kind} · {name}"))
        .tooltip_mode(components::TruncatedTextTooltipMode::FullTextIfTruncated);
    let mut items = vec![ContextMenuItem::Header(header), ContextMenuItem::Separator];
    let mut branch_entry_tooltips: FxHashMap<usize, SharedString> = FxHashMap::default();
    let mut branch_entry_source_labels = FxHashMap::default();

    let repo = this.state.repos.iter().find(|r| r.id == repo_id);

    let active_branch_name = repo.and_then(|r| r.head_branch.ready());
    let active_branch = repo.and_then(|r| match (&r.branches, active_branch_name) {
        (Loadable::Ready(branches), Some(head)) => {
            branches.iter().find(|branch| branch.name == *head)
        }
        _ => None,
    });
    let active_upstream = active_branch.and_then(|branch| branch.upstream.as_ref());
    let active_branch_has_no_upstream =
        active_branch.is_some_and(|branch| branch.upstream.is_none());
    // The target's own parts: exact even when `remote/branch` is ambiguous.
    let remote_parts = target.remote_parts();
    let target_upstream = remote_parts.map(|(remote, branch)| Upstream {
        remote: remote.to_string(),
        branch: branch.to_string(),
    });
    // Setting an upstream needs the remote-tracking ref to exist; unlinking
    // only needs the target to be the configured upstream.
    let exact_remote_branch = target_upstream.clone().filter(|upstream| {
        repo.and_then(|repo| repo.remote_branches.ready())
            .is_some_and(|branches| {
                branches.iter().any(|candidate| {
                    candidate.remote == upstream.remote && candidate.name == upstream.branch
                })
            })
    });
    let is_current_branch = section == BranchSection::Local && active_branch_name == Some(&name);
    // Name the branch being moved rather than the opaque "HEAD".
    let current_branch_label = active_branch_name.map_or("HEAD", String::as_str);
    // Git refuses to start a rebase while another rebase, cherry-pick,
    // revert, or unconcluded merge is in flight; grey the entry out instead
    // of letting the click surface that refusal.
    let worktree_finish_busy = this.state.worktree_finish_blocks_workdir_mutation(repo_id);
    let history_rewrite_disabled =
        repo.is_some_and(|r| r.history_rewrite_busy()) || worktree_finish_busy;
    let merge_disabled = worktree_finish_busy
        || !super::super::merge_commit_confirm::merge_commit_is_allowed(
            repo,
            &this.state.large_file_settings,
        );
    let branch_commit_id: Option<CommitId> = match target {
        BranchMenuTarget::Local { name } => repo.and_then(|repo| {
            repo.branches
                .ready()?
                .iter()
                .find(|branch| branch.name == *name)
                .map(|branch| branch.target.clone())
        }),
        BranchMenuTarget::Remote { remote, branch } => repo.and_then(|repo| {
            repo.remote_branches
                .ready()?
                .iter()
                .find(|candidate| candidate.remote == *remote && candidate.name == *branch)
                .map(|candidate| candidate.target.clone())
        }),
    };
    let branch_tip_is_head = repo.and_then(|repo| repo.head_commit_id()) == branch_commit_id;
    // Local Git actions operate on the exact loaded tip. This is independent
    // of the remote-tracking ref's destination, which can be renamed by a
    // custom fetch refspec.
    let action_reference = match section {
        BranchSection::Local => name.clone(),
        BranchSection::Remote => branch_commit_id
            .as_ref()
            .map(|id| id.as_ref().to_string())
            .unwrap_or_else(|| name.clone()),
    };

    let worktree_items = repo
        .map(|repo| worktree_entries(repo, repo_id, target, language))
        .unwrap_or_default();
    let has_open_worktree = worktree_items.iter().any(|item| {
        matches!(item, ContextMenuItem::Entry { action, .. }
            if matches!(action.as_ref(), ContextMenuAction::OpenRepo { .. }))
    });
    // A local branch checked out elsewhere can be opened at its existing
    // path. Git refuses to check it out in the active worktree.
    if section != BranchSection::Local || !has_open_worktree {
        let checkout_disabled = worktree_finish_busy
            || (section == BranchSection::Local
                && this.state.worktree_finish_protects_branch(repo_id, &name));
        items.push(ContextMenuItem::Entry {
            label: language.text("Checkout", "切換分支").into(),
            icon: Some("icons/git_branch.svg".into()),
            shortcut: None,
            disabled: checkout_disabled,
            action: Box::new(match target {
                BranchMenuTarget::Local { name } => ContextMenuAction::CheckoutBranch {
                    repo_id,
                    name: name.clone(),
                },
                BranchMenuTarget::Remote { remote, branch } => ContextMenuAction::OpenPopover {
                    kind: PopoverKind::CheckoutRemoteBranchPrompt {
                        repo_id,
                        remote: remote.clone(),
                        branch: branch.clone(),
                    },
                },
            }),
        });
    }
    for item in worktree_items {
        if let ContextMenuItem::Entry { action, .. } = &item
            && let ContextMenuAction::OpenRepo { path } = action.as_ref()
        {
            branch_entry_tooltips.insert(items.len(), path.display().to_string().into());
            branch_entry_source_labels.insert(items.len(), "Open worktree".into());
        }
        items.push(item);
    }

    if let BranchMenuTarget::Local { name } = target {
        let (entry, unavailable_reason) = push_entry(repo, repo_id, name, language);
        items.push(ContextMenuItem::Separator);
        branch_entry_source_labels.insert(items.len(), "Push".into());
        if let Some(reason) = unavailable_reason {
            branch_entry_tooltips.insert(items.len(), reason.into());
        }
        items.push(entry);
    }

    if !is_current_branch {
        let current_branch = active_branch_name
            .map(String::as_str)
            .filter(|branch| !branch.is_empty() && *branch != "HEAD");
        let mut label_with_current_branch =
            |index, english: &'static str, chinese: &'static str| -> SharedString {
                let label = language.text(english, chinese);
                let Some(branch) = current_branch else {
                    return label.into();
                };
                // Keep action styling and selectors independent of the branch name.
                branch_entry_source_labels.insert(index, english.into());
                let label: SharedString = match language {
                    Language::English => format!("{label} ({branch})"),
                    Language::TraditionalChinese => format!("{label}（{branch}）"),
                }
                .into();
                branch_entry_tooltips.insert(index, label.clone());
                label
            };
        if section == BranchSection::Remote {
            items.push(ContextMenuItem::Separator);
        }
        let (remote, branch) = remote_parts.unwrap_or((".", &name));
        items.push(ContextMenuItem::Entry {
            label: label_with_current_branch(items.len(), "Pull into current", "拉取至目前分支"),
            icon: Some("icons/arrow_down.svg".into()),
            shortcut: None,
            disabled: worktree_finish_busy,
            action: Box::new(ContextMenuAction::PullBranch {
                repo_id,
                remote: remote.to_string(),
                branch: branch.to_string(),
            }),
        });
        if section == BranchSection::Remote {
            if active_branch_has_no_upstream
                && let Some(active_branch_name) = active_branch_name
                && let Some(upstream) = exact_remote_branch
            {
                items.push(ContextMenuItem::Entry {
                    label: language
                        .text("Set as tracking upstream", "設為追蹤的上游分支")
                        .into(),
                    icon: Some("icons/link.svg".into()),
                    shortcut: None,
                    disabled: false,
                    action: Box::new(ContextMenuAction::SetUpstreamBranch {
                        repo_id,
                        branch: active_branch_name.clone(),
                        upstream,
                    }),
                });
            }
            if active_upstream.is_some() {
                items.push(ContextMenuItem::Entry {
                    label: language
                        .text("Unlink upstream branch", "取消追蹤上游分支")
                        .into(),
                    icon: Some("icons/unlink.svg".into()),
                    shortcut: None,
                    disabled: active_upstream != target_upstream.as_ref(),
                    action: Box::new(ContextMenuAction::UnsetUpstreamBranch {
                        repo_id,
                        branch: active_branch_name.cloned().unwrap_or_default(),
                    }),
                });
            }
        }
        items.push(ContextMenuItem::Separator);
        branch_entry_source_labels.insert(items.len(), "Merge into current".into());
        items.push(ContextMenuItem::Entry {
            label: match language {
                Language::English => format!("Merge {name} into {current_branch_label}"),
                Language::TraditionalChinese => format!("將 {name} 合併至 {current_branch_label}"),
            }
            .into(),
            icon: Some("icons/swap.svg".into()),
            shortcut: None,
            disabled: merge_disabled,
            action: Box::new(ContextMenuAction::MergeRef {
                repo_id,
                reference: action_reference.clone(),
            }),
        });
        branch_entry_source_labels.insert(items.len(), "Squash into current".into());
        items.push(ContextMenuItem::Entry {
            label: match language {
                Language::English => format!("Squash {name} into {current_branch_label}"),
                Language::TraditionalChinese => {
                    format!("將 {name} 壓縮合併至 {current_branch_label}")
                }
            }
            .into(),
            icon: Some("icons/arrow_right.svg".into()),
            shortcut: None,
            disabled: merge_disabled,
            action: Box::new(ContextMenuAction::SquashRef {
                repo_id,
                reference: action_reference.clone(),
            }),
        });
        items.push(ContextMenuItem::Entry {
            label: match language {
                Language::English => format!("Rebase {current_branch_label} onto {name}"),
                Language::TraditionalChinese => format!("將 {current_branch_label} 變基至 {name}"),
            }
            .into(),
            icon: Some("icons/arrow_up.svg".into()),
            shortcut: None,
            disabled: history_rewrite_disabled,
            action: Box::new(ContextMenuAction::OpenPopover {
                kind: PopoverKind::RebaseOntoConfirm {
                    repo_id,
                    onto: action_reference.clone(),
                },
            }),
        });
        if include_tip_actions
            && let Some(base) = branch_commit_id.as_ref()
            && !branch_tip_is_head
        {
            items.push(ContextMenuItem::Entry {
                label: match language {
                    Language::English => {
                        format!("Interactive rebase {current_branch_label} onto {name}")
                    }
                    Language::TraditionalChinese => {
                        format!("將 {current_branch_label} 互動式變基至 {name}")
                    }
                }
                .into(),
                icon: Some("icons/refresh.svg".into()),
                shortcut: None,
                disabled: history_rewrite_disabled,
                action: Box::new(ContextMenuAction::LoadInteractiveRebaseSetup {
                    repo_id,
                    base: base.as_ref().to_owned(),
                }),
            });
        }
    }

    items.push(ContextMenuItem::Separator);
    items.push(ContextMenuItem::Entry {
        label: language.text("Create branch", "建立分支").into(),
        icon: Some("icons/plus.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::CreateBranchFromRefPrompt {
                repo_id,
                target: action_reference.clone(),
                source_selectable: false,
                name_prefix: String::new(),
                checkout_on_open: true,
            },
        }),
    });
    // Follow the commit menu's gate for creating tags from history.
    if let Some(tip) = branch_commit_id.as_ref() {
        let tag_disabled = !this.state.git_log_settings.show_history_tags;
        if tag_disabled {
            branch_entry_tooltips.insert(
                items.len(),
                language
                    .text(
                        "Enable “Show tags in history view” in Settings > Git log to add tags.",
                        "請先在「設定 → Git 歷史」啟用「在歷史檢視中顯示標籤」。",
                    )
                    .into(),
            );
        }
        items.push(ContextMenuItem::Entry {
            label: language.text("Create tag…", "建立標籤…").into(),
            icon: Some("icons/tag.svg".into()),
            shortcut: None,
            disabled: tag_disabled,
            action: Box::new(ContextMenuAction::OpenPopover {
                kind: PopoverKind::CreateTagPrompt {
                    repo_id,
                    target: tip.as_ref().to_string(),
                },
            }),
        });
    }

    if include_tip_actions
        && let Some(tip) = branch_commit_id.as_ref()
        && !is_current_branch
        && !branch_tip_is_head
    {
        items.push(ContextMenuItem::Separator);
        items.push(commit::cherry_pick_entry(
            repo_id,
            tip,
            history_rewrite_disabled,
            None,
            language,
        ));
        items.extend(commit::reset_entries(
            repo_id,
            tip,
            history_rewrite_disabled,
            language,
        ));
        items.push(commit::revert_entry(
            repo_id,
            tip,
            history_rewrite_disabled,
            None,
            language,
        ));
    }

    items.push(ContextMenuItem::Separator);
    if section == BranchSection::Local {
        let branch_protected =
            worktree_finish_busy || this.state.worktree_finish_protects_branch(repo_id, &name);
        items.push(ContextMenuItem::Entry {
            label: language.text("Rename branch", "重新命名分支").into(),
            icon: Some("icons/pencil.svg".into()),
            shortcut: None,
            disabled: branch_protected,
            action: Box::new(ContextMenuAction::OpenPopover {
                kind: PopoverKind::RenameBranchPrompt {
                    repo_id,
                    name: name.clone(),
                    is_current_branch: false,
                },
            }),
        });
        items.push(ContextMenuItem::Entry {
            label: language.text("Delete branch", "刪除分支").into(),
            icon: Some("icons/trash.svg".into()),
            shortcut: None,
            disabled: is_current_branch || branch_protected,
            action: Box::new(ContextMenuAction::DeleteBranch {
                repo_id,
                name: name.clone(),
            }),
        });
    } else if let Some((remote, branch)) = remote_parts {
        items.push(ContextMenuItem::Entry {
            label: language
                .text("Delete remote branch…", "刪除遠端分支…")
                .into(),
            icon: Some("icons/trash.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::OpenPopover {
                kind: PopoverKind::remote(
                    repo_id,
                    RemotePopoverKind::DeleteBranchConfirm {
                        remote: remote.to_string(),
                        branch: branch.to_string(),
                    },
                ),
            }),
        });
    }

    // Comparison: mark this branch's tip, or compare it against a mark.
    if let Some(commit_id) = branch_commit_id.as_ref() {
        items.push(ContextMenuItem::Separator);
        let comparison_mark = repo.and_then(|r| r.navigation.comparison_mark.clone());
        items.push(ContextMenuItem::Entry {
            label: match language {
                Language::English => format!("Mark {name} for comparison"),
                Language::TraditionalChinese => format!("標記 {name} 以便比較"),
            }
            .into(),
            icon: Some("icons/git_branch.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::MarkForComparison {
                repo_id,
                commit_id: commit_id.clone(),
                label: name.clone(),
            }),
        });
        items.push(ContextMenuItem::Entry {
            label: language
                .text("Compare with working tree", "與工作樹比較")
                .into(),
            icon: Some("icons/open_external.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::CompareWithWorkingTree {
                repo_id,
                commit_id: commit_id.clone(),
                label: name.clone(),
            }),
        });
        items.extend(super::marked_comparison_entries(
            repo_id,
            commit_id,
            &name,
            comparison_mark
                .as_ref()
                .map(|mark| (&mark.commit_id, mark.label.as_str())),
            language,
        ));
    }

    if !this.extension_ref_menu.is_empty() {
        items.push(ContextMenuItem::Separator);
        items.extend(super::ref_menu::entries(
            this,
            repo_id,
            match target {
                BranchMenuTarget::Local { name } => {
                    RefMenuTarget::LocalBranch { name: name.clone() }
                }
                BranchMenuTarget::Remote { remote, branch } => RefMenuTarget::RemoteBranch {
                    remote: remote.clone(),
                    branch: branch.clone(),
                },
            },
        ));
    }

    items.push(ContextMenuItem::Separator);
    items.push(super::focus::entry(
        this,
        repo_id,
        match target {
            BranchMenuTarget::Local { name } => {
                gitcomet_core::domain::HistoryFocus::local_branch(name)
            }
            BranchMenuTarget::Remote { remote, branch } => {
                gitcomet_core::domain::HistoryFocus::remote_branch(remote, branch)
            }
        },
        language,
    ));
    let pinned = this.is_branch_pinned(repo_id, section, &name);
    items.push(ContextMenuItem::Entry {
        label: if pinned {
            language.text("Unpin branch", "取消釘選分支").into()
        } else {
            language.text("Pin branch", "釘選分支").into()
        },
        icon: Some("icons/pin.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::ToggleBranchPin {
            repo_id,
            section,
            name: name.clone(),
        }),
    });
    items.push(ContextMenuItem::Entry {
        label: language.text("Copy branch name", "複製分支名稱").into(),
        icon: Some("icons/copy.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::CopyText { text: name.clone() }),
    });
    if include_tip_actions && let Some(tip) = branch_commit_id {
        items.push(ContextMenuItem::Entry {
            label: language.text("Copy commit SHA", "複製提交 SHA").into(),
            icon: Some("icons/copy.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::CopyCommitSha {
                commit_id: tip.clone(),
            }),
        });
        items.push(ContextMenuItem::Entry {
            label: language.text("Export patch…", "匯出修補檔…").into(),
            icon: Some("icons/arrow_down.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::ExportPatch {
                repo_id,
                commit_id: tip,
            }),
        });
    }

    if section == BranchSection::Remote {
        items.push(ContextMenuItem::Separator);
        items.push(ContextMenuItem::Entry {
            label: language.text("Fetch all", "擷取全部").into(),
            icon: Some("icons/arrow_down.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::FetchAll { repo_id }),
        });
        items.push(ContextMenuItem::Entry {
            label: language
                .text("Prune merged branches", "清除已合併的分支")
                .into(),
            icon: Some("icons/broom.svg".into()),
            shortcut: None,
            disabled: this.state.has_active_worktree_finish_run(repo_id),
            action: Box::new(ContextMenuAction::PruneMergedBranches { repo_id }),
        });
        items.push(ContextMenuItem::Entry {
            label: language.text("Prune local tags", "清理本機標籤").into(),
            icon: Some("icons/tag.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::PruneLocalTags { repo_id }),
        });
    }

    let mut model = ContextMenuModel::new(items).with_entry_tooltips(branch_entry_tooltips);
    model.entry_source_labels = branch_entry_source_labels;
    model
}

fn push_entry(
    repo: Option<&RepoState>,
    repo_id: RepoId,
    branch: &str,
    language: Language,
) -> (ContextMenuItem, Option<&'static str>) {
    let local = repo
        .and_then(|repo| repo.branches.ready())
        .and_then(|branches| branches.iter().find(|candidate| candidate.name == branch));
    let remotes = repo.and_then(|repo| repo.remotes.ready());
    let upstream = local.and_then(|local| local.upstream.as_ref());
    let preferred_remote = repo
        .and_then(|repo| super::super::push_set_upstream_prompt::selected_remote(repo, "origin"));
    let unavailable_reason = if local.is_none() || remotes.is_none() {
        Some(language.text("Branch or remotes unavailable", "無法取得分支或遠端"))
    } else if repo.is_some_and(|repo| repo.local_branch_push_blocked_by_annex(branch)) {
        Some(language.text(
            "Use git-annex push on an adjusted branch",
            "調整分支須使用 git-annex 推送",
        ))
    } else if let Some(upstream) = upstream {
        (!remotes
            .is_some_and(|remotes| remotes.iter().any(|remote| remote.name == upstream.remote)))
        .then(|| language.text("The upstream remote is not configured", "上游遠端未設定"))
    } else {
        preferred_remote
            .is_none()
            .then(|| language.text("Add a remote to push to first", "請先新增可推送的遠端"))
    };
    let (label, action) = match upstream {
        Some(upstream) => (
            format!(
                "{} {}/{}",
                language.text("Push to", "推送至"),
                upstream.remote,
                upstream.branch
            )
            .into(),
            ContextMenuAction::PushBranch {
                repo_id,
                local_branch: branch.to_string(),
                upstream: upstream.clone(),
            },
        ),
        None => (
            language.text("Push branch…", "推送分支…").into(),
            ContextMenuAction::OpenPopover {
                kind: PopoverKind::PushSetUpstreamPrompt {
                    repo_id,
                    remote: preferred_remote.unwrap_or_default(),
                    mode: PushUpstreamMode::SelectedBranch(branch.to_string()),
                },
            },
        ),
    };
    (
        ContextMenuItem::Entry {
            label,
            icon: Some("icons/arrow_up.svg".into()),
            shortcut: None,
            disabled: unavailable_reason.is_some(),
            action: Box::new(action),
        },
        unavailable_reason,
    )
}

/// What a new worktree for the branch starts from.
enum WorktreeStart<'a> {
    /// A local branch: checked out as-is, or the start of a new branch when
    /// it is checked out somewhere already.
    Local(String),
    /// A remote branch no local branch tracks yet.
    Track { remote: &'a str, branch: &'a str },
}

/// Starts a new worktree for the branch, after opening the other worktree
/// that has it checked out, if any.
fn worktree_entries(
    repo: &RepoState,
    repo_id: RepoId,
    target: &BranchMenuTarget,
    language: Language,
) -> Vec<ContextMenuItem> {
    let start = match target {
        BranchMenuTarget::Local { name } => WorktreeStart::Local(name.clone()),
        BranchMenuTarget::Remote { remote, branch } => repo
            .branches
            .ready()
            .and_then(|branches| {
                branches.iter().find(|candidate| {
                    candidate.upstream.as_ref().is_some_and(|upstream| {
                        upstream.remote == *remote && upstream.branch == *branch
                    })
                })
            })
            .map_or(WorktreeStart::Track { remote, branch }, |local| {
                WorktreeStart::Local(local.name.clone())
            }),
    };
    let local = match &start {
        WorktreeStart::Local(local) => Some(local.as_str()),
        WorktreeStart::Track { .. } => None,
    };
    let checked_out_path =
        local.and_then(|local| crate::view::rows::branch_checked_out_at(repo, local));
    // Git checks a branch out in one worktree only.
    let checked_out = checked_out_path.is_some();
    let mut items = Vec::new();
    if let Some(path) = checked_out_path.filter(|path| path != &repo.spec.workdir) {
        items.push(ContextMenuItem::Entry {
            label: language.text("Open worktree", "開啟工作樹").into(),
            icon: Some("icons/git_worktree.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::OpenRepo { path }),
        });
    }
    let prefill = match start {
        WorktreeStart::Local(local) => WorktreeAddPrefill {
            source: local,
            source_upstream: None,
            create_branch: checked_out,
            new_branch: String::new(),
            open_after: true,
        },
        WorktreeStart::Track { remote, branch } => WorktreeAddPrefill {
            source: format!("{remote}/{branch}"),
            source_upstream: Some(Upstream {
                remote: remote.to_string(),
                branch: branch.to_string(),
            }),
            create_branch: true,
            new_branch: branch.to_string(),
            open_after: true,
        },
    };
    items.push(ContextMenuItem::Entry {
        label: language
            .text("Open in new worktree", "在新工作樹中開啟")
            .into(),
        icon: Some("icons/git_worktree.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::OpenWorktreeAddPrompt { repo_id, prefill }),
    });
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitcomet_core::domain::{Branch, RepoSpec, Worktree};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn branch(name: &str, upstream: Option<(&str, &str)>) -> Branch {
        Branch {
            name: name.to_string(),
            target: CommitId(Arc::from("1111111111111111111111111111111111111111")),
            upstream: upstream.map(|(remote, branch)| Upstream {
                remote: remote.to_string(),
                branch: branch.to_string(),
            }),
            divergence: None,
        }
    }

    fn worktree(path: &str, branch: &str) -> Worktree {
        Worktree {
            bare: false,
            path: PathBuf::from(path),
            head: None,
            branch: Some(branch.to_string()),
            detached: false,
        }
    }

    /// `/tmp/ws/main` on `main`, with `feat/held` checked out in `/tmp/ws/held`.
    fn repo() -> RepoState {
        let mut repo = RepoState::new_opening(
            RepoId(1),
            RepoSpec {
                workdir: PathBuf::from("/tmp/ws/main"),
            },
        );
        repo.head_branch = Loadable::Ready("main".to_string());
        repo.branches = Loadable::Ready(Arc::new(vec![
            branch("main", Some(("origin", "main"))),
            branch("feat/free", None),
            branch("feat/held", Some(("origin", "held"))),
            branch("local-x", Some(("origin", "x"))),
        ]));
        repo.worktrees = Loadable::Ready(Arc::new(vec![
            worktree("/tmp/ws/main", "main"),
            worktree("/tmp/ws/held", "feat/held"),
        ]));
        repo
    }

    fn local(name: &str) -> BranchMenuTarget {
        BranchMenuTarget::Local {
            name: name.to_string(),
        }
    }

    fn remote(remote: &str, branch: &str) -> BranchMenuTarget {
        BranchMenuTarget::Remote {
            remote: remote.to_string(),
            branch: branch.to_string(),
        }
    }

    /// Each entry's label, whether it is disabled, and its action.
    fn entries_of(
        repo: &RepoState,
        target: BranchMenuTarget,
    ) -> Vec<(String, bool, ContextMenuAction)> {
        worktree_entries(repo, RepoId(1), &target, Language::English)
            .into_iter()
            .map(|item| match item {
                ContextMenuItem::Entry {
                    label,
                    disabled,
                    action,
                    ..
                } => (label.to_string(), disabled, *action),
                _ => panic!("the worktree items are entries"),
            })
            .collect()
    }

    fn entry(target: BranchMenuTarget) -> (String, bool, ContextMenuAction) {
        let [entry] = <[_; 1]>::try_from(entries_of(&repo(), target))
            .unwrap_or_else(|_| panic!("one worktree entry"));
        entry
    }

    /// The "Open worktree" entry, then the "Open in new worktree" one.
    fn open_and_new_entries(target: BranchMenuTarget) -> [(String, bool, ContextMenuAction); 2] {
        <[_; 2]>::try_from(entries_of(&repo(), target))
            .unwrap_or_else(|_| panic!("two worktree entries"))
    }

    /// A new branch off `source`, named after the new folder.
    fn new_branch_from(source: &str) -> WorktreeAddPrefill {
        WorktreeAddPrefill {
            source: source.to_string(),
            source_upstream: None,
            create_branch: true,
            new_branch: String::new(),
            open_after: true,
        }
    }

    fn prefill(action: ContextMenuAction) -> WorktreeAddPrefill {
        match action {
            ContextMenuAction::OpenWorktreeAddPrompt { repo_id, prefill } => {
                assert_eq!(repo_id, RepoId(1));
                prefill
            }
            _ => panic!("expected the Add-worktree action"),
        }
    }

    #[test]
    fn a_free_local_branch_is_checked_out_in_a_new_worktree() {
        let (label, disabled, action) = entry(local("feat/free"));
        assert_eq!(label, "Open in new worktree");
        assert_eq!(
            crate::view::menu_presentation::context_menu_tone(
                &label,
                Some("icons/git_worktree.svg")
            ),
            gitcomet_ui_kit::components::ContextMenuTone::Success,
            "the menu's own label keeps the green icon"
        );
        assert!(!disabled);
        assert_eq!(
            prefill(action),
            WorktreeAddPrefill {
                source: "feat/free".to_string(),
                source_upstream: None,
                create_branch: false,
                new_branch: String::new(),
                open_after: true,
            }
        );
    }

    #[test]
    fn the_current_branch_starts_a_new_branch_in_a_new_worktree() {
        let (label, disabled, action) = entry(local("main"));
        assert_eq!(label, "Open in new worktree");
        assert!(!disabled);
        assert_eq!(
            prefill(action),
            new_branch_from("main"),
            "git refuses to check main out a second time"
        );
    }

    #[test]
    fn the_current_branch_starts_a_new_branch_before_worktrees_load() {
        let mut repo = repo();
        repo.worktrees = Loadable::NotLoaded;
        let [(_, disabled, action)] = <[_; 1]>::try_from(entries_of(&repo, local("main")))
            .unwrap_or_else(|_| panic!("one worktree entry"));
        assert!(!disabled);
        assert_eq!(prefill(action), new_branch_from("main"));
    }

    #[test]
    fn a_branch_held_by_another_worktree_opens_it_or_starts_a_new_branch() {
        let [(open_label, _, open), (new_label, disabled, new)] =
            open_and_new_entries(local("feat/held"));
        assert_eq!(open_label, "Open worktree");
        assert!(matches!(
            open,
            ContextMenuAction::OpenRepo { path } if path == std::path::Path::new("/tmp/ws/held")
        ));
        assert_eq!(new_label, "Open in new worktree");
        assert!(!disabled);
        assert_eq!(prefill(new), new_branch_from("feat/held"));
    }

    #[test]
    fn a_remote_branch_reuses_the_local_branch_tracking_it() {
        let (_, disabled, action) = entry(remote("origin", "x"));
        assert!(!disabled);
        let prefill = prefill(action);
        assert_eq!(prefill.source, "local-x");
        assert!(!prefill.create_branch);
    }

    #[test]
    fn a_remote_branch_whose_tracking_branch_is_held_opens_it_or_starts_a_new_branch() {
        let [(open_label, _, open), (_, _, new)] = open_and_new_entries(remote("origin", "held"));
        assert_eq!(open_label, "Open worktree");
        assert!(matches!(
            open,
            ContextMenuAction::OpenRepo { path } if path == std::path::Path::new("/tmp/ws/held")
        ));
        assert_eq!(prefill(new), new_branch_from("feat/held"));
    }

    #[test]
    fn an_untracked_remote_branch_gets_a_new_tracking_branch() {
        let (label, disabled, action) = entry(remote("origin", "feat/new"));
        assert_eq!(label, "Open in new worktree");
        assert!(!disabled);
        assert_eq!(
            prefill(action),
            WorktreeAddPrefill {
                source: "origin/feat/new".to_string(),
                source_upstream: Some(Upstream {
                    remote: "origin".into(),
                    branch: "feat/new".into()
                }),
                create_branch: true,
                new_branch: "feat/new".to_string(),
                open_after: true,
            }
        );
    }

    #[test]
    fn tracking_prefill_keeps_typed_remote_identity_when_display_names_collide() {
        let left = prefill(entry(remote("team/origin", "topic")).2);
        let right = prefill(entry(remote("team", "origin/topic")).2);
        assert_eq!(left.source, right.source);
        assert_eq!(
            left.source_upstream,
            Some(Upstream {
                remote: "team/origin".into(),
                branch: "topic".into()
            })
        );
        assert_eq!(
            right.source_upstream,
            Some(Upstream {
                remote: "team".into(),
                branch: "origin/topic".into()
            })
        );
        assert_ne!(left.source_upstream, right.source_upstream);
    }
}
