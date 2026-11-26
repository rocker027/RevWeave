use super::bar_items::{self, BarItemOwner, BarItems, ExtensionBarItem, RelocatedBarItems};
use super::*;
use crate::kit::interaction::{self as controls, ControlInteractionExt as _};
use crate::localization::Language;
use crate::view::repository_views::{MORE_VIEWS_INVOKER, RoutedArea, ViewTabs};
use gitcomet_extension_api::{BarItemLocation, BuiltinBarItem};
use rustc_hash::FxHasher;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;

const ACTION_BAR_HEIGHT_PX: f32 = components::CONTROL_HEIGHT_PX + 12.0;
const TOOLBAR_TILE_MIN_WIDTH_PX: f32 = 850.0;

// The main window supports widths down to 820 design pixels. At that size the
// branch name and fixed actions cannot all retain full text. Compact mode uses
// icon-only Pull and Push at Comfortable and Spacious densities. Condensed
// mode keeps action labels while using tighter spacing.
// Tooltips keep omitted labels available in compact mode.
const COMPACT_ACTION_BAR_MAX_WIDTH_PX: f32 = 1120.0;
const CONDENSED_ACTION_BAR_MAX_WIDTH_PX: f32 = 1400.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum ActionBarDensity {
    Compact,
    Condensed,
    Wide,
}

/// How much wider the bar's content is per density step (Comfortable = 1,
/// Spacious = 1.6); the breakpoints move out by the same factor.
const ACTION_BAR_DENSITY_WIDTH_GROWTH: f32 = 0.1;

/// The window width the breakpoints compare against: larger UI text and a
/// roomier density both make the content wider, so they shrink the budget.
fn action_bar_width_budget(window_width: Pixels, metrics: crate::appearance::Appearance) -> Pixels {
    let font_factor = (metrics.ui_font_size_px as f32 / 14.0).max(1.0);
    let density_factor = 1.0 + ACTION_BAR_DENSITY_WIDTH_GROWTH * metrics.density_step();
    window_width / (font_factor * density_factor)
}

/// The widest window (100% zoom, default text size) still in the compact and
/// condensed tiers at `metrics`' density.
#[cfg(test)]
pub(in crate::view) fn action_bar_breakpoints(metrics: crate::appearance::Appearance) -> [f32; 2] {
    let density_factor = 1.0 + ACTION_BAR_DENSITY_WIDTH_GROWTH * metrics.density_step();
    [
        COMPACT_ACTION_BAR_MAX_WIDTH_PX * density_factor,
        CONDENSED_ACTION_BAR_MAX_WIDTH_PX * density_factor,
    ]
}

pub(in super::super) fn action_bar_density(
    window_width: Pixels,
    ui_scale_percent: u32,
    metrics: crate::appearance::Appearance,
) -> ActionBarDensity {
    let window_width = action_bar_width_budget(window_width, metrics);
    if window_width
        <= crate::ui_scale::design_px_from_percent(
            COMPACT_ACTION_BAR_MAX_WIDTH_PX,
            ui_scale_percent,
        )
    {
        ActionBarDensity::Compact
    } else if window_width
        <= crate::ui_scale::design_px_from_percent(
            CONDENSED_ACTION_BAR_MAX_WIDTH_PX,
            ui_scale_percent,
        )
    {
        ActionBarDensity::Condensed
    } else {
        ActionBarDensity::Wide
    }
}

/// The narrowest tier at a density above Compact: Pull and Push drop their
/// labels (their tooltips name them) and badges truncate sooner.
fn tight_action_bar(density: ActionBarDensity, ui_density: crate::appearance::UiDensity) -> bool {
    density == ActionBarDensity::Compact && ui_density != crate::appearance::UiDensity::Compact
}

fn tight_badge_label_max_chars(ui_density: crate::appearance::UiDensity) -> usize {
    match ui_density {
        crate::appearance::UiDensity::Compact => COMPACT_BADGE_LABEL_MAX_CHARS,
        crate::appearance::UiDensity::Comfortable => 7,
        crate::appearance::UiDensity::Spacious => 5,
    }
}

/// Branch and Stash name themselves unless the bar is compact, or
/// condensed beside the repository's view tabs, which need the room more;
/// their tooltips name them then.
fn secondary_action_label(
    density: ActionBarDensity,
    beside_view_tabs: bool,
    label: &'static str,
) -> &'static str {
    match density {
        ActionBarDensity::Compact => "",
        ActionBarDensity::Condensed if beside_view_tabs => "",
        _ => label,
    }
}

/// What the toolbar Pop button would restore: the newest stash
/// (`stash@{0}`), or why there is nothing it can pop. The list only shapes
/// the tooltip: it loads with the sidebar, and `stash@{0}` is the newest
/// stash whether or not the list has loaded, so only a loaded empty list
/// disables the button.
#[derive(Clone, Debug, Eq, PartialEq)]
enum StashPopTarget {
    Ready { index: usize, tooltip: String },
    Unavailable(&'static str),
}

fn stash_pop_target(repo: Option<&RepoState>, language: Language) -> StashPopTarget {
    let empty = StashPopTarget::Unavailable(language.text("No stashes to pop", "沒有可取出的貯藏"));
    let Some(repo) = repo else {
        return empty;
    };
    if repo.is_bare() {
        return StashPopTarget::Unavailable(language.text(
            gitcomet_core::domain::WORKING_TREE_REQUIRED_MESSAGE,
            "這項操作需要工作樹。",
        ));
    }
    let Loadable::Ready(stashes) = &repo.stashes else {
        return StashPopTarget::Ready {
            index: 0,
            tooltip: language
                .text("Pop the newest stash", "取出最新一筆貯藏")
                .to_string(),
        };
    };
    match stashes.iter().min_by_key(|entry| entry.index) {
        Some(entry) => {
            let reference = format!("stash@{{{}}}", entry.index);
            let pop = language.text("Pop", "取出");
            let tooltip = if entry.message.is_empty() {
                format!("{pop} {reference}")
            } else {
                format!("{pop} {reference}: {}", entry.message)
            };
            StashPopTarget::Ready {
                index: entry.index,
                tooltip,
            }
        }
        None => empty,
    }
}

pub(in super::super) fn action_bar_height<C>(cx: &mut C) -> Pixels
where
    C: std::borrow::BorrowMut<gpui::App>,
{
    let scale = crate::ui_scale::UiScale::current(cx);
    scale
        .row_height(ACTION_BAR_HEIGHT_PX, 44.0)
        .max(components::toolbar_tile_height(scale) + scale.px(6.0))
}

/// Longest badge label offered to layout. Badges can shrink further when the
/// viewport cannot fit the preferred labels alongside its fixed actions. The
/// full value stays available in each badge's tooltip.
const BADGE_LABEL_MAX_CHARS: usize = 28;
const CONDENSED_BADGE_LABEL_MAX_CHARS: usize = 16;
const COMPACT_BADGE_LABEL_MAX_CHARS: usize = 10;

/// Label and control ids for a paused rebase, apply, cherry-pick, or revert.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SequencerBanner {
    label: &'static str,
    abort_id: &'static str,
    continue_id: &'static str,
    continue_tooltip: &'static str,
}

fn sequencer_banner(
    state: gitcomet_core::services::SequencerState,
    language: Language,
) -> Option<SequencerBanner> {
    use gitcomet_core::services::SequencerState;
    let (label, abort_id, continue_id, continue_tooltip) = match state {
        SequencerState::None => return None,
        SequencerState::RebaseOrApply => (
            language.text("APPLY/REBASE", "套用／變基中"),
            "abort_rebase_or_apply",
            "continue_rebase_or_apply",
            language.text("Continue the in-progress rebase or apply", "繼續變基或套用"),
        ),
        SequencerState::CherryPick => (
            language.text("CHERRY-PICKING", "揀選提交中"),
            "abort_cherry_pick",
            "continue_cherry_pick",
            language.text("Continue the in-progress cherry-pick", "繼續揀選提交"),
        ),
        SequencerState::Revert => (
            language.text("REVERTING", "正在還原提交"),
            "abort_revert",
            "continue_revert",
            language.text("Continue the in-progress revert", "繼續還原提交"),
        ),
    };
    Some(SequencerBanner {
        label,
        abort_id,
        continue_id,
        continue_tooltip,
    })
}

fn truncate_badge_label_to(label: &str, max_chars: usize) -> SharedString {
    let mut chars = label.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{head}…").into()
    } else {
        head.into()
    }
}

#[cfg(test)]
fn truncate_badge_label(label: &str) -> SharedString {
    truncate_badge_label_to(label, BADGE_LABEL_MAX_CHARS)
}

fn file_browsing_badge(repo: &RepoState) -> Option<(SharedString, SharedString)> {
    if !repo.file_browser.active {
        return None;
    }
    let (label, location) = match repo.browsing_commit() {
        Some(commit_id) => {
            let sha = commit_id.as_ref();
            (
                sha.get(0..8).unwrap_or(sha).to_string(),
                format!("commit {sha}"),
            )
        }
        None if repo.is_bare() => ("HEAD".to_string(), "HEAD (no committed files)".to_string()),
        None => ("Working tree".to_string(), "the working tree".to_string()),
    };
    Some((
        label.into(),
        format!("Browsing {location}. Click for history or Exit file browsing.").into(),
    ))
}

fn head_branch_tracking_upstream_name(
    head_branch: &Loadable<String>,
    branches: &Loadable<Arc<Vec<Branch>>>,
) -> Option<String> {
    head_branch_upstream(head_branch, branches)
        .map(|upstream| format!("{}/{}", upstream.remote, upstream.branch))
}

fn pull_tooltip_text(
    pull_count: usize,
    tracking_branch_name: Option<&str>,
    language: Language,
) -> SharedString {
    let summary = match language {
        Language::English => format!("Pull {pull_count} behind"),
        Language::TraditionalChinese => format!("拉取 · 落後 {pull_count} 筆提交"),
    };
    match tracking_branch_name {
        Some(name) => format!("{summary}\n{name}").into(),
        None => summary.into(),
    }
}

fn push_tooltip_text(
    push_count: usize,
    tracking_branch_name: Option<&str>,
    language: Language,
) -> SharedString {
    let summary = match language {
        Language::English => format!("Push {push_count} ahead"),
        Language::TraditionalChinese => format!("推送 · 領先 {push_count} 筆提交"),
    };
    match tracking_branch_name {
        Some(name) => format!("{summary}\n{name}").into(),
        None => summary.into(),
    }
}

pub(in super::super) struct ActionBarView {
    pub(super) relocated_items: RelocatedBarItems,
    extension_items: Vec<ExtensionBarItem>,
    active_view: gitcomet_extension_api::ViewTarget,
    /// Set while an extension's repository view is selected.
    extension_navigation: Option<(
        gitcomet_extension_api::RepositoryViewContext,
        Option<gitcomet_extension_api::ViewNavigation>,
    )>,
    /// That view's action-bar context.
    extension_slot: Option<gpui::AnyView>,
    /// The repository's views as tabs, while an extension registers any.
    view_tabs: Option<ViewTabs>,
    /// An extension's canvas overlay covers the active repository's view:
    /// Back is enabled to uncover it, and Forward waits.
    canvas_covered: bool,
    store: Arc<AppStore>,
    state: Arc<AppState>,
    theme: AppTheme,
    _ui_model_subscription: gpui::Subscription,
    root_view: WeakEntity<GitCometView>,
    notify_fingerprint: u64,
    active_context_menu_invoker: Option<SharedString>,
    open_terminal_repo_ids: FxHashSet<RepoId>,
    terminal_button_target: TerminalButtonTarget,
}

impl ActionBarView {
    fn notify_fingerprint(state: &AppState) -> u64 {
        let mut hasher = FxHasher::default();
        state.active_repo.hash(&mut hasher);

        if let Some(repo_id) = state.active_repo
            && let Some(repo) = state.repos.iter().find(|r| r.id == repo_id)
        {
            repo.open_rev.hash(&mut hasher);
            repo.repository_mode.hash(&mut hasher);
            std::mem::discriminant(&repo.bare_mode_conversion).hash(&mut hasher);
            repo.head_branch_rev.hash(&mut hasher);
            repo.branches_rev.hash(&mut hasher);
            repo.remotes_rev.hash(&mut hasher);
            repo.remote_branches_rev.hash(&mut hasher);
            repo.annex_takes_over_pull_push(&state.large_file_settings)
                .hash(&mut hasher);
            repo.upstream_divergence_rev.hash(&mut hasher);
            repo.merge_message_rev.hash(&mut hasher);
            repo.ops_rev.hash(&mut hasher);
            // Undo / Redo follow the operation journal.
            repo.git_undo.summary().hash(&mut hasher);
            // Stash controls follow the selected worktree, list and local operations.
            repo.history_state.worktree_selection.hash(&mut hasher);
            repo.stashes_rev.hash(&mut hasher);
            (repo.local_actions_in_flight > 0).hash(&mut hasher);
            repo.status_cache_rev().hash(&mut hasher);
            // The historical-browse badge keys off the file browser source. Not
            // `file_browser_rev`: that moves on every sidebar search keystroke.
            repo.file_browser.active.hash(&mut hasher);
            repo.file_browser.source.hash(&mut hasher);
            // Global back/forward buttons enable/disable with nav stack position.
            repo.navigation.main_history.cursor.hash(&mut hasher);
            repo.navigation.main_history.entries.len().hash(&mut hasher);
        }

        state.worktree_finish_runs.len().hash(&mut hasher);
        for run in &state.worktree_finish_runs {
            run.id.hash(&mut hasher);
            run.origin_repo_id.hash(&mut hasher);
            std::mem::discriminant(&run.phase).hash(&mut hasher);
            run.sources.len().hash(&mut hasher);
            for src in &run.sources {
                std::mem::discriminant(&src.phase).hash(&mut hasher);
                std::mem::discriminant(&src.cleanup).hash(&mut hasher);
            }
        }

        hasher.finish()
    }

    pub(in super::super) fn new(
        store: Arc<AppStore>,
        ui_model: Entity<AppUiModel>,
        theme: AppTheme,
        root_view: WeakEntity<GitCometView>,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let state = Arc::clone(&ui_model.read(cx).state);
        let notify_fingerprint = Self::notify_fingerprint(&state);
        let subscription = cx.observe(&ui_model, |this, model, cx| {
            let next = Arc::clone(&model.read(cx).state);
            let next_fingerprint = Self::notify_fingerprint(&next);

            this.state = next;
            if next_fingerprint != this.notify_fingerprint {
                this.notify_fingerprint = next_fingerprint;
                cx.notify();
            }
        });

        Self {
            relocated_items: Vec::new(),
            extension_items: Vec::new(),
            active_view: gitcomet_extension_api::ViewTarget::History,
            extension_navigation: None,
            extension_slot: None,
            view_tabs: None,
            canvas_covered: false,
            store,
            state,
            theme,
            _ui_model_subscription: subscription,
            root_view,
            notify_fingerprint,
            active_context_menu_invoker: None,
            open_terminal_repo_ids: FxHashSet::default(),
            terminal_button_target: TerminalButtonTarget::default(),
        }
    }

    pub(in super::super) fn set_theme(&mut self, theme: AppTheme, cx: &mut gpui::Context<Self>) {
        self.theme = theme;
        cx.notify();
    }

    pub(in crate::view) fn set_extension_items(
        &mut self,
        items: Vec<ExtensionBarItem>,
        cx: &mut gpui::Context<Self>,
    ) {
        self.extension_items = items;
        cx.notify();
    }

    pub(in crate::view) fn set_active_view(
        &mut self,
        view: gitcomet_extension_api::ViewTarget,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.active_view != view {
            self.active_view = view;
            cx.notify();
        }
    }

    pub(in crate::view) fn set_open_terminal_repo_ids(
        &mut self,
        next: FxHashSet<RepoId>,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.open_terminal_repo_ids != next {
            self.open_terminal_repo_ids = next;
            cx.notify();
        }
    }

    pub(in crate::view) fn set_terminal_button_target(
        &mut self,
        target: TerminalButtonTarget,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.terminal_button_target != target {
            self.terminal_button_target = target;
            cx.notify();
        }
    }

    pub(in crate::view) fn set_extension_navigation(
        &mut self,
        navigation: Option<(
            gitcomet_extension_api::RepositoryViewContext,
            Option<gitcomet_extension_api::ViewNavigation>,
        )>,
        slot: Option<gpui::AnyView>,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.extension_navigation.is_none() && navigation.is_none() {
            return;
        }
        self.extension_navigation = navigation;
        self.extension_slot = slot;
        cx.notify();
    }

    pub(in crate::view) fn set_view_tabs(
        &mut self,
        tabs: Option<ViewTabs>,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.view_tabs == tabs {
            return;
        }
        self.view_tabs = tabs;
        cx.notify();
    }

    pub(in crate::view) fn set_canvas_covered(
        &mut self,
        covered: bool,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.canvas_covered == covered {
            return;
        }
        self.canvas_covered = covered;
        cx.notify();
    }

    /// Shows History (`None`) or view `index`. Deferred: the root updates
    /// this bar as the selection changes.
    fn select_view(&self, index: Option<usize>, window: &mut Window, cx: &mut gpui::Context<Self>) {
        let root = self.root_view.clone();
        window.defer(cx, move |window, cx| {
            let _ = root.update(cx, |root, cx| {
                root.select_routed_view(RoutedArea::Main, index, window, cx)
            });
        });
    }

    /// The repository's views as tabs: History and each registered view,
    /// then one tab for those listed under More, which names the selected
    /// one. Labels give way to icons where the bar is compact.
    fn view_tabs(
        &self,
        tabs: &ViewTabs,
        labelled: bool,
        open: bool,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        let ui_scale = crate::ui_scale::UiScale::current(cx);
        let mut group = div()
            .id("repository_view_tabs")
            .debug_selector(|| "repository_view_tabs".to_string())
            .flex()
            .flex_none()
            .items_center()
            .h_full();
        let history = (
            None,
            SharedString::from("History"),
            Some(SharedString::from("icons/history.svg")),
        );
        let entries = std::iter::once(history).chain(
            tabs.views
                .iter()
                .filter(|view| !view.under_more)
                .map(|view| (Some(view.index), view.title.clone(), view.icon.clone())),
        );
        for (index, title, icon) in entries {
            let id = index.map_or_else(
                || "repository_view_history".to_string(),
                |index| format!("repository_view_{index}"),
            );
            let mut tab = components::BarTab::new(id).selected(tabs.selected == index);
            if let Some(icon) = icon {
                tab = tab.icon(icon);
            }
            if labelled {
                tab = tab.label(title.clone());
            }
            let tab = tab.render(theme, ui_scale).on_activate(
                false,
                controls::ControlActivation::ManagedFocus,
                cx.listener(move |this, _, window, cx| this.select_view(index, window, cx)),
            );
            // An icon alone is named in its tooltip.
            group = group.child(if labelled {
                tab
            } else {
                tab.gitcomet_tooltip(theme, title)
            });
        }
        let mut more = tabs.views.iter().filter(|view| view.under_more).peekable();
        if more.peek().is_none() {
            return group;
        }
        let shown = more.find(|view| Some(view.index) == tabs.selected);
        let mut tab = components::BarTab::new(MORE_VIEWS_INVOKER)
            .selected(shown.is_some())
            .open(open)
            .end_icon("icons/chevron_down.svg");
        match shown {
            Some(view) => {
                if let Some(icon) = view.icon.clone() {
                    tab = tab.icon(icon);
                }
                if labelled {
                    tab = tab.label(view.title.clone());
                }
            }
            None if labelled => tab = tab.label("More"),
            None => tab = tab.icon("icons/more_vertical.svg"),
        }
        // The menu opens below the tab, so keep where it was drawn.
        let drawn: Rc<RefCell<Option<Bounds<Pixels>>>> = Rc::default();
        let painted = Rc::clone(&drawn);
        let tab = tab
            .render(theme, ui_scale)
            .on_activate(
                false,
                controls::ControlActivation::ManagedFocus,
                cx.listener(move |this, e: &ClickEvent, window, cx| {
                    let anchor = (*drawn.borrow())
                        .unwrap_or_else(|| Bounds::new(e.position(), gpui::size(px(0.0), px(0.0))));
                    let root = this.root_view.clone();
                    window.defer(cx, move |window, cx| {
                        let _ = root
                            .update(cx, |root, cx| root.open_more_views_menu(anchor, window, cx));
                    });
                }),
            )
            .gitcomet_tooltip(theme, "More views".into());
        group.child(
            div()
                .flex()
                .h_full()
                .on_children_prepainted(move |children, _window, _cx| {
                    if let Some(bounds) = children.first() {
                        *painted.borrow_mut() = Some(*bounds);
                    }
                })
                .child(tab),
        )
    }

    pub(in super::super) fn set_active_context_menu_invoker(
        &mut self,
        next: Option<SharedString>,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.active_context_menu_invoker == next {
            return;
        }
        self.active_context_menu_invoker = next;
        cx.notify();
    }

    fn active_repo_id(&self) -> Option<RepoId> {
        self.state.active_repo
    }

    fn active_repo(&self) -> Option<&RepoState> {
        let repo_id = self.active_repo_id()?;
        self.state.repos.iter().find(|r| r.id == repo_id)
    }

    fn selected_stash_worktree(&self) -> Option<&std::path::Path> {
        if self.active_view != gitcomet_extension_api::ViewTarget::History {
            return None;
        }
        let repo = self.active_repo()?;
        repo.history_state
            .worktree_selection
            .as_deref()
            .filter(|path| *path != repo.spec.workdir.as_path())
    }

    fn open_popover_at(
        &mut self,
        kind: impl Into<PopoverRequest>,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let kind: PopoverRequest = kind.into();
        let _ = self.root_view.update(cx, |root, cx| {
            root.open_popover_at(kind, anchor, window, cx);
        });
    }

    fn open_popover_for_bounds(
        &mut self,
        kind: impl Into<PopoverRequest>,
        anchor_bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let kind: PopoverRequest = kind.into();
        let _ = self.root_view.update(cx, |root, cx| {
            root.open_popover_for_bounds(kind, anchor_bounds, window, cx);
        });
    }

    fn open_popover_centered(
        &mut self,
        kind: impl Into<PopoverRequest>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let kind: PopoverRequest = kind.into();
        let _ = self.root_view.update(cx, |root, cx| {
            root.open_popover_centered(kind, window, cx);
        });
    }
}

impl BarItemOwner for ActionBarView {
    fn bar_items(
        &mut self,
        action_bar: bool,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> BarItems {
        let mut items = BarItems::new(cx);
        let theme = self.theme;
        let language = Language::current(cx);
        let worktree_required = language.text(
            gitcomet_core::domain::WORKING_TREE_REQUIRED_MESSAGE,
            "這項操作需要工作樹。",
        );
        let ui_scale_percent = crate::ui_scale::current(cx).percent;
        let scaled_px = crate::ui_scale::scaler(ui_scale_percent);
        let scale =
            crate::ui_scale::UiScale::from_percent(ui_scale_percent).with_appearance(theme.metrics);
        let show_toolbar_tiles = action_bar
            && action_bar_width_budget(window.viewport_size().width, theme.metrics)
                >= scale.px(TOOLBAR_TILE_MIN_WIDTH_PX);
        let tile_height = show_toolbar_tiles.then(|| components::toolbar_tile_height(scale));
        let git_button = |id: &'static str, caption: &'static str, fallback: &'static str| {
            components::Button::new(
                id,
                if show_toolbar_tiles {
                    caption
                } else {
                    fallback
                },
            )
            .toolbar_tile(show_toolbar_tiles)
        };
        let density = action_bar_density(
            window.viewport_size().width,
            ui_scale_percent,
            theme.metrics,
        );
        let dense_spacing = density != ActionBarDensity::Wide;
        // Comfortable and Spacious widen every control, so the narrowest
        // tier, which just fits at Compact, has to shed more.
        let tight = tight_action_bar(density, theme.metrics.density);
        let worktree_label_max_chars = match density {
            ActionBarDensity::Compact if tight => {
                tight_badge_label_max_chars(theme.metrics.density)
            }
            ActionBarDensity::Compact => COMPACT_BADGE_LABEL_MAX_CHARS,
            ActionBarDensity::Condensed => CONDENSED_BADGE_LABEL_MAX_CHARS,
            ActionBarDensity::Wide => BADGE_LABEL_MAX_CHARS,
        };
        let beside_view_tabs = self.view_tabs.is_some()
            && items
                .location(BuiltinBarItem::RepositoryViews)
                .is_action_bar();
        let action_label = |label: &'static str| {
            if action_bar {
                secondary_action_label(density, beside_view_tabs, label)
            } else {
                ""
            }
        };
        // Two independent things called density: the responsive breakpoint above
        // picks the base gap from the viewport, then the user's density setting
        // ramps it.
        let gap = |dense: f32, wide: f32, comfortable: f32| {
            if tight {
                // Gaps stop growing where the controls barely fit.
                return scaled_px(dense);
            }
            scaled_px(
                theme
                    .metrics
                    .ramp(if dense_spacing { dense } else { wide }, comfortable),
            )
        };
        let action_group_gap = gap(4.0, 8.0, 6.0);
        let tracking_action_gap = gap(2.0, 4.0, 6.0);
        // Let the picker labels take the available width, down to room for
        // an icon and ellipsis in a narrow window.
        let badge_min_width = scaled_px(theme.metrics.row_height(22.0, 32.0) + 28.0);
        let merge_abort_label = if dense_spacing {
            language.text("Abort", "中止")
        } else {
            language.text("Abort merge", "中止合併")
        };
        let icon_primary = theme.colors.accent.foreground;
        let icon_muted = with_alpha(
            theme.colors.accent.foreground,
            if theme.is_dark { 0.72 } else { 0.82 },
        );
        let icon = |path: &'static str, color: gpui::Rgba| svg_icon(path, color, scaled_px(14.0));
        let spinner = |id: (&'static str, u64), color: gpui::Rgba| {
            components::ButtonSlot::spinner(id, color, scaled_px(14.0))
        };
        let count_badge = |count: usize, color: gpui::Rgba| {
            components::ButtonSlot::custom(move |disabled| {
                div()
                    .text_size(theme.ui_text(12.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.disabled_color_if(disabled, color))
                    .child(count.to_string())
            })
        };

        // Workspace, branch and historical badges all light up while their own
        // picker is open, so they need the active invoker before any of them.
        let menu_selected_bg = components::control_open_background(theme);
        let active_invoker = self.active_context_menu_invoker.clone();

        // Keep the exit control visible throughout file browsing, including
        // while its selection is on the working-tree row.
        let historical_badge = self
            .active_repo()
            .and_then(|repo| {
                file_browsing_badge(repo).map(|(label, tooltip)| (repo.id, label, tooltip))
            })
            .map(|(repo_id, label, tooltip)| {
                let purple = crate::theme::historical_outline(theme.is_dark);
                let invoker: SharedString = "historical_browse_badge".into();
                let is_active = active_invoker
                    .as_ref()
                    .is_some_and(|id| id.as_ref() == invoker.as_ref());
                components::Button::new("historical_browse_badge", label)
                    .start_slot(icon("icons/history.svg", purple))
                    .style(components::ButtonStyle::Subtle)
                    .text_color(purple)
                    .bg(with_alpha(purple, 0.12))
                    .hover_bg(with_alpha(purple, if theme.is_dark { 0.22 } else { 0.18 }))
                    .open(is_active)
                    .selected_bg(with_alpha(purple, if theme.is_dark { 0.30 } else { 0.24 }))
                    .on_click_with_bounds(theme, cx, move |this, _e, bounds, window, cx| {
                        this.open_popover_for_bounds(
                            (PopoverKind::BrowseHistoryMenu { repo_id })
                                .invoked_by(invoker.clone()),
                            bounds,
                            window,
                            cx,
                        );
                    })
                    .debug_selector(|| "historical_browse_badge".to_string())
                    .gitcomet_tooltip(theme, tooltip)
            });

        let is_merging = self.active_repo().is_some_and(merge_in_progress);
        let sequencer_banner = self
            .active_repo()
            .map(active_sequencer_state)
            .and_then(|state| sequencer_banner(state, language));
        let rebase_has_unstaged_conflicts =
            self.active_repo().is_some_and(|r| r.has_unstaged_conflicts);
        // A revert shows REVERT_HEAD while its commit step still runs; the
        // reducer also refuses Continue/Abort until it finishes. A merge tool
        // or submodule clone does not count.
        let sequencer_step_busy = self
            .active_repo()
            .is_some_and(|r| r.sequencer_actions_in_flight > 0);
        let sequencer_busy_tooltip = language.text(
            "Wait for the running Git operation to finish",
            "請等待目前的 Git 操作完成",
        );

        let (pull_count, push_count) = self
            .active_repo()
            .and_then(|r| match &r.upstream_divergence {
                Loadable::Ready(Some(d)) => Some((d.behind, d.ahead)),
                _ => None,
            })
            .unwrap_or((0, 0));
        let (pull_loading, push_loading) = self
            .active_repo()
            .map(|r| (r.pull_in_flight > 0, r.push_in_flight > 0))
            .unwrap_or((false, false));
        let tracking_branch_name = self
            .active_repo()
            .and_then(|repo| head_branch_tracking_upstream_name(&repo.head_branch, &repo.branches));
        let active_repo_key = self.active_repo_id().map(|id| id.0).unwrap_or(0);
        let is_bare = self.active_repo().is_some_and(RepoState::is_bare);

        // Global back/forward navigation, mirroring the mouse side-buttons and
        // Alt+Left / Alt+Right (Option on macOS). Sits at the very start of the action bar.
        let (nav_can_back, nav_can_forward) = self
            .active_repo()
            .map(|repo| {
                (
                    repo.navigation.main_history.can_back(),
                    repo.navigation.main_history.can_forward(),
                )
            })
            .unwrap_or((false, false));
        let (nav_can_back, nav_can_forward) = self.extension_navigation.as_ref().map_or(
            (nav_can_back, nav_can_forward),
            |(context, navigation)| {
                navigation.as_ref().map_or((false, false), |navigation| {
                    (
                        (navigation.can_back)(context, cx),
                        (navigation.can_forward)(context, cx),
                    )
                })
            },
        );
        let (nav_can_back, nav_can_forward) = if self.canvas_covered {
            (true, false)
        } else {
            (nav_can_back, nav_can_forward)
        };
        let nav_back = components::Button::new("global_nav_back", "")
            .start_slot(icon(
                "icons/arrow_left.svg",
                if nav_can_back {
                    theme.colors.foreground.primary
                } else {
                    theme.colors.foreground.secondary
                },
            ))
            .style(components::ButtonStyle::Transparent)
            .disabled(!nav_can_back)
            .on_click(theme, cx, |this, _e, _w, cx| {
                let _ = this
                    .root_view
                    .update(cx, |root, cx| root.dispatch_global_nav(false, cx));
            })
            .gitcomet_tooltip(
                theme,
                format!(
                    "Navigate Back ({})",
                    crate::kit::components::alt_shortcut("Left")
                )
                .into(),
            );
        let nav_forward = components::Button::new("global_nav_forward", "")
            .start_slot(icon(
                "icons/arrow_right.svg",
                if nav_can_forward {
                    theme.colors.foreground.primary
                } else {
                    theme.colors.foreground.secondary
                },
            ))
            .style(components::ButtonStyle::Transparent)
            .disabled(!nav_can_forward)
            .on_click(theme, cx, |this, _e, _w, cx| {
                let _ = this
                    .root_view
                    .update(cx, |root, cx| root.dispatch_global_nav(true, cx));
            })
            .gitcomet_tooltip(
                theme,
                format!(
                    "Navigate Forward ({})",
                    crate::kit::components::alt_shortcut("Right")
                )
                .into(),
            );
        // Undo / Redo of Git operations, as in GitKraken's toolbar.
        let git_undo_repo = self.active_repo_id();
        let git_undo_button = |id: &'static str,
                               direction: gitcomet_state::model::GitUndoDirection,
                               cx: &mut gpui::Context<Self>| {
            let (icon_path, keys) = match direction {
                gitcomet_state::model::GitUndoDirection::Undo => ("icons/undo.svg", "Z"),
                gitcomet_state::model::GitUndoDirection::Redo => ("icons/redo.svg", "Shift+Z"),
            };
            let repo = self.active_repo();
            let finish_busy = repo.is_some_and(|repo| {
                self.state.worktree_finish_blocks_workdir_mutation(repo.id)
                    || repo.git_undo.next(direction).is_some_and(|entry| {
                        entry.changes.iter().any(|change| {
                            self.state
                                .worktree_finish_protects_branch(repo.id, &change.reference)
                        })
                    })
            });
            let unavailable = if finish_busy {
                Some(language.text(
                    "Worktree is protected by an active Worktree Finish run",
                    "此工作樹正由進行中的工作樹收尾任務保護",
                ))
            } else {
                repo.map_or(
                    Some(language.text("Open a repository first", "請先開啟儲存庫")),
                    |repo| crate::view::git_undo_unavailable(repo, direction),
                )
            };
            let label = repo
                .and_then(|repo| repo.git_undo.next(direction))
                .map(|entry| entry.label.clone());
            let verb = match direction {
                gitcomet_state::model::GitUndoDirection::Undo => language.text("Undo", "復原"),
                gitcomet_state::model::GitUndoDirection::Redo => language.text("Redo", "重做"),
            };
            let tooltip: SharedString = match (unavailable, label) {
                (Some(reason), _) => reason.into(),
                (None, Some(label)) => format!(
                    "{verb} {label} ({})",
                    crate::kit::components::secondary_shortcut(keys)
                )
                .into(),
                (None, None) => verb.into(),
            };
            div().debug_selector(move || id.to_string()).child(
                git_button(id, verb, "")
                    .start_slot(icon(
                        icon_path,
                        if unavailable.is_none() {
                            theme.colors.foreground.primary
                        } else {
                            theme.colors.foreground.secondary
                        },
                    ))
                    .style(components::ButtonStyle::Transparent)
                    .disabled(unavailable.is_some())
                    .on_click(theme, cx, move |this, e, _w, cx| {
                        if e.click_count() > 1 {
                            return;
                        }
                        if let Some(repo_id) = git_undo_repo {
                            this.store.dispatch(match direction {
                                gitcomet_state::model::GitUndoDirection::Undo => {
                                    Msg::UndoGitOperation { repo_id }
                                }
                                gitcomet_state::model::GitUndoDirection::Redo => {
                                    Msg::RedoGitOperation { repo_id }
                                }
                            });
                            cx.notify();
                        }
                    })
                    .gitcomet_tooltip(theme, tooltip),
            )
        };
        let git_undo = git_undo_button(
            "git_undo",
            gitcomet_state::model::GitUndoDirection::Undo,
            cx,
        );
        let git_redo = git_undo_button(
            "git_redo",
            gitcomet_state::model::GitUndoDirection::Redo,
            cx,
        );
        let global_nav = div()
            .id("global_nav")
            .debug_selector(|| "global_nav".to_string())
            .flex()
            .items_center()
            .gap(scaled_px(2.0))
            .flex_none()
            .child(nav_back)
            .child(nav_forward)
            .child(git_undo)
            .child(git_redo);

        // Workspace (worktree) and branch badges show the current state at a
        // glance, with each opening a filterable picker.
        let worktree_badge = self.active_repo().map(|repo| {
            let repo_id = repo.id;
            let label = truncate_badge_label_to(
                &super::repo_tabs_bar::workdir_label(repo, &self.state.repos),
                worktree_label_max_chars,
            );
            let workdir = repo.spec.workdir.display().to_string();
            let repository = crate::view::path_display::repo_path_name(
                super::repo_tabs_bar::primary_workdir(repo),
            );
            let worktree = crate::view::path_display::repo_path_name(&repo.spec.workdir);
            let identity = match repo.head_branch.ready() {
                Some(branch) => format!("{repository} / {worktree} · {branch}"),
                None => format!("{repository} / {worktree}"),
            };
            let invoker: SharedString = "worktree_badge".into();
            let is_active = active_invoker
                .as_ref()
                .is_some_and(|id| id.as_ref() == invoker.as_ref());
            components::Button::new("worktree_badge", label.clone())
                .truncate_label()
                .start_slot(icon("icons/git_worktree.svg", icon_primary))
                .style(components::ButtonStyle::Subtle)
                .open(is_active)
                .selected_bg(menu_selected_bg)
                .on_click_with_bounds(theme, cx, move |this, _e, bounds, window, cx| {
                    this.open_popover_for_bounds(
                        (PopoverKind::worktree(repo_id, WorktreePopoverKind::BadgePicker))
                            .invoked_by(invoker.clone()),
                        bounds,
                        window,
                        cx,
                    );
                })
                .min_w(badge_min_width)
                .debug_selector(|| "worktree_badge".to_string())
                .gitcomet_tooltip(
                    theme,
                    format!(
                        "{}\n{identity}\n{workdir}",
                        language.text("Switch worktree", "切換工作樹")
                    )
                    .into(),
                )
        });

        let branch_badge = self.active_repo().and_then(|repo| {
            let Loadable::Ready(head) = &repo.head_branch else {
                return None;
            };
            // Detached HEAD surfaces as the literal "HEAD"; label it as such
            // rather than pretending it is a branch.
            let detached = head == "HEAD";
            // An annex adjusted branch reads as its base branch plus the mode:
            // commits made here propagate to the base via git-annex.
            let adjusted = gitcomet_core::annex::adjusted_branch(head);
            let label: SharedString = if detached {
                language.text("detached", "分離狀態").into()
            } else if let Some((base, mode)) = adjusted {
                format!("{base} · adjusted ({mode})").into()
            } else {
                head.as_str().into()
            };
            let invoker: SharedString = "branch_badge".into();
            let is_active = active_invoker
                .as_ref()
                .is_some_and(|id| id.as_ref() == invoker.as_ref());
            let tooltip: SharedString = if repo.is_bare() {
                format!("HEAD: {head}\n{}", worktree_required).into()
            } else if detached {
                language.text("Detached HEAD — click to check out a branch", "HEAD 處於分離狀態。點選以切換分支。").into()
            } else if let Some((base, _)) = adjusted {
                format!("On git-annex adjusted branch {head}; commits propagate to {base} through git-annex sync, pull and push").into()
            } else {
                match language {
                    Language::English => format!("On branch {head} — click to switch"),
                    Language::TraditionalChinese => format!("目前分支為 {head}。點選以切換。"),
                }.into()
            };
            Some(
                components::Button::new("branch_badge", label)
                    .truncate_label()
                    .start_slot(icon("icons/git_branch.svg", icon_primary))
                    .style(components::ButtonStyle::Subtle)
                    .open(is_active)
                    .selected_bg(menu_selected_bg)
                    .disabled(repo.is_bare())
                    .on_click_with_bounds(theme, cx, move |this, _e, bounds, window, cx| {
                        this.open_popover_for_bounds(
                            (PopoverKind::BranchPicker {
                                purpose: BranchPickerPurpose::Checkout,
                            })
                            .invoked_by(invoker.clone()),
                            bounds,
                            window,
                            cx,
                        );
                    })
                    .min_w(badge_min_width)
                    .debug_selector(|| "branch_badge".to_string())
                    .gitcomet_tooltip(theme, tooltip),
            )
        });

        let pull_color = if pull_count > 0 {
            theme.colors.status.warning.foreground
        } else {
            icon_muted
        };
        let pull_caption = language.text("Pull", "拉取");
        let mut pull_main = git_button(
            "pull_main",
            pull_caption,
            if tight { "" } else { pull_caption },
        )
        .busy(pull_loading)
        .start_slot(if pull_loading {
            spinner(("pull_spinner", active_repo_key), pull_color)
        } else {
            icon("icons/arrow_down_to_line.svg", pull_color).into()
        })
        .style(components::ButtonStyle::Subtle);
        if pull_count > 0 {
            pull_main = pull_main.end_slot(count_badge(pull_count, pull_color));
        }
        let pull_picker_invoker: SharedString = "pull_picker".into();
        let pull_picker_active = self
            .active_context_menu_invoker
            .as_ref()
            .is_some_and(|id| id.as_ref() == pull_picker_invoker.as_ref());
        let pull_tracking_branch_name = tracking_branch_name.clone();
        let finish_owns_active_repo = self
            .active_repo()
            .is_some_and(|repo| self.state.worktree_finish_blocks_workdir_mutation(repo.id));
        let pull_unavailable = self
            .active_repo()
            .and_then(|repo| pull_unavailable_reason(repo, &self.state.large_file_settings));
        let pull_available =
            self.active_repo().is_some() && pull_unavailable.is_none() && !finish_owns_active_repo;
        let pull_menu_icon_color = if pull_picker_active {
            theme.colors.accent.foreground
        } else {
            icon_muted
        };
        let pull_menu = git_button("pull_menu", "", "")
            .start_slot(icon("icons/chevron_down.svg", pull_menu_icon_color))
            .style(components::ButtonStyle::Subtle)
            .open(pull_picker_active)
            .selected_bg(menu_selected_bg);

        let pull = div()
            .id("pull")
            .flex_none()
            .debug_selector(|| "pull".to_string())
            .child(
                components::SplitButton::action_menu(
                    pull_main.disabled(!pull_available),
                    pull_menu,
                    theme,
                    cx,
                    |this, _e, _w, _cx| {
                        let Some(repo) = this.active_repo() else {
                            return;
                        };
                        let repo_id = repo.id;
                        if !this.state.worktree_finish_blocks_workdir_mutation(repo_id)
                            && pull_unavailable_reason(repo, &this.state.large_file_settings)
                                .is_none()
                        {
                            this.store.dispatch(Msg::Pull {
                                repo_id,
                                mode: PullMode::Default,
                            });
                        }
                    },
                    move |this, _e, bounds, window, cx| {
                        this.open_popover_for_bounds(
                            PopoverKind::PullPicker.invoked_by(pull_picker_invoker.clone()),
                            bounds,
                            window,
                            cx,
                        );
                    },
                )
                .style(components::SplitButtonStyle::Borderless)
                .render(theme, ui_scale_percent)
                .when_some(tile_height, |split, height| split.h(height)),
            )
            .gitcomet_tooltip(
                theme,
                match pull_unavailable {
                    Some(reason) => localized_action_reason(reason, language).into(),
                    None => pull_tooltip_text(
                        pull_count,
                        pull_tracking_branch_name.as_deref(),
                        language,
                    ),
                },
            );

        let push_color = if push_count > 0 {
            theme.colors.status.success.foreground
        } else {
            icon_muted
        };
        let push_caption = language.text("Push", "推送");
        let mut push_main = git_button(
            "push_main",
            push_caption,
            if tight { "" } else { push_caption },
        )
        .busy(push_loading)
        .start_slot(if push_loading {
            spinner(("push_spinner", active_repo_key), push_color)
        } else {
            icon("icons/arrow_up_to_line.svg", push_color).into()
        })
        .style(components::ButtonStyle::Subtle);
        if push_count > 0 {
            push_main = push_main.end_slot(count_badge(push_count, push_color));
        }
        let push_picker_invoker: SharedString = "push_picker".into();
        let push_picker_active = self
            .active_context_menu_invoker
            .as_ref()
            .is_some_and(|id| id.as_ref() == push_picker_invoker.as_ref());
        let push_tracking_branch_name = tracking_branch_name.clone();
        let push_unavailable = self.active_repo().and_then(|repo| {
            push_request(repo, &self.state.large_file_settings).unavailable_reason()
        });
        let push_available = self.active_repo().is_some() && push_unavailable.is_none();
        let push_menu_icon_color = if push_picker_active {
            theme.colors.accent.foreground
        } else {
            icon_muted
        };
        let push_menu = git_button("push_menu", "", "")
            .start_slot(icon("icons/chevron_down.svg", push_menu_icon_color))
            .style(components::ButtonStyle::Subtle)
            .open(push_picker_active)
            .selected_bg(menu_selected_bg);

        let push = div()
            .id("push")
            .flex_none()
            .debug_selector(|| "push".to_string())
            .child(
                components::SplitButton::action_menu(
                    push_main.disabled(!push_available),
                    push_menu,
                    theme,
                    cx,
                    |this, e, window, cx| {
                        let Some(repo) = this.active_repo() else {
                            return;
                        };
                        let repo_id = repo.id;
                        match push_request(repo, &this.state.large_file_settings) {
                            PushRequest::Push => this.store.dispatch(Msg::Push { repo_id }),
                            PushRequest::SetUpstream { remote } => this.open_popover_at(
                                PopoverKind::PushSetUpstreamPrompt {
                                    repo_id,
                                    remote,
                                    mode: PushUpstreamMode::CurrentBranch,
                                },
                                e.position(),
                                window,
                                cx,
                            ),
                            PushRequest::NoRemotes | PushRequest::NotReady => {}
                        }
                    },
                    move |this, _e, bounds, window, cx| {
                        this.open_popover_for_bounds(
                            PopoverKind::PushPicker.invoked_by(push_picker_invoker.clone()),
                            bounds,
                            window,
                            cx,
                        );
                    },
                )
                .style(components::SplitButtonStyle::Borderless)
                .render(theme, ui_scale_percent)
                .when_some(tile_height, |split, height| split.h(height)),
            )
            .gitcomet_tooltip(
                theme,
                match push_unavailable {
                    Some(reason) => localized_action_reason(reason, language).into(),
                    None => push_tooltip_text(
                        push_count,
                        push_tracking_branch_name.as_deref(),
                        language,
                    ),
                },
            );

        let stash = if let Some(path) = self.selected_stash_worktree() {
            let path = path.to_path_buf();
            let label = language.text("Open worktree", "開啟工作樹");
            let tooltip = format!("{label}\n{}", path.display());
            div().child(
                components::Button::new("stash_open_worktree", if action_bar { label } else { "" })
                    .start_slot(icon("icons/git_worktree.svg", icon_primary))
                    .style(components::ButtonStyle::Subtle)
                    .on_click(theme, cx, move |this, _, window, cx| {
                        if this.selected_stash_worktree() == Some(path.as_path()) {
                            crate::app::open_repository_from_view(
                                cx,
                                window.window_handle().window_id(),
                                path.clone(),
                            );
                        }
                    })
                    .debug_selector(|| "stash_open_worktree".to_string())
                    .gitcomet_tooltip(theme, tooltip.into()),
            )
        } else {
            let repo = self.active_repo();
            let workdir = repo.map(|repo| repo.spec.workdir.display().to_string());
            let target_tooltip = |label: &str| -> SharedString {
                match &workdir {
                    Some(workdir) => format!("{label}\n{workdir}").into(),
                    None => label.to_owned().into(),
                }
            };
            let busy = repo.is_some_and(|repo| {
                repo.local_actions_in_flight > 0
                    || self.state.worktree_finish_blocks_workdir_mutation(repo.id)
            });
            let busy_text = language.text(
                "Wait for the running Git operation to finish",
                "請等待目前的 Git 操作完成",
            );
            let has_changes = repo.is_some_and(|repo| {
                repo.worktree_status_entries()
                    .is_some_and(|entries| !entries.is_empty())
                    || repo
                        .staged_status_entries()
                        .is_some_and(|entries| !entries.is_empty())
            });
            let can_stash = has_changes && !is_bare && !busy;
            let stash_help = if is_bare {
                worktree_required
            } else if busy {
                busy_text
            } else if can_stash {
                language.text("Create stash", "新增貯藏")
            } else {
                language.text("No changes to stash", "沒有可貯藏的變更")
            };
            let stash_invoker: SharedString = "stash_btn".into();
            let stash_caption = language.text("Stash", "貯藏");
            let stash_button = git_button("stash", stash_caption, action_label(stash_caption))
                .start_slot(icon(crate::view::icons::STASH_ICON_PATH, icon_primary))
                .style(components::ButtonStyle::Subtle)
                .open(active_invoker.as_ref() == Some(&stash_invoker))
                .selected_bg(menu_selected_bg)
                .disabled(!can_stash)
                .on_click_with_bounds(theme, cx, move |this, _e, bounds, window, cx| {
                    if this.selected_stash_worktree().is_none() {
                        this.open_popover_for_bounds(
                            PopoverKind::StashPrompt.invoked_by(stash_invoker.clone()),
                            bounds,
                            window,
                            cx,
                        );
                    }
                })
                .gitcomet_tooltip(theme, target_tooltip(stash_help));

            let (pop_index, pop_help) = match stash_pop_target(repo, language) {
                StashPopTarget::Ready { index, tooltip } => (Some(index), tooltip),
                StashPopTarget::Unavailable(reason) => (None, reason.to_owned()),
            };
            let repo_id = self.active_repo_id();
            let can_pop = pop_index.is_some() && repo_id.is_some() && !busy;
            // A linked worktree can add a stash after this tab last read the list.
            let can_choose_stash = !is_bare
                && !busy
                && repo.is_some_and(|repo| matches!(repo.open, Loadable::Ready(())));
            let pop_tooltip = target_tooltip(if busy { busy_text } else { &pop_help });
            let menu_tooltip = if can_choose_stash {
                target_tooltip(
                    language.text("Choose a stash to apply or pop", "選擇要套用或取出的貯藏"),
                )
            } else {
                pop_tooltip.clone()
            };
            let restore_invoker: SharedString = "stash_restore_menu".into();
            let pop_caption = language.text("Pop", "取出");
            let pop_button = git_button("stash_pop", pop_caption, action_label(pop_caption))
                .start_slot(icon("icons/stash_pop.svg", icon_primary))
                .style(components::ButtonStyle::Subtle)
                .disabled(!can_pop);
            let menu_button = git_button("stash_restore_menu", "", "")
                .start_slot(icon("icons/chevron_down.svg", icon_muted))
                .style(components::ButtonStyle::Subtle)
                .open(active_invoker.as_ref() == Some(&restore_invoker))
                .selected_bg(menu_selected_bg)
                .disabled(!can_choose_stash);
            let restore_buttons = components::SplitButton::from_buttons(
                pop_button,
                menu_button,
                cx,
                |button, cx| {
                    button
                        .on_click(theme, cx, move |this, e, _window, cx| {
                            // A double click must not pop twice before the list reloads.
                            if e.click_count() > 1 || this.selected_stash_worktree().is_some() {
                                return;
                            }
                            if let (Some(repo_id), Some(index)) = (repo_id, pop_index)
                                && this.active_repo().is_some_and(|repo| {
                                    repo.id == repo_id && repo.local_actions_in_flight == 0
                                })
                            {
                                this.store.dispatch(Msg::PopStash { repo_id, index });
                                this.store.dispatch(Msg::LoadStashes { repo_id });
                                cx.notify();
                            }
                        })
                        .debug_selector(|| "stash_pop".to_string())
                        .gitcomet_tooltip(theme, pop_tooltip)
                },
                |button, cx| {
                    button
                        .on_click_with_bounds(theme, cx, move |this, _e, bounds, window, cx| {
                            if let Some(repo_id) = repo_id
                                && this.active_repo_id() == Some(repo_id)
                                && this.selected_stash_worktree().is_none()
                            {
                                this.open_popover_for_bounds(
                                    (PopoverKind::StashRestoreMenu { repo_id })
                                        .invoked_by(restore_invoker.clone()),
                                    bounds,
                                    window,
                                    cx,
                                );
                            }
                        })
                        .debug_selector(|| "stash_restore_menu".to_string())
                        .gitcomet_tooltip(theme, menu_tooltip)
                },
            )
            .style(components::SplitButtonStyle::Borderless)
            .render(theme, ui_scale_percent)
            .when_some(tile_height, |split, height| split.h(height));

            let finish_caption = language.text("Finish", "收尾");
            let finish_button = git_button(
                "finish_worktrees",
                finish_caption,
                action_label(finish_caption),
            )
            .start_slot(icon("icons/swap.svg", icon_primary))
            .style(components::ButtonStyle::Subtle)
            .disabled(repo_id.is_none())
            .on_click(theme, cx, move |this, _e, window, cx| {
                if let Some(repo_id) = this.active_repo_id() {
                    this.open_popover_centered(
                        PopoverKind::worktree(
                            repo_id,
                            crate::view::WorktreePopoverKind::FinishSetup {
                                preselected: Vec::new(),
                                target: None,
                                mode: None,
                            },
                        ),
                        window,
                        cx,
                    );
                }
            })
            .debug_selector(|| "finish_worktrees".to_string())
            .gitcomet_tooltip(
                theme,
                language.text("Finish worktrees…", "收尾工作樹…").into(),
            );

            div()
                .flex()
                .items_center()
                .gap(tracking_action_gap)
                .child(stash_button)
                .child(restore_buttons)
                .child(finish_button)
        }
        .flex_none()
        .debug_selector(|| "stash".to_string());

        let create_branch_invoker: SharedString = "create_branch_btn".into();
        let create_branch_active = self
            .active_context_menu_invoker
            .as_ref()
            .is_some_and(|id| id.as_ref() == create_branch_invoker.as_ref());
        let branch_caption = language.text("Branch", "分支");
        let create_branch = div().debug_selector(|| "create_branch".to_string()).child(
            git_button(
                "create_branch",
                branch_caption,
                action_label(branch_caption),
            )
            .start_slot(icon("icons/git_branch.svg", icon_primary))
            .style(components::ButtonStyle::Subtle)
            .open(create_branch_active)
            .selected_bg(menu_selected_bg)
            .on_click_with_bounds(theme, cx, move |this, _e, bounds, window, cx| {
                if let Some(repo_id) = this.state.active_repo {
                    let target = this
                        .active_repo()
                        .and_then(|repo| {
                            if let Loadable::Ready(head) = &repo.head_branch {
                                Some(head.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| "HEAD".to_string());
                    this.open_popover_for_bounds(
                        (PopoverKind::CreateBranchFromRefPrompt {
                            repo_id,
                            target,
                            source_selectable: true,
                            name_prefix: String::new(),
                            checkout_on_open: true,
                        })
                        .invoked_by(create_branch_invoker.clone()),
                        bounds,
                        window,
                        cx,
                    );
                }
            })
            .gitcomet_tooltip(theme, language.text("Create branch", "建立分支").into()),
        );

        let tracking_actions = div()
            .id("tracking_actions")
            .debug_selector(|| "tracking_actions".to_string())
            .flex()
            .min_w(px(0.0))
            .items_center()
            .gap(tracking_action_gap)
            .children(branch_badge)
            .child(pull)
            .child(push);

        items.push(BuiltinBarItem::Navigation, global_nav);
        if (self.extension_navigation.is_none() || is_bare)
            && let Some(badge) = worktree_badge
        {
            items.push(BuiltinBarItem::Worktree, badge);
        }
        if let Some(repo) = self.active_repo().filter(|repo| repo.is_bare()) {
            let repo_id = repo.id;
            let busy = repo.bare_mode_conversion.is_busy();
            let invoker: SharedString = "repository_mode_badge".into();
            let is_active = active_invoker.as_deref() == Some(invoker.as_ref());
            items.push(
                BuiltinBarItem::RepositoryMode,
                components::Button::new("repository_mode_badge", "Git Bare")
                    .style(components::ButtonStyle::Subtle)
                    .open(is_active)
                    .selected_bg(menu_selected_bg)
                    .busy(busy)
                    .on_click_with_bounds(theme, cx, move |this, _, bounds, window, cx| {
                        let kind = if this
                            .active_repo()
                            .is_some_and(|repo| repo.bare_mode_conversion.is_busy())
                        {
                            RepoPopoverKind::DisableBareModeConfirm
                        } else {
                            RepoPopoverKind::RepositoryModeMenu
                        };
                        this.open_popover_for_bounds(
                            PopoverKind::Repo { repo_id, kind }.invoked_by(invoker.clone()),
                            bounds,
                            window,
                            cx,
                        );
                    })
                    .debug_selector(|| "repository_mode_badge".to_owned())
                    .gitcomet_tooltip(
                        theme,
                        if busy {
                            "Disabling bare mode; waiting for verified reopening.".into()
                        } else {
                            super::popover::bare_mode::BARE_MODE_TOOLTIP.into()
                        },
                    ),
            );
        }
        // The selected view owns its context and visibility, in either bar.
        if self.extension_navigation.is_some() {
            if let Some(slot) = self.extension_slot.clone() {
                items.push(
                    BuiltinBarItem::ViewContext,
                    div()
                        .debug_selector(|| "extension_action_bar".to_string())
                        .flex()
                        .items_center()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(slot),
                );
            }
        } else {
            items.push(BuiltinBarItem::Tracking, tracking_actions);
        }
        items.push(BuiltinBarItem::CreateBranch, create_branch.flex_none());
        items.push(BuiltinBarItem::Stash, stash);
        if self.extension_navigation.is_none() {
            if let Some(badge) = historical_badge {
                items.push(BuiltinBarItem::HistoricalBrowse, badge);
            }
            let active_finish_run = self
                .active_repo_id()
                .and_then(|repo_id| self.state.worktree_finish_run_for_repo(repo_id))
                .cloned();

            let has_active_finish_run = active_finish_run.is_some();
            let operation_controls = div()
                .flex()
                .items_center()
                .gap(action_group_gap)
                .when_some(active_finish_run, |d, run| {
                    let run_id = run.id;
                    let total = run.sources.len();
                    let done_count = run.sources.iter().filter(|s| s.phase.is_terminal()).count();
                    let banner_label = match run.phase {
                        gitcomet_state::model::RunPhase::Previewing => language
                            .text("Finish · Re-previewing", "收尾 · 重新預覽中")
                            .to_string(),
                        gitcomet_state::model::RunPhase::AwaitingConfirm { .. } => language
                            .text("Finish · Confirm preview", "收尾 · 待重新確認")
                            .to_string(),
                        gitcomet_state::model::RunPhase::Running { current } => match language {
                            Language::English => format!("Finish {}/{total}", current + 1),
                            Language::TraditionalChinese => format!("收尾 {}/{total}", current + 1),
                        },
                        gitcomet_state::model::RunPhase::Paused { current } => match language {
                            Language::English => {
                                format!("Finish {}/{total} · Paused", current + 1)
                            }
                            Language::TraditionalChinese => {
                                format!("收尾 {}/{total} · 已暫停", current + 1)
                            }
                        },
                        gitcomet_state::model::RunPhase::CleanupCheck
                        | gitcomet_state::model::RunPhase::CleaningUp => language
                            .text("Finish · Cleaning up", "收尾 · 清理中")
                            .to_string(),
                        gitcomet_state::model::RunPhase::Finished => match language {
                            Language::English => format!("Finish {done_count}/{total} · Done"),
                            Language::TraditionalChinese => {
                                format!("收尾 {done_count}/{total} · 已完成")
                            }
                        },
                        gitcomet_state::model::RunPhase::Aborted => language
                            .text("Finish · Aborted", "收尾 · 已中止")
                            .to_string(),
                    };
                    let repo_id = run.origin_repo_id;
                    d.child(
                        components::Button::new("worktree_finish_banner", banner_label)
                            .style(components::ButtonStyle::Outlined)
                            .on_click(theme, cx, move |this, e: &ClickEvent, window, cx| {
                                this.open_popover_at(
                                    PopoverKind::worktree(
                                        repo_id,
                                        WorktreePopoverKind::FinishQueue { run_id },
                                    ),
                                    e.position(),
                                    window,
                                    cx,
                                );
                            })
                            .debug_selector(|| "worktree_finish_banner".to_string()),
                    )
                })
                .when(is_merging, |d| {
                    d.child(
                        div()
                            .debug_selector(|| "merge_controls".to_string())
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(theme.ui_text(12.0))
                                    .text_color(theme.colors.status.warning.foreground)
                                    .font_weight(FontWeight::BOLD)
                                    .child(language.text("MERGING", "合併中")),
                            )
                            .child(
                                components::Button::new("abort_merge", merge_abort_label)
                                    .style(components::ButtonStyle::Danger)
                                    .disabled(is_bare)
                                    .on_click(theme, cx, |this, e: &ClickEvent, window, cx| {
                                        if let Some(repo_id) = this.active_repo_id() {
                                            this.open_popover_at(
                                                PopoverKind::MergeAbortConfirm { repo_id },
                                                e.position(),
                                                window,
                                                cx,
                                            );
                                        }
                                    })
                                    .when(is_bare, |button| {
                                        button.gitcomet_tooltip(theme, worktree_required.into())
                                    }),
                            ),
                    )
                })
                .when_some(sequencer_banner.filter(|_| !is_merging), |d, banner| {
                    d.child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(theme.ui_text(12.0))
                                    .text_color(theme.colors.status.warning.foreground)
                                    .font_weight(FontWeight::BOLD)
                                    .child(banner.label),
                            )
                            .child(
                                components::Button::new(
                                    banner.abort_id,
                                    language.text("Abort", "中止"),
                                )
                                .style(components::ButtonStyle::Danger)
                                .disabled(is_bare || sequencer_step_busy)
                                .on_click(theme, cx, |this, e: &ClickEvent, window, cx| {
                                    if let Some(repo_id) = this.active_repo_id() {
                                        this.open_popover_at(
                                            PopoverKind::MergeAbortConfirm { repo_id },
                                            e.position(),
                                            window,
                                            cx,
                                        );
                                    }
                                })
                                .when(
                                    is_bare || sequencer_step_busy,
                                    |button| {
                                        button.gitcomet_tooltip(
                                            theme,
                                            if is_bare {
                                                worktree_required.into()
                                            } else {
                                                sequencer_busy_tooltip.into()
                                            },
                                        )
                                    },
                                ),
                            )
                            .child(
                                components::Button::new(
                                    banner.continue_id,
                                    language.text("Continue", "繼續"),
                                )
                                .style(components::ButtonStyle::Outlined)
                                .disabled(
                                    is_bare || rebase_has_unstaged_conflicts || sequencer_step_busy,
                                )
                                .on_click(theme, cx, |this, _e, _w, _cx| {
                                    if let Some(repo_id) = this.active_repo_id() {
                                        this.store.dispatch(Msg::RebaseContinue { repo_id });
                                    }
                                })
                                .gitcomet_tooltip(
                                    theme,
                                    if is_bare {
                                        worktree_required.into()
                                    } else if sequencer_step_busy {
                                        sequencer_busy_tooltip.into()
                                    } else if rebase_has_unstaged_conflicts {
                                        language
                                            .text(
                                                "Resolve all conflicts before continuing",
                                                "請先解決所有衝突再繼續",
                                            )
                                            .into()
                                    } else {
                                        banner.continue_tooltip.into()
                                    },
                                ),
                            ),
                    )
                });
            if has_active_finish_run || is_merging || sequencer_banner.is_some() {
                items.push(BuiltinBarItem::OperationControls, operation_controls);
            }
        }

        // The repository's views lead the right group, a divider apart from
        // the actions that act on the repository.
        let more_open = active_invoker.as_deref() == Some(MORE_VIEWS_INVOKER);
        let view_tabs = self.view_tabs.clone().map(|tabs| {
            div()
                .flex()
                .flex_none()
                .items_center()
                .h_full()
                .gap(action_group_gap)
                .child(self.view_tabs(&tabs, density != ActionBarDensity::Compact, more_open, cx))
                .child(
                    div()
                        .flex_none()
                        .w(px(1.0))
                        .h(scaled_px(17.0))
                        .bg(theme.colors.stroke.subtle),
                )
        });

        if let Some(tabs) = view_tabs {
            items.push(BuiltinBarItem::RepositoryViews, tabs);
        }
        let terminal_opens_external = self.terminal_button_target == TerminalButtonTarget::External;
        let terminal_is_open = !terminal_opens_external
            && self
                .active_repo_id()
                .is_some_and(|repo| self.open_terminal_repo_ids.contains(&repo));
        let terminal = div().debug_selector(|| "terminal".to_string()).child(
            components::Button::new(
                "terminal",
                action_label(language.text("Terminal", "終端機")),
            )
            .start_slot(icon("icons/terminal.svg", icon_primary))
            .style(components::ButtonStyle::Subtle)
            .selected(terminal_is_open)
            .selected_bg(menu_selected_bg)
            .disabled(self.active_repo_id().is_none())
            .on_click(theme, cx, |this, _, window, cx| {
                let _ = this.root_view.update(cx, |root, cx| {
                    root.activate_terminal_button_for_active_repo(window, cx);
                });
            })
            .gitcomet_tooltip(
                theme,
                if terminal_opens_external {
                    language
                        .text("Open external terminal", "開啟外部終端機")
                        .into()
                } else if terminal_is_open {
                    language.text("Hide terminal", "隱藏終端機").into()
                } else {
                    language.text("Show terminal", "顯示終端機").into()
                },
            ),
        );
        items.push(BuiltinBarItem::Terminal, terminal);
        items.extensions(&self.extension_items, &self.active_view);
        items
    }
}

impl Render for ActionBarView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let mut items = self.bar_items(true, window, cx);
        let theme = self.theme;
        let scale = crate::ui_scale::UiScale::current(cx);
        let density =
            action_bar_density(window.viewport_size().width, scale.percent(), theme.metrics);
        let dense = density != ActionBarDensity::Wide;
        let tight = tight_action_bar(density, theme.metrics.density);
        let gap = scale.px(if tight {
            4.0
        } else {
            theme.metrics.ramp(if dense { 4.0 } else { 8.0 }, 6.0)
        });
        div()
            .debug_selector(|| "action_bar".to_string())
            .w_full()
            .h(action_bar_height(cx))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px(scale.px(if dense { 4.0 } else { 8.0 }))
            .bg(theme.colors.surface.chrome)
            .child(
                div()
                    .debug_selector(|| "left_action_group".to_string())
                    .flex()
                    .items_center()
                    .gap(gap)
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .children(items.take(BarItemLocation::ActionBarStart))
                    .children(bar_items::placed_views(
                        &self.relocated_items,
                        BarItemLocation::ActionBarStart,
                    )),
            )
            .child(
                div()
                    .debug_selector(|| "right_action_group".to_string())
                    .flex()
                    .items_center()
                    .gap(gap)
                    .flex_none()
                    .h_full()
                    .children(items.take(BarItemLocation::ActionBarEnd))
                    .children(bar_items::placed_views(
                        &self.relocated_items,
                        BarItemLocation::ActionBarEnd,
                    )),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitcomet_core::domain::RepoSpec;
    use gitcomet_core::domain::Upstream;
    use std::path::PathBuf;

    fn stash_entry(index: usize, message: &str) -> gitcomet_core::domain::StashEntry {
        gitcomet_core::domain::StashEntry {
            index,
            id: gitcomet_core::domain::CommitId(format!("{index:040x}").into()),
            message: message.into(),
            created_at: None,
        }
    }

    fn repo_with_stashes(
        stashes: Loadable<Arc<Vec<gitcomet_core::domain::StashEntry>>>,
    ) -> RepoState {
        let mut repo = RepoState::new_opening(
            RepoId(7),
            RepoSpec {
                workdir: PathBuf::from("/tmp/stash-pop"),
            },
        );
        repo.stashes = stashes;
        repo
    }

    #[test]
    fn stash_pop_targets_the_newest_stash() {
        let repo = repo_with_stashes(Loadable::Ready(Arc::new(vec![
            stash_entry(1, "older"),
            stash_entry(0, "WIP on main"),
        ])));
        assert_eq!(
            stash_pop_target(Some(&repo), Language::English),
            StashPopTarget::Ready {
                index: 0,
                tooltip: "Pop stash@{0}: WIP on main".to_string(),
            }
        );
    }

    #[test]
    fn stash_pop_is_unavailable_without_stashes_or_a_working_tree() {
        assert_eq!(
            stash_pop_target(None, Language::English),
            StashPopTarget::Unavailable("No stashes to pop")
        );
        let empty = repo_with_stashes(Loadable::Ready(Arc::new(Vec::new())));
        assert_eq!(
            stash_pop_target(Some(&empty), Language::English),
            StashPopTarget::Unavailable("No stashes to pop")
        );
        // An unloaded list (the sidebar loads it) still pops stash@{0}.
        for unloaded in [Loadable::Loading, Loadable::NotLoaded] {
            let repo = repo_with_stashes(unloaded);
            assert_eq!(
                stash_pop_target(Some(&repo), Language::English),
                StashPopTarget::Ready {
                    index: 0,
                    tooltip: "Pop the newest stash".to_string(),
                }
            );
        }
        let mut bare = repo_with_stashes(Loadable::Ready(Arc::new(vec![stash_entry(0, "WIP")])));
        bare.repository_mode = Some(gitcomet_core::domain::RepositoryMode::Bare);
        assert_eq!(
            stash_pop_target(Some(&bare), Language::English),
            StashPopTarget::Unavailable(gitcomet_core::domain::WORKING_TREE_REQUIRED_MESSAGE)
        );
    }

    #[test]
    fn sequencer_banner_names_each_paused_operation() {
        use gitcomet_core::services::SequencerState;

        assert_eq!(
            sequencer_banner(SequencerState::None, Language::English),
            None
        );
        for (state, label, abort_id, continue_id) in [
            (
                SequencerState::RebaseOrApply,
                "APPLY/REBASE",
                "abort_rebase_or_apply",
                "continue_rebase_or_apply",
            ),
            (
                SequencerState::CherryPick,
                "CHERRY-PICKING",
                "abort_cherry_pick",
                "continue_cherry_pick",
            ),
            (
                SequencerState::Revert,
                "REVERTING",
                "abort_revert",
                "continue_revert",
            ),
        ] {
            let banner =
                sequencer_banner(state, Language::English).expect("paused operation has a banner");
            assert_eq!(
                (banner.label, banner.abort_id, banner.continue_id),
                (label, abort_id, continue_id)
            );
        }
    }

    #[test]
    fn file_browsing_badge_stays_available_on_the_working_tree_until_exit() {
        let mut repo = RepoState::new_opening(
            RepoId(1),
            RepoSpec {
                workdir: PathBuf::from("/tmp/repo"),
            },
        );
        assert!(file_browsing_badge(&repo).is_none());
        repo.file_browser.active = true;
        repo.file_browser.source =
            gitcomet_core::domain::FileSource::Commit(CommitId("deadbeef1234".into()));
        assert_eq!(file_browsing_badge(&repo).unwrap().0, "deadbeef");
        repo.file_browser.source = gitcomet_core::domain::FileSource::WorkingDirectory;
        let (label, tooltip) =
            file_browsing_badge(&repo).expect("browsing still has an exit control");
        assert_eq!(label, "Working tree");
        assert!(tooltip.contains("Exit file browsing"));
        repo.repository_mode = Some(gitcomet_core::domain::RepositoryMode::Bare);
        let (label, tooltip) = file_browsing_badge(&repo).unwrap();
        assert_eq!(label, "HEAD");
        assert_eq!(
            tooltip,
            "Browsing HEAD (no committed files). Click for history or Exit file browsing."
        );
        repo.file_browser.active = false;
        assert!(file_browsing_badge(&repo).is_none());
    }

    fn test_branch(name: &str, upstream: Option<Upstream>) -> Branch {
        Branch {
            name: name.to_string(),
            target: CommitId("deadbeef".into()),
            upstream,
            divergence: None,
        }
    }

    #[test]
    fn truncate_badge_label_leaves_short_labels_alone() {
        assert_eq!(truncate_badge_label("main"), "main");
    }

    #[test]
    fn truncate_badge_label_elides_long_labels() {
        // Unbounded labels would push the Pull/Push controls off the right edge.
        let long = "feature/PROJ-1234-refactor-the-entire-rendering-pipeline";
        let out = truncate_badge_label(long);
        assert!(out.ends_with('\u{2026}'), "expected an ellipsis, got {out}");
        assert_eq!(out.chars().count(), BADGE_LABEL_MAX_CHARS + 1);
    }

    #[test]
    fn truncate_badge_label_counts_characters_not_bytes() {
        // Multi-byte names must not be cut mid-character.
        let label = "ä".repeat(BADGE_LABEL_MAX_CHARS + 5);
        let out = truncate_badge_label(&label);
        assert_eq!(out.chars().count(), BADGE_LABEL_MAX_CHARS + 1);
    }

    #[test]
    fn roomier_densities_move_the_breakpoints_out() {
        let at = |density| crate::appearance::Appearance {
            density,
            ..crate::appearance::Appearance::default()
        };
        let comfortable = at(crate::appearance::UiDensity::Comfortable);
        assert_eq!(
            action_bar_density(px(1121.0), 100, comfortable),
            ActionBarDensity::Compact,
            "Comfortable needs more room before labels return"
        );
        assert_eq!(
            action_bar_density(px(1233.0), 100, comfortable),
            ActionBarDensity::Condensed
        );
    }

    #[test]
    fn action_bar_density_changes_at_scaled_breakpoints() {
        assert_eq!(
            action_bar_density(px(960.0), 100, crate::appearance::Appearance::default()),
            ActionBarDensity::Compact
        );
        assert_eq!(
            action_bar_density(px(961.0), 100, crate::appearance::Appearance::default()),
            ActionBarDensity::Compact
        );
        assert_eq!(
            action_bar_density(px(1120.0), 100, crate::appearance::Appearance::default()),
            ActionBarDensity::Compact
        );
        assert_eq!(
            action_bar_density(px(1121.0), 100, crate::appearance::Appearance::default()),
            ActionBarDensity::Condensed
        );
        assert_eq!(
            action_bar_density(px(1400.0), 100, crate::appearance::Appearance::default()),
            ActionBarDensity::Condensed
        );
        assert_eq!(
            action_bar_density(px(1401.0), 100, crate::appearance::Appearance::default()),
            ActionBarDensity::Wide
        );
        assert_eq!(
            action_bar_density(px(1200.0), 125, crate::appearance::Appearance::default()),
            ActionBarDensity::Compact
        );
    }

    #[test]
    fn secondary_action_labels_remain_visible_in_condensed_mode() {
        assert_eq!(
            secondary_action_label(ActionBarDensity::Compact, false, "Stash"),
            ""
        );
        assert_eq!(
            secondary_action_label(ActionBarDensity::Condensed, false, "Stash"),
            "Stash"
        );
        assert_eq!(
            secondary_action_label(ActionBarDensity::Wide, false, "Stash"),
            "Stash"
        );
    }

    /// Beside the repository's view tabs the condensed bar gives the badges
    /// the room the labels took.
    #[test]
    fn secondary_action_labels_give_way_to_view_tabs_below_wide() {
        assert_eq!(
            secondary_action_label(ActionBarDensity::Condensed, true, "Stash"),
            ""
        );
        assert_eq!(
            secondary_action_label(ActionBarDensity::Wide, true, "Stash"),
            "Stash"
        );
    }

    #[test]
    fn head_branch_tracking_upstream_name_returns_remote_tracking_name() {
        let head_branch = Loadable::Ready("main".to_string());
        let branches = Loadable::Ready(Arc::new(vec![test_branch(
            "main",
            Some(Upstream {
                remote: "origin".to_string(),
                branch: "feature/tooltip".to_string(),
            }),
        )]));

        assert_eq!(
            head_branch_tracking_upstream_name(&head_branch, &branches).as_deref(),
            Some("origin/feature/tooltip")
        );
    }

    #[test]
    fn pull_tooltip_text_includes_tracking_branch_on_second_line() {
        assert_eq!(
            pull_tooltip_text(3, Some("origin/main"), Language::English).as_ref(),
            "Pull 3 behind\norigin/main"
        );
        assert_eq!(
            pull_tooltip_text(0, None, Language::English).as_ref(),
            "Pull 0 behind"
        );
    }

    #[test]
    fn push_tooltip_text_includes_tracking_branch_on_second_line() {
        assert_eq!(
            push_tooltip_text(2, Some("origin/main"), Language::English).as_ref(),
            "Push 2 ahead\norigin/main"
        );
        assert_eq!(
            push_tooltip_text(0, None, Language::English).as_ref(),
            "Push 0 ahead"
        );
    }

    #[test]
    fn traditional_chinese_action_bar_preserves_upstream_and_sequencer_controls() {
        let language = Language::TraditionalChinese;
        let upstream = "origin/release/2026";
        assert_eq!(
            pull_tooltip_text(3, Some(upstream), language).as_ref(),
            "拉取 · 落後 3 筆提交\norigin/release/2026"
        );
        let banner = sequencer_banner(
            gitcomet_core::services::SequencerState::RebaseOrApply,
            language,
        )
        .unwrap();
        assert_eq!(banner.label, "套用／變基中");
        assert_eq!(banner.abort_id, "abort_rebase_or_apply");
        assert_eq!(banner.continue_id, "continue_rebase_or_apply");
    }

    #[test]
    fn notify_fingerprint_follows_the_stash_list() {
        let repo_id = RepoId(1);
        let mut state = AppState {
            active_repo: Some(repo_id),
            ..AppState::test_default()
        };
        state.repos.push(RepoState::new_opening(
            repo_id,
            RepoSpec {
                workdir: PathBuf::from("/tmp/repo"),
            },
        ));
        let before = ActionBarView::notify_fingerprint(&state);
        state.repos[0].stashes_rev = state.repos[0].stashes_rev.wrapping_add(1);
        assert_ne!(before, ActionBarView::notify_fingerprint(&state));
    }

    #[test]
    fn notify_fingerprint_changes_when_branches_rev_changes() {
        let repo_id = RepoId(1);
        let mut state = AppState {
            active_repo: Some(repo_id),
            ..AppState::test_default()
        };
        state.repos.push(RepoState::new_opening(
            repo_id,
            RepoSpec {
                workdir: PathBuf::from("/tmp/repo"),
            },
        ));

        let before = ActionBarView::notify_fingerprint(&state);
        state.repos[0].branches = Loadable::Ready(Arc::new(vec![test_branch(
            "main",
            Some(Upstream {
                remote: "origin".to_string(),
                branch: "main".to_string(),
            }),
        )]));
        state.repos[0].branches_rev = state.repos[0].branches_rev.wrapping_add(1);
        let after = ActionBarView::notify_fingerprint(&state);

        assert_ne!(before, after);
    }

    #[test]
    fn notify_fingerprint_changes_when_annex_pull_availability_changes() {
        let repo_id = RepoId(1);
        let mut repo = RepoState::new_opening(
            repo_id,
            RepoSpec {
                workdir: "/tmp/repo".into(),
            },
        );
        repo.head_branch = Loadable::Ready("adjusted/main(unlocked)".into());
        let mut state = AppState {
            repos: vec![repo],
            active_repo: Some(repo_id),
            ..AppState::test_default()
        };
        let before = ActionBarView::notify_fingerprint(&state);
        let mut support = gitcomet_core::large_files::LargeFileSupport::default();
        support.annex.uuid = Some("u".into());
        state.repos[0].large_file_support = Loadable::Ready(Arc::new(support));
        let detected = ActionBarView::notify_fingerprint(&state);
        assert_eq!(
            before, detected,
            "unknown support already protects the adjusted branch"
        );
        state.repos[0].large_file_support = Loadable::Ready(Arc::default());
        assert_ne!(detected, ActionBarView::notify_fingerprint(&state));
        state.repos[0].large_file_support = Loadable::Loading;
        state.large_file_settings.annex_pull_push = false;
        assert_ne!(detected, ActionBarView::notify_fingerprint(&state));
    }

    #[test]
    fn notify_fingerprint_changes_when_remote_branches_rev_changes() {
        let repo_id = RepoId(1);
        let mut state = AppState {
            active_repo: Some(repo_id),
            ..AppState::test_default()
        };
        state.repos.push(RepoState::new_opening(
            repo_id,
            RepoSpec {
                workdir: PathBuf::from("/tmp/repo"),
            },
        ));

        let before = ActionBarView::notify_fingerprint(&state);
        state.repos[0].remote_branches_rev = state.repos[0].remote_branches_rev.wrapping_add(1);
        let after = ActionBarView::notify_fingerprint(&state);

        assert_ne!(before, after);
    }

    /// Sidebar file search and folder expansion move `file_browser_rev` on
    /// every keystroke; the bar only shows the browse badge, which follows the
    /// browser's `active` flag and `source`.
    #[test]
    fn notify_fingerprint_ignores_file_browser_search() {
        let repo_id = RepoId(1);
        let mut state = AppState {
            active_repo: Some(repo_id),
            ..AppState::test_default()
        };
        state.repos.push(RepoState::new_opening(
            repo_id,
            RepoSpec {
                workdir: PathBuf::from("/tmp/repo"),
            },
        ));
        let before = ActionBarView::notify_fingerprint(&state);

        let browser = &mut state.repos[0].file_browser;
        browser.search_query = "src".into();
        browser.expanded_dirs.insert(Arc::new(PathBuf::from("src")));
        browser.bump_rev();
        assert_eq!(before, ActionBarView::notify_fingerprint(&state));

        state.repos[0].file_browser.active = true;
        state.repos[0].file_browser.bump_rev();
        assert_ne!(before, ActionBarView::notify_fingerprint(&state));
    }
}
