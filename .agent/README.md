# SDAL Agent Context & Project Overview

This directory (`.agent/`) contains canonical technical documentation, architecture specifications, and roadmap details for AI coding assistants working on the SDAL codebase.

---

## 🚀 System Quick-Start & Core Design Philosophy

**SDAL** is an open-source, client-first version control system and data synchronization engine written in Rust. It communicates with **SDAL Hub** (a separate, proprietary server application) via a publicly documented protocol.

### Key Rule

**SDAL always reconstructs repositories. The Hub only serves authenticated metadata and chunks. This rule must never be violated.**

### Canonical Architectural Principles
1. **SDAL and SDAL Hub are separate applications.** The protocol is the contract between them.
2. **Client-First Reconstruction**: The client owns repository reconstruction. Reconstruction logic exists only once — in SDAL.
3. **Hub Role**: The Hub serves authenticated metadata and chunk data. It never reconstructs files.
4. **Transport Independence**: Protocol operates identically over HTTP/HTTPS, and is designed for future transports (P2P, LAN, offline bundles).
5. **Globally Deduplicated Storage**: Content-addressed append-only chunk storage with SHA-256 hash verification.
6. **Cryptographic Identity**: Ed25519 public/private key signed requests with timestamp and nonce anti-replay protection.
7. **Public Protocol, Proprietary Hub**: The protocol spec, wire format, and crypto requirements are public. The Hub implementation is proprietary.

---

## 📦 What This Repository Contains

This is the **SDAL (Open Source)** repository. It contains only the client-side application and local VCS.

```
SDAL/
├── Cargo.toml                  # Workspace dependencies & profile configs
├── .agent/                     # AI Context & Documentation (YOU ARE HERE)
│   ├── README.md               # Index & Quick Start
│   ├── architecture.md         # Canonical SDAL Ecosystem Architecture Specification
│   ├── roadmap.md              # Completed features & remaining tasks
│   └── plan.md                 # Networking migration plan (gap analysis + phased execution)
└── crates/
    ├── core/                   # Object models (Commit, Tree, Blob, Object), Refs, Index, Ignore, Merge, Checkout
    ├── storage/                # Storage trait & FilesystemStorage (.sdal/objects/xx/yyyy...)
    ├── chunking/               # FastCDC chunker implementation
    ├── checkpoint/             # Working directory snapshots / undo history
    ├── network/                # Client protocol logic, Transport trait, wire codec, identity signing
    ├── policy/                 # Authorization primitives (shared types — enforcement is Hub-side)
    ├── tui/                    # Terminal UI (Ratatui) for merge conflict resolution
    └── cli/                    # `sdal` binary entry point (Clap CLI commands & execution)
```

### What This Repo Does NOT Contain

The following belong to **SDAL Hub** (separate proprietary repository):

* Hub server application
* Repository registry & hosting
* Global chunk store implementation
* Multi-user / organization management
* Server-side authentication & authorization
* Policy engine backend
* Branch protection, pull request metadata, audit logs
* Web API, enterprise features

---

## 📚 Agent File Index

- [`architecture.md`](./architecture.md): **Canonical SDAL Ecosystem Architecture Specification** — defines product separation, protocol design, storage model, and architectural principles.
- [`roadmap.md`](./roadmap.md): Status of implemented features and technical specs for remaining tasks.
- [`plan.md`](./plan.md): **Networking Migration Plan** — gap analysis and phased execution plan for aligning this codebase with the architecture spec.
- [`hub-spec.md`](./hub-spec.md): **SDAL Hub Server Guide** — implementation details, endpoints, and authentication spec for the proprietary Hub codebase.
