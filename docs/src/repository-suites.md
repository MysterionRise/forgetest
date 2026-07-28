# Repository Suites

## Layout

```text
my-suite/
  suite.toml
  tasks/
    fix-boundary/
      task.toml
      prompt.md
      reference.patch
      workspace/
        Cargo.toml
        Cargo.lock
        src/
      grader/
        tests/
```

The visible `workspace` is copied to the agent. The `grader` is not.

## Suite Manifest

```toml
schema_version = 2
id = "my-rust-suite"
name = "My Rust Suite"
description = "Audited repository tasks."

[[tasks]]
id = "fix-boundary"
path = "tasks/fix-boundary"
```

IDs must be unique ASCII identifiers. Unknown fields and path escapes fail.

## Task Manifest

```toml
schema_version = 1
id = "fix-boundary"
name = "Fix exclusive boundary"
description = "Correct an off-by-one error."
category = "bug_fix"
language = "rust"
prompt = "prompt.md"
workspace = "workspace"
grader = "grader"
reference_patch = "reference.patch"
timeout_secs = 120
tags = ["boundary"]

[verifier]
timeout_secs = 60

[[verifier.checks]]
name = "boundary regression"
kind = "fail_to_pass"
command = ["cargo", "test", "--test", "hidden", "--locked"]

[[verifier.checks]]
name = "existing tests"
kind = "pass_to_pass"
command = ["cargo", "test", "--lib", "--locked"]

[provenance]
kind = "authored"
license = "MIT OR Apache-2.0"
audited_at = "2026-07-27"
```

Snapshot provenance also requires `source_url` and `source_revision`.

## Categories

- `bug_fix`
- `feature`
- `api_migration`
- `async_concurrency`
- `security_robustness`

These categories support corpus balance; they do not affect grading.

## Verifier Checks

Use one legacy `verifier.command` or one or more `verifier.checks`, never both.
Named checks are preferred because reports can separate:

- `fail_to_pass`
- `pass_to_pass`
- `compile`
- `clippy`
- `other`

Commands are trusted suite configuration. Only run suites you have reviewed.

## Calibration

Every publishable task should have a reference patch:

```bash
forgetest validate --suite my-suite/suite.toml --calibrate
```

Admission fails when:

- The null workspace passes.
- The reference patch is missing.
- The reference patch cannot be applied.
- Any check fails after the reference patch.

The reference patch is never shown to the agent.

## Content Identity

Task SHA-256 covers trusted metadata, prompt, visible workspace, protected
grader, and reference patch. Suite SHA-256 covers suite metadata, task
membership/order, and task identities. Editing any of those values makes old
benchmark locks and gating comparisons invalid.
