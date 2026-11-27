//! "Open the next file after staging its last change": once Stage, Unstage
//! or Discard leaves the open file with nothing in its list, the diff opens
//! the file's neighbour there; with the setting off it stays.

use super::*;

const BASE: &str = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n";
const ONE_CHANGE: &str = "one\nTWO\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n";
/// Two change blocks far enough apart to be two hunks.
const TWO_CHANGES: &str = "one\nTWO\nthree\nfour\nfive\nsix\nseven\neight\nnine\nTEN\n";

#[derive(Clone, Copy, Debug)]
enum Trigger {
    /// A key pressed in the diff, which acts on the outlined first block.
    Key(&'static str),
    /// "Stage hunk" in the first hunk's menu.
    StageHunkMenu,
    /// "Discard" in the open file's row menu, then the dialog's Discard.
    DiscardFileDialog,
    /// Discard `c.txt` through the dialog, then stage the open file the way
    /// a status row does, which does not arm the advance.
    DiscardOtherThenStageFromRow,
    FileButton,
    RowButton,
    LongFileButton,
    FileButtonThenOpenOther,
    FileButtonThenClose,
}

#[derive(Clone, Copy, Debug)]
struct Case {
    area: DiffArea,
    /// The file opened, of `a.txt`, `b.txt` and `c.txt`; all are changed.
    open: &'static str,
    /// What `a.txt` holds; the others always have one change.
    a_contents: &'static str,
    trigger: Trigger,
    enabled: bool,
    partial_staged: bool,
    index_locked: bool,
}

/// The file the diff shows once the action and the status after it settled.
fn shown_after(cx: &mut gpui::TestAppContext, case: Case) -> Option<String> {
    let label = format!("{case:?}");
    let dir = tempfile::tempdir().unwrap();
    let mut scenario = ScenarioApp {
        app: cx.new_app(),
        store: None,
        finished: false,
    };
    restore_repository(dir.path(), clean_repository_template());
    for name in ["a.txt", "b.txt", "c.txt"] {
        std::fs::write(dir.path().join(name), BASE).unwrap();
        git(dir.path(), &["add", name]);
    }
    if matches!(case.trigger, Trigger::LongFileButton) {
        for ix in 0..60 {
            let name = format!("b-{ix:03}.txt");
            std::fs::write(dir.path().join(&name), BASE).unwrap();
            git(dir.path(), &["add", &name]);
        }
    }
    commit(dir.path(), "base");
    std::fs::write(dir.path().join("a.txt"), case.a_contents).unwrap();
    std::fs::write(dir.path().join("b.txt"), ONE_CHANGE).unwrap();
    std::fs::write(dir.path().join("c.txt"), ONE_CHANGE).unwrap();
    if case.area == DiffArea::Staged {
        git(dir.path(), &["add", "a.txt", "b.txt", "c.txt"]);
    }
    if case.partial_staged {
        git(dir.path(), &["add", "a.txt"]);
        std::fs::write(dir.path().join("a.txt"), TWO_CHANGES).unwrap();
    }

    if matches!(case.trigger, Trigger::LongFileButton) {
        // Destination already contains 60 paths before c.txt is transferred.
        for ix in 0..60 {
            let name = format!("b-{ix:03}.txt");
            std::fs::write(dir.path().join(&name), ONE_CHANGE).unwrap();
            if case.area == DiffArea::Unstaged {
                git(dir.path(), &["add", &name]);
            }
        }
    }

    let backend = gitcomet_git_gix::GixBackend.open(dir.path()).unwrap();
    let status = backend
        .status(&gitcomet_core::services::CancellationToken::new())
        .unwrap();
    let mut repo = opening_repo_state(REPO, dir.path());
    repo.open = Loadable::Ready(());
    repo.repository_mode = Some(gitcomet_core::domain::RepositoryMode::WorkingTree);
    repo.worktree_status = Loadable::Ready(status.unstaged.clone());
    repo.staged_status = Loadable::Ready(status.staged.clone());
    repo.status = Loadable::Ready(Arc::new(status));
    let (store, events) = AppStore::new_test(Arc::new(TestBackend));
    scenario.store = Some(store.clone());
    store.disable_repo_monitors_for_test();
    let store_for_view = store.clone();
    let (view, cx) = scenario
        .app
        .add_window_view(|window, cx| GitCometView::new(store_for_view, events, None, window, cx));
    crate::view::test_support::drain_store_worker(&view, cx);
    super::super::shortcuts::apply_state(cx, &view, app_state_with_repo(repo, REPO));
    store.insert_repo_for_test(REPO, backend);
    cx.simulate_resize(gpui::size(px(1600.0), px(1000.0)));
    cx.update(|_, app| {
        view.read(app)
            .diff_renderer
            .clone()
            .update(app, |pane, _cx| {
                pane.set_open_next_file_after_last_change(case.enabled);
            });
    });

    let target = DiffTarget::working_tree(case.open.into(), case.area);
    store.dispatch(Msg::SelectDiff {
        repo_id: REPO,
        target: target.clone(),
    });
    wait_for(cx, &view, &store, &label, |repo| {
        repo.diff_state.diff_target.as_ref() == Some(&target)
            && matches!(repo.diff_state.diff, Loadable::Ready(_))
            && matches!(repo.diff_state.diff_file, Loadable::Ready(Some(_)))
    });
    // Opening the diff outlines its first change block, which S and U take.
    super::super::shortcuts::wait_until(cx, "the first block outlined", |cx| {
        cx.update(|_, app| {
            view.update(app, |this, cx| {
                crate::view::test_support::sync_store_snapshot(this, cx);
            });
            let pane = view.read(app).diff_renderer.read(app);
            pane.live_focused_change_block().is_some()
        })
    });
    cx.update(|window, app| {
        app.clear_key_bindings();
        crate::app::bind_app_keys_for_test(app);
        crate::app::bind_text_input_keys_for_test(app);
        crate::app::install_global_diff_shortcut_fallback_for_test(app);
        let handle = view
            .read(app)
            .diff_renderer
            .read(app)
            .diff_panel_focus_handle
            .clone();
        window.focus(&handle, app);
    });
    draw_and_drain_test_window(cx);

    let mut before = store.snapshot().repos[0].ops_rev;
    if case.index_locked {
        std::fs::write(dir.path().join(".git/index.lock"), b"locked for test").unwrap();
    }
    match case.trigger {
        Trigger::FileButton
        | Trigger::LongFileButton
        | Trigger::FileButtonThenOpenOther
        | Trigger::FileButtonThenClose => {
            click(cx, "diff_bar_stage");
            if matches!(case.trigger, Trigger::FileButtonThenOpenOther) {
                store.dispatch(Msg::SelectDiff {
                    repo_id: REPO,
                    target: DiffTarget::working_tree("b.txt".into(), case.area),
                });
            } else if matches!(case.trigger, Trigger::FileButtonThenClose) {
                cx.update(|_, app| {
                    view.read(app)
                        .diff_renderer
                        .clone()
                        .update(app, |pane, cx| {
                            pane.clear_diff_selection_or_exit(REPO, cx);
                        });
                });
            }
        }
        Trigger::RowButton => {
            let section = match case.area {
                DiffArea::Unstaged => "unstaged",
                DiffArea::Staged => "staged",
            };
            let row = format!("status_row_{}_{section}_0", REPO.0);
            let bounds = cx.debug_bounds(Box::leak(row.into_boxed_str())).unwrap();
            cx.simulate_mouse_move(bounds.center(), None, Modifiers::default());
            draw_and_drain_test_window(cx);
            click(cx, &format!("status_stage_button_{}_{section}_0", REPO.0));
        }
        Trigger::Key(key) => {
            cx.simulate_keystrokes(key);
            draw_and_drain_test_window(cx);
            // U in an unstaged diff asks before discarding; accept it.
            let asks = cx.update(|_, app| {
                matches!(
                    view.read(app)
                        .popover_host
                        .read(app)
                        .popover_kind_for_tests(),
                    Some(PopoverKind::DiscardLinesConfirm { .. })
                )
            });
            if asks {
                click(cx, "discard_lines_go");
            }
        }
        Trigger::StageHunkMenu => cx.update(|window, app| {
            let pane = view.read(app).diff_renderer.read(app);
            let src_ix = (0..pane.patch_diff_row_len())
                .find(|ix| {
                    pane.patch_diff_row(*ix)
                        .is_some_and(|line| line.text.starts_with("@@"))
                })
                .expect("a hunk header");
            view.read(app).popover_host.clone().update(app, |host, cx| {
                host.context_menu_activate_action(
                    ContextMenuAction::StageHunk {
                        repo_id: REPO,
                        src_ix,
                    },
                    window,
                    cx,
                );
            });
        }),
        Trigger::DiscardFileDialog => discard_through_dialog(cx, &view, case.open),
        Trigger::DiscardOtherThenStageFromRow => {
            discard_through_dialog(cx, &view, "c.txt");
            draw_and_drain_test_window(cx);
            wait_settled(cx, &view, &store, &label, before);
            before = store.snapshot().repos[0].ops_rev;
            store.dispatch(Msg::StagePath {
                repo_id: REPO,
                path: case.open.into(),
            });
        }
    }
    draw_and_drain_test_window(cx);
    wait_settled(cx, &view, &store, &label, before);
    assert_eq!(
        store.snapshot().repos[0].feedback.last_error.is_some(),
        case.index_locked,
        "{label}: {:?}",
        store.snapshot().repos[0].feedback.last_error
    );
    // Until the pane settles: either it opened a file or it disarmed.
    super::super::shortcuts::wait_until(cx, "the pane to settle", |cx| {
        cx.update(|_, app| {
            view.update(app, |this, cx| {
                crate::view::test_support::sync_store_snapshot(this, cx);
            });
            let pane = view.read(app).diff_renderer.read(app);
            pane.next_file_advance.is_none() && pane.stage_file_transfer.is_none()
        })
    });
    crate::view::test_support::drain_store_worker(&view, cx);
    draw_and_drain_test_window(cx);
    let shown = match store.snapshot().repos[0].diff_state.diff_target.as_ref() {
        Some(DiffTarget::WorkingTree { path, area, .. }) => {
            let expected_area = if matches!(
                case.trigger,
                Trigger::FileButton | Trigger::LongFileButton | Trigger::RowButton
            ) && !case.index_locked
            {
                match case.area {
                    DiffArea::Unstaged => DiffArea::Staged,
                    DiffArea::Staged => DiffArea::Unstaged,
                }
            } else {
                case.area
            };
            assert_eq!(*area, expected_area, "{label}: the list's area");
            Some(path.display().to_string())
        }
        None => None,
        other => panic!("{label}: expected a working-tree diff, got {other:?}"),
    };
    if matches!(
        case.trigger,
        Trigger::FileButton | Trigger::LongFileButton | Trigger::RowButton
    ) && !case.index_locked
    {
        // Exercise the completion against its real post-SelectDiff snapshot:
        // that request advanced the canvas generation, and must still restore
        // the retained position. A later user request must cancel instead.
        cx.update(|_, app| {
            view.read(app)
                .diff_renderer
                .clone()
                .update(app, |pane, cx| {
                    let repo = pane.active_repo().unwrap();
                    let generation = repo.navigation.canvas_requests;
                    let ops_rev = repo.ops_rev;
                    let repo_lifetime = repo.lifetime();
                    for (requests_since_source, expected) in [(1, -13.0), (2, -13.0)] {
                        pane.stage_file_transfer = Some(
                            crate::view::panes::diff::helpers::state::StageFileTransfer {
                                repo_id: REPO,
                                repo_lifetime,
                                path: case.open.into(),
                                from: case.area,
                                ops_rev,
                                canvas_requests: generation.wrapping_sub(requests_since_source),
                                opened: true,
                                scroll_offsets: [
                                    point(px(0.0), px(-13.0 * requests_since_source as f32)),
                                    point(px(-9.0), px(-27.0 * requests_since_source as f32)),
                                ],
                            },
                        );
                        pane.resolve_stage_file_transfer(cx);
                        assert!(pane.stage_file_transfer.is_none(), "{label}");
                        assert_eq!(
                            [&pane.diff_scroll, &pane.diff_split_right_scroll].map(|scroll| scroll
                                .0
                                .borrow()
                                .base_handle
                                .offset()),
                            [point(px(0.0), px(expected)), point(px(-9.0), px(-27.0))],
                            "{label}"
                        );
                    }
                });
        });
    }

    if matches!(case.trigger, Trigger::LongFileButton) {
        draw_and_drain_test_window(cx);
        let (selector, viewport, offset_before) = cx.update(|_, app| {
            let pane = view.read(app).details_pane.read(app);
            let section = match case.area {
                DiffArea::Unstaged => StatusSection::Staged,
                DiffArea::Staged => StatusSection::CombinedUnstaged,
            };
            let position = pane
                .status_path_display_position(section, std::path::Path::new(case.open))
                .unwrap();
            assert!(position >= 60, "{label}: fixture must require scrolling");
            let scroll = if case.area == DiffArea::Unstaged {
                &pane.staged_scroll
            } else {
                &pane.unstaged_scroll
            };
            assert!(
                scroll.0.borrow().base_handle.offset().y < px(0.0),
                "{label}: destination must scroll"
            );
            let scroll = scroll.0.borrow();
            (
                format!("status_row_{}_{}_{position}", REPO.0, section.id_label()),
                scroll.base_handle.bounds(),
                scroll.base_handle.offset(),
            )
        });
        let row = cx
            .debug_bounds(Box::leak(selector.into_boxed_str()))
            .expect("transferred path is rendered");
        assert!(
            row.top() >= viewport.top() && row.bottom() <= viewport.bottom(),
            "{label}: row must be fully visible"
        );
        // Another reveal of the already visible row must not center or jump it.
        cx.update(|_, app| {
            view.read(app)
                .diff_renderer
                .clone()
                .update(app, |pane, cx| {
                    let repo = pane.active_repo().unwrap();
                    pane.commands.defer(
                        crate::view::panes::diff::services::DiffCommand::RevealStatus {
                            section: if case.area == DiffArea::Unstaged {
                                StatusSection::Staged
                            } else {
                                StatusSection::CombinedUnstaged
                            },
                            path: case.open.into(),
                            after_transfer: Some((
                                REPO,
                                repo.lifetime(),
                                repo.navigation.canvas_requests,
                            )),
                        },
                        cx,
                    );
                });
        });
        draw_and_drain_test_window(cx);
        cx.update(|_, app| {
            let pane = view.read(app).details_pane.read(app);
            let scroll = if case.area == DiffArea::Unstaged {
                &pane.staged_scroll
            } else {
                &pane.unstaged_scroll
            };
            assert_eq!(
                scroll.0.borrow().base_handle.offset(),
                offset_before,
                "{label}: visible row must stay put"
            );
        });
    }

    store.dispatch(Msg::CloseRepo { repo_id: REPO });
    crate::view::test_support::drain_store_worker(&view, cx);
    drop(view);
    scenario.finish().expect("next file scenario teardown");
    shown
}

/// "Discard" in `path`'s row menu, then the dialog's Discard.
fn discard_through_dialog(cx: &mut gpui::VisualTestContext, view: &View, path: &str) {
    cx.update(|window, app| {
        view.read(app).popover_host.clone().update(app, |host, cx| {
            host.context_menu_activate_action(
                ContextMenuAction::DiscardWorktreeChangesSelectionOrPath {
                    repo_id: REPO,
                    area: DiffArea::Unstaged,
                    path: path.into(),
                },
                window,
                cx,
            );
        });
    });
    draw_and_drain_test_window(cx);
    click(cx, "discard_changes_go");
}

/// Until one action begun after `before` finished and the status after it loaded.
fn wait_settled(
    cx: &mut gpui::VisualTestContext,
    view: &View,
    store: &AppStore,
    label: &str,
    before: u64,
) {
    wait_for(cx, view, store, label, |repo| {
        repo.ops_rev >= before + 2
            && repo.local_actions_in_flight == 0
            && !repo
                .loads_in_flight
                .is_in_flight(gitcomet_state::model::LoadKind::WorktreeStatus)
            && !repo
                .loads_in_flight
                .is_in_flight(gitcomet_state::model::LoadKind::StagedStatus)
    });
}

fn case(area: DiffArea, open: &'static str, trigger: Trigger) -> Case {
    Case {
        area,
        open,
        a_contents: ONE_CHANGE,
        trigger,
        enabled: true,
        partial_staged: false,
        index_locked: false,
    }
}

/// S stages, U unstages in a staged diff and discards in an unstaged one;
/// menus and the discard dialog do the same.
#[gpui::test]
fn the_last_change_opens_the_next_file(cx: &mut gpui::TestAppContext) {
    for case in [
        case(DiffArea::Unstaged, "a.txt", Trigger::Key("s")),
        case(DiffArea::Unstaged, "a.txt", Trigger::Key("u")),
        case(DiffArea::Staged, "a.txt", Trigger::Key("u")),
        case(DiffArea::Unstaged, "a.txt", Trigger::StageHunkMenu),
        case(DiffArea::Unstaged, "a.txt", Trigger::DiscardFileDialog),
    ] {
        assert_eq!(shown_after(cx, case).as_deref(), Some("b.txt"), "{case:?}");
    }
}

#[gpui::test]
fn the_last_file_of_a_list_opens_the_one_before(cx: &mut gpui::TestAppContext) {
    let case = case(DiffArea::Unstaged, "c.txt", Trigger::Key("s"));
    assert_eq!(shown_after(cx, case).as_deref(), Some("b.txt"), "{case:?}");
}

#[gpui::test]
fn a_change_that_is_not_the_last_keeps_the_file(cx: &mut gpui::TestAppContext) {
    let case = Case {
        a_contents: TWO_CHANGES,
        ..case(DiffArea::Unstaged, "a.txt", Trigger::Key("s"))
    };
    assert_eq!(shown_after(cx, case).as_deref(), Some("a.txt"), "{case:?}");
}

/// Off, a line or hunk action keeps the file open, and discarding the whole
/// file closes the diff as it always did.
#[gpui::test]
fn the_setting_off_opens_no_other_file(cx: &mut gpui::TestAppContext) {
    for (trigger, shown) in [
        (Trigger::Key("s"), Some("a.txt")),
        (Trigger::DiscardFileDialog, None),
    ] {
        let case = Case {
            enabled: false,
            ..case(DiffArea::Unstaged, "a.txt", trigger)
        };
        assert_eq!(shown_after(cx, case).as_deref(), shown, "{case:?}");
    }
}

/// Discarding another file arms nothing, so staging the open file from its
/// row later closes the diff as it always did instead of opening `b.txt`.
#[gpui::test]
fn only_an_action_on_the_open_file_arms_the_advance(cx: &mut gpui::TestAppContext) {
    let case = case(
        DiffArea::Unstaged,
        "a.txt",
        Trigger::DiscardOtherThenStageFromRow,
    );
    assert_eq!(shown_after(cx, case), None, "{case:?}");
}

#[gpui::test]
fn explicit_file_buttons_follow_the_same_file_to_its_other_lane(cx: &mut gpui::TestAppContext) {
    for area in [DiffArea::Unstaged, DiffArea::Staged] {
        for enabled in [false, true] {
            for partial_staged in [false, true] {
                let case = Case {
                    enabled,
                    partial_staged,
                    ..case(area, "a.txt", Trigger::FileButton)
                };
                assert_eq!(shown_after(cx, case).as_deref(), Some("a.txt"), "{case:?}");
            }
        }
    }
}

#[gpui::test]
fn a_failed_file_button_keeps_the_original_diff(cx: &mut gpui::TestAppContext) {
    for area in [DiffArea::Unstaged, DiffArea::Staged] {
        let case = Case {
            index_locked: true,
            ..case(area, "a.txt", Trigger::FileButton)
        };
        assert_eq!(shown_after(cx, case).as_deref(), Some("a.txt"), "{case:?}");
    }
}

#[gpui::test]
fn selecting_another_file_cancels_the_pending_lane_transfer(cx: &mut gpui::TestAppContext) {
    for area in [DiffArea::Unstaged, DiffArea::Staged] {
        let case = case(area, "a.txt", Trigger::FileButtonThenOpenOther);
        assert_eq!(shown_after(cx, case).as_deref(), Some("b.txt"), "{case:?}");
    }
}

#[gpui::test]
fn closing_the_diff_cancels_the_pending_lane_transfer(cx: &mut gpui::TestAppContext) {
    for area in [DiffArea::Unstaged, DiffArea::Staged] {
        let case = case(area, "a.txt", Trigger::FileButtonThenClose);
        assert_eq!(shown_after(cx, case), None, "{case:?}");
    }
}

#[gpui::test]
fn row_buttons_follow_the_open_file_to_the_other_lane(cx: &mut gpui::TestAppContext) {
    for area in [DiffArea::Unstaged, DiffArea::Staged] {
        assert_eq!(
            shown_after(cx, case(area, "a.txt", Trigger::RowButton)).as_deref(),
            Some("a.txt")
        );
    }
}

#[gpui::test]
fn file_buttons_reveal_the_transferred_path_in_long_destination_lists(
    cx: &mut gpui::TestAppContext,
) {
    for area in [DiffArea::Unstaged, DiffArea::Staged] {
        assert_eq!(
            shown_after(cx, case(area, "c.txt", Trigger::LongFileButton)).as_deref(),
            Some("c.txt")
        );
    }
}
