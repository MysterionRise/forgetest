# Synthetic fixture

## BMAD contributor workflow

BMAD Method 6.11.0 is shared through [AGENTS.md](AGENTS.md) and
[CLAUDE.md](CLAUDE.md). Read [_bmad-output/project-context.md](_bmad-output/project-context.md),
[_bmad-output/authorities.yaml](_bmad-output/authorities.yaml), and
[_bmad-output/index.yaml](_bmad-output/index.yaml). Run
`scripts/verify-bmad.sh`. Minor or major updates require a new approved plan.
Internal artifacts remain outside the public `docs/src` mdBook.

BMAD launcher rendering requires `uv` and follows the renderer's PEP 723
`requires-python = ">=3.11"` declaration. The repository BMAD verifier is
separate and requires Python >=3.10.
