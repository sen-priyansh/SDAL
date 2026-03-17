# sdal-mergetool

`sdal-mergetool` is the conflict resolution TUI for SDAL. Unlike Git's approach of writing `<<<<<<<` markers directly into source files, this tool keeps your files completely clean and compilable throughout the entire merge. All conflict state is stored as structured data in `.sdal/conflicts.json` — conflicts have a type, base/ours/theirs content, the commit message from each side, and a resolution field.

### First-Class Conflicts
Conflicts are first-class citizens in SDAL. You can commit locally with unresolved conflicts sitting in the queue. Only `sdal push` is gated until everything is resolved, so you're never blocked from saving local progress.

### Interactive TUI
The TUI is built with `ratatui` and `crossterm`. It provides a powerful interface for resolving differences:

- **Sidebar**: Lists all conflicted files with their status:
    - `✓ Resolved`: Conflict handled by the user.
    - `◆ Pending`: Awaiting attention.
    - `⏸ Deferred`: Put aside for later.
- **Three-Pane Diff**: Shows **BASE**, **OURS**, and **THEIRS** side by side with changed lines highlighted.
- **Result Preview**: Updates live as you make choices, showing exactly what will be written to disk.

### Five Core Actions
Resolution is streamlined into five simple actions:
1. **Take Ours**: Resolve using the change from the current branch.
2. **Take Theirs**: Resolve using the change from the incoming branch.
3. **Open $EDITOR**: Launch your preferred editor to handle a specific hunk manually.
4. **Defer**: Set the conflict aside to come back to it later.
5. **Undo**: Revert the last resolution action.

### Persistent Progress
Progress is saved whenever you close the terminal, so you can resume your work with `sdal resolve` at any point.
