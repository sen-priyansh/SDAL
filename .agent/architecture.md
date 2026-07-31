# SDAL Networking & Storage Architecture

## Overview

SDAL is designed around a **client-first, protocol-driven architecture**. The client is responsible for reconstructing repositories, while servers (or peers) are responsible only for serving authenticated metadata and chunk data. This separation allows the same protocol to work across cloud servers, self-hosted instances, local office deployments, peer-to-peer networking, and even offline bundle transfers.

The architecture is intentionally transport-independent. HTTP is the first transport implementation, but the protocol is designed so that SSH, SDALP (native peer protocol), LAN discovery, or offline bundle transfer can all use the exact same synchronization logic.

---

# Core Design Philosophy

The SDAL client owns repository reconstruction.

The server **never reconstructs files** and never generates a working directory.

Instead, the server:

* authenticates requests
* enforces repository policies
* resolves metadata
* streams required chunks

The client:

* downloads metadata
* determines missing objects
* downloads missing chunks
* verifies hashes
* reconstructs blobs
* reconstructs files
* updates the working tree

This makes every transport behave identically.

---

# High-Level Architecture

```text
                    SDAL Ecosystem

             +-----------------------+
             |      SDAL CLI         |
             |-----------------------|
             | init                  |
             | commit                |
             | checkout              |
             | merge                 |
             | push/pull             |
             | clone                 |
             +-----------+-----------+
                         |
                    SDAL Protocol
                         |
          +--------------+--------------+
          |                             |
   +------+-------+             +-------+------+
   |   SDAL Hub   |             |  SDAL Peer   |
   | (HTTP Server)|             | (Future)     |
   +--------------+             +--------------+
```

The protocol remains identical regardless of transport.

---

# Components

## 1. SDAL CLI

Responsibilities:

* local repository management
* commits
* checkout
* merge
* chunk generation
* blob reconstruction
* repository reconstruction
* request signing
* hash verification

The CLI should **never contain organization management or server-side policy logic.**

---

## 2. SDAL Hub

A dedicated server implementation.

Responsibilities:

* repository registry
* repository metadata
* global chunk storage
* user identity verification
* policy enforcement
* branch protection
* pull request metadata
* audit logs
* hooks
* organization management

The Hub is a data service, not a repository reconstruction engine.

---

## 3. SDAL Peer (Future)

A lightweight node implementing the same protocol.

Responsibilities:

* authenticate peers
* advertise repositories
* exchange metadata
* exchange chunks

Unlike SDAL Hub, peers do not require organization or multi-user management.

---

# Storage Model

The server stores two logically separate datasets.

## Repository Metadata Store

Stores information describing repositories.

Examples:

* repository information
* HEAD
* branch references
* commit graph
* trees
* blob metadata
* permissions
* pull requests
* repository configuration

This metadata references chunk hashes but does not own chunk data.

---

## Global Chunk Store

Stores chunk data only.

Properties:

* content-addressed
* append-only
* globally deduplicated
* hash indexed

Example:

```text
Chunk Store

hashA
hashB
hashC
hashD
```

If two repositories contain identical chunks, only one physical copy exists.

---

# Data Relationship

```text
Repository
    │
    ├── refs
    ├── HEAD
    ├── permissions
    └── settings
          │
          ▼
      Commit Graph
          │
          ▼
         Trees
          │
          ▼
         Blobs
          │
          ▼
         Chunks
```

Repositories reference metadata.

Metadata references blobs.

Blobs reference chunk hashes.

Chunks store the actual bytes.

---

# Clone Flow

Client executes:

```bash
sdal clone <url>
```

Workflow:

1. Request repository metadata.
2. Receive refs, commits, trees and blob metadata.
3. Determine required chunk hashes.
4. Request only required chunks.
5. Verify every chunk hash.
6. Reconstruct blobs.
7. Reconstruct files.
8. Checkout repository.

The server never reconstructs files.

---

# Push Flow

Client:

* determines new commits
* determines required blobs
* determines missing chunks
* signs request

Server:

* verifies identity
* validates policy
* checks existing chunks
* stores only missing chunks
* updates repository metadata
* updates branch references

---

# Fetch / Pull Flow

Client requests:

* latest refs
* required metadata

Server:

* resolves commit graph
* resolves trees
* resolves blob metadata

Client:

* determines missing chunks
* downloads only missing chunks
* reconstructs repository locally

Pull is simply:

Fetch

*

Checkout

---

# Partial Clone

Because metadata and chunks are separated:

Client can request only a subset of repository metadata.

Example:

```bash
sdal clone --filter src/
```

Server:

* resolves tree
* determines blobs under src/
* determines required chunks
* streams only those chunks

Client reconstructs only requested files.

---

# Resumable Downloads

Client maintains existing chunk hashes.

If download stops:

Client reconnects and sends:

Existing chunk hashes

Server only streams missing chunks.

No special resume protocol is necessary.

---

# P2P Synchronization

Exactly the same reconstruction process is used.

Example:

Peer A:

Chunk A

Chunk C

Peer B:

Chunk B

Peer C:

Chunk D

Client downloads chunks from whichever peers have them.

After collecting all required chunks, the client reconstructs the repository locally.

Peers never reconstruct repositories.

---

# Protocol Design

The protocol is divided into two phases.

## Phase 1 — Discovery

Transfer only metadata.

* refs
* commits
* trees
* blob metadata

The client now knows exactly which chunk hashes are required.

---

## Phase 2 — Data Transfer

Transfer only chunk data.

The client verifies every chunk.

The client reconstructs the repository.

This separation enables:

* partial clone
* resumable downloads
* multi-peer downloads
* prioritized downloads
* transport independence

---

# Identity

Authentication uses Ed25519 public/private key cryptography.

Every request is signed.

Server verifies:

* signature
* timestamp
* nonce

Identity is represented by public keys rather than usernames or passwords.

---

# Policy Layer

Before storage operations:

1. Verify identity.
2. Resolve repository.
3. Evaluate policy.
4. Execute operation.

Examples:

* can_read()
* can_push()
* can_merge()

Branch protection is implemented inside the policy engine.

---

# Multi-Repository Support

Repositories are identified using path routing.

Example:

```text
/owner/repository/...
```

Server maps this to repository metadata.

The server remains stateless.

---

# Future Extensions

The architecture is intentionally designed to support future additions without changing the protocol.

Examples:

* blockchain-backed audit layer
* enterprise policy modules
* distributed chunk replication
* storage engine improvements
* S3/object storage backend
* local office deployments
* air-gapped deployments
* SDAL Hub SaaS
* SDAL Peer networking

None of these require changes to the client reconstruction algorithm.

---

# Design Principles

1. Client reconstructs repositories.
2. Server serves authenticated metadata and chunks.
3. Protocol is transport independent.
4. Storage is globally deduplicated.
5. Identity is cryptographic.
6. Policy is enforced before storage access.
7. Metadata and chunk storage remain logically separate.
8. Every chunk is hash verified before reconstruction.
9. The protocol is stateless.
10. Every deployment (Hub, local server, peer, offline bundle) follows the same synchronization model.
