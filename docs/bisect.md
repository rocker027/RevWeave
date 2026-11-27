# Bisect

`git bisect` finds the commit that introduced a problem by testing commits
between one known to be bad and one known to be good. RevWeave drives it from the
history graph and the toolbar.

## Starting

Right-click a commit in the history and choose **Bisect: mark as bad** or
**Bisect: mark as good**. The first mark starts the session. Once one bad and one
good commit are marked, Git checks out a commit halfway between them.

Marking needs a clean working tree and no other Git operation in progress, because
Git checks commits out as the session goes. Commit or stash your changes first.

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

The graph follows the history scope: with a scope reachable from HEAD alone, the
commits after the one under test are not drawn until the session ends.
