# SDAL Roadmap

This document outlines the strategic plan to evolve SDAL from a concept into a production-ready Sovereign Decentralized Asset Ledger.

## Phase 1: The Foundation (The "Walking Skeleton")
**Goal:** A working Content-Addressable Storage (CAS) system that can ingest files, chunk them, and retrieve them.

- [ ] **Chunking Engine (`crates/chunking`)**
    - Implement FastCDC (Content-Defined Chunking).
    - Support static chunking for fallback.
- [ ] **Storage Backend (`crates/storage`)**
    - Implement a local filesystem backend (similar to `.git/objects`).
    - Implement a flat-file storage for chunks.
- [ ] **Core DAG (`crates/core`)**
    - Define `Blob`, `Tree`, and `Commit` structs.
    - Implement Merkle Tree construction from chunks.
- [ ] **CLI v0.1**
    - `sdal init`: Initialize a repo.
    - `sdal add <file>`: Chunk and store a file.
    - `sdal cat <hash>`: Retrieve and reconstruct a file.


## Phase 1.5: Internal Guarantees
**Goal:** Not user-visible, but critical. Ensure the system fails loudly and safely.

- [ ] **Invariants Enforcement**
    - Assert immutability assumptions.
    - Validate DAG correctness.
- [ ] **Fail-Stop**
    - Refuse corrupted states early.
    - Panic loudly on invariant violations.

## Phase 2: Version Control Primitives
**Goal:** A usable VCS that can track history and switch between states.

- [ ] **Commit Graph**
    - Implement parent pointers in commits.
    - Implement `HEAD` and branch references (`refs/heads/main`).
- [ ] **Working Directory**
    - Implement `checkout`: Reconstruct a full directory tree from the DAG.
    - Implement `status`: Detect changes between working dir and HEAD.
- [ ] **Index/Staging Area**
    - Design a mechanism to stage changes before committing.

## Phase 3: The SDAL Differentiators (Manifesto Realization)
**Goal:** Implement the features that make SDAL unique and superior to Git for assets.

- [ ] **Global Static Files**
    - Implement the "Policy" engine (`crates/policy`).
    - Create a manifest for static files (e.g., `SDAL.policy`).
    - Enforce immutability of these files across branches.
- [ ] **Scoped Branches**
    - Implement "Partial Checkout" logic.
    - Allow branches to only track a subtree of the DAG.
- [ ] **Binary Patching**
    - Implement delta compression for chunks to optimize storage for large binaries.

## Phase 4: Collaboration & Security
**Goal:** Enable team workflows and enforce security.

- [ ] **Networking**
    - Design a sync protocol (push/pull).
    - Implement a remote backend (HTTP/gRPC).
- [ ] **Permissions**
    - Implement file-level Access Control Lists (ACLs).
    - Enforce "Who can touch what" at the commit level.
- [ ] **Ledger Mode (Optional)**
    - Implement a cryptographic append-only log.
    - (Optional) Integration with a transparency log or blockchain.

## Phase 5: Ecosystem & Polish
**Goal:** Production readiness and developer experience.

- [ ] **FUSE Filesystem**
    - Mount an SDAL repo as a virtual filesystem (lazy loading assets).
- [ ] **GUI Tools**
    - Visual graph explorer.
- [ ] **LFS Migration Tool**
    - Import existing Git LFS repositories into SDAL.
