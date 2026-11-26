//! `git bisect`: reading a worktree's session and recording verdicts.
//!
//! The session lives in the worktree's own git dir (`BISECT_START`) and its
//! per-worktree `refs/bisect/` refs, so each worktree bisects on its own.

use super::GixRepo;
use crate::util::{run_git_capture, run_git_simple, run_git_with_output, validate_hex_commit_id};
use gitcomet_core::domain::CommitId;
use gitcomet_core::services::{BisectState, BisectVerdict, CommandOutput, Result};

impl GixRepo {
    fn bisect_in_progress(&self) -> bool {
        self.repo().git_dir().join("BISECT_START").is_file()
    }

    pub(super) fn bisect_state_impl(&self) -> Result<Option<BisectState>> {
        if !self.bisect_in_progress() {
            return Ok(None);
        }
        let mut cmd = self.git_workdir_cmd();
        cmd.args([
            "for-each-ref",
            "--format=%(refname)%09%(objectname)",
            "refs/bisect/",
        ]);
        let refs = run_git_capture(cmd, "git for-each-ref refs/bisect/")?;
        let mut state = parse_bisect_refs(&refs);
        if let Some(bad) = state.bad.clone()
            && !state.good.is_empty()
        {
            let mut cmd = self.git_workdir_cmd();
            cmd.args(["rev-list", "--bisect-vars", bad.as_ref(), "--not"]);
            cmd.args(state.good.iter().map(|good| good.as_ref()));
            let vars = run_git_capture(cmd, "git rev-list --bisect-vars")?;
            apply_bisect_vars(&mut state, &bad, &vars);
        }
        Ok(Some(state))
    }

    pub(super) fn bisect_mark_with_output_impl(
        &self,
        verdict: BisectVerdict,
        commit: &CommitId,
    ) -> Result<CommandOutput> {
        validate_hex_commit_id(commit)?;
        if !self.bisect_in_progress() {
            let mut cmd = self.git_workdir_cmd();
            cmd.args(["bisect", "start"]);
            run_git_simple(cmd, "git bisect start")?;
        }
        let term = verdict.git_term();
        let mut cmd = self.git_workdir_cmd();
        cmd.args(["bisect", term, commit.as_ref()]);
        run_git_with_output(cmd, &format!("git bisect {term} {}", commit.as_ref()))
    }

    pub(super) fn bisect_reset_with_output_impl(&self) -> Result<CommandOutput> {
        let mut cmd = self.git_workdir_cmd();
        cmd.args(["bisect", "reset"]);
        run_git_with_output(cmd, "git bisect reset")
    }
}

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

/// Reads `bisect_nr`, `bisect_steps` and `bisect_all` from
/// `git rev-list --bisect-vars`: with only the bad commit left in the range,
/// it is the first bad commit.
fn apply_bisect_vars(state: &mut BisectState, bad: &CommitId, vars: &str) {
    let value = |key: &str| {
        vars.lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
            .and_then(|value| value.trim().trim_matches('\'').parse::<usize>().ok())
    };
    state.remaining = value("bisect_nr");
    state.steps = value("bisect_steps");
    if value("bisect_all") == Some(1) {
        state.first_bad = Some(bad.clone());
    }
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
    fn bisect_vars_give_progress_and_the_first_bad_commit() {
        let bad = CommitId("b1".into());
        let mut state = BisectState::default();
        apply_bisect_vars(
            &mut state,
            &bad,
            "bisect_rev='m1'\nbisect_nr=3\nbisect_good=1\nbisect_bad=2\nbisect_all=7\nbisect_steps=2\n",
        );
        assert_eq!((state.remaining, state.steps), (Some(3), Some(2)));
        assert_eq!(state.first_bad, None);

        apply_bisect_vars(
            &mut state,
            &bad,
            "bisect_nr=0\nbisect_all=1\nbisect_steps=0\n",
        );
        assert_eq!(state.first_bad, Some(bad));
    }
}
