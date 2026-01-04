# SDAL
## Sovereign Decentralized Asset Ledger

SDAL is a version control system for assets, not just source code. It is designed to track, verify, and preserve the full reality of a project: source code, binaries, build outputs, game assets, 3D models, AI models, and configuration rules.

### Core Philosophy
- **Data-First**: Everything is data. Files are broken into content-defined chunks.
- **Strict by Choice**: Free by default, but can enforce strict correctness when asked.
- **Change Mapping**: Maps changes inside binaries, not just text.
- **Global Static Files**: Enforce shared reality across branches.
- **Scoped Branches**: Branches can be scoped to specific directories.

### Structure

- `crates/core`: Merkle DAG, objects, commits
- `crates/storage`: Chunk storage (fs backend)
- `crates/chunking`: CDC logic
- `crates/policy`: Static files, scopes
- `crates/cli`: Command-line interface

See [docs/MANIFESTO.md](docs/MANIFESTO.md) for the full philosophy and [docs/INVARIANTS.md](docs/INVARIANTS.md) for core invariants.
