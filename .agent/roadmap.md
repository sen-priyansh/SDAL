# SDAL Roadmap & Remaining Tasks

This document tracks completed features and technical specifications for upcoming core VCS & P2P tasks.

---

## 🏗️ Architectural Realignment

- **SDAL Core (This Repository)**: Focuses on core VCS functionality, FastCDC chunking, CAS, DAG graphs, local performance, and **P2P peer-to-peer code sharing/cloning**.
- **Central SDAL Server (Separate Application)**: Multi-tenant repo management, enterprise ACLs, remote access policies, and web UI/API are built in a separate server repository.

---

## ✅ Completed Core Features

- [x] **Core CAS Engine**: SHA-256 content addressing for commits, trees, blobs, chunks.
- [x] **FastCDC Chunking**: Dynamic rolling-hash content-defined chunking.
- [x] **Branching & Checkout**: Full branch creation, switching, `reset --hard`, and `restore`.
- [x] **Checkpoint & Undo**: Transient working directory snapshots without creating commits.
- [x] **Merge & Conflict Resolution**: 3-way merge detection, conflict markers, and Ratatui TUI conflict resolution tool (`sdal mergetool`).
- [x] **Network Crate Core (`sdal-network`)**:
  - `wire.rs` binary codec with 64MB frame cap.
  - `identity.rs` Ed25519 payload signing with nonces and timestamps.
  - `transport.rs` HTTP transport client abstraction.
  - `protocol.rs` commit graph traversal & server hash validation.
- [x] **CLI Subcommands**: `clone`, `push`, `fetch`, `pull`, `remote` configuration.

---

## 🔮 Core Roadmap & Remaining Tasks

### 1. P2P Transport Layer (`sdal-network`)
- **Focus**: Enable direct peer-to-peer cloning and code sharing between developer machines without requiring a central server.
- **Task**: Implement a `P2pTransport` implementing the `Transport` trait (e.g. using `libp2p` or local network discovery) for direct peer sync.

### 2. Smart Delta Fetching (Negotiation)
- **Focus**: Bandwidth optimization for P2P and remote sync.
- **Task**: Implement client-side commit graph walking to send local chunk hashes (`have_chunks`) in `FetchRequest` so only missing objects/chunks are transferred.

### 3. Native Pull Requests (`sdal pr`)
- **Focus**: Local & P2P code review workflows.
- **Task**: Implement CLI subcommands:
  - `sdal pr create --title <t> --body <b> --target <branch>`
  - `sdal pr list`
  - `sdal pr merge <id>`
- Store PR metadata objects directly in CAS object storage.

### 4. Garbage Collection (`sdal gc`)
- **Focus**: Disk space management.
- **Task**: Implement a garbage collector command that traverses all branch heads, tags reachable objects/chunks, and deletes dangling or orphaned objects in `.sdal/objects/`.
