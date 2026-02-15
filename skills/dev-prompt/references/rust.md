# Rust Development Guidelines

## Style & Standards
-   **Rustfmt**: Always run `cargo fmt`.
-   **Clippy**: Ensure code passes `cargo clippy` without warnings.
-   **Idiomatic Rust**: Embrace RAII and the borrow checker. Avoid `unsafe` unless absolutely necessary.

## Best Practices
-   **Error Handling**: Use `Result` for recoverable errors and `Option` for optional values. Avoid `unwrap()` in production code; use `expect()` or `?` operator.
-   **Ownership**: Prefer borrowing over cloning. Use `Cow` for smart copy-on-write when appropriate.
-   **Pattern Matching**: Use `match` and `if let` for control flow involving enums.
-   **Iterators**: Prefer functional-style iterator chains (`map`, `filter`, `fold`) over explicit loops for transformations.

## Testing
-   **Unit Tests**: Place unit tests in the same file within a `mod tests` module.
-   **Integration Tests**: Place integration tests in the `tests/` directory.
-   **Doc Tests**: Write documentation examples that double as tests.

## Cargo
-   Keep `Cargo.toml` dependencies minimal and up-to-date.
-   Use feature flags to reduce compile times and binary size.
