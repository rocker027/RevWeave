# Reflog

The reflog panel lists where `HEAD` has pointed, newest first. Clicking an entry
selects its commit in the details pane.

## Rescuing a commit

A hard reset, a rebase, or an amend can leave a commit that no branch reaches.
Its reflog entry still points at it. To get it back without touching the working
tree, right-click the entry and choose **Create branch here…**.

The branch prompt opens with **Checkout** off, so only the branch is created. That
creation can be undone with `Cmd-Z` / `Ctrl-Z`, like any branch creation. Turn
Checkout on in the prompt to switch to the new branch as well; checking out ends
what Git Undo can undo.

A branch keeps only the commit and its history. Changes that were never committed
are not in the reflog and cannot be rescued this way.

## Resetting to an entry

The same menu offers **Reset (--soft)**, **Reset (--mixed)** and **Reset (--hard)**
to the entry's commit, with the same confirmation as the history log's commit menu.
A hard reset discards uncommitted changes.
