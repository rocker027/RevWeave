//! Recording undoable ref moves and running Undo / Redo.
//!
//! Commits record from their completion, which carries the moved ref. Branch
//! creation and deletion and soft resets prepare their entry from the state
//! when they start; for a deletion or soft reset the worker then reports the
//! value it actually moves from, and the entry is recorded when the
//! completion reports success. Any other operation that can move a branch or
//! HEAD clears the journal.
use super::util::push_diagnostic;
use super::{ReducerInputs, begin_local_action};
use crate::model::{
    AppState, DiagnosticKind, GitRefChange, GitUndoBlock, GitUndoDirection, GitUndoEntry, Loadable,
    PendingGitUndoKind, RepoId, RepoState, is_full_commit_id,
};
use crate::msg::{Effect, RepoActionKind, RepoCommandKind};
use gitcomet_core::domain::CommitId;
use gitcomet_core::services::{CommitOperationOutcome, ResetMode};

fn repo_mut(state: &mut AppState, repo_id: RepoId) -> Option<&mut RepoState> {
    state.repos.iter_mut().find(|repo| repo.id == repo_id)
}

fn local_branch_ref(name: &str) -> String {
    format!("refs/heads/{name}")
}

/// The checked-out local branch, if HEAD is on one.
fn head_branch(repo: &RepoState) -> Option<&str> {
    match &repo.head_branch {
        Loadable::Ready(name) if !name.is_empty() && name != "HEAD" => Some(name.as_str()),
        _ => None,
    }
}

/// The commit `target` names, from what the state has loaded: a full id,
/// HEAD, or a local branch, remote branch or tag name.
fn resolve_commit(repo: &RepoState, target: &str) -> Option<CommitId> {
    if is_full_commit_id(target) {
        return Some(CommitId(target.into()));
    }
    if target == "HEAD" {
        return repo.head_commit_id();
    }
    if let Some(branch) = repo
        .branches
        .ready()
        .and_then(|branches| branches.iter().find(|branch| branch.name == target))
    {
        return Some(branch.target.clone());
    }
    if let Some(branch) = repo.remote_branches.ready().and_then(|branches| {
        branches
            .iter()
            .find(|branch| format!("{}/{}", branch.remote, branch.name) == target)
    }) {
        return Some(branch.target.clone());
    }
    repo.tags
        .ready()
        .and_then(|tags| tags.iter().find(|tag| tag.name == target))
        .map(|tag| tag.target.clone())
}

/// `git branch <name> <target>` is starting.
pub(super) fn prepare_create_branch(
    state: &mut AppState,
    repo_id: RepoId,
    name: &str,
    target: &str,
) {
    let Some(repo) = repo_mut(state, repo_id) else {
        return;
    };
    let entry = resolve_commit(repo, target).map(|tip| GitUndoEntry {
        label: format!("Create branch {name}"),
        changes: vec![GitRefChange {
            reference: local_branch_ref(name),
            before: None,
            after: Some(tip),
        }],
        requires_head: false,
    });
    let busy = repo.git_undo_busy();
    repo.git_undo
        .begin_pending(PendingGitUndoKind::CreateBranch, entry, busy);
}

/// `git branch -d` (or `-D` when `force`) is starting.
pub(super) fn prepare_delete_branch(
    state: &mut AppState,
    repo_id: RepoId,
    name: &str,
    force: bool,
) {
    let Some(repo) = repo_mut(state, repo_id) else {
        return;
    };
    let tip = repo
        .branches
        .ready()
        .and_then(|branches| branches.iter().find(|branch| branch.name == name))
        .map(|branch| branch.target.clone());
    let entry = tip.map(|tip| GitUndoEntry {
        label: format!("Delete branch {name}"),
        changes: vec![GitRefChange {
            reference: local_branch_ref(name),
            before: Some(tip),
            after: None,
        }],
        requires_head: false,
    });
    let kind = if force {
        PendingGitUndoKind::ForceDeleteBranch
    } else {
        PendingGitUndoKind::DeleteBranch
    };
    let busy = repo.git_undo_busy();
    repo.git_undo.begin_pending(kind, entry, busy);
}

/// `git reset` is starting. Only a soft reset of a checked-out branch to a
/// known commit is undoable: it moves the branch and nothing else.
pub(super) fn prepare_reset(state: &mut AppState, repo_id: RepoId, target: &str, mode: ResetMode) {
    if mode != ResetMode::Soft {
        return;
    }
    let Some(repo) = repo_mut(state, repo_id) else {
        return;
    };
    let entry = head_branch(repo)
        .map(str::to_owned)
        .zip(repo.head_commit_id())
        .filter(|_| is_full_commit_id(target))
        .map(|(branch, head)| GitUndoEntry {
            label: format!("Reset {branch}"),
            changes: vec![GitRefChange {
                reference: local_branch_ref(&branch),
                before: Some(head),
                after: Some(CommitId(target.into())),
            }],
            requires_head: true,
        });
    let busy = repo.git_undo_busy();
    repo.git_undo
        .begin_pending(PendingGitUndoKind::SoftReset, entry, busy);
}

/// A commit (or amend) finished; its outcome names the moved branch.
pub(super) fn commit_finished(
    state: &mut AppState,
    repo_id: RepoId,
    amend: bool,
    outcome: &CommitOperationOutcome,
) {
    let Some(repo) = repo_mut(state, repo_id) else {
        return;
    };
    let (Some(branch), Some(before), Some(after)) = (
        outcome.local_branch.as_deref(),
        outcome.pre_head.clone(),
        outcome.post_head.clone(),
    ) else {
        return;
    };
    if before == after {
        return;
    }
    let verb = if amend { "Amend commit" } else { "Commit" };
    repo.git_undo.record(GitUndoEntry {
        label: format!("{verb} on {branch}"),
        changes: vec![GitRefChange {
            reference: local_branch_ref(branch),
            before: Some(before),
            after: Some(after),
        }],
        requires_head: true,
    });
}

/// The worker read the ref a pending deletion or soft reset moves from.
pub(super) fn ref_observed(
    state: &mut AppState,
    repo_id: RepoId,
    kind: PendingGitUndoKind,
    reference: &str,
    value: Option<CommitId>,
) {
    if let Some(repo) = repo_mut(state, repo_id) {
        repo.git_undo.observe_pending(kind, reference, value);
    }
}

/// A repository action finished: settles a pending entry or an undo/redo.
/// Any other action that can move a branch or HEAD clears the journal.
pub(super) fn repo_action_finished(
    state: &mut AppState,
    repo_id: RepoId,
    action: RepoActionKind,
    succeeded: bool,
) {
    let Some(repo) = repo_mut(state, repo_id) else {
        return;
    };
    let journal = &mut repo.git_undo;
    match action {
        RepoActionKind::CreateBranch => {
            journal.finish_pending(PendingGitUndoKind::CreateBranch, succeeded)
        }
        RepoActionKind::DeleteBranch => {
            journal.finish_pending(PendingGitUndoKind::DeleteBranch, succeeded)
        }
        RepoActionKind::ForceDeleteBranch => {
            journal.finish_pending(PendingGitUndoKind::ForceDeleteBranch, succeeded)
        }
        RepoActionKind::UndoGitOperation | RepoActionKind::RedoGitOperation => {
            let direction = if action == RepoActionKind::UndoGitOperation {
                GitUndoDirection::Undo
            } else {
                GitUndoDirection::Redo
            };
            journal.finish(direction, succeeded);
            // The branch a lease was taken for has moved.
            if succeeded {
                repo.pending.force_push_lease = None;
            }
        }
        RepoActionKind::CheckoutBranch
        | RepoActionKind::CheckoutRemoteBranch
        | RepoActionKind::CheckoutCommit
        | RepoActionKind::Bisect
        | RepoActionKind::CherryPickCommit
        | RepoActionKind::CreateBranchAndCheckout
        | RepoActionKind::RenameBranch
        | RepoActionKind::DeleteBranches => journal.clear_history(),
        RepoActionKind::StagePath
        | RepoActionKind::StagePaths
        | RepoActionKind::UnstagePath
        | RepoActionKind::UnstagePaths
        | RepoActionKind::DiscardWorktreeChangesPath
        | RepoActionKind::DiscardWorktreeChangesPaths
        | RepoActionKind::Stash
        | RepoActionKind::ApplyStash
        | RepoActionKind::PopStash
        | RepoActionKind::DropStash => {}
    }
}

/// Commands that never move a local branch or HEAD; any other command clears
/// the journal when it finishes, whatever its result, since even a failed
/// one can leave refs moved.
fn command_keeps_git_undo(command: &RepoCommandKind) -> bool {
    matches!(
        command,
        RepoCommandKind::FetchAll
            | RepoCommandKind::FetchRemote { .. }
            | RepoCommandKind::FetchRefspecs { .. }
            | RepoCommandKind::PruneLocalTags
            | RepoCommandKind::RunMaintenance
            | RepoCommandKind::Push
            | RepoCommandKind::PushBranch { .. }
            | RepoCommandKind::PushWithTags { .. }
            | RepoCommandKind::PushAfterCommit { .. }
            | RepoCommandKind::ForcePush
            | RepoCommandKind::ForcePushWithLease { .. }
            | RepoCommandKind::PushSetUpstream { .. }
            | RepoCommandKind::SetUpstreamBranch { .. }
            | RepoCommandKind::UnsetUpstreamBranch { .. }
            | RepoCommandKind::DeleteRemoteBranch { .. }
            | RepoCommandKind::DeleteRemoteBranches { .. }
            | RepoCommandKind::CreateTag { .. }
            | RepoCommandKind::DeleteTag { .. }
            | RepoCommandKind::PushTag { .. }
            | RepoCommandKind::DeleteRemoteTag { .. }
            | RepoCommandKind::AddRemote { .. }
            | RepoCommandKind::RemoveRemote { .. }
            | RepoCommandKind::SetRemoteUrl { .. }
            | RepoCommandKind::CheckoutConflict { .. }
            | RepoCommandKind::AcceptConflictDeletion { .. }
            | RepoCommandKind::CheckoutConflictBase { .. }
            | RepoCommandKind::LaunchMergetool { .. }
            | RepoCommandKind::SaveWorktreeFile { .. }
            | RepoCommandKind::AppendGitignorePatterns { .. }
            | RepoCommandKind::AppendGitattributesRule { .. }
            | RepoCommandKind::ExportPatch { .. }
            | RepoCommandKind::ApplyPatch { .. }
            | RepoCommandKind::StageHunk
            | RepoCommandKind::UnstageHunk
            | RepoCommandKind::ApplyWorktreePatch { .. }
    )
}

/// A repository command finished: a soft reset settles its pending entry,
/// and a command that may have moved a branch or HEAD clears the journal.
pub(super) fn repo_command_finished(
    state: &mut AppState,
    repo_id: RepoId,
    command: &RepoCommandKind,
    succeeded: bool,
) {
    let Some(repo) = repo_mut(state, repo_id) else {
        return;
    };
    if let RepoCommandKind::Reset {
        mode: ResetMode::Soft,
        ..
    } = command
    {
        repo.git_undo
            .finish_pending(PendingGitUndoKind::SoftReset, succeeded);
    } else if !command_keeps_git_undo(command) {
        repo.git_undo.clear_history();
    }
}

/// Undo or Redo was asked for: runs the next entry's ref transaction when
/// nothing else is running and the branches it touches allow it. A refusal
/// the user can act on is shown; having nothing to do, or waiting on other
/// work, is what the disabled controls already say.
pub(super) fn request(
    inputs: &ReducerInputs,
    state: &mut AppState,
    repo_id: RepoId,
    direction: GitUndoDirection,
) -> Vec<Effect> {
    let Some(repo) = repo_mut(state, repo_id) else {
        return Vec::new();
    };
    match repo.git_undo_block(direction) {
        None => {}
        Some(GitUndoBlock::Nothing(_) | GitUndoBlock::Busy) => return Vec::new(),
        Some(block) => {
            if let Some(entry) = repo.git_undo.next(direction) {
                let message = block.message(entry, direction);
                push_diagnostic(inputs, repo, DiagnosticKind::Error, message);
            }
            return Vec::new();
        }
    }
    let Some((updates, checks)) = repo.git_undo.begin(direction) else {
        return Vec::new();
    };
    begin_local_action(state, repo_id);
    vec![Effect::ApplyGitUndo {
        repo_id,
        direction,
        updates,
        checks,
    }]
}
