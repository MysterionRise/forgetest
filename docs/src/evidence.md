# Evidence

This page separates executable proof from roadmap claims. The linked reports
are deterministic offline evidence generated with bundled mock or scripted
agents. They exercise real compilation, tests, grading, patch capture,
redaction, and report rendering; they are not model benchmark results. The
links resolve after the Pages workflow has completed on the default branch.

![Repository trial report](assets/repository-report.png)

## Inspect the Artifacts

- [Repository demo, local verifier](https://mysterionrise.github.io/forgetest/reports/repository-local/report.html)
- [Repository demo, Docker verifier](https://mysterionrise.github.io/forgetest/reports/repository-docker/report.html)
- [Snippet demo, local runner](https://mysterionrise.github.io/forgetest/reports/snippet-local/report-2026-07-27T155019.html)
- [Snippet demo, Docker runner](https://mysterionrise.github.io/forgetest/reports/docker/report-2026-07-27T155934.html)
- [CI workflow](https://github.com/MysterionRise/forgetest/actions/workflows/ci.yml)
- [Release workflow](https://github.com/MysterionRise/forgetest/actions/workflows/release.yml)

Each repository report is accompanied by redacted JSON, deterministic SARIF,
and an artifact manifest containing SHA-256 digests and byte sizes.

## Proof Ledger

| Claim | Evidence | Boundary |
|---|---|---|
| The no-key repository loop runs end to end | Local and Docker CI jobs plus browser reports | Uses a deterministic scripted agent |
| All 12 corpus tasks are calibrated | CI runs null-patch and reference-patch calibration | Calibration is not agent performance |
| Docker verification is hardened as documented | Gated integration tests and Docker demo | Not a hostile-repository sandbox |
| Release assets are packaged, checksummed, attested, and SBOM-backed | Green tagged release workflow | Applies only after a successful tag run |
| Real-agent performance claim | None | The [pilot](https://github.com/MysterionRise/forgetest/tree/master/studies/rust-agent-pilot) and 72-trial study remain unexecuted |

## Reproduce Locally

```bash
cargo run --locked --bin forgetest -- \
  demo --mode repository --runner local \
  --output ./forgetest-results --format all

bash scripts/verify-sample-reports.sh
```

Build exactly what GitHub Pages publishes:

```bash
bash scripts/build-pages.sh
bash scripts/test-build-pages.sh
```

The source report, screenshot, and hosted copy are intentionally committed or
built from public redacted evidence only. Raw external-agent traces are never
published without an explicit redaction review.
