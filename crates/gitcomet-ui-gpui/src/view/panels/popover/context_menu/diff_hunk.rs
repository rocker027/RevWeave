use super::*;
use crate::localization::Language;

use crate::kit::components::secondary_shortcut;

fn diff_hunk_primary_metadata(
    diff_target: Option<&DiffTarget>,
    language: Language,
) -> (bool, &'static str, &'static str, Option<String>) {
    match diff_target {
        Some(DiffTarget::WorkingTree { area, .. }) => match area {
            DiffArea::Unstaged => (
                false,
                language.text("Stage hunk", "暫存這段變更"),
                "icons/plus.svg",
                Some(secondary_shortcut("S")),
            ),
            DiffArea::Staged => (
                false,
                language.text("Unstage hunk", "取消暫存這段變更"),
                "icons/minus.svg",
                Some(secondary_shortcut("U")),
            ),
        },
        _ => (
            true,
            language.text("Stage/Unstage hunk", "暫存或取消暫存這段變更"),
            "icons/plus.svg",
            None,
        ),
    }
}

fn diff_hunk_primary_action(
    repo_id: RepoId,
    src_ix: usize,
    diff_target: Option<&DiffTarget>,
) -> ContextMenuAction {
    match diff_target {
        Some(DiffTarget::WorkingTree {
            area: DiffArea::Staged,
            ..
        }) => ContextMenuAction::UnstageHunk { repo_id, src_ix },
        _ => ContextMenuAction::StageHunk { repo_id, src_ix },
    }
}

pub(super) fn model(
    this: &PopoverHost,
    repo_id: RepoId,
    src_ix: usize,
    cx: &gpui::App,
    language: Language,
) -> ContextMenuModel {
    let mut items = vec![ContextMenuItem::Header(
        language.text("Hunk", "變更區塊").into(),
    )];
    items.push(ContextMenuItem::Separator);

    let pane = this.diff_renderer.read(cx);
    let diff_target = pane.rendered_diff_target();
    let (disabled, label, icon, shortcut) = diff_hunk_primary_metadata(diff_target, language);
    let patch = this.build_unified_patch_for_hunk_src_ix(repo_id, src_ix, cx);

    items.push(ContextMenuItem::Entry {
        label: label.into(),
        icon: Some(icon.into()),
        shortcut: shortcut.map(Into::into),
        disabled: disabled || patch.is_none(),
        action: Box::new(diff_hunk_primary_action(repo_id, src_ix, diff_target)),
    });

    let is_unstaged = diff_target.is_some_and(|target| {
        matches!(
            target,
            DiffTarget::WorkingTree {
                area: DiffArea::Unstaged,
                ..
            }
        )
    });

    // Not beside Unstage hunk: the two share Ctrl/Cmd+U.
    let is_staged = matches!(
        diff_target,
        Some(DiffTarget::WorkingTree {
            area: DiffArea::Staged,
            ..
        })
    );
    if !is_staged {
        items.push(ContextMenuItem::Entry {
            label: language.text("Discard hunk", "捨棄這段變更").into(),
            icon: Some("icons/refresh.svg".into()),
            shortcut: Some(secondary_shortcut("U").into()),
            disabled: !is_unstaged || patch.is_none(),
            action: Box::new(ContextMenuAction::OpenPopover {
                kind: PopoverKind::DiscardLinesConfirm {
                    repo_id,
                    lines: None,
                    patch,
                },
            }),
        });
    }

    ContextMenuModel::new(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unstaged_target_uses_stage_shortcut_and_action() {
        let target =
            DiffTarget::working_tree(std::path::PathBuf::from("src/lib.rs"), DiffArea::Unstaged);

        let (disabled, label, icon, shortcut) =
            diff_hunk_primary_metadata(Some(&target), Language::English);
        assert!(!disabled);
        assert_eq!(label, "Stage hunk");
        assert_eq!(icon, "icons/plus.svg");
        assert_eq!(shortcut, Some(secondary_shortcut("S")));
        assert!(matches!(
            diff_hunk_primary_action(RepoId(9), 4, Some(&target)),
            ContextMenuAction::StageHunk {
                repo_id,
                src_ix: 4
            } if repo_id == RepoId(9)
        ));
    }

    #[test]
    fn staged_target_uses_unstage_shortcut_and_action() {
        let target =
            DiffTarget::working_tree(std::path::PathBuf::from("src/lib.rs"), DiffArea::Staged);

        let (disabled, label, icon, shortcut) =
            diff_hunk_primary_metadata(Some(&target), Language::English);
        assert!(!disabled);
        assert_eq!(label, "Unstage hunk");
        assert_eq!(icon, "icons/minus.svg");
        assert_eq!(shortcut, Some(secondary_shortcut("U")));
        assert!(matches!(
            diff_hunk_primary_action(RepoId(10), 5, Some(&target)),
            ContextMenuAction::UnstageHunk {
                repo_id,
                src_ix: 5
            } if repo_id == RepoId(10)
        ));
    }

    #[test]
    fn traditional_chinese_hunk_labels_keep_shortcuts_and_actions() {
        for (area, expected_label, key) in [
            (DiffArea::Unstaged, "暫存這段變更", "S"),
            (DiffArea::Staged, "取消暫存這段變更", "U"),
        ] {
            let target = DiffTarget::working_tree("src/lib.rs".into(), area);
            let (disabled, label, _, shortcut) =
                diff_hunk_primary_metadata(Some(&target), Language::TraditionalChinese);
            assert!(!disabled);
            assert_eq!(label, expected_label);
            assert_eq!(shortcut, Some(secondary_shortcut(key)));
            let action = diff_hunk_primary_action(RepoId(9), 4, Some(&target));
            assert!(match (area, action) {
                (DiffArea::Unstaged, ContextMenuAction::StageHunk { repo_id, src_ix })
                | (DiffArea::Staged, ContextMenuAction::UnstageHunk { repo_id, src_ix }) => {
                    repo_id == RepoId(9) && src_ix == 4
                }
                _ => false,
            });
        }
    }
}
