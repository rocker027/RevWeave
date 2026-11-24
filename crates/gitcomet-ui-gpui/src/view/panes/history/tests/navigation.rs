use super::*;

struct NavigationFixture {
    root: Entity<GitCometView>,
    history: Entity<HistoryView>,
    store: AppStore,
    commits: Vec<Commit>,
}

fn mount_navigation(
    cx: &mut gpui::TestAppContext,
    indexed: bool,
    count: usize,
) -> (NavigationFixture, &mut gpui::VisualTestContext) {
    let (index, commits) = super::refresh::indexed_fixture(count);
    mount_navigation_graph(cx, indexed, index, commits)
}

fn mount_navigation_graph(
    cx: &mut gpui::TestAppContext,
    indexed: bool,
    index: gitcomet_core::history_index::HistoryIndexHandle,
    commits: Vec<Commit>,
) -> (NavigationFixture, &mut gpui::VisualTestContext) {
    let count = commits.len();
    // Most indexed destinations have no loaded text. Navigation must resolve
    // them against the projection, not stop at the end of this initial page.
    let loaded = if indexed { count.min(4) } else { count };
    let (root, cx, mut state, store) =
        super::refresh::mount(cx, Arc::new(log_page(commits[..loaded].to_vec(), None)));
    state.repos[0].history_state.selected_commit = Some(commits[0].id.clone());
    if indexed {
        install_index(&mut state, index.clone());
    }
    store.replace_snapshot_for_test(Arc::new(state.clone()));
    set_history_view_state_for_tests(cx, &root, Arc::new(state));
    let history = cx.update(|_, app| root.read(app).main_pane.read(app).history_view.clone());
    if indexed {
        wait_until(cx, "indexed navigation layout and publication", |cx| {
            cx.debug_bounds("indexed_history_viewport").is_some()
                && store.snapshot().repos[0]
                    .history_state
                    .indexed
                    .displayed_index
                    .as_ref()
                    .is_some_and(|shown| Arc::ptr_eq(shown, &index))
        });
    }
    cx.update(|window, app| {
        app.clear_key_bindings();
        crate::app::bind_app_keys_for_test(app);
        crate::app::bind_text_input_keys_for_test(app);
        window.activate();
        let focus = history.read(app).history_panel_focus_handle.clone();
        window.focus(&focus, app);
        let _ = window.draw(app);
    });
    cx.run_until_parked();
    (
        NavigationFixture {
            root,
            history,
            store,
            commits,
        },
        cx,
    )
}

impl NavigationFixture {
    // Measure rendered geometry independently of the navigation helper.
    fn metrics(&self, cx: &mut gpui::VisualTestContext) -> (f64, f64, f64) {
        cx.update(|_, app| {
            self.history.update(app, |history, _| {
                if history.indexed.presentation.is_some() {
                    let scroll = history.scroll_interaction.borrow();
                    let logical = scroll.logical.as_ref().unwrap();
                    return (logical.position(), logical.viewport, logical.height);
                }
                let commits = history
                    .history_cache
                    .as_ref()
                    .unwrap()
                    .base
                    .visible_indices
                    .len();
                let count = history.ensure_history_list_plan().list_len(commits);
                let scroll = history.history_scroll.0.borrow();
                let size = scroll.last_item_size.unwrap();
                (
                    -f64::from(f32::from(scroll.base_handle.offset().y)),
                    f64::from(f32::from(size.item.height)),
                    f64::from(f32::from(size.contents.height)) / count as f64,
                )
            })
        })
    }

    fn page_rows(&self, cx: &mut gpui::VisualTestContext) -> usize {
        let (_, viewport, height) = self.metrics(cx);
        (viewport / height).floor().max(1.0) as usize
    }

    fn resize_to_rows(&self, cx: &mut gpui::VisualTestContext, rows: f64) {
        let (_, viewport, height) = self.metrics(cx);
        let size = cx.update(|window, _| window.window_bounds().get_bounds().size);
        cx.simulate_resize(gpui::size(
            size.width,
            size.height + px((rows * height - viewport) as f32),
        ));
        sync_view_with_store(cx, &self.root);
        assert_eq!(self.page_rows(cx), rows.floor() as usize);
    }

    fn assert_commit(&self, cx: &mut gpui::VisualTestContext, index: usize) {
        let expected = &self.commits[index].id;
        wait_for_store(cx, &self.root, "page destination selected", || {
            self.store.snapshot().repos[0]
                .history_state
                .selected_commit
                .as_ref()
                == Some(expected)
        });
        cx.update(|_, app| {
            let history = self.history.read(app);
            assert_eq!(
                history
                    .active_repo()
                    .unwrap()
                    .history_state
                    .selected_commit
                    .as_ref(),
                Some(expected)
            );
        });
    }

    fn assert_visible(&self, cx: &mut gpui::VisualTestContext, row: usize) {
        let (position, viewport, height) = self.metrics(cx);
        let top = row as f64 * height;
        assert!(top >= position - 0.01, "destination above the viewport");
        assert!(
            top + height <= position + viewport + 0.01,
            "destination below the viewport"
        );
    }
}

fn paging_tracks_viewport_and_repeated_keys(cx: &mut gpui::TestAppContext, indexed: bool) {
    let (fixture, cx) = mount_navigation(cx, indexed, 300);
    fixture.resize_to_rows(cx, 10.5);
    cx.simulate_keystrokes("pagedown");
    fixture.assert_commit(cx, 10);
    fixture.assert_visible(cx, 10);
    cx.simulate_keystrokes("pageup");
    fixture.assert_commit(cx, 0);
    fixture.assert_visible(cx, 0);
    cx.simulate_keystrokes("down");
    fixture.assert_commit(cx, 1);
    cx.simulate_keystrokes("up");
    fixture.assert_commit(cx, 0);

    // No store snapshot is synchronized between these inputs. Each key must
    // step from the previous intent, and only its final target is selected.
    cx.simulate_keystrokes("pagedown pagedown pagedown");
    cx.update(|_, app| {
        let history = fixture.history.read(app);
        assert_eq!(history.pending_history_selections.len(), 3);
    });
    fixture.assert_commit(cx, 30);
    fixture.assert_visible(cx, 30);

    fixture.resize_to_rows(cx, 20.5);
    cx.simulate_keystrokes("pagedown");
    fixture.assert_commit(cx, 50);
    fixture.assert_visible(cx, 50);

    cx.update(|window, app| {
        crate::ui_scale::set_default(app, 150);
        crate::ui_scale::apply_to_window(window, 150);
        fixture.history.update(app, |history, cx| {
            history.ui_scale_percent = 150;
            cx.notify();
        });
        window.refresh();
        let _ = window.draw(app);
    });
    cx.run_until_parked();
    let rows = fixture.page_rows(cx);
    assert!(
        rows > 0 && rows < 20,
        "larger rows reduce the page distance"
    );
    cx.simulate_keystrokes("pagedown");
    fixture.assert_commit(cx, 50 + rows);
    fixture.assert_visible(cx, 50 + rows);
}

#[gpui::test]
fn cached_history_paging_tracks_viewport_and_repeated_keys(cx: &mut gpui::TestAppContext) {
    paging_tracks_viewport_and_repeated_keys(cx, false);
}

#[gpui::test]
fn indexed_history_paging_tracks_viewport_and_repeated_keys(cx: &mut gpui::TestAppContext) {
    paging_tracks_viewport_and_repeated_keys(cx, true);
}

fn paging_clamps_and_respects_focus(cx: &mut gpui::TestAppContext, indexed: bool) {
    let (fixture, cx) = mount_navigation(cx, indexed, 3);
    assert!(fixture.page_rows(cx) > 3);
    cx.simulate_keystrokes("pagedown pagedown");
    fixture.assert_commit(cx, 2);
    fixture.assert_visible(cx, 2);
    cx.simulate_keystrokes("pageup pageup");
    fixture.assert_commit(cx, 0);
    fixture.assert_visible(cx, 0);

    // Modified keys belong to text selection, repository tabs and other
    // contexts, even when the history list has focus.
    cx.simulate_keystrokes("shift-pagedown alt-pagedown secondary-pagedown");
    cx.update(|_, app| {
        assert!(
            fixture
                .history
                .read(app)
                .pending_history_selections
                .is_empty()
        );
    });
    cx.simulate_keystrokes("secondary-f");
    sync_view_with_store(cx, &fixture.root);
    cx.update(|window, app| {
        let history = fixture.history.read(app);
        let input = &history.find.as_ref().unwrap().input;
        assert!(input.read(app).focus_handle().is_focused(window));
    });
    cx.simulate_keystrokes("pagedown pageup");
    cx.update(|_, app| {
        assert!(
            fixture
                .history
                .read(app)
                .pending_history_selections
                .is_empty()
        );
    });
    fixture.assert_commit(cx, 0);

    cx.simulate_keystrokes("escape pagedown");
    fixture.assert_commit(cx, 2);
}

#[gpui::test]
fn cached_history_paging_clamps_and_respects_focus(cx: &mut gpui::TestAppContext) {
    paging_clamps_and_respects_focus(cx, false);
}

#[gpui::test]
fn indexed_history_paging_clamps_and_respects_focus(cx: &mut gpui::TestAppContext) {
    paging_clamps_and_respects_focus(cx, true);
}

fn home_end_select_first_and_last_row(cx: &mut gpui::TestAppContext, indexed: bool) {
    let (fixture, cx) = mount_navigation(cx, indexed, 3);
    cx.simulate_keystrokes("end");
    fixture.assert_commit(cx, 2);
    cx.simulate_keystrokes("home");
    fixture.assert_commit(cx, 0);
    // macOS's Command+Down/Up do the same, as in GitKraken.
    if cfg!(target_os = "macos") {
        cx.simulate_keystrokes("cmd-down");
        fixture.assert_commit(cx, 2);
        cx.simulate_keystrokes("cmd-up");
        fixture.assert_commit(cx, 0);
    }

    for (key, commit) in [("end", 2), ("home", 0)] {
        let mut state = (*fixture.store.snapshot()).clone();
        state.repos[0].history_state.selected_commit = None;
        fixture
            .store
            .replace_snapshot_for_test(Arc::new(state.clone()));
        set_history_view_state_for_tests(cx, &fixture.root, Arc::new(state));
        sync_view_with_store(cx, &fixture.root);
        cx.simulate_keystrokes(key);
        fixture.assert_commit(cx, commit);
    }

    // Without history focus (find input focused), the keys do nothing.
    cx.simulate_keystrokes("secondary-f");
    sync_view_with_store(cx, &fixture.root);
    let before = cx.update(|_, app| fixture.history.read(app).pending_history_selections.len());
    cx.simulate_keystrokes("end");
    cx.update(|_, app| {
        assert_eq!(
            fixture.history.read(app).pending_history_selections.len(),
            before
        );
    });
    fixture.assert_commit(cx, 0);
}

#[gpui::test]
fn cached_history_home_end_select_first_and_last_row(cx: &mut gpui::TestAppContext) {
    home_end_select_first_and_last_row(cx, false);
}

#[gpui::test]
fn indexed_history_home_end_select_first_and_last_row(cx: &mut gpui::TestAppContext) {
    home_end_select_first_and_last_row(cx, true);
}

fn paging_includes_worktree_rows(cx: &mut gpui::TestAppContext, indexed: bool) {
    use gitcomet_core::domain::{FileStatus, FileStatusKind, WorktreeDirtySummary};

    let (fixture, cx) = mount_navigation(cx, indexed, 100);
    let path = PathBuf::from("/tmp/history-page-linked-worktree");
    let mut state = (*fixture.store.snapshot()).clone();
    let repo = &mut state.repos[0];
    repo.history_state.selected_commit = None;
    repo.worktree_status = Loadable::Ready(Arc::new(vec![FileStatus {
        path: "README.md".into(),
        kind: FileStatusKind::Modified,
        conflict: None,
    }]));
    repo.worktree_status_rev += 1;
    repo.worktree_dirty = Loadable::Ready(Arc::new(vec![WorktreeDirtySummary {
        path: path.clone(),
        head: Some(fixture.commits[9].id.clone()),
        branch: Some("linked".into()),
        detached: false,
        added: 1,
        modified: 0,
        deleted: 0,
        staged: Vec::new(),
        unstaged: Vec::new(),
        line_stats: Default::default(),
    }]));
    repo.worktree_dirty_rev += 1;
    fixture
        .store
        .replace_snapshot_for_test(Arc::new(state.clone()));
    set_history_view_state_for_tests(cx, &fixture.root, Arc::new(state));
    fixture.resize_to_rows(cx, 10.5);

    // The working tree is row 0. The linked worktree sits at row 10,
    // immediately above commit 9, so exactly one page must select it.
    cx.simulate_keystrokes("pagedown");
    wait_for_store(
        cx,
        &fixture.root,
        "linked worktree selected by page",
        || {
            fixture.store.snapshot().repos[0]
                .history_state
                .worktree_selection
                .as_ref()
                == Some(&path)
        },
    );
    fixture.assert_visible(cx, 10);
    cx.simulate_keystrokes("pagedown");
    fixture.assert_commit(cx, 18);
    fixture.assert_visible(cx, 20);
    cx.simulate_keystrokes("pageup pageup");
    wait_for_store(cx, &fixture.root, "working tree selected by page", || {
        let snapshot = fixture.store.snapshot();
        let history = &snapshot.repos[0].history_state;
        history.selected_commit.is_none() && history.worktree_selection.is_none()
    });
    fixture.assert_visible(cx, 0);
}

#[gpui::test]
fn cached_history_paging_includes_worktree_rows(cx: &mut gpui::TestAppContext) {
    paging_includes_worktree_rows(cx, false);
}

#[gpui::test]
fn indexed_history_paging_includes_worktree_rows(cx: &mut gpui::TestAppContext) {
    paging_includes_worktree_rows(cx, true);
}

#[gpui::test]
fn cached_history_worktree_rows_open_exact_live_path_menus(cx: &mut gpui::TestAppContext) {
    worktree_row_menus_use_live_paths(cx, false);
}

#[gpui::test]
fn indexed_history_worktree_rows_open_exact_live_path_menus(cx: &mut gpui::TestAppContext) {
    worktree_row_menus_use_live_paths(cx, true);
}

#[gpui::test]
fn cached_worktree_row_double_click_opens_only_a_complete_exact_path_gesture(
    cx: &mut gpui::TestAppContext,
) {
    worktree_row_double_clicks_require_exact_paths(cx, false);
}

#[gpui::test]
fn indexed_worktree_row_double_click_opens_only_a_complete_exact_path_gesture(
    cx: &mut gpui::TestAppContext,
) {
    worktree_row_double_clicks_require_exact_paths(cx, true);
}

fn worktree_row_double_clicks_require_exact_paths(cx: &mut gpui::TestAppContext, indexed: bool) {
    use gitcomet_core::domain::{FileStatus, FileStatusKind, Worktree, WorktreeDirtySummary};

    let (fixture, cx) = mount_navigation(cx, indexed, 8);
    let repo_id = RepoId(1);
    let paths = ["a", "b"].map(|folder| {
        std::env::temp_dir()
            .join("history-row-double-click")
            .join(folder)
    });
    let mut state = (*fixture.store.snapshot()).clone();
    state.repos[0].spec.workdir = std::env::temp_dir().join("history-row-double-click-main");
    let original_ops_rev = state.repos[0].ops_rev;
    // Allocate the original tab through OpenRepo before restoring the mounted
    // snapshot, so the first linked tab receives its own ID instead of ID 1.
    let mut empty = state.clone();
    empty.repos.clear();
    empty.active_repo = None;
    fixture.store.replace_snapshot_for_test(Arc::new(empty));
    let original_path = state.repos[0].spec.workdir.clone();
    fixture.store.dispatch(Msg::OpenRepo(original_path.clone()));
    wait_until(cx, "original repository ID allocated", |_| {
        fixture
            .store
            .snapshot()
            .repos
            .iter()
            .any(|repo| repo.spec.workdir == original_path)
    });
    let repo = &mut state.repos[0];
    repo.head_branch = Loadable::Ready("main".into());
    repo.head_branch_rev += 1;
    repo.worktree_status = Loadable::Ready(Arc::new(vec![]));
    repo.worktree_status_rev += 1;
    repo.worktree_dirty = Loadable::Ready(Arc::new(
        paths
            .iter()
            .enumerate()
            .map(|(ix, path)| WorktreeDirtySummary {
                path: path.clone(),
                head: Some(fixture.commits[0].id.clone()),
                branch: Some(format!("linked-{ix}")),
                modified: 1,
                unstaged: vec![FileStatus {
                    path: "modified.txt".into(),
                    kind: FileStatusKind::Modified,
                    conflict: None,
                }],
                ..Default::default()
            })
            .collect(),
    ));
    repo.worktree_dirty_rev += 1;
    let worktrees = Arc::new(
        paths
            .iter()
            .enumerate()
            .map(|(ix, path)| Worktree {
                path: path.clone(),
                head: Some(fixture.commits[0].id.clone()),
                branch: Some(format!("linked-{ix}")),
                bare: false,
                detached: false,
            })
            .collect::<Vec<_>>(),
    );
    repo.worktrees = Loadable::Ready(worktrees.clone());
    repo.worktrees_rev += 1;
    fixture
        .store
        .replace_snapshot_for_test(Arc::new(state.clone()));
    set_history_view_state_for_tests(cx, &fixture.root, Arc::new(state));
    wait_until(cx, "two double-click worktree bands", |cx| {
        cx.debug_bounds("history_worktree_row_0").is_some()
            && cx.debug_bounds("history_worktree_row_1").is_some()
    });
    let positions = [
        cx.debug_bounds("history_worktree_row_0").unwrap().center(),
        cx.debug_bounds("history_worktree_row_1").unwrap().center(),
    ];
    let event = |cx: &mut gpui::VisualTestContext, row: usize, count, down, modifiers| {
        if down {
            cx.simulate_event(gpui::MouseDownEvent {
                position: positions[row],
                button: gpui::MouseButton::Left,
                click_count: count,
                modifiers,
                ..Default::default()
            });
        } else {
            cx.simulate_event(gpui::MouseUpEvent {
                position: positions[row],
                button: gpui::MouseButton::Left,
                click_count: count,
                modifiers,
                ..Default::default()
            });
        }
        cx.run_until_parked();
    };
    let click = |cx: &mut gpui::VisualTestContext, row, count, modifiers| {
        event(cx, row, count, true, modifiers);
        event(cx, row, count, false, modifiers);
    };
    let no_open = |cx: &mut gpui::VisualTestContext| {
        fixture
            .store
            .barrier_for_test()
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        sync_view_with_store(cx, &fixture.root);
        let snapshot = fixture.store.snapshot();
        assert_eq!(
            snapshot.repos.len(),
            1,
            "a rejected gesture must not create a tab"
        );
        assert_eq!(snapshot.active_repo, Some(repo_id));
    };
    let plain = gpui::Modifiers::default();
    click(cx, 1, 1, plain);
    no_open(cx);
    assert_eq!(
        fixture.store.snapshot().repos[0]
            .history_state
            .worktree_selection
            .as_ref(),
        Some(&paths[1])
    );
    assert!(
        cx.debug_bounds("worktree_uncommitted_body").is_some(),
        "single click still previews Worktree changes"
    );
    for (first, second) in [(0, 1), (1, 0)] {
        click(cx, first, 1, plain);
        click(cx, second, 2, plain);
        no_open(cx);
    }
    // A lone second release, then a second press released on the other band.
    click(cx, 1, 1, plain);
    event(cx, 1, 2, false, plain);
    no_open(cx);
    click(cx, 1, 1, plain);
    event(cx, 1, 2, true, plain);
    no_open(cx);
    event(cx, 0, 2, false, plain);
    no_open(cx);
    for modifiers in [
        gpui::Modifiers {
            shift: true,
            ..plain
        },
        gpui::Modifiers {
            control: true,
            ..plain
        },
        gpui::Modifiers {
            platform: true,
            ..plain
        },
        gpui::Modifiers { alt: true, ..plain },
    ] {
        click(cx, 1, 1, plain);
        click(cx, 1, 2, modifiers);
        no_open(cx);
    }
    click(
        cx,
        1,
        1,
        gpui::Modifiers {
            shift: true,
            ..plain
        },
    );
    click(cx, 1, 2, plain);
    no_open(cx);

    // Remove the authoritative entry between clicks while keeping its dirty
    // summary. Both an interrupted and a fresh stale gesture must be refused.
    click(cx, 1, 1, plain);
    let mut state = (*fixture.store.snapshot()).clone();
    state.repos[0].worktrees = Loadable::Ready(Arc::new(vec![worktrees[0].clone()]));
    state.repos[0].worktrees_rev += 1;
    fixture
        .store
        .replace_snapshot_for_test(Arc::new(state.clone()));
    set_history_view_state_for_tests(cx, &fixture.root, Arc::new(state));
    click(cx, 1, 2, plain);
    no_open(cx);
    click(cx, 1, 1, plain);
    click(cx, 1, 2, plain);
    no_open(cx);
    let mut state = (*fixture.store.snapshot()).clone();
    state.repos[0].worktrees = Loadable::Ready(worktrees);
    state.repos[0].worktrees_rev += 1;
    fixture
        .store
        .replace_snapshot_for_test(Arc::new(state.clone()));
    set_history_view_state_for_tests(cx, &fixture.root, Arc::new(state));

    click(cx, 1, 1, plain);
    click(cx, 1, 2, plain);
    wait_for_store(
        cx,
        &fixture.root,
        "complete second-path gesture opens its tab",
        || {
            let snapshot = fixture.store.snapshot();
            snapshot
                .repos
                .iter()
                .any(|repo| repo.spec.workdir == paths[1] && snapshot.active_repo == Some(repo.id))
        },
    );
    let opened = fixture.store.snapshot().active_repo.unwrap();
    assert_ne!(opened, repo_id);
    assert_eq!(fixture.store.snapshot().repos.len(), 2);
    assert!(
        !fixture
            .store
            .snapshot()
            .repos
            .iter()
            .any(|repo| repo.spec.workdir == paths[0])
    );
    fixture.store.dispatch(Msg::SetActiveRepo { repo_id });
    wait_for_store(
        cx,
        &fixture.root,
        "return to the original graph tab",
        || fixture.store.snapshot().active_repo == Some(repo_id),
    );
    click(cx, 1, 1, plain);
    click(cx, 1, 2, plain);
    wait_for_store(
        cx,
        &fixture.root,
        "already-open worktree tab reused",
        || fixture.store.snapshot().active_repo == Some(opened),
    );
    let snapshot = fixture.store.snapshot();
    assert_eq!(snapshot.repos.len(), 2);
    let original = snapshot
        .repos
        .iter()
        .find(|repo| repo.id == repo_id)
        .unwrap();
    // Returning to a synthetic tab refreshes HEAD without a repository handle.
    // Assert routing identity here; the real-Git fixture verifies HEAD/index.
    assert_eq!(original.spec.workdir, original_path);
    assert_eq!(
        original.ops_rev, original_ops_rev,
        "opening a worktree must not dispatch checkout"
    );
    assert_eq!(original.local_actions_in_flight, 0);
}

fn worktree_row_menus_use_live_paths(cx: &mut gpui::TestAppContext, indexed: bool) {
    use gitcomet_core::domain::{Worktree, WorktreeDirtySummary};

    let (fixture, cx) = mount_navigation(cx, indexed, 8);
    let repo_id = RepoId(1);
    let paths = [
        PathBuf::from("/tmp/history-row-menu-a"),
        PathBuf::from("/tmp/history-row-menu-b"),
    ];
    let mut state = (*fixture.store.snapshot()).clone();
    let repo = &mut state.repos[0];
    repo.head_branch = Loadable::Ready("main".into());
    repo.head_branch_rev += 1;
    repo.worktree_status = Loadable::Ready(Arc::new(vec![]));
    repo.worktree_status_rev += 1;
    repo.worktree_dirty = Loadable::Ready(Arc::new(
        paths
            .iter()
            .enumerate()
            .map(|(ix, path)| WorktreeDirtySummary {
                path: path.clone(),
                head: Some(fixture.commits[0].id.clone()),
                branch: Some(format!("stale-summary-{ix}")),
                detached: false,
                added: 0,
                modified: 1,
                deleted: 0,
                staged: vec![],
                unstaged: vec![],
                line_stats: Default::default(),
            })
            .collect(),
    ));
    repo.worktree_dirty_rev += 1;
    repo.worktrees = Loadable::Ready(Arc::new(
        paths
            .iter()
            .enumerate()
            .map(|(ix, path)| Worktree {
                path: path.clone(),
                head: Some(fixture.commits[0].id.clone()),
                branch: Some(format!("live-branch-{ix}")),
                bare: false,
                detached: false,
            })
            .collect(),
    ));
    repo.worktrees_rev += 1;
    fixture
        .store
        .replace_snapshot_for_test(Arc::new(state.clone()));
    set_history_view_state_for_tests(cx, &fixture.root, Arc::new(state));
    wait_until(cx, "two dirty worktree rows sharing one commit", |cx| {
        cx.debug_bounds("history_worktree_row_0").is_some()
            && cx.debug_bounds("history_worktree_row_1").is_some()
    });
    let second = cx.debug_bounds("history_worktree_row_1").unwrap().center();
    let selected_before = fixture.store.snapshot().repos[0]
        .history_state
        .selected_commit
        .clone();
    let menu_kind = |cx: &mut gpui::VisualTestContext| {
        cx.update(|_, app| crate::view::test_support::popover_kind(fixture.root.read(app), app))
    };
    let expected_menu = PopoverKind::worktree(
        repo_id,
        WorktreePopoverKind::Menu {
            path: paths[1].clone(),
            branch: Some("live-branch-1".into()),
        },
    );
    cx.simulate_mouse_down(second, gpui::MouseButton::Right, gpui::Modifiers::default());
    cx.simulate_mouse_up(second, gpui::MouseButton::Right, gpui::Modifiers::default());
    cx.run_until_parked();
    crate::test_support::refresh_and_draw(cx);
    assert_eq!(menu_kind(cx), Some(expected_menu.clone()));
    assert_eq!(
        fixture.store.snapshot().repos[0]
            .history_state
            .selected_commit,
        selected_before
    );
    assert!(
        fixture.store.snapshot().repos[0]
            .history_state
            .worktree_selection
            .is_none()
    );

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(menu_kind(cx), None);
    cx.update(|window, app| {
        assert!(
            fixture
                .history
                .read(app)
                .history_panel_focus_handle
                .is_focused(window)
        )
    });
    // The two bands precede commit 0. Up therefore selects the second path,
    // not a worktree guessed from the shared commit SHA.
    cx.simulate_keystrokes("up");
    wait_for_store(
        cx,
        &fixture.root,
        "navigate into the second worktree band",
        || {
            fixture.store.snapshot().repos[0]
                .history_state
                .worktree_selection
                .as_ref()
                == Some(&paths[1])
        },
    );
    cx.simulate_keystrokes("shift-f10");
    cx.run_until_parked();
    crate::test_support::refresh_and_draw(cx);
    assert_eq!(menu_kind(cx), Some(expected_menu.clone()));
    assert_eq!(
        fixture.store.snapshot().repos[0]
            .history_state
            .worktree_selection
            .as_ref(),
        Some(&paths[1])
    );
    // The shared dialogs replace the context menu; cancelling either still
    // returns to the graph row that opened the menu.
    for kind in [
        WorktreePopoverKind::FinishSetup {
            preselected: vec![paths[1].clone()],
            target: None,
            mode: None,
        },
        WorktreePopoverKind::RemoveConfirm {
            path: paths[1].clone(),
            branch: Some("live-branch-1".into()),
        },
    ] {
        let dialog = PopoverKind::worktree(repo_id, kind);
        cx.update(|window, app| {
            let host = fixture.root.read(app).popover_host.clone();
            host.update(app, |host, cx| {
                host.context_menu_activate_action(
                    crate::view::panels::ContextMenuAction::OpenPopover {
                        kind: dialog.clone(),
                    },
                    window,
                    cx,
                );
            });
        });
        crate::test_support::refresh_and_draw(cx);
        assert_eq!(menu_kind(cx), Some(dialog));
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert_eq!(menu_kind(cx), None);
        cx.update(|window, app| {
            assert!(
                fixture
                    .history
                    .read(app)
                    .history_panel_focus_handle
                    .is_focused(window)
            )
        });
        cx.simulate_keystrokes("shift-f10");
        cx.run_until_parked();
        crate::test_support::refresh_and_draw(cx);
        assert_eq!(menu_kind(cx), Some(expected_menu.clone()));
    }

    // A released menu click can still carry the old branch while an update
    // changes the worktree. Even that old action must not open a confirmation.
    let mut state = (*fixture.store.snapshot()).clone();
    let repo = &mut state.repos[0];
    let mut worktrees = repo.worktrees.ready().unwrap().as_ref().clone();
    worktrees[1].branch = Some("renamed-while-open".into());
    repo.worktrees = Loadable::Ready(Arc::new(worktrees));
    repo.worktrees_rev += 1;
    fixture
        .store
        .replace_snapshot_for_test(Arc::new(state.clone()));
    set_history_view_state_for_tests(cx, &fixture.root, Arc::new(state));
    cx.update(|window, app| {
        let host = fixture.root.read(app).popover_host.clone();
        host.update(app, |host, cx| {
            host.context_menu_activate_action(
                crate::view::panels::ContextMenuAction::OpenPopover {
                    kind: PopoverKind::worktree(
                        repo_id,
                        WorktreePopoverKind::RemoveConfirm {
                            path: paths[1].clone(),
                            branch: Some("live-branch-1".into()),
                        },
                    ),
                },
                window,
                cx,
            );
        });
    });
    cx.run_until_parked();
    assert_eq!(menu_kind(cx), None);

    // A stale dirty summary can outlive the authoritative worktree list. Its
    // row remains visible, but neither menu entry point may operate on it.
    let mut state = (*fixture.store.snapshot()).clone();
    state.repos[0].worktrees = Loadable::Ready(Arc::new(vec![Worktree {
        path: paths[0].clone(),
        head: Some(fixture.commits[0].id.clone()),
        branch: Some("live-branch-0".into()),
        bare: false,
        detached: false,
    }]));
    state.repos[0].worktrees_rev += 1;
    fixture
        .store
        .replace_snapshot_for_test(Arc::new(state.clone()));
    set_history_view_state_for_tests(cx, &fixture.root, Arc::new(state));
    let stale = cx
        .debug_bounds("history_worktree_row_1")
        .expect("stale dirty band remains visible")
        .center();
    cx.simulate_mouse_down(stale, gpui::MouseButton::Right, gpui::Modifiers::default());
    cx.simulate_mouse_up(stale, gpui::MouseButton::Right, gpui::Modifiers::default());
    cx.run_until_parked();
    assert_eq!(menu_kind(cx), None);
    assert_eq!(
        fixture.store.snapshot().repos[0]
            .history_state
            .worktree_selection
            .as_ref(),
        Some(&paths[1]),
        "the keyboard rejection must be tested with the stale worktree still selected"
    );
    cx.simulate_keystrokes("shift-f10");
    cx.run_until_parked();
    assert_eq!(menu_kind(cx), None);
    assert_eq!(
        fixture.store.snapshot().repos[0].head_branch,
        Loadable::Ready("main".into())
    );
    assert_eq!(fixture.store.snapshot().repos[0].local_actions_in_flight, 0);
}

#[gpui::test]
fn cached_history_paging_uses_retained_rows_while_loading(cx: &mut gpui::TestAppContext) {
    let (fixture, cx) = mount_navigation(cx, false, 100);
    fixture.resize_to_rows(cx, 10.5);
    let mut state = (*fixture.store.snapshot()).clone();
    let repo = &mut state.repos[0];
    let Loadable::Ready(page) = &repo.log else {
        panic!("loaded fixture")
    };
    repo.history_state.retained_log_while_loading = Some(page.clone());
    repo.log = Loadable::Loading;
    repo.log_rev += 1;
    repo.history_state.log = Loadable::Loading;
    repo.history_state.log_rev += 1;
    fixture
        .store
        .replace_snapshot_for_test(Arc::new(state.clone()));
    set_history_view_state_for_tests(cx, &fixture.root, Arc::new(state));
    cx.simulate_keystrokes("pagedown");
    fixture.assert_commit(cx, 10);
    fixture.assert_visible(cx, 10);
}

#[gpui::test]
fn history_paging_without_rows_or_layout_is_safe(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new_test(Arc::new(BlockingBackend));
    let (view, cx) =
        cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));
    cx.update(|_, app| {
        let history = view.read(app).main_pane.read(app).history_view.clone();
        history.update(app, |history, cx| {
            assert_eq!(history.history_page_rows(), 1);
            assert!(!history.history_select_page(1, cx));
            assert!(!history.history_select_page(-1, cx));
            assert!(history.pending_history_selections.is_empty());
        });
    });
}

/// With nothing selected, a page key enters the list at its first row in
/// both modes. Cached history jumped to its last loaded row on PageUp, and
/// indexed history skipped the whole first page on PageDown.
fn paging_without_selection_enters_at_the_first_row(cx: &mut gpui::TestAppContext, indexed: bool) {
    let (fixture, cx) = mount_navigation(cx, indexed, 100);
    fixture.resize_to_rows(cx, 10.5);
    for key in ["pageup", "pagedown"] {
        let mut state = (*fixture.store.snapshot()).clone();
        state.repos[0].history_state.selected_commit = None;
        fixture
            .store
            .replace_snapshot_for_test(Arc::new(state.clone()));
        set_history_view_state_for_tests(cx, &fixture.root, Arc::new(state));
        sync_view_with_store(cx, &fixture.root);
        cx.simulate_keystrokes(key);
        fixture.assert_commit(cx, 0);
        fixture.assert_visible(cx, 0);
    }
}

#[gpui::test]
fn cached_history_paging_without_selection_enters_at_the_first_row(cx: &mut gpui::TestAppContext) {
    paging_without_selection_enters_at_the_first_row(cx, false);
}

#[gpui::test]
fn indexed_history_paging_without_selection_enters_at_the_first_row(cx: &mut gpui::TestAppContext) {
    paging_without_selection_enters_at_the_first_row(cx, true);
}

/// Holding a page key at either end must not re-select the same row on every
/// repeat; each re-selection dispatches to the store and re-scrolls.
fn paging_at_a_boundary_keeps_the_selection(cx: &mut gpui::TestAppContext, indexed: bool) {
    let (fixture, cx) = mount_navigation(cx, indexed, 3);
    for (key, last) in [("pageup", 0), ("pagedown", 2)] {
        cx.simulate_keystrokes(key);
        fixture.assert_commit(cx, last);
        cx.simulate_keystrokes(&format!("{key} {key}"));
        cx.update(|_, app| {
            assert!(
                fixture
                    .history
                    .read(app)
                    .pending_history_selections
                    .is_empty(),
                "{key} at the boundary re-selected the current row"
            );
        });
        fixture.assert_commit(cx, last);
    }
}

#[gpui::test]
fn cached_history_paging_at_a_boundary_keeps_the_selection(cx: &mut gpui::TestAppContext) {
    paging_at_a_boundary_keeps_the_selection(cx, false);
}

#[gpui::test]
fn indexed_history_paging_at_a_boundary_keeps_the_selection(cx: &mut gpui::TestAppContext) {
    paging_at_a_boundary_keeps_the_selection(cx, true);
}

/// Rows interleave two branches. The merge's first parent is main1, while
/// side2/main2 share an ancestor. An upward step must retain the route used
/// to reach that ancestor rather than jumping to its nearer main child.
fn mount_interleaved_navigation(
    cx: &mut gpui::TestAppContext,
    indexed: bool,
) -> (NavigationFixture, &mut gpui::VisualTestContext) {
    use gitcomet_core::history_index::HistoryIndexBuilder;
    use gitcomet_core::services::{CancellationToken, HistorySnapshot};
    let mut builder = HistoryIndexBuilder::new(
        HistorySnapshot("navigation-interleaved".into()),
        LogScope::AllBranches,
        20,
    )
    .unwrap();
    let raw = |row: usize| {
        let mut id = [0u8; 20];
        id[19] = (7 - row) as u8;
        id
    };
    let parents: &[&[usize]] = &[&[2, 1], &[3], &[4], &[5], &[5], &[6], &[]];
    let mut commits = Vec::new();
    for (row, parents) in parents.iter().enumerate() {
        let id = raw(row);
        let parents: Vec<_> = parents.iter().map(|row| raw(*row)).collect();
        builder
            .push(&id, parents.iter().map(|id| id.as_slice()), false)
            .unwrap();
        commits.push(Commit {
            id: CommitId(gitcomet_core::hex::encode(&id).into()),
            parent_ids: parents
                .iter()
                .map(|id| CommitId(gitcomet_core::hex::encode(id).into()))
                .collect(),
            summary: format!("interleaved row {row}").into(),
            author: "author".into(),
            time: SystemTime::UNIX_EPOCH,
        });
    }
    mount_navigation_graph(
        cx,
        indexed,
        builder.finish(&CancellationToken::new()).unwrap(),
        commits,
    )
}

fn keyboard_first_parent_paths(cx: &mut gpui::TestAppContext, indexed: bool) {
    let (fixture, cx) = mount_interleaved_navigation(cx, indexed);
    // Ordinary Vim keys visit adjacent displayed rows, across branch lanes.
    for (key, row) in [("j", 1), ("j", 2), ("k", 1), ("k", 0)] {
        cx.simulate_keystrokes(key);
        fixture.assert_commit(cx, row);
        fixture.assert_visible(cx, row);
    }
    // First-parent navigation skips the interleaved side lane and ignores
    // the merge's second parent. Both arrow and Vim spellings are exercised.
    for (key, row) in [
        ("shift-down", 2),
        ("shift-j", 4),
        ("shift-down", 5),
        ("shift-k", 4),
        ("shift-up", 2),
        ("shift-k", 0),
    ] {
        cx.simulate_keystrokes(key);
        fixture.assert_commit(cx, row);
        fixture.assert_visible(cx, row);
    }
    cx.simulate_keystrokes("shift-up shift-k");
    cx.update(|_, app| {
        assert!(
            fixture
                .history
                .read(app)
                .pending_history_selections
                .is_empty()
        )
    });
    fixture.assert_commit(cx, 0);

    // Enter side1 by ordinary row navigation. Return from the shared ancestor
    // along side2, even though main2 is the closer child on screen.
    cx.simulate_keystrokes("j");
    fixture.assert_commit(cx, 1);
    for (key, row) in [
        ("shift-j", 3),
        ("shift-down", 5),
        ("shift-k", 3),
        ("shift-up", 1),
    ] {
        cx.simulate_keystrokes(key);
        fixture.assert_commit(cx, row);
    }
    // Multiple inputs before synchronizing the store use the pending intent.
    cx.simulate_keystrokes("shift-j shift-down shift-k");
    fixture.assert_commit(cx, 3);
    fixture.assert_visible(cx, 3);
    cx.simulate_keystrokes("shift-j shift-j");
    fixture.assert_commit(cx, 6);
    cx.simulate_keystrokes("shift-down shift-j j");
    cx.update(|_, app| {
        assert!(
            fixture
                .history
                .read(app)
                .pending_history_selections
                .is_empty()
        )
    });
    fixture.assert_commit(cx, 6);

    cx.simulate_keystrokes("secondary-f");
    sync_view_with_store(cx, &fixture.root);
    cx.update(|window, app| {
        let history = fixture.history.read(app);
        assert!(
            history
                .find
                .as_ref()
                .unwrap()
                .input
                .read(app)
                .focus_handle()
                .is_focused(window)
        );
    });
    cx.simulate_keystrokes("j k shift-j shift-k shift-down shift-up");
    fixture.assert_commit(cx, 6);
    cx.update(|_, app| {
        assert!(
            fixture
                .history
                .read(app)
                .pending_history_selections
                .is_empty()
        )
    });
}

#[gpui::test]
fn cached_history_keyboard_follows_first_parent_and_retraces_side_branch(
    cx: &mut gpui::TestAppContext,
) {
    keyboard_first_parent_paths(cx, false);
}

#[gpui::test]
fn indexed_history_keyboard_follows_first_parent_and_retraces_side_branch(
    cx: &mut gpui::TestAppContext,
) {
    keyboard_first_parent_paths(cx, true);
}

fn branch_keys_keep_worktree_rows(cx: &mut gpui::TestAppContext, indexed: bool) {
    use gitcomet_core::domain::{FileStatus, FileStatusKind, WorktreeDirtySummary};
    let (fixture, cx) = mount_navigation(cx, indexed, 12);
    let path = PathBuf::from("/tmp/history-branch-navigation-worktree");
    let mut state = (*fixture.store.snapshot()).clone();
    let repo = &mut state.repos[0];
    repo.history_state.selected_commit = None;
    repo.worktree_status = Loadable::Ready(Arc::new(vec![FileStatus {
        path: "README.md".into(),
        kind: FileStatusKind::Modified,
        conflict: None,
    }]));
    repo.worktree_status_rev += 1;
    repo.worktree_dirty = Loadable::Ready(Arc::new(vec![WorktreeDirtySummary {
        path: path.clone(),
        head: Some(fixture.commits[3].id.clone()),
        branch: Some("linked".into()),
        detached: false,
        added: 1,
        modified: 0,
        deleted: 0,
        staged: Vec::new(),
        unstaged: Vec::new(),
        line_stats: Default::default(),
    }]));
    repo.worktree_dirty_rev += 1;
    for selection in [None, Some(path)] {
        state.repos[0].history_state.worktree_selection = selection.clone();
        fixture
            .store
            .replace_snapshot_for_test(Arc::new(state.clone()));
        set_history_view_state_for_tests(cx, &fixture.root, Arc::new(state.clone()));
        sync_view_with_store(cx, &fixture.root);
        cx.simulate_keystrokes("shift-down shift-j shift-up shift-k");
        sync_view_with_store(cx, &fixture.root);
        let snapshot = fixture.store.snapshot();
        assert!(snapshot.repos[0].history_state.selected_commit.is_none());
        assert_eq!(
            snapshot.repos[0].history_state.worktree_selection,
            selection
        );
        cx.update(|_, app| {
            assert!(
                fixture
                    .history
                    .read(app)
                    .pending_history_selections
                    .is_empty()
            )
        });
    }
}

#[gpui::test]
fn cached_history_branch_keys_keep_worktree_rows(cx: &mut gpui::TestAppContext) {
    branch_keys_keep_worktree_rows(cx, false);
}

#[gpui::test]
fn indexed_history_branch_keys_keep_worktree_rows(cx: &mut gpui::TestAppContext) {
    branch_keys_keep_worktree_rows(cx, true);
}

#[gpui::test]
fn indexed_history_branch_boundary_cancels_a_late_find_jump(cx: &mut gpui::TestAppContext) {
    use gitcomet_core::history_find::HistoryFindQuery;
    use gitcomet_core::text_search::TextSearchOptions;

    for queued_step in [false, true] {
        let (fixture, cx) = mount_interleaved_navigation(cx, true);
        cx.simulate_keystrokes("secondary-f");
        sync_view_with_store(cx, &fixture.root);
        cx.update(|window, app| {
            fixture.history.update(app, |history, cx| {
                // No indexed results exist yet. Exercise both a query's
                // first-match intent and an Enter step waiting on the scan.
                history.set_history_find_query("late", true);
                if queued_step {
                    assert!(history.history_find_step(true, cx));
                }
                window.focus(&history.history_panel_focus_handle, cx);
            });
        });
        // Row zero has no first-parent child. Even though it cannot move,
        // the user's graph input must cancel the older find intent.
        cx.simulate_keystrokes("shift-up");
        assert_eq!(
            fixture.store.snapshot().repos[0]
                .history_state
                .selected_commit
                .as_ref(),
            Some(&fixture.commits[0].id),
        );

        let mut state = (*fixture.store.snapshot()).clone();
        let repo = &mut state.repos[0];
        let index = repo.history_state.indexed.displayed_index.as_ref().unwrap();
        let results = &mut repo.history_state.find;
        results.query = HistoryFindQuery::new("late", TextSearchOptions::default());
        results.index = Some(Arc::downgrade(index));
        results.stashes_rev = repo.stashes_rev;
        results.matches = Arc::new(vec![Arc::from([1u32])]);
        results.done = true;
        results.error = None;
        results.rev += 1;
        fixture
            .store
            .replace_snapshot_for_test(Arc::new(state.clone()));
        cx.update(|_, app| {
            fixture.history.update(app, |history, cx| {
                history.state = Arc::new(state);
                assert_eq!(history.history_find_matches().unwrap().visible, vec![1]);
                assert!(
                    !history.sync_history_find(cx),
                    "a late result must not revive the cancelled intent (queued_step={queued_step})"
                );
                assert!(history.pending_history_selections.is_empty());
            });
        });
        fixture.assert_commit(cx, 0);
    }
}
