use super::*;
use crate::localization::Language;

/// The reflog panel's right-click menu, targeting the commit the clicked
/// reflog entry points at rather than a history row.
///
/// "Create branch here" comes first: it rescues a commit that a reset or a
/// rebase left behind without touching the working tree, and it stays
/// undoable, so its prompt starts with Checkout off. The three reset actions
/// follow, the same ones the history log's commit context menu offers (see
/// `commit.rs`).
pub(super) fn model(
    repo_id: RepoId,
    selector: &SharedString,
    target: &CommitId,
    language: Language,
) -> ContextMenuModel {
    let sha = target.as_ref().to_string();
    let short: SharedString = sha.get(0..8).unwrap_or(&sha).to_string().into();

    let mut items = vec![
        ContextMenuItem::Header(components::ContextMenuText::new(selector.clone())),
        ContextMenuItem::Label(components::ContextMenuText::new(format!("commit {short}"))),
        ContextMenuItem::Separator,
        ContextMenuItem::Entry {
            label: language.text("Create branch here…", "在此建立分支…").into(),
            icon: Some("icons/git_branch.svg".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::OpenPopover {
                kind: PopoverKind::CreateBranchFromRefPrompt {
                    repo_id,
                    target: sha,
                    source_selectable: false,
                    name_prefix: String::new(),
                    checkout_on_open: false,
                },
            }),
        },
        ContextMenuItem::Separator,
    ];
    items.extend(super::commit::reset_entries(
        repo_id, target, false, language,
    ));

    ContextMenuModel::new(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn english_model() -> ContextMenuModel {
        let commit_id = CommitId("deadbeefcafe".into());
        let selector: SharedString = "HEAD@{2}".into();
        model(RepoId(1), &selector, &commit_id, Language::English)
    }

    #[test]
    fn model_offers_create_branch_before_the_three_reset_modes() {
        assert_eq!(
            english_model().entry_labels(),
            vec![
                "Create branch here…",
                "Reset (--soft) to here",
                "Reset (--mixed) to here",
                "Reset (--hard) to here",
            ]
        );
    }

    #[test]
    fn create_branch_targets_the_entrys_commit_with_checkout_off() {
        let model = english_model();
        let prompt = model.items.iter().find_map(|item| match item {
            ContextMenuItem::Entry { action, .. } => match action.as_ref() {
                ContextMenuAction::OpenPopover {
                    kind: kind @ PopoverKind::CreateBranchFromRefPrompt { .. },
                } => Some(kind.clone()),
                _ => None,
            },
            _ => None,
        });
        assert_eq!(
            prompt,
            Some(PopoverKind::CreateBranchFromRefPrompt {
                repo_id: RepoId(1),
                target: "deadbeefcafe".into(),
                source_selectable: false,
                name_prefix: String::new(),
                checkout_on_open: false,
            })
        );
    }

    #[test]
    fn each_reset_targets_the_reflog_entrys_commit() {
        let model = english_model();
        let modes: Vec<ResetMode> = model
            .items
            .iter()
            .filter_map(|item| match item {
                ContextMenuItem::Entry { action, .. } => match action.as_ref() {
                    ContextMenuAction::OpenPopover {
                        kind:
                            PopoverKind::ResetPrompt {
                                repo_id,
                                target,
                                mode,
                            },
                    } => {
                        assert_eq!(*repo_id, RepoId(1));
                        assert_eq!(target, "deadbeefcafe");
                        Some(*mode)
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(
            modes,
            vec![ResetMode::Soft, ResetMode::Mixed, ResetMode::Hard]
        );
    }
}
