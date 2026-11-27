# Bisect

`git bisect` finds the commit that introduced a problem by testing commits
between one known to be bad and one known to be good. RevWeave drives it from the
history graph and the toolbar.

## Starting

Right-click a commit in the history and choose **Bisect: mark as bad** or
**Bisect: mark as good**. The first mark starts the session. Once one bad and one
good commit are marked, Git checks out a commit halfway between them.

Marking needs no uncommitted changes to tracked files and no other Git operation
in progress, because Git checks commits out as the session goes. Commit or stash
your changes first. Untracked files are left alone; if one is in the way of the
next commit to test, Git records the verdict but refuses that checkout, and the
error says which file to move.

## Testing

While a session runs, the toolbar shows **BISECTING**, what the session needs next
or roughly how many steps are left, and four buttons for the checked-out commit:

| Button | What it does |
| --- | --- |
| **Good** | The checked-out commit does not have the problem. |
| **Bad** | The checked-out commit has the problem. |
| **Skip** | The checked-out commit cannot be tested; Git picks another. |
| **End** | Ends the session and returns to the branch and commit it started from. |

The commit menu offers the same verdicts for any commit, plus **Bisect: skip** and
**End bisect**. History rows show the verdicts as `good`, `bad` and `skip` marks.
While the session runs, the commit menu's history-rewriting actions (rebase,
interactive rebase, edit message, fixup, revert, cherry-pick) are disabled.

## Finding the first bad commit

When only one candidate is left, the toolbar shows **First bad: <sha>** and that
row is marked `first bad`. Click the status to select the commit in History. The
verdict buttons turn off; choose **End** to finish.

If every commit left besides the bad one was skipped, Git cannot narrow it down:
the toolbar shows **Only skipped commits left**, and the first bad commit is the
bad one or one of the skipped ones. Mark a skipped commit good or bad to go on, or
choose **End**.

A session started in a terminal is picked up as well. If it uses its own terms
(such as `old`/`new`), RevWeave shows it and offers **End**, but leaves the
verdicts to the terminal, since its Good and Bad would not match those terms.

The graph follows the history scope: with a scope reachable from HEAD alone, the
commits after the one under test are not drawn until the session ends.
