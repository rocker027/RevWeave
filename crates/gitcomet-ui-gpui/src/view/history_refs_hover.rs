use super::*;
use crate::kit::interaction::{self as controls, ControlInteractionExt as _};
use crate::localization::Language;
use gitcomet_core::domain::Worktree;
use gitcomet_state::msg::CommitSelectMode;
use std::cell::RefCell;
use std::rc::Rc;

/// Rows in the refs hover card share the sidebar tree's row rhythm.
const HISTORY_REFS_HOVER_ROW_HEIGHT_PX: f32 = 24.0;
const HISTORY_REFS_HOVER_ROW_COMFORTABLE_HEIGHT_PX: f32 = 32.0;

const HISTORY_REFS_HOVER_CLOSE_GRACE_MS: u64 = 120;
const HISTORY_REFS_HOVER_OPEN_DELAY_MS: u64 = 160;
const HISTORY_REFS_HOVER_WIDTH_PX: f32 = 240.0;
// The card's padding and border around its rows, for sizing it to its content.
const HISTORY_REFS_HOVER_CHROME_PX: f32 = 12.0;
const HISTORY_REFS_HOVER_POINTER_INSET_PX: f32 = 16.0;
pub(in crate::view) const HISTORY_REFS_HOVER_MENU_INVOKER_PREFIX: &str = "history_refs_hover_menu_";
pub(in crate::view) const HISTORY_REFS_PICKER_MENU_INVOKER_PREFIX: &str = "history_refs_picker_";

#[derive(Clone, Debug)]
struct HistoryRefsHoverState {
    repo_id: RepoId,
    commit_id: CommitId,
    source_bounds: Bounds<Pixels>,
    source_pointer_x: Pixels,
    items: Arc<[HistoryRefListItem]>,
    worktrees: Option<Arc<Vec<Worktree>>>,
}

impl HistoryRefsHoverState {
    fn worktree_for_item(&self, item: &HistoryRefListItem) -> Option<&Worktree> {
        let branch = match &item.kind {
            HistoryRefListItemKind::LocalBranch { name }
            | HistoryRefListItemKind::AttachedHead { branch: name } => name,
            _ => return None,
        };
        super::caches::history_worktree_for_local_ref(
            self.worktrees.as_deref()?,
            &self.commit_id,
            branch,
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct HistoryRefsHoverLayout {
    anchor: Point<Pixels>,
    anchor_corner: Anchor,
    panel_w: Pixels,
    max_panel_h: Pixels,
}

fn same_history_refs_hover_state(lhs: &HistoryRefsHoverState, rhs: &HistoryRefsHoverState) -> bool {
    lhs.repo_id == rhs.repo_id
        && lhs.commit_id == rhs.commit_id
        && lhs.source_bounds == rhs.source_bounds
        && *lhs.items == *rhs.items
        && lhs.worktrees == rhs.worktrees
}

pub(in crate::view) struct HistoryRefsHoverHost {
    theme: AppTheme,
    root_view: WeakEntity<GitCometView>,
    state: Option<HistoryRefsHoverState>,
    pending_show: Option<HistoryRefsHoverState>,
    item_menu_open: bool,
    pinned_item_ix: Option<usize>,
    // The item the last single click landed on; a double click must repeat it.
    first_item_click: Option<(CommitId, usize)>,
    // The commit a click in the card selected. Its own selection arriving in
    // the history state must not retract the card, as GitKraken's list stays.
    card_selection: Option<(RepoId, CommitId)>,
    panel_bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,
    last_mouse_pos: Point<Pixels>,
    show_seq: u64,
    close_seq: u64,
}

impl HistoryRefsHoverHost {
    pub(in crate::view) fn new(theme: AppTheme, root_view: WeakEntity<GitCometView>) -> Self {
        Self {
            theme,
            root_view,
            state: None,
            pending_show: None,
            item_menu_open: false,
            pinned_item_ix: None,
            first_item_click: None,
            card_selection: None,
            panel_bounds: Rc::new(RefCell::new(None)),
            last_mouse_pos: point(px(0.0), px(0.0)),
            show_seq: 0,
            close_seq: 0,
        }
    }

    pub(in crate::view) fn set_theme(&mut self, theme: AppTheme, cx: &mut gpui::Context<Self>) {
        self.theme = theme;
        cx.notify();
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::view) fn show(
        &mut self,
        repo_id: RepoId,
        commit_id: CommitId,
        source_bounds: Bounds<Pixels>,
        items: Arc<[HistoryRefListItem]>,
        worktrees: Option<Arc<Vec<Worktree>>>,
        pointer: Point<Pixels>,
        cx: &mut gpui::Context<Self>,
    ) {
        if items.is_empty() {
            self.close(cx);
            return;
        }

        self.last_mouse_pos = pointer;

        if self.item_menu_open {
            return;
        }

        let next = HistoryRefsHoverState {
            repo_id,
            commit_id,
            source_bounds,
            source_pointer_x: pointer.x,
            items,
            worktrees,
        };

        if self
            .state
            .as_ref()
            .is_some_and(|state| same_history_refs_hover_state(state, &next))
        {
            self.cancel_pending_show();
            return;
        }

        if self.state.is_some() {
            self.cancel_pending_show();
            self.show_now(next, cx);
            return;
        }

        if let Some(pending) = self.pending_show.as_mut()
            && same_history_refs_hover_state(pending, &next)
        {
            pending.source_pointer_x = pointer.x;
            return;
        }

        self.pending_show = Some(next);
        self.show_seq = self.show_seq.wrapping_add(1);
        let seq = self.show_seq;
        cx.spawn(async move |view: WeakEntity<HistoryRefsHoverHost>, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(HISTORY_REFS_HOVER_OPEN_DELAY_MS))
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.show_seq != seq {
                    return;
                }
                let Some(next) = this.pending_show.take() else {
                    return;
                };
                if !next.source_bounds.contains(&this.last_mouse_pos) {
                    return;
                }
                // An overlay may have opened during the open delay (e.g. a
                // right-click context menu); don't pop the hover under it.
                if this
                    .root_view
                    .upgrade()
                    .is_some_and(|root| root.read(cx).background_hovers_blocked(cx))
                {
                    return;
                }
                this.state = Some(next);
                this.item_menu_open = false;
                this.pinned_item_ix = None;
                this.first_item_click = None;
                this.card_selection = None;
                *this.panel_bounds.borrow_mut() = None;
                this.close_seq = this.close_seq.wrapping_add(1);
                cx.notify();
            });
        })
        .detach();
    }

    fn show_now(&mut self, next: HistoryRefsHoverState, cx: &mut gpui::Context<Self>) {
        self.close_seq = self.close_seq.wrapping_add(1);
        let changed = self
            .state
            .as_ref()
            .is_none_or(|state| !same_history_refs_hover_state(state, &next));

        if changed {
            self.item_menu_open = false;
            self.pinned_item_ix = None;
            self.first_item_click = None;
            self.card_selection = None;
            *self.panel_bounds.borrow_mut() = None;
            self.state = Some(next);
            // The host is an uncached overlay of the root view, so notifying it
            // redraws the card; `window.refresh()` would also re-render every
            // cached pane for this frame.
            cx.notify();
        }
    }

    fn cancel_pending_show(&mut self) {
        if self.pending_show.is_none() {
            return;
        }
        self.pending_show = None;
        self.show_seq = self.show_seq.wrapping_add(1);
    }

    fn cancel_pending_show_if_pointer_left(&mut self, position: Point<Pixels>) {
        let pointer_left_pending_source = self
            .pending_show
            .as_ref()
            .is_some_and(|pending| !pending.source_bounds.contains(&position));
        if pointer_left_pending_source {
            self.cancel_pending_show();
        }
    }

    fn keep_open_at(&mut self, position: Point<Pixels>) {
        self.last_mouse_pos = position;
        if self.state.is_some() {
            self.close_seq = self.close_seq.wrapping_add(1);
        }
    }

    pub(in crate::view) fn close(&mut self, cx: &mut gpui::Context<Self>) {
        let had_state = self.state.take().is_some();
        let had_pending = self.pending_show.take().is_some();
        self.item_menu_open = false;
        self.pinned_item_ix = None;
        self.first_item_click = None;
        self.card_selection = None;
        if !had_state && !had_pending {
            return;
        }

        if had_pending {
            self.show_seq = self.show_seq.wrapping_add(1);
        }
        if had_state {
            self.close_seq = self.close_seq.wrapping_add(1);
            *self.panel_bounds.borrow_mut() = None;
            cx.notify();
        }
    }

    pub(in crate::view) fn on_mouse_moved(
        &mut self,
        position: Point<Pixels>,
        cx: &mut gpui::Context<Self>,
    ) {
        self.last_mouse_pos = position;
        self.cancel_pending_show_if_pointer_left(position);
        if self.state.is_none() {
            return;
        }
        if self.item_menu_open {
            self.close_seq = self.close_seq.wrapping_add(1);
            return;
        }
        if self.pointer_inside_open_regions(position) {
            self.close_seq = self.close_seq.wrapping_add(1);
            return;
        }
        self.schedule_close(cx);
    }

    #[cfg(test)]
    pub(in crate::view) fn is_open_for_tests(&self) -> bool {
        self.state.is_some()
    }

    #[cfg(test)]
    pub(in crate::view) fn source_bounds_for_tests(&self) -> Option<Bounds<Pixels>> {
        self.state.as_ref().map(|state| state.source_bounds)
    }

    #[cfg(test)]
    pub(in crate::view) fn pinned_item_ix_for_tests(&self) -> Option<usize> {
        self.pinned_item_ix
    }

    #[cfg(test)]
    pub(in crate::view) fn pinned_item_text_for_tests(&self) -> Option<SharedString> {
        let ix = self.pinned_item_ix?;
        Some(self.state.as_ref()?.items.get(ix)?.text.shared().clone())
    }

    fn pointer_inside_open_regions(&self, position: Point<Pixels>) -> bool {
        self.state
            .as_ref()
            .is_some_and(|state| state.source_bounds.contains(&position))
            || self
                .panel_bounds
                .borrow()
                .as_ref()
                .is_some_and(|bounds| bounds.contains(&position))
    }

    fn schedule_close(&mut self, cx: &mut gpui::Context<Self>) {
        self.close_seq = self.close_seq.wrapping_add(1);
        let seq = self.close_seq;
        cx.spawn(async move |view: WeakEntity<HistoryRefsHoverHost>, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(HISTORY_REFS_HOVER_CLOSE_GRACE_MS))
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.close_seq != seq {
                    return;
                }
                if this.item_menu_open {
                    return;
                }
                if this.pointer_inside_open_regions(this.last_mouse_pos) {
                    return;
                }
                this.state = None;
                this.first_item_click = None;
                this.card_selection = None;
                *this.panel_bounds.borrow_mut() = None;
                this.close_seq = this.close_seq.wrapping_add(1);
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::view) fn is_item_menu_open(&self) -> bool {
        self.item_menu_open
    }

    pub(in crate::view) fn set_item_menu_open(&mut self, open: bool, cx: &mut gpui::Context<Self>) {
        if self.item_menu_open == open {
            return;
        }

        self.item_menu_open = open;
        if open {
            self.first_item_click = None;
            self.card_selection = None;
            self.cancel_pending_show();
        } else {
            self.pinned_item_ix = None;
        }
        self.close_seq = self.close_seq.wrapping_add(1);

        if !open && self.state.is_some() && !self.pointer_inside_open_regions(self.last_mouse_pos) {
            self.schedule_close(cx);
        }
    }

    fn item_popover_kind(
        repo_id: RepoId,
        commit_id: &CommitId,
        item: &HistoryRefListItem,
    ) -> PopoverKind {
        let refs = match &item.kind {
            HistoryRefListItemKind::Tag { name } => {
                CommitMenuRefs::Only(HistoryMenuRef::Tag(name.clone()))
            }
            HistoryRefListItemKind::LocalBranch { name }
            | HistoryRefListItemKind::AttachedHead { branch: name } => {
                CommitMenuRefs::Only(HistoryMenuRef::Branch(BranchMenuTarget::local(name)))
            }
            HistoryRefListItemKind::RemoteBranch { remote, branch, .. } => CommitMenuRefs::Only(
                HistoryMenuRef::Branch(BranchMenuTarget::remote(remote, branch)),
            ),
            HistoryRefListItemKind::DetachedHead => CommitMenuRefs::Hidden,
        };
        PopoverKind::CommitMenu {
            repo_id,
            commit_id: commit_id.clone(),
            refs,
        }
    }

    fn item_debug_selector(item: &HistoryRefListItem) -> String {
        let prefix = match item.kind {
            HistoryRefListItemKind::Tag { .. } => "history_refs_hover_item_tag",
            HistoryRefListItemKind::LocalBranch { .. } => "history_refs_hover_item_local_branch",
            HistoryRefListItemKind::RemoteBranch { .. } => "history_refs_hover_item_remote_branch",
            HistoryRefListItemKind::AttachedHead { .. } => "history_refs_hover_item_attached_head",
            HistoryRefListItemKind::DetachedHead => "history_refs_hover_item_detached_head",
        };
        format!("{}_{}", prefix, slug_for_debug_selector(item.text.as_ref()))
    }

    fn item_icon(item: &HistoryRefListItem) -> &'static str {
        match item.kind {
            HistoryRefListItemKind::Tag { .. } => "icons/tag.svg",
            HistoryRefListItemKind::LocalBranch { .. }
            | HistoryRefListItemKind::AttachedHead { .. } => "icons/computer.svg",
            HistoryRefListItemKind::RemoteBranch { .. } => "icons/cloud.svg",
            HistoryRefListItemKind::DetachedHead => "icons/question.svg",
        }
    }

    fn item_icon_color(theme: AppTheme, kind: &HistoryRefListItemKind) -> gpui::Rgba {
        match kind {
            HistoryRefListItemKind::Tag { .. }
            | HistoryRefListItemKind::LocalBranch { .. }
            | HistoryRefListItemKind::AttachedHead { .. } => theme.colors.accent.foreground,
            HistoryRefListItemKind::RemoteBranch { .. } | HistoryRefListItemKind::DetachedHead => {
                theme.colors.foreground.secondary
            }
        }
    }

    fn open_item_menu(
        &mut self,
        repo_id: RepoId,
        commit_id: CommitId,
        item_ix: usize,
        item: &HistoryRefListItem,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let kind = Self::item_popover_kind(repo_id, &commit_id, item);
        let invoker: SharedString = format!(
            "{}{}_{}_{}",
            HISTORY_REFS_HOVER_MENU_INVOKER_PREFIX,
            repo_id.0,
            commit_id.as_ref(),
            item_ix
        )
        .into();
        self.pinned_item_ix = Some(item_ix);
        self.set_item_menu_open(true, cx);
        let root_view = self.root_view.clone();
        let _ = root_view.update(cx, |root, cx| {
            root.popover_host.update(cx, |host, cx| {
                host.open_popover_at(kind.invoked_by(invoker), position, window, cx)
            });
            cx.notify();
        });
    }

    /// The branch a double click on this item activates; tags and a detached
    /// HEAD have none.
    fn item_branch_target(item: &HistoryRefListItem) -> Option<BranchMenuTarget> {
        match &item.kind {
            HistoryRefListItemKind::LocalBranch { name }
            | HistoryRefListItemKind::AttachedHead { branch: name } => {
                Some(BranchMenuTarget::local(name))
            }
            HistoryRefListItemKind::RemoteBranch { remote, branch, .. } => {
                Some(BranchMenuTarget::remote(remote, branch))
            }
            HistoryRefListItemKind::Tag { .. } | HistoryRefListItemKind::DetachedHead => None,
        }
    }

    /// Left clicks behave like the graph's branch chips: a click selects the
    /// commit and leaves the card open, a double click on the same branch
    /// checks it out. The menu stays on right click.
    #[allow(clippy::too_many_arguments)]
    fn click_item(
        &mut self,
        repo_id: RepoId,
        commit_id: CommitId,
        item_ix: usize,
        item: &HistoryRefListItem,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        // The card's rows take no keyboard focus, so only pointer clicks land here.
        let ClickEvent::Mouse(click) = event else {
            return;
        };
        let first = self.first_item_click.take();
        if click.down.click_count != click.up.click_count {
            return;
        }
        let clicked = (commit_id.clone(), item_ix);
        let modifiers = event.modifiers();
        let Some(history_view) = self.history_view(cx) else {
            return;
        };
        match click.up.click_count {
            1 => {
                if modifiers.number_of_modifiers() == 0 {
                    self.first_item_click = Some(clicked);
                }
                self.card_selection = Some((repo_id, commit_id.clone()));
                let mode = if modifiers.shift {
                    CommitSelectMode::Range
                } else if modifiers.secondary() || modifiers.control || modifiers.platform {
                    CommitSelectMode::Toggle
                } else {
                    CommitSelectMode::Single
                };
                history_view.update(cx, |history, cx| {
                    // The card occludes the panel's own mouse-down focus, so
                    // move focus like a row click does: arrows then step from here.
                    window.focus(&history.history_panel_focus_handle, cx);
                    history.cancel_history_find_navigation();
                    history.select_history_commit(repo_id, commit_id, mode, None);
                    cx.notify();
                });
            }
            2 if first == Some(clicked) && modifiers.number_of_modifiers() == 0 => {
                // A checkout rewrites the refs this card lists; let it close.
                self.card_selection = None;
                let Some(target) = Self::item_branch_target(item) else {
                    return;
                };
                let position = event.position();
                history_view.update(cx, |history, cx| {
                    // The card can outlive a ref update; act only on a branch
                    // that still points at the commit it was listed under.
                    let still_listed = history.active_repo().is_some_and(|repo| {
                        repo.id == repo_id
                            && crate::view::rows::sidebar::branch_commit_id(repo, &target).as_ref()
                                == Some(&commit_id)
                    });
                    if still_listed {
                        history.activate_history_branch(repo_id, &target, position, window, cx);
                    }
                });
            }
            _ => {}
        }
    }

    /// Whether a history state change leaves the card open: only when the card
    /// is still on the commit its own click selected and that is the selection.
    pub(in crate::view) fn kept_by_card_selection(
        &self,
        selected: Option<&(RepoId, CommitId)>,
    ) -> bool {
        let (Some(state), Some(card)) = (self.state.as_ref(), self.card_selection.as_ref()) else {
            return false;
        };
        selected == Some(card) && state.repo_id == card.0 && state.commit_id == card.1
    }

    fn history_view(&self, cx: &App) -> Option<Entity<HistoryView>> {
        let root = self.root_view.upgrade()?;
        Some(root.read(cx).main_pane.read(cx).history_view.clone())
    }
}

fn history_refs_hover_layout(
    source: Bounds<Pixels>,
    source_pointer_x: Pixels,
    window_size: Size<Pixels>,
    preferred_panel_w: Pixels,
    preferred_max_panel_h: Pixels,
    pointer_inset: Pixels,
    gap: Pixels,
    margin: Pixels,
) -> HistoryRefsHoverLayout {
    let horizontal_margin = margin.min(window_size.width * 0.5);
    let min_x = horizontal_margin;
    let max_right = (window_size.width - horizontal_margin).max(min_x);
    let available_w = (max_right - min_x).max(px(0.0));
    let panel_w = preferred_panel_w.min(available_w);
    let max_x = (max_right - panel_w).max(min_x);
    let pointer_inset = pointer_inset.min(panel_w * 0.5).max(px(0.0));
    let mut preferred_x = source.left();
    if source_pointer_x < preferred_x + pointer_inset {
        preferred_x = source_pointer_x - pointer_inset;
    } else if source_pointer_x > preferred_x + panel_w - pointer_inset {
        preferred_x = source_pointer_x - panel_w + pointer_inset;
    }
    let anchor_x = preferred_x.max(min_x).min(max_x);

    let vertical_margin = margin.min(window_size.height * 0.5);
    let min_y = vertical_margin;
    let max_y = (window_size.height - vertical_margin).max(min_y);
    let below_anchor_y = (source.bottom() + gap).max(min_y).min(max_y);
    let above_anchor_y = (source.top() - gap).max(min_y).min(max_y);
    let below_h = preferred_max_panel_h.min((max_y - below_anchor_y).max(px(0.0)));
    let above_h = preferred_max_panel_h.min((above_anchor_y - min_y).max(px(0.0)));

    if above_h > below_h {
        HistoryRefsHoverLayout {
            anchor: point(anchor_x, above_anchor_y),
            anchor_corner: Anchor::BottomLeft,
            panel_w,
            max_panel_h: above_h,
        }
    } else {
        HistoryRefsHoverLayout {
            anchor: point(anchor_x, below_anchor_y),
            anchor_corner: Anchor::TopLeft,
            panel_w,
            max_panel_h: below_h,
        }
    }
}

impl Render for HistoryRefsHoverHost {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let Some(state) = self.state.clone() else {
            return div().into_any_element();
        };

        let theme = self.theme;
        let language = Language::current(cx);
        let ui_scale = ui_scale::UiScale::current(cx);
        let panel_w = ui_scale.px(HISTORY_REFS_HOVER_WIDTH_PX);
        // Size the card to every ref, as GitKraken's list shows them all at
        // once; the layout opens it on the side that fits and only scrolls
        // when the window has no room for the whole list.
        let max_panel_h = ui_scale.row_height(
            HISTORY_REFS_HOVER_ROW_HEIGHT_PX,
            HISTORY_REFS_HOVER_ROW_COMFORTABLE_HEIGHT_PX,
        ) * state.items.len() as f32
            + ui_scale.px(HISTORY_REFS_HOVER_CHROME_PX);
        let pointer_inset = ui_scale.px(HISTORY_REFS_HOVER_POINTER_INSET_PX);
        let gap = px(0.0);
        let margin = ui_scale.px(8.0);
        let window_size = window.viewport_size();
        let source = state.source_bounds;
        let layout = history_refs_hover_layout(
            source,
            state.source_pointer_x,
            window_size,
            panel_w,
            max_panel_h,
            pointer_inset,
            gap,
            margin,
        );
        let panel_bounds_for_prepaint = Rc::clone(&self.panel_bounds);

        let items = state.items.iter().enumerate().map(|(ix, item)| {
            let label = item.text.shared().clone();
            let worktree = state.worktree_for_item(item);
            let tooltip = worktree.map_or_else(
                || label.clone(),
                |worktree| {
                    let worktree_label = language.text("Worktree", "工作樹");
                    format!("{label}\n{worktree_label}\n{}", worktree.path.display()).into()
                },
            );
            // Icon only, like GitKraken's list: the branch name keeps the
            // width, and the row's tooltip names the worktree's path.
            let worktree_badge = worktree.map(|_| {
                div()
                    .debug_selector(|| format!("{}_worktree", Self::item_debug_selector(item)))
                    .flex_none()
                    .flex()
                    .items_center()
                    .child(svg_icon(
                        "icons/git_worktree.svg",
                        theme.colors.accent.foreground,
                        ui_scale.px(12.0),
                    ))
            });
            let frozen = self.item_menu_open;
            let pinned = self.pinned_item_ix == Some(ix);
            let icon = Self::item_icon(item);
            let icon_color = Self::item_icon_color(theme, &item.kind);
            div()
                .id(("history_refs_hover_item", ix))
                .debug_selector(|| Self::item_debug_selector(item))
                .h(ui_scale.row_height(
                    HISTORY_REFS_HOVER_ROW_HEIGHT_PX,
                    HISTORY_REFS_HOVER_ROW_COMFORTABLE_HEIGHT_PX,
                ))
                .w_full()
                .min_w(px(0.0))
                .px_2()
                .flex()
                .items_center()
                .gap_1()
                .text_size(theme.ui_text(12.0))
                .line_height(ui_scale.px(16.0))
                .rounded(px(theme.radii.row))
                .text_color(theme.colors.foreground.primary)
                .control_interaction(
                    controls::InteractionStyle::new(theme).pointer_feedback(!frozen),
                    controls::InteractionState::default()
                        .selected(pinned, theme.colors.interaction.selected_background),
                )
                .child(svg_icon(icon, icon_color, ui_scale.px(12.0)))
                .child(div().flex_1().min_w(px(0.0)).truncate().child(label))
                .children(worktree_badge)
                .gitcomet_tooltip(theme, tooltip)
                // `on_click`/`on_aux_click` rather than raw mouse-up: the panel
                // shows on hover, so it can slide under a button that is
                // already held, and gpui only fires these when the press
                // landed on this row too.
                .on_activate(
                    false,
                    controls::ControlActivation::Composite,
                    cx.listener({
                        let commit_id = state.commit_id.clone();
                        let item_for_left = item.clone();
                        move |this, e: &ClickEvent, window, cx| {
                            if !e.standard_click() {
                                return;
                            }
                            cx.stop_propagation();
                            this.click_item(
                                state.repo_id,
                                commit_id.clone(),
                                ix,
                                &item_for_left,
                                e,
                                window,
                                cx,
                            );
                        }
                    }),
                )
                .on_aux_click(cx.listener({
                    let commit_id = state.commit_id.clone();
                    let item_for_right = item.clone();
                    move |this, e: &ClickEvent, window, cx| {
                        if !e.is_right_click() {
                            return;
                        }
                        cx.stop_propagation();
                        this.open_item_menu(
                            state.repo_id,
                            commit_id.clone(),
                            ix,
                            &item_for_right,
                            e.position(),
                            window,
                            cx,
                        );
                    }
                }))
                .into_any_element()
        });

        let panel = div()
            .id("history_refs_hover_panel")
            .debug_selector(|| "history_refs_hover_panel".to_string())
            .w(layout.panel_w)
            .max_h(layout.max_panel_h)
            .overflow_y_scroll()
            .p_1()
            .bg(theme.colors.surface.raised)
            .border_1()
            .border_color(theme.colors.stroke.default)
            .rounded(px(theme.radii.popover))
            .occlude()
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _window, cx| {
                this.keep_open_at(e.position);
                cx.stop_propagation();
            }))
            .on_any_mouse_down(|_e, _w, cx| cx.stop_propagation())
            .children(items);

        let measured_panel = div()
            .on_children_prepainted(move |children_bounds, _window, _app| {
                if let Some(bounds) = children_bounds.first() {
                    *panel_bounds_for_prepaint.borrow_mut() = Some(*bounds);
                }
            })
            .child(panel);

        div()
            .id("history_refs_hover_layer")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                anchored()
                    .position(layout.anchor)
                    .anchor(layout.anchor_corner)
                    .offset(point(px(0.0), px(0.0)))
                    .child(measured_panel),
            )
            .into_any_element()
    }
}

fn slug_for_debug_selector(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    let mut previous_was_separator = true;

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            slug.push('_');
            previous_was_separator = true;
        }
    }

    while slug.ends_with('_') {
        slug.pop();
    }

    if slug.is_empty() {
        "ref".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs_hover_only_shows_worktrees_for_matching_local_refs() {
        let commit_id = CommitId("tip".into());
        let worktree = Worktree {
            path: std::path::PathBuf::from("/worktrees/topic"),
            bare: false,
            head: Some(commit_id.clone()),
            branch: Some("feature/topic".to_string()),
            detached: false,
        };
        let state = HistoryRefsHoverState {
            repo_id: RepoId(1),
            commit_id,
            source_bounds: Bounds::default(),
            source_pointer_x: px(0.0),
            items: Arc::from([]),
            worktrees: Some(Arc::new(vec![worktree.clone()])),
        };
        let item = |kind| HistoryRefListItem {
            text: HistoryTextVm::new("feature/topic".into()),
            kind,
        };
        let local = item(HistoryRefListItemKind::LocalBranch {
            name: "feature/topic".to_string(),
        });
        let attached_head = item(HistoryRefListItemKind::AttachedHead {
            branch: "feature/topic".to_string(),
        });
        let other_local = item(HistoryRefListItemKind::LocalBranch {
            name: "other/topic".to_string(),
        });
        let remote = item(HistoryRefListItemKind::RemoteBranch {
            name: "origin/feature/topic".to_string(),
            remote: "origin".to_string(),
            branch: "feature/topic".to_string(),
        });

        assert_eq!(state.worktree_for_item(&local), Some(&worktree));
        assert_eq!(state.worktree_for_item(&attached_head), Some(&worktree));
        assert!(state.worktree_for_item(&other_local).is_none());
        assert!(state.worktree_for_item(&remote).is_none());

        let previous_commit = HistoryRefsHoverState {
            commit_id: CommitId("previous".into()),
            ..state
        };
        assert!(previous_commit.worktree_for_item(&local).is_none());
    }

    #[test]
    fn local_branch_computer_icons_match_the_sidebar_accent() {
        let local = HistoryRefListItemKind::LocalBranch {
            name: "main".to_string(),
        };
        let head = HistoryRefListItemKind::AttachedHead {
            branch: "main".to_string(),
        };
        let remote = HistoryRefListItemKind::RemoteBranch {
            name: "origin/main".to_string(),
            remote: "origin".to_string(),
            branch: "main".to_string(),
        };

        for theme in [AppTheme::gitcomet_dark(), AppTheme::gitcomet_light()] {
            assert_eq!(
                HistoryRefsHoverHost::item_icon_color(theme, &local),
                theme.colors.accent.foreground
            );
            assert_eq!(
                HistoryRefsHoverHost::item_icon_color(theme, &head),
                theme.colors.accent.foreground
            );
            assert_eq!(
                HistoryRefsHoverHost::item_icon_color(theme, &remote),
                theme.colors.foreground.secondary
            );
        }
    }

    #[test]
    fn history_refs_hover_layout_clamps_right_and_chooses_above_near_bottom() {
        let layout = history_refs_hover_layout(
            Bounds::new(point(px(280.0), px(150.0)), size(px(20.0), px(16.0))),
            px(290.0),
            size(px(300.0), px(180.0)),
            px(HISTORY_REFS_HOVER_WIDTH_PX),
            px(260.0),
            px(HISTORY_REFS_HOVER_POINTER_INSET_PX),
            px(0.0),
            px(8.0),
        );

        assert!(matches!(layout.anchor_corner, Anchor::BottomLeft));
        assert_eq!(layout.panel_w, px(240.0));
        assert_eq!(layout.anchor.x, px(52.0));
        assert_eq!(layout.anchor.y, px(150.0));
        assert_eq!(layout.max_panel_h, px(142.0));
        assert!(layout.anchor.x + layout.panel_w <= px(292.0));
        assert!(layout.anchor.y - layout.max_panel_h >= px(8.0));
    }

    #[test]
    fn history_refs_hover_layout_shrinks_width_in_narrow_viewport() {
        let layout = history_refs_hover_layout(
            Bounds::new(point(px(140.0), px(40.0)), size(px(16.0), px(16.0))),
            px(148.0),
            size(px(160.0), px(240.0)),
            px(HISTORY_REFS_HOVER_WIDTH_PX),
            px(260.0),
            px(HISTORY_REFS_HOVER_POINTER_INSET_PX),
            px(0.0),
            px(8.0),
        );

        assert_eq!(layout.panel_w, px(144.0));
        assert_eq!(layout.anchor.x, px(8.0));
        assert!(layout.anchor.x + layout.panel_w <= px(152.0));
    }

    #[test]
    fn history_refs_hover_layout_uses_below_when_more_space_is_below() {
        let layout = history_refs_hover_layout(
            Bounds::new(point(px(24.0), px(20.0)), size(px(40.0), px(16.0))),
            px(44.0),
            size(px(320.0), px(220.0)),
            px(HISTORY_REFS_HOVER_WIDTH_PX),
            px(260.0),
            px(HISTORY_REFS_HOVER_POINTER_INSET_PX),
            px(0.0),
            px(8.0),
        );

        assert!(matches!(layout.anchor_corner, Anchor::TopLeft));
        assert_eq!(layout.anchor, point(px(24.0), px(36.0)));
        assert_eq!(layout.max_panel_h, px(176.0));
        assert!(layout.anchor.y + layout.max_panel_h <= px(212.0));
    }

    #[test]
    fn history_refs_hover_layout_opens_below_when_the_content_fits_there() {
        // More room above, but a two-row card fits below the source.
        let source = Bounds::new(point(px(24.0), px(300.0)), size(px(40.0), px(24.0)));
        let content_h = px(24.0 * 2.0 + HISTORY_REFS_HOVER_CHROME_PX);
        let layout = history_refs_hover_layout(
            source,
            px(44.0),
            size(px(320.0), px(600.0)),
            px(HISTORY_REFS_HOVER_WIDTH_PX),
            content_h,
            px(HISTORY_REFS_HOVER_POINTER_INSET_PX),
            px(0.0),
            px(8.0),
        );
        assert!(matches!(layout.anchor_corner, Anchor::TopLeft));
        assert_eq!(layout.max_panel_h, content_h);

        // A list taller than the room below opens above when it fits there.
        let tall_h = px(24.0 * 11.0 + HISTORY_REFS_HOVER_CHROME_PX);
        let layout = history_refs_hover_layout(
            source,
            px(44.0),
            size(px(320.0), px(600.0)),
            px(HISTORY_REFS_HOVER_WIDTH_PX),
            tall_h,
            px(HISTORY_REFS_HOVER_POINTER_INSET_PX),
            px(0.0),
            px(8.0),
        );
        assert!(matches!(layout.anchor_corner, Anchor::BottomLeft));
        assert_eq!(layout.max_panel_h, tall_h);
    }

    #[test]
    fn history_refs_hover_layout_keeps_source_pointer_inside_panel_top_edge() {
        let layout = history_refs_hover_layout(
            Bounds::new(point(px(24.0), px(20.0)), size(px(360.0), px(16.0))),
            px(340.0),
            size(px(520.0), px(220.0)),
            px(HISTORY_REFS_HOVER_WIDTH_PX),
            px(260.0),
            px(HISTORY_REFS_HOVER_POINTER_INSET_PX),
            px(0.0),
            px(8.0),
        );

        assert!(layout.anchor.x < px(340.0));
        assert!(layout.anchor.x + layout.panel_w > px(340.0));
    }
}
