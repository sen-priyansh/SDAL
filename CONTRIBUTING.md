# Contributing to SDAL

Thank you for your interest in contributing to SDAL! We welcome contributions from the community.

## How to Contribute

### Reporting Bugs

If you find a bug, please open an issue describing:
1.  What you did (steps to reproduce).
2.  What you expected to happen.
3.  What actually happened.
4.  Your environment (OS, Rust version).

### submitting Pull Requests

1.  **Fork the repository** and create a new branch for your feature or fix.
2.  **Write code** following our coding standards.
3.  **Add tests** to cover your changes.
4.  **Run tests** to ensure no regressions:
    ```bash
    cargo test
    ```
5.  **Format your code**:
    ```bash
    cargo fmt
    ```
6.  **Run linting checks**:
    ```bash
    cargo clippy
    ```
7.  **Submit a Pull Request (PR)** with a clear description of your changes.

## Coding Standards

-   We use **Rustfmt** for code formatting.
-   We use **Clippy** for linting. Please ensure your code is warning-free.
-   Write clear, idiomatic Rust code.
-   Document public APIs with doc comments (`///`).

## License

By contributing to SDAL, you agree that your contributions will be licensed under the **Business Source License 1.1** (or the Change License after the Change Date) as defined in the `LICENSE` file.
