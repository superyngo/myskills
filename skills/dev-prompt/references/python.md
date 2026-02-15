# Python Development Guidelines

## Style & Standards
-   **PEP 8**: Follow PEP 8 guidelines. Use `black` or `ruff` for formatting.
-   **Type Hints**: Use type annotations for all function signatures (Python 3.10+ syntax preferred).
-   **Imports**: Organize imports (Standard lib -> Third party -> Local).

## Best Practices
-   **Virtual Environments**: Always use a virtual environment (`venv`, `poetry`, `uv`).
-   **Path Handling**: Use `pathlib.Path` instead of `os.path` strings.
-   **Context Managers**: Use `with` statements for resource management (files, locks, connections).
-   **F-Strings**: Prefer f-strings for string formatting.
-   **Comprehensions**: Use list/dict comprehensions for simple transformations, but avoid complex nested ones.

## Testing
-   **Pytest**: Use `pytest` for testing.
-   **Fixtures**: Leverage fixtures for setup/teardown.
-   **Parametrization**: Use `@pytest.mark.parametrize` to reduce test code duplication.

## Dependency Management
-   Prefer `pyproject.toml` for configuration.
-   Use `uv` for fast package management if available.
