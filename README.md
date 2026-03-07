# SDAL

<p align="center">
  <img src="logo_assets/SDAL_logo.png" alt="SDAL Logo" width="200" />
</p>

## Sovereign Decentralized Asset Ledger

> **Version Control for Reality.**
> Track, verify, and preserve source code, binaries, game assets, and AI models with paranoid integrity.

---

SDAL is a distributed version control system designed for **assets**, not just text. While traditional VCS struggles with large binaries, SDAL treats everything as data, using content-defined chunking to efficiently deduplicate and store the full reality of your project.

### Core Philosophy: "Paranoid & Safe"

-   **Data-First**: Everything is a blob. We don't care if it's text or a 4GB texture.
-   **Strict Integrity**: Every byte is hashed. Every tree is verified. If the hash matches, the data is identical.
-   **Deterministic**: The same content always produces the same hash, everywhere.

---

## Key Features

### 📦 Content-Defined Chunking (FastCDC)
SDAL doesn't store files as monolithic blobs. It uses **FastCDC** to split files into content-aware chunks (Avg: 64KB).
-   **Why?** If you change one byte in a 100MB file, SDAL only stores the new ~64KB chunk. The rest is deduplicated.
-   [Read more about FastCDC](docs/fastcdc.md)

### 🌳 Merkle DAG Architecture
The entire history is a Directed Acyclic Graph of hashes.
-   **Commits** point to **Trees**.
-   **Trees** point to **Blobs** or sub-Trees.
-   **Blobs** point to **Chunks**.
-   [Read about the Architecture](docs/architecture.md)

### 👻 Ghost Checkpoints
Save your work without polluting your commit history.
-   **Problem**: "WIP" commits clutter the log.
-   **Solution**: `sdal checkpoint save`. Create local, ephemeral snapshots you can jump back to.
-   **Auto-Cleanup**: Committing automatically cleans up checkpoints, leaving a pristine history.

---

## Quick Start

### Installation

```bash
git clone https://github.com/sen-priyansh/SDAL.git
cd SDAL
cargo build --release
# Add target/release/sdal to your PATH
```

### Basic Usage

1.  **Initialize a Repository**
    ```bash
    sdal init
    ```

2.  **Track Files**
    ```bash
    sdal add .
    ```

3.  **Save a Checkpoint (Optional/WIP)**
    ```bash
    sdal checkpoint save "working on physics engine"
    ```

4.  **Confirm Changes**
    ```bash
    sdal status
    ```

5.  **Commit**
    ```bash
    sdal commit -m "Initial commit"
    ```

[View Full Command Reference](docs/commands.md)

---

## Documentation

-   [**Command Reference**](docs/commands.md): Detailed guide to CLI commands.
-   [**Architecture**](docs/architecture.md): Deep dive into the data structures.
-   [**FastCDC**](docs/fastcdc.md): How the chunking algorithm works.
-   [**Contributing**](CONTRIBUTING.md): How to get involved.
-   [**Code of Conduct**](CODE_OF_CONDUCT.md): Community standards.

---

## License

SDAL is licensed under the **Business Source License 1.1**.

-   **Non-Commercial Use**: You are free to use SDAL for personal, educational, research, or non-profit purposes.
-   **Commercial Use**: Use by for-profit entities (including internal business operations) requires a Commercial License.
-   **Change Date**: On **2030-01-01**, the license converts to **Apache 2.0**, making it fully open source.

See [LICENSE](LICENSE) for details.
