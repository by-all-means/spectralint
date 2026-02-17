#!/usr/bin/env python3
"""Summarise spectralint benchmark results.

Usage:
    python3 summarise.py /path/to/results/
"""
import json, os, sys, glob
from collections import Counter

if len(sys.argv) < 2:
    print(f"Usage: {sys.argv[0]} <results_dir>", file=sys.stderr)
    sys.exit(1)

results_dir = sys.argv[1]
files = sorted(glob.glob(os.path.join(results_dir, "*.json")))

if not files:
    print(f"No JSON files found in {results_dir}", file=sys.stderr)
    sys.exit(1)

rule_counts = Counter()
severity_counts = Counter()
repos_with_findings = set()
repos_with_errors_or_warnings = set()
total_repos = 0

for jf in files:
    total_repos += 1
    repo = os.path.basename(jf).replace(".json", "")

    with open(jf) as f:
        text = f.read().strip()
        if not text:
            continue
        data = json.loads(text)

    for d in data.get("diagnostics", []):
        rule = d["category"]
        sev = d["severity"]
        rule_counts[rule] += 1
        severity_counts[sev] += 1
        repos_with_findings.add(repo)
        if sev in ("error", "warning"):
            repos_with_errors_or_warnings.add(repo)

total_findings = sum(rule_counts.values())
pct_with_findings = len(repos_with_findings) * 100 // total_repos if total_repos else 0
pct_errors_warnings = len(repos_with_errors_or_warnings) * 100 // total_repos if total_repos else 0

print(f"\n{'=' * 60}")
print(f"  spectralint benchmark — {total_repos} repos scanned")
print(f"{'=' * 60}")
print(f"  Total findings:           {total_findings}")
print(f"  Repos with any finding:   {len(repos_with_findings)} ({pct_with_findings}%)")
print(f"  Repos with error/warning: {len(repos_with_errors_or_warnings)} ({pct_errors_warnings}%)")
print()

print("  Severity breakdown:")
for sev in ("error", "warning", "info"):
    print(f"    {sev:>8}: {severity_counts.get(sev, 0)}")
print()

print("  Findings by rule:")
for rule, count in rule_counts.most_common():
    repos_hit = sum(1 for jf in files
                    if any(d["category"] == rule
                           for d in json.loads(open(jf).read().strip() or "{}").get("diagnostics", [])))
    print(f"    {rule:<32} {count:>4}  ({repos_hit} repos)")

print(f"\n{'=' * 60}")
