use super::*;
use crate::localization::Language;
use gitcomet_core::services::{GitRepository, InteractiveRebaseAction, InteractiveRebaseEntry};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;
use std::sync::Arc;

struct CommitTitleCopyTask {
    _task: gpui::Task<()>,
}

impl gpui::Global for CommitTitleCopyTask {}

fn read_titles(repo: &dyn GitRepository, commit_ids: &[CommitId]) -> Result<String, String> {
    let mut titles = String::new();
    // Keep peak memory bounded even if a large selection has long message bodies.
    for batch in commit_ids.chunks(128) {
        let messages = repo
            .commit_messages(batch)
            .map_err(|error| error.to_string())?;
        for title in messages
            .iter()
            .filter_map(|message| message.lines().next())
            .filter(|title| !title.trim().is_empty())
        {
            if !titles.is_empty() {
                titles.push('\n');
            }
            titles.push_str(title);
        }
    }
    Ok(titles)
}

pub(super) fn copy_titles(
    this: &mut PopoverHost,
    repo_id: RepoId,
    mut commit_ids: Arc<Vec<CommitId>>,
    cx: &mut gpui::Context<PopoverHost>,
) {
    let Some(repo) = this.state.repos.iter().find(|repo| repo.id == repo_id) else {
        return;
    };
    let page = crate::view::panes::history::HistoryView::display_log_page_for_repo(repo);
    let indexed = &repo.history_state.indexed;
    let index = indexed.displayed_index.clone().or_else(|| {
        if page.is_none() {
            indexed.range_index.clone()
        } else {
            None
        }
    });
    let lifetime = repo.lifetime();
    let store = this.store.clone();
    let language = Language::current(cx);
    let clipboard_rev = crate::clipboard::files_revision();
    let read = cx.background_executor().spawn(async move {
        if commit_ids.len() > 1 {
            let ids = Arc::make_mut(&mut commit_ids);
            if let Some(index) = index {
                ids.sort_by_cached_key(|id| index.position(id.as_ref()).unwrap_or(usize::MAX));
            } else if let Some(page) = page {
                let positions: FxHashMap<_, _> = page
                    .commits
                    .iter()
                    .enumerate()
                    .map(|(row, commit)| (&commit.id, row))
                    .collect();
                ids.sort_by_cached_key(|id| positions.get(id).copied().unwrap_or(usize::MAX));
            }
        }
        let repository = store.repository(repo_id, lifetime).ok_or_else(|| {
            language
                .text("The repository is unavailable.", "無法存取儲存庫。")
                .to_owned()
        })?;
        read_titles(repository.as_ref(), &commit_ids)
    });
    let task = cx.spawn(async move |this, cx| {
        let result = read.await;
        let _ = this.update(cx, |this, cx| {
            // A copy made while the objects were loading takes precedence.
            if crate::clipboard::files_revision() != clipboard_rev {
                return;
            }
            match result {
                Ok(titles) if !titles.is_empty() => {
                    crate::clipboard::write_text(
                        cx,
                        titles,
                        crate::clipboard::CopySource::CommitTitle,
                    );
                }
                Ok(_) => this.push_toast(
                    components::ToastKind::Warning,
                    language
                        .text("No commit titles to copy.", "沒有可複製的提交標題。")
                        .to_owned(),
                    cx,
                ),
                Err(error) => this.push_toast(
                    components::ToastKind::Error,
                    format!(
                        "{} {error}",
                        language.text("Could not copy commit titles.", "無法複製提交標題。")
                    ),
                    cx,
                ),
            }
        });
    });
    // A newer title copy replaces the pending one, including across windows.
    cx.set_global(CommitTitleCopyTask { _task: task });
}

type InteractiveRebaseSourceColor = (String, u8);
type MultiCherryPickPlan = (
    Vec<InteractiveRebaseEntry>,
    Vec<InteractiveRebaseSourceColor>,
);

fn multi_cherry_pick_plan(
    this: &PopoverHost,
    repo_id: RepoId,
    commit_id: &CommitId,
) -> Option<MultiCherryPickPlan> {
    let repo = this.active_repo().filter(|repo| repo.id == repo_id)?;
    let selection = &repo.history_state.multi_selection;
    if !(selection.is_multi() && selection.contains(commit_id)) {
        return None;
    }
    if let Some(index) = &repo.history_state.indexed.range_index {
        let mut selected: Vec<_> = selection
            .commits
            .iter()
            .filter_map(|id| index.position(id.as_ref()).map(|row| (row, id)))
            .collect();
        if selected.len() == selection.commits.len() {
            selected.sort_unstable_by_key(|(row, _)| std::cmp::Reverse(*row));
            let entries = selected
                .iter()
                .map(|(row, id)| {
                    let summary = repo
                        .history_state
                        .indexed
                        .commit(&index.snapshot, *row)
                        .map_or_else(String::new, |commit| commit.summary.to_string());
                    InteractiveRebaseEntry {
                        action: InteractiveRebaseAction::Pick,
                        commit_id: id.as_ref().to_owned(),
                        message: summary.clone(),
                        summary,
                        new_message: None,
                    }
                })
                .collect();
            let colors = selected
                .iter()
                .map(|(_, id)| (id.as_ref().to_owned(), 0))
                .collect();
            return Some((entries, colors));
        }
        // Never silently cherry-pick just the loaded part of a selection.
        return None;
    }
    let Loadable::Ready(page) = &repo.log else {
        return None;
    };

    let mut selected = page
        .commits
        .iter()
        .enumerate()
        .filter(|(_, commit)| selection.contains(&commit.id))
        .collect::<Vec<_>>();
    if selected.len() < 2 {
        return None;
    }

    let branches = repo.branches.ready().map_or(&[][..], |b| b.as_slice());
    let stashes = repo.stashes.ready().map_or(&[][..], |s| s.as_slice());
    let remote_branches = repo.history_remote_branches();
    // The colours of the rows the history list shows for these commits.
    let graph = crate::view::panes::history::build_history_page_graph(
        repo.history_state.history_scope,
        &repo.history_state.history_focus,
        repo.detached_head_commit.as_ref(),
        repo.head_branch.ready().map(String::as_str),
        branches,
        &remote_branches,
        stashes,
        &page.commits,
        this.theme,
    );
    // Use reversed page order as the stable oldest-first baseline. Before the
    // editor becomes available, the repository-backed setup load follows the
    // complete graph through unselected commits and corrects any ancestry
    // inversions caused by clock skew.
    selected.reverse();

    let mut entries = Vec::with_capacity(selected.len());
    let mut source_colors = Vec::with_capacity(selected.len());
    for (page_ix, commit) in selected {
        entries.push(InteractiveRebaseEntry {
            action: InteractiveRebaseAction::Pick,
            commit_id: commit.id.as_ref().to_string(),
            summary: commit.summary.to_string(),
            // The log page only carries the subject; the state layer loads
            // the full messages right after the setup opens so a reword
            // edit does not start from a body-less seed.
            message: commit.summary.to_string(),
            new_message: None,
        });
        let color_ix = graph
            .row_for_commit_ix(page_ix)
            .map(|row| row.node_color_ix)
            .unwrap_or(0);
        source_colors.push((commit.id.as_ref().to_string(), color_ix));
    }

    Some((entries, source_colors))
}

/// Returns `true` when `commit_id` is already reachable from HEAD — i.e. it is
/// part of the current branch's history (including HEAD itself). The check walks
/// the parents of the loaded log page from HEAD. The right-clicked commit is by
/// construction present in that page, so a path through the page reaching it
/// proves ancestry; a truncated page can only produce a false negative, which
/// git corrects at merge time with "Already up to date".
fn commit_is_ancestor_of_head(this: &PopoverHost, repo_id: RepoId, commit_id: &CommitId) -> bool {
    let Some(repo) = this.state.repos.iter().find(|repo| repo.id == repo_id) else {
        return false;
    };
    repo_commit_is_ancestor_of_head(repo, commit_id)
}

fn repo_commit_is_ancestor_of_head(repo: &RepoState, commit_id: &CommitId) -> bool {
    let Some(head) = repo.head_commit_id() else {
        return false;
    };
    if head == *commit_id {
        return true;
    }
    let Loadable::Ready(page) = &repo.log else {
        return false;
    };
    let mut by_id: FxHashMap<CommitId, &Commit> = FxHashMap::default();
    for commit in &page.commits {
        by_id.insert(commit.id.clone(), commit);
    }
    let mut seen: FxHashSet<CommitId> = FxHashSet::default();
    let mut queue: VecDeque<CommitId> = VecDeque::from([head]);
    while let Some(current) = queue.pop_front() {
        if current == *commit_id {
            return true;
        }
        if !seen.insert(current.clone()) {
            continue;
        }
        if let Some(commit) = by_id.get(&current) {
            for parent in &commit.parent_ids {
                queue.push_back(parent.clone());
            }
        }
    }
    false
}

/// Above this many branches on a commit, "Copy branch name" lives only in each
/// branch's group of the commit menu.
const COMMIT_MENU_MAX_BRANCH_COPY_ENTRIES: usize = 3;

/// Branch names a commit's menu offers to copy: the local branches pointing at
/// it, or, when there are none, its remote branches as `remote/branch` (the
/// same text the branch's own menu copies). Only branches `refs` admits.
fn commit_branch_names_to_copy(
    this: &PopoverHost,
    repo_id: RepoId,
    commit_id: &CommitId,
    refs: &CommitMenuRefs,
) -> Vec<String> {
    let Some(repo) = this.state.repos.iter().find(|repo| repo.id == repo_id) else {
        return Vec::new();
    };
    let local: Vec<String> = repo
        .branches
        .ready()
        .map(|branches| {
            branches
                .iter()
                .filter(|branch| {
                    branch.target == *commit_id
                        && refs.admits(&HistoryMenuRef::Branch(BranchMenuTarget::local(
                            &branch.name,
                        )))
                })
                .map(|branch| branch.name.clone())
                .collect()
        })
        .unwrap_or_default();
    if !local.is_empty() {
        return local;
    }
    repo.history_remote_branches_iter()
        .filter(|branch| {
            branch.target == *commit_id
                && refs.admits(&HistoryMenuRef::Branch(BranchMenuTarget::remote(
                    &branch.remote,
                    &branch.name,
                )))
        })
        .map(|branch| format!("{}/{}", branch.remote, branch.name))
        .collect()
}

pub(super) fn model(
    this: &PopoverHost,
    repo_id: RepoId,
    commit_id: &CommitId,
    refs: &CommitMenuRefs,
    language: Language,
) -> ContextMenuModel {
    model_with_header(this, repo_id, commit_id, refs, true, language)
}

pub(super) fn action_items(
    this: &PopoverHost,
    repo_id: RepoId,
    commit_id: &CommitId,
    language: Language,
) -> ContextMenuModel {
    model_with_header(
        this,
        repo_id,
        commit_id,
        &CommitMenuRefs::All,
        false,
        language,
    )
}

pub(super) fn cherry_pick_entry(
    repo_id: RepoId,
    commit_id: &CommitId,
    disabled: bool,
    shortcut: Option<&'static str>,
    language: Language,
) -> ContextMenuItem {
    ContextMenuItem::Entry {
        label: language
            .text("Cherry-pick", "摘取提交（Cherry-pick）")
            .into(),
        icon: Some("icons/arrow_up.svg".into()),
        shortcut: shortcut.map(Into::into),
        disabled,
        action: Box::new(ContextMenuAction::CherryPickCommit {
            repo_id,
            commit_id: commit_id.clone(),
        }),
    }
}

pub(super) fn revert_entry(
    repo_id: RepoId,
    commit_id: &CommitId,
    disabled: bool,
    shortcut: Option<&'static str>,
    language: Language,
) -> ContextMenuItem {
    let sha = commit_id.as_ref();
    let short = sha.get(..8).unwrap_or(sha);
    ContextMenuItem::Entry {
        label: format!("{} {short}…", language.text("Revert", "還原")).into(),
        icon: Some("icons/undo.svg".into()),
        shortcut: shortcut.map(Into::into),
        disabled,
        action: Box::new(ContextMenuAction::RevertCommit {
            repo_id,
            commit_id: commit_id.clone(),
        }),
    }
}

pub(super) fn reset_entries(
    repo_id: RepoId,
    commit_id: &CommitId,
    disabled: bool,
    language: Language,
) -> [ContextMenuItem; 3] {
    [
        (
            language.text("Reset (--soft) to here", "軟重設（--soft）至此提交"),
            ResetMode::Soft,
        ),
        (
            language.text("Reset (--mixed) to here", "混合重設（--mixed）至此提交"),
            ResetMode::Mixed,
        ),
        (
            language.text("Reset (--hard) to here", "硬重設（--hard）至此提交"),
            ResetMode::Hard,
        ),
    ]
    .map(|(label, mode)| ContextMenuItem::Entry {
        label: label.into(),
        icon: Some("icons/refresh.svg".into()),
        shortcut: None,
        disabled,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::ResetPrompt {
                repo_id,
                target: commit_id.as_ref().to_string(),
                mode,
            },
        }),
    })
}

fn model_with_header(
    this: &PopoverHost,
    repo_id: RepoId,
    commit_id: &CommitId,
    refs: &CommitMenuRefs,
    include_header: bool,
    language: Language,
) -> ContextMenuModel {
    let sha = commit_id.as_ref().to_string();
    let short: SharedString = sha.get(0..8).unwrap_or(&sha).to_string().into();

    let commit_summary = this
        .active_repo()
        .and_then(|r| {
            if let Some(index) = &r.history_state.indexed.range_index
                && let Some(row) = index.position(commit_id.as_ref())
                && let Some(commit) = r.history_state.indexed.commit(&index.snapshot, row)
            {
                return Some(format!("{} — {}", commit.author, commit.summary));
            }
            match &r.log {
                Loadable::Ready(page) => page
                    .commits
                    .iter()
                    .find(|c| c.id == *commit_id)
                    .map(|c| format!("{} — {}", c.author, c.summary)),
                _ => None,
            }
        })
        .unwrap_or_default();

    let branch_names: Vec<String> = this
        .active_repo()
        .and_then(|r| match &r.branches {
            Loadable::Ready(branches) => Some(
                branches
                    .iter()
                    .filter(|b| {
                        b.target == *commit_id
                            && refs
                                .admits(&HistoryMenuRef::Branch(BranchMenuTarget::local(&b.name)))
                    })
                    .map(|b| b.name.clone())
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default();

    let header_text: SharedString = match branch_names.as_slice() {
        [] => format!("{} {short}", language.text("Commit", "提交")).into(),
        [name] => name.clone().into(),
        names => names.join(", ").into(),
    };
    let mut items = Vec::new();
    if include_header {
        items.push(ContextMenuItem::Header(
            components::ContextMenuText::new(header_text).max_lines(2),
        ));
    }
    let mut entry_tooltips = FxHashMap::default();
    if include_header && !commit_summary.is_empty() {
        items.push(ContextMenuItem::Label(
            components::ContextMenuText::new(commit_summary).max_lines(4),
        ));
    }
    if include_header {
        items.push(ContextMenuItem::Separator);
    }
    let multi_cherry_pick_plan = multi_cherry_pick_plan(this, repo_id, commit_id);
    let has_multi_cherry_pick = multi_cherry_pick_plan.is_some();
    let is_head_commit = this
        .active_repo()
        .filter(|repo| repo.id == repo_id)
        .and_then(|repo| repo.head_commit_id())
        .is_some_and(|head| head == *commit_id);
    // Cherry-pick, revert, squash, and rebase all contend for git's single
    // sequencer slot, so any in-flight operation (or a merge waiting to be
    // concluded) disables starting every one of them, not just its own kind.
    let worktree_finish_busy = this.state.worktree_finish_blocks_workdir_mutation(repo_id);
    let history_rewrite_disabled = this
        .active_repo()
        .filter(|repo| repo.id == repo_id)
        .is_some_and(|repo| repo.history_rewrite_busy())
        || worktree_finish_busy;

    // "Squash N commits" appears only when the right-clicked commit is part
    // of the active multi-selection and the whole selection passes the squash
    // criteria (contiguous linear first-parent chain, non-root base). The
    // range may end at HEAD or sit anywhere in the chain.
    let squash_plan = this
        .active_repo()
        .filter(|repo| repo.id == repo_id)
        .and_then(|repo| {
            let selection = &repo.history_state.multi_selection;
            if !(selection.is_multi() && selection.contains(commit_id)) {
                return None;
            }
            repo.history_squash_plan()
        });
    if let Some(plan) = squash_plan {
        let label = match language {
            Language::English => format!("Squash {} commits", plan.commit_count),
            Language::TraditionalChinese => format!("壓縮合併 {} 筆提交", plan.commit_count),
        };
        items.push(ContextMenuItem::Entry {
            label: label.into(),
            // The branch menu's "Squash into current" icon.
            icon: Some("icons/arrow_right.svg".into()),
            shortcut: None,
            disabled: history_rewrite_disabled,
            action: Box::new(ContextMenuAction::SquashSelectedCommits { repo_id }),
        });
        items.push(ContextMenuItem::Separator);
    }
    if !is_head_commit && let Some((entries, source_colors)) = multi_cherry_pick_plan {
        let label = match language {
            Language::English => format!("Cherry-pick {} commits…", entries.len()),
            Language::TraditionalChinese => format!("摘取 {} 筆提交…", entries.len()),
        };
        items.push(ContextMenuItem::Entry {
            label: label.into(),
            icon: Some("icons/arrow_up.svg".into()),
            shortcut: Some("P".into()),
            disabled: history_rewrite_disabled,
            action: Box::new(ContextMenuAction::OpenInteractiveCherryPickSetup {
                repo_id,
                entries,
                source_colors,
            }),
        });
        items.push(ContextMenuItem::Separator);
    }
    items.push(ContextMenuItem::Entry {
        label: language.text("Start file browsing", "開始瀏覽檔案").into(),
        icon: Some("icons/history.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::BrowseRepositoryAtCommit {
            repo_id,
            commit_id: commit_id.clone(),
        }),
    });
    // The file history menu embeds these actions without the header and
    // offers its own "Copy SHA" among its file entries.
    if include_header {
        items.push(ContextMenuItem::Entry {
            label: language.text("Copy commit SHA", "複製提交 SHA").into(),
            icon: Some("icons/copy.svg".into()),
            shortcut: Some("C".into()),
            disabled: false,
            action: Box::new(ContextMenuAction::CopyCommitSha {
                commit_id: commit_id.clone(),
            }),
        });
        let mut names = commit_branch_names_to_copy(this, repo_id, commit_id, refs);
        // A busy commit would push its own actions far down the menu with one
        // entry per branch. Each branch's group offers the copy too, as
        // GitKraken keeps it in the branch's submenu.
        if names.len() > COMMIT_MENU_MAX_BRANCH_COPY_ENTRIES {
            names.clear();
        }
        let single = names.len() == 1;
        for name in names {
            items.push(ContextMenuItem::Entry {
                label: if single {
                    language.text("Copy branch name", "複製分支名稱").into()
                } else {
                    format!(
                        "{} {name}",
                        language.text("Copy branch name", "複製分支名稱")
                    )
                    .into()
                },
                icon: Some("icons/copy.svg".into()),
                shortcut: None,
                disabled: false,
                action: Box::new(ContextMenuAction::CopyText { text: name }),
            });
        }
        let commit_ids = this
            .active_repo()
            .filter(|repo| repo.id == repo_id && *refs == CommitMenuRefs::All)
            .map(|repo| &repo.history_state.multi_selection)
            .filter(|selection| selection.is_multi() && selection.contains(commit_id))
            .map_or_else(
                || Arc::new(vec![commit_id.clone()]),
                |selection| Arc::clone(&selection.commits),
            );
        let label = if commit_ids.len() > 1 {
            language.text("Copy selected commit titles", "複製所選提交標題")
        } else {
            language.text("Copy commit title", "複製提交標題")
        };
        items.push(ContextMenuItem::Entry {
            label: label.into(),
            icon: Some("icons/copy.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::CopyCommitTitles {
                repo_id,
                commit_ids,
            }),
        });
    }
    if let Some(permalink) = this
        .state
        .repos
        .iter()
        .find(|repo| repo.id == repo_id)
        .and_then(|repo| match &repo.remotes {
            Loadable::Ready(remotes) => crate::view::permalink::commit_permalink(remotes, &sha),
            _ => None,
        })
    {
        items.push(ContextMenuItem::Entry {
            label: language
                .text("Copy commit permalink", "複製提交永久連結")
                .into(),
            icon: Some("icons/copy.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::CopyText { text: permalink }),
        });
    }
    // Comparison: mark this commit as a base, or compare it against a mark.
    // Look the repo up by id rather than via `active_repo`, so a menu opened for
    // a non-active repo offers the same comparison entries the branch and tag
    // menus do — the dispatched `Msg` carries `repo_id` and works either way.
    let comparison_mark = this
        .state
        .repos
        .iter()
        .find(|repo| repo.id == repo_id)
        .and_then(|repo| repo.navigation.comparison_mark.clone());
    items.push(ContextMenuItem::Entry {
        label: match language {
            Language::English => format!("Mark {short} for comparison"),
            Language::TraditionalChinese => format!("標記 {short} 以便比較"),
        }
        .into(),
        icon: Some("icons/git_commit.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::MarkForComparison {
            repo_id,
            commit_id: commit_id.clone(),
            label: short.to_string(),
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
            label: short.to_string(),
        }),
    });
    items.extend(super::marked_comparison_entries(
        repo_id,
        commit_id,
        short.as_ref(),
        comparison_mark
            .as_ref()
            .map(|mark| (&mark.commit_id, mark.label.as_str())),
        language,
    ));
    items.extend(super::ref_menu::entries(
        this,
        repo_id,
        gitcomet_extension_api::RefMenuTarget::Commit {
            id: commit_id.clone(),
        },
    ));
    items.push(ContextMenuItem::Entry {
        label: language.text("Export patch…", "匯出修補檔…").into(),
        icon: Some("icons/arrow_down.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::ExportPatch {
            repo_id,
            commit_id: commit_id.clone(),
        }),
    });
    let add_tag_disabled = !this.state.git_log_settings.show_history_tags;
    let add_tag_ix = items.len();
    items.push(ContextMenuItem::Entry {
        label: language.text("Add tag…", "新增標籤…").into(),
        icon: Some("icons/tag.svg".into()),
        shortcut: Some("T".into()),
        disabled: add_tag_disabled,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::CreateTagPrompt {
                repo_id,
                target: sha.clone(),
            },
        }),
    });
    if add_tag_disabled {
        entry_tooltips.insert(
            add_tag_ix,
            language
                .text(
                    "Enable “Show tags in history view” in Settings > Git log to add tags.",
                    "請先在「設定 → Git 歷史」啟用「在歷史檢視中顯示標籤」。",
                )
                .into(),
        );
    }
    items.push(ContextMenuItem::Entry {
        label: language
            .text("Checkout (detached)", "切換至此提交（分離 HEAD）")
            .into(),
        icon: Some("icons/git_branch.svg".into()),
        shortcut: Some("D".into()),
        disabled: worktree_finish_busy,
        action: Box::new(ContextMenuAction::CheckoutCommit {
            repo_id,
            commit_id: commit_id.clone(),
        }),
    });
    items.push(ContextMenuItem::Entry {
        label: language
            .text("Create worktree from this commit…", "從此提交建立工作樹…")
            .into(),
        icon: Some("icons/git_worktree.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::OpenWorktreeAddPrompt {
            repo_id,
            prefill: WorktreeAddPrefill {
                source: sha.clone(),
                source_upstream: None,
                create_branch: true,
                new_branch: String::new(),
                open_after: true,
            },
        }),
    });
    items.push(ContextMenuItem::Entry {
        label: language
            .text("Create branch from this commit", "從此提交建立分支")
            .into(),
        icon: Some("icons/git_branch.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::CreateBranchFromRefPrompt {
                repo_id,
                target: sha.clone(),
                source_selectable: false,
                name_prefix: String::new(),
                checkout_on_open: true,
            },
        }),
    });
    if !has_multi_cherry_pick && !is_head_commit {
        items.push(cherry_pick_entry(
            repo_id,
            commit_id,
            history_rewrite_disabled,
            Some("P"),
            language,
        ));
    }
    // The commit-details pencil, offered here too. Its rules decide: HEAD
    // amends; an older commit must be on HEAD's linear first-parent history.
    // The first-parent position is prepared for HEAD and the selected commit
    // only; other rows are judged once their details load.
    let reword_blocker = this
        .active_repo()
        .filter(|repo| repo.id == repo_id)
        .and_then(|repo| {
            let judged = repo.head_commit_id().as_ref() == Some(commit_id)
                || repo.history_state.selected_commit.as_ref() == Some(commit_id);
            if !judged {
                return None;
            }
            let parent_count = repo
                .log
                .ready()
                .and_then(|page| page.commits.iter().find(|c| c.id == *commit_id))
                .map(|commit| commit.parent_ids.len())?;
            crate::view::panels::commit_reword_availability_for_commit(
                repo,
                commit_id,
                parent_count,
            )
            .blocker
        });
    if let Some(blocker) = reword_blocker {
        entry_tooltips.insert(items.len(), blocker.into());
    }
    items.push(ContextMenuItem::Entry {
        label: language
            .text("Edit commit message…", "編輯提交訊息…")
            .into(),
        icon: Some("icons/pencil.svg".into()),
        shortcut: Some("E".into()),
        disabled: reword_blocker.is_some() || worktree_finish_busy,
        action: Box::new(ContextMenuAction::EditCommitMessage {
            repo_id,
            commit_id: commit_id.clone(),
        }),
    });
    items.push(revert_entry(
        repo_id,
        commit_id,
        history_rewrite_disabled,
        Some("R"),
        language,
    ));
    let current_branch: SharedString = this
        .active_repo()
        .and_then(|r| match &r.head_branch {
            Loadable::Ready(head) if !head.is_empty() && head != "HEAD" => {
                Some(head.as_str().into())
            }
            _ => None,
        })
        .unwrap_or_else(|| short.clone());

    // Rebasing the current branch onto the commit it already points to is a
    // no-op (plain rebase) or produces an empty `HEAD..HEAD` todo list
    // (interactive), so skip both entries on the HEAD commit. The topmost
    // commit is still editable via an interactive rebase from the commit below.
    if !is_head_commit {
        // Prefer a branch name at the target commit; fall back to the abbreviated
        // sha for the label and the full sha for the actual rebase target.
        let target_label: SharedString = branch_names
            .first()
            .map(|s| s.as_str())
            .unwrap_or(&short)
            .into();
        let onto_ref = branch_names.first().cloned().unwrap_or_else(|| sha.clone());
        items.push(ContextMenuItem::Entry {
            label: match language {
                Language::English => format!("Rebase {current_branch} onto {target_label}"),
                Language::TraditionalChinese => {
                    format!("將 {current_branch} 變基至 {target_label}")
                }
            }
            .into(),
            icon: Some("icons/arrow_up.svg".into()),
            shortcut: Some("B".into()),
            disabled: history_rewrite_disabled,
            action: Box::new(ContextMenuAction::OpenPopover {
                kind: PopoverKind::RebaseOntoConfirm {
                    repo_id,
                    onto: onto_ref,
                },
            }),
        });
        // Count the commits the interactive rebase will rewrite (`this..HEAD`).
        // The count is only exact on a strictly linear chain — merges pull in
        // side-branch commits — so fall back to the onto-style label, which
        // claims no count, whenever exactness is not guaranteed.
        let children_count = this
            .active_repo()
            .filter(|repo| repo.id == repo_id)
            .and_then(|repo| {
                let head = repo.head_commit_id()?;
                match &repo.log {
                    Loadable::Ready(page) => gitcomet_core::squash::linear_first_parent_distance(
                        &page.commits,
                        &head,
                        commit_id,
                    ),
                    _ => None,
                }
            });
        let irebase_label = match (language, children_count) {
            (Language::English, Some(count)) => {
                let noun = if count == 1 { "child" } else { "children" };
                format!("Interactive rebase {count} {noun} of {short}")
            }
            (Language::English, None) => {
                format!("Interactive rebase {current_branch} onto {target_label}")
            }
            (Language::TraditionalChinese, Some(count)) => {
                format!("互動式變基 {short} 之後的 {count} 筆提交")
            }
            (Language::TraditionalChinese, None) => {
                format!("將 {current_branch} 互動式變基至 {target_label}")
            }
        };
        items.push(ContextMenuItem::Entry {
            label: irebase_label.into(),
            icon: Some("icons/refresh.svg".into()),
            shortcut: Some("I".into()),
            disabled: history_rewrite_disabled,
            action: Box::new(ContextMenuAction::LoadInteractiveRebaseSetup {
                repo_id,
                base: sha.clone(),
            }),
        });
    }

    // "Merge into current" merges the right-clicked commit into the checked-out
    // branch through the shared MergeRef pipeline. It is a no-op when the commit
    // is already part of HEAD's history (including HEAD itself), so it is
    // disabled rather than letting git reject it as "Already up to date". It
    // also contends for the same repository operation slot as history rewrites.
    //
    // Resolved through `state.repos` rather than `active_repo()`, and through
    // the dialog's own two helpers, because this entry opens that dialog: with
    // several repositories open, a right-click on a commit outside the active
    // one made the entry read "into HEAD" while the confirmation it opened read
    // "into main", and left the entry enabled for a repository the dialog was
    // about to refuse. `commit_is_ancestor_of_head` already looks the repository
    // up this way.
    let merge_repo = this.state.repos.iter().find(|repo| repo.id == repo_id);
    let merge_into_current_disabled = worktree_finish_busy
        || !super::super::merge_commit_confirm::merge_commit_is_allowed(
            merge_repo,
            &this.state.large_file_settings,
        )
        || commit_is_ancestor_of_head(this, repo_id, commit_id);
    let merge_destination =
        super::super::merge_commit_confirm::merge_commit_destination_label(merge_repo);
    items.push(ContextMenuItem::Entry {
        label: match language {
            Language::English => format!("Merge {short} into {merge_destination}"),
            Language::TraditionalChinese => format!("將 {short} 合併至 {merge_destination}"),
        }
        .into(),
        icon: Some("icons/swap.svg".into()),
        shortcut: Some("M".into()),
        disabled: merge_into_current_disabled,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::MergeCommitConfirm {
                repo_id,
                commit_id: commit_id.clone(),
            },
        }),
    });

    items.push(ContextMenuItem::Separator);
    // Undo the last commit: a soft reset to its parent, through the usual
    // reset confirmation. Its changes stay staged for a new commit.
    if is_head_commit {
        let parent = this
            .active_repo()
            .filter(|repo| repo.id == repo_id)
            .and_then(|repo| repo.log.ready())
            .and_then(|page| page.commits.iter().find(|c| c.id == *commit_id))
            .filter(|commit| commit.parent_ids.len() == 1)
            .and_then(|commit| commit.parent_ids.first().cloned());
        if let Some(parent) = parent {
            entry_tooltips.insert(
                items.len(),
                language
                    .text(
                        "Soft reset to the parent commit; the changes stay staged",
                        "軟重設至父提交，變更會保留在暫存區。",
                    )
                    .into(),
            );
            items.push(ContextMenuItem::Entry {
                label: language.text("Undo commit…", "復原提交…").into(),
                icon: Some("icons/undo.svg".into()),
                shortcut: Some("U".into()),
                disabled: history_rewrite_disabled,
                action: Box::new(ContextMenuAction::OpenPopover {
                    kind: PopoverKind::ResetPrompt {
                        repo_id,
                        target: parent.as_ref().to_string(),
                        mode: ResetMode::Soft,
                    },
                }),
            });
        }
    }
    items.extend(reset_entries(
        repo_id,
        commit_id,
        worktree_finish_busy,
        language,
    ));

    ContextMenuModel::new(items).with_entry_tooltips(entry_tooltips)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitcomet_core::domain::{CommitParentIds, LogPage, RepoSpec};
    use gitcomet_state::model::{Loadable, RepoId, RepoState};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::SystemTime;

    fn repo_state() -> RepoState {
        RepoState::new_opening(
            RepoId(1),
            RepoSpec {
                workdir: PathBuf::from("/tmp/repo"),
            },
        )
    }

    fn commit(id: &str, parents: &[&str]) -> Commit {
        Commit {
            id: CommitId(id.into()),
            parent_ids: parents
                .iter()
                .map(|p| CommitId((*p).into()))
                .collect::<CommitParentIds>(),
            summary: "summary".into(),
            author: "author".into(),
            time: SystemTime::UNIX_EPOCH,
        }
    }

    fn with_log(mut repo: RepoState, head: &str, commits: Vec<Commit>) -> RepoState {
        repo.detached_head_commit = Some(CommitId(head.into()));
        repo.log = Loadable::Ready(Arc::new(LogPage {
            commits,
            next_cursor: None,
        }));
        repo
    }

    #[test]
    fn head_commit_is_its_own_ancestor() {
        let repo = with_log(repo_state(), "a", vec![commit("a", &[])]);

        assert!(repo_commit_is_ancestor_of_head(
            &repo,
            &CommitId("a".into())
        ));
    }

    #[test]
    fn commit_reachable_through_parent_chain_is_ancestor() {
        // c -> b -> a (HEAD is c)
        let repo = with_log(
            repo_state(),
            "c",
            vec![commit("c", &["b"]), commit("b", &["a"]), commit("a", &[])],
        );

        assert!(repo_commit_is_ancestor_of_head(
            &repo,
            &CommitId("a".into())
        ));
    }

    #[test]
    fn commit_on_unrelated_branch_is_not_ancestor() {
        // HEAD is b, which does not descend from the side-branch commit x
        let repo = with_log(
            repo_state(),
            "b",
            vec![commit("b", &["a"]), commit("a", &[])],
        );

        assert!(!repo_commit_is_ancestor_of_head(
            &repo,
            &CommitId("x".into())
        ));
    }

    #[test]
    fn no_head_commit_is_not_ancestor() {
        let repo = repo_state();

        assert!(!repo_commit_is_ancestor_of_head(
            &repo,
            &CommitId("a".into())
        ));
    }

    #[test]
    fn merge_entry_disabled_when_commit_is_ancestor_of_head() {
        let repo = with_log(
            repo_state(),
            "c",
            vec![commit("c", &["b"]), commit("b", &[])],
        );

        assert!(repo_commit_is_ancestor_of_head(
            &repo,
            &CommitId("b".into())
        ));
        assert!(!repo_commit_is_ancestor_of_head(
            &repo,
            &CommitId("missing".into())
        ));
    }
}
