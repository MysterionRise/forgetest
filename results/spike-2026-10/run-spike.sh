#!/usr/bin/env bash
# S2 live spike (spec-live-spike): OpenRouter fact-finding for S3, S4 and
# pilot-r2, run in throwaway containers on the maintainer's machine. Everything
# this script produces is fact-finding, not evidence.
#
# The OpenRouter key is read from FORGETEST_SPIKE_KEY and then removed from
# this script's environment, so child processes do not inherit it. A single
# command that needs it gets it as an environment variable (`docker run --env
# NAME` takes the value from the docker client's environment) or on stdin,
# never as an argument, so it never shows in `ps` argument lists. The script
# never prints it or writes it to a file on the host, and refuses to run with
# xtrace on. Docker keeps --env values in the container config while the
# container exists, and anyone with daemon access can see them with
# `docker inspect`.
#
# Raw output goes to results/spike-2026-10/raw/, which is gitignored;
# `fixtures` writes the redacted golden streams for S4.
set -euo pipefail

case $- in
  *x*)
    echo "run-spike.sh: refusing to run with xtrace on (set -x or bash -x): it would print the key" >&2
    exit 2
    ;;
esac
SPIKE_KEY="${FORGETEST_SPIKE_KEY:-}"
unset FORGETEST_SPIKE_KEY

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
ROOT="$(cd "$HERE/../.." && pwd -P)"
RAW="$HERE/raw"
TOOLS="$HERE/spike_tools.py"
CANARY="$HERE/canary"
FIXTURES="$ROOT/crates/forgetest-agents/tests/fixtures/openrouter"

PLAN_ID="spec-live-spike"
# The run's own spend cap, measured from billed usage at preflight.
CAP_USD="2.00"
# Key gates checked at preflight: the approved S2 conditions are a key limit
# of at most USD 2 and an expiry within 48 hours (with half an hour of slack).
# Overriding either through SPIKE_KEY_LIMIT_MAX_USD or
# SPIKE_KEY_EXPIRY_MAX_HOURS needs a Don decision record; the run record shows
# the effective values.
KEY_LIMIT_MAX_USD="${SPIKE_KEY_LIMIT_MAX_USD:-2.00}"
KEY_EXPIRY_MAX_HOURS="${SPIKE_KEY_EXPIRY_MAX_HOURS:-48.5}"
if [ -n "${SPIKE_KEY_LIMIT_MAX_USD:-}${SPIKE_KEY_EXPIRY_MAX_HOURS:-}" ]; then
  KEY_GATES="override"
else
  KEY_GATES="default"
fi
# Headroom for usage that OpenRouter has not yet accounted when the budget is
# read before a scenario.
RESERVE_USD="0.10"
CODEX_IMAGE_TAG="${SPIKE_CODEX_IMAGE:-localhost:5001/forgetest-codex:0.163.0}"
CLAUDE_IMAGE_TAG="${SPIKE_CLAUDE_IMAGE:-localhost:5001/forgetest-claude:2.1.302}"
OPENROUTER="https://openrouter.ai"

# The four study models from the 2026-10-18 roadmap amendment, and the single
# model outside a key's model allowlist used by the guardrail negative test.
# That test runs only when named, and only with SPIKE_GUARDRAIL_ALLOWLISTED=1,
# because it means something only for a key that has a model allowlist.
CLAUDE_MODEL="anthropic/claude-haiku-5.5"
CLAUDE_CALIBRATION_MODEL="anthropic/claude-sonnet-5.5"
CODEX_MODEL="openai/gpt-6-luna"
CODEX_CALIBRATION_MODEL="openai/gpt-6.1-sol"
NEGATIVE_MODEL="openai/gpt-5.6-luna"

DEFAULT_SCENARIOS="slugs capture git sandbox key-before live-slugs claude-auth codex-provider claude-canary codex-canary key-after"
ALL_SCENARIOS="$DEFAULT_SCENARIOS guardrail"
KEYED_SCENARIOS="key-before live-slugs claude-auth codex-provider guardrail claude-canary codex-canary key-after"
SPENDING_SCENARIOS="live-slugs claude-auth codex-provider guardrail claude-canary codex-canary"
KEYLESS_SCENARIOS="slugs capture git sandbox"

# Worst-case spend per spending scenario in USD, many times what the token
# counts at the Row 1 prices suggest. A scenario starts only when the
# remaining budget covers its estimate plus RESERVE_USD.
estimate_usd() {
  case "$1" in
    live-slugs | guardrail) echo 0.02 ;;
    claude-auth | codex-provider) echo 0.10 ;;
    claude-canary | codex-canary) echo 0.40 ;;
    *) echo 0 ;;
  esac
}

# Profile arguments from CommandProfile::codex and CommandProfile::claude in
# crates/forgetest-agents/src/lib.rs at the baseline commit. Codex reads the
# prompt from stdin through the trailing `-`, appended at run time.
CODEX_PROFILE=(codex exec --json --ephemeral --sandbox workspace-write
  --ignore-user-config --ignore-rules)
CLAUDE_PROFILE=(claude --print --bare --output-format stream-json
  --no-session-persistence --permission-mode bypassPermissions)

# Settings applied on every spike call: medium effort and web tools off.
CODEX_SPIKE=(-c 'model_reasoning_effort="medium"' -c 'web_search="disabled"')
CLAUDE_SPIKE=(--verbose --effort medium --disallowedTools WebSearch,WebFetch)
CODEX_OPENROUTER='model_providers.openrouter={name="OpenRouter",base_url="https://openrouter.ai/api/v1",env_key="OPENROUTER_API_KEY",wire_api="responses"}'
CODEX_CAPTURE='model_providers.openrouter={name="OpenRouter",base_url="http://capture:8080/api/v1",env_key="OPENROUTER_API_KEY",wire_api="responses",request_max_retries=0,stream_max_retries=0}'
CLAUDE_HELPER='{"apiKeyHelper":"printenv OPENROUTER_API_KEY"}'
SHORT_PROMPT="Reply with the single word ok. Do not use any tools."

usage() {
  cat <<'EOF'
usage: results/spike-2026-10/run-spike.sh [SCENARIO...]
       results/spike-2026-10/run-spike.sh fixtures [RUN_DIR]

With no SCENARIO, runs these scenarios in plan order:
  slugs capture git sandbox key-before live-slugs claude-auth codex-provider
  claude-canary codex-canary key-after
`guardrail` runs only when named, and only with SPIKE_GUARDRAIL_ALLOWLISTED=1:
it means something only for a key that has a model allowlist.

The first four need no key. If any other scenario is requested, the script
fails closed before running anything unless FORGETEST_SPIKE_KEY holds a key
that GET /api/v1/key accepts, whose spend limit is at most
SPIKE_KEY_LIMIT_MAX_USD (default 2.00), and whose expiry is reported and at
most SPIKE_KEY_EXPIRY_MAX_HOURS away (default 48.5). Overriding either gate
needs a Don decision record.

The run's spend is billed usage less the usage read at preflight. The budget
is checked between scenarios: a spending scenario starts only when the
remaining budget covers its worst-case estimate plus a USD 0.10 reserve. The
remaining budget is the least of the USD 2.00 run cap less the run's spend,
the key's limit less its usage, and the key's limit_remaining. Within a
scenario, only the key's own limit stops spend.

Output goes to a new results/spike-2026-10/raw/<UTC time>/. SPIKE_RUN_DIR,
which must name a directory directly under raw/ (not raw/latest), adds
scenarios to an earlier run instead. raw/latest is repointed when a run
finishes.

`fixtures` writes redacted golden streams from a run (default raw/latest) to
crates/forgetest-agents/tests/fixtures/openrouter/. It needs
FORGETEST_SPIKE_KEY for the exact-key leak check; SPIKE_ALLOW_NO_KEY_CHECK=1
skips that check and says so.

Images default to localhost:5001/forgetest-codex:0.163.0 and
localhost:5001/forgetest-claude:2.1.302 (S1); override with SPIKE_CODEX_IMAGE
and SPIKE_CLAUDE_IMAGE. Tags are resolved to repository digests at start, and
every container runs by digest.
EOF
}

UID_GID="$(id -u):$(id -g)"
NET="spike-net-$$"
CAPTURE="spike-$$-capture"
# Utility containers (curl, the capture endpoint) never see the workspace.
UTIL_FLAGS=(--rm --platform linux/amd64 --read-only --cap-drop ALL
  --security-opt no-new-privileges --user "$UID_GID"
  --tmpfs /tmp:rw,nosuid,size=16m --env HOME=/tmp/home)

log() {
  printf '%s %s\n' "$(date -u +%H:%M:%SZ)" "$*" | tee -a "$RUN/run.log" >&2
}

# record SCENARIO OUTCOME NOTE
record() {
  printf '%s\t%s\t%s\n' "$1" "$2" "$3" >>"$RUN/outcomes.tsv"
  log "$1: $2 - $3"
}

tool() {
  python3 -I "$TOOLS" "$@"
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}

cleanup() {
  local ids
  ids="$(docker ps -aq --filter "name=spike-$$-" 2>/dev/null || true)"
  if [ -n "$ids" ]; then
    # shellcheck disable=SC2086
    docker rm -f $ids >/dev/null 2>&1 || true
  fi
  docker network rm "$NET" >/dev/null 2>&1 || true
}

# resolve_image TAG: print TAG's repository digest reference, or fail.
resolve_image() {
  local ref
  ref="$(docker image inspect --format '{{index .RepoDigests 0}}' "$1" 2>/dev/null)" || return 1
  case "$ref" in
    *@sha256:*) printf '%s' "$ref" ;;
    *) return 1 ;;
  esac
}

# resolve_run_dir: set RUN to a directory directly under raw/. A new run's
# directory is created without -p, so a same-second collision fails;
# SPIKE_RUN_DIR may name an existing run to add scenarios to.
resolve_run_dir() {
  local raw parent base
  mkdir -p "$RAW"
  raw="$(cd "$RAW" && pwd -P)"
  if [ -z "${SPIKE_RUN_DIR:-}" ]; then
    RUN="$raw/$(date -u +%Y%m%dT%H%M%SZ)"
    if ! mkdir "$RUN" 2>/dev/null; then
      echo "run-spike.sh: run directory raw/$(basename "$RUN") already exists; rerun in a second. Nothing was run." >&2
      return 1
    fi
    return 0
  fi
  base="$(basename "$SPIKE_RUN_DIR")"
  parent="$(cd "$(dirname "$SPIKE_RUN_DIR")" 2>/dev/null && pwd -P)" || parent=""
  case "$base" in
    "" | . | .. | latest) parent="" ;;
  esac
  if [ "$parent" != "$raw" ] || [ -L "$raw/$base" ]; then
    echo "run-spike.sh: SPIKE_RUN_DIR must name a directory directly under results/spike-2026-10/raw/, not raw/latest or a symlink. Nothing was run." >&2
    return 1
  fi
  RUN="$raw/$base"
  if [ ! -d "$RUN" ]; then
    mkdir "$RUN"
  fi
}

# forgetest's agent container flags, as DockerProcessAgent::docker_arguments
# renders them with DockerAgentConfig::default(). Only the network varies:
# keyless scenarios use none or the internal capture network.
# agent_flags WORKSPACE NETWORK LABEL
agent_flags() {
  FLAGS=(--rm --interactive --name "spike-$$-$3" --network "$2" --read-only
    --cap-drop ALL --security-opt no-new-privileges --memory 2g --cpus 2.00
    --pids-limit 256 --user "$UID_GID" --mount "type=bind,src=$1,dst=/work"
    --tmpfs /tmp:rw,nosuid,size=256m --workdir /work --env HOME=/tmp/home)
}

# fresh_workspace DIR [git]: a new copy of the canary, as a trial workspace
# (forgetest copies task files without .git).
fresh_workspace() {
  rm -rf "$1"
  mkdir -p "$(dirname "$1")"
  cp -R "$CANARY" "$1"
  if [ "${2:-}" = git ]; then
    git -C "$1" init -q
  fi
}

# grade WORKSPACE OUT: run the canary's tests without network or key. Prints
# pass, fail (cargo's exit 101) or error:<exit code>.
grade() {
  local rc=0
  agent_flags "$1" none grade
  docker run "${FLAGS[@]}" "$CODEX_IMAGE" timeout 300 cargo test --quiet \
    </dev/null >"$2" 2>&1 || rc=$?
  rm -rf "$1/target"
  case "$rc" in
    0) echo pass ;;
    101) echo fail ;;
    *) echo "error:$rc" ;;
  esac
}

# Curl inside a throwaway container. The key, when used, goes to a file on
# the container's in-memory /tmp and is passed with -H @file, never as an
# argument.
API_SCRIPT='set -eu
umask 077
: >/tmp/auth
if [ -n "${OPENROUTER_API_KEY:-}" ]; then
  printf "Authorization: Bearer %s\n" "$OPENROUTER_API_KEY" >/tmp/auth
fi
method=$1 url=$2
shift 2
if [ "$method" = POST ]; then set -- --data-binary @- "$@"; fi
exec curl -sS --max-time 120 -X "$method" -H @/tmp/auth \
  -H "content-type: application/json" -o - -w "%{stderr}%{http_code}" "$@" "$url"'

# api METHOD PATH STATUS_FILE [keyless] <BODY >RESPONSE
api() {
  if [ "${4:-}" = keyless ]; then
    docker run -i "${UTIL_FLAGS[@]}" --network bridge "$CODEX_IMAGE" \
      sh -c "$API_SCRIPT" sh "$1" "$OPENROUTER$2" 2>"$3"
  else
    OPENROUTER_API_KEY="$SPIKE_KEY" docker run -i "${UTIL_FLAGS[@]}" \
      --network bridge --env OPENROUTER_API_KEY "$CODEX_IMAGE" \
      sh -c "$API_SCRIPT" sh "$1" "$OPENROUTER$2" 2>"$3"
  fi
}

http_status() {
  tail -c 3 "$1"
}

# read_key OUT: GET /api/v1/key, keeping only the numeric budget fields.
read_key() {
  api GET /api/v1/key "$1.status" </dev/null | tool key-usage - >"$1"
}

# key_preflight: fail closed unless the key is set, accepted by OpenRouter,
# and within the key gates. Writes this invocation's own preflight file, whose
# usage is the run's baseline. Runs before any scenario.
key_preflight() {
  local verdict
  if [ -z "$SPIKE_KEY" ]; then
    log "FORGETEST_SPIKE_KEY is not set; billed scenarios need the scoped spike key. Nothing was run."
    log "Keyless scenarios only: bash results/spike-2026-10/run-spike.sh $KEYLESS_SCENARIOS"
    return 1
  fi
  case "$SPIKE_KEY" in
    *[[:space:]]*)
      log "FORGETEST_SPIKE_KEY contains whitespace; check how it was loaded. Nothing was run."
      return 1
      ;;
  esac
  read_key "$PREFLIGHT"
  if [ "$(http_status "$PREFLIGHT.status")" != 200 ]; then
    log "GET /api/v1/key returned HTTP $(http_status "$PREFLIGHT.status"); the key is not usable. Nothing was run."
    return 1
  fi
  if ! verdict="$(tool key-preflight "$PREFLIGHT" "$CAP_USD" "$KEY_LIMIT_MAX_USD" "$KEY_EXPIRY_MAX_HOURS")"; then
    log "$verdict Nothing was run."
    return 1
  fi
  BASELINE="$(tool usage-value "$PREFLIGHT")"
  log "$verdict Run baseline: usage USD $BASELINE."
}

# budget_ok SCENARIO: read billed usage and check the remaining budget covers
# the scenario's estimate plus the reserve.
budget_ok() {
  local out verdict
  if [ "$SPENT" = 1 ]; then
    # Let OpenRouter account the previous scenario's calls first.
    sleep 30
  fi
  mkdir -p "$RUN/budget"
  out="$RUN/budget/$(date -u +%Y%m%dT%H%M%SZ)-before-$1.json"
  read_key "$out"
  if [ "$(http_status "$out.status")" != 200 ]; then
    log "could not read billed usage before $1 (HTTP $(http_status "$out.status"))"
    return 1
  fi
  if verdict="$(tool budget-check "$out" "$BASELINE" "$CAP_USD" "$(estimate_usd "$1")" "$RESERVE_USD")"; then
    log "before $1: $verdict"
    return 0
  fi
  log "before $1: $verdict"
  return 1
}

capture_stop() {
  docker rm -f "$CAPTURE" >/dev/null 2>&1 || true
  docker network rm "$NET" >/dev/null 2>&1 || true
}

# capture_start: a fresh capture endpoint on an internal network. Fails if
# the server does not accept connections within about 30 attempts.
capture_start() {
  local tries=0
  capture_stop
  docker network create --internal "$NET" >/dev/null
  mkdir -p "$RUN/capture"
  rm -f "$RUN/capture/requests.jsonl"
  docker run -d "${UTIL_FLAGS[@]}" --name "$CAPTURE" --network "$NET" \
    --network-alias capture \
    --mount "type=bind,src=$TOOLS,dst=/spike/spike_tools.py,readonly" \
    --mount "type=bind,src=$RUN/capture,dst=/capture" \
    "$CODEX_IMAGE" python3 -I /spike/spike_tools.py capture-server \
    /capture/requests.jsonl 8080 >/dev/null
  # A bare TCP connect, so the readiness probe is not recorded as a request.
  until docker exec "$CAPTURE" python3 -I -c \
    'import socket; socket.create_connection(("127.0.0.1", 8080), 2).close()' \
    >/dev/null 2>&1; do
    tries=$((tries + 1))
    if [ "$tries" -ge 30 ]; then
      log "the capture endpoint did not accept connections after $tries attempts"
      return 1
    fi
    sleep 1
  done
}

# capture_take NAME: move the requests captured so far to NAME.jsonl.
capture_take() {
  local out="$RUN/capture/$1.jsonl"
  if [ -f "$RUN/capture/requests.jsonl" ]; then
    mv "$RUN/capture/requests.jsonl" "$out"
  else
    : >"$out"
  fi
  tool captured-requests "$out" >"$RUN/capture/$1.summary.jsonl"
  wc -l <"$out" | tr -d ' '
}

# capture_codex NAME WORKSPACE EXTRA_ARG...: Codex against the capture endpoint.
capture_codex() {
  local name=$1 workspace=$2
  shift 2
  agent_flags "$workspace" "$NET" "$name"
  printf '%s\n' "$SHORT_PROMPT" | OPENROUTER_API_KEY=spike-capture-dummy \
    docker run "${FLAGS[@]}" --env OPENROUTER_API_KEY "$CODEX_IMAGE" \
    timeout 180 "${CODEX_PROFILE[@]}" --model "$CODEX_MODEL" \
    -c model_provider=openrouter -c "$CODEX_CAPTURE" "$@" -C /work - \
    >"$RUN/capture/$name.stdout" 2>"$RUN/capture/$name.stderr" || true
  capture_take "$name"
}

# capture_codex_builtin NAME AUTH_ENV EXTRA_ARG...: Codex's built-in openai
# provider with a dummy value in AUTH_ENV, on the capture network.
capture_codex_builtin() {
  local name=$1 auth=$2 workspace="$RUN/capture/work"
  shift 2
  agent_flags "$workspace" "$NET" "$name"
  (
    export "$auth=spike-capture-dummy"
    printf '%s\n' "$SHORT_PROMPT" | docker run "${FLAGS[@]}" --env "$auth" "$@" \
      >"$RUN/capture/$name.stdout" 2>"$RUN/capture/$name.stderr" || true
  )
  capture_take "$name"
}

# capture_claude NAME AUTH_ENV EXTRA_ARG...: Claude Code against the capture
# endpoint, authenticated by a dummy value in AUTH_ENV.
capture_claude() {
  local name=$1 auth=$2 workspace="$RUN/capture/work"
  shift 2
  fresh_workspace "$workspace"
  agent_flags "$workspace" "$NET" "$name"
  (
    export "$auth=spike-capture-dummy"
    printf '%s\n' "$SHORT_PROMPT" | docker run "${FLAGS[@]}" --env "$auth" \
      --env ANTHROPIC_BASE_URL=http://capture:8080/api "$CLAUDE_IMAGE" \
      timeout 180 "${CLAUDE_PROFILE[@]}" --model "$CLAUDE_MODEL" "$@" \
      >"$RUN/capture/$name.stdout" 2>"$RUN/capture/$name.stderr" || true
  )
  capture_take "$name"
}

# web_tools_guard SCENARIO DIR: record a BLOCKER when any live stream in DIR
# shows a web tool in use.
web_tools_guard() {
  local found
  if ! found="$(tool web-tools "$2"/*.stream.jsonl)"; then
    record "$1" BLOCKER "web tool in use: $found"
  fi
}

# Row: slug discovery (no key). A study slug that is not listed, or a
# discovery call that is not HTTP 200, is a blocker.
scenario_slugs() {
  local dir="$RUN/slugs" slug file status missing problems=""
  api GET /api/v1/models "$dir/models.status" keyless </dev/null >"$dir/models.json"
  status="$(http_status "$dir/models.status")"
  if [ "$status" != 200 ]; then
    record slugs BLOCKER "GET /api/v1/models returned HTTP $status"
    return
  fi
  tool models "$dir/models.json" "$CLAUDE_MODEL" "$CLAUDE_CALIBRATION_MODEL" \
    "$CODEX_MODEL" "$CODEX_CALIBRATION_MODEL" "$NEGATIVE_MODEL" >"$dir/study-models.json"
  missing="$(tool unlisted "$dir/study-models.json" "$CLAUDE_MODEL" \
    "$CLAUDE_CALIBRATION_MODEL" "$CODEX_MODEL" "$CODEX_CALIBRATION_MODEL")"
  if [ -n "$missing" ]; then
    problems="$problems not listed: $missing;"
  fi
  for slug in "$CLAUDE_MODEL" "$CLAUDE_CALIBRATION_MODEL" "$CODEX_MODEL" "$CODEX_CALIBRATION_MODEL"; do
    file="$dir/$(printf '%s' "$slug" | tr '/' '_').endpoints"
    api GET "/api/v1/models/$slug/endpoints" "$file.status" keyless </dev/null >"$file.json"
    status="$(http_status "$file.status")"
    if [ "$status" != 200 ]; then
      problems="$problems $slug endpoints HTTP $status;"
      continue
    fi
    tool endpoints "$file.json" >"$file.summary.json"
  done
  if [ -n "$problems" ]; then
    record slugs BLOCKER "$problems"
  else
    record slugs recorded "$(tool listed "$dir/study-models.json")"
  fi
}

# Rows: effort on the wire, Claude auth headers and web tools, from the CLIs'
# own requests to a local capture endpoint on an internal network (no key).
scenario_capture() {
  local work="$RUN/capture/work" counts=""
  trap capture_stop EXIT
  capture_start
  fresh_workspace "$work"
  counts="$counts codex-default=$(capture_codex codex-default "$work" --skip-git-repo-check)"
  counts="$counts codex-effort-low=$(capture_codex codex-effort-low "$work" \
    --skip-git-repo-check -c 'model_reasoning_effort="low"')"
  counts="$counts codex-spike=$(capture_codex codex-spike "$work" \
    --skip-git-repo-check "${CODEX_SPIKE[@]}")"
  # Built-in openai provider: which base-URL setting and key variable it
  # honours. On the internal network a request to api.openai.com cannot leave.
  counts="$counts codex-builtin-env-base-url=$(capture_codex_builtin \
    codex-builtin-env-base-url CODEX_API_KEY \
    --env OPENAI_BASE_URL=http://capture:8080/api/v1 "$CODEX_IMAGE" \
    timeout 60 "${CODEX_PROFILE[@]}" --model "$CODEX_MODEL" \
    --skip-git-repo-check "${CODEX_SPIKE[@]}" -C /work -)"
  counts="$counts codex-builtin-openai-api-key=$(capture_codex_builtin \
    codex-builtin-openai-api-key OPENAI_API_KEY "$CODEX_IMAGE" \
    timeout 180 "${CODEX_PROFILE[@]}" --model "$CODEX_MODEL" --skip-git-repo-check \
    -c 'openai_base_url="http://capture:8080/api/v1"' "${CODEX_SPIKE[@]}" -C /work -)"
  counts="$counts codex-builtin-codex-api-key=$(capture_codex_builtin \
    codex-builtin-codex-api-key CODEX_API_KEY "$CODEX_IMAGE" \
    timeout 180 "${CODEX_PROFILE[@]}" --model "$CODEX_MODEL" --skip-git-repo-check \
    -c 'openai_base_url="http://capture:8080/api/v1"' "${CODEX_SPIKE[@]}" -C /work -)"
  counts="$counts claude-profile-as-is=$(capture_claude claude-profile-as-is ANTHROPIC_API_KEY)"
  counts="$counts claude-default=$(capture_claude claude-default ANTHROPIC_API_KEY --verbose)"
  counts="$counts claude-spike=$(capture_claude claude-spike ANTHROPIC_API_KEY "${CLAUDE_SPIKE[@]}")"
  counts="$counts claude-api-key-helper=$(capture_claude claude-api-key-helper \
    OPENROUTER_API_KEY "${CLAUDE_SPIKE[@]}" --settings "$CLAUDE_HELPER")"
  counts="$counts claude-auth-token=$(capture_claude claude-auth-token \
    ANTHROPIC_AUTH_TOKEN "${CLAUDE_SPIKE[@]}")"
  record capture recorded "requests captured:$counts"
}

# git_variant NAME WORKSPACE EXTRA_ARG...: Codex against the capture endpoint,
# classified as sent(N), refused (no request and Codex's repository-check
# message on stderr) or error (no request for any other reason).
git_variant() {
  local name=$1 count
  count="$(capture_codex "$@")"
  if [ "$count" -gt 0 ]; then
    printf '%s=sent(%s)' "$name" "$count"
  elif grep -q 'skip-git-repo-check was not specified' "$RUN/capture/$name.stderr"; then
    printf '%s=refused' "$name"
  else
    printf '%s=error' "$name"
  fi
}

# Row: Codex git repo. Codex runs against the capture endpoint, so a request
# appears only when Codex gets past its repository check.
scenario_git() {
  local dir="$RUN/git" result=""
  trap capture_stop EXIT
  capture_start
  fresh_workspace "$dir/no-git"
  fresh_workspace "$dir/with-git" git
  result="$result $(git_variant git-no-git "$dir/no-git" "${CODEX_SPIKE[@]}")"
  result="$result $(git_variant git-no-git-skip-check "$dir/no-git" \
    --skip-git-repo-check "${CODEX_SPIKE[@]}")"
  result="$result $(git_variant git-with-git "$dir/with-git" "${CODEX_SPIKE[@]}")"
  case "$result" in
    *=error*) record git error "$result" ;;
    *) record git recorded "$result" ;;
  esac
}

# sandbox_probe NAME WORKSPACE_MODE EXTRA_DOCKER_FLAG... -- CODEX_SANDBOX_ARG...
sandbox_probe() {
  local name=$1 mode=$2 dir="$RUN/sandbox" status written
  shift 2
  local extra=()
  while [ "$1" != -- ]; do
    extra+=("$1")
    shift
  done
  shift
  fresh_workspace "$dir/$name" "$mode"
  agent_flags "$dir/$name" none "$name"
  # ${extra[@]+...} keeps bash 3.2 happy when no extra flag is given.
  status=0
  docker run "${FLAGS[@]}" ${extra[@]+"${extra[@]}"} "$CODEX_IMAGE" \
    codex sandbox "$@" -C /work -- \
    sh -c 'echo probe >/work/probe.txt && cargo test --quiet' \
    </dev/null >"$dir/$name.log" 2>&1 || status=$?
  written=no
  if [ -f "$dir/$name/probe.txt" ]; then written=yes; fi
  rm -rf "$dir/$name/target"
  printf '%s exit=%s file_written=%s;' "$name" "$status" "$written"
}

# Row: Codex sandbox, without the model: `codex sandbox` with the built-in
# workspace-write profile (:workspace) under the agent flags. The diag-*
# probes vary one flag to locate the failure; they are not candidate settings.
scenario_sandbox() {
  local result=""
  result="$result $(sandbox_probe agent-flags-no-git '' -- -P :workspace)"
  result="$result $(sandbox_probe agent-flags-with-git git -- -P :workspace)"
  result="$result $(sandbox_probe diag-legacy-landlock git -- -P :workspace --enable use_legacy_landlock)"
  result="$result $(sandbox_probe diag-seccomp-unconfined-no-git '' \
    --security-opt seccomp=unconfined -- -P :workspace)"
  result="$result $(sandbox_probe diag-seccomp-unconfined-with-git git \
    --security-opt seccomp=unconfined -- -P :workspace)"
  record sandbox recorded "$result"
}

scenario_key_before() {
  read_key "$RUN/key-before/key.json"
  record key-before recorded "$(cat "$RUN/key-before/key.json")"
}

# live_call NAME MODEL: one minimal request on the endpoint the model's CLI uses.
live_call() {
  local name=$1 model=$2 dir="$RUN/live-slugs" path body
  case "$model" in
    anthropic/*)
      path=/api/v1/messages
      body="{\"model\":\"$model\",\"max_tokens\":16,\"messages\":[{\"role\":\"user\",\"content\":\"Reply with the single word ok.\"}]}"
      ;;
    *)
      path=/api/v1/responses
      body="{\"model\":\"$model\",\"max_output_tokens\":16,\"reasoning\":{\"effort\":\"low\"},\"input\":\"Reply with the single word ok.\"}"
      ;;
  esac
  printf '%s' "$body" >"$dir/$name.request.json"
  api POST "$path" "$dir/$name.status" <"$dir/$name.request.json" >"$dir/$name.json"
  tool response-id "$dir/$name.json" >"$dir/$name.id.json"
  printf '%s %s HTTP %s %s;' "$model" "$path" "$(http_status "$dir/$name.status")" \
    "$(cat "$dir/$name.id.json")"
}

# Row: slugs, live. One minimal call per study model: confirms each slug on
# the key and is the positive control for the guardrail negative test. Any
# call that is not HTTP 200 is a blocker.
scenario_live_slugs() {
  local dir="$RUN/live-slugs" result="" name status failed=""
  result="$result $(live_call claude-haiku "$CLAUDE_MODEL")"
  result="$result $(live_call claude-sonnet "$CLAUDE_CALIBRATION_MODEL")"
  result="$result $(live_call gpt-luna "$CODEX_MODEL")"
  result="$result $(live_call gpt-sol "$CODEX_CALIBRATION_MODEL")"
  for name in claude-haiku claude-sonnet gpt-luna gpt-sol; do
    status="$(http_status "$dir/$name.status")"
    if [ "$status" != 200 ]; then
      failed="$failed $name=HTTP$status"
    fi
  done
  if [ -n "$failed" ]; then
    record live-slugs BLOCKER "non-200:$failed;$result"
  else
    record live-slugs recorded "$result"
  fi
}

# claude_live NAME WORKSPACE AUTH PROMPT_FILE EXTRA_ARG...: Claude Code through
# OpenRouter under the agent flags. AUTH is api-key-helper or auth-token.
claude_live() {
  local name=$1 workspace=$2 auth=$3 prompt=$4 dir
  shift 4
  dir="$(dirname "$workspace")"
  agent_flags "$workspace" bridge "$name"
  local auth_env=ANTHROPIC_AUTH_TOKEN settings=()
  if [ "$auth" = api-key-helper ]; then
    auth_env=OPENROUTER_API_KEY
    settings=(--settings "$CLAUDE_HELPER")
  fi
  # `export` is a builtin, so the key never appears in a process argument list.
  (
    export "$auth_env=$SPIKE_KEY"
    docker run "${FLAGS[@]}" --env "$auth_env" \
      --env "ANTHROPIC_BASE_URL=$OPENROUTER/api" "$CLAUDE_IMAGE" \
      timeout 900 "${CLAUDE_PROFILE[@]}" --model "$CLAUDE_MODEL" "${CLAUDE_SPIKE[@]}" \
      ${settings[@]+"${settings[@]}"} "$@" \
      <"$prompt" >"$dir/$name.stream.jsonl" 2>"$dir/$name.stderr" || true
  )
  tool last-event "$dir/$name.stream.jsonl" result >"$dir/$name.result.json"
  tool claude-ok "$dir/$name.result.json"
}

# codex_live NAME WORKSPACE PROVIDER PROMPT_FILE EXTRA_ARG...: Codex through
# OpenRouter under the agent flags. PROVIDER is overrides, builtin-base-url or
# codex-home-config, in plan order; anything else fails. builtin-base-url
# keeps the built-in openai provider and overrides openai_base_url; Codex
# 0.163.0 ignores the OPENAI_BASE_URL variable (it would send the key to
# api.openai.com) and reads the built-in provider's key from CODEX_API_KEY,
# not OPENAI_API_KEY.
codex_live() {
  local name=$1 workspace=$2 provider=$3 prompt=$4 dir
  shift 4
  dir="$(dirname "$workspace")"
  agent_flags "$workspace" bridge "$name"
  case "$provider" in
    overrides)
      OPENROUTER_API_KEY="$SPIKE_KEY" docker run "${FLAGS[@]}" \
        --env OPENROUTER_API_KEY "$CODEX_IMAGE" \
        timeout 900 "${CODEX_PROFILE[@]}" --model "$CODEX_MODEL" \
        -c model_provider=openrouter -c "$CODEX_OPENROUTER" "${CODEX_SPIKE[@]}" "$@" -C /work - \
        <"$prompt" >"$dir/$name.stream.jsonl" 2>"$dir/$name.stderr" || true
      ;;
    builtin-base-url)
      CODEX_API_KEY="$SPIKE_KEY" docker run "${FLAGS[@]}" \
        --env CODEX_API_KEY "$CODEX_IMAGE" \
        timeout 900 "${CODEX_PROFILE[@]}" --model "$CODEX_MODEL" \
        -c "openai_base_url=\"$OPENROUTER/api/v1\"" "${CODEX_SPIKE[@]}" "$@" -C /work - \
        <"$prompt" >"$dir/$name.stream.jsonl" 2>"$dir/$name.stderr" || true
      ;;
    codex-home-config)
      # The provider lives in $CODEX_HOME/config.toml, so this path drops
      # --ignore-user-config from the profile.
      OPENROUTER_API_KEY="$SPIKE_KEY" docker run "${FLAGS[@]}" \
        --env OPENROUTER_API_KEY "$CODEX_IMAGE" sh -c '
          mkdir -p "$HOME/.codex"
          printf "%s\n" "model_provider = \"openrouter\"" "" "[model_providers.openrouter]" \
            "name = \"OpenRouter\"" "base_url = \"https://openrouter.ai/api/v1\"" \
            "env_key = \"OPENROUTER_API_KEY\"" "wire_api = \"responses\"" >"$HOME/.codex/config.toml"
          exec "$@"' sh \
        timeout 900 codex exec --json --ephemeral --sandbox workspace-write --ignore-rules \
        --model "$CODEX_MODEL" "${CODEX_SPIKE[@]}" "$@" -C /work - \
        <"$prompt" >"$dir/$name.stream.jsonl" 2>"$dir/$name.stderr" || true
      ;;
    *)
      log "unknown Codex provider path: $provider"
      return 1
      ;;
  esac
  tool last-event "$dir/$name.stream.jsonl" turn.completed >"$dir/$name.turn-completed.json"
  tool codex-ok "$dir/$name.stream.jsonl"
}

# Row: Claude auth with --bare: apiKeyHelper in --settings, then
# ANTHROPIC_AUTH_TOKEN, both with ANTHROPIC_BASE_URL set to OpenRouter.
scenario_claude_auth() {
  local dir="$RUN/claude-auth" auth result=""
  printf '%s\n' "$SHORT_PROMPT" >"$dir/prompt.txt"
  rm -f "$dir/working-path"
  for auth in api-key-helper auth-token; do
    fresh_workspace "$dir/work"
    if [ "$(claude_live "$auth" "$dir/work" "$auth" "$dir/prompt.txt")" = ok ]; then
      echo "$auth" >"$dir/working-path"
      result="$result $auth=works"
      break
    fi
    result="$result $auth=failed"
  done
  if [ -f "$dir/working-path" ]; then
    record claude-auth recorded "$result"
  else
    record claude-auth BLOCKER "$result"
  fi
  web_tools_guard claude-auth "$dir"
}

# Row: Codex provider. Fallbacks run in plan order until one works.
scenario_codex_provider() {
  local dir="$RUN/codex-provider" provider result=""
  printf '%s\n' "$SHORT_PROMPT" >"$dir/prompt.txt"
  rm -f "$dir/working-path"
  for provider in overrides builtin-base-url codex-home-config; do
    fresh_workspace "$dir/work"
    if [ "$(codex_live "$provider" "$dir/work" "$provider" "$dir/prompt.txt" \
      --skip-git-repo-check)" = ok ]; then
      echo "$provider" >"$dir/working-path"
      result="$result $provider=works"
      break
    fi
    result="$result $provider=failed"
  done
  if [ -f "$dir/working-path" ]; then
    record codex-provider recorded "$result"
  else
    record codex-provider BLOCKER "$result"
  fi
  web_tools_guard codex-provider "$dir"
}

# Row: guardrail negative. One model outside the key's model allowlist, on
# both endpoints. Runs only on request, for a key with an allowlist.
scenario_guardrail() {
  local dir="$RUN/guardrail" messages responses before after
  read_key "$dir/key-before.json"
  printf '{"model":"%s","max_tokens":16,"messages":[{"role":"user","content":"Reply with the single word ok."}]}' \
    "$NEGATIVE_MODEL" >"$dir/messages.request.json"
  printf '{"model":"%s","max_output_tokens":16,"input":"Reply with the single word ok."}' \
    "$NEGATIVE_MODEL" >"$dir/responses.request.json"
  api POST /api/v1/messages "$dir/messages.status" <"$dir/messages.request.json" >"$dir/messages.json"
  api POST /api/v1/responses "$dir/responses.status" <"$dir/responses.request.json" >"$dir/responses.json"
  sleep 20
  read_key "$dir/key-after.json"
  messages="$(http_status "$dir/messages.status")"
  responses="$(http_status "$dir/responses.status")"
  before="$(tool usage-value "$dir/key-before.json")"
  after="$(tool usage-value "$dir/key-after.json")"
  if [ "$messages" = 200 ] || [ "$responses" = 200 ]; then
    record guardrail BLOCKER "$NEGATIVE_MODEL accepted: messages HTTP $messages, responses HTTP $responses; usage $before -> $after"
  else
    record guardrail recorded "$NEGATIVE_MODEL rejected: messages HTTP $messages, responses HTTP $responses; usage $before -> $after"
  fi
}

# Row: canary, Claude. Produces the Claude golden stream.
scenario_claude_canary() {
  local dir="$RUN/claude-canary" auth ok verdict
  if [ ! -f "$RUN/claude-auth/working-path" ]; then
    record claude-canary skipped "no working Claude auth path in this run"
    return
  fi
  auth="$(cat "$RUN/claude-auth/working-path")"
  fresh_workspace "$dir/work"
  ok="$(claude_live canary "$dir/work" "$auth" "$CANARY/PROMPT.txt" --max-budget-usd 0.25)"
  diff -u --label original --label agent "$CANARY/src/lib.rs" "$dir/work/src/lib.rs" >"$dir/lib.rs.diff" || true
  verdict="$(grade "$dir/work" "$dir/grade.log")"
  record claude-canary recorded "auth=$auth result=$ok tests=$verdict $(tool claude-usage "$dir/canary.result.json")"
  web_tools_guard claude-canary "$dir"
}

# Row: Codex sandbox canary, under the agent flags with --sandbox
# workspace-write. The mode is never switched here.
scenario_codex_canary() {
  local dir="$RUN/codex-canary" provider ok verdict
  if [ ! -f "$RUN/codex-provider/working-path" ]; then
    record codex-canary skipped "no working Codex provider path in this run"
    return
  fi
  provider="$(cat "$RUN/codex-provider/working-path")"
  fresh_workspace "$dir/work"
  ok="$(codex_live canary "$dir/work" "$provider" "$CANARY/PROMPT.txt" --skip-git-repo-check)"
  diff -u --label original --label agent "$CANARY/src/lib.rs" "$dir/work/src/lib.rs" >"$dir/lib.rs.diff" || true
  verdict="$(grade "$dir/work" "$dir/grade.log")"
  record codex-canary recorded "provider=$provider turn=$ok tests=$verdict $(tool codex-commands "$dir/canary.stream.jsonl")"
  web_tools_guard codex-canary "$dir"
}

# Row: budget. Poll billed usage every 30 seconds until two consecutive
# readings agree, for up to 5 minutes, then look up the live calls'
# generations. Every reading is kept; key.json is the settled one.
scenario_key_after() {
  local dir="$RUN/key-after" n=0 id reading=1 waited=0 previous current settled=no
  read_key "$dir/reading-1.json"
  previous="$(tool usage-value "$dir/reading-1.json")"
  while [ "$waited" -lt 300 ]; do
    sleep 30
    waited=$((waited + 30))
    reading=$((reading + 1))
    read_key "$dir/reading-$reading.json"
    current="$(tool usage-value "$dir/reading-$reading.json")"
    if [ -n "$current" ] && [ "$current" = "$previous" ]; then
      settled=yes
      break
    fi
    previous="$current"
  done
  cp "$dir/reading-$reading.json" "$dir/key.json"
  if [ -d "$RUN/live-slugs" ]; then
    for id in $(cat "$RUN"/live-slugs/*.id.json | tool ids -); do
      n=$((n + 1))
      api GET "/api/v1/generation?id=$id" "$dir/generation-$n.status" </dev/null >"$dir/generation-$n.json"
      tool generation "$dir/generation-$n.json" >"$dir/generation-$n.summary.json"
    done
  fi
  if [ "$settled" = yes ]; then
    record key-after recorded "settled after $reading readings; $(tool usage-delta "$PREFLIGHT" "$dir/key.json"); $(cat "$dir/key.json"); generations=$n"
  else
    record key-after unsettled "no two consecutive readings agreed in 5 minutes ($reading readings); last: $(tool usage-delta "$PREFLIGHT" "$dir/key.json"); generations=$n"
  fi
}

write_run_record() {
  local commit dirty
  commit="$(git -C "$ROOT" rev-parse HEAD)"
  dirty="$(git -C "$ROOT" status --porcelain | wc -l | tr -d ' ')"
  {
    printf 'invocation=%s\n' "$INVOCATION"
    printf 'scenarios=%s\n' "$1"
    printf 'plan_id=%s\n' "$PLAN_ID"
    printf 'commit=%s\n' "$commit"
    printf 'uncommitted_paths=%s\n' "$dirty"
    printf 'run_spike_sha256=%s\n' "$(sha256_of "$HERE/run-spike.sh")"
    printf 'spike_tools_sha256=%s\n' "$(sha256_of "$TOOLS")"
    printf 'cap_usd=%s\n' "$CAP_USD"
    printf 'reserve_usd=%s\n' "$RESERVE_USD"
    printf 'key_limit_max_usd=%s\n' "$KEY_LIMIT_MAX_USD"
    printf 'key_expiry_max_hours=%s\n' "$KEY_EXPIRY_MAX_HOURS"
    printf 'key_gates=%s\n' "$KEY_GATES"
    printf 'host_arch=%s\n' "$(uname -m)"
    printf 'docker_server=%s\n' "$(docker version --format '{{.Server.Version}}')"
    printf 'codex_image_tag=%s\n' "$CODEX_IMAGE_TAG"
    printf 'codex_image=%s\n' "$CODEX_IMAGE"
    printf 'claude_image_tag=%s\n' "$CLAUDE_IMAGE_TAG"
    printf 'claude_image=%s\n' "$CLAUDE_IMAGE"
    printf 'key_present=%s\n' "$([ -n "$SPIKE_KEY" ] && echo yes || echo no)"
    printf 'started_at=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  } >>"$RUN/run-record.txt"
}

# Leak checks over the run directory, printing paths relative to it. The
# grep pattern is the spec's check; it is assembled from parts so that this
# file does not match it.
scan_run() {
  local leaks pattern
  leaks="$(printf '%s\n' "$SPIKE_KEY" | tool contains-secret "$RUN")"
  if [ -n "$leaks" ]; then
    record scan FAILED "key material found in: $(echo $leaks)"
  fi
  pattern="sk""-or-|/User""s/"
  leaks="$(cd "$RUN" && grep -rlE "$pattern" . || true)"
  if [ -n "$leaks" ]; then
    record scan FAILED "key prefix or host home path found in: $(echo $leaks)"
  fi
}

# make_fixtures [RUN_DIR]: redact a run's streams into temporary files, check
# them, and move them into the fixtures directory only when all succeed.
make_fixtures() {
  local run="${1:-$RAW/latest}" staging source
  if [ -z "$SPIKE_KEY" ]; then
    if [ "${SPIKE_ALLOW_NO_KEY_CHECK:-}" != 1 ]; then
      echo "run-spike.sh: fixtures needs FORGETEST_SPIKE_KEY for the exact-key leak check; set SPIKE_ALLOW_NO_KEY_CHECK=1 to skip that check. Nothing was written." >&2
      return 2
    fi
    echo "run-spike.sh: SPIKE_ALLOW_NO_KEY_CHECK=1: the exact-key leak check was skipped; the pattern and placeholder checks still run." >&2
  fi
  for source in claude-canary/canary.stream.jsonl codex-canary/canary.stream.jsonl \
    codex-provider/overrides.stream.jsonl capture/claude-spike.stdout; do
    if [ ! -f "$run/$source" ]; then
      echo "run-spike.sh: $(basename "$run")/$source is missing. Nothing was written." >&2
      return 2
    fi
  done
  if [ "$(tool last-event "$run/claude-canary/canary.stream.jsonl" result | tool claude-ok -)" != ok ]; then
    echo "run-spike.sh: the Claude canary result is missing or has is_error true. Nothing was written." >&2
    return 2
  fi
  if [ -z "$(tool last-event "$run/codex-canary/canary.stream.jsonl" turn.completed)" ]; then
    echo "run-spike.sh: the Codex canary stream has no turn.completed event. Nothing was written." >&2
    return 2
  fi
  mkdir -p "$FIXTURES"
  staging="$(mktemp -d "$FIXTURES/.staging.XXXXXX")"
  (
    trap 'rm -rf "$staging"' EXIT
    set -e
    redact_into() {
      printf '%s\n' "$SPIKE_KEY" | tool redact "$run/$1" "$staging/$2"
    }
    redact_into claude-canary/canary.stream.jsonl claude-haiku-5.5-canary.jsonl
    redact_into codex-canary/canary.stream.jsonl codex-gpt-6-luna-canary.jsonl
    redact_into codex-provider/overrides.stream.jsonl codex-gpt-6-luna-provider-ok.jsonl
    # An error result from an endpoint that answers 400 (the capture scenario).
    redact_into capture/claude-spike.stdout claude-haiku-5.5-error-result.jsonl
    tool check-fixture "$staging"/*.jsonl
    mv "$staging"/*.jsonl "$FIXTURES/"
  )
}

# validate_number NAME VALUE: refuse anything that is not a plain decimal.
validate_number() {
  if ! [[ "$2" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
    echo "run-spike.sh: $1 must be a plain decimal number. Nothing was run." >&2
    return 1
  fi
}

main() {
  local arg name status keyed=0 scenarios outcome_start
  for arg in "$@"; do
    case "$arg" in
      *[![:space:]]*) ;;
      *)
        usage >&2
        return 2
        ;;
    esac
  done
  if [ "${1:-}" = -h ] || [ "${1:-}" = --help ]; then
    usage
    return 0
  fi
  if [ "${1:-}" = fixtures ]; then
    make_fixtures "${2:-}"
    return
  fi
  if [ "$#" -eq 0 ]; then
    scenarios="$DEFAULT_SCENARIOS"
  else
    scenarios="$*"
  fi
  for name in $scenarios; do
    case " $ALL_SCENARIOS " in
      *" $name "*) ;;
      *)
        echo "unknown scenario: $name" >&2
        usage >&2
        return 2
        ;;
    esac
    case " $KEYED_SCENARIOS " in
      *" $name "*) keyed=1 ;;
    esac
    if [ "$name" = guardrail ] && [ "${SPIKE_GUARDRAIL_ALLOWLISTED:-}" != 1 ]; then
      echo "run-spike.sh: guardrail needs a key with a model allowlist; set SPIKE_GUARDRAIL_ALLOWLISTED=1 only for such a key. Nothing was run." >&2
      return 2
    fi
  done
  if [ "$keyed" = 1 ]; then
    validate_number SPIKE_KEY_LIMIT_MAX_USD "$KEY_LIMIT_MAX_USD" || return 2
    validate_number SPIKE_KEY_EXPIRY_MAX_HOURS "$KEY_EXPIRY_MAX_HOURS" || return 2
    if [ -z "$SPIKE_KEY" ]; then
      echo "run-spike.sh: FORGETEST_SPIKE_KEY is not set, and billed scenarios need the scoped spike key. Nothing was run." >&2
      echo "Keyless scenarios only: bash results/spike-2026-10/run-spike.sh $KEYLESS_SCENARIOS" >&2
      return 2
    fi
  fi

  resolve_run_dir || return 2
  if ! CODEX_IMAGE="$(resolve_image "$CODEX_IMAGE_TAG")" ||
    ! CLAUDE_IMAGE="$(resolve_image "$CLAUDE_IMAGE_TAG")"; then
    echo "run-spike.sh: $CODEX_IMAGE_TAG and $CLAUDE_IMAGE_TAG must be local images with a repository digest; build them with scripts/build-agent-images.sh. Nothing was run." >&2
    rmdir "$RUN" 2>/dev/null || true
    return 2
  fi
  INVOCATION="$(date -u +%Y%m%dT%H%M%SZ)-$$"
  PREFLIGHT="$RUN/key-preflight-$INVOCATION.json"
  BASELINE=""
  trap cleanup EXIT
  write_run_record "$scenarios"
  log "run $(basename "$RUN"), invocation $INVOCATION: $scenarios"
  if [ "$keyed" = 1 ] && ! key_preflight; then
    return 2
  fi

  touch "$RUN/outcomes.tsv"
  outcome_start="$(wc -l <"$RUN/outcomes.tsv" | tr -d ' ')"
  SPENT=0
  for name in $scenarios; do
    case " $SPENDING_SCENARIOS " in
      *" $name "*)
        if ! budget_ok "$name"; then
          record "$name" skipped "budget stop: the remaining budget cannot cover this scenario"
          continue
        fi
        SPENT=1
        ;;
    esac
    mkdir -p "$RUN/$name"
    # Not `( ... ) || status=$?`: errexit is ignored inside a subshell that
    # is the left side of ||.
    set +e
    (
      set -e
      "scenario_${name//-/_}"
    )
    status=$?
    set -e
    if [ "$status" -ne 0 ]; then
      record "$name" error "scenario exited with status $status; see run.log"
    fi
  done
  scan_run
  printf 'finished_at=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"$RUN/run-record.txt"
  case "$RUN" in
    "$(cd "$RAW" && pwd -P)"/*) ln -sfn "$(basename "$RUN")" "$RAW/latest" ;;
  esac

  # Only this invocation's outcomes count. A blocker is a recorded outcome; a
  # skip, an error, an unsettled reading or a failed scan is not.
  if tail -n +"$((outcome_start + 1))" "$RUN/outcomes.tsv" |
    awk -F '\t' '$2 != "recorded" && $2 != "BLOCKER" { bad = 1 } END { exit !bad }'; then
    log "some scenarios did not record an outcome; see outcomes.tsv"
    return 3
  fi
  log "every scenario recorded an outcome"
}

main "$@"
