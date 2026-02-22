#!/usr/bin/env bash
set -euo pipefail

# release.sh — Publish forgetest crates to crates.io in dependency order.
#
# Usage:
#   ./release.sh           # Dry-run (default)
#   ./release.sh --execute # Actually publish

DRY_RUN=true
if [[ "${1:-}" == "--execute" ]]; then
    DRY_RUN=false
fi

# Publishing order respects the dependency graph:
#   1. forgetest-core       (no internal deps)
#   2. forgetest-providers  (depends on core)
#   3. forgetest-runner     (depends on core)
#   4. forgetest-report     (depends on core)
#   5. forgetest-cli        (depends on all)
CRATES=(
    "crates/forgetest-core"
    "crates/forgetest-providers"
    "crates/forgetest-runner"
    "crates/forgetest-report"
    "crates/forgetest-cli"
)

DELAY=30  # seconds between publishes for crates.io indexing

for crate in "${CRATES[@]}"; do
    name=$(basename "$crate")
    echo "==> Publishing $name ..."

    if $DRY_RUN; then
        cargo publish --dry-run -p "$name"
        echo "    (dry-run OK)"
    else
        cargo publish -p "$name"
        echo "    Published $name"

        # Wait for crates.io to index before publishing dependents
        if [[ "$crate" != "${CRATES[-1]}" ]]; then
            echo "    Waiting ${DELAY}s for crates.io indexing ..."
            sleep "$DELAY"
        fi
    fi

    echo ""
done

if $DRY_RUN; then
    echo "Dry-run complete. Run with --execute to publish for real."
else
    echo "All crates published successfully!"
    echo ""
    echo "Verify installation:"
    echo "  cargo install forgetest-cli"
fi
