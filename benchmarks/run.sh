#!/usr/bin/env bash
# ─── spectralint benchmark runner ───────────────────────────────────────────
#
# Reproduces the benchmark numbers from the README.
#
# Prerequisites:
#   - spectralint on PATH (cargo install spectralint)
#   - git, python3
#
# Usage:
#   ./benchmarks/run.sh              # clone + scan (standard mode)
#   ./benchmarks/run.sh --strict     # clone + scan (strict mode)
#   ./benchmarks/run.sh --skip-clone # re-scan without re-cloning
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_LIST="$SCRIPT_DIR/repos.txt"
CLONE_DIR="${SPECTRALINT_BENCH_DIR:-/tmp/spectralint-bench-repos}"
RESULTS_DIR="${SPECTRALINT_BENCH_RESULTS:-/tmp/spectralint-bench-results}"

STRICT=false
SKIP_CLONE=false

for arg in "$@"; do
  case "$arg" in
    --strict)     STRICT=true ;;
    --skip-clone) SKIP_CLONE=true ;;
    --help|-h)
      echo "Usage: $0 [--strict] [--skip-clone]"
      exit 0
      ;;
  esac
done

# ── Clone repos ────────────────────────────────────────────────────────────

if [ "$SKIP_CLONE" = false ]; then
  echo "==> Cloning repos to $CLONE_DIR"
  mkdir -p "$CLONE_DIR"

  while IFS= read -r repo; do
    [[ "$repo" =~ ^#.*$ || -z "$repo" ]] && continue
    dir_name="${repo//\//_}"
    dest="$CLONE_DIR/$dir_name"
    if [ -d "$dest" ]; then
      echo "  skip (exists): $repo"
      continue
    fi
    echo "  cloning: $repo"
    git clone --depth 1 --quiet "https://github.com/$repo.git" "$dest" 2>/dev/null || \
      echo "  WARN: failed to clone $repo"
  done < "$REPO_LIST"
fi

# ── Scan repos ─────────────────────────────────────────────────────────────

echo "==> Scanning repos"
mkdir -p "$RESULTS_DIR"

STRICT_FLAG=""
if [ "$STRICT" = true ]; then
  STRICT_FLAG="--strict"
fi

for dir in "$CLONE_DIR"/*/; do
  repo_name="$(basename "$dir")"
  out="$RESULTS_DIR/$repo_name.json"
  spectralint check "$dir" --format json $STRICT_FLAG > "$out" 2>/dev/null || true
done

# ── Summarise ──────────────────────────────────────────────────────────────

echo "==> Results in $RESULTS_DIR"
python3 "$SCRIPT_DIR/summarise.py" "$RESULTS_DIR"
