# SDAL — Networking Migration Plan

This plan covers **this repository only** — the SDAL open-source client. SDAL Hub (proprietary server) is a completely separate application and is not part of this codebase.

---

## Scope

| This Repo (SDAL — Open Source) | Separate Repo (SDAL Hub — Proprietary) |
| :--- | :--- |
| CLI binary (`sdal push`, `fetch`, `clone`, etc.) | Server application |
| Client-side protocol logic & graph resolution | Repository registry & hosting |
| `Transport` trait + HTTP transport impl | Global chunk store |
| Protocol types (request/response structs) | Multi-user / organization management |
| `wire.rs` binary codec (public protocol format) | Server-side authentication & authorization |
| `identity.rs` Ed25519 signing (client-side) | Policy engine backend |
| `sdal-policy` shared types | Branch protection, audit logs, web API |

**The protocol is the contract between SDAL and SDAL Hub.** Protocol types, wire format, and crypto requirements are public. Hub implementation is proprietary.

---

## 1. Current State vs. Architecture Spec

| Area | Architecture Spec | Current Code | Status |
| :--- | :--- | :--- | :--- |
| **Product Separation** | SDAL and Hub are separate apps | `server.rs` and `sdal serve` implement server logic inside this repo | ⚠️ Needs cleanup |
| **Protocol** | 2-Phase: metadata discovery → chunk transfer | Monolithic: single request carries all objects + chunk bytes | ❌ Not yet |
| **Chunk Negotiation** | Client sends `have_chunks` for dedup | `have_chunks` always empty → full fetch every time | ❌ Not yet |
| **Wire Streaming** | Binary frames (`wire.rs`) with bounded RAM | `wire.rs` exists but unused; full JSON buffers in memory | ❌ Not yet |
| **Identity & Auth** | Ed25519 signed envelopes on every request | Signed envelopes implemented on client side | ✅ Done |
| **Partial Clone** | `--filter src/` via subtree-scoped metadata | Not implemented | ❌ Not yet |
| **Resumable Downloads** | Resume by sending existing chunk hashes | Falls out of `have_chunks` implementation | ❌ Not yet |

---

## 2. Implementation Phases

### Phase 0: Codebase Cleanup ⬅️ DO FIRST
> **Goal**: Remove server-side code that belongs in SDAL Hub.

- [x] **Remove `server.rs`**: Delete `crates/network/src/server.rs` and remove `pub mod server` from `lib.rs`.
- [x] **Remove `sdal serve` CLI command**: Delete the `Serve` variant from `Commands` enum and its handler in `main.rs`.
- [x] **Remove server-side protocol handlers**: Move `handle_fetch`, `handle_push`, `list_refs` out of `protocol.rs`. Keep only protocol types (structs) and any shared logic.
- [x] **Clean up dependencies**: Remove `axum` from `sdal-network` Cargo.toml (it's a server framework — belongs in Hub). Remove `sdal-policy` server-side usage.

---

### Phase 1: Authentication ✅ COMPLETE
> Client-side signing is done.

- [x] All client functions (`push`, `fetch`, `fetch_refs`) wrap payloads in Ed25519 `SignedEnvelope`.
- [x] Global identity auto-generated on `sdal init` at `~/.sdal/identity/`.

---

### Phase 2: 2-Phase Client Protocol ✅ COMPLETE
> **Goal**: Implement the 2-phase sync protocol on the client side.

- [x] **Phase 1 — Metadata Fetch**: Client requests refs + commit graph + trees + blob metadata (no chunk bytes).
- [x] **Phase 2 — Chunk Fetch**: Client determines missing chunk hashes locally, then requests only those chunks.
- [x] **Client Graph Resolution**: Walk local CAS after receiving metadata to compute accurate `have_chunks`.

---

### Phase 3: Binary Wire Streaming ✅ COMPLETE
> **Goal**: Stream chunks with bounded memory using `wire.rs` frames.

- [x] **Stream Transport Trait**: Refactor `Transport` from `fn post(path, Vec<u8>) -> Vec<u8>` to stream-based reader/writer.
- [x] **Wire Integration**: Encode objects as `FrameType::Object` and chunks as `FrameType::Chunk`, terminated by `FrameType::End`.

---

### Phase 4: Partial Clone & Resumable Downloads ✅ COMPLETE
> **Goal**: Advanced client features enabled by 2-phase protocol.

- [x] **Partial Clone (`--filter <path>`)**: Request metadata for full tree but fetch chunks only under specified subtrees.
- [x] **Resumable Downloads**: On interrupted transfers, resume by comparing local chunk hashes against required hashes. (Inherently supported by Phase 2 graph walk + atomic CAS).

---

### Phase 5: P2P Transport ✅ COMPLETE
> **Goal**: Direct peer-to-peer sync without any server.

- [x] **P2P Transport Implementation**: Implement `P2pTransport` using the same 2-phase protocol over direct peer connections (via `sdalp://` protocol).
- [x] **Multi-Peer Downloads**: Client connects to peers via TCP using `sdal peer-serve` and streams chunks directly.

---

### Phase 6: Native Pull Requests ✅ COMPLETE
> **Goal**: Local code review workflows.

- [x] **PullRequest Object**: Extend `Object` enum to include `PullRequest` storing PR metadata.
- [x] **PR CLI Subcommands**: Implement `sdal pr create`, `sdal pr list`, `sdal pr merge`.
- [x] **PR P2P Synchronization**: Ensure graph resolution logic in `client.rs` and `p2p.rs` parses PR metadata correctly for transfers.

---

### Phase 7: Garbage Collection ✅ COMPLETE
> **Goal**: Disk space management.

- [x] **Garbage Collector CLI (`sdal gc`)**: Command to locate active roots (branches, PRs, checkpoints, HEAD).
- [x] **Reachability Traversal**: Traverses all reachable commit graphs, trees, and chunk hashes.
- [x] **CAS Pruning**: Safely removes unvisited files from `.sdal/objects/`.

---

## 3. Testing Strategy

1. **Auth Tests**: Verify signed envelope creation, serialization, and rejection of tampered/expired envelopes.
2. **2-Phase Protocol Tests**: Verify client correctly separates metadata fetch from chunk fetch.
3. **Memory Benchmarks**: Verify push/fetch of multi-GB files stays bounded at ~1-2 MB via wire frame streaming.
4. **Integration Tests**: End-to-end push/fetch/clone against a test Hub instance.
