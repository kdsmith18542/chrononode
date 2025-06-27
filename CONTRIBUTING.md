# Contributing to ChronoNode

Thank you for your interest in contributing to ChronoNode! We welcome contributions from everyone.

## Code of Conduct

This project and everyone participating in it is governed by our [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## How to Contribute

1. **Fork** the repository on GitLab
2. **Clone** the project to your machine
3. **Create a branch** for your feature or bugfix
4. **Commit** your changes
5. **Push** your work to your fork
6. Open a **Merge Request**

## Development Setup

1. Ensure you have the latest stable Rust toolchain installed:
   ```bash
   rustup update stable
   ```

2. Clone the repository:
   ```bash
   git clone [repository-url]
   cd ChronoNode
   ```

3. Build the project:
   ```bash
   cargo build
   ```

## Running Tests

Run the test suite with:

```bash
cargo test
```

## Code Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Run `cargo fmt` before committing
- Run `cargo clippy` to catch common mistakes and improve your code

## Commit Messages

- Use the present tense ("Add feature" not "Added feature")
- Use the imperative mood ("Move cursor to..." not "Moves cursor to...")
- Limit the first line to 72 characters or fewer
- Reference issues and merge requests liberally

## Creating a Merge Request

1. Ensure your branch is up to date with the main branch
2. Push your changes to your fork
3. Create a new merge request in GitLab
4. Fill in the merge request template
5. Request a review from a maintainer
