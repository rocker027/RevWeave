//! "Create fixup commit" in the history's commit menu: commits what is staged
//! as `fixup! <subject>`, so a later autosquash folds it into that commit.
//! It only adds a commit on the checked-out branch, which Git Undo can take
//! back; folding it into the target is left to the interactive rebase
//! editor's autosquash.

use super::*;
use crate::localization::Language;
use gitcomet_core::squash::FirstParentPosition;

/// The message of a fixup for the commit titled `summary`, in Git's autosquash
/// form: both `git rebase --autosquash` and the interactive rebase editor's
/// autosquash group it with its target by subject.
pub(super) fn fixup_message(summary: &str) -> String {
    format!("fixup! {}", summary.trim())
}

/// Why a fixup for `commit_id` cannot be created right now, worded for a
/// tooltip; `None` when it can.
///
/// The target's place in HEAD's first-parent history is prepared for HEAD and
/// the selected commit only, so other rows (`judged` false) skip that check,
/// as the commit menu's "Edit commit message" does.
pub(super) fn fixup_blocker(
    repo: &RepoState,
    commit_id: &CommitId,
    summary: Option<&str>,
    judged: bool,
) -> Option<&'static str> {
    if repo.is_bare() {
        return Some(gitcomet_core::domain::WORKING_TREE_REQUIRED_MESSAGE);
    }
    if repo.history_rewrite_busy() || repo.worktree_pull_in_flight > 0 {
        return Some("Finish the running Git operation to create a fixup commit");
    }
    // While these reload, a merge or rebase may still be waiting to be
    // concluded, and a plain commit would conclude it.
    let operations_known = repo.merge_commit_message.ready().is_some()
        && repo.sequencer_state.ready().is_some()
        && repo.rebase_in_progress.ready().is_some();
    if !operations_known {
        return Some("Loading the repository state…");
    }
    // Git Undo only records commits on a branch.
    if !matches!(&repo.head_branch, Loadable::Ready(head) if !head.is_empty() && head != "HEAD") {
        return Some("Check out a branch to create a fixup commit");
    }
    match repo.staged_status_entries() {
        None => return Some("Loading the working tree status…"),
        Some([]) => return Some("Stage the changes to fold into this commit first"),
        Some(_) => {}
    }
    if summary.is_none_or(|summary| summary.trim().is_empty()) {
        return Some("This commit's subject is not loaded yet");
    }
    (judged && !on_first_parent_history(repo, commit_id)).then_some(NOT_ON_HEAD_HISTORY)
}

/// Whether the target sits on HEAD's first-parent line, merges below HEAD
/// included: a fixup is a new commit, so unlike a reword it needs no linear
/// history to replay.
fn on_first_parent_history(repo: &RepoState, commit_id: &CommitId) -> bool {
    matches!(
        repo.first_parent_position_from_head(commit_id),
        FirstParentPosition::Linear { .. } | FirstParentPosition::BelowMerge
    )
}

const NOT_ON_HEAD_HISTORY: &str =
    "Only commits in the current branch's history can get a fixup commit";

/// Rechecks the target when the entry runs. The menu can show the entry for a
/// row whose place in HEAD's history was not known yet (a sidebar tag, or a
/// row whose selection had not landed); by now it is, and an unknown place
/// counts as off HEAD's history.
pub(in crate::view) fn fixup_target_blocker(
    repo: &RepoState,
    commit_id: &CommitId,
) -> Option<&'static str> {
    (!on_first_parent_history(repo, commit_id)).then_some(NOT_ON_HEAD_HISTORY)
}

/// The menu entry, disabled while [`fixup_blocker`] names a reason.
pub(super) fn entry(
    repo_id: RepoId,
    commit_id: &CommitId,
    summary: Option<&str>,
    disabled: bool,
    language: Language,
) -> ContextMenuItem {
    ContextMenuItem::Entry {
        label: language
            .text("Create fixup commit", "建立 fixup 提交")
            .into(),
        icon: Some("icons/git_commit.svg".into()),
        shortcut: None,
        disabled,
        action: Box::new(ContextMenuAction::CreateFixupCommit {
            repo_id,
            commit_id: commit_id.clone(),
            message: fixup_message(summary.unwrap_or_default()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixup_message_uses_the_autosquash_prefix_and_trims_the_subject() {
        assert_eq!(fixup_message("  Add login  "), "fixup! Add login");
        assert_eq!(fixup_message("fixup! Add login"), "fixup! fixup! Add login");
    }
}
