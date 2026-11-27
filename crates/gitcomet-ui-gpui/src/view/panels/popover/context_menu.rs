use super::*;
use crate::localization::Language;
use gitcomet_core::domain::Upstream;

mod activate;
mod annex;
pub(in crate::view) mod bisect;
mod branch;
pub(super) mod branch_drop;
mod branch_group;
mod branch_section;
mod browse_history;
mod change_tracking_settings;
mod commit;
mod commit_file;
mod commit_file_sort;
mod commit_options;
mod commit_sha_link;
mod conflict_resolver_chunk;
mod conflict_resolver_input_row;
mod conflict_resolver_output;
mod diff_actions;
mod diff_content_mode_settings;
mod diff_editor;
mod diff_hunk;
mod explorer_operations;
mod explorer_settings;
mod file_browser_file;
mod file_browser_folder;
pub(super) mod file_history_commit;
mod file_list_folder;
mod file_list_layout;
mod fixup_commit;
#[cfg(test)]
pub(in crate::view) use fixup_commit::fixup_target_blocker as fixup_target_blocker_for_test;
mod focus;
mod history_branch_filter;
mod history_refs;
mod large_file;
mod local_file_link;
mod mergetool_settings;
mod previous_commit_messages;
pub(super) mod pull;
mod push;
mod ref_menu;
mod ref_picker;
mod reflog_entry;
mod remote;
mod remote_edit_url_picker;
mod remote_web_picker;
mod repo_picker_row;
mod repo_tab;
mod stash;
mod status_file;
mod submodule;
mod submodule_inner_diff;
mod submodule_section;
mod tag;
mod terminal;
mod text_format_menu;
mod ui_scale_picker;
mod web_link;
pub(super) mod worktree;
mod worktree_file;
pub(super) mod worktree_section;

/// The menu entry that compares a commit against the mark named `label`.
pub(in crate::view) fn compare_with_mark_label(label: &str) -> SharedString {
    format!("Compare with {label}").into()
}

/// Comparing `commit_id` (named `label`) against the comparison mark: compare
/// and merge-base compare when the mark is another commit, and clearing the
/// mark wherever one is set, the marked commit's own row included.
fn marked_comparison_entries(
    repo_id: RepoId,
    commit_id: &CommitId,
    label: &str,
    mark: Option<(&CommitId, &str)>,
    language: Language,
) -> Vec<ContextMenuItem> {
    let Some((mark_commit, mark_label)) = mark else {
        return Vec::new();
    };
    let mut items = Vec::new();
    if mark_commit != commit_id {
        items.push(ContextMenuItem::Entry {
            label: match language {
                Language::English => compare_with_mark_label(mark_label),
                Language::TraditionalChinese => format!("與 {mark_label} 比較").into(),
            },
            icon: Some("icons/open_external.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::CompareWithMarked {
                repo_id,
                commit_id: commit_id.clone(),
                label: label.to_string(),
            }),
        });
        items.push(ContextMenuItem::Entry {
            label: language
                .text("Compare with merge base", "與共同祖先比較")
                .into(),
            icon: Some("icons/open_external.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::CompareWithMergeBase {
                repo_id,
                commit_id: commit_id.clone(),
                label: label.to_string(),
            }),
        });
    }
    items.push(ContextMenuItem::Entry {
        label: language
            .text("Clear comparison mark", "清除比較標記")
            .into(),
        icon: Some("icons/generic_close.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::ClearComparisonMark { repo_id }),
    });
    items
}

fn resolve_checkout_file_path(
    workdir: &std::path::Path,
    path: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    if path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
            )
        })
    {
        return Err("Refusing to open path outside repository".to_string());
    }
    Ok(normalize_platform_path(workdir.join(path)))
}

fn normalize_platform_path(path: std::path::PathBuf) -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        let mut normalized = std::path::PathBuf::new();
        for component in path.components() {
            normalized.push(component.as_os_str());
        }
        normalized
    }

    #[cfg(not(target_os = "windows"))]
    {
        path
    }
}

/// One line of the "Add to .gitignore" field as a submittable pattern, or
/// `None` when the line is blank.
///
/// Trailing spaces go through git's own rule rather than `str::trim`, which
/// would unescape a deliberate `foo\ ` back into a dangling backslash. Leading
/// whitespace is dropped: it is significant to git, but a leading space in a
/// hand-typed line is copy-paste noise far more often than intent, and the
/// resulting pattern would silently match nothing.
fn gitignore_pattern_line(line: &str) -> Option<&str> {
    let line = gitcomet_core::gitignore::trim_trailing_spaces(line.trim_start());
    (!line.is_empty()).then_some(line)
}

pub(super) fn path_text_for_copy(path: &std::path::Path) -> String {
    normalize_platform_path(path.to_path_buf())
        .display()
        .to_string()
}

/// The `Copy absolute path` / `Copy relative path` pair every file-ish menu
/// ends with, for a repo-relative `path`.
///
/// Built in one place so the labels, icons and mnemonic cannot drift apart
/// between menus: the mnemonic is matched on the key alone, so `c` has to mean
/// the same thing in whichever menu is open.
pub(super) fn push_copy_path_entries(
    items: &mut Vec<ContextMenuItem>,
    host: &PopoverHost,
    repo_id: RepoId,
    path: &std::path::Path,
    relative_shortcut: Option<SharedString>,
    language: Language,
) {
    // Offered only when the workdir join actually resolves. Falling back to the
    // repo-relative text would put identical content behind two entries whose
    // labels promise different things.
    if let Ok(absolute) = host.resolve_workdir_path(repo_id, path) {
        items.push(ContextMenuItem::Entry {
            label: language.text("Copy absolute path", "複製絕對路徑").into(),
            icon: Some("icons/copy.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::CopyText {
                text: path_text_for_copy(&absolute),
            }),
        });
    }
    // The root's relative path is empty text.
    if path.as_os_str().is_empty() {
        return;
    }
    items.push(ContextMenuItem::Entry {
        label: language.text("Copy relative path", "複製相對路徑").into(),
        icon: Some("icons/copy.svg".into()),
        shortcut: relative_shortcut,
        disabled: false,
        action: Box::new(ContextMenuAction::CopyText {
            text: path_text_for_copy(path),
        }),
    });
}

fn localized_menu(cx: &App, build: impl Fn(Language) -> ContextMenuModel) -> ContextMenuModel {
    let language = Language::current(cx);
    let mut model = build(language);
    if language != Language::English {
        // Both passes use the same repository state and action order. The menu
        // renderer still needs the source labels for its existing presentation rules.
        let source = build(Language::English);
        let source_labels = source.entry_source_labels;
        let translated_source_labels = source
            .items
            .into_iter()
            .zip(&model.items)
            .enumerate()
            .filter_map(
                |(index, (original, translated))| match (original, translated) {
                    (
                        ContextMenuItem::Entry {
                            label: source,
                            action: source_action,
                            ..
                        },
                        ContextMenuItem::Entry { label, action, .. },
                    ) if &source != label
                        && std::mem::discriminant(source_action.as_ref())
                            == std::mem::discriminant(action.as_ref()) =>
                    {
                        let label = source_labels.get(&index).cloned().unwrap_or(source);
                        Some((index, label))
                    }
                    _ => None,
                },
            );
        model.entry_source_labels.extend(translated_source_labels);
    }
    model
}

fn active_branch_tracking_upstream(host: &PopoverHost) -> Option<&Upstream> {
    let repo = host.active_repo()?;
    head_branch_upstream(&repo.head_branch, &repo.branches)
}

fn action_menu_title(base: &'static str, upstream: Option<&Upstream>) -> SharedString {
    match upstream {
        Some(upstream) => format!("{base} {}/{}", upstream.remote, upstream.branch).into(),
        None => base.into(),
    }
}

pub(super) fn context_menu_entry_debug_selector(label: &str) -> String {
    let mut slug = String::with_capacity(label.len());
    let mut previous_was_separator = true;

    for ch in label.chars() {
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
        "context_menu_entry".to_string()
    } else {
        format!("context_menu_{slug}")
    }
}

pub(super) fn selection_key(
    model: &ContextMenuModel,
    ix: usize,
) -> Option<(std::mem::Discriminant<ContextMenuAction>, SharedString)> {
    let ContextMenuItem::Entry { label, action, .. } = model.items.get(ix)? else {
        return None;
    };
    let identity = match action.as_ref() {
        ContextMenuAction::ToggleHistoryRefGroup { target } => format!("{target:?}").into(),
        ContextMenuAction::PushWithTags { mode, .. } => mode.label().into(),
        _ => label.clone(),
    };
    Some((std::mem::discriminant(action.as_ref()), identity))
}

pub(super) fn context_menu_entry_action_at(
    model: &ContextMenuModel,
    ix: usize,
) -> Option<ContextMenuAction> {
    match model.items.get(ix) {
        Some(ContextMenuItem::Entry { action, .. }) => Some((**action).clone()),
        _ => None,
    }
}

fn context_menu_entry_tooltip(action: &ContextMenuAction) -> Option<SharedString> {
    match action {
        ContextMenuAction::UseCommitMessage { message } => {
            let text = message.trim();
            (!text.is_empty()).then(|| text.to_owned().into())
        }
        _ => None,
    }
}

pub(in super::super) fn context_menu_activate_entry_ix(
    model: &ContextMenuModel,
    selected_ix: Option<usize>,
) -> Option<usize> {
    selected_ix
        .filter(|&ix| model.is_selectable(ix))
        .or_else(|| model.first_selectable())
}

pub(in super::super) fn context_menu_shortcut_entry_ix(
    model: &ContextMenuModel,
    key: &str,
) -> Option<usize> {
    if key.chars().count() != 1 {
        return None;
    }

    model.items.iter().enumerate().find_map(|(ix, item)| {
        let ContextMenuItem::Entry {
            shortcut, disabled, ..
        } = item
        else {
            return None;
        };
        if *disabled {
            return None;
        }
        let shortcut = shortcut.as_ref()?;
        let shortcut_key = shortcut
            .as_ref()
            .rsplit('+')
            .next()
            .unwrap_or(shortcut.as_ref());
        shortcut_key.eq_ignore_ascii_case(key).then_some(ix)
    })
}

impl PopoverHost {
    pub(super) fn sidebar_pin_overflow_view(
        &mut self,
        repo_id: RepoId,
        bottom: bool,
        max_height: Pixels,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::Div {
        if !self
            .pin_menu_entries
            .as_ref()
            .is_some_and(|(repo, edge, _)| *repo == repo_id && *edge == bottom)
        {
            let entries = self
                .sidebar_pane
                .read(cx)
                .sidebar_pin_overflow_entries(bottom);
            self.pin_menu_entries = Some((repo_id, bottom, entries.into()));
            self.pin_menu_scroll
                .scroll_to_item(0, gpui::ScrollStrategy::Top);
        }
        let entries = self.pin_menu_entries.as_ref().unwrap().2.clone();
        let for_keys = entries.clone();
        let theme = self.theme;
        let scale = super::popover_ui_scale(cx);
        let row_height = components::control_height(scale);
        let height =
            (row_height * entries.len().min(8)).min((max_height - scale.px(40.0)).max(row_height));
        let list = uniform_list(
            "sidebar_pin_overflow_list",
            entries.len(),
            cx.processor(move |this, range: Range<usize>, _, cx| {
                range
                    .map(|ix| {
                        let (label, key) = entries[ix].clone();
                        let selected = this.context_menu_selected_ix == Some(ix);
                        components::Button::new(format!("pin_overflow_entry_{ix}"), label)
                            .borderless()
                            .style(components::ButtonStyle::Subtle)
                            .selected(selected)
                            .on_click(theme, cx, move |this, _, window, cx| {
                                this.sidebar_pane.update(cx, |pane, cx| {
                                    pane.navigate_sidebar_row_key(key.clone(), cx)
                                });
                                this.close_popover_and_restore_focus(window, cx);
                            })
                            .w_full()
                            .h(row_height)
                            .into_any_element()
                    })
                    .collect()
            }),
        )
        .h_full()
        .track_scroll(&self.pin_menu_scroll);
        div()
            .w(scale.px(280.0))
            .flex()
            .flex_col()
            .min_h(px(0.0))
            .track_focus(&self.context_menu_focus_handle)
            .key_context("ContextMenu")
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    match event.keystroke.key.as_str() {
                        "escape" => this.close_popover_and_restore_focus(window, cx),
                        "enter" | "space" => {
                            if let Some((_, key)) = this
                                .context_menu_selected_ix
                                .and_then(|ix| for_keys.get(ix))
                            {
                                this.sidebar_pane.update(cx, |pane, cx| {
                                    pane.navigate_sidebar_row_key(key.clone(), cx)
                                });
                                this.close_popover_and_restore_focus(window, cx);
                            }
                        }
                        "up" | "down" | "home" | "end" if !for_keys.is_empty() => {
                            let next = match event.keystroke.key.as_str() {
                                "home" => 0,
                                "end" => for_keys.len() - 1,
                                "up" => this
                                    .context_menu_selected_ix
                                    .unwrap_or(for_keys.len())
                                    .saturating_sub(1),
                                _ => this
                                    .context_menu_selected_ix
                                    .map_or(0, |ix| (ix + 1).min(for_keys.len() - 1)),
                            };
                            this.context_menu_selected_ix = Some(next);
                            this.pin_menu_scroll
                                .scroll_to_item(next, gpui::ScrollStrategy::Center);
                            cx.notify();
                        }
                        _ => return,
                    }
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .px(scale.px(8.0))
                    .py(scale.px(6.0))
                    .text_size(theme.ui_text(12.0))
                    .child("More pinned items"),
            )
            .child(
                div().relative().h(height).min_h(px(0.0)).child(list).child(
                    components::Scrollbar::new(
                        "pin_overflow_scrollbar",
                        self.pin_menu_scroll.clone(),
                    )
                    .render(theme),
                ),
            )
    }

    pub(super) fn workdir_for_repo(&self, repo_id: RepoId) -> Option<std::path::PathBuf> {
        if let Some(binding) = self.store.binding {
            let repo = binding
                .repository(&self.state)
                .filter(|repo| repo.id == repo_id)?;
            return Some(
                repo.diff_sessions
                    .get(&binding.view)?
                    .worktree()
                    .unwrap_or(&repo.spec.workdir)
                    .to_path_buf(),
            );
        }
        self.state
            .repos
            .iter()
            .find(|r| r.id == repo_id)
            .map(|r| r.spec.workdir.clone())
    }

    fn resolve_workdir_path(
        &self,
        repo_id: RepoId,
        path: &std::path::Path,
    ) -> Result<std::path::PathBuf, String> {
        let workdir = self
            .workdir_for_repo(repo_id)
            .ok_or_else(|| "Repository is not available".to_string())?;
        resolve_checkout_file_path(&workdir, path)
    }

    fn open_path_default(&mut self, path: std::path::PathBuf, cx: &mut gpui::Context<Self>) {
        crate::view::platform_open::spawn_launch(
            cx,
            move || crate::view::platform_open::open_path_blocking(&path),
            |this, result, cx| {
                if let Err(err) = result {
                    this.push_toast(
                        components::ToastKind::Error,
                        format!("Failed to open: {err}"),
                        cx,
                    );
                }
            },
        );
    }

    fn open_file_location(&mut self, path: std::path::PathBuf, cx: &mut gpui::Context<Self>) {
        crate::view::platform_open::spawn_launch(
            cx,
            move || crate::view::platform_open::open_file_location_blocking(&path),
            |this, result, cx| {
                if let Err(err) = result {
                    this.push_toast(
                        components::ToastKind::Error,
                        format!("Failed to open location: {err}"),
                        cx,
                    );
                }
            },
        );
    }

    fn reveal_path_in_file_manager(
        &mut self,
        path: std::path::PathBuf,
        fallback: Option<std::path::PathBuf>,
        cx: &mut gpui::Context<Self>,
    ) {
        let target = if path.exists() {
            path
        } else {
            path.parent()
                .map(ToOwned::to_owned)
                .or(fallback)
                .unwrap_or(path)
        };

        if !target.exists() {
            self.push_toast(
                components::ToastKind::Error,
                format!("Path not found: {}", target.display()),
                cx,
            );
        } else {
            self.open_file_location(target, cx);
        }
    }

    /// The paths a context-menu action on `clicked_path` covers, plus whether
    /// they came out of the row selection. Reads only — see
    /// [`Self::clear_status_multi_selection`] for the other half.
    /// The screen point a follow-up popover should open at, derived from the
    /// anchor of the menu that is currently open.
    ///
    /// Every context-menu action that opens a dialog needs this, and the six
    /// copies it replaced all carried the same duplicated fallback constant.
    fn popover_anchor_point(&self) -> gpui::Point<Pixels> {
        self.popover_anchor
            .as_ref()
            .map(|anchor| match anchor {
                PopoverAnchor::Point(point) => *point,
                PopoverAnchor::Bounds(bounds) => bounds.bottom_right(),
                PopoverAnchor::Centered => point(px(64.0), px(64.0)),
            })
            .unwrap_or_else(|| point(px(64.0), px(64.0)))
    }

    fn status_paths_for_action(
        &self,
        repo_id: RepoId,
        area: DiffArea,
        clicked_path: &std::path::PathBuf,
        cx: &gpui::App,
    ) -> (Vec<std::path::PathBuf>, bool) {
        self.details_pane
            .read(cx)
            .status_selected_paths_for_action(repo_id, area, clicked_path)
    }

    /// Drop the row selection because an action has gone ahead with it. Never
    /// call this before the action is settled: a confirmation the user cancels
    /// must leave the selection standing.
    pub(super) fn clear_status_multi_selection(
        &mut self,
        repo_id: RepoId,
        cx: &mut gpui::Context<Self>,
    ) {
        self.details_pane.update(cx, |pane, cx| {
            pane.clear_status_multi_selection(repo_id);
            cx.notify();
        });
    }

    fn take_status_paths_for_action(
        &mut self,
        repo_id: RepoId,
        area: DiffArea,
        clicked_path: &std::path::PathBuf,
        cx: &mut gpui::Context<Self>,
    ) -> (Vec<std::path::PathBuf>, bool) {
        let (paths, used_selection) = self.status_paths_for_action(repo_id, area, clicked_path, cx);
        if used_selection {
            self.clear_status_multi_selection(repo_id, cx);
        }
        (paths, used_selection)
    }

    fn repo_is_open(&self, repo_id: RepoId) -> bool {
        self.state.repos.iter().any(|repo| repo.id == repo_id)
    }

    /// A toast rather than an error banner: nothing failed, the row just went
    /// stale, and the banner belongs to whichever repository is active now — not
    /// to the one that left.
    fn warn_repository_gone(&mut self, cx: &mut gpui::Context<Self>) {
        self.push_toast(
            components::ToastKind::Warning,
            "That repository is no longer open.".to_owned(),
            cx,
        );
    }

    /// The menu a repository row in the picker offers. Not reachable through
    /// [`Self::context_menu_model`] because it has no popover kind of its own:
    /// it is only ever floated over the picker by
    /// [`super::picker_row_menu`](crate::view::panels::popover), never opened as
    /// a popover in its own right.
    pub(super) fn repo_picker_row_menu_model(
        &self,
        entry: &repo_picker::RepoPickerEntry,
    ) -> ContextMenuModel {
        repo_picker_row::model(self, entry, Language::English)
    }

    pub(super) fn localized_repo_picker_row_menu_model(
        &self,
        entry: &repo_picker::RepoPickerEntry,
        cx: &App,
    ) -> ContextMenuModel {
        localized_menu(cx, |language| repo_picker_row::model(self, entry, language))
    }

    /// The actions of the entries of the menu `kind` opens, for tests.
    #[cfg(test)]
    pub(in crate::view) fn context_menu_actions_for_test(
        &self,
        kind: &PopoverKind,
        cx: &gpui::Context<Self>,
    ) -> Vec<ContextMenuAction> {
        let model = self.context_menu_model(kind, cx).unwrap();
        model
            .items
            .iter()
            .filter_map(|item| match item {
                ContextMenuItem::Entry { action, .. } => Some((**action).clone()),
                _ => None,
            })
            .collect()
    }

    pub(in super::super) fn context_menu_model(
        &self,
        kind: &PopoverKind,
        cx: &gpui::Context<Self>,
    ) -> Option<ContextMenuModel> {
        let model = match kind {
            PopoverKind::Repo {
                repo_id,
                kind: RepoPopoverKind::RepositoryModeMenu,
            } => Some(bare_mode::menu(self, *repo_id)),
            PopoverKind::Hosted { id, menu: true } => self
                .extension_dialog
                .as_ref()
                .filter(|dialog| dialog.id == *id)
                .and_then(|dialog| dialog.menu.clone()),
            PopoverKind::AppMenu => Some(localized_menu(cx, |language| {
                app_menu::model(self, language)
            })),
            PopoverKind::AddRepoMenu => Some(localized_menu(cx, add_repo_menu::model)),
            PopoverKind::PullPicker => {
                Some(localized_menu(cx, |language| pull::model(self, language)))
            }
            PopoverKind::PushPicker => {
                Some(localized_menu(cx, |language| push::model(self, language)))
            }
            PopoverKind::CommitOptionsMenu { repo_id } => {
                Some(commit_options::model(self, *repo_id))
            }
            PopoverKind::PreviousCommitMessagesMenu { repo_id } => {
                Some(previous_commit_messages::model(self, *repo_id))
            }
            PopoverKind::RepoTabMenu { repo_id } => Some(localized_menu(cx, |language| {
                repo_tab::model(self, *repo_id, language)
            })),
            PopoverKind::CommitMenu {
                repo_id,
                commit_id,
                refs,
            } => Some(localized_menu(cx, |language| {
                // A stash in the graph gets the stash menu, as in GitKraken:
                // Apply, Pop, and Drop through its confirmation.
                stash_menu_for_graph_node(self, *repo_id, commit_id, refs, language).unwrap_or_else(
                    || history_refs::model(self, *repo_id, commit_id, refs, language),
                )
            })),
            PopoverKind::ReflogEntryMenu {
                repo_id,
                target,
                selector,
            } => Some(localized_menu(cx, |language| {
                reflog_entry::model(*repo_id, selector, target, language)
            })),
            PopoverKind::FileHistoryCommitMenu {
                repo_id,
                commit_id,
                path,
            } => Some(localized_menu(cx, |language| {
                file_history_commit::model(self, *repo_id, commit_id, path, language)
            })),
            PopoverKind::TagMenu { repo_id, commit_id } => Some(localized_menu(cx, |language| {
                tag::model(self, *repo_id, commit_id, language)
            })),
            PopoverKind::StatusFileMenu {
                repo_id,
                area,
                path,
            } => Some(localized_menu(cx, |language| {
                status_file::model(self, *repo_id, *area, path, cx, language)
            })),
            PopoverKind::StatusConflictMenu {
                repo_id,
                area,
                path,
            } => Some(localized_menu(cx, |language| {
                status_file::conflict_model(self, *repo_id, *area, path, cx, language)
            })),
            PopoverKind::BranchMenu { repo_id, target } => Some(localized_menu(cx, |language| {
                branch::model(self, *repo_id, target, true, language)
            })),
            PopoverKind::BranchDropMenu {
                repo_id,
                source,
                target,
            } => Some(localized_menu(cx, |language| {
                branch_drop::model(self, *repo_id, source, target, language)
            })),
            PopoverKind::RefPickerMenu { repo_id, purpose } => {
                Some(localized_menu(cx, |language| {
                    ref_picker::model(self, *repo_id, *purpose, language)
                }))
            }
            PopoverKind::BranchSectionMenu { repo_id, section } => {
                Some(localized_menu(cx, |language| {
                    branch_section::model(self, *repo_id, *section, language)
                }))
            }
            PopoverKind::Repo {
                repo_id,
                kind: RepoPopoverKind::Remote(RemotePopoverKind::Menu { name }),
            } => Some(localized_menu(cx, |language| {
                remote::model(self, *repo_id, name, language)
            })),
            PopoverKind::Repo {
                repo_id,
                kind: RepoPopoverKind::Remote(RemotePopoverKind::OpenInBrowserMenu),
            } => Some(localized_menu(cx, |language| {
                remote_web_picker::model(self, *repo_id, language)
            })),
            PopoverKind::Repo {
                repo_id,
                kind: RepoPopoverKind::Remote(RemotePopoverKind::EditUrlMenu),
            } => Some(localized_menu(cx, |language| {
                remote_edit_url_picker::model(self, *repo_id, language)
            })),
            PopoverKind::Repo {
                repo_id,
                kind: RepoPopoverKind::Remote(RemotePopoverKind::RemoveMenu),
            } => Some(localized_menu(cx, |language| {
                remote_edit_url_picker::remove_model(self, *repo_id, language)
            })),
            PopoverKind::WebLinkMenu {
                url,
                load_remote_image_url,
            } => {
                let load_remote_image_url = load_remote_image_url.as_deref().filter(|_| {
                    self.diff_renderer.read(cx).remote_markdown_images.policy
                        == RemoteMarkdownImagePolicy::AskBeforeLoading
                });
                Some(web_link::model(url, load_remote_image_url))
            }
            PopoverKind::LocalFileLinkMenu {
                repo_id,
                source,
                path,
                missing,
                load_remote_image_url,
            } => {
                let load_remote_image_url = load_remote_image_url.as_deref().filter(|_| {
                    self.diff_renderer.read(cx).remote_markdown_images.policy
                        == RemoteMarkdownImagePolicy::AskBeforeLoading
                });
                Some(local_file_link::model(
                    *repo_id,
                    source,
                    path,
                    *missing,
                    load_remote_image_url,
                ))
            }
            PopoverKind::CommitShaLinkMenu { repo_id, commit_id } => {
                Some(commit_sha_link::model(*repo_id, commit_id))
            }
            PopoverKind::StashMenu {
                repo_id,
                index,
                message,
            } => Some(localized_menu(cx, |language| {
                stash::model(*repo_id, *index, message, language)
            })),
            PopoverKind::StashRestoreMenu { repo_id } => Some(stash::restore_menu(*repo_id, cx)),
            PopoverKind::Repo {
                repo_id,
                kind: RepoPopoverKind::Worktree(WorktreePopoverKind::SectionMenu),
            } => Some(localized_menu(cx, |language| {
                worktree_section::model(*repo_id, language)
            })),
            PopoverKind::Repo {
                repo_id,
                kind: RepoPopoverKind::Worktree(WorktreePopoverKind::CommitMenu { commit_id }),
            } => Some(localized_menu(cx, |language| {
                worktree::commit_model(self, *repo_id, commit_id, language)
            })),
            PopoverKind::Repo {
                repo_id,
                kind: RepoPopoverKind::Worktree(WorktreePopoverKind::Menu { path, branch }),
            } => Some(localized_menu(cx, |language| {
                if self.history_worktree_menu_is_stale() {
                    return ContextMenuModel::new(vec![ContextMenuItem::Label(
                        language
                            .text(
                                "Worktree changed. Reopen the menu.",
                                "工作樹已變更，請重新開啟選單。",
                            )
                            .into(),
                    )]);
                }
                let protected_by_finish =
                    self.state.worktree_finish_protects_worktree(*repo_id, path)
                        || branch.as_deref().is_some_and(|b| {
                            self.state.worktree_finish_protects_branch(*repo_id, b)
                        });
                worktree::model(
                    *repo_id,
                    path,
                    branch.as_deref(),
                    self.external_editor_available(),
                    self.worktree_menu_location
                        .as_ref()
                        .is_some_and(|(cached_path, available)| cached_path == path && *available),
                    protected_by_finish,
                    language,
                )
            })),
            PopoverKind::Repo {
                repo_id,
                kind: RepoPopoverKind::Submodule(SubmodulePopoverKind::SectionMenu),
            } => Some(submodule_section::model(*repo_id)),
            PopoverKind::Repo {
                repo_id,
                kind: RepoPopoverKind::Submodule(SubmodulePopoverKind::Menu { path }),
            } => Some(submodule::model(self, *repo_id, path)),
            PopoverKind::Repo {
                repo_id,
                kind: RepoPopoverKind::Annex(AnnexPopoverKind::SectionMenu),
            } => Some(annex::section_model(self, *repo_id)),
            PopoverKind::Repo {
                repo_id,
                kind: RepoPopoverKind::Annex(AnnexPopoverKind::RepositoryMenu { uuid }),
            } => Some(annex::repository_model(self, *repo_id, uuid)),
            PopoverKind::CommitFileMenu {
                repo_id,
                commit_id,
                path,
            } => Some(localized_menu(cx, |language| {
                commit_file::model(
                    self,
                    *repo_id,
                    commit_file::FileMenuSource::Commit(commit_id),
                    path,
                    cx,
                    language,
                )
            })),
            PopoverKind::WorktreeFileMenu {
                repo_id,
                worktree_path,
                target,
            } => Some(worktree_file::model(self, *repo_id, worktree_path, target)),
            PopoverKind::CommitRangeFileMenu {
                repo_id,
                from_commit_id,
                to_commit_id,
                path,
            } => Some(localized_menu(cx, |language| {
                commit_file::model(
                    self,
                    *repo_id,
                    commit_file::FileMenuSource::Range {
                        from: from_commit_id,
                        to: to_commit_id.as_ref(),
                    },
                    path,
                    cx,
                    language,
                )
            })),
            PopoverKind::CommitFileSortMenu { list } => {
                Some(commit_file_sort::model(self, *list, cx))
            }
            PopoverKind::FileListLayoutMenu { repo_id, list } => {
                Some(file_list_layout::model(self, *repo_id, *list, cx))
            }
            PopoverKind::FileBrowserFileMenu { repo_id, path } => {
                Some(localized_menu(cx, |language| {
                    file_browser_file::model(self, *repo_id, path, cx, language)
                }))
            }
            PopoverKind::FileBrowserFolderMenu { repo_id, path } => {
                Some(localized_menu(cx, |language| {
                    file_browser_folder::model(self, *repo_id, path, language)
                }))
            }
            PopoverKind::FileListFolderMenu {
                repo_id,
                list,
                key,
                chain,
                collapsed,
                apply_source,
            } => Some(localized_menu(cx, |language| {
                file_list_folder::model(
                    self,
                    file_list_folder::FolderMenu {
                        repo_id: *repo_id,
                        list: *list,
                        key,
                        chain,
                        collapsed: *collapsed,
                        apply_source: apply_source.as_ref(),
                    },
                    cx,
                    language,
                )
            })),
            PopoverKind::BranchGroupMenu {
                repo_id,
                section,
                remote,
                path,
            } => Some(localized_menu(cx, |language| {
                branch_group::model(self, *repo_id, *section, remote.as_deref(), path, language)
            })),
            PopoverKind::SidebarAncestorMenu { section, .. } => {
                let mut items = vec![ContextMenuItem::Header("Active branch groups".into())];
                for (label, key) in self
                    .sidebar_pane
                    .read(cx)
                    .sidebar_ancestor_menu_entries(*section)
                {
                    items.push(ContextMenuItem::Entry {
                        label: format!("Collapse {label}").into(),
                        icon: Some("icons/chevron_down.svg".into()),
                        shortcut: None,
                        disabled: false,
                        action: Box::new(ContextMenuAction::SetSidebarCollapseKey {
                            collapse_key: key,
                            collapsed: true,
                        }),
                    });
                }
                Some(ContextMenuModel::new(items))
            }
            PopoverKind::BrowseHistoryMenu { repo_id } => {
                Some(browse_history::model(self, *repo_id))
            }
            PopoverKind::SubmoduleInnerDiffMenu {
                repo_id,
                submodule_repo_path,
                target,
            } => Some(submodule_inner_diff::model(
                *repo_id,
                submodule_repo_path,
                target,
            )),
            PopoverKind::DiffHunkMenu { repo_id, src_ix } => Some(localized_menu(cx, |language| {
                diff_hunk::model(self, *repo_id, *src_ix, cx, language)
            })),
            PopoverKind::DiffEditorMenu {
                context_actions,
                repo_id,
                area,
                path,
                hunk_patch,
                hunks_count,
                lines_patch,
                discard_lines_patch,
                lines_count,
                copy_text,
                copy_target,
            } => Some(diff_editor::model(
                context_actions,
                *repo_id,
                *area,
                path,
                hunk_patch,
                *hunks_count,
                lines_patch,
                discard_lines_patch,
                *lines_count,
                copy_text,
                *copy_target,
                self.external_editor_available(),
            )),
            PopoverKind::ConflictResolverInputRowMenu {
                line_label,
                line_target,
                chunk_label,
                chunk_target,
            } => Some(conflict_resolver_input_row::model(
                line_label,
                line_target,
                chunk_label,
                chunk_target,
            )),
            PopoverKind::ConflictResolverChunkMenu {
                conflict_ix,
                has_base,
                is_three_way,
                selected_choices,
                output_line_ix,
                split_selection_rows,
                join_previous_region,
                join_next_region,
                alignment_marked_columns,
                has_manual_alignments,
                output_is_protected,
            } => Some(conflict_resolver_chunk::model(
                *conflict_ix,
                *has_base,
                *is_three_way,
                selected_choices,
                *output_line_ix,
                *split_selection_rows,
                join_previous_region.clone(),
                join_next_region.clone(),
                *alignment_marked_columns,
                *has_manual_alignments,
                *output_is_protected,
            )),
            PopoverKind::ConflictResolverOutputMenu {
                cursor_line,
                selected_text,
                has_source_a,
                has_source_b,
                has_source_c,
                is_three_way,
            } => Some(conflict_resolver_output::model(
                *cursor_line,
                selected_text,
                *has_source_a,
                *has_source_b,
                *has_source_c,
                *is_three_way,
            )),
            PopoverKind::HistoryBranchFilter { repo_id } => {
                Some(history_branch_filter::model(self, *repo_id, cx))
            }
            PopoverKind::DiffActionMenu => Some(diff_actions::model(self, cx)),
            PopoverKind::ExplorerSettingsMenu { repo_id } => {
                Some(explorer_settings::model(self, *repo_id))
            }
            PopoverKind::MergetoolSettingsMenu => Some(mergetool_settings::model(self, cx)),
            PopoverKind::DiffContentModeSettings => Some(diff_content_mode_settings::model(self)),
            PopoverKind::TextFormatMenu { section } => {
                Some(text_format_menu::model(self, *section, cx))
            }
            PopoverKind::ChangeTrackingSettings => Some(change_tracking_settings::model(self)),
            PopoverKind::UiScalePicker => Some(ui_scale_picker::model(cx)),
            PopoverKind::InteractiveRebaseActionMenu {
                ix,
                can_squash,
                can_drop,
            } => {
                let pick_locked = self
                    .main_pane
                    .read_with(cx, |pane, _| pane.active_entry_pick_locked(*ix));
                Some(interactive_rebase_action_menu_model(
                    *ix,
                    *can_squash,
                    *can_drop,
                    pick_locked,
                ))
            }
            PopoverKind::InteractiveRebaseAutosquashMenu => {
                Some(interactive_rebase_autosquash_menu_model())
            }
            PopoverKind::TerminalMenu {
                repo_id,
                session_seq,
                context,
            } => Some(terminal::model(*repo_id, *session_seq, *context, cx)),
            _ => None,
        }?;
        Some(self.gate_bare_menu(model))
    }

    /// What an "Add to .gitignore" action on `path` would cover, or `None` when
    /// the action does not apply.
    ///
    /// The single source of truth for eligibility: the menu uses it to decide
    /// whether to show the entry and the dialog uses it to seed itself. Two
    /// copies of this rule would let the menu offer an action the dialog then
    /// refuses (or the reverse), and nothing would catch the drift.
    ///
    /// The selection is read but *not* consumed — the dialog is cancellable, and
    /// losing a selection to a dialog the user backed out of is exactly the
    /// failure the read/take split documented above exists to prevent.
    pub(super) fn add_to_gitignore_target(
        &self,
        repo_id: RepoId,
        area: DiffArea,
        path: &std::path::PathBuf,
        cx: &gpui::App,
    ) -> Option<(
        Vec<std::path::PathBuf>,
        gitcomet_core::gitignore::GitignoreSuggestions,
    )> {
        use gitcomet_core::domain::FileStatusKind;

        // `.gitignore` has no effect on anything already in the index, so a
        // pattern for a tracked path is a line that changes nothing and leaves
        // the row exactly where it was.
        if area != DiffArea::Unstaged {
            return None;
        }
        let repo = self.state.repos.iter().find(|r| r.id == repo_id)?;
        let (paths, used_selection) = self.status_paths_for_action(repo_id, area, path, cx);

        // Every targeted path must be untracked, not just the clicked one: the
        // untracked and unstaged buckets are separate, and a tracked path that
        // snuck into the selection is a silent no-op the user would debug.
        // Indexed when there is a selection to check, because one
        // `status_entry_for_path` per path is a linear scan of the whole status
        // list each time and this runs on every right-click.
        let all_untracked = if used_selection {
            let untracked: FxHashSet<&std::path::Path> = repo
                .status_entries_for_area(area)
                .unwrap_or(&[])
                .iter()
                .filter(|entry| entry.kind == FileStatusKind::Untracked)
                .map(|entry| entry.path.as_path())
                .collect();
            paths.iter().all(|p| untracked.contains(p.as_path()))
        } else {
            paths.first().is_some_and(|p| {
                matches!(
                    repo.status_entry_for_path(area, p).map(|s| s.kind),
                    Some(FileStatusKind::Untracked)
                )
            })
        };
        if !all_untracked {
            return None;
        }

        // Rules out anything with no expressible pattern at all: a non-UTF-8
        // path, or a name containing a line break.
        let suggestions = gitcomet_core::gitignore::suggestions_for_paths(&paths)?;
        Some((paths, suggestions))
    }

    pub(super) fn explorer_gitignore_target(
        &self,
        repo_id: RepoId,
        path: &std::path::Path,
    ) -> Option<(
        Vec<std::path::PathBuf>,
        gitcomet_core::gitignore::GitignoreSuggestions,
    )> {
        let repo = self.state.repos.iter().find(|r| r.id == repo_id)?;
        if repo.file_browser.source != gitcomet_core::domain::FileSource::WorkingDirectory
            || path.as_os_str().is_empty()
        {
            return None;
        }
        let paths: Vec<_> = if repo.file_browser.selection.paths.contains(path) {
            repo.file_browser.selection.paths.iter().cloned().collect()
        } else {
            vec![path.to_path_buf()]
        };
        let Loadable::Ready(entries) = &repo.file_browser.entries else {
            return None;
        };
        let mut targets = Vec::new();
        for path in &paths {
            if matches!(&repo.submodules, Loadable::Ready(submodules) if submodules.iter().any(|submodule| submodule.path.as_path() == path))
            {
                return None;
            }
            let directory = entries.iter().any(|e| {
                e.path.as_path() == path
                    && e.kind == gitcomet_core::domain::FileEntryKind::Directory
            });
            if !directory
                && !repo
                    .status_entry_for_path(DiffArea::Unstaged, path)
                    .is_some_and(|s| s.kind == gitcomet_core::domain::FileStatusKind::Untracked)
            {
                return None;
            }
            targets.push((path.clone(), directory));
        }
        let suggestions = gitcomet_core::gitignore::suggestions_for_entries(&targets)?;
        Some((paths, suggestions))
    }

    /// Seed the "Add to .gitignore" dialog when it opens.
    pub(super) fn prepare_add_to_gitignore(
        &mut self,
        repo_id: RepoId,
        area: DiffArea,
        path: &std::path::PathBuf,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let target = if self.gitignore_explorer {
            self.explorer_gitignore_target(repo_id, path)
        } else {
            self.add_to_gitignore_target(repo_id, area, path, cx)
        };
        let scope = gitcomet_core::gitignore::GitignoreScope::File;
        let text = target
            .as_ref()
            .map(|(_, suggestions)| suggestions.lines_for(scope).join("\n"))
            .unwrap_or_default();
        let (paths, suggestions) = match target {
            Some((paths, suggestions)) => (paths, Some(suggestions)),
            None => (vec![path.clone()], None),
        };

        self.gitignore_paths = paths;
        self.gitignore_suggestions = suggestions;
        self.gitignore_scope = scope;

        self.gitignore_patterns_input.update(cx, |input, cx| {
            input.clear_transient_key_presses();
            input.set_text(&text, cx);
            cx.notify();
        });
        // `set_text` resets only the horizontal offset, so a dialog reopened
        // after scrolling a long selection would show blank space where the
        // patterns are.
        self.gitignore_patterns_scroll
            .set_offset(gpui::point(px(0.0), px(0.0)));
        let focus = self
            .gitignore_patterns_input
            .read_with(cx, |i, _| i.focus_handle());
        window.focus(&focus, cx);
    }

    /// Re-seed the pattern field after the user picks a different scope.
    ///
    /// This overwrites whatever is in the field. That is the point: the user
    /// just asked for a different pattern, and merging the old text into the
    /// new scope would leave a field matching neither.
    pub(super) fn set_add_to_gitignore_scope(
        &mut self,
        scope: gitcomet_core::gitignore::GitignoreScope,
        cx: &mut gpui::Context<Self>,
    ) {
        let Some(text) = self
            .gitignore_suggestions
            .as_ref()
            .map(|s| s.lines_for(scope).join("\n"))
        else {
            return;
        };
        self.gitignore_scope = scope;
        self.gitignore_patterns_input.update(cx, |input, cx| {
            input.set_text(&text, cx);
            cx.notify();
        });
        // As in `prepare_add_to_gitignore`: the new text is usually shorter than
        // what it replaced, so a stale vertical offset would scroll it off.
        self.gitignore_patterns_scroll
            .set_offset(gpui::point(px(0.0), px(0.0)));
        cx.notify();
    }

    /// The non-blank lines currently in the pattern field.
    pub(super) fn add_to_gitignore_patterns(&self, cx: &gpui::App) -> Vec<String> {
        self.gitignore_patterns_input.read_with(cx, |input, _| {
            input
                .text()
                .lines()
                .filter_map(gitignore_pattern_line)
                .map(ToOwned::to_owned)
                .collect()
        })
    }

    /// Whether the pattern field holds anything submittable.
    ///
    /// Separate from [`Self::add_to_gitignore_patterns`] because this runs every
    /// frame the dialog is on screen, and building the whole `Vec<String>` just
    /// to ask whether it is empty allocates one `String` per selected file per
    /// frame.
    pub(super) fn can_submit_add_to_gitignore(&self, cx: &gpui::App) -> bool {
        self.gitignore_patterns_input.read_with(cx, |input, _| {
            input
                .text()
                .lines()
                .any(|line| gitignore_pattern_line(line).is_some())
        })
    }

    pub(super) fn submit_add_to_gitignore(
        &mut self,
        repo_id: RepoId,
        area: DiffArea,
        path: std::path::PathBuf,
        cx: &mut gpui::Context<Self>,
    ) {
        let patterns = self.add_to_gitignore_patterns(cx);
        if patterns.is_empty() {
            return;
        }
        // Now that the action is going ahead, the row selection has served its
        // purpose and is cleared. The returned paths are unused — the patterns
        // come from the field, which the user may have edited.
        if !self.gitignore_explorer {
            let _ = self.take_status_paths_for_action(repo_id, area, &path, cx);
        }
        self.store
            .dispatch(Msg::AppendGitignorePatterns { repo_id, patterns });
        self.close_popover(cx);
    }

    /// A Stage, Unstage or Discard from a menu or dialog, sent through the
    /// diff so it can open the next file once the open one is left empty.
    pub(super) fn dispatch_open_file_change(&self, msg: Msg, cx: &mut gpui::Context<Self>) {
        self.diff_renderer
            .update(cx, |pane, cx| pane.dispatch_open_file_change(msg, cx));
    }

    pub(super) fn discard_worktree_changes_confirmed(
        &mut self,
        repo_id: RepoId,
        area: DiffArea,
        path: Option<std::path::PathBuf>,
        cx: &mut gpui::Context<Self>,
    ) {
        let (paths, _used_selection) = match path.as_ref() {
            Some(clicked_path) => {
                let selection = self.details_pane.update(cx, |pane, cx| {
                    let selection = pane
                        .status_multi_selection
                        .get(&repo_id)
                        .map(|sel| sel.selected_paths_for_area(area))
                        .unwrap_or(&[]);

                    let use_selection =
                        selection.len() > 1 && selection.iter().any(|p| p == clicked_path);
                    if !use_selection {
                        return None;
                    }

                    let sel = pane.status_multi_selection.remove(&repo_id)?;
                    cx.notify();
                    Some(sel.take_selected_paths_for_area(area))
                });

                match selection {
                    Some(paths) if !paths.is_empty() => (paths, true),
                    _ => (vec![clicked_path.clone()], false),
                }
            }
            None => {
                let paths = self
                    .details_pane
                    .update(cx, |pane, cx| {
                        let sel = pane.status_multi_selection.remove(&repo_id)?;
                        cx.notify();
                        Some(sel.take_selected_paths_for_area(area))
                    })
                    .unwrap_or_default();
                if paths.is_empty() {
                    return;
                }
                (paths, true)
            }
        };

        if paths.len() > 1 {
            self.dispatch_open_file_change(Msg::DiscardWorktreeChangesPaths { repo_id, paths }, cx);
            return;
        }

        let Some(path) = paths.into_iter().next() else {
            return;
        };

        self.dispatch_open_file_change(Msg::DiscardWorktreeChangesPath { repo_id, path }, cx);
    }

    pub(super) fn build_unified_patch_for_hunk_src_ix(
        &self,
        repo_id: RepoId,
        hunk_src_ix: usize,
        cx: &gpui::App,
    ) -> Option<gitcomet_state::msg::ContentBytes> {
        let pane = self.diff_renderer.read(cx);
        // Payload row numbers do not address hunks in the Git pointer patch.
        if pane.active_repo_id() != Some(repo_id)
            || pane.is_inline_submodule_diff_active()
            || pane.has_large_file_text_diff()
        {
            return None;
        }
        let Some(Loadable::Ready(diff)) = pane.rendered_patch_diff_loadable() else {
            return None;
        };
        crate::view::diff_utils::build_unified_patch_for_hunk(diff.lines.as_slice(), hunk_src_ix)
            .map(Into::into)
    }

    /// Scrolls the least that fully shows the selected row, counting the area
    /// under a visible scroll arrow as hidden. Reads the extents recorded in the
    /// last prepaint; navigation does not change the rows, so they still hold.
    fn scroll_context_menu_selection(&self, cx: &mut gpui::Context<Self>) {
        let Some((top, height)) = self
            .context_menu_selected_ix
            .and_then(|ix| self.context_menu_row_extents.borrow().get(ix).copied())
            .flatten()
        else {
            return;
        };
        let scroll = &self.context_menu_scroll;
        let offset = scroll.offset();
        let current = -offset.y;
        let edge = super::popover_ui_scale(cx).px(components::MENU_SCROLL_ARROW_HEIGHT_PX);
        let target = crate::kit::menu_placement::reveal_scroll(
            top,
            top + height,
            scroll.bounds().size.height,
            scroll.max_offset().y,
            current,
            edge,
        );
        if target != current {
            scroll.set_offset(point(offset.x, -target));
        }
    }

    pub(super) fn context_menu_view(
        &mut self,
        kind: PopoverKind,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::Div {
        let theme = self.theme;
        let ui_scale = super::popover_ui_scale(cx);
        let width = super::popover_width_spec(&kind).unwrap_or(super::DEFAULT_CONTEXT_MENU_WIDTH);
        let model = self
            .context_menu_model(&kind, cx)
            .unwrap_or_else(|| ContextMenuModel::new(vec![]));
        self.context_menu_row_extents
            .borrow_mut()
            .resize(model.items.len(), None);
        let row_extents = self.context_menu_row_extents.clone();
        let menu_scroll = self.context_menu_scroll.clone();
        let model_for_keys = model.clone();
        let model_for_mouse = model.clone();
        let tooltip_host = self.tooltip_host.clone();
        let entry_tooltips = model.entry_tooltips.clone();
        let entry_debug_selectors = model.entry_debug_selectors.clone();
        let entry_source_labels = model.entry_source_labels.clone();
        let slots = context_menu_slots(model.items.len(), &model.groups);
        let shortcut_keycaps = model.shortcut_keycaps;

        let focus = self.context_menu_focus_handle.clone();
        // No fallback highlight: the menu opens with nothing selected (like
        // native menus), and hovering a disabled entry parks the selection on
        // it, which renders as no highlight at all rather than jumping to the
        // first selectable row.
        let current_selected = self.context_menu_selected_ix;
        let selected_for_render = current_selected.filter(|&ix| model.is_selectable(ix));

        // Keep labels aligned across entries when only some of them (e.g. the
        // checked option) carry an icon; icon-less menus stay compact.
        let reserve_icon_column = model
            .items
            .iter()
            .any(|item| matches!(item, ContextMenuItem::Entry { icon: Some(_), .. }));

        div()
            .flex()
            .flex_col()
            .items_stretch()
            .text_color(theme.colors.foreground.primary)
            .min_w(width.min_px(ui_scale))
            .max_w(width.max_px(ui_scale))
            .track_focus(&focus)
            .key_context("ContextMenu")
            .when(
                matches!(
                    &kind,
                    PopoverKind::CommitMenu {
                        refs: CommitMenuRefs::Chooser,
                        ..
                    }
                ),
                |menu| {
                    menu.capture_any_mouse_down(cx.listener(
                        |this, event: &gpui::MouseDownEvent, _, _| {
                            // Stop layout changes before the entry's bubble
                            // handler claims the second press.
                            if event.button == MouseButton::Left && event.click_count > 1 {
                                this.history_ref_expansion_task = None;
                            }
                        },
                    ))
                },
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, window, cx| {
                    // Entries activate on mouse-*up*, so without this the press
                    // would collapse the very selection a Copy entry reads.
                    crate::text_selection_owner::preserve(cx);
                    window.focus(&this.context_menu_focus_handle, cx);
                }),
            )
            .on_key_down(
                cx.listener(move |this, e: &gpui::KeyDownEvent, window, cx| {
                    this.latch_context_menu_placement();
                    this.first_history_ref_click = None;
                    this.history_ref_expansion_task = None;
                    let key = e.keystroke.key.as_str();
                    let mods = e.keystroke.modifiers;
                    if mods.control || mods.platform || mods.alt || mods.function {
                        return;
                    }

                    match key {
                        "escape" => {
                            cx.stop_propagation();
                            this.close_popover_and_restore_focus(window, cx);
                        }
                        "right" => {
                            if let Some(ix) = this.context_menu_selected_ix
                                && let Some(ContextMenuAction::ToggleHistoryRefGroup { target }) =
                                    context_menu_entry_action_at(&model_for_keys, ix)
                                && this.expanded_history_ref.as_ref() != Some(&target)
                            {
                                cx.stop_propagation();
                                this.context_menu_activate_action(
                                    ContextMenuAction::ToggleHistoryRefGroup { target },
                                    window,
                                    cx,
                                );
                            }
                        }
                        "left" => {
                            if let Some(target) = this.expanded_history_ref.clone() {
                                cx.stop_propagation();
                                this.context_menu_activate_action(
                                    ContextMenuAction::ToggleHistoryRefGroup { target },
                                    window,
                                    cx,
                                );
                            }
                        }
                        "up" => {
                            cx.stop_propagation();
                            let next =
                                model_for_keys.next_selectable(this.context_menu_selected_ix, -1);
                            this.context_menu_selected_ix = next;
                            this.scroll_context_menu_selection(cx);
                            cx.notify();
                        }
                        "down" => {
                            cx.stop_propagation();
                            let next =
                                model_for_keys.next_selectable(this.context_menu_selected_ix, 1);
                            this.context_menu_selected_ix = next;
                            this.scroll_context_menu_selection(cx);
                            cx.notify();
                        }
                        "tab" => {
                            cx.stop_propagation();
                            let direction = if mods.shift { -1 } else { 1 };
                            this.context_menu_selected_ix = model_for_keys
                                .next_selectable(this.context_menu_selected_ix, direction);
                            this.scroll_context_menu_selection(cx);
                            cx.notify();
                        }
                        "home" => {
                            cx.stop_propagation();
                            this.context_menu_selected_ix = model_for_keys.first_selectable();
                            this.scroll_context_menu_selection(cx);
                            cx.notify();
                        }
                        "end" => {
                            cx.stop_propagation();
                            this.context_menu_selected_ix = model_for_keys.last_selectable();
                            this.scroll_context_menu_selection(cx);
                            cx.notify();
                        }
                        "enter" | "space" => {
                            let Some(ix) = context_menu_activate_entry_ix(
                                &model_for_keys,
                                this.context_menu_selected_ix,
                            ) else {
                                return;
                            };
                            cx.stop_propagation();
                            this.context_menu_activate_model_entry(&model_for_keys, ix, window, cx);
                        }
                        _ => {
                            if let Some(ix) = context_menu_shortcut_entry_ix(&model_for_keys, key) {
                                cx.stop_propagation();
                                this.context_menu_activate_model_entry(
                                    &model_for_keys,
                                    ix,
                                    window,
                                    cx,
                                );
                            }
                        }
                    }
                }),
            )
            .on_children_prepainted({
                let slots = slots.clone();
                record_row_extents(row_extents.clone(), menu_scroll.clone(), move |child| {
                    match slots.get(child) {
                        Some(MenuSlot::Row(ix)) => Some(*ix),
                        _ => None,
                    }
                })
            })
            .children(group_context_menu_rows(
                model
                    .items
                    .into_iter()
                    .enumerate()
                    .map(move |(ix, item)| match item {
                        ContextMenuItem::Separator => {
                            components::context_menu_separator(theme, ui_scale)
                                .id(("context_menu_sep", ix))
                                .into_any_element()
                        }
                        ContextMenuItem::Header(title) => components::context_menu_header(
                            theme,
                            ui_scale,
                            title,
                            Some(tooltip_host.clone()),
                            cx,
                        )
                        .id(("context_menu_header", ix))
                        .into_any_element(),
                        ContextMenuItem::Description(text) => components::context_menu_description(
                            theme,
                            ui_scale,
                            text,
                            Some(tooltip_host.clone()),
                            cx,
                        )
                        .id(("context_menu_description", ix))
                        .into_any_element(),
                        ContextMenuItem::Label(text) => components::context_menu_label(
                            theme,
                            ui_scale,
                            text,
                            Some(tooltip_host.clone()),
                            cx,
                        )
                        .id(("context_menu_label", ix))
                        .into_any_element(),
                        ContextMenuItem::Segmented { label, segments } => {
                            // The kit's segmented control, as the diff toolbar's
                            // Inline/Split uses.
                            let mut control = components::SegmentedControl::new(
                                SharedString::from(format!("context_menu_segmented_{ix}")),
                            );
                            let mut actions = Vec::with_capacity(segments.len());
                            for segment in segments {
                                let ContextMenuSegment {
                                    id,
                                    label,
                                    tooltip,
                                    selected,
                                    action,
                                } = segment;
                                actions.push(action);
                                let mut part =
                                    components::Segment::new(id, label).selected(selected);
                                if let Some(tooltip) = tooltip {
                                    part = part.tooltip(tooltip, Vec::new());
                                }
                                control = control.segment(part);
                            }
                            let control = control.render(
                                theme,
                                ui_scale,
                                cx,
                                move |this, index, window, cx| {
                                    if let Some(action) = actions.get(index) {
                                        this.context_menu_activate_action(
                                            action.clone(),
                                            window,
                                            cx,
                                        );
                                    }
                                },
                            );
                            components::context_menu_label(
                                theme,
                                ui_scale,
                                label,
                                Some(tooltip_host.clone()),
                                cx,
                            )
                            .id(("context_menu_segmented_row", ix))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(control)
                            .into_any_element()
                        }
                        ContextMenuItem::Entry {
                            label,
                            icon,
                            shortcut,
                            disabled,
                            action,
                        } => {
                            let selected = selected_for_render == Some(ix);
                            let source_label = entry_source_labels.get(&ix).unwrap_or(&label);
                            let debug_selector = entry_debug_selectors
                                .get(&ix)
                                .map(|selector| selector.to_string())
                                .unwrap_or_else(|| {
                                    context_menu_entry_debug_selector(source_label.as_ref())
                                });
                            let tooltip_text = entry_tooltips
                                .get(&ix)
                                .cloned()
                                .or_else(|| context_menu_entry_tooltip(action.as_ref()));
                            let tooltip_host_for_move = tooltip_host.clone();
                            let tooltip_text_for_move = tooltip_text.clone();
                            let tooltip_host_for_hover = tooltip_host.clone();
                            let activate_on_release = model_for_mouse.clone();
                            let (icon_slot, tone) =
                                crate::view::menu_presentation::entry_presentation(
                                    source_label.as_ref(),
                                    icon,
                                    reserve_icon_column,
                                );
                            let row = components::ContextMenuEntry::new(
                                ("context_menu_entry", ix),
                                label,
                            )
                            .icon(icon_slot)
                            .tone(tone)
                            .shortcut(shortcut)
                            .shortcut_keycaps(shortcut_keycaps)
                            .selected(selected)
                            .disabled(disabled)
                            .tooltip_host(tooltip_host.clone())
                            .render(theme, ui_scale, cx)
                            .debug_selector(move || debug_selector.clone());

                            row.on_mouse_move(cx.listener(
                                move |this, event: &MouseMoveEvent, _w, cx| {
                                    this.context_menu_selected_ix = Some(ix);
                                    if let Some(tooltip_text) = tooltip_text_for_move.as_ref() {
                                        let _ = tooltip_host_for_move.update(cx, |host, cx| {
                                            host.on_mouse_moved(event.position, cx);
                                            host.set_tooltip_text_if_changed(
                                                Some(tooltip_text.clone()),
                                                cx,
                                            );
                                        });
                                    }
                                    cx.notify();
                                },
                            ))
                            .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                                if *hovering {
                                    this.context_menu_selected_ix = Some(ix);
                                    cx.notify();
                                } else if let Some(tooltip_text) = tooltip_text.as_ref() {
                                    let _ = tooltip_host_for_hover.update(cx, |host, cx| {
                                        host.clear_tooltip_if_matches(tooltip_text, cx);
                                    });
                                }
                            }))
                            .on_menu_activate(
                                disabled,
                                cx.listener(move |this, event: &ClickEvent, window, cx| {
                                    if let Some(action) =
                                        context_menu_entry_action_at(&activate_on_release, ix)
                                        && !this
                                            .handle_history_ref_click(&action, event, window, cx)
                                    {
                                        this.context_menu_activate_action(action, window, cx);
                                    }
                                }),
                            )
                            .into_any_element()
                        }
                    }),
                &slots,
                theme,
                ui_scale,
                &row_extents,
                &menu_scroll,
            ))
    }
}

/// Content-space `(top, height)` of each menu row from the last prepaint.
pub(super) type RowExtents = Rc<std::cell::RefCell<Vec<Option<(Pixels, Pixels)>>>>;

/// A direct child of the menu column: one row, or one group block of rows.
#[derive(Clone, Debug, PartialEq)]
enum MenuSlot {
    Row(usize),
    Group(std::ops::Range<usize>),
}

/// How `len` rows lay out once each model group is wrapped in one block. A
/// group that runs past the last row stays unwrapped.
fn context_menu_slots(len: usize, groups: &[std::ops::Range<usize>]) -> Vec<MenuSlot> {
    let mut slots = Vec::new();
    let mut groups = groups.iter().filter(|range| !range.is_empty()).peekable();
    let mut block_start = None;
    for ix in 0..len {
        let Some(range) = groups.peek().filter(|range| range.start <= ix) else {
            slots.push(MenuSlot::Row(ix));
            continue;
        };
        let start = *block_start.get_or_insert(ix);
        if ix + 1 == range.end {
            slots.push(MenuSlot::Group(start..ix + 1));
            block_start = None;
            groups.next();
        }
    }
    if let Some(start) = block_start {
        slots.extend((start..len).map(MenuSlot::Row));
    }
    slots
}

/// Records the content-space extents of a column's children; `row_of` maps a
/// child index to the menu row it is, if it is one.
pub(super) fn record_row_extents(
    extents: RowExtents,
    scroll: ScrollHandle,
    row_of: impl Fn(usize) -> Option<usize> + 'static,
) -> impl Fn(Vec<Bounds<Pixels>>, &mut Window, &mut gpui::App) + 'static {
    move |bounds, _window, _cx| {
        let content_top = scroll.bounds().top() + scroll.offset().y;
        let mut extents = extents.borrow_mut();
        for (child, bounds) in bounds.iter().enumerate() {
            if let Some(slot) = row_of(child).and_then(|ix| extents.get_mut(ix)) {
                *slot = Some((bounds.top() - content_top, bounds.size.height));
            }
        }
    }
}

/// Wrap each model group's rows in one tinted block; other rows pass through.
fn group_context_menu_rows(
    rows: impl Iterator<Item = AnyElement>,
    slots: &[MenuSlot],
    theme: AppTheme,
    ui_scale: crate::ui_scale::UiScale,
    extents: &RowExtents,
    scroll: &ScrollHandle,
) -> Vec<AnyElement> {
    let mut rows: Vec<Option<AnyElement>> = rows.map(Some).collect();
    let mut take = |ix: usize| rows[ix].take().expect("each menu row is placed once");
    let mut previous_end = None;
    let mut out = Vec::with_capacity(slots.len());
    for slot in slots {
        match slot {
            MenuSlot::Row(ix) => out.push(take(*ix)),
            MenuSlot::Group(range) => {
                let start = range.start;
                // Neighbouring blocks would merge into one tint without a gap.
                let spaced = previous_end == Some(start);
                previous_end = Some(range.end);
                out.push(
                    components::context_menu_group(theme, ui_scale, spaced)
                        .on_children_prepainted(record_row_extents(
                            extents.clone(),
                            scroll.clone(),
                            move |child| Some(start + child),
                        ))
                        .id(("context_menu_group", start))
                        .debug_selector(move || format!("context_menu_group_{start}"))
                        .children(range.clone().map(&mut take))
                        .into_any_element(),
                );
            }
        }
    }
    out
}

/// The stash menu for a stash's node in the history graph; `None` for any
/// other commit, or for a menu narrowed to one of the row's refs.
fn stash_menu_for_graph_node(
    host: &PopoverHost,
    repo_id: RepoId,
    commit_id: &CommitId,
    refs: &CommitMenuRefs,
    language: Language,
) -> Option<ContextMenuModel> {
    if !matches!(refs, CommitMenuRefs::All) {
        return None;
    }
    let stash = host
        .state
        .repos
        .iter()
        .find(|repo| repo.id == repo_id)?
        .stashes
        .ready()?
        .iter()
        .find(|stash| stash.id == *commit_id)?;
    Some(stash::model(repo_id, stash.index, &stash.message, language))
}

fn interactive_rebase_action_menu_model(
    ix: usize,
    can_squash: bool,
    can_drop: bool,
    pick_locked: bool,
) -> ContextMenuModel {
    let mut items = vec![
        ContextMenuItem::Entry {
            label: "pick".into(),
            icon: None,
            shortcut: Some("P".into()),
            // A squash run's target is auto-managed: it stays Reword while
            // commits squash into it, and a dropped entry in target position
            // would re-promote the instant it were picked back — so `pick`
            // locks for the position rather than turning into a surprise
            // reword. Demote a target by dropping it or removing the squash.
            disabled: pick_locked,
            action: Box::new(ContextMenuAction::SetInteractiveRebaseAction {
                ix,
                action: InteractiveRebaseAction::Pick,
            }),
        },
        ContextMenuItem::Entry {
            label: "reword".into(),
            icon: None,
            shortcut: Some("R".into()),
            disabled: false,
            action: Box::new(ContextMenuAction::SetInteractiveRebaseAction {
                ix,
                action: InteractiveRebaseAction::Reword,
            }),
        },
        ContextMenuItem::Entry {
            label: "drop".into(),
            icon: None,
            shortcut: Some("D".into()),
            disabled: !can_drop,
            action: Box::new(ContextMenuAction::SetInteractiveRebaseAction {
                ix,
                action: InteractiveRebaseAction::Drop,
            }),
        },
    ];
    if can_squash {
        items.push(ContextMenuItem::Entry {
            label: "squash".into(),
            icon: None,
            shortcut: Some("S".into()),
            disabled: !can_squash,
            action: Box::new(ContextMenuAction::SetInteractiveRebaseAction {
                ix,
                action: InteractiveRebaseAction::Squash,
            }),
        });
    }
    ContextMenuModel::new(items)
}

fn interactive_rebase_autosquash_menu_model() -> ContextMenuModel {
    // Auto Squash is a one-shot action: pick a strategy and it folds the
    // duplicate-message commits, no persisted on/off state to display.
    let entry = |mode: AutosquashMode| ContextMenuItem::Entry {
        label: mode.label().into(),
        icon: None,
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::SetInteractiveRebaseAutosquashMode { mode }),
    };
    ContextMenuModel::new(vec![
        ContextMenuItem::Header("Auto Squash".into()),
        entry(AutosquashMode::ToTop),
        entry(AutosquashMode::Neighbor),
        entry(AutosquashMode::ToBottom),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::kit::components::secondary_shortcut;

    fn entry_labels(items: &[ContextMenuItem]) -> Vec<String> {
        items
            .iter()
            .filter_map(|item| match item {
                ContextMenuItem::Entry { label, .. } => Some(label.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_marked_commit_offers_only_to_clear_its_mark() {
        let marked = CommitId("c00".into());
        let entries = marked_comparison_entries(
            RepoId(1),
            &marked,
            "c00",
            Some((&marked, "main")),
            Language::English,
        );
        assert_eq!(entry_labels(&entries), ["Clear comparison mark"]);
    }

    #[test]
    fn another_commit_compares_against_the_mark() {
        let marked = CommitId("c00".into());
        let entries = marked_comparison_entries(
            RepoId(1),
            &CommitId("c01".into()),
            "c01",
            Some((&marked, "main")),
            Language::English,
        );
        assert_eq!(
            entry_labels(&entries),
            [
                "Compare with main",
                "Compare with merge base",
                "Clear comparison mark"
            ]
        );
        assert!(
            marked_comparison_entries(RepoId(1), &marked, "c00", None, Language::English)
                .is_empty()
        );
    }

    #[gpui::test]
    fn traditional_chinese_menu_keeps_warning_tone_and_debug_selector(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(|app| {
            app.set_global(Language::TraditionalChinese);
            let model = localized_menu(app, |language| stash::model(RepoId(1), 0, "WIP", language));
            let (index, label) = model
                .items
                .iter()
                .enumerate()
                .find_map(|(index, item)| match item {
                    ContextMenuItem::Entry { label, action, .. }
                        if matches!(
                            action.as_ref(),
                            ContextMenuAction::DropStashConfirm { .. }
                        ) =>
                    {
                        Some((index, label))
                    }
                    _ => None,
                })
                .unwrap();
            let source = model.entry_source_labels.get(&index).unwrap();
            assert_eq!(label.as_ref(), "捨棄貯藏…");
            assert_eq!(
                context_menu_entry_debug_selector(source.as_ref()),
                "context_menu_drop_stash"
            );
            assert_eq!(
                crate::view::menu_presentation::context_menu_tone(source.as_ref(), None),
                crate::kit::components::ContextMenuTone::Danger
            );
        });
    }

    #[test]
    fn context_menu_shortcut_entry_ix_matches_first_enabled_single_character_entry() {
        let model = ContextMenuModel::new(vec![
            ContextMenuItem::Header("Test".into()),
            ContextMenuItem::Entry {
                label: "Disabled A".into(),
                icon: None,
                shortcut: Some("A".into()),
                disabled: true,
                action: Box::new(ContextMenuAction::FetchAll { repo_id: RepoId(1) }),
            },
            ContextMenuItem::Entry {
                label: "Enter".into(),
                icon: None,
                shortcut: Some("Enter".into()),
                disabled: false,
                action: Box::new(ContextMenuAction::FetchAll { repo_id: RepoId(2) }),
            },
            ContextMenuItem::Entry {
                label: "Ctrl Copy".into(),
                icon: None,
                shortcut: Some(secondary_shortcut("C").into()),
                disabled: false,
                action: Box::new(ContextMenuAction::FetchAll { repo_id: RepoId(3) }),
            },
            ContextMenuItem::Entry {
                label: "Enabled A".into(),
                icon: None,
                shortcut: Some("A".into()),
                disabled: false,
                action: Box::new(ContextMenuAction::FetchAll { repo_id: RepoId(4) }),
            },
        ]);

        assert_eq!(context_menu_shortcut_entry_ix(&model, "a"), Some(4));
        assert_eq!(context_menu_shortcut_entry_ix(&model, "A"), Some(4));
        assert_eq!(context_menu_shortcut_entry_ix(&model, "c"), Some(3));
        assert_eq!(context_menu_shortcut_entry_ix(&model, "e"), None);
        assert_eq!(context_menu_shortcut_entry_ix(&model, "enter"), None);
    }

    #[test]
    fn context_menu_activate_entry_ix_prefers_selected_entry_and_falls_back_to_first_selectable() {
        let model = ContextMenuModel::new(vec![
            ContextMenuItem::Header("Test".into()),
            ContextMenuItem::Entry {
                label: "Disabled".into(),
                icon: None,
                shortcut: Some("D".into()),
                disabled: true,
                action: Box::new(ContextMenuAction::FetchAll { repo_id: RepoId(1) }),
            },
            ContextMenuItem::Entry {
                label: "First".into(),
                icon: None,
                shortcut: Some("Enter".into()),
                disabled: false,
                action: Box::new(ContextMenuAction::FetchAll { repo_id: RepoId(2) }),
            },
            ContextMenuItem::Entry {
                label: "Second".into(),
                icon: None,
                shortcut: Some("S".into()),
                disabled: false,
                action: Box::new(ContextMenuAction::FetchAll { repo_id: RepoId(3) }),
            },
        ]);

        assert_eq!(context_menu_activate_entry_ix(&model, None), Some(2));
        assert_eq!(context_menu_activate_entry_ix(&model, Some(3)), Some(3));
        assert_eq!(context_menu_activate_entry_ix(&model, Some(1)), Some(2));
        assert_eq!(context_menu_activate_entry_ix(&model, Some(99)), Some(2));
    }

    #[test]
    fn use_commit_message_action_exposes_full_message_tooltip() {
        let tooltip = context_menu_entry_tooltip(&ContextMenuAction::UseCommitMessage {
            message: "\n\nsubject\n\nbody".to_string(),
        });

        assert_eq!(
            tooltip.as_ref().map(|text| text.as_ref()),
            Some("subject\n\nbody")
        );
    }

    #[test]
    fn non_commit_message_actions_do_not_expose_entry_tooltips() {
        assert!(
            context_menu_entry_tooltip(&ContextMenuAction::FetchAll { repo_id: RepoId(1) })
                .is_none()
        );
    }
}
