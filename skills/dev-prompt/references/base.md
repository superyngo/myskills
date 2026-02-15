# General Development Guidelines

## Core Principles
1.  **Readability First**: Write code that is easy to understand. Favor clarity over cleverness.
2.  **KISS (Keep It Simple, Stupid)**: Avoid over-engineering. Solve the problem at hand.
3.  **DRY (Don't Repeat Yourself)**: Extract common logic into functions or variables.
4.  **SOLID**: Adhere to SOLID principles where applicable, especially Single Responsibility.
5.  **Fail Fast**: Validate inputs and assumptions early.

## Code Quality
-   **Naming**: Use descriptive variable and function names. `user_id` is better than `uid`.
-   **Comments**: Explain *why*, not *what*. Code should be self-documenting.
-   **Formatting**: Follow standard formatting rules for the language (e.g., Prettier, Black, Rustfmt).
-   **Error Handling**: Handle errors gracefully. Don't suppress exceptions without a good reason.

## Workflow
-   **Commits**: Make atomic commits with clear messages (Conventional Commits preferred).
-   **Testing**: Write tests for new functionality. Ensure existing tests pass.

## CLI Argument Handling
-   **Version Flag**: Prefer `--version`/`-v` to display version information
-   **Help on Insufficient Args**: When required arguments are missing, display the same help content as `-h`/`--help` instead of generic error messages

## File Placement for User-level Applications
Follow the **XDG Base Directory Specification** for user-level application data placement across platforms:


### Priority Order
1.  Check XDG environment variables
2.  Use platform-specific defaults
3.  Ensure backward compatibility with legacy paths
