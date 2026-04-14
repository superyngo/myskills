# Model Routing Table

## Primary Routing by Task Category

| Task Category | Best Model | Budget Alternative | Agent Role | CLI |
|---------------|------------|--------------------|------------|-----|
| `orchestrate` | `claude-opus-4-6` | `kimi-k2.5` | Sisyphus | `claude` / `opencode` |
| `deep` | `gpt-5.3-codex` / `o3` | `claude-sonnet-4-6` | Hephaestus | `codex` / `claude` |
| `quick` | `gemini-2.5-flash` | `claude-haiku-4-5` | Hephaestus | `gemini` / `claude` |
| `ultrabrain` | `claude-opus-4-6` | `o3` | Prometheus | `claude` / `codex` |
| `visual` | `gemini-2.5-pro` | `claude-sonnet-4-6` | Hephaestus | `gemini` / `claude` |
| `research` | `gemini-2.5-pro` | `claude-sonnet-4-6` | Oracle | `gemini` / `claude` |
| `librarian` | `gemini-2.5-flash` | `claude-haiku-4-5` | Librarian | `gemini` / `claude` |

---

## Fallback Chains

Each category has a priority-ordered fallback chain.
Use the first agent whose CLI is available and whose API key is set.

### `orchestrate`
```
1. claude-opus-4-6    (via claude CLI)
2. kimi-k2.5          (via opencode CLI + oh-my-openagent)
3. glm-5              (via opencode CLI + oh-my-openagent)
4. claude-sonnet-4-6  (via claude CLI)
```

### `deep`
```
1. gpt-5.3-codex / o3     (via codex CLI, --approval-mode full-auto)
2. claude-opus-4-6         (via claude CLI)
3. kimi-k2.5               (via opencode CLI)
4. gemini-2.5-pro          (via gemini CLI)
5. claude-sonnet-4-6       (via claude CLI)
```

### `quick`
```
1. gemini-2.5-flash        (via gemini CLI --model gemini-2.5-flash)
2. claude-haiku-4-5        (via claude CLI --model claude-haiku-4-5-20251001)
3. claude-sonnet-4-6       (via claude CLI)
4. codex o4-mini           (via codex CLI)
```

### `ultrabrain`
```
1. claude-opus-4-6         (via claude CLI)
2. o3                      (via codex CLI --model o3)
3. gemini-2.5-pro          (via gemini CLI)
4. kimi-k2.5               (via opencode CLI)
```

### `visual`
```
1. gemini-2.5-pro          (via gemini CLI)
2. claude-sonnet-4-6       (via claude CLI)
3. kimi-k2.5               (via opencode CLI)
```

### `research`
```
1. gemini-2.5-pro          (via gemini CLI)  ← 2M context advantage
2. claude-opus-4-6         (via claude CLI)
3. claude-sonnet-4-6       (via claude CLI)
4. opencode                (via opencode CLI --agent oracle)
```

### `librarian`
```
1. gemini-2.5-flash        (via gemini CLI)
2. claude-haiku-4-5        (via claude CLI)
3. claude-sonnet-4-6       (via claude CLI)
```

---

## Model Characteristics Summary

### claude-opus-4-6
- **Strengths**: Best instruction-following, structured output, multi-step reasoning
- **Weaknesses**: Most expensive, slower
- **Token limit**: 200K input
- **Best for**: Orchestration, architecture, complex multi-file refactoring

### claude-sonnet-4-6
- **Strengths**: Great balance of speed/quality
- **Weaknesses**: Not as deep as Opus on complex tasks
- **Token limit**: 200K input
- **Best for**: General coding, code review, most day-to-day tasks

### claude-haiku-4-5
- **Strengths**: Very fast, very cheap
- **Weaknesses**: Less capable on complex reasoning
- **Token limit**: 200K input
- **Best for**: Quick edits, documentation, simple transformations

### gemini-2.5-pro
- **Strengths**: 2M token context window, strong at visual/frontend, creative
- **Weaknesses**: Sometimes less precise on structured output
- **Token limit**: 2M input
- **Best for**: Large codebase research, frontend/UI, reading entire repos

### gemini-2.5-flash
- **Strengths**: Very fast, very cheap, still capable
- **Weaknesses**: Less capable than Pro
- **Token limit**: 1M input
- **Best for**: Quick tasks, documentation, summarization

### gpt-5.3-codex / o3 (via codex CLI)
- **Strengths**: Deep autonomous reasoning, strong at end-to-end implementation
- **Weaknesses**: Can be slow on long tasks, requires `--approval-mode full-auto` for headless
- **Best for**: Autonomous deep implementation (Hephaestus role)

### kimi-k2.5
- **Strengths**: Claude-like behavior, affordable, good orchestration ability
- **Weaknesses**: Less community tooling, API via Moonshot AI
- **Best for**: Budget replacement for Claude Opus in orchestration roles

### glm-5 (Z.ai)
- **Strengths**: Cheap, decent instruction following
- **Weaknesses**: Less tested than Claude/GPT
- **Best for**: Budget orchestration when Kimi not available

---

## Cost Optimization Presets

### Ultra-Budget (minimize API spend)
```
orchestrate → kimi-k2.5 (opencode)
deep        → gemini-2.5-pro (gemini)
quick       → gemini-2.5-flash (gemini)
ultrabrain  → claude-sonnet-4-6 (claude)
research    → gemini-2.5-pro (gemini)
librarian   → gemini-2.5-flash (gemini)
```

### Quality-First (best results)
```
orchestrate → claude-opus-4-6 (claude)
deep        → o3 (codex)
quick       → claude-sonnet-4-6 (claude)
ultrabrain  → claude-opus-4-6 (claude)
research    → gemini-2.5-pro (gemini)  ← context window advantage
librarian   → claude-haiku-4-5 (claude)
```

### CHT API Gateway Compatible
If routing through Chunghwa Telecom API gateway:
- Use `claude-sonnet-4-6` for most tasks (confirm supported models with CHT gateway)
- Set `ANTHROPIC_BASE_URL` or equivalent to CHT gateway endpoint
- Avoid model-specific flags that may not be supported by the gateway
```bash
ANTHROPIC_BASE_URL=https://cht-gateway.example.com/v1 claude -p "PROMPT"
```
