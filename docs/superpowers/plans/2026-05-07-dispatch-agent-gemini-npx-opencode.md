# dispatch-agent: Gemini-NPX & OpenCode Support Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add support for detecting and invoking Gemini CLI via `npx @google/gemini-cli@latest` and OpenCode CLI via `opencode run "<prompt>" --model <model>` with positional-prompt and subcommand syntax.

**Architecture:** Two new template fields (`subcommand`, `prompt_positional`) in `cli-templates.toml` unlock opencode's call pattern; one new field (`detect_binary`) enables gemini-npx detection via the `npx` binary. `build_command()` in `dispatch.py` is extended to honour these fields. `detect.py` gains `detect_binary` support and adds `gemini-npx` to the known-CLI list.

**Tech Stack:** Python 3.11+ stdlib only (`tomllib`, `shutil`, `subprocess`). Test runner: `pytest` (or `python -m unittest`). Config format: TOML.

---

## File Map

| File | Change |
|------|--------|
| `data/cli-templates.toml` | Add `[gemini-npx]` section; update `[opencode]` (subcommand, prompt_positional, model_flag, verified=true) |
| `scripts/detect.py` | Add `gemini-npx` to `KNOWN_CLIS`; support `detect_binary` template field in `check_cli()` |
| `scripts/dispatch.py` | Extend `build_command()` to handle `subcommand` and `prompt_positional` fields |
| `tests/test_detect.py` | Update opencode verified test; add gemini-npx detection tests |
| `tests/test_dispatch.py` | Update opencode empty-flag test; add subcommand + positional-prompt tests; add gemini-npx command-build test |
| `references/init-guide.md` | Remove opencode unverified warning; add gemini-npx notes; update default-models table |

---

## Task 1: Update `cli-templates.toml`

**Files:**
- Modify: `data/cli-templates.toml`

- [ ] **Step 1: Edit the file**

Replace the `[opencode]` section and append `[gemini-npx]`:

```toml
[claude]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[gemini]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[codex]
prompt_flag = "-q"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[copilot]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[opencode]
subcommand = "run"
prompt_positional = true
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
verified = true
extra_args = []

[gemini-npx]
detect_binary = "npx"
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = ""
extra_args = ["@google/gemini-cli@latest", "--skip-trust"]
```

- [ ] **Step 2: Verify TOML is valid**

```bash
python3 -c "
import tomllib
with open('data/cli-templates.toml', 'rb') as f:
    d = tomllib.load(f)
print('sections:', list(d.keys()))
"
```

Expected output:
```
sections: ['claude', 'gemini', 'codex', 'copilot', 'opencode', 'gemini-npx']
```

- [ ] **Step 3: Commit**

```bash
git add data/cli-templates.toml
git commit -m "feat(dispatch-agent): add gemini-npx and update opencode templates"
```

---

## Task 2: Extend `detect.py` — `detect_binary` support + `gemini-npx`

**Files:**
- Modify: `scripts/detect.py`
- Test: `tests/test_detect.py`

- [ ] **Step 1: Write failing tests first**

Open `tests/test_detect.py` and add these test cases inside `TestDetectCli`:

```python
def setUp(self):
    self.templates = {
        "claude": {"version_flag": "--version", "verified": True},
        "opencode": {"version_flag": "--version", "verified": True,  # updated — no longer False
                     "subcommand": "run", "prompt_positional": True},
        "nodecli": {"version_flag": "", "verified": True},
        "gemini-npx": {"detect_binary": "npx", "version_flag": "", "verified": True,
                        "extra_args": ["@google/gemini-cli@latest", "--skip-trust"]},
    }

@patch("detect.shutil.which")
@patch("detect.os.access")
def test_gemini_npx_detected_via_npx_binary(self, mock_access, mock_which):
    """gemini-npx is callable when 'npx' binary is available."""
    mock_which.return_value = "/usr/local/bin/npx"
    mock_access.return_value = True
    result = detect.check_cli("gemini-npx", self.templates)
    mock_which.assert_called_with("npx")
    self.assertTrue(result["callable"])
    self.assertEqual(result["path"], "/usr/local/bin/npx")

@patch("detect.shutil.which")
def test_gemini_npx_not_callable_when_npx_missing(self, mock_which):
    """gemini-npx is not callable when 'npx' is absent."""
    mock_which.return_value = None
    result = detect.check_cli("gemini-npx", self.templates)
    mock_which.assert_called_with("npx")
    self.assertFalse(result["callable"])

@patch("detect.shutil.which")
@patch("detect.os.access")
def test_opencode_now_verified(self, mock_access, mock_which):
    """opencode template has verified=True in the updated templates."""
    mock_which.return_value = "/usr/bin/opencode"
    mock_access.return_value = True
    result = detect.check_cli("opencode", self.templates)
    self.assertTrue(result["verified"])
```

Also **update the existing `test_verified_false_copied_from_template` test** — opencode is no longer `verified=False` in production templates, but the test logic is still valid for any CLI with `verified=False`. Change the test to use a synthetic template key instead of "opencode":

```python
@patch("detect.shutil.which")
@patch("detect.os.access")
@patch("detect.subprocess.run")
def test_verified_false_copied_from_template(self, mock_run, mock_access, mock_which):
    mock_which.return_value = "/usr/bin/somecli"
    mock_access.return_value = True
    mock_run.return_value = MagicMock(returncode=0, stdout="somecli 0.5\n")
    templates_with_unverified = {"somecli": {"version_flag": "--version", "verified": False}}
    result = detect.check_cli("somecli", templates_with_unverified)
    self.assertFalse(result["verified"])
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Volumes/Home/Users/wen/.local/share/agm/source/myskills/skills/dispatch-agent
python3 -m pytest tests/test_detect.py -v 2>&1 | tail -20
```

Expected: new tests FAIL with `AssertionError` (gemini-npx not in KNOWN_CLIS yet, check_cli doesn't read detect_binary).

- [ ] **Step 3: Update `scripts/detect.py`**

Change `KNOWN_CLIS` and `check_cli()`:

```python
KNOWN_CLIS = ["claude", "gemini", "gemini-npx", "codex", "copilot", "opencode"]


def check_cli(name: str, templates: dict) -> dict:
    tmpl = templates.get(name)
    detect_binary = tmpl.get("detect_binary", name) if tmpl else name

    path = shutil.which(detect_binary)
    if path is None or not os.access(path, os.X_OK):
        return {"path": None, "version": None, "callable": False, "verified": True}

    if tmpl is None:
        return {"path": path, "version": None, "callable": True, "verified": True}

    verified = tmpl.get("verified", True)
    version_flag = tmpl.get("version_flag", "--version")

    version = None
    if version_flag:
        try:
            result = subprocess.run(
                [detect_binary, version_flag],
                capture_output=True,
                text=True,
                timeout=5,
            )
            if result.returncode == 0 and result.stdout.strip():
                version = result.stdout.strip().splitlines()[0]
        except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
            pass

    return {"path": path, "version": version, "callable": True, "verified": verified}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
python3 -m pytest tests/test_detect.py -v 2>&1 | tail -20
```

Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add scripts/detect.py tests/test_detect.py
git commit -m "feat(dispatch-agent): support detect_binary in detect.py, add gemini-npx"
```

---

## Task 3: Extend `dispatch.py` — `subcommand` and `prompt_positional`

**Files:**
- Modify: `scripts/dispatch.py`
- Test: `tests/test_dispatch.py`

- [ ] **Step 1: Write failing tests**

Add the following to `TestBuildCommand` in `tests/test_dispatch.py`.

First, update `SAMPLE_TEMPLATES` at the top of the file to replace the `[opencode]` block and add `[gemini-npx]`:

```python
SAMPLE_TEMPLATES = """
[claude]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[gemini]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[copilot]
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
extra_args = []

[opencode]
subcommand = "run"
prompt_positional = true
model_flag = "--model"
file_input_mode = "arg"
version_flag = "--version"
verified = true
extra_args = []

[gemini-npx]
detect_binary = "npx"
prompt_flag = "-p"
model_flag = "--model"
file_input_mode = "arg"
version_flag = ""
extra_args = ["@google/gemini-cli@latest", "--skip-trust"]
"""
```

Then add these test methods inside `TestBuildCommand`:

```python
def test_opencode_subcommand_positional_prompt(self):
    """opencode: subcommand 'run' + positional prompt, no model."""
    agent = {"id": "opencode-default", "cli": "opencode", "model": "default", "args": [], "env": []}
    cmd = dispatch.build_command(agent, self.templates["opencode"], "hello world")
    self.assertEqual(cmd, ["opencode", "run", "hello world"])

def test_opencode_positional_prompt_with_model(self):
    """opencode: subcommand + positional prompt + --model flag."""
    agent = {"id": "opencode-glm", "cli": "opencode", "model": "zai-coding-plan/glm-5.1", "args": [], "env": []}
    cmd = dispatch.build_command(agent, self.templates["opencode"], "hi")
    self.assertEqual(cmd, ["opencode", "run", "hi", "--model", "zai-coding-plan/glm-5.1"])

def test_opencode_positional_prompt_with_model_and_agent_args(self):
    """opencode: agent args (e.g. --thinking) appear after model."""
    agent = {"id": "opencode-glm", "cli": "opencode", "model": "zai-coding-plan/glm-5.1",
             "args": ["--thinking"], "env": []}
    cmd = dispatch.build_command(agent, self.templates["opencode"], "hi")
    self.assertEqual(cmd, ["opencode", "run", "hi", "--model", "zai-coding-plan/glm-5.1", "--thinking"])

def test_gemini_npx_command(self):
    """gemini-npx: npx + package + extra_args + -p prompt."""
    agent = {"id": "gemini-npx-default", "cli": "npx", "model": "default", "args": [], "env": []}
    cmd = dispatch.build_command(agent, self.templates["gemini-npx"], "hi")
    self.assertEqual(cmd, ["npx", "@google/gemini-cli@latest", "--skip-trust", "-p", "hi"])

def test_gemini_npx_with_model(self):
    """gemini-npx: model flag inserted before prompt flag."""
    agent = {"id": "gemini-npx-pro", "cli": "npx", "model": "gemini-2.5-pro", "args": [], "env": []}
    cmd = dispatch.build_command(agent, self.templates["gemini-npx"], "hi")
    self.assertEqual(cmd,
        ["npx", "@google/gemini-cli@latest", "--skip-trust", "--model", "gemini-2.5-pro", "-p", "hi"])

def test_positional_no_prompt_flag_no_subcommand(self):
    """prompt_positional=True with no subcommand: prompt appended directly after cli."""
    tmpl = {"prompt_positional": True, "model_flag": "", "extra_args": [], "subcommand": ""}
    agent = {"id": "x", "cli": "mycli", "model": "default", "args": [], "env": []}
    cmd = dispatch.build_command(agent, tmpl, "task text")
    self.assertEqual(cmd, ["mycli", "task text"])
```

Also **update** the existing `test_empty_prompt_flag_returns_none` test — it used the opencode template which no longer has an empty prompt_flag. Use a synthetic template instead:

```python
def test_empty_prompt_flag_and_not_positional_returns_none(self):
    """A template with empty prompt_flag and prompt_positional=False cannot dispatch."""
    tmpl = {"prompt_flag": "", "prompt_positional": False, "model_flag": "", "extra_args": []}
    agent = {"id": "x", "cli": "x", "model": "default", "args": [], "env": []}
    result = dispatch.build_command(agent, tmpl, "hi")
    self.assertIsNone(result)
```

(Remove the old `test_empty_prompt_flag_returns_none`.)

- [ ] **Step 2: Run tests to verify they fail**

```bash
python3 -m pytest tests/test_dispatch.py::TestBuildCommand -v 2>&1 | tail -30
```

Expected: new tests FAIL (subcommand/prompt_positional not yet implemented).

- [ ] **Step 3: Update `build_command()` in `scripts/dispatch.py`**

Replace the existing `build_command` function (lines 74–92) with:

```python
def build_command(agent: dict, template: dict, prompt: str) -> list | None:
    prompt_flag = template.get("prompt_flag", "")
    prompt_positional = template.get("prompt_positional", False)
    subcommand = template.get("subcommand", "")
    model_flag = template.get("model_flag", "")
    extra_args = template.get("extra_args", [])
    agent_args = agent.get("args", [])
    model = agent.get("model", "default")

    if not prompt_positional and not prompt_flag:
        return None

    cmd = [agent["cli"]]
    if subcommand:
        cmd += [subcommand]

    if prompt_positional:
        cmd += [prompt]
        if model != "default" and model_flag:
            cmd += [model_flag, model]
        elif model != "default" and not model_flag:
            print(
                f"Warning: agent {agent['id']} has model={model!r} but model_flag is empty — model ignored",
                file=sys.stderr,
            )
        cmd += extra_args
        cmd += agent_args
    else:
        cmd += extra_args
        cmd += agent_args
        if model != "default" and model_flag:
            cmd += [model_flag, model]
        elif model != "default" and not model_flag:
            print(
                f"Warning: agent {agent['id']} has model={model!r} but model_flag is empty — model ignored",
                file=sys.stderr,
            )
        cmd += [prompt_flag, prompt]

    return cmd
```

- [ ] **Step 4: Run all dispatch tests**

```bash
python3 -m pytest tests/test_dispatch.py -v 2>&1 | tail -30
```

Expected: all tests PASS.

- [ ] **Step 5: Run full test suite**

```bash
python3 -m pytest tests/ -v 2>&1 | tail -30
```

Expected: all tests PASS (no regressions).

- [ ] **Step 6: Commit**

```bash
git add scripts/dispatch.py tests/test_dispatch.py
git commit -m "feat(dispatch-agent): support subcommand and prompt_positional in build_command"
```

---

## Task 4: Update `references/init-guide.md`

**Files:**
- Modify: `references/init-guide.md`

- [ ] **Step 1: Update the file**

Make three changes:

**a) Remove the opencode unverified warning** in Step 1. Change:
```
> "CLIs marked `verified: false` (e.g. opencode) will be skipped at dispatch even if added to config, because their non-interactive mode is unverified."
```
to:
```
> "CLIs marked `verified: false` will be skipped at dispatch even if added to config, because their non-interactive mode is unverified."
```

**b) In Step 3, update the default-models table:**
```markdown
| CLI | Default model |
|-----|--------------|
| claude | default |
| gemini | default |
| gemini-npx | default |
| codex | default |
| copilot | sonnet-4.6 |
| opencode | zai-coding-plan/glm-5.1 |
```

**c) Add a note for gemini-npx after the table in Step 3:**
```markdown
> **Note:** `gemini-npx` uses `npx` as the underlying binary with `@google/gemini-cli@latest`.
> Set `cli = "npx"` in your config when using this template. Agent args (e.g. `["--thinking"]`)
> are appended after the model flag for `opencode`.
```

- [ ] **Step 2: Commit**

```bash
git add references/init-guide.md
git commit -m "docs(dispatch-agent): update init-guide for opencode verified and gemini-npx"
```

---

## Task 5: Smoke Test End-to-End (dry-run)

- [ ] **Step 1: Verify detect sees gemini-npx**

```bash
python3 scripts/detect.py | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('gemini-npx'))"
```

Expected (if npx is installed):
```json
{"path": "/path/to/npx", "version": null, "callable": true, "verified": true}
```

- [ ] **Step 2: Create a minimal test config**

```bash
cat > /tmp/test-dispatch.toml << 'EOF'
version = 1

[[tiers]]
id = "primary"

  [[tiers.agents]]
  id = "opencode-glm"
  cli = "opencode"
  model = "zai-coding-plan/glm-5.1"
  args = ["--thinking"]

  [[tiers.agents]]
  id = "gemini-npx-default"
  cli = "npx"
  model = "default"
  args = []
EOF
```

- [ ] **Step 3: Dry-run dispatch**

```bash
python3 scripts/dispatch.py --config /tmp/test-dispatch.toml -p "hello" --dry-run
```

Expected output (two dry-run lines):
```
[DRY RUN] tier=primary  agent=opencode-glm
  command: ['opencode', 'run', 'hello', '--model', 'zai-coding-plan/glm-5.1', '--thinking']
```

```bash
python3 scripts/dispatch.py --config /tmp/test-dispatch.toml -p "hello" --dry-run --agent gemini-npx-default
```

Expected:
```
[DRY RUN] agent=gemini-npx-default
  command: ['npx', '@google/gemini-cli@latest', '--skip-trust', '-p', 'hello']
```

- [ ] **Step 4: Clean up temp config**

```bash
rm /tmp/test-dispatch.toml
```

- [ ] **Step 5: Final test run**

```bash
python3 -m pytest tests/ -v
```

Expected: all tests PASS.
