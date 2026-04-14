#!/usr/bin/env python3
# classify_intent.py — IntentGate classifier for multi-agent-dispatch
# Usage: python3 scripts/classify_intent.py "implement user authentication"
# Output: {"category": "deep", "agent": "hephaestus", "confidence": "high"}

import json
import sys

CATEGORIES = {
    "orchestrate": {
        "agent": "sisyphus",
        "priority": 0,
        "phrases": [
            "plan and implement",
            "build a full",
            "create entire",
            "multi-step",
            "end-to-end",
        ],
        "words": [],
    },
    "deep": {
        "agent": "hephaestus",
        "priority": 1,
        "phrases": [
            "add feature",
        ],
        "words": [
            "implement",
            "build",
            "create",
            "refactor",
            "rewrite",
            "migrate",
        ],
    },
    "quick": {
        "agent": "hephaestus",
        "priority": 2,
        "phrases": [],
        "words": [
            "fix",
            "typo",
            "rename",
            "update",
            "change",
            "small",
            "simple",
            "tweak",
        ],
    },
    "ultrabrain": {
        "agent": "prometheus",
        "priority": 3,
        "phrases": [
            "analyze complexity",
        ],
        "words": [
            "architecture",
            "design",
            "security",
            "review",
            "audit",
            "trade-off",
        ],
    },
    "visual": {
        "agent": "hephaestus",
        "priority": 4,
        "phrases": [],
        "words": [
            "css",
            "ui",
            "ux",
            "frontend",
            "layout",
            "responsive",
            "design",
            "style",
            "component",
        ],
    },
    "research": {
        "agent": "oracle",
        "priority": 5,
        "phrases": [
            "how does",
            "where is",
            "what is",
        ],
        "words": [
            "find",
            "search",
            "explore",
            "understand",
            "grep",
            "trace",
        ],
    },
    "librarian": {
        "agent": "librarian",
        "priority": 6,
        "phrases": [
            "explain code",
            "api doc",
        ],
        "words": [
            "document",
            "readme",
            "changelog",
            "comment",
            "jsdoc",
        ],
    },
}

PHRASE_WEIGHT = 3
WORD_WEIGHT = 1


def score_category(text, cat_info):
    """Score a category against normalized input text."""
    score = 0
    for phrase in cat_info["phrases"]:
        if phrase in text:
            score += PHRASE_WEIGHT
    for word in cat_info["words"]:
        if word in text.split():
            score += WORD_WEIGHT
    return score


def classify(prompt):
    """Classify a user prompt into a category with agent and confidence."""
    text = prompt.lower().strip()

    scores = {}
    for cat, info in CATEGORIES.items():
        scores[cat] = score_category(text, info)

    ranked = sorted(scores.items(), key=lambda x: (-x[1], CATEGORIES[x[0]]["priority"]))
    top_cat, top_score = ranked[0]
    runner_up_score = ranked[1][1] if len(ranked) > 1 else 0

    if top_score == 0:
        confidence = "low"
        top_cat = "deep"
    elif top_score > 0 and runner_up_score > 0 and top_score - runner_up_score <= 1:
        confidence = "medium"
    else:
        confidence = "high"

    return {
        "category": top_cat,
        "agent": CATEGORIES[top_cat]["agent"],
        "confidence": confidence,
    }


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--categories":
        print(json.dumps(list(CATEGORIES.keys())))
        return

    if len(sys.argv) > 1:
        prompt = sys.argv[1]
    elif not sys.stdin.isatty():
        prompt = sys.stdin.read().strip()
    else:
        print("Usage: classify_intent.py <prompt>", file=sys.stderr)
        sys.exit(1)

    if not prompt:
        print("Error: empty prompt", file=sys.stderr)
        sys.exit(1)

    result = classify(prompt)
    print(json.dumps(result))


if __name__ == "__main__":
    main()
