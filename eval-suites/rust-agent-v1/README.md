# Rust Agent v1 Corpus

This suite contains eight authored fixtures and four fixture-focused
adaptations of audited upstream fixes. Adapted tasks are not full upstream
repository checkouts and are not claimed to reproduce every upstream behavior.
Each records its source commit, license, and audit date in `task.toml`.

| Category | Tasks |
|---|---:|
| Bug fix | 3 |
| Feature | 3 |
| API migration | 2 |
| Async/concurrency | 2 |
| Security/robustness | 2 |

Every task has a fail-to-pass hidden check and a pass-to-pass regression check.
The committed reference patches are trusted calibration oracles, not agent
outputs and not visible to agents.

```bash
forgetest validate --suite eval-suites/rust-agent-v1/suite.toml
forgetest validate --suite eval-suites/rust-agent-v1/suite.toml --calibrate
```

Calibration executes trusted commands on the host. Review third-party suites
before running it.
