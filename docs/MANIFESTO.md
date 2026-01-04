# SDAL Manifesto
## Sovereign Decentralized Asset Ledger

### 1. What SDAL Is

SDAL is a version control system for assets, not just source code.

It is designed to track, verify, and preserve the full reality of a project:

- source code
- binaries
- build outputs
- game assets
- 3D models
- AI models
- configuration rules

SDAL treats all of these as first-class citizens, not exceptions.

It is not a better Git.
It is a different VCS with a different philosophy.

### 2. The Core Philosophy
Git trusts.
SDAL enforces — only when asked to.

SDAL is free by default and strict by choice.

- It does not assume users are malicious
- It does not assume users are perfect
- It allows projects to decide when correctness matters more than freedom

SDAL does not distrust by nature.
It distrusts only when explicitly instructed to.

### 3. Why SDAL Exists

Modern projects outgrew Git’s assumptions.

Git was designed for:
- text files
- small repositories
- human-centric workflows
- trust-based collaboration

Modern projects are:
- binary-heavy
- asset-driven
- machine-built
- security-sensitive
- long-lived

Git can store these things.
It cannot understand, map, or enforce correctness around them.

SDAL exists to solve that gap from the ground up.

### 4. Data-First, Format-Agnostic Design

SDAL is data-centric, not text-centric.

At its core, SDAL uses:
- content-addressable storage
- content-defined chunking
- a Merkle DAG

This means:
- Files are broken into chunks based on content patterns
- Chunks are hashed and stored once
- Files become graphs of chunks
- Commits become graphs of files

As a result:
- A 1% change in a 500MB binary stores ~1% new data
- Large files remain efficient
- History reflects actual change, not blind replacement
- File format does not matter

To SDAL:
Everything is data.

### 5. Change Mapping Is a First-Class Feature

SDAL can map change inside:
- executables
- DLLs
- AI weights
- assets
- generated files

This is non-negotiable.

Git fundamentally cannot do this due to its blob model.

SDAL’s change mapping alone:
- Enables proper binary history
- Reduces storage cost
- Enables future artifact tracing
- Removes reliance on external systems like LFS

Even without any other advanced features, this alone justifies SDAL.

### 6. Global Static Files (Shared Reality)

Some files are not experiments.
They are laws.

SDAL introduces Global Static Files:
- Files that exist outside branch isolation
- Files that must be identical everywhere

Examples:
- build rules
- engine configuration
- ABI definitions
- asset pipelines
- kernel interfaces

When a file is marked static:
- It cannot diverge per branch
- It cannot be modified accidentally
- It is updated intentionally and globally

This prevents:
- branch drift
- configuration mismatch
- “works on my machine” failures

Static files are explicit, opt-in, and enforced.

### 7. Scoped Branches (Authority, Not Illusion)

In SDAL, a branch does not have to represent the entire repository.

A branch can be scoped to specific files or folders.

Example:
A branch scoped to `/modules/rendering/**`

Everything else becomes immutable in that branch

This enables:
- conflict prevention instead of conflict resolution
- safe parallel work
- clearer intent
- reduced coordination cost

Scoped branches are especially powerful for:
- monorepos
- open source projects
- large teams
- asset-heavy workflows

Git cannot enforce this at the system level.
SDAL can.

### 8. Permissions Like a Database

SDAL supports file-level and branch-level permissions, similar to a database.

This allows:
- least-privilege access
- protected core modules
- safer open-source contributions
- reduced blast radius from mistakes

Instead of:
“Please don’t touch this”

SDAL enforces:
“You cannot touch this.”

Permissions are:
- optional
- explicit
- enforced by the core system

### 9. Optional Ledger (Blockchain) Mode

Blockchain is not mandatory in SDAL.

It is an infrastructure mode for organizations that need:
- tamper-evident history
- insider-threat protection
- strong auditability
- zero-trust guarantees

When ledger mode is enabled:
- history becomes append-only
- commits are cryptographically signed
- silent rewrites become impossible
- tampering becomes detectable

This is especially valuable for:
- Big Tech
- game studios
- AI companies
- infrastructure teams
- kernel and compiler projects

For everyone else:
SDAL works perfectly without it

### 10. Strictness Is Intentional, Not Mandatory

SDAL is strict by nature, but not strict by default.

- No rule exists unless declared
- No enforcement exists unless enabled
- No restriction is accidental

Once a rule is enabled:
It cannot be bypassed.

This is a deliberate contract.

### 11. Modular by Design

SDAL is not monolithic.

It is built from replaceable modules:
- storage backends
- chunking engines
- policy enforcement
- verification
- interfaces

This allows:
- future evolution
- enterprise customization
- long-term survivability

Git is hard to evolve because it is tightly coupled.
SDAL avoids that by design.

### 12. Rust from the Start

SDAL is written in Rust from day one.

This provides:
- memory safety
- predictable performance
- safe concurrency
- fewer security vulnerabilities
- alignment with SDAL’s strict philosophy

Git cannot be rewritten to solve modern problems without breaking itself.
SDAL was designed with those problems in mind from the start.

### 13. Who SDAL Is For

SDAL is ideal for:
- engine developers
- game studios
- AI companies
- kernel and OS developers
- infrastructure teams
- serious open-source projects

SDAL is not optimized for:
- small scripts
- throwaway prototypes
- purely text-based repos

This is intentional.

### 14. The Long-Term Vision

SDAL is not a tool.
It is infrastructure.

It exists to:
- scale with project maturity
- reduce human error
- preserve reality
- enforce correctness when required
- support decades-long codebases

It is designed to outlive trends.

### 15. Final Statement

Git manages collaboration.
SDAL manages shared reality.

SDAL gives freedom first,
and guarantees when freedom becomes risky.

That is its purpose.
That is its philosophy.
