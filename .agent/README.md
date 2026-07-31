# SDAL Agent Context & Project Overview

This directory (`.agent/`) contains canonical technical documentation, architecture specifications, data models, and roadmap details for AI coding assistants working on the SDAL codebase.

---

## 🚀 System Quick-Start & Core Design Philosophy

**SDAL** is a high-performance, client-first, protocol-driven version control system (VCS) and data synchronization engine written in Rust.

### Canonical Architectural Principles
1. **Client-First Reconstruction**: The client owns repository reconstruction. The server/peer **never reconstructs files** or generates a working directory.
2. **Server/Hub Role**: Servers and Hubs serve authenticated metadata and chunk data, enforce policy, and log actions—never reconstructing files.
3. **Transport Independence**: Protocol operates identically over HTTP, SSH, SDALP (P2P), LAN discovery, or offline bundle transfers.
4. **Globally Deduplicated Storage**: Content-addressed append-only chunk storage with SHA-256 hash verification on every chunk.
5. **Cryptographic Identity & Policy**: Ed25519 public/private key signed envelopes with timestamp and nonce anti-replay protection.

---

## 📦 Workspace Architecture & Workspace Crates

The core SDAL codebase is structured as a Rust Cargo Workspace:

```
SDAL/
├── Cargo.toml                  # Workspace dependencies & profile configs
├── .agent/                     # AI Context & Documentation (YOU ARE HERE)
│   ├── README.md               # Index & Quick Start
│   ├── architecture.md         # Full Canonical Architecture & Protocol Spec
│   └── roadmap.md              # Remaining Tasks & Core Roadmap
└── crates/
    ├── core/                   # Object models (Commit, Tree, Blob, Object), Refs, Index, Ignore, Merge, Checkout
    ├── storage/                # Storage trait & FilesystemStorage (.sdal/objects/xx/yyyy...)
    ├── chunking/               # FastCDC chunker implementation
    ├── checkpoint/             # Working directory snapshots / undo history
    ├── network/                # Binary wire codec, Transport abstractions, P2P & local peer sync logic
    ├── policy/                 # Action authorization primitives & policy scaffolding
    ├── tui/                    # Terminal UI (Ratatui) for merge conflict resolution
    └── cli/                    # `sdal` binary entry point (Clap CLI commands & execution)
```

---

## 📚 Agent File Index

- [`architecture.md`](./architecture.md): **Canonical SDAL Networking & Storage Architecture Specification**.
- [`roadmap.md`](./roadmap.md): Status of implemented features and detailed technical specifications for upcoming tasks.
