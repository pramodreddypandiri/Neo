#!/usr/bin/env python3
"""
token_usage.py — Calculate token usage from a Claude Code session file.

Usage:
    python3 token_usage.py <path-to-session.jsonl>

Example:
    python3 token_usage.py ~/.claude/projects/-Users-.../session-id.jsonl

To find session files:
    ls -lt ~/.claude/projects/<project-folder>/*.jsonl
"""

import sys
import json


def calculate(path: str):
    totals = {"input": 0, "output": 0, "cache_read": 0, "cache_write": 0}

    with open(path) as f:
        for line in f:
            try:
                obj = json.loads(line)
                u = obj.get("message", {}).get("usage", {})
                totals["input"]       += u.get("input_tokens", 0)
                totals["output"]      += u.get("output_tokens", 0)
                totals["cache_read"]  += u.get("cache_read_input_tokens", 0)
                totals["cache_write"] += u.get("cache_creation_input_tokens", 0)
            except (json.JSONDecodeError, AttributeError):
                pass

    total = sum(totals.values())

    print(f"\nSession: {path.split('/')[-1]}")
    print(f"{'─' * 35}")
    print(f"  Input:       {totals['input']:>10,}")
    print(f"  Output:      {totals['output']:>10,}")
    print(f"  Cache read:  {totals['cache_read']:>10,}")
    print(f"  Cache write: {totals['cache_write']:>10,}")
    print(f"{'─' * 35}")
    print(f"  Total:       {total:>10,}\n")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)
    calculate(sys.argv[1])
