# Outcomes and Statistics

## Repository Tasks

Repository tasks use a binary primary outcome. A trial passes only when every
required named verifier check succeeds after the agent patch is applied to a
fresh workspace.

Checks normally include:

- `fail_to_pass`: behavior that is broken in the visible workspace and must be
  fixed.
- `pass_to_pass`: existing behavior that must remain working.
- Optional deterministic compile or Clippy checks.

A null patch must fail calibration. The trusted reference patch must pass all
checks. Weighted partial credit does not turn a broken repository patch into a
pass.

## Status Accounting

Task failures, agent failures, timeouts, verifier failures, and environment
failures are distinct. Reports expose both:

- Observed resolution rate over every scheduled trial.
- Valid-trial resolution rate excluding infrastructure errors and cancellation.

Agent process errors and timeouts remain agent outcomes, not infrastructure
exclusions.

## Confidence Intervals

For each exact agent identity, reports include a Wilson 95% interval around the
observed pass rate. This is more informative than a bare percentage for a small
task corpus.

The interval describes uncertainty in the observed binomial rate. It does not
make the 12 tasks representative of all Rust engineering work.

## Reliability Metrics

- `pass@1`: fraction of tasks whose first trial passed.
- `pass^3`: fraction of tasks where the first three trials all passed.

`pass^3` is deliberately strict. It answers whether an agent configuration is
reliably successful across three attempts, not whether any one attempt worked.

## Paired Comparison

When two agents share tasks, `forgetest` computes task-level resolution-rate
differences and a deterministic paired bootstrap 95% interval for
`agent B - agent A`. Pairing controls for task difficulty better than comparing
two unrelated aggregate percentages.

The bootstrap uses 10,000 deterministic resamples. A dated study should publish
the point estimate, interval, task count, raw status counts, costs, and
infrastructure failures.

## Legacy Snippet Score

Function-level snippet reports retain the original diagnostic score:

| Component | Weight |
|---|---:|
| Compilation | 30% |
| Tests | 45% |
| Structure | 15% |
| Clippy | 10% |

Compilation failure sets the overall score to zero. New reports persist the
score computed with the original expectations. Old reports remain readable and
fall back to score recomputation.

Legacy snippet Pass@k uses the unbiased estimator:

```text
Pass@k = 1 - C(n-c, k) / C(n, k)
```

Snippet aggregate timing is labeled average trial duration because it includes
provider, compile, test, and Clippy work, not only model latency.
