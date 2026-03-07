# SDAL Build Instructions

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable version recommended)

## Building from Source

1.  **Clone the repository:**

    ```bash
    git clone <repository_url>
    cd sdal
    ```

2.  **Build in release mode:**

    ```bash
    cargo build --release
    ```

    The compiled binary will be available at `target/release/sdal`.

## Running Tests

To run the test suite:

```bash
cargo test
```
