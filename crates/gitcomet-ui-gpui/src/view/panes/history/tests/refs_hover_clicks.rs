use super::*;

/// The refs hover card answers clicks like the graph's branch chips, as
/// GitKraken's ref list does: a click picks the commit and keeps the card open,
/// a double click on the same branch checks it out, and a right click still
/// opens that ref's menu.
#[gpui::test]
fn refs_hover_click_selects_and_double_click_checks_out(cx: &mut gpui::TestAppContext) {
    cx.update(|app| {
        crate::appearance::pin_compact_for_test(app);
        app.set_global(crate::localization::Language::English);
    });
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let output = gitcomet_core::test_support::git_fixture::git_command()
            .arg("-C")
            .arg(dir.path())
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    let branches = ["same-tip-branch-a", "same-tip-branch-b"];
    git(&["init", "-q", "-b", "main"]);
    git(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "--allow-empty",
        "-qm",
        "fixture",
    ]);
    for name in branches {
        git(&["branch", name]);
    }
    let sha = git(&["rev-parse", "HEAD"]);
    let refs_before = git(&["show-ref", "--heads"]);
    let repository = gitcomet_git_gix::GixBackend.open(dir.path()).unwrap();
    let (view, cx, mut state, store) = refresh::mount(
        cx,
        Arc::new(log_page(vec![commit(&sha, &[], "fixture")], None)),
    );
    let repo = &mut state.repos[0];
    let repo_id = repo.id;
    repo.spec = repository.spec().clone();
    repo.head_branch = Loadable::Ready("main".into());
    repo.head_branch_rev += 1;
    repo.branches = Loadable::Ready(Arc::new(
        std::iter::once("main")
            .chain(branches)
            .map(|name| branch(name, &sha))
            .collect(),
    ));
    repo.branches_rev += 1;
    repo.history_state.selected_commit = None;
    store.replace_snapshot_for_test(Arc::new(state.clone()));
    store.insert_repo_for_test(repo_id, repository);
    ensure_history_cache_for_tests(cx, &view, Arc::new(state));
    cx.simulate_resize(size(px(1500.0), px(900.0)));
    let mode = HistoryBranchNamesMode::SeparateColumn;
    cx.update(|window, app| {
        let history = view.read(app).main_pane.read(app).history_view.clone();
        history.update(app, |history, cx| {
            history.history_branch_names = mode;
            history.history_col_branch = px(60.0);
            window.activate();
            window.focus(&history.history_panel_focus_handle, cx);
            cx.notify();
        });
    });
    crate::view::test_support::redraw(cx);

    // Open the card from the row's refs.
    let badge = HistoryView::painted_overflow_bounds_for_test(repo_id, &sha, mode)
        .expect("the same-tip branches must overflow");
    cx.simulate_mouse_move(badge.center(), None, gpui::Modifiers::default());
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    crate::view::test_support::redraw(cx);
    assert!(cx.debug_bounds("history_refs_hover_panel").is_some());

    let item = |cx: &mut gpui::VisualTestContext, name: &str| {
        let selector: &'static str = format!(
            "history_refs_hover_item_local_branch_{}",
            name.replace('-', "_")
        )
        .leak();
        cx.debug_bounds(selector).expect(selector).center()
    };
    let click_at = |cx: &mut gpui::VisualTestContext, position, click_count| {
        cx.simulate_mouse_move(position, None, gpui::Modifiers::default());
        cx.simulate_event(gpui::MouseDownEvent {
            position,
            button: gpui::MouseButton::Left,
            click_count,
            ..Default::default()
        });
        cx.simulate_event(gpui::MouseUpEvent {
            position,
            button: gpui::MouseButton::Left,
            click_count,
            ..Default::default()
        });
        cx.run_until_parked();
        crate::view::test_support::redraw(cx);
    };
    let assert_still_main = || {
        store
            .barrier_for_test()
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        assert_eq!(git(&["symbolic-ref", "HEAD"]), "refs/heads/main");
        assert_eq!(git(&["show-ref", "--heads"]), refs_before);
    };
    let assert_card_without_menu = |cx: &mut gpui::VisualTestContext| {
        cx.update(|_, app| {
            assert_eq!(
                crate::view::test_support::popover_kind(view.read(app), app),
                None,
                "a left click on the card must not open a menu"
            );
            assert!(crate::view::test_support::history_refs_hover_is_open(
                view.read(app),
                app
            ));
        });
    };

    // A click selects the commit, keeps the card and changes no branch.
    let branch_a = item(cx, branches[0]);
    click_at(cx, branch_a, 1);
    assert_card_without_menu(cx);
    wait_until(cx, "the card click selects its commit", |_| {
        store.snapshot().repos[0].history_state.selected_commit
            == Some(CommitId(sha.as_str().into()))
    });
    // The selection reaching the view must not retract the card it came from.
    // Deliver it the way the app does, through the model's observers.
    cx.update(|_, app| {
        let ui_model = view.read(app).ui_model.clone();
        ui_model.update(app, |model, cx| model.set_state(store.snapshot(), cx));
    });
    cx.run_until_parked();
    crate::view::test_support::redraw(cx);
    assert_eq!(
        cx.update(|_, app| view
            .read(app)
            .main_pane
            .read(app)
            .history_view
            .read(app)
            .active_repo()
            .and_then(|repo| repo.history_state.selected_commit.clone())),
        Some(CommitId(sha.as_str().into())),
        "the view has applied the selection"
    );
    assert_card_without_menu(cx);
    assert_still_main();

    // A refs change still retracts the kept card: its list would be stale.
    let mut refs_changed = (*store.snapshot()).clone();
    refs_changed.repos[0].branches_rev += 1;
    cx.update(|_, app| {
        let ui_model = view.read(app).ui_model.clone();
        ui_model.update(app, |model, cx| model.set_state(Arc::new(refs_changed), cx));
    });
    cx.run_until_parked();
    crate::view::test_support::redraw(cx);
    cx.update(|_, app| {
        assert!(!crate::view::test_support::history_refs_hover_is_open(
            view.read(app),
            app
        ));
    });
    cx.simulate_mouse_move(badge.center(), None, gpui::Modifiers::default());
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    crate::view::test_support::redraw(cx);
    assert!(cx.debug_bounds("history_refs_hover_panel").is_some());

    // A right click still opens the clicked ref's own menu.
    cx.simulate_mouse_move(branch_a, None, gpui::Modifiers::default());
    cx.simulate_mouse_down(
        branch_a,
        gpui::MouseButton::Right,
        gpui::Modifiers::default(),
    );
    cx.simulate_mouse_up(
        branch_a,
        gpui::MouseButton::Right,
        gpui::Modifiers::default(),
    );
    cx.run_until_parked();
    crate::view::test_support::redraw(cx);
    cx.update(|_, app| {
        assert_eq!(
            crate::view::test_support::popover_kind(view.read(app), app),
            Some(PopoverKind::CommitMenu {
                repo_id,
                commit_id: CommitId(sha.as_str().into()),
                refs: CommitMenuRefs::Only(HistoryMenuRef::Branch(BranchMenuTarget::local(
                    branches[0]
                ))),
            })
        );
    });
    cx.update(|_, app| {
        let popover_host = view.read(app).popover_host.clone();
        popover_host.update(app, |host, cx| host.close_popover(cx));
    });
    cx.run_until_parked();
    crate::view::test_support::redraw(cx);
    assert!(cx.debug_bounds("history_refs_hover_panel").is_some());

    // A second click on another branch does not finish the first one's gesture.
    click_at(cx, branch_a, 1);
    let branch_b = item(cx, branches[1]);
    click_at(cx, branch_b, 2);
    assert_card_without_menu(cx);
    assert_still_main();

    // A double click on one branch checks out exactly that branch.
    click_at(cx, branch_b, 1);
    click_at(cx, branch_b, 2);
    wait_until(cx, "the card double click checks out branch b", |_| {
        git(&["symbolic-ref", "HEAD"]) == format!("refs/heads/{}", branches[1])
    });
    assert_eq!(git(&["rev-parse", "HEAD"]), sha);
    assert_eq!(git(&["show-ref", "--heads"]), refs_before);
}
