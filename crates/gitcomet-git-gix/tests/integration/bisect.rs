use crate::git_cli::{git_stdout, run_git};
use crate::test_git_env;
use gitcomet_core::domain::CommitId;
use gitcomet_core::services::{BisectVerdict, CancellationToken, GitBackend, GitRepository};
use gitcomet_core::test_support::git_fixture::append_config;
use gitcomet_git_gix::GixBackend;
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// `main` with five commits; `value.txt` says `bad` from the fourth one on.
fn repo_with_a_regression(repo: &Path) -> Vec<String> {
    run_git(repo, &["init", "-b", "main"]);
    append_config(
        repo,
        &[("user.email", "you@example.com"), ("user.name", "You")],
    );
    (1..=5)
        .map(|n| {
            let value = if n >= 4 { "bad" } else { "good" };
            fs::write(repo.join("value.txt"), format!("{n} {value}\n")).expect("write file");
            run_git(repo, &["add", "."]);
            run_git(
                repo,
                &[
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "-m",
                    &format!("c{n}"),
                ],
            );
            git_stdout(repo, &["rev-parse", "HEAD"])
        })
        .collect()
}

fn open(repo: &Path) -> Arc<dyn GitRepository> {
    GixBackend.open(repo).expect("open repository")
}

fn state(repo: &Arc<dyn GitRepository>) -> Option<gitcomet_core::services::BisectState> {
    repo.bisect_state(&CancellationToken::new())
        .expect("read bisect state")
}

/// Marks what git checked out, judging by the file's content.
fn mark_checked_out(dir: &Path, repo: &Arc<dyn GitRepository>) {
    let head = git_stdout(dir, &["rev-parse", "HEAD"]);
    let content = fs::read_to_string(dir.join("value.txt")).expect("read file");
    let verdict = if content.contains("bad") {
        BisectVerdict::Bad
    } else {
        BisectVerdict::Good
    };
    repo.bisect_mark_with_output(verdict, Some(&CommitId(head.into())))
        .expect("mark the checked-out commit");
}

#[test]
fn bisect_finds_the_first_bad_commit_and_resets() {
    test_git_env::ensure_initialized();
    let dir = tempfile::tempdir().unwrap();
    let commits = repo_with_a_regression(dir.path());
    let repo = open(dir.path());
    assert_eq!(state(&repo), None, "no session yet");

    // Marking the tip bad starts the session; nothing is checked out yet.
    repo.bisect_mark_with_output(
        BisectVerdict::Bad,
        Some(&CommitId(commits[4].clone().into())),
    )
    .expect("mark the tip bad");
    let started = state(&repo).expect("a session");
    assert_eq!(started.bad, Some(CommitId(commits[4].clone().into())));
    assert!(started.good.is_empty());
    assert_eq!(started.remaining, None);

    repo.bisect_mark_with_output(
        BisectVerdict::Good,
        Some(&CommitId(commits[0].clone().into())),
    )
    .expect("mark the root good");
    let narrowed = state(&repo).expect("a session");
    assert!(narrowed.remaining.is_some(), "{narrowed:?}");
    assert_eq!(narrowed.first_bad, None);

    for _ in 0..4 {
        if state(&repo).and_then(|s| s.first_bad).is_some() {
            break;
        }
        mark_checked_out(dir.path(), &repo);
    }
    assert_eq!(
        state(&repo).and_then(|s| s.first_bad),
        Some(CommitId(commits[3].clone().into())),
        "c4 introduced `bad`"
    );

    repo.bisect_reset_with_output().expect("reset");
    assert_eq!(state(&repo), None);
    assert_eq!(
        git_stdout(dir.path(), &["symbolic-ref", "--short", "HEAD"]),
        "main"
    );
    assert_eq!(git_stdout(dir.path(), &["rev-parse", "HEAD"]), commits[4]);
}

#[test]
fn skipping_every_candidate_left_is_a_result_not_an_error() {
    test_git_env::ensure_initialized();
    let dir = tempfile::tempdir().unwrap();
    let commits = repo_with_a_regression(dir.path());
    let repo = open(dir.path());
    repo.bisect_mark_with_output(
        BisectVerdict::Bad,
        Some(&CommitId(commits[4].clone().into())),
    )
    .expect("mark the tip bad");
    repo.bisect_mark_with_output(
        BisectVerdict::Good,
        Some(&CommitId(commits[0].clone().into())),
    )
    .expect("mark the root good");
    // Git checks out c3 first, then c4.
    assert_eq!(git_stdout(dir.path(), &["rev-parse", "HEAD"]), commits[2]);
    repo.bisect_mark_with_output(
        BisectVerdict::Good,
        Some(&CommitId(commits[2].clone().into())),
    )
    .expect("mark c3 good");
    assert_eq!(git_stdout(dir.path(), &["rev-parse", "HEAD"]), commits[3]);

    let output = repo
        .bisect_mark_with_output(
            BisectVerdict::Skip,
            Some(&CommitId(commits[3].clone().into())),
        )
        .expect("skipping the last candidate ends the search, not in an error");
    assert_eq!(output.exit_code, Some(2));
    let left = state(&repo).expect("the session goes on");
    assert!(left.only_skipped_left, "{left:?}");
    assert_eq!(left.first_bad, None);
    assert_eq!(left.skipped, vec![CommitId(commits[3].clone().into())]);

    repo.bisect_reset_with_output().expect("reset");
}

/// Good and bad on sibling branches: git first tests their merge base, so the
/// range alone (one candidate) does not mean the search is over.
#[test]
fn diverged_good_and_bad_test_the_merge_base_before_ending() {
    test_git_env::ensure_initialized();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    run_git(path, &["init", "-b", "main"]);
    append_config(
        path,
        &[("user.email", "you@example.com"), ("user.name", "You")],
    );
    let commit = |message: &str| {
        fs::write(path.join("value.txt"), format!("{message}\n")).expect("write file");
        run_git(path, &["add", "."]);
        run_git(
            path,
            &["-c", "commit.gpgsign=false", "commit", "-m", message],
        );
        git_stdout(path, &["rev-parse", "HEAD"])
    };
    let base = commit("base");
    let bad = commit("bad");
    run_git(path, &["switch", "-c", "side", &base]);
    let good = commit("good");
    run_git(path, &["switch", "main"]);

    let repo = open(path);
    repo.bisect_mark_with_output(BisectVerdict::Bad, Some(&CommitId(bad.clone().into())))
        .expect("mark bad");
    repo.bisect_mark_with_output(BisectVerdict::Good, Some(&CommitId(good.into())))
        .expect("mark good");
    assert_eq!(
        git_stdout(path, &["rev-parse", "HEAD"]),
        base,
        "git tests the merge base"
    );
    let testing = state(&repo).expect("a session");
    assert_eq!(testing.first_bad, None, "{testing:?}");

    // The toolbar marks what git checked out, not a SHA the UI last saw.
    repo.bisect_mark_with_output(BisectVerdict::Good, None)
        .expect("mark the checked-out merge base good");
    assert_eq!(
        state(&repo).and_then(|state| state.first_bad),
        Some(CommitId(bad.into()))
    );
    repo.bisect_reset_with_output().expect("reset");
}
