//! The commit card at the top of the commit details: author, dates and the
//! short SHA on one row, and the parent with a `+N` chooser for the refs on
//! this commit beside it.

use super::*;
use crate::localization::text;
use crate::theme::with_alpha;
use gitcomet_core::domain::CommitDetails;
use std::cell::Cell;
use std::rc::Rc;

const CHIP_HEIGHT_PX: f32 = 22.0;
const CHIP_BORDER_PX: f32 = 1.0;
const CHIP_DIVIDER_PX: f32 = 1.0;
const CHIP_CHEVRON_PAD_X: gpui::Rems = gpui::rems(0.25);
const CHIP_CHEVRON_ICON_PX: f32 = 12.0;

/// The card's two SHA chips. A click copies the commit's own SHA but reveals
/// the parent; both chevrons open the same commit menu.
#[derive(Clone, Copy)]
enum ShaChip {
    /// Washed when the history find query matched the SHA.
    Commit {
        find_matched: bool,
    },
    Parent,
}

impl ShaChip {
    fn id(self) -> &'static str {
        match self {
            Self::Commit { .. } => "commit_card_sha",
            Self::Parent => "commit_card_parent",
        }
    }

    fn menu_id(self) -> &'static str {
        match self {
            Self::Commit { .. } => "commit_card_sha_menu",
            Self::Parent => "commit_card_parent_menu",
        }
    }

    fn tooltip(self, cx: &App) -> &'static str {
        match self {
            Self::Commit { .. } => text(cx, "Copy commit SHA", "複製提交的 SHA"),
            Self::Parent => text(cx, "Go to parent commit", "前往父提交"),
        }
    }

    fn icon(self) -> Option<&'static str> {
        match self {
            Self::Commit { .. } => Some("icons/copy.svg"),
            Self::Parent => None,
        }
    }

    fn washed(self) -> bool {
        matches!(self, Self::Commit { find_matched: true })
    }
}

/// What a chip's chevron opens, anchored under the whole chip. `invoker`
/// also names the chevron element.
struct ChipMenu {
    invoker: SharedString,
    kind: PopoverKind,
    tooltip: &'static str,
    enabled: bool,
}

impl DetailsPaneView {
    /// How many branches (local and remote, hidden remotes included) and
    /// tags point at `commit`: what the details chooser lists.
    fn commit_ref_count(&self, commit: &CommitId) -> usize {
        let Some(repo) = self.active_repo() else {
            return 0;
        };
        let local = match &repo.branches {
            Loadable::Ready(branches) => branches.iter().filter(|b| &b.target == commit).count(),
            _ => 0,
        };
        let remote = match &repo.remote_branches {
            Loadable::Ready(branches) => branches.iter().filter(|b| &b.target == commit).count(),
            _ => 0,
        };
        let tags = match &repo.tags {
            Loadable::Ready(tags) => tags.iter().filter(|t| &t.target == commit).count(),
            _ => 0,
        };
        local + remote + tags
    }

    /// A short SHA whose body runs the chip's click and whose chevron opens
    /// the commit menu under the whole chip. While the card still shows the
    /// previous selection (`!current`) only copying stays live.
    fn commit_card_sha_chip(
        &self,
        chip: ShaChip,
        repo_id: RepoId,
        commit_id: CommitId,
        current: bool,
        cx: &mut gpui::Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let body_enabled = current || matches!(chip, ShaChip::Commit { .. });
        let ui_scale = self.ui_scale();
        let menu_id = chip.menu_id();

        let body = div()
            .id(chip.id())
            .debug_selector(move || chip.id().to_string())
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .h_full()
            .font_family(crate::view::UI_MONOSPACE_FONT_FAMILY)
            .text_size(theme.ui_text(12.0))
            .control_interaction(
                components::InteractionStyle::new(theme),
                components::InteractionState::default().disabled(!body_enabled),
            )
            .child(crate::view::caches::short_commit_sha(commit_id.as_ref()).to_owned())
            .when_some(chip.icon(), |d, icon| {
                d.child(svg_icon(
                    icon,
                    theme.colors.foreground.secondary,
                    ui_scale.px(12.0),
                ))
            })
            .on_activate(
                !body_enabled,
                components::ControlActivation::Action,
                cx.listener({
                    let commit_id = commit_id.clone();
                    move |this, e: &ClickEvent, _w, cx| {
                        if !e.standard_click() {
                            return;
                        }
                        match chip {
                            ShaChip::Commit { .. } => this.copy_commit_sha(commit_id.clone(), cx),
                            ShaChip::Parent => {
                                this.reveal_commit_in_history(repo_id, commit_id.clone(), cx)
                            }
                        }
                    }
                }),
            )
            .gitcomet_tooltip(theme, chip.tooltip(cx).into());

        let menu = ChipMenu {
            invoker: menu_id.into(),
            kind: PopoverKind::CommitShaLinkMenu {
                repo_id,
                commit_id: commit_id.clone(),
            },
            tooltip: "Commit actions",
            enabled: current,
        };
        let bg = if chip.washed() {
            with_alpha(theme.colors.accent.foreground, 0.18)
        } else {
            theme.colors.surface.raised
        };
        self.commit_card_chip(body, bg, menu, cx)
            .when(chip.washed(), |d| {
                d.debug_selector(|| "commit_card_sha_find_wash".to_string())
            })
    }

    /// A bordered chip: `body`, a divider and a chevron that opens `menu`
    /// under the whole chip.
    fn commit_card_chip(
        &self,
        body: impl IntoElement,
        bg: gpui::Rgba,
        menu: ChipMenu,
        cx: &mut gpui::Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let ui_scale = self.ui_scale();
        let ChipMenu {
            invoker,
            kind,
            tooltip,
            enabled,
        } = menu;
        let open = self
            .active_context_menu_invoker
            .as_ref()
            .is_some_and(|active| active == &invoker);
        let chip_bounds: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::default();
        let chevron = div()
            .id(invoker.clone())
            .debug_selector({
                let invoker = invoker.clone();
                move || invoker.to_string()
            })
            .flex()
            .items_center()
            .justify_center()
            .px(CHIP_CHEVRON_PAD_X)
            .h_full()
            .control_interaction(
                components::InteractionStyle::new(theme),
                components::InteractionState::default()
                    .open(open)
                    .disabled(!enabled),
            )
            .child(svg_icon(
                "icons/chevron_down.svg",
                if open {
                    theme.colors.accent.foreground
                } else {
                    theme.colors.foreground.secondary
                },
                ui_scale.px(CHIP_CHEVRON_ICON_PX),
            ))
            .on_activate(
                !enabled,
                components::ControlActivation::Action,
                cx.listener({
                    let chip_bounds = Rc::clone(&chip_bounds);
                    move |this, e: &ClickEvent, window, cx| {
                        let Some(bounds) = chip_bounds.get().filter(|_| e.standard_click()) else {
                            return;
                        };
                        this.open_popover_for_bounds(
                            kind.clone().invoked_by(invoker.clone()),
                            bounds,
                            window,
                            cx,
                        );
                    }
                }),
            )
            .gitcomet_tooltip(theme, tooltip.into());

        div()
            .flex_none()
            .on_children_prepainted(move |children, _window, _cx| {
                chip_bounds.set(children.first().copied());
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .min_w(px(0.0))
                    .max_w_full()
                    .h(ui_scale.px(CHIP_HEIGHT_PX))
                    .rounded(px(theme.radii.control))
                    .border(px(CHIP_BORDER_PX))
                    .border_color(theme.colors.stroke.default)
                    .bg(bg)
                    .overflow_hidden()
                    .text_color(theme.colors.foreground.primary)
                    .child(body)
                    .child(
                        div()
                            .flex_none()
                            .w(px(CHIP_DIVIDER_PX))
                            .h_full()
                            .bg(theme.colors.stroke.default),
                    )
                    .child(chevron.flex_none()),
            )
    }

    /// The `+N` button listing every branch and tag on the commit, with
    /// each ref's actions one level down.
    fn commit_card_ref_overflow(
        &self,
        repo_id: RepoId,
        commit_id: CommitId,
        count: usize,
        current: bool,
        cx: &mut gpui::Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme;
        let invoker: SharedString = format!(
            "commit_card_ref_overflow_{}_{}",
            repo_id.0,
            commit_id.as_ref()
        )
        .into();
        let open = self.active_context_menu_invoker.as_ref() == Some(&invoker);
        components::Button::new(invoker.clone(), format!("+{count}"))
            .style(components::ButtonStyle::Solid)
            .end_slot(svg_icon(
                "icons/chevron_down.svg",
                theme.colors.foreground.secondary,
                self.ui_scale().px(CHIP_CHEVRON_ICON_PX),
            ))
            .open(open)
            .disabled(!current)
            .on_click_with_bounds(theme, cx, move |this, event, bounds, window, cx| {
                if !event.standard_click()
                    || !this.active_repo().is_some_and(|repo| {
                        repo.id == repo_id
                            && repo.history_state.selected_commit.as_ref() == Some(&commit_id)
                    })
                {
                    return;
                }
                this.open_popover_for_bounds(
                    PopoverKind::CommitMenu {
                        repo_id,
                        commit_id: commit_id.clone(),
                        refs: CommitMenuRefs::DetailsChooser,
                    }
                    .invoked_by(invoker.clone()),
                    bounds,
                    window,
                    cx,
                );
            })
            .flex_none()
            .debug_selector(|| "commit_card_ref_overflow".to_string())
            .gitcomet_tooltip(
                theme,
                text(cx, "Show all branches and tags", "顯示所有分支與標籤").into(),
            )
    }

    pub(super) fn commit_details_card(
        &mut self,
        details: &CommitDetails,
        signature: Option<&gitcomet_core::domain::CommitSignature>,
        name_matches: &[std::ops::Range<usize>],
        sha_matched: bool,
        repo_id: RepoId,
        current: bool,
        cx: &mut gpui::Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme;
        let ui_scale = self.ui_scale();
        let parent = details.parent_ids.first().cloned();
        let date = self.commit_details_date_display(details);
        let relative = (details.authored_at_unix != 0).then(|| {
            crate::view::date_time::format_relative_time(
                details.authored_at_unix,
                std::time::SystemTime::now(),
            )
        });
        let when = match (relative, date.is_empty()) {
            (Some(relative), false) => format!("{relative} · {date}"),
            (Some(relative), true) => relative,
            (None, _) => date,
        };

        let display_name = commit_details_author_display_name(details).to_owned();
        let wash = crate::view::rows::query_highlight_style(theme);
        let mut name_highlights: crate::view::commit_message_text::TextHighlights = name_matches
            .iter()
            .map(|range| (range.clone(), wash))
            .collect();
        crate::text_runs::sanitize_highlights(&display_name, &mut name_highlights);

        let sha_chip = self.commit_card_sha_chip(
            ShaChip::Commit {
                find_matched: sha_matched,
            },
            repo_id,
            details.id.clone(),
            current,
            cx,
        );

        // The author block asks for its natural width, so a narrow pane wraps
        // the badges onto the next line instead of truncating name and date.
        let author = div()
            .debug_selector(|| "commit_card_author".to_string())
            .flex()
            .flex_auto()
            .items_center()
            .gap_2()
            .min_w(px(0.0))
            .child(components::author_avatar(theme, ui_scale, &display_name))
            .child(
                div()
                    .flex()
                    .flex_auto()
                    .flex_col()
                    .min_w(px(0.0))
                    .child(
                        div()
                            .text_size(theme.ui_text(14.0))
                            .font_weight(FontWeight::MEDIUM)
                            .line_clamp(1)
                            .whitespace_nowrap()
                            .child(
                                gpui::StyledText::new(display_name)
                                    .with_highlights(name_highlights),
                            ),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_text(12.0))
                            .text_color(theme.colors.foreground.secondary)
                            .line_clamp(1)
                            .whitespace_nowrap()
                            .child(when),
                    ),
            );
        let badges = div()
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .when_some(signature, |row, signature| {
                row.child(commit_details_signature_badge(theme, signature))
            })
            .child(sha_chip);
        let top = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .w_full()
            .min_w(px(0.0))
            .child(author)
            .child(badges);

        let ref_count = self.commit_ref_count(&details.id);
        let mut bottom = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .pt_2()
            .border_t_1()
            .border_color(theme.colors.stroke.subtle)
            .child(
                div()
                    .text_size(theme.ui_text(12.0))
                    .text_color(theme.colors.foreground.secondary)
                    .child(text(cx, "Parent", "父提交")),
            );
        bottom = match parent {
            Some(parent) => bottom.child(self.commit_card_sha_chip(
                ShaChip::Parent,
                repo_id,
                parent,
                current,
                cx,
            )),
            None => bottom.child(
                div()
                    .text_size(theme.ui_text(12.0))
                    .text_color(theme.colors.foreground.secondary)
                    .child(text(cx, "none (root commit)", "無（根提交）")),
            ),
        };
        if ref_count > 0 {
            bottom = bottom.child(self.commit_card_ref_overflow(
                repo_id,
                details.id.clone(),
                ref_count,
                current,
                cx,
            ));
        }

        div()
            .id("commit_details_card")
            .debug_selector(|| "commit_details_card".to_string())
            .flex()
            .flex_col()
            .gap_2()
            .w_full()
            .min_w(px(0.0))
            .p_2()
            .border_1()
            .border_color(theme.colors.stroke.default)
            .rounded(px(theme.radii.panel))
            .child(top)
            .child(bottom)
    }
}
