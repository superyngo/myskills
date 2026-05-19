#!/usr/bin/env python3
"""Dispatch tasks to agent CLIs with tier-based fallback and round-robin."""
import argparse
import fcntl
import json
import os
import select
import shlex
import signal
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import tomllib
from pathlib import Path

TEMPLATES_PATH = Path(__file__).parent.parent / "data" / "cli-templates.toml"
RR_STATE_PATH = Path.home() / ".cache" / "dispatch-agent" / "rr-state.json"
MAX_FILE_BYTES = 256 * 1024
VALID_ENV_TYPES = {"file", "env", "source"}


def load_config(path: str) -> dict:
    try:
        with open(path, "rb") as f:
            config = tomllib.load(f)
    except FileNotFoundError:
        print(f"Error: config file not found: {path}", file=sys.stderr)
        sys.exit(1)
    except tomllib.TOMLDecodeError as e:
        print(f"Error: config parse error: {e}", file=sys.stderr)
        sys.exit(1)

    if "version" not in config:
        print("Warning: config missing 'version' field, assuming v1", file=sys.stderr)

    for tier in config.get("tiers", []):
        for agent in tier.get("agents", []):
            for ev in agent.get("env", []):
                if ev.get("type") not in VALID_ENV_TYPES:
                    print(f"Error: agent {agent['id']} has invalid env type {ev.get('type')!r}", file=sys.stderr)
                    sys.exit(1)

    return config


def load_templates(path: str = None) -> dict:
    p = Path(path) if path else TEMPLATES_PATH
    if not p.exists():
        print(f"Error: cli-templates.toml not found: {p}", file=sys.stderr)
        sys.exit(1)
    with open(p, "rb") as f:
        return tomllib.load(f)


def find_config(config_arg: str | None) -> str | None:
    if config_arg:
        return config_arg
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True, text=True
    )
    project_root = result.stdout.strip() if result.returncode == 0 else os.getcwd()
    project_cfg = Path(project_root) / ".config" / "dispatch-agent.toml"
    if project_cfg.exists():
        return str(project_cfg)
    user_cfg = Path.home() / ".config" / "dispatch-agent.toml"
    if user_cfg.exists():
        return str(user_cfg)
    return None


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
            print(f"Warning: agent {agent['id']} has model={model!r} but model_flag is empty — model ignored", file=sys.stderr)
        cmd += extra_args
        cmd += agent_args
    else:
        cmd += extra_args
        cmd += agent_args
        if model != "default" and model_flag:
            cmd += [model_flag, model]
        elif model != "default" and not model_flag:
            print(f"Warning: agent {agent['id']} has model={model!r} but model_flag is empty — model ignored", file=sys.stderr)
        cmd += [prompt_flag, prompt]

    return cmd


def resolve_env_var(ev: dict) -> tuple | None:
    if ev["type"] == "source":
        return None  # handled via shell sourcing in wrap_with_sources
    name = ev["name"]
    if ev["type"] == "env":
        val = os.environ.get(ev["var"])
        if val is None:
            print(f"Warning: env var {ev['var']!r} not set, skipping", file=sys.stderr)
            return None
        return (name, val)
    elif ev["type"] == "file":
        path = os.path.expanduser(ev["path"])
        try:
            return (name, Path(path).read_text().strip())
        except OSError:
            print(f"Warning: env file {path!r} not found, skipping", file=sys.stderr)
            return None


def get_source_files(agent: dict) -> list[str]:
    files = []
    for ev in agent.get("env", []):
        if ev.get("type") == "source":
            path = os.path.expanduser(ev["path"])
            if not Path(path).exists():
                print(f"Warning: source env file {path!r} not found, skipping", file=sys.stderr)
            else:
                files.append(path)
    return files


def wrap_with_sources(cmd: list, source_files: list[str]) -> list:
    if not source_files:
        return cmd
    source_cmds = "; ".join(f"source {shlex.quote(f)}" for f in source_files)
    return ["bash", "-c", f"set -a; {source_cmds}; set +a; exec \"$@\"", "--"] + cmd


def build_env(agent: dict, current_depth: int) -> dict:
    env = os.environ.copy()
    for ev in agent.get("env", []):
        result = resolve_env_var(ev)
        if result:
            env[result[0]] = result[1]
    env["DISPATCH_AGENT_DEPTH"] = str(current_depth + 1)
    return env


def load_rr_state(path: str | Path = None) -> dict:
    p = Path(path) if path else RR_STATE_PATH
    try:
        return json.loads(p.read_text())
    except Exception:
        return {}


def write_rr_state(state: dict, path: str | Path = None) -> None:
    p = Path(path) if path else RR_STATE_PATH
    p.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w", dir=p.parent, suffix=".tmp", delete=False
    ) as tmp:
        json.dump(state, tmp)
        tmp_path = tmp.name
    os.chmod(tmp_path, 0o600)
    os.replace(tmp_path, p)


def make_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="dispatch.py",
        description="Dispatch tasks to agent CLIs with tier-based fallback.",
    )
    prompt_group = parser.add_mutually_exclusive_group()
    prompt_group.add_argument("-p", metavar="PROMPT", help="Prompt text")
    prompt_group.add_argument("-f", metavar="FILE", help="File containing prompt")

    target_group = parser.add_mutually_exclusive_group()
    target_group.add_argument("--tier", metavar="ID", help="Start from named tier")
    target_group.add_argument("--agent", metavar="ID", help="Force specific agent.id")

    parser.add_argument("--timeout", type=int, default=-1, metavar="N",
                        help="Per-agent timeout seconds (-1 = no timeout)")
    parser.add_argument("--config", metavar="PATH", help="Config file path")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--show-config", action="store_true")
    parser.add_argument("--verbose", action="store_true")
    return parser


def _find_agent_by_id(tiers: list, agent_id: str) -> dict | None:
    matches = [
        agent
        for tier in tiers
        for agent in tier.get("agents", [])
        if agent["id"] == agent_id
    ]
    if len(matches) > 1:
        print(f"Warning: multiple agents with id {agent_id!r}, using first", file=sys.stderr)
    return matches[0] if matches else None


def call_agent_with_result(agent_id: str, tier_id: str | None, cmd: list, env: dict,
                            timeout: int, verbose: bool) -> tuple[int, str]:
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        start_new_session=True,
    )

    killed = threading.Event()
    timer = None

    def _kill():
        killed.set()
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
        except ProcessLookupError:
            pass

    def _handle_signal(signum, frame):
        try:
            os.killpg(os.getpgid(proc.pid), signum)
        except ProcessLookupError:
            pass
        killed.set()
        sys.exit(1)

    signal.signal(signal.SIGINT, _handle_signal)
    signal.signal(signal.SIGTERM, _handle_signal)

    if timeout > 0:
        timer = threading.Timer(timeout, _kill)
        timer.start()

    verbose_stop = threading.Event()
    if verbose:
        start_time = time.time()
        def _verbose_tick():
            while not verbose_stop.wait(10):
                elapsed = int(time.time() - start_time)
                print(f"[waiting: {agent_id} — {elapsed}s elapsed]", file=sys.stderr)
        threading.Thread(target=_verbose_tick, daemon=True).start()

    stderr_buf = []

    try:
        while True:
            rlist, _, _ = select.select([proc.stdout, proc.stderr], [], [], 0.1)
            for fd in rlist:
                data = fd.read1(4096)
                if not data:
                    continue
                if fd is proc.stdout:
                    sys.stdout.buffer.write(data)
                    sys.stdout.buffer.flush()
                else:
                    stderr_buf.append(data.decode(errors="replace"))

            if proc.poll() is not None:
                for fd in [proc.stdout, proc.stderr]:
                    remaining = fd.read()
                    if remaining:
                        if fd is proc.stdout:
                            sys.stdout.buffer.write(remaining)
                            sys.stdout.buffer.flush()
                        else:
                            stderr_buf.append(remaining.decode(errors="replace"))
                break
    finally:
        if timer:
            timer.cancel()
        verbose_stop.set()

    rc = proc.returncode
    stderr_text = "".join(stderr_buf)

    if rc == 0:
        if tier_id:
            print(f"[{agent_id}] (tier: {tier_id})", file=sys.stderr)
    else:
        if stderr_text:
            print(stderr_text, file=sys.stderr)

    return rc, stderr_text


def _cmd_show_config(config: dict, path: str) -> None:
    user_cfg = str(Path.home() / ".config" / "dispatch-agent.toml")
    layer = "user" if path == user_cfg else "project"
    print(f"Config: {path}  ({layer} layer)\n")
    for tier in config.get("tiers", []):
        print(f"TIER {tier['id']}")
        for agent in tier.get("agents", []):
            print(f"  agent: {agent['id']}   cli={agent['cli']}  model={agent.get('model','default')}  args={agent.get('args', [])}")
            for ev in agent.get("env", []):
                if ev["type"] == "source":
                    print(f"    env: (source: {ev['path']})")
                elif ev["type"] == "file":
                    print(f"    env: {ev['name']} (file: {ev['path']})")
                else:
                    print(f"    env: {ev['name']} (env: {ev['var']})")


def _cmd_list(config: dict, templates: dict) -> None:
    for tier in config.get("tiers", []):
        print(f"TIER {tier['id']}")
        for agent in tier.get("agents", []):
            cli = agent["cli"]
            path = shutil.which(cli)
            ok = path is not None and os.access(path, os.X_OK)
            marker = "✓" if ok else "✗"
            loc = path if ok else "(not found)"
            print(f"  [{marker}] {agent['id']}   cli={cli}   model={agent.get('model','default')}    {loc}")


def _cmd_list_detect() -> None:
    detect_path = Path(__file__).parent / "detect.py"
    result = subprocess.run(
        [sys.executable, str(detect_path)],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        print("Error running detect.py", file=sys.stderr)
        sys.exit(1)
    data = json.loads(result.stdout)
    print("[SYSTEM CLIs — no config loaded, run 'init' to configure]")
    for cli, info in data.items():
        if not info["callable"]:
            print(f"  [✗] {cli}  (not found)")
        elif not info.get("verified", True):
            ver = info.get("version") or ""
            print(f"  [!] {cli}  {info['path']}  {ver}  (verified=false — will be skipped at dispatch)")
        else:
            ver = info.get("version") or ""
            print(f"  [✓] {cli}  {info['path']}  {ver}")


def _cmd_dispatch(config: dict, templates: dict, prompt: str, args, depth: int) -> None:
    tiers = config.get("tiers", [])

    if args.agent:
        agent = _find_agent_by_id(tiers, args.agent)
        if agent is None:
            print(f"Error: agent id {args.agent!r} not found in config", file=sys.stderr)
            sys.exit(1)
        tmpl_key = agent.get("template", agent["cli"])
        tmpl = templates.get(tmpl_key)
        if tmpl is None:
            print(f"Error: CLI {tmpl_key!r} not in cli-templates.toml", file=sys.stderr)
            sys.exit(1)
        cmd = build_command(agent, tmpl, prompt)
        if cmd is None:
            print(f"Error: agent {agent['id']} has empty prompt_flag, cannot dispatch", file=sys.stderr)
            sys.exit(1)
        source_files = get_source_files(agent)
        cmd = wrap_with_sources(cmd, source_files)
        if args.dry_run:
            print(f"[DRY RUN] agent={agent['id']}")
            print(f"  command: {cmd}")
            return
        env = build_env(agent, depth)
        _, _ = call_agent_with_result(agent["id"], None, cmd, env, args.timeout, args.verbose)
        return

    if args.tier:
        tier_ids = [t["id"] for t in tiers]
        if args.tier not in tier_ids:
            print(f"Error: tier {args.tier!r} not found in config", file=sys.stderr)
            sys.exit(1)
        tiers = tiers[tier_ids.index(args.tier):]

    rr_path = RR_STATE_PATH
    rr_path.parent.mkdir(parents=True, exist_ok=True)
    rr_fd = open(rr_path, "a+")
    fcntl.flock(rr_fd, fcntl.LOCK_EX)
    rr_fd.seek(0)
    try:
        rr_state = json.load(rr_fd)
    except Exception:
        rr_state = {}
    fcntl.flock(rr_fd, fcntl.LOCK_UN)

    failures = []

    for tier in tiers:
        agents = tier.get("agents", [])
        if not agents:
            continue

        agent_ids = [a["id"] for a in agents]
        next_id = rr_state.get(tier["id"])
        start = agent_ids.index(next_id) if next_id in agent_ids else 0
        n = len(agents)

        for i in range(n):
            agent = agents[(start + i) % n]
            tmpl_key = agent.get("template", agent["cli"])
            tmpl = templates.get(tmpl_key)
            if tmpl is None:
                print(f"Warning: CLI {tmpl_key!r} not in cli-templates.toml, skipping", file=sys.stderr)
                failures.append((agent["id"], "skip: no template", ""))
                continue
            cmd = build_command(agent, tmpl, prompt)
            if cmd is None:
                print(f"Warning: agent {agent['id']} has empty prompt_flag, skipping", file=sys.stderr)
                failures.append((agent["id"], "skip: empty prompt_flag", ""))
                continue

            source_files = get_source_files(agent)
            cmd = wrap_with_sources(cmd, source_files)
            if args.dry_run:
                print(f"[DRY RUN] tier={tier['id']}  agent={agent['id']}")
                print(f"  command: {cmd}")
                rr_fd.close()
                return

            env = build_env(agent, depth)
            if args.verbose:
                print(f"[attempting {agent['id']}]", file=sys.stderr)

            rc, stderr_snippet = call_agent_with_result(
                agent["id"], tier["id"], cmd, env, args.timeout, args.verbose
            )

            if rc == 0:
                next_agent_id = agents[(start + i + 1) % n]["id"]
                fcntl.flock(rr_fd, fcntl.LOCK_EX)
                rr_fd.seek(0)
                try:
                    rr_state = json.load(rr_fd)
                except Exception:
                    rr_state = {}
                rr_state[tier["id"]] = next_agent_id
                write_rr_state(rr_state)
                fcntl.flock(rr_fd, fcntl.LOCK_UN)
                rr_fd.close()
                sys.exit(0)
            else:
                reason = "timeout" if rc == -signal.SIGKILL else str(rc)
                failures.append((agent["id"], reason, stderr_snippet))

    rr_fd.close()
    print("\nAll agents failed:", file=sys.stderr)
    for agent_id, reason, stderr_snip in failures:
        print(f"  {agent_id}: {reason}", file=sys.stderr)
        if stderr_snip:
            print(f"    stderr: {stderr_snip[:200]}", file=sys.stderr)
    sys.exit(1)


def main():
    depth = int(os.environ.get("DISPATCH_AGENT_DEPTH", 0))
    if depth >= 5:
        print("Error: dispatch recursion limit reached (DISPATCH_AGENT_DEPTH >= 5)", file=sys.stderr)
        sys.exit(1)

    parser = make_parser()
    args = parser.parse_args()

    if args.timeout == 0:
        print("Error: --timeout 0 is invalid, use -1 for no timeout", file=sys.stderr)
        sys.exit(1)

    templates = load_templates()

    if args.list:
        cfg_path = find_config(args.config)
        if cfg_path:
            _cmd_list(load_config(cfg_path), templates)
        else:
            _cmd_list_detect()
        return

    cfg_path = find_config(args.config)
    if cfg_path is None:
        print("No config found. Run with 'init' to configure.", file=sys.stderr)
        sys.exit(1)
    config = load_config(cfg_path)

    if args.show_config:
        _cmd_show_config(config, cfg_path)
        return

    prompt = None
    if args.p:
        prompt = args.p
    elif args.f:
        fpath = Path(args.f)
        if not fpath.exists():
            print(f"Error: file not found: {args.f}", file=sys.stderr)
            sys.exit(1)
        if fpath.stat().st_size > MAX_FILE_BYTES:
            print(f"Error: file {args.f} exceeds 256KB limit", file=sys.stderr)
            sys.exit(1)
        prompt = fpath.read_text()

    if prompt is None and not args.dry_run:
        print("Error: -p or -f required", file=sys.stderr)
        sys.exit(1)

    if args.dry_run and prompt is None:
        prompt = "<prompt>"

    _cmd_dispatch(config, templates, prompt, args, depth)


if __name__ == "__main__":
    main()
