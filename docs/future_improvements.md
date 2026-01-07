# Future Improvements & Deferred Issues

This document tracks correctness improvements and features that were identified but deferred for future implementation.

## High Priority (Should Do Soon)

### 1. Conflict File Materialization
**Location**: `crates/cli/src/main.rs` - Merge command (lines 850-880)

**Issue**: When merge conflicts occur, `.ours` and `.theirs` files are not actually written to disk.

**Current State**: TODO placeholder exists in merge conflict handling code.

**What's Needed**:
- Implement proper tree walking to extract blob hashes for conflicted paths
- Use `write_conflict_files` helper function from `merge.rs`
- Write actual `.ours` and `.theirs` files for each conflict

**Why Deferred**: Requires implementing recursive tree walking logic to map paths to blob hashes.

---

### 2. Blob Size Validation in Tree
**Location**: `crates/core/src/lib.rs` - `Tree::validate()`

**Issue**: When a tree contains `TreeEntry::Blob { hash, size }`, we don't validate that the stored blob's actual size matches the `size` field.

**Current State**: Validation checks tree structure but not blob metadata accuracy.

**What's Needed**:
```rust
// In Tree::validate()
TreeEntry::Blob { hash, size } => {
    let blob_data = storage.get(hash)?;
    let blob: Blob = serde_json::from_slice(&blob_data)?;
    if blob.total_size != size {
        return Err("Blob size mismatch");
    }
}
```

**Why Deferred**: Requires I/O operations during validation, making it expensive. Would need to be opt-in or done during specific operations.

---

## Medium Priority (Nice to Have)

### 3. Nested Tree Support in Merge
**Location**: `crates/core/src/merge.rs` - `flatten_tree()`

**Issue**: Merge currently only handles flat trees (single level). Nested directories are ignored during merge.

**Current State**:
```rust
TreeEntry::Tree { .. } => {
    // For now, treat nested trees as single entries
    // Full implementation would recursively flatten
}
```

**What's Needed**:
- Recursively flatten nested trees during merge
- Properly handle directory-level conflicts
- Merge directory structures

**Why Deferred**: Current implementation works for simple use cases. Full nested merge is complex.

---

### 4. Inline Conflict Markers
**Location**: `crates/core/src/merge.rs` - Conflict materialization

**Issue**: Conflicts are only shown as separate `.ours` and `.theirs` files, not with inline markers like Git.

**Current State**: Minimal conflict handling (separate files only).

**What's Needed**:
```
<<<<<<< ours
our content
=======
their content
>>>>>>> theirs
```

**Why Deferred**: Requires content-aware merging and is a UX enhancement, not a correctness issue.

---

## Low Priority (Future Enhancements)

### 5. Symbolic Link Support
**Location**: Throughout codebase

**Issue**: SDAL doesn't handle symbolic links in the working directory.

**What's Needed**:
- Detect symlinks during `sdal add`
- Store symlink targets in tree
- Restore symlinks during checkout

**Why Deferred**: Edge case that adds complexity. Most projects don't heavily use symlinks.

---

### 6. File Permission Tracking
**Location**: `crates/core/src/lib.rs` - `TreeEntry::Blob`

**Issue**: File permissions (executable bit, etc.) are not tracked.

**What's Needed**:
- Add `mode` field to `TreeEntry::Blob`
- Capture permissions during `add`
- Restore permissions during `checkout`

**Why Deferred**: Not critical for basic VCS functionality. Can be added later without breaking changes.

---

### 7. Sparse Checkout
**Location**: `crates/core/src/checkout.rs`

**Issue**: Always checks out entire tree. No support for partial checkouts.

**What's Needed**:
- Sparse checkout patterns
- Selective tree restoration
- Index awareness of sparse state

**Why Deferred**: Advanced feature not needed for MVP.

---

### 8. Reflog
**Location**: `crates/core/src/refs.rs`

**Issue**: No history of ref updates (reflog).

**What's Needed**:
- Log all ref updates with timestamps
- `sdal reflog` command
- Garbage collection awareness

**Why Deferred**: Safety net feature that can be added later.

---

### 9. Pack Files
**Location**: `crates/storage/src/lib.rs`

**Issue**: Each object is stored as a separate file. No packing for efficiency.

**What's Needed**:
- Pack file format
- Garbage collection
- Network transfer optimization

**Why Deferred**: Performance optimization, not a correctness issue.

---

### 10. Content-Aware Merging
**Location**: `crates/core/src/merge.rs`

**Issue**: Merge is purely tree-based (3-way merge of hashes). No line-level merging.

**What's Needed**:
- Diff algorithm
- Line-by-line 3-way merge
- Hunk-level conflict detection

**Why Deferred**: Complex feature. Current hash-based merge is correct, just conservative.

---

## Cleanup Tasks

### 11. Remove Unused Imports
**Locations**: Multiple files

**Files with unused imports**:
- `crates/core/src/checkout.rs` - `PathBuf`
- `crates/core/src/index.rs` - `PathBuf`
- `crates/core/src/refs.rs` - `Context`, `Write`, `self`
- `crates/core/src/workdir.rs` - `PathBuf`, `Context`
- `crates/cli/src/main.rs` - `Chunker`, `FixedSizeChunker`, `workdir`

**Why Deferred**: Warnings only, not errors. Can be cleaned up in batch.

---

### 12. Remove Unused Variables
**Location**: `crates/cli/src/main.rs`

**Variables**:
- Line 482: `path` in file iteration
- Line 875: `ours_commit`, `theirs_commit` in merge conflict handling (placeholders for future conflict file writing)

**Why Deferred**: Warnings only. Some are placeholders for future implementation.

---

## Notes

- This list should be reviewed periodically and updated as issues are addressed
- High priority items should be tackled before adding new features
- Medium/Low priority items can be addressed based on user needs
- Cleanup tasks can be done in batch during refactoring sessions
