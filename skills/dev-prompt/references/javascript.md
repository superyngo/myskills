# JavaScript/TypeScript Development Guidelines

## Style & Standards
-   **TypeScript**: Prefer TypeScript over JavaScript for type safety. Use `strict: true`.
-   **Formatting**: Use Prettier.
-   **Linting**: Use ESLint with a standard config (e.g., Airbnb or Google).

## Best Practices
-   **Modern JS**: Use ES6+ features (const/let, arrow functions, destructuring, spread/rest).
-   **Async/Await**: Prefer `async/await` over raw Promises or callbacks.
-   **Immutability**: Treat data as immutable where possible. Use spread operator or libraries like Immer.
-   **Equality**: Always use strict equality (`===`) and inequality (`!==`).

## TypeScript Specifics
-   **No Any**: Avoid `any`. Use `unknown` if the type is truly not known yet, or generics.
-   **Interfaces/Types**: Prefer `interface` for object shapes and `type` for unions/primitives.
-   **Null/Undefined**: Handle `null` and `undefined` explicitly. Use optional chaining (`?.`) and nullish coalescing (`??`).

## Testing
-   **Frameworks**: Use Vitest or Jest.
-   **Testing Library**: Use `testing-library` for UI component tests (focus on user behavior).
