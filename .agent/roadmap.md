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
- [x] **2-Phase Protocol (Client-Side)**:
  - Splitted client fetch into Phase 1 (metadata discovery) and Phase 2 (chunk transfer).
  - Implemented client-side graph resolution to walk local commit graph and build accurate `have_chunks` set, ensuring only missing chunks are transferred.
- [x] **Binary Wire Streaming**:
  - Implemented memory-bounded transfers via `wire.rs` frames.
  - Added `post_stream` and `post_receive_stream` to `Transport` trait.
  - Client streaming: `push` uses a custom `PushStreamer` to incrementally yield frames; `fetch` reads frames dynamically, avoiding memory bloat for large clones.
- [x] **Partial Clone (`--filter`) & Resumable Downloads**:
  - Added `--filter` flag to `fetch`, `pull`, and `clone` CLI commands.
  - Client locally prunes `want_chunks` calculation based on the filter tree path.
  - Client automatically supports resumable chunk downloads thanks to phase 2 graph walking over local CAS storage state.
- [x] **P2P Transport (`sdalp://`)**:
  - Implemented `P2pTransport` allowing raw TCP socket synchronization.
  - Added `sdal peer-serve` CLI command to start a direct peer sync server.
  - Uses the same 2-phase protocol and `wire.rs` streaming, completely bypassing the HTTP Hub.

---

## 🔮 Remaining Tasks

- [x] **Native Pull Requests (`sdal pr`)**:
  - Implemented `PullRequest` object type in CAS.
  - Implemented `sdal pr create`, `list`, `merge` subcommands.
  - Allowed fetching/syncing PRs via P2P graph walk (`p2p.rs` and `client.rs`).

---

## 🔮 Remaining Tasks

- [x] **Garbage Collection (`sdal gc`)**:
  - Implemented `sdal gc` command.
  - Added full object traversal starting from branches, HEAD, PRs, and checkpoints.
  - Implemented cleanup of unreferenced objects and chunks from `.sdal/objects/`.
