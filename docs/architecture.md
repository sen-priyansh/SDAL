# SDAL Architecture

SDAL (Simple Distributed Abstraction Layer) is a distributed version control system built on a **Merkle Directed Acyclic Graph (DAG)**. It prioritizes data integrity, deterministic storage, and safety.

## Core Philosophy: "Paranoid & Safe"
- **Content-Addressable Storage (CAS):** All data is addressed by its SHA-256 hash. If the hash matches, the data is guaranteed to be identical.
- **Append-Only:** New objects are created, but existing objects are rarely modified (except strictly managed refs/index).
- **Determinism:** The same file content + same directory structure MUST always result in the exact same Commit Hash, regardless of time or machine.
- **Verification:** Every read operation verifies the hash of the data it loads.

---

## 1. Storage Layer

The repository layout is contained within the `.sdal` directory:

```
.sdal/
├── objects/           # The CAS object store
│   ├── ab/            # First 2 hex chars of hash (sharding)
│   │   └── cdef...    # Remaining hash chars (blob/tree/commit data)
├── refs/              # References (branches, tags)
│   └── heads/         # Branch tips (e.g., main)
├── index              # Staging area (tracks files to be committed)
├── HEAD               # Current active reference (e.g., ref: refs/heads/main)
├── checkpoints.json   # Ghost Checkpoints (ephemeral history)
└── MERGE_STATE        # Temporary state during merge conflicts
```

### Object Serialization Strategy
SDAL is transitioning from JSON to efficient Binary formats.
- **Reading:** The system uses a **Smart Loader** (`Object::from_bytes`) that detects the format:
    1.  Checks for Binary Magic Bytes (e.g., `SDTR`).
    2.  If found, deserializes as Binary.
    3.  If not, attempts fallback to JSON (legacy support).
- **Writing:** New objects are written in the Binary format (where implemented).

---

## 2. The Object Model

There are three primary immutable object types.

### A. Blob (`Blob`)
Represents file content.
- **Storage:** Not stored as a single contiguous file.
- **Chunking:** Files are split into chunks using **FastCDC** (Fast Content-Defined Chunking).
    - **Algorithmic Parameters:**
        - Min Size: 16 KB
        - Avg Size: 64 KB
        - Max Size: 1 MB
    - **Benefit:** Insertions/deletions shift chunk boundaries locally, preserving deduplication for the rest of the file.
- **Structure:** A Blob object is a list of Chunk references (Hash + Size + Offset).

### B. Tree (`Tree`)
Represents a directory structure. Maps logical names to object hashes.
- **Format:** Binary (`SDTR` format).
- **Ordering:** Entries are strictly sorted by name. This guarantees that limits and hasing are deterministic.
- **Limits:** Max 1,000,000 entries; Max 4KB name length (Security hardening).

**Binary Layout (Version 1):**
| Bytes | Field | Description |
|---|---|---|
| 4 | Magic | `SDTR` (0x53 0x44 0x54 0x52) |
| 1 | Version | `1` |
| 1 | Flags | Reserved (0) |
| 2 | Reserved | Reserved (0) |
| 4 | Entry Count | Number of entries `N` |
| ... | Entries | List of `N` entries |

**Entry Layout:**
| Bytes | Field | Description |
|---|---|---|
| 1 | Type | `0`=Blob, `1`=Tree |
| 2 | Name Len | Length `L` |
| L | Name | UTF-8 bytes |
| 32 | Hash | Raw SHA-256 (32 bytes) |
*Note: Entry size is not stored in the Tree object.*

### C. Commit (`Commit`)
Represents a snapshot of the project history.
- **Format:** JSON (Currently).
- **Fields:**
    - `tree`: Hash of the root Tree object.
    - `parents`: List of parent Commit hashes (0 for initial, 1 for normal, 2 for merge).
    - `author`: Committer identity.
    - `message`: Description.
    - `timestamp`: Unix timestamp (seconds).
- **DAG:** Commits point backwards to parents, forming the history graph.

---

## 3. Workflow & Data Flow

### Index (Staging Area)
The `index` file acts as a draft for the next commit.
- Stores a mapping of `Path -> Blob Hash` for all tracked files.
- `sdal add <file>`:
    1.  Reads file.
    2.  Chunks content via FastCDC.
    3.  Writes Chunks and Blob object to storage.
    4.  Updates `index` with the new Blob hash.

### Commit Process
`sdal commit -m "msg"`:
1.  Reads the `index`.
2.  Recursively constructs `Tree` objects from the index paths.
3.  Writes valid `Tree` objects to storage (Binary `SDTR`).
4.  Creates a `Commit` object pointing to the root Tree.
5.  Updates the current branch ref (e.g., `refs/heads/main`) to point to the new Commit.

### Workflows
- **Checkout:** Updates working directory to match a specific Commit/Tree.
    - *Clean Checkout:* Removes untracked files not in the target tree.
- **Status:** Compares distinct states:
    1.  **Head vs Index:** (Staged changes).
    2.  **Index vs Workdir:** (Modified/Unstaged changes).
    - Uses FastCDC hashing to detect modifications accurately.

---

## 4. Ghost Checkpoints
An ephemeral, parallel history mechanism for "auto-save" functionality.
- **Purpose:** Save work-in-progress without polluting the main commit history.
- **Storage:** `.sdal/checkpoints.json`.
- **Structure:**
    ```json
    {
      "id": "cp_timestamp_hash",
      "timestamp": 123456789,
      "message": "wip",
      "tree": "hash_of_workdir_state"
    }
    ```
- **Behavior:**
    - `sdal checkpoint save`: Snapshots current workdir state to a Tree and saves a record.
    - `sdal commit`: Clears all checkpoints (conceptually, they are "solidified" into the commit).

---

## 5. Merge Strategy
SDAL implements a **3-Way Merge** algorithm.
- **Inputs:** `Base` (Common Ancestor), `Ours` (Current Head), `Theirs` (Target Branch).
- **Logic:**
    - If `Ours == Theirs`: No change.
    - If `Base == Ours` AND `Base != Theirs`: Accept `Theirs` (Fast-forward).
    - If `Base != Ours` AND `Base == Theirs`: Keep `Ours`.
    - If `Base != Ours` AND `Base != Theirs` AND `Ours != Theirs`: **Conflict**.
- **Conflict Handling:**
    - Generates `.ours` and `.theirs` files for conflicting paths.
    - Stores conflict state in `.sdal/MERGE_STATE`.
    - User must resolve manually and commit.
