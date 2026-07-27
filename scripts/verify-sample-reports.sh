#!/usr/bin/env bash
set -euo pipefail

REPORT_ROOT="${1:-examples/reports}"

set -- "$REPORT_ROOT"/snippet-local/report-*.json
if [ "$#" -ne 1 ] || [ ! -f "$1" ]; then
  echo "expected exactly one local snippet sample report" >&2
  exit 1
fi
SNIPPET_LOCAL="$1"

set -- "$REPORT_ROOT"/docker/report-*.json
if [ "$#" -ne 1 ] || [ ! -f "$1" ]; then
  echo "expected exactly one Docker snippet sample report" >&2
  exit 1
fi
SNIPPET_DOCKER="$1"

jq -e '
  .manifest.schema_version == 2 and
  .manifest.hash_algorithm == "sha256" and
  .manifest.runner.runner_type == "local" and
  .aggregate.per_model["mock-model"].pass_at_k["1"] == 1 and
  ([.results[].score.overall] | all(. == 1))
' "$SNIPPET_LOCAL" >/dev/null

jq -e '
  .manifest.schema_version == 2 and
  .manifest.hash_algorithm == "sha256" and
  .manifest.runner.runner_type == "docker" and
  (.manifest.runner.docker_image_digest | startswith("sha256:")) and
  .aggregate.per_model["mock-model"].pass_at_k["1"] == 1 and
  ([.results[].score.overall] | all(. == 1))
' "$SNIPPET_DOCKER" >/dev/null

jq -e '
  .schema_version == 2 and
  .redaction.redacted == true and
  .policy.profile == "offline-demo" and
  .policy.verifier_environment == "local" and
  .trials[0].status == "passed" and
  .trials[0].agent.adapter == "scripted" and
  ([.trials[].events[] |
    (.raw == null and .kind != "message" and .kind != "unknown" and .message == .kind)
  ] | all)
' "$REPORT_ROOT/repository-local/report.json" >/dev/null

jq -e '
  .schema_version == 2 and
  .redaction.redacted == true and
  .policy.profile == "offline-demo" and
  .policy.verifier_environment == "docker" and
  .policy.network == "agent=none;verifier=none" and
  .trials[0].status == "passed" and
  .trials[0].agent.adapter == "scripted" and
  ([.trials[].events[] |
    (.raw == null and .kind != "message" and .kind != "unknown" and .message == .kind)
  ] | all)
' "$REPORT_ROOT/repository-docker/report.json" >/dev/null

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

verify_manifest() {
  local directory="$1"
  local manifest="$directory/artifact-manifest.json"
  local relative expected_hash expected_size artifact actual_hash actual_size

  while IFS=$'\t' read -r relative expected_hash expected_size; do
    case "$relative" in
      "" | /* | ../* | */../* | */..)
        echo "unsafe artifact path in $manifest: $relative" >&2
        return 1
        ;;
    esac
    artifact="$directory/$relative"
    if [ ! -f "$artifact" ]; then
      echo "missing artifact declared by $manifest: $relative" >&2
      return 1
    fi
    actual_hash="$(sha256_file "$artifact")"
    actual_size="$(wc -c <"$artifact" | tr -d '[:space:]')"
    if [ "$actual_hash" != "$expected_hash" ] || [ "$actual_size" != "$expected_size" ]; then
      echo "artifact integrity mismatch: $artifact" >&2
      return 1
    fi
  done < <(jq -r '.files[] | [.path, .sha256, (.size_bytes | tostring)] | @tsv' "$manifest")
}

for DIRECTORY in "$REPORT_ROOT/repository-local" "$REPORT_ROOT/repository-docker"; do
  jq -e '
    .schema_version == 1 and
    .hash_algorithm == "sha256" and
    ([.files[].path] | sort) ==
      ["report.html", "report.json", "report.sarif"]
  ' "$DIRECTORY/artifact-manifest.json" >/dev/null
  verify_manifest "$DIRECTORY"
done

if grep -R -q '"avg_latency_ms"\|Avg Latency' "$REPORT_ROOT"; then
  echo "sample reports contain a retired latency field or label" >&2
  exit 1
fi

echo "sample report contract verified"
