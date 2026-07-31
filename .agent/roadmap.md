# SDAL Roadmap & Remaining Tasks

This document tracks completed features and technical specifications for upcoming tasks in the **SDAL (open source)** repository.

SDAL Hub (proprietary server) is a separate application and is not tracked here.

---

## 🏗️ Product Separation

- **SDAL (This Repository)**: Open-source client-side VCS — repository management, chunking, commits, checkout, merge, push, pull, fetch, clone, repository reconstruction, cryptographic signing, chunk verification.
- **SDAL Hub (Separate Repository)**: Proprietary server — repository hosting, registry, global chunk store, multi-user management, authentication, authorization, policy enforcement, web API, enterprise features.
- **Protocol**: The public contract between SDAL and SDAL Hub. Protocol spec, wire format, and crypto requirements are documented in [`architecture.md`](./architecture.md).

---

## ✅ Completed Features

- [x] **Core CAS Engine**: SHA-256 content addressing for commits, trees, blobs, chunks.
- [x] **FastCDC Chunking**: Dynamic rolling-hash content-defined chunking with streaming support.
- [x] **Branching & Checkout**: Full branch creation, switching, `reset --hard`, and `restore`.
- [x] **Checkpoint & Undo**: Transient working directory snapshots without creating commits.
- [x] **Merge & Conflict Resolution**: 3-way merge detection, conflict markers, and Ratatui TUI conflict resolution tool (`sdal mergetool`).
- [x] **Network Crate (`sdal-network`)**:
  - `wire.rs` — binary frame codec with 64MB cap.
  - `identity.rs` — Ed25519 payload signing, key generation, envelope verification.
  - `transport.rs` — Transport trait abstraction + HTTP implementation.
  - `protocol.rs` — protocol types (FetchRequest, PushRequest, etc.) and commit graph traversal.
  - `client.rs` — client-side push/fetch/clone logic with signed envelopes.
- [x] **CLI Subcommands**: `init`, `add`, `commit`, `log`, `status`, `reset`, `restore`, `branch`, `checkout`, `merge`, `mergetool`, `checkpoint`, `save`, `undo`, `template`, `clone`, `push`, `fetch`, `pull`, `remote`.
- [x] **Authenticated Networking**:
  - All client requests (`push`, `fetch`, `fetch_refs`) wrapped in Ed25519 `SignedEnvelope`.
  - Global identity auto-generated on `sdal init` and stored at `~/.sdal/identity/`.

---

## 🔧 Codebase Cleanup Required

- [x] **Remove `server.rs` and `sdal serve`**: The `server.rs` file in `crates/network/` and the `Serve` CLI command implement server-side functionality that belongs in SDAL Hub. These should be removed from this repository.
- [x] **Remove server-side protocol handlers**: The `handle_fetch`, `handle_push`, and `list_refs` functions in `protocol.rs` are server-side logic. Protocol types (structs) should stay; server handlers should be removed.
- [x] **Clean up dependencies**: Policy enforcement is Hub-side. This crate should only contain shared types/primitives that both SDAL and the Hub can use. Axum is also removed.

- [x] **2-Phase Protocol (Client-Side)**:
  - Splitted client fetch into Phase 1 (metadata discovery) and Phase 2 (chunk transfer).
  - Implemented client-side graph resolution to walk local commit graph and build accurate `have_chunks` set, ensuring only missing chunks are transferred.

---

## 🔮 Remaining Tasks

### 1. Binary Wire Streaming
- **Focus**: Memory-bounded transfers using `wire.rs` frames.
- **Task**: Refactor `Transport` trait to support stream-based I/O. Client should stream `wire::Frame` sequences to/from the network without buffering entire payloads in memory.

### 4. Partial Clone (`--filter`)
- **Focus**: Allow cloning subsets of a repository.
- **Task**: Client requests metadata for full tree but only fetches chunks under specified subtrees.

### 5. P2P Transport
- **Focus**: Direct peer-to-peer transfers without any server.
- **Task**: Implement a `P2pTransport` using the same 2-phase protocol over direct peer connections.

### 6. Native Pull Requests (`sdal pr`)
- **Focus**: Local code review workflows.
- **Task**: Implement CLI subcommands (`sdal pr create`, `list`, `merge`). Store PR metadata objects in CAS.

### 7. Garbage Collection (`sdal gc`)
- **Focus**: Disk space management.
- **Task**: Traverse all branch heads, tag reachable objects/chunks, delete dangling objects in `.sdal/objects/`.
