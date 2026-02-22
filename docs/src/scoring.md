# Scoring

forgetest uses a multi-component scoring system to evaluate LLM-generated code quality.

## Score Components

Each eval result receives a score from 0.0 to 1.0 composed of three weighted components:

| Component | Weight | Description |
|-----------|--------|-------------|
| **Compilation** | 40% | Does the code compile without errors? Binary: 0 or 1. |
| **Tests** | 50% | Fraction of test cases that pass: `passed / (passed + failed)`. |
| **Clippy** | 10% | Penalty for clippy warnings: `max(0, 1 - warnings * 0.1)`. |

### Score Formula

```
if compilation fails:
    overall = 0.0
else:
    test_score = passed / (passed + failed)    # 0.0 if no tests
    clippy_score = max(0.0, 1.0 - warnings * 0.1)
    overall = 0.4 + test_score * 0.5 + clippy_score * 0.1
```

Key behaviors:

- **Compilation failure zeroes everything** — if the code doesn't compile, the score is 0.0 regardless of other factors.
- **Tests dominate** — the 50% weight means test pass rate is the most important factor.
- **Clippy is a bonus** — clean code gets 10%, each warning deducts 1%.
- **A perfect score is 1.0** — compiles (0.4) + all tests pass (0.5) + no clippy warnings (0.1).

## Pass@k

Pass@k answers: "If I sample k code generations, what's the probability that at least one is correct?"

forgetest uses the **unbiased estimator** from the [Codex paper](https://arxiv.org/abs/2107.03374) (Chen et al., 2021):

```
Pass@k = 1 - C(n-c, k) / C(n, k)
```

Where:

- `n` = total number of samples generated
- `c` = number of correct (passing) samples
- `k` = the k in Pass@k

This is computed in log-space to avoid numerical overflow with large values.

### Interpreting Pass@k

| Metric | Meaning |
|--------|---------|
| Pass@1 | Probability of getting correct code on the first try |
| Pass@5 | Probability that at least 1 of 5 samples is correct |
| Pass@10 | Probability that at least 1 of 10 samples is correct |

### Running with multiple samples

To compute Pass@5, you need at least 5 samples per case:

```bash
forgetest run --eval-set eval-sets/rust-basics.toml --pass-k 1,5 --temperature 0.8
```

Note: Temperature > 0 is recommended when computing Pass@k > 1 to get diverse samples.

## Regression Detection

Compare two eval reports to detect score changes:

```bash
forgetest compare --baseline old-report.json --current new-report.json
```

A **regression** is detected when a case's score drops by more than the threshold (default: 5%). An **improvement** is when it increases by more than the threshold.

```bash
# Fail CI if any regressions are found
forgetest compare --baseline baseline.json --current latest.json \
  --fail-on-regression --threshold 0.05
```

Output formats:

```bash
forgetest compare --baseline a.json --current b.json --format text     # default
forgetest compare --baseline a.json --current b.json --format json
forgetest compare --baseline a.json --current b.json --format markdown
```

## Aggregate Statistics

Reports include per-model and per-case aggregate statistics:

### Per-Model Stats

- **Pass@k** for each requested k value
- **Average compilation rate** — fraction of samples that compile
- **Average test pass rate** — average (passed/total) across cases
- **Total cost** — sum of API costs
- **Average latency** — mean LLM request time

### Per-Case Stats

- **Pass@k** per model
- **Average score** across attempts
