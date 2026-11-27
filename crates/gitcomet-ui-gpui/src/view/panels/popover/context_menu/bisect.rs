//! `git bisect` from the history's commit menu: marking a commit good, bad
//! or skipped (the first mark starts the session), and ending the session.
//! The toolbar's bisect controls act on the checked-out commit the same way.

use super::*;
use crate::localization::Language;
use gitcomet_core::domain::FileStatusKind;
use gitcomet_core::services::BisectVerdict;
use gitcomet_state::msg::BisectStep;

/// Why `repo` cannot take a bisect mark right now, worded for a tooltip;
/// `None` when it can. Marking can check out the next commit to test, so it
/// needs a clean working tree and no other Git operation in progress.
pub(in crate::view) fn bisect_mark_blocker(repo: &RepoState) -> Option<&'static str> {
    if repo.is_bare() {
        return Some(gitcomet_core::domain::WORKING_TREE_REQUIRED_MESSAGE);
    }
    let other_operation = repo.local_actions_in_flight > 0
        || repo.worktree_pull_in_flight > 0
        || matches!(
            repo.sequencer_state,
            Loadable::Ready(state) if state != gitcomet_core::services::SequencerState::None
        )
        || matches!(repo.rebase_in_progress, Loadable::Ready(true))
        || matches!(&repo.merge_commit_message, Loadable::Ready(Some(_)));
    if other_operation {
        return Some("Finish the running Git operation to bisect");
    }
    let (Some(unstaged), Some(staged)) =
        (repo.worktree_status_entries(), repo.staged_status_entries())
    else {
        return Some("Loading the working tree status…");
    };
    let dirty = !staged.is_empty()
        || unstaged
            .iter()
            .any(|entry| entry.kind != FileStatusKind::Untracked);
    if dirty {
        return Some("Commit or stash your changes to bisect");
    }
    if let Loadable::Ready(Some(bisect)) = &repo.bisect {
        if bisect.custom_terms {
            return Some(
                "This bisect was started with its own terms; continue it in a terminal or end it",
            );
        }
        if bisect.first_bad.is_some() {
            return Some("The first bad commit is found; end the bisect");
        }
    }
    None
}

fn step_entry(
    label: &'static str,
    icon: &'static str,
    disabled: bool,
    repo_id: RepoId,
    step: BisectStep,
) -> ContextMenuItem {
    ContextMenuItem::Entry {
        label: label.into(),
        icon: Some(icon.into()),
        shortcut: None,
        disabled,
        action: Box::new(ContextMenuAction::Bisect { repo_id, step }),
    }
}

/// The commit menu's bisect entries for `commit_id`, with the tooltip of the
/// first disabled one. Outside a session only good and bad are offered: one
/// of each is needed before git checks anything out.
pub(super) fn entries(
    repo: &RepoState,
    commit_id: &CommitId,
    worktree_finish_busy: bool,
    language: Language,
) -> (Vec<ContextMenuItem>, Option<&'static str>) {
    let blocker = bisect_mark_blocker(repo);
    let disabled = blocker.is_some() || worktree_finish_busy;
    let mark = |verdict| BisectStep::Mark {
        verdict,
        commit_id: commit_id.clone(),
    };
    let in_session = repo.bisect_in_progress();
    let mut items = vec![
        step_entry(
            language.text("Bisect: mark as bad", "Bisect：標為壞版本"),
            "icons/generic_close.svg",
            disabled,
            repo.id,
            mark(BisectVerdict::Bad),
        ),
        step_entry(
            language.text("Bisect: mark as good", "Bisect：標為好版本"),
            "icons/check.svg",
            disabled,
            repo.id,
            mark(BisectVerdict::Good),
        ),
    ];
    if in_session {
        items.push(step_entry(
            language.text("Bisect: skip", "Bisect：略過"),
            "icons/box.svg",
            disabled,
            repo.id,
            mark(BisectVerdict::Skip),
        ));
        items.push(step_entry(
            language.text("End bisect", "結束 Bisect"),
            "icons/repo_tab_close.svg",
            repo.is_bare()
                || repo.local_actions_in_flight > 0
                || repo.worktree_pull_in_flight > 0
                || worktree_finish_busy,
            repo.id,
            BisectStep::Reset,
        ));
    }
    (items, blocker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitcomet_core::domain::{FileStatus, RepoSpec, RepoStatus};
    use gitcomet_core::services::BisectState;
    use std::sync::Arc;

    /// An open, clean working tree with no operation in progress.
    fn clean_repo() -> RepoState {
        let mut repo = RepoState::new_opening(
            RepoId(1),
            RepoSpec {
                workdir: "/tmp/repo-bisect".into(),
            },
        );
        repo.open = Loadable::Ready(());
        repo.status = Loadable::Ready(
            RepoStatus {
                staged: Arc::new(Vec::new()),
                unstaged: Arc::new(Vec::new()),
            }
            .into(),
        );
        repo.sequencer_state = Loadable::Ready(Default::default());
        repo.rebase_in_progress = Loadable::Ready(false);
        repo.merge_commit_message = Loadable::Ready(None);
        repo.bisect = Loadable::Ready(None);
        repo
    }

    fn labels(items: &[ContextMenuItem]) -> Vec<String> {
        items
            .iter()
            .filter_map(|item| match item {
                ContextMenuItem::Entry {
                    label, disabled, ..
                } => Some(format!("{label}{}", if *disabled { " (off)" } else { "" })),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_session_blocks_amend() {
        let mut repo = clean_repo();
        repo.head_branch = Loadable::Ready("HEAD".to_string());
        assert!(crate::view::mod_helpers::amend_allowed(&repo));
        repo.bisect = Loadable::Ready(Some(BisectState::default()));
        assert!(!crate::view::mod_helpers::amend_allowed(&repo));
    }

    #[test]
    fn a_session_started_with_its_own_terms_only_ends() {
        let mut repo = clean_repo();
        repo.bisect = Loadable::Ready(Some(BisectState {
            custom_terms: true,
            ..Default::default()
        }));
        assert_eq!(
            bisect_mark_blocker(&repo),
            Some("This bisect was started with its own terms; continue it in a terminal or end it")
        );
        let (items, _) = entries(&repo, &CommitId("c1".into()), false, Language::English);
        assert_eq!(
            labels(&items).last().map(String::as_str),
            Some("End bisect")
        );
    }

    #[test]
    fn outside_a_session_only_good_and_bad_start_one() {
        let repo = clean_repo();
        let (items, blocker) = entries(&repo, &CommitId("c1".into()), false, Language::English);
        assert_eq!(blocker, None);
        assert_eq!(
            labels(&items),
            vec!["Bisect: mark as bad", "Bisect: mark as good"]
        );
    }

    #[test]
    fn a_session_adds_skip_and_end() {
        let mut repo = clean_repo();
        repo.bisect = Loadable::Ready(Some(BisectState::default()));
        let (items, _) = entries(&repo, &CommitId("c1".into()), false, Language::English);
        assert_eq!(
            labels(&items),
            vec![
                "Bisect: mark as bad",
                "Bisect: mark as good",
                "Bisect: skip",
                "End bisect"
            ]
        );
    }

    #[test]
    fn marking_needs_a_clean_tree_and_no_other_operation() {
        let mut repo = clean_repo();
        repo.status = Loadable::Ready(
            RepoStatus {
                staged: Arc::new(Vec::new()),
                unstaged: Arc::new(vec![FileStatus {
                    path: "a.txt".into(),
                    kind: FileStatusKind::Modified,
                    conflict: None,
                }]),
            }
            .into(),
        );
        assert_eq!(
            bisect_mark_blocker(&repo),
            Some("Commit or stash your changes to bisect")
        );

        let mut repo = clean_repo();
        repo.rebase_in_progress = Loadable::Ready(true);
        assert_eq!(
            bisect_mark_blocker(&repo),
            Some("Finish the running Git operation to bisect")
        );

        // The session itself does not block its own marks.
        let mut repo = clean_repo();
        repo.bisect = Loadable::Ready(Some(BisectState::default()));
        assert_eq!(bisect_mark_blocker(&repo), None);

        repo.bisect = Loadable::Ready(Some(BisectState {
            first_bad: Some(CommitId("c1".into())),
            ..Default::default()
        }));
        let (items, blocker) = entries(&repo, &CommitId("c2".into()), false, Language::English);
        assert_eq!(
            blocker,
            Some("The first bad commit is found; end the bisect")
        );
        assert_eq!(
            labels(&items),
            vec![
                "Bisect: mark as bad (off)",
                "Bisect: mark as good (off)",
                "Bisect: skip (off)",
                "End bisect"
            ]
        );
    }
}
