# SDAL Ecosystem Architecture Specification

## Overview

The SDAL ecosystem is intentionally divided into two independent products:

1. **SDAL** (Open Source)
2. **SDAL Hub** (Proprietary)

Although they communicate using the same networking protocol, they are separate codebases with separate responsibilities.

The protocol, message formats, and networking behavior are publicly documented. The implementation of SDAL Hub is proprietary.

---

# Product Separation

## SDAL (Open Source)

SDAL is the client-side application and local version control system.

Responsibilities:

* Repository initialization
* Content-addressed storage
* Chunking
* Commits
* Trees
* Blobs
* Checkout
* Merge
* Diff
* Local repository management
* Push
* Pull
* Fetch
* Clone
* Repository reconstruction
* Cryptographic signing
* Chunk verification

SDAL is responsible for reconstructing repositories.

It never performs server-side policy management or organization management.

---

## SDAL Hub (Proprietary)

SDAL Hub is a completely separate application.

It is **not** part of the SDAL repository.

Responsibilities:

* Repository hosting
* Repository registry
* Multi-user management
* Organization management
* Authentication
* Authorization
* Policy enforcement
* Global chunk storage
* Repository metadata storage
* Branch protection
* Pull request metadata
* Audit logs
* Web API
* Future enterprise features

The Hub does **not** reconstruct repositories.

Its job is to securely provide metadata and chunks.

---

# Design Philosophy

The SDAL client always reconstructs repositories.

The Hub only serves authenticated metadata and chunk data.

This rule must never be violated.

Repository reconstruction logic should exist only once—in the SDAL client.

---

# High-Level Architecture

```text
                SDAL Ecosystem

        +--------------------------+
        |        SDAL CLI          |
        |--------------------------|
        | init                     |
        | commit                   |
        | checkout                 |
        | merge                    |
        | clone                    |
        | push                     |
        | pull                     |
        | fetch                    |
        | reconstruct repository   |
        +------------+-------------+
                     |
                SDAL Protocol
                     |
              HTTP / HTTPS
                     |
        +------------+-------------+
        |        SDAL Hub          |
        |--------------------------|
        | repository registry      |
        | metadata store           |
        | global chunk store       |
        | policy engine            |
        | authentication           |
        | organizations            |
        | branch protection        |
        | audit                    |
        +--------------------------+
```

---

# Storage Model

The Hub stores two independent categories of data.

## Repository Metadata

Stores repository information.

Examples:

* repository configuration
* HEAD
* branch references
* commit graph
* tree objects
* blob metadata
* repository permissions
* pull requests
* audit metadata

Repository metadata never stores raw chunk bytes.

---

## Global Chunk Store

Stores only chunk data.

Properties:

* content-addressed
* append-only
* globally deduplicated
* hash indexed

If identical chunks appear across multiple repositories, only one physical copy is stored.

Repository metadata references chunk hashes.

---

# Repository Model

```text
Repository

│

├── HEAD

├── refs

├── settings

├── permissions

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

Chunk Hashes

│

▼

Global Chunk Store
```

The repository contains references.

The chunk store contains data.

---

# Clone Flow

Client executes:

```bash
sdal clone <repository>
```

Workflow:

1. Request repository metadata.
2. Receive refs, commits, trees and blob metadata.
3. Determine required chunk hashes.
4. Request required chunks.
5. Verify every chunk hash.
6. Reconstruct blobs.
7. Reconstruct files.
8. Checkout repository.

The Hub never reconstructs files.

---

# Fetch Flow

The client first requests metadata.

Metadata includes:

* refs
* commits
* trees
* blob metadata

The client compares this information with its local repository.

It determines which chunks are already available.

It requests only missing chunks.

The Hub streams those chunks.

The client reconstructs the repository.

---

# Push Flow

Client:

* signs request
* sends metadata
* sends missing chunks

Hub:

* authenticates client
* validates permissions
* stores only missing chunks
* updates metadata
* updates branch references

Duplicate chunks are never stored twice.

---

# Partial Clone

The client may request only a subset of the repository.

Example:

```bash
sdal clone --filter src/
```

The Hub:

* resolves the requested subtree
* determines required blob metadata
* determines required chunk hashes
* streams only those chunks

The client reconstructs only the requested portion.

---

# Resumable Downloads

Because storage is content-addressed:

The client knows which chunks it already possesses.

Interrupted downloads simply resume by requesting only missing chunk hashes.

No separate resume mechanism is required.

---

# Protocol Design

The networking protocol is divided into two phases.

## Phase 1 — Metadata Discovery

Transfer:

* refs
* commit graph
* trees
* blob metadata

No chunk bytes are transferred.

The client now knows exactly which chunk hashes are required.

---

## Phase 2 — Chunk Transfer

The client requests required chunk hashes.

The Hub streams those chunks.

The client verifies hashes and reconstructs the repository locally.

---

# Authentication

Authentication uses Ed25519 public/private key cryptography.

Every request contains:

* public key
* signature
* timestamp
* nonce

The Hub verifies:

* signature
* timestamp
* nonce

Only then are repository operations allowed.

No passwords are used.

---

# Policy

Before every repository operation:

1. Authenticate request.
2. Resolve repository.
3. Evaluate policy.
4. Execute operation.

Policy determines permissions such as:

* read
* push
* merge
* administration

---

# Transport Independence

The SDAL protocol is independent of HTTP.

Although SDAL Hub initially uses HTTP/HTTPS, the protocol is designed so future transports can reuse the exact same synchronization model.

Examples include:

* custom peer-to-peer transport
* LAN synchronization
* offline bundle transfer

The client reconstruction algorithm never changes.

---

# Public vs Proprietary Boundary

The following are public and documented:

* SDAL protocol specification
* request/response behavior
* wire format
* cryptographic requirements
* synchronization flow

The following are proprietary and implemented only inside SDAL Hub:

* Hub server application
* repository registry
* global chunk storage implementation
* metadata management implementation
* policy backend
* organization management
* web API implementation
* enterprise features

This allows SDAL to remain open while SDAL Hub provides the official hosted and self-hosted server implementation.

---

# Architectural Principles

1. SDAL and SDAL Hub are separate applications.
2. The protocol is the contract between them.
3. The client always reconstructs repositories.
4. The Hub only serves authenticated metadata and chunk data.
5. Metadata and chunk storage remain logically separate.
6. Storage is globally deduplicated.
7. Authentication is cryptographic.
8. Authorization always occurs before storage access.
9. The protocol remains transport-independent.
10. Future features must extend this architecture rather than replace it.
