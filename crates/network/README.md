# SDAL Networking Layer (`sdal-network`)

The `sdal-network` crate is the distributed synchronization engine for the SDAL Version Control System. It is designed to be a highly resilient, stateless, and zero-trust protocol. 

Unlike traditional Git network protocols (which often rely heavily on complex stateful SSH connections or specialized "smart HTTP" negotiations), SDAL networking is built around modern async primitives (`tokio`), stateless HTTP REST (`axum` / `reqwest`), and cryptography (`ed25519-dalek`).

## 🧱 Core Architectural Principles

1. **Stateless Server Design:**
   The server holds zero repository data or user sessions in memory. Every request (`/push`, `/fetch`) independently opens the local storage, streams the exact required chunks, and closes the connection. This allows a single, low-resource server to handle thousands of concurrent synchronization requests.
2. **Zero-Trust Verification:**
   Neither the client nor the server trusts each other. The client cryptographically signs requests with an Ed25519 private key. When sending or receiving blobs/chunks, *both* ends of the connection re-calculate the SHA-256 hash of the incoming bytes. If a hash mismatch is detected, the transaction is immediately aborted.
3. **Memory-Safe Binary Streaming:**
   Large files are chunked before hitting the network. We do not load multi-gigabyte JSON payloads into RAM. The `wire.rs` module implements a custom `[type][size][data]` binary framing protocol with a hardcoded `64MB` safety cap to prevent Memory Exhaustion / OOM attacks.
4. **Transport Agnosticism:**
   The core negotiation logic (`protocol.rs`) does not know about HTTP. It operates over a generic `Transport` trait, making it trivial to add `P2P` or `SSH` transports in the future without rewriting any synchronization logic.

---

## 📂 Module Breakdown

### `server.rs` (The Backend API)
An `axum` powered asynchronous HTTP server. It provides 4 primary routes:
* `GET /health`: Uptime monitoring.
* `GET /refs`: Returns the current branch mappings (e.g., what commit `main` points to).
* `POST /fetch`: Receives a `FetchRequest`, walks the commit graph, and streams back the requested `[TransferObject]`s.
* `POST /push`: Receives a `PushRequest`, validates signatures, verifies payload hashes, streams chunks directly to disk (`storage.put`), and finally updates local branch refs.

### `client.rs` (The Consumer API)
Translates high-level CLI commands into transport calls.
* **`fetch()`**: Queries the server for missing objects, locally computes the SHA-256 hash of every received blob to verify network integrity, and saves them to local storage.
* **`push()`**: Walks the local commit graph from the `HEAD` down, collects all required Tree/Blob/Chunk objects, and dispatches them to the server.

### `protocol.rs` (The Business Logic)
The brains of the operation. This module defines the `PushRequest` and `FetchRequest` JSON envelopes. It handles the deep object-graph traversal required to figure out which chunks belong to which trees, and which trees belong to which commits.

### `identity.rs` (Cryptographic Auth)
Handles all Authentication. Users are identified by their **Ed25519 Public Keys** (No passwords!). 
When a client makes a push request, it creates a `SignedEnvelope` containing:
1. The JSON payload.
2. A random `nonce`.
3. A `timestamp`.
4. A cryptographic signature of the above.
The server mathematically verifies the signature. The `nonce` and `timestamp` strictly prevent **Replay Attacks** (where an attacker intercepts a valid push request and attempts to send it again later).

### `wire.rs` (Binary Codec)
A low-level byte codec. It breaks outgoing streams into manageable `Frame`s (`[1 byte type][4 bytes size][N bytes payload]`). This ensures that a maliciously large request is aborted at the byte level before memory allocation occurs.

### `transport.rs` (Network Interface)
Defines the `Transport` trait. Currently implements `HttpTransport` using `reqwest` backed by `rustls` (ensuring we don't depend on system-level OpenSSL libraries, making the binary universally portable).

---

## 🌊 Flow Diagrams

### The Push Flow (`sdal push origin main`)
1. **Walk & Collect:** Client parses local `HEAD` and builds a list of all reachable Commits, Trees, Blobs, and Chunks.
2. **Serialize & Sign:** Client packages these into a `PushRequest`, adds a nonce/timestamp, and signs it via `identity.rs`.
3. **Transmit:** Dispatched via `HttpTransport::post`.
4. **Auth Check:** Server receives the envelope, validates the Ed25519 signature, checks timestamp limits, and hands off to `policy.rs`.
5. **Policy Check:** `policy::can_push()` verifies if the user's Public Key is permitted to write to the `main` branch.
6. **Hash Verification & Storage:** Server unrolls the payload. For every chunk, it calculates the SHA-256 hash of the raw bytes. If it matches the expected hash, it is streamed to `.sdal/objects/`.
7. **Ref Update:** Server updates its `refs/heads/main` file to point to the new commit hash.

### The Fetch Flow (`sdal fetch origin`)
1. **Ref Discovery:** Client requests `GET /refs` to discover the remote's `HEAD` hash.
2. **Negotiation:** Client sends a `FetchRequest` with `want: [HEAD_HASH]` and `have: [LOCAL_CHUNKS]` *(delta-negotiation implemented in Phase 2)*.
3. **Server Collection:** Server walks its graph starting from `HEAD_HASH`, filters out chunks the client already has, and replies with a `FetchResponse`.
4. **Client Verification:** Client receives the data, rigorously hashes every chunk to ensure the server hasn't been compromised, and writes to local disk.

---

## 🔮 Roadmap (Next Phases)
* **Smart Delta Negotiation:** Update the client `fetch` logic to properly populate the `have_chunks` array, ensuring we only download diffs rather than full repository clones.
* **Access Control Lists (ACLs):** Upgrade `policy.rs` from an "Open Policy" to a persistent JSON-backed rules engine mapped to user Public Keys.
* **P2P Transport Implementation:** Implement a `P2pTransport` using `libp2p` to enable true decentralized syncing without a central HTTP server.
