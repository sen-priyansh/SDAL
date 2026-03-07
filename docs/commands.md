# SDAL Command Reference

## Core Workflow

### `init`
**Usage**: `sdal init`
Initializes a new SDAL repository in the current directory.

### `add`
**Usage**: `sdal add <files>...`
Stages files for the next commit.
-   **Args**:
    -   `<files>`: File paths to stage. Use `.` for all recursive files.
-   **Example**: `sdal add src/ main.rs`

### `commit`
**Usage**: `sdal commit -m <message>`
Records a snapshot of the staging area.
-   **Options**:
    -   `-m, --message <msg>`: Required. Description of changes.
-   **Note**: Automatically solidifies any active checkpoints.

### `status`
**Usage**: `sdal status`
Shows the state of the working directory and staging area.
-   **Output**:
    -   <span style="color:green">A</span> (Added/Staged)
    -   <span style="color:yellow">M</span> (Modified)
    -   <span style="color:red">D</span> (Deleted)
    -   <span style="color:red">?</span> (Untracked)

### `log`
**Usage**: `sdal log`
Displays the commit history in reverse chronological order.

---

## Branching & Merging

### `branch`
**Usage**: `sdal branch [name] [options]`
Manages branches.
-   **Args**:
    -   `[name]`: Branch name to create. (If omitted, lists branches).
-   **Options**:
    -   `-d, --delete`: Deletes the specified branch.
-   **Example**: `sdal branch feature-login`

### `checkout`
**Usage**: `sdal checkout <branch> [options]`
Switches branches or restores working tree files.
-   **Options**:
    -   `-b, --create`: Create a new branch and switch to it.
-   **Example**: `sdal checkout -b new-feature`

### `merge`
**Usage**: `sdal merge <branch>`
Merges the specified branch into the current branch (3-way merge).

---

## Navigation & Recovery

### `reset`
**Usage**: `sdal reset [commit] --mode <mode>`
Resets HEAD to a specific state.
-   **Args**:
    -   `[commit]`: Target commit (default: `HEAD`).
-   **Options**:
    -   `--mode soft`: Moves HEAD only. Staged/Working changes preserved.
    -   `--mode mixed`: Moves HEAD + Unstages files. Working changes preserved. (Default)
    -   `--mode hard`: **Destructive**. Moves HEAD + Unstages + Resets Workdir.

### `restore`
**Usage**: `sdal restore <files>...`
Discards uncommitted changes in working directory (restores from HEAD).

---

## Ghost Checkpoints (WIP)
*Ephemeral snapshots for "save-scumming" your work without committing.*

### `checkpoint save`
**Usage**: `sdal checkpoint save [message]`
Saves current state as a checkpoint.

### `checkpoint list`
**Usage**: `sdal checkpoint list`
Lists all active checkpoints.

### `checkpoint checkout`
**Usage**: `sdal checkpoint checkout <id>`
Restores working directory to a specific checkpoint state.

### `checkpoint drop`
**Usage**: `sdal checkpoint drop <id>`
Deletes a checkpoint.

---

## Debugging

### `cat`
**Usage**: `sdal cat <hash>`
Inspects a blob object.
-   **Args**:
    -   `<hash>`: The SHA-256 hash of the blob.
