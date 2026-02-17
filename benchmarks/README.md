# Benchmark Reproduction

This directory contains everything needed to reproduce the benchmark numbers from the project README.

## Methodology

1. **Repo selection**: GitHub code search for `filename:CLAUDE.md`, ranked by `stargazers_count`, top 100 results (February 2026)
2. **Cloning**: Shallow clones (`--depth 1`) of each repository
3. **Scanning**: `spectralint check <repo> --format json` for each repo
4. **Aggregation**: Python script summarises findings by rule and severity

## Files

| File | Description |
|------|-------------|
| `repos.txt` | The 100 GitHub repositories used in the benchmark |
| `run.sh` | End-to-end script: clone repos, scan, summarise |
| `summarise.py` | Aggregates JSON results into a summary table |

## Quick Start

```bash
# Standard mode
./benchmarks/run.sh

# Strict mode (enables opinionated checkers)
./benchmarks/run.sh --strict

# Re-scan without re-cloning
./benchmarks/run.sh --skip-clone
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SPECTRALINT_BENCH_DIR` | `/tmp/spectralint-bench-repos` | Where repos are cloned |
| `SPECTRALINT_BENCH_RESULTS` | `/tmp/spectralint-bench-results` | Where JSON results are written |

## Notes

- Repos are shallow-cloned to minimise disk usage (~2 GB total)
- Cloning is idempotent — existing repos are skipped on re-run
- Results may vary slightly over time as upstream repos update their instruction files
- The scan runs against all markdown files spectralint discovers, not just `CLAUDE.md`
