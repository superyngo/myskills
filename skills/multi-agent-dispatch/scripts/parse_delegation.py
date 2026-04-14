#!/usr/bin/env python3
# parse_delegation.py — Parse Sisyphus delegation blocks and Hephaestus completion reports
# Usage: echo "$SISYPHUS_OUTPUT" | python3 scripts/parse_delegation.py [--format json|dispatch|summary]

import argparse
import json
import re
import sys

# Fields expected inside each block type
DELEGATE_FIELDS = ("category", "agent", "task", "context", "output_format")
COMPLETION_FIELDS = ("status", "files_changed", "summary", "blockers")

# Timeouts by category for dispatch format
CATEGORY_TIMEOUTS = {
    "deep": 600,
    "quick": 120,
    "ultrabrain": 600,
    "visual": 300,
    "research": 300,
    "orchestrate": 600,
    "librarian": 300,
}
DEFAULT_TIMEOUT = 300


def _extract_field(block_text, field_name):
    """Extract the text content of a single XML-style field from a block."""
    pattern = rf"<{field_name}>(.*?)</{field_name}>"
    match = re.search(pattern, block_text, re.DOTALL)
    if match:
        return match.group(1).strip()
    return ""


def parse_delegate_blocks(text):
    """Extract all <delegate>...</delegate> blocks and return parsed dicts."""
    blocks = []
    for match in re.finditer(r"<delegate>(.*?)</delegate>", text, re.DOTALL):
        raw = match.group(1)
        block = {}
        for field in DELEGATE_FIELDS:
            block[field] = _extract_field(raw, field)
        blocks.append(block)
    return blocks


def parse_completion_blocks(text):
    """Extract all <completion_report>...</completion_report> blocks."""
    blocks = []
    for match in re.finditer(
        r"<completion_report>(.*?)</completion_report>", text, re.DOTALL
    ):
        raw = match.group(1)
        block = {}
        for field in COMPLETION_FIELDS:
            block[field] = _extract_field(raw, field)
        blocks.append(block)
    return blocks


def extract_raw_text(text):
    """Return text outside all recognised XML blocks, collapsed to single spaces."""
    cleaned = re.sub(
        r"<delegate>.*?</delegate>", "", text, flags=re.DOTALL
    )
    cleaned = re.sub(
        r"<completion_report>.*?</completion_report>", "", cleaned, flags=re.DOTALL
    )
    # Collapse whitespace runs into single spaces
    cleaned = re.sub(r"\s+", " ", cleaned).strip()
    return cleaned


def warn_malformed(text):
    """Emit stderr warnings for unmatched opening/closing tags."""
    for tag in ("delegate", "completion_report"):
        opens = len(re.findall(rf"<{tag}>", text))
        closes = len(re.findall(rf"</{tag}>", text))
        if opens != closes:
            print(
                f"warning: mismatched <{tag}> tags — "
                f"{opens} opening vs {closes} closing (skipping orphans)",
                file=sys.stderr,
            )


def format_json(delegations, completions, raw_text):
    """Return the canonical JSON structure."""
    return json.dumps(
        {
            "delegations": delegations,
            "completion_reports": completions,
            "raw_text": raw_text,
        },
        indent=2,
    )


def format_dispatch(delegations):
    """Return shell dispatch commands, one per delegation."""
    lines = []
    for d in delegations:
        cat = d.get("category") or "deep"
        task = d.get("task", "").replace('"', '\\"')
        timeout = CATEGORY_TIMEOUTS.get(cat, DEFAULT_TIMEOUT)
        lines.append(f'dispatch_agent {cat} "{task}" --timeout {timeout}')
    return "\n".join(lines)


def format_summary(delegations, completions, raw_text):
    """Return a human-readable summary."""
    parts = []

    if delegations:
        parts.append(f"=== Delegations ({len(delegations)}) ===")
        for i, d in enumerate(delegations, 1):
            agent = d.get("agent") or "unspecified"
            cat = d.get("category") or "?"
            task = d.get("task") or "(no task)"
            parts.append(f"  {i}. [{cat}] {agent}: {task}")

    if completions:
        parts.append(f"\n=== Completion Reports ({len(completions)}) ===")
        for i, c in enumerate(completions, 1):
            status = c.get("status") or "unknown"
            summary = c.get("summary") or "(no summary)"
            parts.append(f"  {i}. [{status}] {summary}")
            blockers = c.get("blockers")
            if blockers:
                parts.append(f"     Blockers: {blockers}")

    if raw_text:
        parts.append(f"\n=== Additional Output ===\n  {raw_text}")

    if not parts:
        parts.append("(no delegation or completion blocks found)")

    return "\n".join(parts)


def main():
    parser = argparse.ArgumentParser(
        description="Parse Sisyphus delegation blocks and Hephaestus completion reports."
    )
    parser.add_argument(
        "--format",
        choices=("json", "dispatch", "summary"),
        default="json",
        help="Output format (default: json)",
    )
    args = parser.parse_args()

    if sys.stdin.isatty():
        print("Error: no input on stdin (pipe agent output into this script)", file=sys.stderr)
        sys.exit(1)

    text = sys.stdin.read()
    if not text.strip():
        print("Error: empty input", file=sys.stderr)
        sys.exit(1)

    warn_malformed(text)

    delegations = parse_delegate_blocks(text)
    completions = parse_completion_blocks(text)
    raw_text = extract_raw_text(text)

    if args.format == "json":
        print(format_json(delegations, completions, raw_text))
    elif args.format == "dispatch":
        output = format_dispatch(delegations)
        if output:
            print(output)
    elif args.format == "summary":
        print(format_summary(delegations, completions, raw_text))


if __name__ == "__main__":
    main()
