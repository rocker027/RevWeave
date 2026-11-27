//! Undo and redo of this run's ref moves: recording, the transaction an undo
//! runs, and refusals that protect work elsewhere.
use super::*;
use crate::model::{GitUndoCheck, GitUndoDirection, GitUndoEntry, PendingGitUndoKind};
use gitcomet_core::domain::{Branch, Worktree};
use gitcomet_core::services::{CommitOperationOutcome, RefUpdate, ResetMode};

const A: &str = "1111111111111111111111111111111111111111";
const B: &str = "2222222222222222222222222222222222222222";
const C: &str = "3333333333333333333333333333333333333333";
const ZERO: &str = "0000000000000000000000000000000000000000";

fn id(value: &str) -> CommitId {
    CommitId(value.into())
}

fn setup() -> (RepositoryRuntimes, AtomicU64, AppState, RepoId) {
    let repo_id = RepoId(1);
    let mut state = AppState::test_default();
    let mut repo = RepoState::new_opening(
        repo_id,
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    );
    repo.head_branch = Loadable::Ready("main".to_string());
    repo.worktrees = Loadable::Ready(Arc::new(Vec::new()));
    repo.branches = Loadable::Ready(Arc::new(vec![
        Branch {
            name: "main".to_string(),
            target: id(B),
            upstream: None,
            divergence: None,
        },
        Branch {
            name: "topic".to_string(),
            target: id(A),
            upstream: None,
            divergence: None,
        },
    ]));
    state.repos.push(repo);
    state.active_repo = Some(repo_id);
    (Default::default(), AtomicU64::new(1), state, repo_id)
}

fn commit_finished(repos: &mut RepositoryRuntimes, ids: &AtomicU64, state: &mut AppState) {
    commit_moved(repos, ids, state, A, B);
}

fn commit_moved(
    repos: &mut RepositoryRuntimes,
    ids: &AtomicU64,
    state: &mut AppState,
    pre: &str,
    post: &str,
) {
    let _ = reduce(
        repos,
        ids,
        state,
        Msg::Internal(crate::msg::InternalMsg::CommitFinished {
            repo_id: RepoId(1),
            request: crate::model::CommitRequest {
                message: "ship".to_string(),
                amend: false,
                push_after_commit: false,
                hooks: gitcomet_core::services::GitHooks::Run,
                stage_paths: Vec::new(),
                fixup_target: None,
            },
            result: Ok(CommitOperationOutcome {
                local_branch: Some("main".to_string()),
                pre_head: Some(id(pre)),
                post_head: Some(id(post)),
            }),
        }),
    );
}

fn command_finished(
    repos: &mut RepositoryRuntimes,
    ids: &AtomicU64,
    state: &mut AppState,
    command: RepoCommandKind,
) {
    let _ = reduce(
        repos,
        ids,
        state,
        Msg::Internal(crate::msg::InternalMsg::RepoCommandFinished {
            repo_id: RepoId(1),
            command,
            result: Ok(CommandOutput::empty_success("git")),
        }),
    );
}

fn observed(
    repos: &mut RepositoryRuntimes,
    ids: &AtomicU64,
    state: &mut AppState,
    kind: PendingGitUndoKind,
    reference: &str,
    value: Option<&str>,
) {
    let _ = reduce(
        repos,
        ids,
        state,
        Msg::Internal(crate::msg::InternalMsg::GitUndoRefObserved {
            repo_id: RepoId(1),
            kind,
            reference: reference.to_string(),
            value: value.map(id),
        }),
    );
}

fn request(
    repos: &mut RepositoryRuntimes,
    ids: &AtomicU64,
    state: &mut AppState,
    direction: GitUndoDirection,
) -> Vec<Effect> {
    let repo_id = RepoId(1);
    let msg = match direction {
        GitUndoDirection::Undo => Msg::UndoGitOperation { repo_id },
        GitUndoDirection::Redo => Msg::RedoGitOperation { repo_id },
    };
    reduce(repos, ids, state, msg)
}

fn last_diagnostic(state: &AppState) -> Option<String> {
    state.repos[0]
        .feedback
        .diagnostics
        .last()
        .map(|entry| entry.message.clone())
}

fn check(branch: &str, must_be_head: bool, deletes: bool) -> GitUndoCheck {
    GitUndoCheck {
        branch: branch.to_string(),
        must_be_head,
        deletes,
    }
}

fn next_undo(state: &AppState) -> Option<GitUndoEntry> {
    state.repos[0].git_undo.next_undo().cloned()
}

fn action_finished(
    repos: &mut RepositoryRuntimes,
    ids: &AtomicU64,
    state: &mut AppState,
    action: RepoActionKind,
    ok: bool,
) {
    let _ = reduce(
        repos,
        ids,
        state,
        Msg::Internal(crate::msg::InternalMsg::RepoActionFinished {
            repo_id: RepoId(1),
            action,
            result: if ok {
                Ok(())
            } else {
                Err(Error::new(ErrorKind::Backend("refused".into())))
            },
        }),
    );
}

#[test]
fn undoing_a_commit_moves_the_branch_back_expecting_the_new_commit() {
    let (mut repos, ids, mut state, repo_id) = setup();
    commit_finished(&mut repos, &ids, &mut state);
    assert_eq!(
        next_undo(&state).map(|entry| entry.label),
        Some("Commit on main".into())
    );

    let effects = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::UndoGitOperation { repo_id },
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::ApplyGitUndo { repo_id: r, direction: GitUndoDirection::Undo, updates, checks }]
            if *r == repo_id
                && updates == &vec![RefUpdate {
                    reference: "refs/heads/main".into(),
                    expected: Some(id(B)),
                    new: id(A),
                }]
                && checks == &vec![check("main", true, false)]
    ));
    // One at a time: a second request while it runs does nothing.
    assert!(
        reduce(
            &mut repos,
            &ids,
            &mut state,
            Msg::UndoGitOperation { repo_id }
        )
        .is_empty()
    );

    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::UndoGitOperation,
        true,
    );
    assert!(next_undo(&state).is_none());
    assert!(state.repos[0].git_undo.next_redo().is_some());

    let redo = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::RedoGitOperation { repo_id },
    );
    assert!(matches!(
        redo.as_slice(),
        [Effect::ApplyGitUndo { direction: GitUndoDirection::Redo, updates, .. }]
            if updates[0].expected == Some(id(A)) && updates[0].new == id(B)
    ));
}

#[test]
fn a_failed_undo_keeps_the_entry() {
    let (mut repos, ids, mut state, repo_id) = setup();
    commit_finished(&mut repos, &ids, &mut state);
    let _ = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::UndoGitOperation { repo_id },
    );
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::UndoGitOperation,
        false,
    );
    assert!(next_undo(&state).is_some());
    assert!(!state.repos[0].git_undo.in_flight());
}

#[test]
fn deleted_branches_are_recorded_on_success_and_recreated_by_undo() {
    let (mut repos, ids, mut state, repo_id) = setup();
    let _ = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::DeleteBranch {
            repo_id,
            name: "topic".to_string(),
        },
    );
    assert!(
        next_undo(&state).is_none(),
        "recorded only once it succeeds"
    );
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::DeleteBranch,
        true,
    );
    let effects = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::UndoGitOperation { repo_id },
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::ApplyGitUndo { updates, .. }]
            if updates == &vec![RefUpdate {
                reference: "refs/heads/topic".into(),
                expected: None,
                new: id(A),
            }]
    ));
}

#[test]
fn created_branches_are_deleted_by_undo_unless_checked_out() {
    let (mut repos, ids, mut state, repo_id) = setup();
    let _ = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::CreateBranch {
            repo_id,
            name: "fresh".to_string(),
            target: "main".to_string(),
        },
    );
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::CreateBranch,
        true,
    );
    let effects = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::UndoGitOperation { repo_id },
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::ApplyGitUndo { updates, .. }]
            if updates == &vec![RefUpdate {
                reference: "refs/heads/fresh".into(),
                expected: Some(id(B)),
                new: id(ZERO),
            }]
    ));

    // Checked out since: deleting it is refused and reported.
    let (mut repos, ids, mut state, repo_id) = setup();
    let _ = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::CreateBranch {
            repo_id,
            name: "fresh".to_string(),
            target: "main".to_string(),
        },
    );
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::CreateBranch,
        true,
    );
    state.repos[0].head_branch = Loadable::Ready("fresh".to_string());
    assert!(
        reduce(
            &mut repos,
            &ids,
            &mut state,
            Msg::UndoGitOperation { repo_id }
        )
        .is_empty()
    );
    assert!(next_undo(&state).is_some(), "the refused entry stays");
}

/// A mixed or hard reset is not recorded, and since it moved the branch,
/// nothing before it can be undone either.
#[test]
fn mixed_and_hard_resets_are_not_undoable_and_clear_the_journal() {
    for mode in [ResetMode::Mixed, ResetMode::Hard] {
        let (mut repos, ids, mut state, repo_id) = setup();
        commit_finished(&mut repos, &ids, &mut state);
        let _ = reduce(
            &mut repos,
            &ids,
            &mut state,
            Msg::Reset {
                repo_id,
                target: A.to_string(),
                mode,
            },
        );
        command_finished(
            &mut repos,
            &ids,
            &mut state,
            RepoCommandKind::Reset {
                mode,
                target: A.to_string(),
            },
        );
        assert!(next_undo(&state).is_none(), "{mode:?}");
    }
}

#[test]
fn branches_checked_out_in_another_worktree_are_not_moved() {
    let (mut repos, ids, mut state, repo_id) = setup();
    commit_finished(&mut repos, &ids, &mut state);
    state.repos[0].worktrees = Loadable::Ready(Arc::new(vec![Worktree {
        bare: false,
        path: PathBuf::from("/tmp/repo-other"),
        head: None,
        branch: Some("main".to_string()),
        detached: false,
    }]));
    assert!(
        reduce(
            &mut repos,
            &ids,
            &mut state,
            Msg::UndoGitOperation { repo_id }
        )
        .is_empty()
    );
    assert!(!state.repos[0].git_undo.in_flight());
}

#[test]
fn overlapping_branch_deletions_are_not_recorded() {
    let (mut repos, ids, mut state, repo_id) = setup();
    for name in ["topic", "main"] {
        let _ = reduce(
            &mut repos,
            &ids,
            &mut state,
            Msg::DeleteBranch {
                repo_id,
                name: name.to_string(),
            },
        );
    }
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::DeleteBranch,
        true,
    );
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::DeleteBranch,
        true,
    );
    assert!(next_undo(&state).is_none());
}

#[test]
fn a_soft_reset_of_the_checked_out_branch_is_undoable() {
    let (mut repos, ids, mut state, repo_id) = setup();
    let head = state.repos[0].head_commit_id();
    assert_eq!(head, Some(id(B)), "HEAD is main's tip");
    let _ = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::Reset {
            repo_id,
            target: A.to_string(),
            mode: ResetMode::Soft,
        },
    );
    let _ = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::Internal(crate::msg::InternalMsg::RepoCommandFinished {
            repo_id,
            command: RepoCommandKind::Reset {
                mode: ResetMode::Soft,
                target: A.to_string(),
            },
            result: Ok(CommandOutput::empty_success("git reset")),
        }),
    );
    let entry = next_undo(&state).expect("the soft reset is recorded");
    assert_eq!(entry.label, "Reset main");
    assert_eq!(entry.changes[0].before, Some(id(B)));
    assert_eq!(entry.changes[0].after, Some(id(A)));
}

/// H1: undoing a commit moves its branch back and returns the changes to
/// the index, which only makes sense with that branch checked out here.
#[test]
fn undoing_a_commit_needs_its_branch_checked_out() {
    let (mut repos, ids, mut state, _) = setup();
    commit_finished(&mut repos, &ids, &mut state);
    state.repos[0].head_branch = Loadable::Ready("topic".to_string());
    assert!(request(&mut repos, &ids, &mut state, GitUndoDirection::Undo).is_empty());
    assert_eq!(
        last_diagnostic(&state).as_deref(),
        Some("Cannot undo \"Commit on main\": check out main first")
    );
    assert!(next_undo(&state).is_some(), "the refused entry stays");

    // Detached HEAD is not main either.
    state.repos[0].head_branch = Loadable::Ready("HEAD".to_string());
    assert!(request(&mut repos, &ids, &mut state, GitUndoDirection::Undo).is_empty());
}

/// H2: a deletion records the tip the worker read right before deleting,
/// not the one the state had loaded.
#[test]
fn a_deletion_records_the_tip_the_worker_read() {
    let (mut repos, ids, mut state, repo_id) = setup();
    let _ = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::DeleteBranch {
            repo_id,
            name: "topic".to_string(),
        },
    );
    observed(
        &mut repos,
        &ids,
        &mut state,
        PendingGitUndoKind::DeleteBranch,
        "refs/heads/topic",
        Some(C),
    );
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::DeleteBranch,
        true,
    );
    let entry = next_undo(&state).expect("recorded");
    assert_eq!(entry.changes[0].before, Some(id(C)), "not the loaded {A}");

    // A tip the worker could not read leaves nothing to recreate.
    let (mut repos, ids, mut state, repo_id) = setup();
    let _ = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::DeleteBranch {
            repo_id,
            name: "topic".to_string(),
        },
    );
    observed(
        &mut repos,
        &ids,
        &mut state,
        PendingGitUndoKind::DeleteBranch,
        "refs/heads/topic",
        None,
    );
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::DeleteBranch,
        true,
    );
    assert!(next_undo(&state).is_none());
}

/// H2: a soft reset records the commit the worker found checked out; one
/// that ran on another branch than planned is not recorded.
#[test]
fn a_soft_reset_records_what_the_worker_found_and_undoes_and_redoes() {
    let soft_reset = |observed_ref: &str, observed_value: &str| {
        let (mut repos, ids, mut state, repo_id) = setup();
        let _ = reduce(
            &mut repos,
            &ids,
            &mut state,
            Msg::Reset {
                repo_id,
                target: A.to_string(),
                mode: ResetMode::Soft,
            },
        );
        observed(
            &mut repos,
            &ids,
            &mut state,
            PendingGitUndoKind::SoftReset,
            observed_ref,
            Some(observed_value),
        );
        command_finished(
            &mut repos,
            &ids,
            &mut state,
            RepoCommandKind::Reset {
                mode: ResetMode::Soft,
                target: A.to_string(),
            },
        );
        (repos, ids, state)
    };

    let (_, _, state) = soft_reset("refs/heads/topic", C);
    assert!(next_undo(&state).is_none(), "ran on another branch");

    let (mut repos, ids, mut state) = soft_reset("refs/heads/main", C);
    let entry = next_undo(&state).expect("recorded");
    assert_eq!(entry.changes[0].before, Some(id(C)));
    let undo = request(&mut repos, &ids, &mut state, GitUndoDirection::Undo);
    assert!(matches!(
        undo.as_slice(),
        [Effect::ApplyGitUndo { updates, checks, .. }]
            if updates[0].expected == Some(id(A))
                && updates[0].new == id(C)
                && checks == &vec![check("main", true, false)]
    ));
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::UndoGitOperation,
        true,
    );
    let redo = request(&mut repos, &ids, &mut state, GitUndoDirection::Redo);
    assert!(matches!(
        redo.as_slice(),
        [Effect::ApplyGitUndo { updates, .. }]
            if updates[0].expected == Some(id(C)) && updates[0].new == id(A)
    ));
}

/// Redo of an undone deletion deletes the branch again, expecting the tip
/// undo restored, and must not delete the checked-out branch.
#[test]
fn redoing_a_deletion_deletes_the_branch_again() {
    let (mut repos, ids, mut state, repo_id) = setup();
    let _ = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::DeleteBranch {
            repo_id,
            name: "topic".to_string(),
        },
    );
    observed(
        &mut repos,
        &ids,
        &mut state,
        PendingGitUndoKind::DeleteBranch,
        "refs/heads/topic",
        Some(A),
    );
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::DeleteBranch,
        true,
    );
    let _ = request(&mut repos, &ids, &mut state, GitUndoDirection::Undo);
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::UndoGitOperation,
        true,
    );
    let redo = request(&mut repos, &ids, &mut state, GitUndoDirection::Redo);
    assert!(matches!(
        redo.as_slice(),
        [Effect::ApplyGitUndo { updates, checks, .. }]
            if updates == &vec![RefUpdate {
                reference: "refs/heads/topic".into(),
                expected: Some(id(A)),
                new: id(ZERO),
            }] && checks == &vec![check("topic", false, true)]
    ));
}

/// `git branch -d` refused, then the user forced it: only the forced
/// deletion is recorded.
#[test]
fn a_forced_deletion_after_a_refusal_is_recorded() {
    let (mut repos, ids, mut state, repo_id) = setup();
    let _ = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::DeleteBranch {
            repo_id,
            name: "topic".to_string(),
        },
    );
    let _ = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::Internal(crate::msg::InternalMsg::BranchNotFullyMerged {
            repo_id,
            name: "topic".to_string(),
        }),
    );
    assert!(next_undo(&state).is_none());
    let _ = reduce(
        &mut repos,
        &ids,
        &mut state,
        Msg::ForceDeleteBranch {
            repo_id,
            name: "topic".to_string(),
        },
    );
    observed(
        &mut repos,
        &ids,
        &mut state,
        PendingGitUndoKind::ForceDeleteBranch,
        "refs/heads/topic",
        Some(A),
    );
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::ForceDeleteBranch,
        true,
    );
    assert_eq!(
        next_undo(&state).map(|entry| entry.label),
        Some("Delete branch topic".into())
    );
}

/// M2: an operation the journal does not record that moves a branch or
/// HEAD ends what can be undone; one that cannot move them does not.
#[test]
fn unrecorded_ref_moves_clear_the_journal() {
    let (mut repos, ids, mut state, _) = setup();
    commit_finished(&mut repos, &ids, &mut state);
    command_finished(&mut repos, &ids, &mut state, RepoCommandKind::Push);
    command_finished(&mut repos, &ids, &mut state, RepoCommandKind::FetchAll);
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::StagePaths,
        true,
    );
    assert!(next_undo(&state).is_some(), "push, fetch and stage keep it");

    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::CheckoutBranch,
        true,
    );
    assert!(next_undo(&state).is_none(), "a checkout clears it");

    commit_finished(&mut repos, &ids, &mut state);
    command_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoCommandKind::Rebase {
            onto: "topic".to_string(),
        },
    );
    assert!(next_undo(&state).is_none(), "a rebase clears it");
}

/// M3: Undo waits for any other Git work, pull and push included, without
/// reporting an error: the disabled controls already say so.
#[test]
fn undo_waits_for_other_git_work() {
    let (mut repos, ids, mut state, _) = setup();
    commit_finished(&mut repos, &ids, &mut state);
    for busy in [
        |repo: &mut RepoState| repo.pull_in_flight = 1,
        |repo: &mut RepoState| repo.push_in_flight = 1,
        |repo: &mut RepoState| repo.commit_in_flight = 1,
        |repo: &mut RepoState| repo.sequencer_actions_in_flight = 1,
        |repo: &mut RepoState| repo.local_actions_in_flight = 1,
    ] {
        let mut busy_state = state.clone();
        busy(&mut busy_state.repos[0]);
        assert!(request(&mut repos, &ids, &mut busy_state, GitUndoDirection::Undo).is_empty());
        assert!(busy_state.repos[0].feedback.diagnostics.is_empty());
    }
}

/// M1: with the checked-out branch or the worktrees not loaded, nothing
/// shows the branch is free, so Undo refuses and says why.
#[test]
fn undo_refuses_until_the_repository_has_loaded() {
    let (mut repos, ids, mut state, _) = setup();
    commit_finished(&mut repos, &ids, &mut state);
    state.repos[0].worktrees = Loadable::Loading;
    assert!(request(&mut repos, &ids, &mut state, GitUndoDirection::Undo).is_empty());
    assert_eq!(
        last_diagnostic(&state).as_deref(),
        Some("Cannot undo \"Commit on main\": Wait for the repository to finish loading")
    );
}

/// M3: a commit that lands while an undo runs keeps its own entry; the
/// undo moves the entry it started with.
#[test]
fn an_undo_finishes_with_the_entry_it_started() {
    let (mut repos, ids, mut state, _) = setup();
    commit_finished(&mut repos, &ids, &mut state);
    let _ = request(&mut repos, &ids, &mut state, GitUndoDirection::Undo);
    commit_moved(&mut repos, &ids, &mut state, B, C);
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::UndoGitOperation,
        true,
    );
    let undo_next = next_undo(&state).expect("the later commit stays");
    assert_eq!(undo_next.changes[0].after, Some(id(C)));
    let redo_next = state.repos[0].git_undo.next_redo().expect("undone entry");
    assert_eq!(redo_next.changes[0].after, Some(id(B)));
}

/// L2: a successful undo moved the branch a force-push lease was taken for.
#[test]
fn a_successful_undo_drops_a_stale_force_push_lease() {
    let (mut repos, ids, mut state, _) = setup();
    commit_finished(&mut repos, &ids, &mut state);
    state.repos[0].pending.force_push_lease = Some(gitcomet_core::services::ForcePushLease {
        remote: "origin".to_string(),
        branch: "main".to_string(),
        expected: id(A),
        local_branch: "main".to_string(),
        local_head: id(B),
    });
    let _ = request(&mut repos, &ids, &mut state, GitUndoDirection::Undo);
    action_finished(
        &mut repos,
        &ids,
        &mut state,
        RepoActionKind::UndoGitOperation,
        true,
    );
    assert!(state.repos[0].pending.force_push_lease.is_none());
}
