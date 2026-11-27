use super::*;
use crate::localization::{Language, text};

/// Asks before the diff's selected lines are discarded: unlike staging, a
/// discard rewrites the working-tree file, and Git Undo cannot restore it.
pub(super) fn panel(
    this: &mut PopoverHost,
    repo_id: RepoId,
    scope: DiscardScope,
    patch: Option<gitcomet_state::msg::ContentBytes>,
    cx: &mut gpui::Context<PopoverHost>,
) -> gpui::Div {
    let theme = this.theme;
    let language = Language::current(cx);
    let title = match scope {
        DiscardScope::Lines(_) => language.text("Discard lines", "捨棄選取的行"),
        DiscardScope::Hunks(1) => language.text("Discard hunk", "捨棄整段變更"),
        DiscardScope::Hunks(_) => language.text("Discard hunks", "捨棄多段變更"),
        DiscardScope::Block => language.text("Discard block", "捨棄整個區塊"),
    };
    let detail = match (scope, language) {
        (DiscardScope::Lines(Some(1)), Language::English) => {
            "This will discard the selected line from the working tree.".to_string()
        }
        (DiscardScope::Lines(Some(count)), Language::English) => {
            format!("This will discard {count} selected lines from the working tree.")
        }
        (DiscardScope::Lines(None), Language::English) => {
            "This will discard the selected changes from the working tree.".to_string()
        }
        (DiscardScope::Lines(Some(count)), Language::TraditionalChinese) => {
            format!("這會從工作樹捨棄選取的 {count} 行。")
        }
        (DiscardScope::Lines(None), Language::TraditionalChinese) => {
            "這會從工作樹捨棄選取的變更。".to_string()
        }
        (DiscardScope::Hunks(1), Language::English) => {
            "This will discard the whole hunk from the working tree, not only the selected lines."
                .to_string()
        }
        (DiscardScope::Hunks(count), Language::English) => format!(
            "This will discard {count} whole hunks from the working tree, not only the selected lines."
        ),
        (DiscardScope::Hunks(1), Language::TraditionalChinese) => {
            "這會從工作樹捨棄整段變更，不只選取的行。".to_string()
        }
        (DiscardScope::Hunks(count), Language::TraditionalChinese) => {
            format!("這會從工作樹捨棄 {count} 段變更，不只選取的行。")
        }
        (DiscardScope::Block, Language::English) => {
            "This will discard the whole block from the working tree, not only the selected lines."
                .to_string()
        }
        (DiscardScope::Block, Language::TraditionalChinese) => {
            "這會從工作樹捨棄整個區塊，不只選取的行。".to_string()
        }
    };
    ConfirmDialog::new(title, DIALOG_420_WIDTH)
        .text(theme, detail)
        .note(
            theme,
            text(
                cx,
                "Git Undo cannot restore discarded lines.",
                "捨棄的行無法用 Git Undo 復原。",
            ),
        )
        .render(
            theme,
            dialog_cancel_button(
                "discard_lines_cancel",
                "discard_lines_cancel_hint",
                theme,
                cx,
            ),
            components::Button::new("discard_lines_go", text(cx, "Discard", "捨棄"))
                .style(components::ButtonStyle::Danger)
                .on_click(theme, cx, move |this, _e, window, cx| {
                    let opened_on = this.discard_confirm_source.take();
                    // Read while the pane that opened the dialog is still the
                    // one bound: closing hands the host back to the main diff.
                    let current = patch
                        .as_ref()
                        .and_then(|_| this.diff_renderer.read(cx).worktree_source_fingerprint());
                    this.close_popover_and_restore_focus(window, cx);
                    match &patch {
                        Some(patch) => {
                            // Git applies a stale patch wherever its context
                            // matches, so a file changed since the dialog
                            // opened could lose other lines than were shown.
                            if opened_on.is_none() || opened_on != current {
                                this.push_toast(
                                    components::ToastKind::Error,
                                    gitcomet_core::patch_action::PatchActionError::Stale
                                        .to_string(),
                                    cx,
                                );
                                return;
                            }
                            this.context_menu_activate_action(
                                ContextMenuAction::ApplyWorktreePatch {
                                    repo_id,
                                    patch: patch.clone(),
                                    reverse: true,
                                },
                                window,
                                cx,
                            )
                        }
                        None => this.diff_renderer.update(cx, |pane, cx| {
                            pane.confirm_pending_structural_discard(cx);
                        }),
                    }
                }),
            cx,
        )
}
