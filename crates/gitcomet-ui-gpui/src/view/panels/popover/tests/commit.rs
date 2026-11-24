use super::*;
use gitcomet_core::domain::{Branch, RemoteBranch, RepoSpec};
use gitcomet_state::model::{AppState, Loadable, RepoState};

fn open_repo(repo_id: RepoId, workdir: &str) -> RepoState {
    let mut repo = RepoState::new_opening(
        repo_id,
        RepoSpec {
            workdir: workdir.into(),
        },
    );
    repo.open = Loadable::Ready(());
    repo.repository_mode = Some(gitcomet_core::domain::RepositoryMode::WorkingTree);
    repo
}

#[gpui::test]
fn graph_commit_worktree_prompt_uses_the_full_sha(cx: &mut gpui::TestAppContext) {
    use gitcomet_core::domain::RepositoryMode;
    use gitcomet_core::services::WorktreeCheckout;

    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let repo_id = RepoId(1);
    let mut repo = open_repo(repo_id, "/tmp/commit-worktree-source");
    repo.repository_mode = Some(RepositoryMode::Bare);
    repo.head_branch = Loadable::Ready("main".into());
    repo.branches = Loadable::Ready(Arc::new(vec![Branch {
        name: "main".into(),
        target: CommitId("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()),
        upstream: None,
        divergence: None,
    }]));
    store.replace_snapshot_for_test(Arc::new(AppState {
        repos: vec![repo],
        active_repo: Some(repo_id),
        ..AppState::test_default()
    }));
    let (view, cx) =
        cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));
    let commit_id = CommitId("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into());
    cx.update(|window, app| {
        view.read(app).popover_host.clone().update(app, |host, cx| {
            let action = host
                .context_menu_model(
                    &PopoverKind::CommitMenu {
                        repo_id,
                        commit_id: commit_id.clone(),
                        refs: CommitMenuRefs::All,
                    },
                    cx,
                )
                .expect("commit context menu")
                .items
                .into_iter()
                .find_map(|item| match item {
                    ContextMenuItem::Entry {
                        disabled: false,
                        action,
                        ..
                    } if matches!(
                        action.as_ref(),
                        ContextMenuAction::OpenWorktreeAddPrompt { .. }
                    ) =>
                    {
                        Some(*action)
                    }
                    _ => None,
                })
                .expect("enabled worktree action");
            host.context_menu_activate_action(action, window, cx);
            assert!(matches!(
                host.popover,
                Some(PopoverKind::Repo {
                    repo_id: owner,
                    kind: RepoPopoverKind::Worktree(WorktreePopoverKind::AddPrompt)
                }) if owner == repo_id
            ));
            assert!(!host.can_submit_worktree_add(cx));
            assert!(host.worktree_add_open_after);
            let destination = "/tmp/commit-worktree-target";
            host.worktree_path_input
                .update(cx, |input, cx| input.set_text(destination, cx));
            assert_eq!(
                host.worktree_add_request(repo_id, cx).unwrap(),
                (
                    destination.into(),
                    WorktreeCheckout::NewBranch {
                        name: "commit-worktree-target".into(),
                        start: Some(commit_id.as_ref().to_string()),
                    },
                )
            );
        });
    });
}

#[gpui::test]
fn commit_prompt_field_aligns_with_its_footer_and_scrolls_inside_the_border(
    cx: &mut gpui::TestAppContext,
) {
    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let repo_id = RepoId(1);
    store.replace_snapshot_for_test(Arc::new(AppState {
        repos: vec![open_repo(repo_id, "/tmp/commit-prompt-field-layout")],
        active_repo: Some(repo_id),
        ..AppState::test_default()
    }));
    let (view, cx) =
        cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));
    let (input, scroll) = cx.update(|window, app| {
        let host = view.read(app).popover_host.clone();
        host.update(app, |host, cx| {
            host.open_popover_centered(PopoverKind::CommitPrompt { repo_id }, window, cx);
            (
                host.commit_prompt_message_input.clone(),
                host.commit_prompt_message_scroll.clone(),
            )
        })
    });
    for percent in [100, 150] {
        cx.update(|_, app| crate::ui_scale::set_default(app, percent));
        for message in [
            "A commit message".to_owned(),
            "A commit message\n".repeat(50),
        ] {
            cx.update(|_, app| input.update(app, |input, cx| input.set_text(message.clone(), cx)));
            for _ in 0..4 {
                crate::test_support::refresh_and_draw(cx);
            }
            let field = cx.debug_bounds("scrollable_text_input_field").unwrap();
            let submit = cx.debug_bounds("commit_prompt_submit").unwrap();
            let rule = cx.debug_bounds("commit_prompt_rule").unwrap();
            let scope = cx.debug_bounds("commit_prompt_scope_summary").unwrap();
            assert!(scope.bottom() <= rule.top());
            assert!(scope.left() >= field.left() && scope.right() <= field.right());
            assert!(
                ((field.top() - rule.bottom()) - (submit.top() - field.bottom())).abs()
                    <= gpui::px(1.0),
                "the field must have equal spacing above and below"
            );
            assert!(
                (field.right() - submit.right()).abs() <= gpui::px(1.0),
                "the field border must fill the same width as the footer"
            );
            assert!(field.size.height <= gpui::px(200.0 * percent as f32 / 100.0) + gpui::px(1.0));
            assert_eq!(
                scroll.max_offset().y > gpui::px(0.0),
                message.contains('\n')
            );
            let viewport = scroll.bounds();
            assert!(viewport.left() >= field.left() && viewport.right() <= field.right());
        }
    }
}

#[gpui::test]
fn clean_repo_disables_commit_prompt_submission(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let repo_id = RepoId(1);
    let mut repo = open_repo(repo_id, "/tmp/clean-commit-prompt");
    repo.staged_status = Loadable::Ready(Arc::new(Vec::new()));
    store.replace_snapshot_for_test(Arc::new(AppState {
        repos: vec![repo],
        active_repo: Some(repo_id),
        ..AppState::test_default()
    }));

    let store_for_view = store.clone();
    let (view, cx) = cx
        .add_window_view(|window, cx| GitCometView::new(store_for_view, events, None, window, cx));

    cx.update(|window, app| {
        view.update(app, |this, cx| {
            this.popover_host.update(cx, |host, cx| {
                host.open_popover_at(
                    PopoverKind::CommitPrompt { repo_id },
                    gpui::point(gpui::px(120.0), gpui::px(72.0)),
                    window,
                    cx,
                );
                host.commit_prompt_message_input
                    .update(cx, |input, cx| input.set_text("Commit message", cx));

                assert!(
                    !host.can_submit_commit_prompt(cx),
                    "expected a clean repo to disable the Commit button"
                );
                host.submit_commit_prompt(window, cx);
                assert!(
                    matches!(host.popover, Some(PopoverKind::CommitPrompt { .. })),
                    "expected disabled submission to keep the dialog open"
                );
            });
        });
    });
}

#[gpui::test]
fn commit_prompt_restores_drafts_per_repo(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let repo_a = RepoId(11);
    let repo_b = RepoId(12);
    store.replace_snapshot_for_test(Arc::new(AppState {
        repos: vec![
            open_repo(repo_a, "/tmp/commit-prompt-a"),
            open_repo(repo_b, "/tmp/commit-prompt-b"),
        ],
        active_repo: Some(repo_a),
        ..AppState::test_default()
    }));

    let store_for_view = store.clone();
    let (view, cx) = cx
        .add_window_view(|window, cx| GitCometView::new(store_for_view, events, None, window, cx));

    cx.update(|window, app| {
        view.update(app, |this, cx| {
            this.popover_host.update(cx, |host, cx| {
                let anchor = gpui::point(gpui::px(120.0), gpui::px(72.0));
                host.open_popover_at(
                    PopoverKind::CommitPrompt { repo_id: repo_a },
                    anchor,
                    window,
                    cx,
                );
                host.commit_prompt_message_input
                    .update(cx, |input, cx| input.set_text("repo A draft", cx));
                host.dismiss_prompt_popover(window, cx);

                host.open_popover_at(
                    PopoverKind::CommitPrompt { repo_id: repo_b },
                    anchor,
                    window,
                    cx,
                );
                assert_eq!(host.commit_prompt_message_input.read(cx).text(), "");
                host.commit_prompt_message_input
                    .update(cx, |input, cx| input.set_text("repo B draft", cx));
                host.dismiss_prompt_popover(window, cx);

                host.open_popover_at(
                    PopoverKind::CommitPrompt { repo_id: repo_a },
                    anchor,
                    window,
                    cx,
                );
                assert_eq!(
                    host.commit_prompt_message_input.read(cx).text(),
                    "repo A draft"
                );
                host.dismiss_prompt_popover(window, cx);

                host.open_popover_at(
                    PopoverKind::CommitPrompt { repo_id: repo_b },
                    anchor,
                    window,
                    cx,
                );
                assert_eq!(
                    host.commit_prompt_message_input.read(cx).text(),
                    "repo B draft"
                );
            });
        });
    });
}

fn submit_prompt_with_staged_repo(
    cx: &mut gpui::TestAppContext,
    repo_id: RepoId,
) -> (
    gpui::Entity<GitCometView>,
    RepoState,
    &mut gpui::VisualTestContext,
) {
    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let mut repo = open_repo(repo_id, "/tmp/commit-prompt-submit");
    repo.staged_status = Loadable::Ready(Arc::new(Vec::new()));
    repo.merge_commit_message = Loadable::Ready(Some("Merge branch 'topic'".to_string()));
    store.replace_snapshot_for_test(Arc::new(AppState {
        repos: vec![repo.clone()],
        active_repo: Some(repo_id),
        ..AppState::test_default()
    }));

    let store_for_view = store.clone();
    let (view, cx) = cx
        .add_window_view(|window, cx| GitCometView::new(store_for_view, events, None, window, cx));

    cx.update(|window, app| {
        view.update(app, |this, cx| {
            this.popover_host.update(cx, |host, cx| {
                let anchor = gpui::point(gpui::px(120.0), gpui::px(72.0));
                host.open_popover_at(PopoverKind::CommitPrompt { repo_id }, anchor, window, cx);
                host.commit_prompt_message_input
                    .update(cx, |input, cx| input.set_text("finish merge", cx));
                assert!(host.can_submit_commit_prompt(cx));
                host.submit_commit_prompt(window, cx);
            });
        });
    });
    (view, repo, cx)
}

fn reopened_prompt_text(
    view: &gpui::Entity<GitCometView>,
    cx: &mut gpui::VisualTestContext,
    repo_id: RepoId,
) -> String {
    cx.update(|window, app| {
        view.update(app, |this, cx| {
            this.popover_host.update(cx, |host, cx| {
                let anchor = gpui::point(gpui::px(120.0), gpui::px(72.0));
                host.open_popover_at(PopoverKind::CommitPrompt { repo_id }, anchor, window, cx);
                host.commit_prompt_message_input.read(cx).text().to_string()
            })
        })
    })
}

#[gpui::test]
fn successful_commit_prompt_submission_clears_draft(cx: &mut gpui::TestAppContext) {
    let repo_id = RepoId(21);
    let (view, mut repo, cx) = submit_prompt_with_staged_repo(cx, repo_id);

    repo.feedback_mut()
        .push_command_result(gitcomet_state::model::CommandLogEntry {
            kind: gitcomet_state::model::CommandLogKind::Other,
            time: std::time::UNIX_EPOCH,
            ok: true,
            command: "Commit".to_string(),
            summary: "Commit: test".to_string(),
            stdout: "".into(),
            stderr: "".into(),
            announce_success: true,
            handled_by_prompt: false,
            failure_id: None,
            hook_operation_id: None,
            source: gitcomet_state::model::CommandSource::User,
        });
    let state = Arc::new(AppState {
        repos: vec![repo],
        active_repo: Some(repo_id),
        ..AppState::test_default()
    });
    cx.update(|_window, app| {
        view.update(app, |this, cx| {
            crate::view::test_support::push_test_state(this, state, cx);
        });
    });
    cx.run_until_parked();

    assert_eq!(reopened_prompt_text(&view, cx, repo_id), "");
}

#[gpui::test]
fn failed_commit_prompt_submission_keeps_draft(cx: &mut gpui::TestAppContext) {
    let repo_id = RepoId(22);
    let (view, _repo, cx) = submit_prompt_with_staged_repo(cx, repo_id);

    crate::view::test_support::drain_store_worker(&view, cx);
    cx.update(|_window, app| {
        view.update(app, |this, cx| {
            crate::view::test_support::sync_store_snapshot(this, cx);
        });
    });
    cx.run_until_parked();

    assert_eq!(reopened_prompt_text(&view, cx, repo_id), "finish merge");
}

/// The "Merge into current" entry and the dialog it opens must agree.
///
/// The entry used to resolve the destination through `active_repo()`, which is
/// `None` for a commit belonging to any *other* open repository, so with two
/// repositories open the menu read "into HEAD" while the confirmation it opened
/// -- which resolves through `state.repos` -- read "into release". The same
/// split left the entry enabled for a repository the dialog was about to
/// refuse.
#[gpui::test]
fn merge_entry_names_and_gates_on_the_commits_own_repository(cx: &mut gpui::TestAppContext) {
    fn merge_entry(
        cx: &mut gpui::VisualTestContext,
        view: &gpui::Entity<GitCometView>,
        repo_id: RepoId,
        commit_id: &CommitId,
    ) -> (String, bool) {
        cx.update(|_window, app| {
            view.update(app, |this, cx| {
                this.popover_host.update(cx, |host, cx| {
                    host.context_menu_model(
                        &PopoverKind::CommitMenu {
                            repo_id,
                            commit_id: commit_id.clone(),
                            refs: CommitMenuRefs::All,
                        },
                        cx,
                    )
                })
            })
        })
        .expect("expected a commit context menu model")
        .items
        .iter()
        .find_map(|item| match item {
            ContextMenuItem::Entry {
                label, disabled, ..
            } if label.starts_with("Merge ") => Some((label.to_string(), *disabled)),
            _ => None,
        })
        .expect("expected the merge entry")
    }

    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let (view, cx) =
        cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));

    let active_id = RepoId(1);
    let other_id = RepoId(2);
    let commit_id = CommitId("0123456789abcdef".into());

    let apply = |cx: &mut gpui::VisualTestContext, other_busy: bool| {
        cx.update(|_window, app| {
            view.update(app, |this, cx| {
                let mut active = open_repo(active_id, "/tmp/merge-entry-active");
                active.head_branch = Loadable::Ready("active-branch".into());
                let mut other = open_repo(other_id, "/tmp/merge-entry-other");
                other.head_branch = Loadable::Ready("release".into());
                other.local_actions_in_flight = u32::from(other_busy);

                let state = Arc::new(AppState {
                    repos: vec![active, other],
                    active_repo: Some(active_id),
                    ..AppState::test_default()
                });
                this.state = Arc::clone(&state);
                this.ui_model
                    .update(cx, |model, cx| model.set_state(state, cx));
                cx.notify();
            });
        });
    };

    apply(cx, false);

    let (active_label, _) = merge_entry(cx, &view, active_id, &commit_id);
    assert!(
        active_label.ends_with(" into active-branch"),
        "the active repository still names its own head: {active_label:?}"
    );

    let (other_label, other_disabled) = merge_entry(cx, &view, other_id, &commit_id);
    assert!(
        other_label.ends_with(" into release"),
        "the entry must name the destination its confirmation dialog will: {other_label:?}"
    );
    assert!(
        !other_disabled,
        "an idle repository's commit is mergeable from any menu"
    );

    // And readiness comes from that same repository, not from the active one.
    apply(cx, true);
    let (_, other_disabled) = merge_entry(cx, &view, other_id, &commit_id);
    assert!(
        other_disabled,
        "a busy repository's entry must be disabled, not offered and then refused"
    );
    let (_, active_disabled) = merge_entry(cx, &view, active_id, &commit_id);
    assert!(
        !active_disabled,
        "the other repository being busy says nothing about this one"
    );
}

#[gpui::test]
fn commit_menu_copies_titles_in_history_order_including_uncached_commits(
    cx: &mut gpui::TestAppContext,
) {
    use gitcomet_core::domain::LogScope;
    use gitcomet_core::services::CancellationToken;
    use gitcomet_core::test_support::git_fixture::{
        LinearCommit, git_command, import_linear_history,
    };
    use gitcomet_state::model::CommitMultiSelection;

    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let mut git = git_command();
    git.arg("-C").arg(root);
    assert!(
        git.args(["init", "-q", "-b", "main"])
            .status()
            .unwrap()
            .success()
    );
    let titles = ["修正舊流程", "單獨複製這筆", "", "fix: 修正新流程"];
    let messages = [
        format!("{}\r\n\r\n舊流程的正文\r\n", titles[0]),
        format!("{}\n\n單筆的正文\n", titles[1]),
        " \t\n\n沒有標題的正文\n".to_owned(),
        format!("{}\n\n新流程的正文\n", titles[3]),
    ];
    import_linear_history(
        git_command().arg("-C").arg(root),
        "main",
        messages
            .iter()
            .enumerate()
            .map(|(i, message)| LinearCommit {
                author: "Test <test@example.com>",
                timestamp: 1_600_000_000 + i as i64,
                message,
                path: "fixture.txt",
                contents: "fixture",
            }),
    );

    let backend = gitcomet_git_gix::GixBackend.open(root).unwrap();
    let token = CancellationToken::new();
    let page = backend
        .log_history_mode_page(LogScope::AllBranches, 4, None, &token)
        .unwrap();
    let [newest, untitled, unselected, oldest] = page.commits.as_slice() else {
        panic!("expected four fixture commits in newest-first history order")
    };
    assert_eq!(newest.summary.as_ref(), titles[3]);
    assert_eq!(oldest.summary.as_ref(), titles[0]);
    let index = backend
        .build_history_index(
            &LogScope::AllBranches.into(),
            None,
            &Default::default(),
            &token,
            &mut |_| {},
        )
        .unwrap()
        .expect("indexed history for the fixture");
    let repo_id = RepoId(1);
    let expected = format!("{}\n{}", titles[3], titles[0]);

    for indexed in [false, true] {
        let (store, events) = AppStore::new_test(Arc::new(TestBackend));
        store.disable_repo_monitors_for_test();
        let mut repo = open_repo(repo_id, root.to_str().unwrap());
        // Click order intentionally differs from the order in the graph.
        repo.history_state.multi_selection = CommitMultiSelection {
            commits: Arc::new(vec![
                oldest.id.clone(),
                untitled.id.clone(),
                newest.id.clone(),
            ]),
            ..Default::default()
        };
        if indexed {
            // Keep the graph order available without loading either the bootstrap
            // page or any cached range containing a title.
            repo.history_state.log_snapshot = Some(index.snapshot.clone());
            repo.history_state.indexed.index = Some(index.clone());
            repo.history_state.indexed.displayed_index = Some(index.clone());
            repo.history_state.indexed.range_index = Some(index.clone());
        } else {
            repo.log = Loadable::Ready(page.clone());
        }
        store.replace_snapshot_for_test(Arc::new(AppState {
            repos: vec![repo],
            active_repo: Some(repo_id),
            ..AppState::test_default()
        }));
        store.insert_repo_for_test(repo_id, backend.clone());
        let (view, vcx) =
            cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));

        for (commit_id, label, expected) in [
            (&newest.id, "Copy selected commit titles", expected.as_str()),
            (&unselected.id, "Copy commit title", titles[1]),
        ] {
            vcx.write_to_clipboard(gpui::ClipboardItem::new_string("before copy".to_string()));
            vcx.update(|window, app| {
                let host = view.read(app).popover_host.clone();
                host.update(app, |host, cx| {
                    let model = host
                        .context_menu_model(
                            &PopoverKind::CommitMenu {
                                repo_id,
                                commit_id: commit_id.clone(),
                                refs: CommitMenuRefs::All,
                            },
                            cx,
                        )
                        .expect("commit context menu");
                    let action = model
                        .items
                        .into_iter()
                        .find_map(|item| match item {
                            ContextMenuItem::Entry {
                                label: text,
                                disabled: false,
                                action,
                                ..
                            } if text.as_ref() == label => Some(*action),
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("missing enabled entry: {label}"));
                    host.context_menu_activate_action(action, window, cx);
                });
            });
            vcx.run_until_parked();
            assert_eq!(
                vcx.read_from_clipboard()
                    .and_then(|item| item.text())
                    .as_deref(),
                Some(expected),
                "{label} must copy one title per line, indexed={indexed}"
            );
        }
    }
}

#[gpui::test]
fn copy_branch_name_entries_follow_the_commits_branches(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let (view, cx) =
        cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));

    let repo_id = RepoId(1);
    // The active tab is a different, empty repository: the entries must come
    // from the repository the menu was opened for.
    let active_id = RepoId(2);
    let commit_id = CommitId("0123456789abcdef".into());
    let other_commit = CommitId("fedcba9876543210".into());
    let local = |name: &str, target: &CommitId| Branch {
        name: name.into(),
        target: target.clone(),
        upstream: None,
        divergence: None,
    };
    let remote = |name: &str, target: &CommitId| RemoteBranch {
        remote: "origin".into(),
        name: name.into(),
        target: target.clone(),
    };

    let copy_entries = |cx: &mut gpui::VisualTestContext,
                        branches: Vec<Branch>,
                        remote_branches: Vec<RemoteBranch>| {
        cx.update(|_window, app| {
            view.update(app, |this, cx| {
                let mut repo = open_repo(repo_id, "/tmp/copy-branch-name");
                repo.branches = Loadable::Ready(Arc::new(branches));
                repo.remote_branches = Loadable::Ready(Arc::new(remote_branches));
                let state = Arc::new(AppState {
                    repos: vec![repo, open_repo(active_id, "/tmp/copy-branch-name-active")],
                    active_repo: Some(active_id),
                    ..AppState::test_default()
                });
                this.state = Arc::clone(&state);
                this.ui_model
                    .update(cx, |model, cx| model.set_state(state, cx));
                cx.notify();
            });
        });
        // The popover host picks up the new state once that update ends.
        let model = cx
            .update(|_window, app| {
                view.update(app, |this, cx| {
                    this.popover_host.update(cx, |host, cx| {
                        host.context_menu_model(
                            &PopoverKind::CommitMenu {
                                repo_id,
                                commit_id: commit_id.clone(),
                                refs: CommitMenuRefs::All,
                            },
                            cx,
                        )
                    })
                })
            })
            .expect("expected a commit context menu model");
        let labels: Vec<String> = model
            .entry_labels()
            .into_iter()
            .map(str::to_string)
            .collect();
        // The copy entries sit directly under "Copy commit SHA".
        if let Some(first) = labels
            .iter()
            .position(|l| l.starts_with("Copy branch name"))
        {
            assert_eq!(labels[first - 1], "Copy commit SHA");
        }
        model
            .items
            .iter()
            // No ref group is expanded here, so these are only the commit-level
            // entries, not the "Copy branch name" inside a branch's own group.
            .filter_map(|item| match item {
                ContextMenuItem::Entry { label, action, .. }
                    if label.starts_with("Copy branch name") =>
                {
                    match action.as_ref() {
                        ContextMenuAction::CopyText { text } => {
                            Some((label.to_string(), text.clone()))
                        }
                        _ => panic!("{label} must copy text"),
                    }
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    // One local branch: a plain entry, preferred over its remote-tracking ref.
    assert_eq!(
        copy_entries(
            cx,
            vec![
                local("feat/app-cert-check", &commit_id),
                local("main", &other_commit)
            ],
            vec![remote("feat/app-cert-check", &commit_id)],
        ),
        vec![("Copy branch name".into(), "feat/app-cert-check".into())]
    );

    // Several local branches: one entry each, named.
    assert_eq!(
        copy_entries(
            cx,
            vec![local("feat/a", &commit_id), local("feat/b", &commit_id)],
            Vec::new(),
        ),
        vec![
            ("Copy branch name feat/a".into(), "feat/a".into()),
            ("Copy branch name feat/b".into(), "feat/b".into()),
        ]
    );

    // Many branches: the copies move into each branch's own group.
    assert_eq!(
        copy_entries(
            cx,
            ["feat/a", "feat/b", "feat/c", "feat/d"]
                .map(|name| local(name, &commit_id))
                .to_vec(),
            Vec::new(),
        ),
        Vec::<(String, String)>::new()
    );

    // Only a remote branch: copy it the way its own menu does.
    assert_eq!(
        copy_entries(cx, Vec::new(), vec![remote("feat/x", &commit_id)]),
        vec![("Copy branch name".into(), "origin/feat/x".into())]
    );

    // No branch at this commit: no entry.
    assert_eq!(
        copy_entries(cx, vec![local("main", &other_commit)], Vec::new()),
        Vec::<(String, String)>::new()
    );
}

/// A busy commit's menu lists the checked-out branch's group first, as the
/// graph's chips and refs card do, instead of in name order.
#[gpui::test]
fn commit_menu_lists_the_checked_out_branch_first(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    let (view, cx) =
        cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));
    let repo_id = RepoId(1);
    let commit_id = CommitId("0123456789abcdef".into());
    let local = |name: &str| Branch {
        name: name.into(),
        target: commit_id.clone(),
        upstream: None,
        divergence: None,
    };
    cx.update(|_window, app| {
        view.update(app, |this, cx| {
            let mut repo = open_repo(repo_id, "/tmp/commit-menu-head-first");
            repo.head_branch = Loadable::Ready("main".into());
            repo.branches = Loadable::Ready(Arc::new(vec![
                local("feat/a"),
                local("main"),
                local("menu-a"),
                local("menu-b"),
            ]));
            let state = Arc::new(AppState {
                repos: vec![repo],
                active_repo: Some(repo_id),
                ..AppState::test_default()
            });
            this.state = Arc::clone(&state);
            this.ui_model
                .update(cx, |model, cx| model.set_state(state, cx));
            cx.notify();
        });
    });
    let menu_labels = |cx: &mut gpui::VisualTestContext, expanded: Option<&str>| {
        cx.update(|_window, app| {
            view.update(app, |this, cx| {
                this.popover_host.update(cx, |host, cx| {
                    host.expanded_history_ref =
                        expanded.map(|name| HistoryMenuRef::Branch(BranchMenuTarget::local(name)));
                    host.context_menu_model(
                        &PopoverKind::CommitMenu {
                            repo_id,
                            commit_id: commit_id.clone(),
                            refs: CommitMenuRefs::All,
                        },
                        cx,
                    )
                })
            })
        })
        .expect("expected a commit context menu model")
        .entry_labels()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>()
    };
    let labels = menu_labels(cx, None);
    assert_eq!(
        labels
            .iter()
            .filter(|label| label.starts_with("Local branch"))
            .collect::<Vec<_>>(),
        [
            "Local branch main",
            "Local branch feat/a",
            "Local branch menu-a",
            "Local branch menu-b"
        ]
    );
    // Four branches put "Copy branch name" in each branch's own group only.
    assert!(
        !labels
            .iter()
            .any(|label| label.starts_with("Copy branch name"))
    );
    let expanded = menu_labels(cx, Some("menu-a"));
    assert!(
        expanded.iter().any(|label| label == "Copy branch name"),
        "the expanded group still copies its branch: {expanded:?}"
    );
}
