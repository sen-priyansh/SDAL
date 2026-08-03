# SDAL — Networking Migration Plan

See [`.agent/plan.md`](./.agent/plan.md) for the full plan.

> **SDAL** (this repo) is the open-source client. **SDAL Hub** is a completely separate proprietary application. The protocol is the public contract between them.

## Phases

0. **Cleanup** ✅ — Remove server-side code (`server.rs`, `sdal serve`, server protocol handlers, `axum`) that belongs in Hub
1. **Authentication** ✅ — Client-side Ed25519 signed envelopes
2. **2-Phase Protocol** ✅ — Separate metadata discovery from chunk transfer (client-side)
3. **Wire Streaming** ✅ — Stream `wire.rs` frames with bounded RAM
4. **Partial Clone & Resume** ✅ — `--filter`, resumable downloads
5. **P2P Transport** ✅ — Direct peer-to-peer sync
6. **Native Pull Requests** ✅ — Native PR CLI commands and PR object natively stored in CAS.
7. **Garbage Collection** ✅ — Native `sdal gc` command for memory management.
