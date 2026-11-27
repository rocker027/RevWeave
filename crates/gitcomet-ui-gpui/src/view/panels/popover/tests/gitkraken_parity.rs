//! Entry points added so GitKraken's operations have a counterpart here:
//! the palette's Edit Remote URL picker and the interactive rebase row
//! menu's single-key actions.
use super::*;
use gitcomet_core::domain::{Remote, RepoSpec};

fn repo_with_remotes(remotes: Vec<Remote>) -> RepoState {
    let mut repo = RepoState::new_opening(
        RepoId(31),
        RepoSpec {
            workdir: "/tmp/gitkraken-parity".into(),
        },
    );
    repo.open = Loadable::Ready(());
    repo.repository_mode = Some(gitcomet_core::domain::RepositoryMode::WorkingTree);
    repo.remotes = Loadable::Ready(Arc::new(remotes));
    repo
}

fn remote(name: &str, url: &str) -> Remote {
    Remote {
        name: name.into(),
        url: Some(url.into()),
        default_branch: None,
    }
}

fn edit_url_menu_model(
    cx: &mut gpui::VisualTestContext,
    view: &gpui::Entity<GitCometView>,
    repo: RepoState,
) -> ContextMenuModel {
    let repo_id = repo.id;
    let state = Arc::new(AppState {
        active_repo: Some(repo_id),
        repos: vec![repo],
        ..AppState::test_default()
    });
    cx.update(|_window, app| {
        view.update(app, |this, cx| {
            this.state = state.clone();
            this.popover_host.update(cx, |host, cx| {
                host.state = state;
                host.context_menu_model(
                    &PopoverKind::remote(repo_id, RemotePopoverKind::EditUrlMenu),
                    cx,
                )
                .expect("expected the edit remote URL menu")
            })
        })
    })
}

#[gpui::test]
fn edit_remote_url_menu_opens_each_remotes_fetch_url_prompt(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let (view, cx) =
        cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));
    let model = edit_url_menu_model(
        cx,
        &view,
        repo_with_remotes(vec![
            remote("origin", "https://example.com/a.git"),
            remote("upstream", "https://example.com/b.git"),
        ]),
    );

    let entries: Vec<_> = model
        .items
        .iter()
        .filter_map(|item| match item {
            ContextMenuItem::Entry {
                label,
                shortcut,
                action,
                ..
            } => Some((label.to_string(), shortcut.clone(), (**action).clone())),
            _ => None,
        })
        .collect();
    assert_eq!(entries.len(), 2);
    for ((label, shortcut, action), (name, digit)) in entries
        .into_iter()
        .zip([("origin", "1"), ("upstream", "2")])
    {
        assert!(label.starts_with(name), "{label}");
        assert_eq!(shortcut.as_deref(), Some(digit));
        assert!(
            matches!(
                &action,
                ContextMenuAction::OpenPopover {
                    kind: PopoverKind::Repo {
                        repo_id: RepoId(31),
                        kind: RepoPopoverKind::Remote(RemotePopoverKind::EditUrlPrompt {
                            name: remote_name,
                            kind: RemoteUrlKind::Fetch,
                        }),
                    },
                } if remote_name == name
            ),
            "{label} should open the fetch URL prompt of {name}"
        );
    }
}

#[gpui::test]
fn edit_remote_url_menu_says_when_there_are_no_remotes(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let (view, cx) =
        cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));
    let model = edit_url_menu_model(cx, &view, repo_with_remotes(Vec::new()));
    assert!(model.items.iter().any(|item| matches!(
        item,
        ContextMenuItem::Label(text) if text.as_ref() == "This repository has no remotes"
    )));
    assert!(
        !model
            .items
            .iter()
            .any(|item| matches!(item, ContextMenuItem::Entry { .. }))
    );
}

#[gpui::test]
fn interactive_rebase_row_menu_declares_gitkraken_keys(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let (view, cx) =
        cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));
    let model = cx.update(|_window, app| {
        view.update(app, |this, cx| {
            this.popover_host.update(cx, |host, cx| {
                host.context_menu_model(
                    &PopoverKind::InteractiveRebaseActionMenu {
                        ix: 1,
                        can_squash: true,
                        can_drop: true,
                    },
                    cx,
                )
                .expect("expected the rebase action menu")
            })
        })
    });
    let keys: Vec<(String, Option<String>)> = model
        .items
        .iter()
        .filter_map(|item| match item {
            ContextMenuItem::Entry {
                label, shortcut, ..
            } => Some((label.to_string(), shortcut.as_ref().map(|s| s.to_string()))),
            _ => None,
        })
        .collect();
    assert_eq!(
        keys,
        vec![
            ("pick".to_string(), Some("P".to_string())),
            ("reword".to_string(), Some("R".to_string())),
            ("drop".to_string(), Some("D".to_string())),
            ("squash".to_string(), Some("S".to_string())),
        ]
    );
}

#[test]
fn gitkraken_palette_commands_resolve_by_id() {
    for id in ["merge", "search-commits", "edit-remote-url"] {
        assert!(
            crate::view::commands::Command::from_id(id).is_some(),
            "missing command {id}"
        );
    }
}

#[gpui::test]
fn edit_remote_url_menu_hides_credentials(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let (view, cx) =
        cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));
    let model = edit_url_menu_model(
        cx,
        &view,
        repo_with_remotes(vec![remote(
            "origin",
            "https://user:secret-token@example.com/a.git",
        )]),
    );
    let label = model
        .items
        .iter()
        .find_map(|item| match item {
            ContextMenuItem::Entry { label, .. } => Some(label.to_string()),
            _ => None,
        })
        .expect("expected a remote row");
    assert!(!label.contains("secret-token"), "{label}");
}

/// The palette's Remove Remote lists the remotes; each row opens the existing
/// removal confirmation rather than removing anything.
#[gpui::test]
fn remove_remote_menu_rows_open_the_removal_confirmation(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let (view, cx) =
        cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));
    let repo = repo_with_remotes(vec![
        remote("origin", "https://example.com/a.git"),
        remote("fork", "https://example.com/b.git"),
    ]);
    let repo_id = repo.id;
    let state = Arc::new(AppState {
        active_repo: Some(repo_id),
        repos: vec![repo],
        ..AppState::test_default()
    });
    let model = cx.update(|_window, app| {
        view.update(app, |this, cx| {
            this.state = state.clone();
            this.popover_host.update(cx, |host, cx| {
                host.state = state;
                host.context_menu_model(
                    &PopoverKind::remote(repo_id, RemotePopoverKind::RemoveMenu),
                    cx,
                )
                .expect("expected the remove remote menu")
            })
        })
    });
    let targets: Vec<String> = model
        .items
        .iter()
        .filter_map(|item| match item {
            ContextMenuItem::Entry { action, .. } => match action.as_ref() {
                ContextMenuAction::OpenPopover {
                    kind:
                        PopoverKind::Repo {
                            kind: RepoPopoverKind::Remote(RemotePopoverKind::RemoveConfirm { name }),
                            ..
                        },
                } => Some(name.clone()),
                _ => panic!("every row must open the removal confirmation"),
            },
            _ => None,
        })
        .collect();
    assert_eq!(targets, vec!["origin".to_string(), "fork".to_string()]);
}

#[test]
fn removal_commands_are_in_the_palette() {
    for (query, id) in [
        ("remove worktree", "remove-worktree"),
        ("remove submodule", "remove-submodule"),
        ("remove remote", "remove-remote"),
        ("focus commit message", "focus-commit-message"),
    ] {
        let ids = crate::view::command_palette::filtered_commands(true, query)
            .iter()
            .map(|m| m.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&id), "{query:?} should find {id}, got {ids:?}");
    }
}

fn popover_kind(
    cx: &mut gpui::VisualTestContext,
    view: &gpui::Entity<GitCometView>,
) -> Option<PopoverKind> {
    cx.update(|_window, app| {
        view.read(app)
            .popover_host
            .read(app)
            .popover_kind_for_tests()
    })
}

/// Deleting a tag from any menu opens the confirmation; the tag is deleted
/// only when the user confirms.
#[gpui::test]
fn deleting_a_tag_asks_first(cx: &mut gpui::TestAppContext) {
    let (store, events, repo, _workdir) =
        super::branch::create_tracking_store("delete-tag-confirm");
    let repo_id = store.snapshot().active_repo.expect("active repository");
    let store_for_view = store.clone();
    let (view, cx) = cx
        .add_window_view(|window, cx| GitCometView::new(store_for_view, events, None, window, cx));
    cx.update(|window, app| {
        let _ = window.draw(app);
    });
    let host = cx.update(|_window, app| view.read(app).popover_host.clone());
    cx.update(|window, app| {
        host.update(app, |host, cx| {
            host.context_menu_activate_action(
                ContextMenuAction::DeleteTag {
                    repo_id,
                    name: "v1.0".to_string(),
                },
                window,
                cx,
            );
        });
    });
    cx.run_until_parked();
    assert_eq!(
        popover_kind(cx, &view),
        Some(PopoverKind::DeleteTagConfirm {
            repo_id,
            name: "v1.0".to_string(),
            remote: None,
        })
    );
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(
        repo.actions().is_empty(),
        "nothing is deleted before confirming"
    );

    cx.update(|window, app| {
        let _ = window.draw(app);
    });
    let go = cx
        .debug_bounds("delete_tag_confirm_go")
        .expect("expected the Delete button")
        .center();
    cx.simulate_click(go, gpui::Modifiers::default());
    cx.run_until_parked();
    super::branch::wait_until("tag deleted after confirming", || {
        repo.actions() == vec!["delete-tag:v1.0".to_string()]
    });
}

/// The palette's deletions open pickers whose rows open confirmations.
#[gpui::test]
fn palette_deletions_open_confirmations(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let (view, cx) =
        cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));
    let mut repo = repo_with_remotes(vec![remote("origin", "https://example.com/a.git")]);
    repo.tags = Loadable::Ready(Arc::new(vec![gitcomet_core::domain::Tag {
        name: "v1.0".into(),
        target: CommitId("1111111111111111111111111111111111111111".into()),
    }]));
    repo.remote_branches = Loadable::Ready(Arc::new(vec![gitcomet_core::domain::RemoteBranch {
        remote: "origin".into(),
        name: "feature".into(),
        target: CommitId("2222222222222222222222222222222222222222".into()),
    }]));
    let repo_id = repo.id;
    let state = Arc::new(AppState {
        active_repo: Some(repo_id),
        repos: vec![repo],
        ..AppState::test_default()
    });
    let mut first_action = |purpose: RefPickerPurpose| {
        cx.update(|_window, app| {
            view.update(app, |this, cx| {
                this.state = state.clone();
                this.popover_host.update(cx, |host, cx| {
                    host.state = state.clone();
                    let model = host
                        .context_menu_model(&PopoverKind::RefPickerMenu { repo_id, purpose }, cx)
                        .expect("expected the picker");
                    model.items.iter().find_map(|item| match item {
                        ContextMenuItem::Entry { action, .. } => Some((**action).clone()),
                        _ => None,
                    })
                })
            })
        })
    };
    assert!(matches!(
        first_action(RefPickerPurpose::DeleteTag),
        Some(ContextMenuAction::OpenPopover {
            kind: PopoverKind::DeleteTagConfirm { remote: None, ref name, .. },
        }) if name == "v1.0"
    ));
    assert!(matches!(
        first_action(RefPickerPurpose::DeleteRemoteBranch),
        Some(ContextMenuAction::OpenPopover {
            kind: PopoverKind::Repo {
                kind: RepoPopoverKind::Remote(RemotePopoverKind::DeleteBranchConfirm { ref remote, ref branch }),
                ..
            },
        }) if remote == "origin" && branch == "feature"
    ));
}

#[test]
fn discard_all_covers_unstaged_and_untracked_paths_but_not_submodules() {
    let status = |path: &str, kind| gitcomet_core::domain::FileStatus {
        path: path.into(),
        kind,
        conflict: None,
    };
    use gitcomet_core::domain::FileStatusKind::{Conflicted, Modified, Untracked};
    let mut repo = repo_with_remotes(Vec::new());
    repo.status = Loadable::Ready(
        gitcomet_core::domain::RepoStatus {
            staged: Arc::new(vec![status("staged.rs", Modified)]),
            unstaged: Arc::new(vec![
                status("a.rs", Modified),
                status("new.rs", Untracked),
                status("vendor/lib", Modified),
                status("both.rs", Conflicted),
            ]),
        }
        .into(),
    );
    // The submodule list has not loaded: the dialog waits for it.
    assert_eq!(
        crate::view::panels::popover::destructive_confirms::discard_all_paths(&repo),
        None
    );
    repo.submodules = Loadable::Ready(Arc::new(vec![gitcomet_core::domain::Submodule {
        path: "vendor/lib".into(),
        recorded_head: CommitId("1111111111111111111111111111111111111111".into()),
        checked_out_head: None,
        status: gitcomet_core::domain::SubmoduleStatus::UpToDate,
    }]));
    let (paths, skipped) =
        crate::view::panels::popover::destructive_confirms::discard_all_paths(&repo)
            .expect("status and submodules loaded");
    assert_eq!(
        paths,
        vec![
            std::path::PathBuf::from("a.rs"),
            std::path::PathBuf::from("new.rs"),
            std::path::PathBuf::from("both.rs"),
        ]
    );
    assert_eq!(skipped, 1, "the submodule is left to its own row");

    // A conflicted file among them brings the existing conflict note.
    let repo_id = repo.id;
    let state = AppState {
        active_repo: Some(repo_id),
        repos: vec![repo],
        ..AppState::test_default()
    };
    assert!(
        crate::view::panels::popover::discard_changes_confirm::conflict_discard_note(
            &state,
            repo_id,
            paths.iter().map(|path| path.as_path()),
        )
        .is_some()
    );
}

#[test]
fn deletion_commands_are_in_the_palette() {
    for (query, id) in [
        ("delete tag", "delete-tag"),
        ("delete remote branch", "delete-remote-branch"),
        ("discard all", "discard-all"),
    ] {
        let ids = crate::view::command_palette::filtered_commands(true, query)
            .iter()
            .map(|m| m.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&id), "{query:?} should find {id}, got {ids:?}");
    }
}

/// The toolbar Undo runs the journal's ref transaction: a commit's branch
/// goes back, expecting the commit it made.
#[gpui::test]
fn the_toolbar_undo_moves_a_committed_branch_back(cx: &mut gpui::TestAppContext) {
    const BEFORE: &str = "1111111111111111111111111111111111111111";
    const AFTER: &str = "2222222222222222222222222222222222222222";
    let (store, events, repo, _workdir) = super::branch::create_tracking_store("git-undo-toolbar");
    let repo_id = store.snapshot().active_repo.expect("active repository");
    store.dispatch(Msg::Internal(
        gitcomet_state::msg::InternalMsg::CommitFinished {
            repo_id,
            request: gitcomet_state::model::CommitRequest {
                message: "ship".to_string(),
                amend: false,
                push_after_commit: false,
                hooks: gitcomet_core::services::GitHooks::Run,
                stage_paths: Vec::new(),
                fixup_target: None,
            },
            result: Ok(gitcomet_core::services::CommitOperationOutcome {
                local_branch: Some("main".to_string()),
                pre_head: Some(CommitId(BEFORE.into())),
                post_head: Some(CommitId(AFTER.into())),
            }),
        },
    ));
    super::branch::wait_until("commit recorded for undo", || {
        store
            .snapshot()
            .repos
            .iter()
            .any(|r| r.id == repo_id && r.git_undo.next_undo().is_some())
    });
    let store_for_view = store.clone();
    let (view, cx) = cx
        .add_window_view(|window, cx| GitCometView::new(store_for_view, events, None, window, cx));
    cx.update(|window, app| {
        view.update(app, |this, cx| {
            crate::view::test_support::sync_store_snapshot(this, cx);
        });
        let _ = window.draw(app);
    });

    let undo = cx
        .debug_bounds("git_undo")
        .expect("expected the Undo button")
        .center();
    cx.simulate_click(undo, gpui::Modifiers::default());
    cx.run_until_parked();
    super::branch::wait_until("undo transaction", || {
        repo.actions()
            .contains(&format!("update-ref:refs/heads/main:{AFTER}->{BEFORE}"))
    });
}

/// The worker confirms the branch against the repository itself: checked
/// out elsewhere since the state last loaded, the commit is not undone and
/// no ref is written.
#[test]
fn undo_refuses_when_the_branch_is_no_longer_checked_out() {
    const BEFORE: &str = "1111111111111111111111111111111111111111";
    const AFTER: &str = "2222222222222222222222222222222222222222";
    let (store, _events, repo, _workdir) =
        super::branch::create_tracking_store("git-undo-stale-head");
    let repo_id = store.snapshot().active_repo.expect("active repository");
    store.dispatch(Msg::Internal(
        gitcomet_state::msg::InternalMsg::CommitFinished {
            repo_id,
            request: gitcomet_state::model::CommitRequest {
                message: "ship".to_string(),
                amend: false,
                push_after_commit: false,
                hooks: gitcomet_core::services::GitHooks::Run,
                stage_paths: Vec::new(),
                fixup_target: None,
            },
            result: Ok(gitcomet_core::services::CommitOperationOutcome {
                local_branch: Some("main".to_string()),
                pre_head: Some(CommitId(BEFORE.into())),
                post_head: Some(CommitId(AFTER.into())),
            }),
        },
    ));
    super::branch::wait_until("commit recorded for undo", || {
        store.snapshot().repos.iter().any(|r| {
            r.id == repo_id
                && r.git_undo.next_undo().is_some()
                && r.git_undo_block(gitcomet_state::model::GitUndoDirection::Undo)
                    .is_none()
        })
    });
    repo.set_current_branch_externally("topic");
    store.dispatch(Msg::UndoGitOperation { repo_id });
    super::branch::wait_until("the undo to finish", || {
        store
            .snapshot()
            .repos
            .iter()
            .any(|r| r.id == repo_id && !r.git_undo.in_flight() && r.local_actions_in_flight == 0)
    });
    assert!(
        !repo
            .actions()
            .iter()
            .any(|action| action.starts_with("update-ref")),
        "{:?}",
        repo.actions()
    );
    let snapshot = store.snapshot();
    let state = snapshot.repos.iter().find(|r| r.id == repo_id).unwrap();
    assert!(state.git_undo.next_undo().is_some(), "the entry stays");
}

#[test]
fn undo_and_redo_are_in_the_palette_and_follow_the_journal() {
    use crate::view::command_palette::{Needs, PaletteContext, unavailable_reason};
    for (query, id) in [("undo", "undo"), ("redo", "redo")] {
        let ids = crate::view::command_palette::filtered_commands(true, query)
            .iter()
            .map(|m| m.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&id), "{query:?} should find {id}, got {ids:?}");
    }
    let nothing = PaletteContext {
        git_undo_unavailable: Some("Nothing to undo"),
        ..PaletteContext::default()
    };
    assert_eq!(
        unavailable_reason(Needs::GitUndo, &nothing),
        Some("Nothing to undo")
    );
    assert_eq!(
        unavailable_reason(Needs::GitUndo, &PaletteContext::default()),
        None
    );
}

/// With nothing staged, the commit button stages every change and commits
/// in one run: the stage lands before the commit, never after it.
#[gpui::test]
fn the_commit_button_stages_all_changes_first_when_nothing_is_staged(
    cx: &mut gpui::TestAppContext,
) {
    let (store, events, repo, _workdir) = super::branch::create_tracking_store("stage-all-commit");
    let repo_id = store.snapshot().active_repo.expect("active repository");
    let changed = |path: &str| gitcomet_core::domain::FileStatus {
        path: path.into(),
        kind: gitcomet_core::domain::FileStatusKind::Modified,
        conflict: None,
    };
    let unstaged = vec![changed("a.txt"), changed("b.txt")];
    for msg in [
        gitcomet_state::msg::InternalMsg::StatusLoaded {
            repo_id,
            result: Ok(gitcomet_core::domain::RepoStatus {
                staged: Arc::new(Vec::new()),
                unstaged: Arc::new(unstaged.clone()),
            }),
        },
        gitcomet_state::msg::InternalMsg::WorktreeStatusLoaded {
            repo_id,
            result: Ok(unstaged),
        },
        gitcomet_state::msg::InternalMsg::StagedStatusLoaded {
            repo_id,
            result: Ok(Vec::new()),
        },
    ] {
        store.dispatch(Msg::Internal(msg));
    }
    super::branch::wait_until("unstaged changes listed", || {
        store
            .snapshot()
            .repos
            .iter()
            .any(|r| r.id == repo_id && r.worktree_status_entries().is_some_and(|e| e.len() == 2))
    });
    let store_for_view = store.clone();
    let (view, cx) = cx
        .add_window_view(|window, cx| GitCometView::new(store_for_view, events, None, window, cx));
    cx.update(|window, app| {
        view.update(app, |this, cx| {
            crate::view::test_support::sync_store_snapshot(this, cx);
            this.details_pane.update(cx, |pane, cx| {
                pane.commit_message_input
                    .update(cx, |input, cx| input.set_text("ship".to_string(), cx));
                cx.notify();
            });
        });
        let _ = window.draw(app);
    });

    // Commit & Push stays with what is staged: with nothing staged it is off.
    let push = cx
        .debug_bounds("commit_and_push_button")
        .expect("expected the commit and push button")
        .center();
    cx.simulate_click(push, gpui::Modifiers::default());
    cx.run_until_parked();
    assert!(repo.actions().is_empty(), "{:?}", repo.actions());

    let commit = cx
        .debug_bounds("commit_button")
        .expect("expected the commit button")
        .center();
    cx.simulate_click(commit, gpui::Modifiers::default());
    cx.run_until_parked();
    super::branch::wait_until("stage and commit", || {
        repo.actions()
            .iter()
            .any(|action| action.starts_with("commit:"))
    });
    assert_eq!(
        repo.actions(),
        vec!["stage:a.txt,b.txt".to_string(), "commit:ship".to_string()]
    );
}

#[test]
fn stage_all_and_commit_waits_for_the_user_while_anything_needs_a_choice() {
    let stage_all_commit_paths =
        |repo: &RepoState, amend: bool| crate::view::stage_all_commit_paths(repo, amend).ok();
    let changed = |path: &str, conflict: bool| gitcomet_core::domain::FileStatus {
        path: path.into(),
        kind: gitcomet_core::domain::FileStatusKind::Modified,
        conflict: conflict.then_some(gitcomet_core::domain::FileConflictKind::BothModified),
    };
    let with_status = |staged: Vec<_>, unstaged: Vec<_>| {
        let mut repo = repo_with_remotes(Vec::new());
        repo.status = Loadable::Ready(
            gitcomet_core::domain::RepoStatus {
                staged: Arc::new(staged),
                unstaged: Arc::new(unstaged),
            }
            .into(),
        );
        repo
    };

    let ready = with_status(Vec::new(), vec![changed("a.txt", false)]);
    assert_eq!(
        stage_all_commit_paths(&ready, false),
        Some(vec![std::path::PathBuf::from("a.txt")])
    );
    // An amend can reword with nothing staged.
    assert_eq!(stage_all_commit_paths(&ready, true), None);
    // Something already staged is what the commit takes.
    let staged = with_status(vec![changed("s.txt", false)], vec![changed("a.txt", false)]);
    assert_eq!(stage_all_commit_paths(&staged, false), None);
    // A conflict must be resolved first.
    let conflicted = with_status(Vec::new(), vec![changed("a.txt", true)]);
    assert_eq!(stage_all_commit_paths(&conflicted, false), None);
    // A merge commit takes what the user staged.
    let mut merging = with_status(Vec::new(), vec![changed("a.txt", false)]);
    merging.merge_commit_message = Loadable::Ready(Some("Merge".into()));
    assert_eq!(stage_all_commit_paths(&merging, false), None);
    // Nothing changed, nothing to commit.
    assert_eq!(
        stage_all_commit_paths(&with_status(Vec::new(), Vec::new()), false),
        None
    );
}

#[test]
fn stage_all_and_commit_is_in_the_palette_with_the_commit_forms_reason() {
    use crate::view::command_palette::{Needs, PaletteContext, unavailable_reason};
    let ids = crate::view::command_palette::filtered_commands(true, "stage all and commit")
        .iter()
        .map(|m| m.id)
        .collect::<Vec<_>>();
    assert!(ids.contains(&"stage-all-and-commit"), "got {ids:?}");
    let blocked = PaletteContext {
        stage_all_commit_unavailable: Some("No changes to commit"),
        ..PaletteContext::default()
    };
    assert_eq!(
        unavailable_reason(Needs::StageAllAndCommit, &blocked),
        Some("No changes to commit")
    );
    assert_eq!(
        unavailable_reason(Needs::StageAllAndCommit, &PaletteContext::default()),
        None
    );
}
