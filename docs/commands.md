# SDAL Command Reference

## Repository Management

### `sdal init`
Initialize a new SDAL repository.
Creates a `.sdal` directory with the necessary structure for version control.

## Staging and Committing

### `sdal add <files>...`
Add files to the staging area.
- `<files>`: List of files to stage. Use `.` to add all files in the current directory (respects `.sdalignore`).

### `sdal commit`
Create a new commit from staged changes.
- `--message`, `-m <msg>`: Commit message describing the changes.
- **Note**: If checkpoints exist, this command automatically uses the current checkpoint tree and deletes all checkpoints.

## History and Status

### `sdal log`
Show the commit history.
Displays commits in reverse chronological order with hash, author, date, and message.

### `sdal status`
Show the working directory status.
Displays:
- Staged files (green)
- Modified files (yellow)
- Deleted files (red)
- Untracked files (red)
- Ghost Checkpoints (if any)
- Ledger Head status

## Navigation and Recovery

### `sdal reset [commit]`
Reset current HEAD to a specified commit.
- `[commit]`: Commit hash or reference (e.g., `HEAD`, `HEAD~1`). Defaults to `HEAD`.
- `--mode <mode>`:
    - `soft`: Move HEAD only (keep staged and working changes).
    - `mixed`: Move HEAD and unstage (default, keep working changes).
    - `hard`: Move HEAD, unstage, and discard all changes.

### `sdal restore <files>...`
Restore files from HEAD.
Discards uncommitted changes and restores files to their state in HEAD.

## Branching

### `sdal branch [name]`
Manage branches.
- `[name]`: Name of the branch to create. If omitted, lists all branches.
- `--delete`, `-d`: Delete the specified branch.

### `sdal checkout <branch>`
Switch branches or restore working tree files.
- `<branch>`: Branch name to switch to.
- `--create`, `-b`: Create the branch before checking out.

### `sdal merge <branch>`
Merge another branch into the current branch.
Performs a 3-way merge.
- `<branch>`: Branch to merge into the current branch.

## Checkpoints
Manage temporary local snapshots. Checkpoints are automatically deleted when you `commit`.

### `sdal checkpoint save`
Save current working state as a checkpoint.
- `[message]`: Optional description.

### `sdal checkpoint list`
List all saved checkpoints.
Shows IDs, messages, and timestamps.

### `sdal checkpoint checkout <id>`
Restore working directory to a checkpoint.
- `<id>`: Checkpoint ID (e.g., `cp_0001`).

### `sdal checkpoint drop <id>`
Delete a checkpoint.
- `<id>`: Checkpoint ID to delete.

## Debug

### `sdal cat <hash>`
Display a blob object.
- `<hash>`: Blob hash to display.
