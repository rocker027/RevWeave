//! `git bisect`: reading a worktree's session and recording verdicts.
//!
//! The session lives in the worktree's own git dir (`BISECT_START`,
//! `BISECT_LOG`, `BISECT_TERMS`, `BISECT_NAMES`) and its per-worktree
//! `refs/bisect/` refs, so each worktree bisects on its own. Whether the
//! search ended is read from the log git writes, not inferred from the
//! commit range: with good commits off the bad one's line, git still has
//! merge bases to test when the range looks done.

use super::GixRepo;
use crate::util::{
    command_output, git_command_failed_error, run_git_capture, run_git_raw_output, run_git_simple,
    run_git_with_output, validate_hex_commit_id,
};
use gitcomet_core::domain::CommitId;
use gitcomet_core::error::{Error, ErrorKind};
use gitcomet_core::services::{BisectState, BisectVerdict, CommandOutput, Result};

impl GixRepo {
    fn bisect_file(&self, name: &str) -> std::path::PathBuf {
        self.repo().git_dir().join(name)
    }

    fn bisect_in_progress(&self) -> bool {
        self.bisect_file("BISECT_START").is_file()
    }

    pub(super) fn bisect_state_impl(&self) -> Result<Option<BisectState>> {
        if !self.bisect_in_progress() {
            return Ok(None);
        }
        // Lossy: a path git logged in non-UTF-8 bytes must not hide the
        // outcome lines, which are ASCII.
        let read = |name| {
            std::fs::read(self.bisect_file(name))
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .unwrap_or_default()
        };
        let custom_terms = !matches!(
            read("BISECT_TERMS")
                .split_whitespace()
                .collect::<Vec<_>>()
                .as_slice(),
            [] | ["bad", "good"]
        );
        // Progress counts every commit in the range; a path-limited or
        // first-parent search tests fewer, so its count would mislead.
        let narrowed = !read("BISECT_NAMES").trim().is_empty()
            || self.bisect_file("BISECT_FIRST_PARENT").is_file();

        let mut cmd = self.git_workdir_cmd();
        cmd.args([
            "for-each-ref",
            "--format=%(refname)%09%(objectname)",
            "refs/bisect/",
        ]);
        let refs = run_git_capture(cmd, "git for-each-ref refs/bisect/")?;
        let mut state = parse_bisect_refs(&refs);
        state.custom_terms = custom_terms;
        apply_bisect_log(&mut state, &read("BISECT_LOG"));

        // Progress is a hint: without it the session still shows and can be
        // ended. A path-limited search counts commits git ignores, so skip it.
        let searching = state.first_bad.is_none() && !state.only_skipped_left;
        if searching
            && !narrowed
            && !custom_terms
            && let Some(bad) = state.bad.clone()
            && !state.good.is_empty()
        {
            let mut cmd = self.git_workdir_cmd();
            cmd.args(["rev-list", "--bisect-vars", bad.as_ref(), "--not"]);
            cmd.args(state.good.iter().map(|good| good.as_ref()));
            if let Ok(vars) = run_git_capture(cmd, "git rev-list --bisect-vars") {
                apply_bisect_vars(&mut state, &vars);
            }
        }
        Ok(Some(state))
    }

    pub(super) fn bisect_mark_with_output_impl(
        &self,
        verdict: BisectVerdict,
        commit: Option<&CommitId>,
    ) -> Result<CommandOutput> {
        if let Some(commit) = commit {
            validate_hex_commit_id(commit)?;
            // Refuse before starting a session: a commit whose object is gone
            // would leave an empty session behind.
            let mut cmd = self.git_workdir_cmd();
            cmd.args(["rev-parse", "--verify", "--quiet"])
                .arg(format!("{}^{{commit}}", commit.as_ref()));
            if run_git_capture(cmd, "git rev-parse --verify <commit>").is_err() {
                return Err(Error::new(ErrorKind::Backend(format!(
                    "git bisect: {} is not a commit in this repository",
                    commit.as_ref()
                ))));
            }
        } else if !self.bisect_in_progress() {
            return Err(Error::new(ErrorKind::Backend(
                "git bisect: no session to mark the checked-out commit in".to_string(),
            )));
        }
        if !self.bisect_in_progress() {
            // Only good and bad start a search; a skip meant for a session
            // that ended meanwhile must not open a new one.
            if verdict == BisectVerdict::Skip {
                return Err(Error::new(ErrorKind::Backend(
                    "git bisect: no session to skip a commit in".to_string(),
                )));
            }
            let mut cmd = self.git_workdir_cmd();
            cmd.args(["bisect", "start"]);
            run_git_simple(cmd, "git bisect start")?;
        }
        let term = verdict.git_term();
        let mut cmd = self.git_workdir_cmd();
        cmd.args(["bisect", term]);
        let label = match commit {
            Some(commit) => {
                cmd.arg(commit.as_ref());
                format!("git bisect {term} {}", commit.as_ref())
            }
            None => format!("git bisect {term}"),
        };
        let output = run_git_raw_output(cmd, &label)?;
        // Exit 2 with this notice ends the search inconclusively: every
        // candidate left was skipped. The verdict is recorded, so it is a
        // result, not a failure; the session state says what is left.
        let only_skipped_left = output.status.code() == Some(2)
            && String::from_utf8_lossy(&output.stdout).contains(ONLY_SKIPPED_LEFT);
        if output.status.success() || only_skipped_left {
            Ok(command_output(&label, &output))
        } else {
            Err(git_command_failed_error(&label, output))
        }
    }

    pub(super) fn bisect_reset_with_output_impl(&self) -> Result<CommandOutput> {
        let mut cmd = self.git_workdir_cmd();
        cmd.args(["bisect", "reset"]);
        run_git_with_output(cmd, "git bisect reset")
    }
}

/// What `git bisect` prints when only skipped commits are left to test.
const ONLY_SKIPPED_LEFT: &str = "only 'skip'ped commits left to test";

/// `refs/bisect/bad`, `refs/bisect/good-<sha>` and `refs/bisect/skip-<sha>`,
/// one `<refname>\t<objectname>` per line.
fn parse_bisect_refs(refs: &str) -> BisectState {
    let mut state = BisectState::default();
    for line in refs.lines() {
        let Some((name, oid)) = line.split_once('\t') else {
            continue;
        };
        let commit = CommitId(oid.trim().into());
        match name.strip_prefix("refs/bisect/") {
            Some("bad") => state.bad = Some(commit),
            Some(rest) if rest.starts_with("good-") => state.good.push(commit),
            Some(rest) if rest.starts_with("skip-") => state.skipped.push(commit),
            _ => {}
        }
    }
    state
}

/// The outcome git logged after the session's last command: `# first bad
/// commit: [<sha>]` (or `# first '<term>' commit: [<sha>]` in newer Git) once
/// found, `# only skipped commits left to test` when the skips cover every
/// candidate. A later command clears both.
fn apply_bisect_log(state: &mut BisectState, log: &str) {
    for line in log.lines() {
        if line.starts_with("git bisect ") {
            state.first_bad = None;
            state.only_skipped_left = false;
        } else if let Some(rest) = line.strip_prefix("# first ")
            && let Some((_, after)) = rest.split_once(" commit: [")
            && let Some((sha, _)) = after.split_once(']')
        {
            state.first_bad = Some(CommitId(sha.trim().into()));
        } else if line.trim() == "# only skipped commits left to test" {
            state.only_skipped_left = true;
        }
    }
}

/// Reads `bisect_nr` and `bisect_steps` from `git rev-list --bisect-vars`.
fn apply_bisect_vars(state: &mut BisectState, vars: &str) {
    let value = |key: &str| {
        vars.lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
            .and_then(|value| value.trim().trim_matches('\'').parse::<usize>().ok())
    };
    state.remaining = value("bisect_nr");
    state.steps = value("bisect_steps");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bisect_refs_sorts_verdicts() {
        let state = parse_bisect_refs(
            "refs/bisect/bad\tb1\nrefs/bisect/good-g1\tg1\nrefs/bisect/skip-s1\ts1\nrefs/bisect/good-g2\tg2\n",
        );
        assert_eq!(state.bad, Some(CommitId("b1".into())));
        assert_eq!(
            state.good,
            vec![CommitId("g1".into()), CommitId("g2".into())]
        );
        assert_eq!(state.skipped, vec![CommitId("s1".into())]);
    }

    #[test]
    fn bisect_vars_give_progress() {
        let mut state = BisectState::default();
        apply_bisect_vars(
            &mut state,
            "bisect_rev='m1'\nbisect_nr=3\nbisect_good=1\nbisect_bad=2\nbisect_all=7\nbisect_steps=2\n",
        );
        assert_eq!((state.remaining, state.steps), (Some(3), Some(2)));
    }

    #[test]
    fn the_log_says_when_the_search_ended() {
        let mut state = BisectState::default();
        apply_bisect_log(
            &mut state,
            "git bisect start\n# bad: [b] c5\ngit bisect bad b\ngit bisect good g\n\
             # first 'bad' commit: [a925ec4] c4\n",
        );
        assert_eq!(state.first_bad, Some(CommitId("a925ec4".into())));

        // Older Git (2.53 among them) writes the outcome without the term quoted.
        let mut state = BisectState::default();
        apply_bisect_log(&mut state, "git bisect bad b\n# first bad commit: [b] c4\n");
        assert_eq!(state.first_bad, Some(CommitId("b".into())));

        let mut state = BisectState::default();
        apply_bisect_log(
            &mut state,
            "git bisect skip s\n# only skipped commits left to test\n\
             # possible first 'bad' commit: [b] c5\n",
        );
        assert!(state.only_skipped_left);
        assert_eq!(state.first_bad, None);

        // A command after the outcome reopens the search.
        let mut state = BisectState::default();
        apply_bisect_log(
            &mut state,
            "# only skipped commits left to test\ngit bisect good s\n",
        );
        assert!(!state.only_skipped_left);
    }
}
