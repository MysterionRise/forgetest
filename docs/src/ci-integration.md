# CI Integration

forgetest is designed for CI pipelines. Use it to continuously monitor LLM code generation quality.

## GitHub Actions

### Basic Eval Run

```yaml
name: LLM Eval

on:
  schedule:
    - cron: '0 6 * * 1'  # Weekly on Mondays
  workflow_dispatch:       # Manual trigger

jobs:
  eval:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy

      - name: Install forgetest
        run: cargo install --path crates/forgetest-cli

      - name: Run evaluations
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
        run: |
          forgetest run \
            --eval-set eval-sets/rust-basics.toml \
            --models anthropic/claude-sonnet-4-20250514 \
            --format all \
            --output results/

      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: eval-results
          path: results/
```

### Regression Detection

Compare results against a saved baseline:

```yaml
      - name: Download baseline
        uses: actions/download-artifact@v4
        with:
          name: eval-baseline
          path: baseline/
        continue-on-error: true

      - name: Check for regressions
        run: |
          if [ -f baseline/report.json ]; then
            forgetest compare \
              --baseline baseline/report.json \
              --current results/report-*.json \
              --fail-on-regression
          fi
```

### SARIF Upload (GitHub Code Scanning)

Upload SARIF reports to see eval failures in the Security tab:

```yaml
      - name: Upload SARIF
        if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: results/
```

## Exit Codes

| Exit Code | Meaning |
|-----------|---------|
| 0 | Success |
| 1 | Error or regression detected (with `--fail-on-regression`) |

## Report Formats

| Format | Flag | Use Case |
|--------|------|----------|
| JSON | `--format json` | Machine-readable, baseline storage |
| HTML | `--format html` | Human review, sharing results |
| SARIF | `--format sarif` | GitHub Code Scanning integration |
| All | `--format all` | Generate all formats at once |

## Multi-Model Comparison in CI

Compare multiple models in a single run:

```yaml
      - name: Run multi-model eval
        run: |
          forgetest run \
            --eval-set eval-sets/rust-basics.toml \
            --models anthropic/claude-sonnet-4-20250514,openai/gpt-4.1 \
            --pass-k 1,5 \
            --temperature 0.8 \
            --parallelism 8 \
            --format json \
            --output results/
```

## Tips

- **Use `--parallelism`** to speed up runs (default: 4).
- **Pin model versions** in CI for reproducible results.
- **Store baselines as artifacts** for regression detection across runs.
- **Run on a schedule** rather than every push to manage API costs.
- **Use `--filter`** to run only specific case subsets in PR checks.
