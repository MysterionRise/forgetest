#!/usr/bin/env bash
# Offline checks for spike_tools.py and run-spike.sh's argument guards, from
# hand-written JSON. Needs no Docker, network or key:
#   bash results/spike-2026-10/test-spike-tools.sh
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
TOOLS="$HERE/spike_tools.py"
SCRIPT="$HERE/run-spike.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
unset FORGETEST_SPIKE_KEY SPIKE_RUN_DIR SPIKE_GUARDRAIL_ALLOWLISTED \
  SPIKE_ALLOW_NO_KEY_CHECK SPIKE_KEY_LIMIT_MAX_USD SPIKE_KEY_EXPIRY_MAX_HOURS

PASSED=0
FAILED=0
# A stand-in for the key in leak checks; not a key.
FAKE_KEY="test-key-not-real-0123456789"

tool() {
  python3 -I "$TOOLS" "$@"
}

pass() {
  PASSED=$((PASSED + 1))
  echo "ok   $1"
}

fail() {
  FAILED=$((FAILED + 1))
  echo "FAIL $1"
  if [ -s "$WORK/out" ] || [ -s "$WORK/err" ]; then
    sed 's/^/     /' "$WORK/out" "$WORK/err"
  fi
}

# check NAME EXPECTED_EXIT COMMAND...: run COMMAND, keeping its stdout and
# stderr in $WORK/out and $WORK/err, and compare its exit status.
check() {
  local name=$1 expected=$2 status=0
  shift 2
  "$@" >"$WORK/out" 2>"$WORK/err" </dev/null || status=$?
  if [ "$status" = "$expected" ]; then
    pass "$name"
  else
    fail "$name (exit $status, expected $expected)"
  fi
}

# check_stdin NAME EXPECTED_EXIT INPUT COMMAND...: as check, with INPUT on stdin.
check_stdin() {
  local name=$1 expected=$2 input=$3 status=0
  shift 3
  printf '%s\n' "$input" | "$@" >"$WORK/out" 2>"$WORK/err" || status=$?
  if [ "$status" = "$expected" ]; then
    pass "$name"
  else
    fail "$name (exit $status, expected $expected)"
  fi
}

# expect NAME FILE PATTERN [absent]: FILE matches PATTERN (a fixed string),
# or does not when the fourth argument is "absent".
expect() {
  local found=no
  if grep -qiF -- "$3" "$2" 2>/dev/null; then found=yes; fi
  if { [ "${4:-}" = absent ] && [ "$found" = no ]; } ||
    { [ "${4:-}" != absent ] && [ "$found" = yes ]; }; then
    pass "$1"
  else
    fail "$1"
  fi
}

hours_from_now() {
  python3 -I -c 'import sys
from datetime import datetime, timedelta, timezone
when = datetime.now(timezone.utc) + timedelta(hours=float(sys.argv[1]))
print(when.strftime("%Y-%m-%dT%H:%M:%S.000Z"))' "$1"
}

cd "$WORK"

echo "# budget-check"
printf '{"usage": 1.85, "limit": 5, "limit_remaining": 3.15}' >cap-binds.json
check "cap binds while a USD 5 key limit would still allow the spend" 1 \
  tool budget-check cap-binds.json 0 2.00 0.10 0.10
printf '{"usage": 1.80, "limit": 5, "limit_remaining": 3.20}' >just-enough.json
check "remaining exactly covers estimate plus reserve" 0 \
  tool budget-check just-enough.json 0 2.00 0.10 0.10
printf '{"usage": 4.80, "limit": 10, "limit_remaining": 5.20}' >baseline-ok.json
check "baseline-relative: USD 1.80 of the run's USD 2.00 spent" 0 \
  tool budget-check baseline-ok.json 3.00 2.00 0.10 0.10
printf '{"usage": 4.85, "limit": 10, "limit_remaining": 5.15}' >baseline-stop.json
check "baseline-relative: USD 1.85 of the run's USD 2.00 spent" 1 \
  tool budget-check baseline-stop.json 3.00 2.00 0.10 0.10
printf '{"usage": 0.50, "limit": 0.65, "limit_remaining": 0.15}' >key-limit-binds.json
check "the key limit binds before the run cap" 1 \
  tool budget-check key-limit-binds.json 0 2.00 0.10 0.10
printf '{"usage": 0.10, "limit": 5, "limit_remaining": 0.15}' >remaining-binds.json
check "limit_remaining binds" 1 tool budget-check remaining-binds.json 0 2.00 0.10 0.10
printf '{"data": {"usage": 0.10, "limit": 2, "limit_remaining": 1.90}}' >raw-form.json
check "accepts a raw GET /api/v1/key response" 0 tool budget-check raw-form.json 0 2.00 0.40 0.10
printf '{"data": null}' >data-null.json
check "data: null has no usage and stops" 1 tool budget-check data-null.json 0 2.00 0.02 0.10

echo "# key-preflight"
in24="$(hours_from_now 24)"
in72="$(hours_from_now 72)"
printf '{"usage": 0, "limit": 5, "expires_at": "%s"}' "$in24" >limit5.json
check "limit USD 5 refused under the default gate" 1 tool key-preflight limit5.json 2.00 2.00 48.5
expect "refusal names the allowed limit" out "above the allowed USD 2.00"
check "limit USD 5 accepted with the gate overridden to 5.00" 0 \
  tool key-preflight limit5.json 2.00 5.00 48.5
printf '{"usage": 0, "limit": 2}' >no-expiry.json
check "missing expiry refused" 1 tool key-preflight no-expiry.json 2.00 2.00 48.5
printf '{"usage": 0, "limit": 2, "expires_at": "soon"}' >bad-expiry.json
check "unparseable expiry refused" 1 tool key-preflight bad-expiry.json 2.00 2.00 48.5
printf '{"usage": 0, "limit": 2, "expires_at": "%s"}' "$in72" >expiry72.json
check "expiry beyond the maximum refused" 1 tool key-preflight expiry72.json 2.00 2.00 48.5
printf '{"usage": 0, "limit": 2, "expires_at": "%s"}' "$in24" >good.json
check "limit USD 2 and a 24-hour expiry accepted" 0 tool key-preflight good.json 2.00 2.00 48.5
printf '{"usage": 0, "limit": null, "expires_at": "%s"}' "$in24" >no-limit.json
check "missing limit refused" 1 tool key-preflight no-limit.json 2.00 2.00 48.5
check "data: null refused" 1 tool key-preflight data-null.json 2.00 2.00 48.5
check "key-usage tolerates data: null" 0 tool key-usage data-null.json
expect "key-usage prints an empty object for data: null" out "{}"

echo "# redact and check-fixture"
upper_uuid="3F2504E0-4F89-11D3-9A0C-0305E82C3301"
hex64="$(printf 'ab%.0s' $(seq 1 32))"
mac_home="/Us""ers/someone/project"
cat >stream.jsonl <<EOF
{"type": "system", "session_id": "$upper_uuid", "cwd": "/home/runner/work", "note": "$mac_home"}
{"type": "assistant", "message": {"id": "msg_011CgGQaU5fnX48Hx2Ycjero", "content": [{"type": "tool_use", "id": "toolu_01AbCdEfGhIjKlMnOpQrStUv", "name": "Bash"}]}, "request_id": "req_011CgGQaU5fnX48Hx2Ycjabc"}
{"type": "result", "generation": "gen-1729559012-AbCdEfGhIjKlMnOp", "device_id": "$hex64", "home": "/tmp/home/.codex", "by_uuid": {"$upper_uuid": 1}}
EOF
check "check-fixture rejects the unredacted stream" 1 tool check-fixture stream.jsonl
check_stdin "redact succeeds with a key on stdin" 0 "$FAKE_KEY" \
  tool redact stream.jsonl redacted.jsonl
check "check-fixture accepts the redacted stream" 0 tool check-fixture redacted.jsonl
expect "uppercase UUID replaced" redacted.jsonl "$upper_uuid" absent
expect "UUID placeholder present" redacted.jsonl "00000000-0000-4000-8000-000000000001"
expect "msg_ ID replaced" redacted.jsonl "msg_011CgGQaU5fnX48Hx2Ycjero" absent
expect "toolu_ ID replaced" redacted.jsonl "toolu_01AbCdEfGhIjKlMnOpQrStUv" absent
expect "req_ ID replaced" redacted.jsonl "req_011CgGQaU5fnX48Hx2Ycjabc" absent
expect "gen- ID replaced" redacted.jsonl "gen-1729559012-AbCdEfGhIjKlMnOp" absent
expect "64-hex device_id replaced" redacted.jsonl "$hex64" absent
expect "/home/ path replaced" redacted.jsonl "/home/runner" absent
expect "macOS home path replaced" redacted.jsonl "$mac_home" absent
expect "container /tmp/home path kept" redacted.jsonl "/tmp/home/.codex"

printf '{"type": "assistant", "text": "write to dev@example.com"}\n' >email.jsonl
check_stdin "an email address is refused" 1 "$FAKE_KEY" tool redact email.jsonl email-out.jsonl
expect "the refused output is not written" email-out.jsonl "dev@" absent
check "check-fixture rejects an email address" 1 tool check-fixture email.jsonl
printf '{"type": "assistant", "text": "the key is %s"}\n' "$FAKE_KEY" >leak.jsonl
check_stdin "the exact key on stdin is refused" 1 "$FAKE_KEY" tool redact leak.jsonl leak-out.jsonl
check_stdin "no key on stdin is refused by default" 1 "" tool redact stream.jsonl nokey.jsonl
check_stdin "no key on stdin is allowed with SPIKE_ALLOW_NO_KEY_CHECK=1" 0 "" \
  env SPIKE_ALLOW_NO_KEY_CHECK=1 python3 -I "$TOOLS" redact stream.jsonl nokey.jsonl
expect "the skipped exact-key check is reported" err "exact-key check skipped"

echo "# contains-secret"
mkdir -p tree/sub
printf 'nothing here\n' >tree/clean.txt
printf 'value=%s\n' "$FAKE_KEY" >tree/sub/leak.txt
check_stdin "contains-secret finds the key" 0 "$FAKE_KEY" tool contains-secret tree
expect "contains-secret prints the relative path" out "sub/leak.txt"
expect "contains-secret skips clean files" out "clean.txt" absent
check_stdin "contains-secret with no key prints nothing" 0 "" tool contains-secret tree
if [ -s "$WORK/out" ]; then fail "contains-secret with no key printed output"; else pass "contains-secret with no key printed nothing"; fi

echo "# web-tools"
printf '%s\n' '{"type": "assistant", "message": {"content": [{"type": "tool_use", "name": "WebSearch", "input": {}}]}}' >claude-web.stream.jsonl
check "Claude WebSearch tool_use detected" 1 tool web-tools claude-web.stream.jsonl
printf '%s\n' '{"type": "assistant", "message": {"content": [{"type": "tool_use", "name": "WebFetch", "input": {}}]}}' >claude-fetch.stream.jsonl
check "Claude WebFetch tool_use detected" 1 tool web-tools claude-fetch.stream.jsonl
printf '%s\n' '{"type": "item.completed", "item": {"id": "item_3", "type": "web_search", "query": "x"}}' >codex-web.stream.jsonl
check "Codex web_search item detected" 1 tool web-tools codex-web.stream.jsonl
printf '%s\n' '{"type": "result", "usage": {"server_tool_use": {"web_search_requests": 1, "web_fetch_requests": 0}}}' >counter.stream.jsonl
check "non-zero web request counter detected" 1 tool web-tools counter.stream.jsonl
printf '%s\n' '{"type": "system", "subtype": "init", "tools": ["Bash", "WebSearch"]}' \
  '{"type": "result", "usage": {"server_tool_use": {"web_search_requests": 0, "web_fetch_requests": 0}}}' \
  '{"type": "item.completed", "item": {"type": "command_execution", "command": "cargo test"}}' >clean.stream.jsonl
check "a clean stream passes (tool lists and zero counters are not use)" 0 \
  tool web-tools clean.stream.jsonl missing.stream.jsonl

echo "# run-spike.sh argument guards"
check "an empty argument prints usage and exits 2" 2 bash "$SCRIPT" ""
expect "usage printed for an empty argument" err "usage: results/spike-2026-10/run-spike.sh"
check "a blank argument exits 2" 2 bash "$SCRIPT" "   "
check "guardrail without SPIKE_GUARDRAIL_ALLOWLISTED exits 2" 2 bash "$SCRIPT" guardrail
expect "guardrail refusal names the variable" err "SPIKE_GUARDRAIL_ALLOWLISTED=1"
check "SPIKE_RUN_DIR outside raw/ exits 2" 2 env SPIKE_RUN_DIR="$WORK/outside" bash "$SCRIPT" slugs
expect "SPIKE_RUN_DIR refusal names the variable" err "SPIKE_RUN_DIR must name a directory"
if [ -e "$WORK/outside" ]; then fail "SPIKE_RUN_DIR outside raw/ was created"; else pass "SPIKE_RUN_DIR outside raw/ was not created"; fi
check "SPIKE_RUN_DIR=raw/latest exits 2" 2 env SPIKE_RUN_DIR="$HERE/raw/latest" bash "$SCRIPT" slugs
check "a keyed scenario without FORGETEST_SPIKE_KEY exits 2" 2 bash "$SCRIPT" key-before
expect "missing-key refusal names the variable" err "FORGETEST_SPIKE_KEY is not set"
check "a non-numeric key gate override exits 2" 2 \
  env FORGETEST_SPIKE_KEY=not-a-real-key SPIKE_KEY_LIMIT_MAX_USD=five bash "$SCRIPT" key-before
check "xtrace is refused" 2 bash -x "$SCRIPT" slugs
check "fixtures without a key or SPIKE_ALLOW_NO_KEY_CHECK exits 2" 2 \
  bash "$SCRIPT" fixtures "$WORK/no-such-run"
expect "fixtures refusal names the override" err "SPIKE_ALLOW_NO_KEY_CHECK=1"
check "an unknown scenario exits 2" 2 bash "$SCRIPT" no-such-scenario

echo
echo "$PASSED passed, $FAILED failed"
[ "$FAILED" -eq 0 ]
