# `agd detect` — output reference

Loaded when the skill routes a `detect` invocation, after install success, and during init pre-flight to enumerate callable agents.

## Output shape

`agd detect` prints a JSON object to stdout. Keys are agent template names; values describe what the CLI found on `PATH`.

```json
{
  "claude": {
    "path": "/usr/local/bin/claude",
    "version": "2.1.144 (Claude Code)",
    "callable": true,
    "verified": true
  },
  "gemini": {
    "path": null,
    "version": null,
    "callable": false,
    "verified": true
  }
}
```

## Field interpretation

| Field | Meaning |
|---|---|
| `path` | Resolved binary location, or `null` if not found on PATH. |
| `version` | Stdout of the agent's version probe, or `null` (probe disabled or failed). |
| `callable` | `true` if the binary was found and runs at all. |
| `verified` | `true` if the agent template's non-interactive contract is trusted upstream. `false` agents are listed but skipped at dispatch. |

## When the skill uses `detect`

- **After install:** to confirm which agent CLIs are ready.
- **Init pre-flight:** to enumerate `callable == true` agents and seed the default JSON payload. Non-callable agents are surfaced as opt-in choices so the user can still include them manually.
- **Troubleshooting:** when dispatch reports "agent not found" or unexpected tier resolution.
