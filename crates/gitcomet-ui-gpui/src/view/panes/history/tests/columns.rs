use super::*;

fn width_for_all_columns_with_slack() -> Pixels {
    px(HISTORY_COL_BRANCH_PX
        + HISTORY_COL_GRAPH_PX
        + HISTORY_COL_AUTHOR_PX
        + HISTORY_COL_DATE_PX
        + HISTORY_COL_SHA_PX
        + HISTORY_COL_MESSAGE_MIN_PX
        + 2.0)
}

#[test]
fn inline_refs_release_width_for_columns_and_graph_resizing() {
    let mut layout = all_columns_visible_drag_layout();
    let available = px(HISTORY_COL_GRAPH_PX
        + HISTORY_COL_AUTHOR_PX
        + HISTORY_COL_DATE_PX
        + HISTORY_COL_MESSAGE_MIN_PX);
    assert_eq!(
        history_visible_columns_for_layout(available, layout, 100),
        (true, false, false)
    );
    layout.branch_w = px(0.0);
    assert_eq!(
        history_visible_columns_for_layout(available, layout, 100),
        (true, true, false)
    );
    let params = history_column_resize_drag_params(HistoryColResizeHandle::Graph, layout, 100);
    assert_eq!(
        params.other_fixed_width,
        layout.author_w + layout.date_w + layout.sha_w
    );
    let widths = history_reset_widths_for_available_width(
        history_columns_available_width(px(396.0)),
        HistoryBranchNamesMode::Inline,
        true,
        (true, true, true),
        100,
    );
    assert_eq!(widths.graph, px(HISTORY_COL_GRAPH_PX));
    assert_eq!(widths.branch, px(HISTORY_COL_BRANCH_PX));
}

#[test]
fn history_columns_available_width_reserves_scrollbar_gutter() {
    let gutter = history_scrollbar_gutter();
    assert_eq!(
        history_columns_available_width(px(200.0)),
        px(200.0) - gutter
    );
    assert_eq!(history_columns_available_width(gutter), px(0.0));
}

#[test]
fn history_column_drag_clamp_respects_static_maximums() {
    let available = history_columns_available_width(px(1436.0));
    let layout = all_columns_visible_drag_layout();
    let next = history_column_drag_clamped_width(
        HistoryColResizeHandle::Branch,
        px(900.0),
        available,
        layout,
        100,
    );
    assert_eq!(next, px(HISTORY_COL_BRANCH_MAX_PX));
}

#[test]
fn history_column_drag_clamp_preserves_message_space() {
    let available = width_for_all_columns_with_slack();
    let layout = all_columns_visible_drag_layout();
    let next = history_column_drag_clamped_width(
        HistoryColResizeHandle::Branch,
        px(500.0),
        available,
        layout,
        100,
    );

    let next_f: f32 = next.into();
    assert!((next_f - (HISTORY_COL_BRANCH_PX + 2.0)).abs() < 1e-3);
}

#[test]
fn history_column_drag_clamp_never_goes_below_minimum() {
    let available = history_columns_available_width(px(1436.0));
    let layout = all_columns_visible_drag_layout();
    let next = history_column_drag_clamped_width(
        HistoryColResizeHandle::Sha,
        px(0.0),
        available,
        layout,
        100,
    );
    assert_eq!(next, px(HISTORY_COL_SHA_MIN_PX));
}

#[test]
fn history_column_widths_recompute_from_design_units_with_ui_scale_percent() {
    let widths = scaled_history_column_widths(
        default_history_column_design_widths(),
        ui_scale::UiScale::from_percent(200),
    );
    assert_eq!(
        widths,
        HistoryColumnWidths {
            branch: px(HISTORY_COL_BRANCH_PX * 2.0),
            graph: px(HISTORY_COL_GRAPH_PX * 2.0),
            author: px(HISTORY_COL_AUTHOR_PX * 2.0),
            date: px(HISTORY_COL_DATE_PX * 2.0),
            sha: px(HISTORY_COL_SHA_PX * 2.0),
        }
    );
}

#[test]
fn graph_drag_ignores_auto_hidden_optional_columns() {
    let available = history_columns_available_width(px(500.0));
    let widths = default_history_column_widths(100);
    let preferred = (true, true, true);

    assert_eq!(
        history_visible_columns_for_width(available, true, preferred, widths, 100),
        (false, false, false)
    );

    let next = history_column_drag_next_width(
        HistoryColResizeHandle::Graph,
        px(90.0),
        available,
        true,
        preferred,
        widths,
        100,
    );

    assert_eq!(next, px(90.0));
}

#[test]
fn reset_widths_clamp_default_graph_in_narrow_windows() {
    let widths = history_reset_widths_for_available_width(
        px(HISTORY_COL_BRANCH_PX + HISTORY_COL_GRAPH_MIN_PX + HISTORY_COL_MESSAGE_MIN_PX),
        HistoryBranchNamesMode::SeparateColumn,
        true,
        (true, true, true),
        100,
    );

    // The graph gives way first; the branch column keeps its default.
    assert_eq!(widths.branch, px(HISTORY_COL_BRANCH_PX));
    assert_eq!(widths.graph, px(HISTORY_COL_GRAPH_MIN_PX));
}

#[test]
fn reset_widths_clamp_branch_after_graph_reaches_minimum() {
    let widths = history_reset_widths_for_available_width(
        history_columns_available_width(px(360.0)),
        HistoryBranchNamesMode::SeparateColumn,
        true,
        (true, true, true),
        100,
    );

    assert_eq!(widths.graph, px(HISTORY_COL_GRAPH_MIN_PX));
    assert_eq!(
        widths.branch,
        history_columns_available_width(px(360.0))
            - px(HISTORY_COL_MESSAGE_MIN_PX + HISTORY_COL_GRAPH_MIN_PX)
    );
}

#[test]
fn history_resize_state_uses_actual_visible_columns_in_narrow_windows() {
    let available = history_columns_available_width(px(500.0));
    let layout = all_columns_visible_drag_layout();
    let state = history_column_resize_state(
        HistoryColResizeHandle::Graph,
        px(0.0),
        available,
        layout,
        100,
    );

    assert_eq!(
        history_resize_state_visible_columns(available, Some(&state)),
        Some((false, false, false))
    );
}

#[test]
fn history_resize_state_preserves_visible_columns_within_drag_bounds() {
    let available = width_for_all_columns_with_slack();
    let layout = all_columns_visible_drag_layout();
    let state = history_column_resize_state(
        HistoryColResizeHandle::Graph,
        px(0.0),
        available,
        layout,
        100,
    );

    assert!(history_resize_state_preserves_visible_columns(
        available,
        layout,
        Some(&state)
    ));
    assert_eq!(
        history_visible_columns_for_layout_with_resize_state(available, layout, Some(&state), 100,),
        (true, true, true)
    );
}

#[test]
fn history_resize_state_visibility_fast_path_falls_back_for_out_of_bounds_layout() {
    let available = width_for_all_columns_with_slack();
    let state = history_column_resize_state(
        HistoryColResizeHandle::Graph,
        px(0.0),
        available,
        all_columns_visible_drag_layout(),
        100,
    );
    let layout = HistoryColumnDragLayout {
        graph_w: px(140.0),
        ..all_columns_visible_drag_layout()
    };

    assert!(!history_resize_state_preserves_visible_columns(
        available,
        layout,
        Some(&state)
    ));
    assert_eq!(
        history_visible_columns_for_layout_with_resize_state(available, layout, Some(&state), 100,),
        history_visible_columns_for_layout(available, layout, 100)
    );
}

#[test]
fn history_resize_state_visible_columns_fast_path_rejects_stale_current_width() {
    let available = width_for_all_columns_with_slack();
    let layout = all_columns_visible_drag_layout();
    let state = history_column_resize_state(
        HistoryColResizeHandle::Date,
        px(0.0),
        available,
        layout,
        100,
    );

    assert_eq!(
        history_resize_state_visible_columns_for_current_width(
            available,
            px(HISTORY_COL_DATE_PX),
            Some(&state),
        ),
        Some((true, true, true))
    );
    assert_eq!(
        history_resize_state_visible_columns_for_current_width(
            available,
            px(HISTORY_COL_DATE_PX + 1.0),
            Some(&state),
        ),
        None
    );
}

/// Narrowing the branch column can let an optional column (here Author) fit
/// and appear. Dragging the branch column back must restore its width, with
/// that column stepping aside again, rather than stopping where the column
/// that its own narrowing let in now begins.
#[test]
fn widening_the_branch_column_pushes_out_columns_its_narrowing_let_in() {
    let mut layout = all_columns_visible_drag_layout();
    // At the default branch width, Author misses by 10px.
    let available = px(HISTORY_COL_BRANCH_PX
        + HISTORY_COL_GRAPH_PX
        + HISTORY_COL_AUTHOR_PX
        + HISTORY_COL_MESSAGE_MIN_PX
        - 10.0);
    assert_eq!(
        history_visible_columns_for_layout(available, layout, 100),
        (false, false, false)
    );
    layout.branch_w = px(HISTORY_COL_BRANCH_PX - 20.0);
    assert_eq!(
        history_visible_columns_for_layout(available, layout, 100),
        (true, false, false),
        "narrowing the branch column lets Author in"
    );

    let mut state = history_column_resize_state(
        HistoryColResizeHandle::Branch,
        px(0.0),
        available,
        layout,
        100,
    );
    let restored =
        history_column_drag_clamped_width_for_state(&mut state, px(20.0), available, 100);
    assert_eq!(restored, px(HISTORY_COL_BRANCH_PX));
    layout.branch_w = restored;
    assert_eq!(
        history_visible_columns_for_layout_with_resize_state(available, layout, Some(&state), 100),
        (false, false, false),
        "Author steps aside again instead of squeezing the message"
    );

    // Within the room Author still has, the drag keeps the columns steady.
    let mut state = history_column_resize_state(
        HistoryColResizeHandle::Branch,
        px(0.0),
        available,
        HistoryColumnDragLayout {
            branch_w: px(HISTORY_COL_BRANCH_PX - 20.0),
            ..layout
        },
        100,
    );
    let nudged = history_column_drag_clamped_width_for_state(&mut state, px(5.0), available, 100);
    assert_eq!(
        history_visible_columns_for_layout_with_resize_state(
            available,
            HistoryColumnDragLayout {
                branch_w: nudged,
                ..layout
            },
            Some(&state),
            100
        ),
        (true, false, false)
    );
}
