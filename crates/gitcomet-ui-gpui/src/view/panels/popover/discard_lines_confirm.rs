use super::*;
use crate::localization::{Language, text};

/// Asks before the diff's selected lines are discarded: unlike staging, a
/// discard rewrites the working-tree file, and Git Undo cannot restore it.
pub(super) fn panel(
    this: &mut PopoverHost,
    repo_id: RepoId,
    lines: Option<usize>,
    patch: Option<gitcomet_state::msg::ContentBytes>,
    cx: &mut gpui::Context<PopoverHost>,
) -> gpui::Div {
    let theme = this.theme;
    let detail = match (lines, Language::current(cx)) {
        (Some(1), Language::English) => {
            "This will discard the selected line from the working tree.".to_string()
        }
        (Some(count), Language::English) => {
            format!("This will discard {count} selected lines from the working tree.")
        }
        (None, Language::English) => {
            "This will discard the selected changes from the working tree.".to_string()
        }
        (Some(count), Language::TraditionalChinese) => {
            format!("這會從工作樹捨棄選取的 {count} 行。")
        }
        (None, Language::TraditionalChinese) => "這會從工作樹捨棄選取的變更。".to_string(),
    };
    ConfirmDialog::new(text(cx, "Discard lines", "捨棄選取的行"), DIALOG_420_WIDTH)
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
                    this.close_popover_and_restore_focus(window, cx);
                    match &patch {
                        // Built when the dialog opened: if the file changed
                        // since, it no longer applies and discards nothing.
                        Some(patch) => this.context_menu_activate_action(
                            ContextMenuAction::ApplyWorktreePatch {
                                repo_id,
                                patch: patch.clone(),
                                reverse: true,
                            },
                            window,
                            cx,
                        ),
                        None => this.diff_renderer.update(cx, |pane, cx| {
                            pane.confirm_pending_structural_discard(cx);
                        }),
                    }
                }),
            cx,
        )
}
