#!/usr/bin/env python3
"""Compare two criterion baselines (dashu vs rug).

Usage: python3 scripts/bench_compare.py [--dashu cmp-dashu] [--rug cmp-rug]

For each bench id, prints the median time from each baseline and the rug/dashu
ratio (>1 means dashu wins). Reads target/criterion/<...>/<baseline>/estimates.json.
"""
from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path


def find_baselines(root: Path, baseline_name: str) -> dict[str, float]:
    """Return {bench_id: median_ns} for every estimates.json under target/criterion
    whose immediate parent directory is `baseline_name`.

    The bench_id is the path from `root` up to (but not including) the baseline
    directory, joined by '/'. Matches criterion's own naming.
    """
    out: dict[str, float] = {}
    for est in root.rglob("estimates.json"):
        if est.parent.name != baseline_name:
            continue
        # bench id = path from root to estimates.json, minus the baseline name and filename
        rel = est.relative_to(root).parent.parent  # drop baseline_name + estimates.json
        bench_id = "/".join(rel.parts)
        try:
            data = json.loads(est.read_text())
            median_ns = data["median"]["point_estimate"]
        except (KeyError, json.JSONDecodeError):
            continue
        out[bench_id] = median_ns
    return out


def fmt_time(ns: float) -> str:
    if ns < 1_000:
        return f"{ns:.2f} ns"
    if ns < 1_000_000:
        return f"{ns / 1_000:.2f} µs"
    if ns < 1_000_000_000:
        return f"{ns / 1_000_000:.2f} ms"
    return f"{ns / 1_000_000_000:.2f} s"


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--root", default="target/criterion")
    p.add_argument("--dashu", default="cmp-dashu")
    p.add_argument("--rug", default="cmp-rug")
    p.add_argument("--filter", default=None, help="only show bench ids containing this substring")
    p.add_argument("--by-ratio", action="store_true", help="sort by rug/dashu ratio (worst dashu first)")
    args = p.parse_args()

    root = Path(args.root)
    dashu = find_baselines(root, args.dashu)
    rug = find_baselines(root, args.rug)

    common = sorted(set(dashu) & set(rug))
    if args.filter:
        common = [b for b in common if args.filter in b]
    if args.by_ratio:
        # Sort by rug/dashu ratio descending — biggest dashu wins last
        common.sort(key=lambda b: rug[b] / dashu[b])

    only_dashu = sorted(set(dashu) - set(rug))
    only_rug = sorted(set(rug) - set(dashu))

    if only_dashu:
        print(f"# Benches only in {args.dashu}: {len(only_dashu)}", file=sys.stderr)
    if only_rug:
        print(f"# Benches only in {args.rug}: {len(only_rug)}", file=sys.stderr)

    # Markdown table
    print("| Bench | dashu | rug | rug/dashu |")
    print("|---|---:|---:|---:|")
    dashu_wins = 0
    rug_wins = 0
    geo_log_sum = 0.0
    for b in common:
        d = dashu[b]
        r = rug[b]
        ratio = r / d
        if ratio > 1.05:
            dashu_wins += 1
        elif ratio < 0.95:
            rug_wins += 1
        geo_log_sum += math.log(ratio)
        print(f"| `{b}` | {fmt_time(d)} | {fmt_time(r)} | {ratio:.2f}× |")

    n = len(common)
    if n:
        gmean = math.exp(geo_log_sum / n)
        print()
        print(f"**Summary**: {n} benches compared. "
              f"dashu wins (>5%): {dashu_wins}. "
              f"rug wins (>5%): {rug_wins}. "
              f"Geometric mean rug/dashu ratio: {gmean:.2f}× "
              f"({'dashu' if gmean > 1 else 'rug'} faster on average).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
