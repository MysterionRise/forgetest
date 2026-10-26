# S2 live spike findings

**Fact-finding, not evidence.** Nothing in this directory is a performance,
cost or safety claim, and it must not be published or cited as evidence
without a separate Don decision (spec `spec-live-spike`, Ask First).
Classification: `repository-visible`; not part of the public mdBook.

Status: **complete**. Every I/O row has a recorded outcome. The live run
(`raw/20261022T012610Z`, 2026-10-22 01:26–01:30 UTC) cost USD 0.005034875, as
billed by OpenRouter. The redacted golden fixtures for S4 are in
`crates/forgetest-agents/tests/fixtures/openrouter/`: a successful Claude
run, a Claude error result, the failed Codex canary and a successful Codex
turn. Their README records provenance and redaction.

## Answers at a glance

| Question | Answer |
|---|---|
| Claude auth under `--bare` through OpenRouter | **Works** with `apiKeyHelper` in `--settings` (first path tried) |
| Codex `-c` provider override with `--ignore-user-config` | **Works**; OpenRouter accepts Codex's "responses lite" request |
| Guardrails | **Not testable with this key**: the Don's key has no allowlists (Don decision 2026-10-22). Moves to S5 |
| Effort on the wire | Both CLIs send the configured effort. Upstream level unverified for both; Codex reported reasoning tokens at `medium`, Claude reported none |
| Codex `workspace-write` in our container | **No-go**: every command and file write fails, because bubblewrap cannot create a namespace |
| Codex without `.git` | Needs `--skip-git-repo-check` |
| Exact slugs and response IDs | Slugs confirmed. Direct Messages calls return `msg_` IDs with no generation record; Responses calls return `gen-` IDs |
| Per-generation audit of CLI runs | **Claude: yes.** Its stream carries a `gen-` `request_id` per API call, which resolves to a full generation record. **Codex: no.** Its stream carries no response or generation ID; audit per key |
| Built-in profiles as of the baseline | **Both fail before any API call**: Claude needs `--verbose`, Codex needs `--skip-git-repo-check`. No trial can run until S3 fixes them |
| Billed vs stream cost | Billed USD 0.005034875; pricing-table cost from the streams is the same to the last digit |

## Run record

| Field | Value |
|---|---|
| Plan ID | `spec-live-spike` |
| Commit | `249947d0869e1cd282e70ea341a2a56fd404a603`, with the uncommitted spike files in `results/spike-2026-10/` |
| Per-run cap | USD 2.00. The script checks it between scenarios only: a spending scenario starts when the remaining budget covers its worst-case estimate (USD 0.02 to 0.40) plus a USD 0.10 reserve. Within a scenario only the key limit stops spend, and the readings lag billing (see Row 8) |
| Key conditions | Don decision 2026-10-22 (`docs/governance/decisions/2026-10-22-don-s2-key.md`): key limit USD 5, expiry 2026-11-21 replaced by revocation right after the run, no model or provider allowlist. The script ran with its key gates relaxed to match (limit up to USD 5, expiry up to 744 hours); after review they default to USD 2 and 48 hours again. The key was supplied in the chat, so it is in the session transcript; it is not in Git, logs, BMAD output or any command line |
| Host | arm64 macOS, Docker 29.8.2; the amd64 agent images run under emulation |
| Codex image | `localhost:5001/forgetest-codex@sha256:ae20a621e675b32319acdbcc932d29ff7f86fb6fb8ce4e6ab8f1c172033bb7b1` (Codex CLI 0.163.0, S1 recipe) |
| Claude image | `localhost:5001/forgetest-claude@sha256:8e44ac59d71902c76e467d9a857309ea70777097e9f3eea266c3848363281beb` (Claude Code 2.1.302, S1 recipe) |
| Live run | `raw/20261022T012610Z`, 2026-10-22 01:26:10–01:30:20 UTC: `slugs capture git sandbox key-before live-slugs claude-auth codex-provider claude-canary codex-canary key-after` (guardrail omitted, see Row 4). Every scenario recorded an outcome; the leak scan was clean. The scripts were changed after the run by the S2 review (redaction, guards, settlement polling); the as-run versions were not hashed |
| Earlier keyless run | `raw/20261019T105942Z`, 2026-10-19 10:59–11:01 UTC: same keyless results |
| Billed usage (`GET /api/v1/key`) | USD 0.000000 before; USD 0.004268 at 01:30:20 (the script's `key-after`, 30 s after the last call); USD 0.005034875 at 01:31:25 and unchanged at 01:32:16 (settled; both read manually after the script ended) |
| Key revoked | No. The key was not revoked after the run as the 2026-10-22 decision required; on 2026-10-26 the Don chose to keep it active (`docs/governance/decisions/2026-10-26-don-d5-codex-sandbox-and-key.md`). It is not used for S5 published runs |

An earlier plumbing dry run used an invalid placeholder key, so it could not be
billed.

## How to run

    read -rs FORGETEST_SPIKE_KEY && export FORGETEST_SPIKE_KEY
    bash results/spike-2026-10/run-spike.sh            # default scenarios, in plan order
    bash results/spike-2026-10/run-spike.sh fixtures   # redacted streams for S4
    unset FORGETEST_SPIKE_KEY

**A rerun is a new paid run.** Under ruling R3 it needs its own approved plan
and cap, whichever key is used. By default the script accepts only a key with
a limit of at most USD 2 and an expiry within 48 hours; overriding those gates
needs a Don decision record. The `guardrail` scenario is not in the default
list and runs only when explicitly requested for a key that has an allowlist.

Each scenario runs in a throwaway container. Raw output goes to
`raw/<UTC time>/`, which is gitignored; `raw/latest` points at the last run,
and `outcomes.tsv` there holds one line per scenario. The canary scenarios use
forgetest's agent container flags exactly (`DockerProcessAgent::docker_arguments`
with `DockerAgentConfig::default()`). Every call disables web tools: Codex
`-c web_search="disabled"`, Claude `--disallowedTools WebSearch,WebFetch`.

The `capture`, `git` and `sandbox` scenarios need no key. `capture` points each
CLI at a local endpoint on an internal Docker network that records the request
(credential headers are dropped, only their names are kept) and answers 400.
It shows exactly what each CLI would send to OpenRouter.

## Row 1: slug discovery

`GET https://openrouter.ai/api/v1/models`, no key, HTTP 200. All four study
slugs are listed. Prices are USD per million tokens.

| Slug | Input | Output | Cache read | Cache write (5 min) | Cache write (1 h) | Context |
|---|---:|---:|---:|---:|---:|---:|
| `anthropic/claude-haiku-5.5` | 0.10 | 0.50 | 0.01 | 0.125 | 0.20 | 1,000,000 |
| `anthropic/claude-sonnet-5.5` | 2.00 | 10.00 | 0.10 | 2.50 | 4.00 | 1,000,000 |
| `openai/gpt-6-luna` | 0.10 | 0.50 | 0.01 | 0.125 | none | 1,050,000 |
| `openai/gpt-6.1-sol` | 2.00 | 10.00 | 0.10 | 2.50 | none | 1,050,000 |

Haiku 5.5 has a long-prompt override above 100,000 prompt tokens (input 0.50,
output 2.50); the OpenAI models have one above 272,000. The negative-test model
`openai/gpt-5.6-luna` is also listed (0.20 / 1.20).

`GET /api/v1/models/{slug}/endpoints` shows that each model is served by
several providers, not only the first-party one:

- Anthropic models: `anthropic`, `claude-on-aws`, `amazon-bedrock` (three
  regions), `google-vertex` (three regions) and `azure` (two regions).
- OpenAI models: `openai`, `openai/flex`, `openai/fast`, `azure` (three
  regions) and `amazon-bedrock/us-east-1`; Sol also has `openai/ultrafast`.

Regional endpoints cost 10 percent more. Within the first-party OpenAI provider
the tags differ in price: for GPT-6.1 Sol, `openai/flex` is 0.5×,
`openai/fast` 2× and `openai/ultrafast` 6× the base price.

Live (`live-slugs`), one minimal call per slug, all HTTP 200:

| Slug | Endpoint | ID format | Served by | Billed cost |
|---|---|---|---|---:|
| `anthropic/claude-haiku-5.5` | `/api/v1/messages` | `msg_…` | Claude Platform on AWS | 0.0000038 |
| `anthropic/claude-sonnet-5.5` | `/api/v1/messages` | `msg_…` | Claude Platform on AWS | 0.0000760 |
| `openai/gpt-6-luna` | `/api/v1/responses` | `gen-…` | OpenAI (generation record) | 0.0000038 |
| `openai/gpt-6.1-sol` | `/api/v1/responses` | `gen-…` | OpenAI (generation record) | 0.0000760 |

`GET /api/v1/generation?id=` returns an empty record for the `msg_` IDs and a
full record for the `gen-` IDs, including `provider_name` and the dated model
(`openai/gpt-6-luna-20260922`, `openai/gpt-6.1-sol-20260929`). With no
provider allowlist, both Anthropic calls were routed to Claude Platform on
AWS, not to Anthropic directly.

What matters for CLI runs is what each stream carries:

- **Claude Code** puts a `request_id` of the form `gen-…` on every assistant
  event, one per API call, plus the serving `provider`. Looked up on
  2026-10-26 (a free read), the first one resolved to a full generation
  record: provider Claude Platform on AWS, model
  `anthropic/claude-haiku-5.5-20261007`, tokens and cost. Claude runs can
  therefore be audited per generation.
- **Codex** streams carry no response, generation or provider ID, so Codex
  runs can only be audited per key.

## Row 2: Claude auth with `--bare`

Keyless capture, Claude Code 2.1.302 with `ANTHROPIC_BASE_URL` set to the
capture endpoint. Every path reaches `POST /api/v1/messages?beta=true`:

| Path | Credential headers sent |
|---|---|
| `--bare`, `--settings '{"apiKeyHelper":"printenv OPENROUTER_API_KEY"}'` | `authorization` and `x-api-key` |
| `--bare`, `ANTHROPIC_AUTH_TOKEN` | `authorization` |
| `--bare`, `ANTHROPIC_API_KEY` (today's profile) | `x-api-key` |

With `ANTHROPIC_AUTH_TOKEN`, Claude Code sends the header under `--bare`,
although `claude --help` says `--bare` auth is "strictly ANTHROPIC_API_KEY or
apiKeyHelper via --settings". This was seen only against the capture
endpoint; whether that path authenticates was not tried live.

With the slug as the model name, Claude Code prints
`[claude-code:unrecognized_model] {"model":"anthropic/claude-haiku-5.5",...}`
to stderr and continues.

Plumbing dry run with an invalid placeholder key against OpenRouter: both
paths got HTTP 401, and Claude Code retried 10 times with backoff before
giving up (about 3 to 4 minutes per path).

Live: the first path tried, `apiKeyHelper` via `--settings`, **works** under
`--bare` with `ANTHROPIC_BASE_URL=https://openrouter.ai/api`. The `result`
event has `is_error: false` and `modelUsage` keyed by the slug
`anthropic/claude-haiku-5.5`, with `costBasis: "unknown"` and
`provider: "firstParty"` (the call was in fact served by Claude Platform on
AWS). `ANTHROPIC_AUTH_TOKEN` was not needed and was not tried live.

The `system`/`init` event shows what `--bare` still loads: tools `Bash`,
`Edit` and `Read`; two built-in plugins (`cc-plugin-agents-md`,
`cc-plugin-plugin-authoring`); 16 skills; 5 subagent types; and 44 slash
commands. The `agents-md` plugin may inject `AGENTS.md` context, which Codex
skips with `--ignore-rules`. That matters for arm symmetry.

## Row 3: Codex provider

Keyless capture, Codex 0.163.0 with today's profile arguments (including
`--ignore-user-config`) plus
`-c model_provider=openrouter -c 'model_providers.openrouter={name="OpenRouter",base_url=...,env_key="OPENROUTER_API_KEY",wire_api="responses"}'`:
Codex sends one `POST /api/v1/responses` to the override's base URL with an
`authorization` header taken from `OPENROUTER_API_KEY`. The override works
alongside `--ignore-user-config` as far as the client is concerned.

The request uses Codex's "responses lite" shape: header
`x-openai-internal-codex-responses-lite: true`, no top-level `tools`, and the
tools in an `input` item of type `additional_tools`. Whether OpenRouter's
Responses endpoint accepted this shape was the main open risk for this row;
the live result below resolves it.

Codex's built-in `openai` provider, checked for the fallback order:

- The `OPENAI_BASE_URL` environment variable is **ignored**: Codex still
  connected to `wss://api.openai.com/v1/responses`. A fallback built on it
  would send the OpenRouter key to OpenAI, so the script does not use it.
- `-c openai_base_url="..."` is honoured. Codex first tries a WebSocket
  upgrade (7 `GET` requests), then falls back to one HTTPS `POST`.
- The built-in provider authenticates from `CODEX_API_KEY`. With only
  `OPENAI_API_KEY` set, which is what today's profile forwards, Codex sent
  **no** credential header.

The script's plan order is therefore: (1) the `-c` provider override with
`--ignore-user-config`; (2) the built-in provider with `-c openai_base_url`
and `CODEX_API_KEY`; (3) the provider in `$CODEX_HOME/config.toml`, without
`--ignore-user-config`.

Live: the first path, the `-c` provider override with
`--ignore-user-config`, **works**. OpenRouter accepted the "responses lite"
request, and the turn completed (`turn.completed` with
`cache_write_input_tokens: 10165`, `input_tokens: 10168`).

## Row 4: guardrail negative

**Not testable with this key.** The Don stated that the spike key has no
model or provider allowlist (decision 2026-10-22), so a call to
`openai/gpt-5.6-luna` would only show that an unrestricted key accepts any
model. The scenario was omitted and no out-of-allowlist call was made.
Guardrail verification moves to the S5 per-arm keys, which must carry model
and provider allowlists; the `live-slugs` calls are the positive control to
reuse there.

## Row 5: effort on the wire

Client side, verified by keyless capture:

| CLI and setting | Field on the wire |
|---|---|
| Claude, no `--effort` | `output_config: {"effort": "high"}`, `thinking: {"type": "adaptive"}` |
| Claude, `--effort medium` | `output_config: {"effort": "medium"}` |
| Codex, no effort setting | `reasoning: {"effort": "medium", "context": "all_turns"}` (model default) |
| Codex, `-c model_reasoning_effort="low"` | `reasoning: {"effort": "low", ...}` |
| Codex, `-c model_reasoning_effort="medium"` | `reasoning: {"effort": "medium", ...}` |

Claude also sends the beta header `effort-2025-11-24`. Its first request asks
for `thinking.display: "updates"`. Against the capture endpoint, which answers
400 to every request, it retried once without that field; whether OpenRouter
rejects the field is not known.

Live: Codex at `medium` reported `reasoning_output_tokens: 889` in the canary
turn, so reasoning ran upstream, but no response echoes the level, so the
upstream level stays **unverified**. Claude at `medium` reported `thinking_tokens: 0` on the
canary, which adaptive thinking allows for a simple task, so the upstream
level for Claude is **unverified**. The `live-slugs` calls used `low` with a
one-word answer and reported no reasoning tokens.

## Row 6: Codex sandbox canary

Keyless probe: `codex sandbox -P :workspace` (the built-in workspace-write
profile) running `echo probe >/work/probe.txt && cargo test`, under the agent
container flags with no network.

| Probe | Result |
|---|---|
| Agent flags, workspace without `.git` | exit 1, nothing written: `bwrap: No permissions to create a new namespace, likely because the kernel does not allow non-privileged user namespaces.` |
| Agent flags, workspace with `.git` | same |
| Diagnostic: legacy Landlock (`--enable use_legacy_landlock`) | exit 101: `filesystem-restricted execution requires bubblewrap to isolate app-server sockets` |
| Diagnostic: `--security-opt seccomp=unconfined` | exit 101: `error applying Linux sandbox restrictions: Sandbox(SeccompInstall(Seccomp(Os { code: 22, ... "Invalid argument" })))` |

Bubblewrap cannot create its user namespace under Docker's default seccomp
profile, which blocks unprivileged namespace creation on every architecture.
Every probe ran on this arm64 host under emulation; none ran on a native
amd64 host, and no scripted probe varied `--cap-drop`. The diagnostics are not
candidate settings; they locate the failure. The `seccomp=unconfined` result is inconclusive for a native amd64
host: Codex's own seccomp filter fails to install, probably because the amd64
binary runs under emulation on an arm64 kernel.

The same flags run the canary's `cargo test` without the Codex sandbox: the
read-only root, `/usr/local/cargo` and the bind-mounted `/work` are enough for
a dependency-free crate.

Live end-to-end canary through `codex exec --sandbox workspace-write` under
the agent flags: the turn completed, but **every command and both file writes
failed**. Each command returned exit 1 with the same bubblewrap error, and
the patch failed with `Failed to write file /work/src/lib.rs`. The workspace
was unchanged and the canary tests still failed. For comparison, Claude Code
with `bypassPermissions` under the same flags fixed the bug in 4 turns, and
the tests passed. The mode was never switched by the script.

## Row 7: Codex git repo

Keyless, Codex against the capture endpoint:

| Workspace | `--skip-git-repo-check` | Result |
|---|---|---|
| no `.git` | no | refused: exit 1 before any request, `Not inside a trusted directory and --skip-git-repo-check was not specified.` |
| no `.git` | yes | request sent |
| `git init`, no trust configuration | no | request sent |

forgetest copies task workspaces without `.git`
(`repository_engine.rs` `copy_tree`), so today's Codex profile stops before any
API call in every trial. `--skip-git-repo-check` is required.

## Row 8: budget

Billed usage stayed far below every per-scenario estimate. The readings
lagged billing, though: the last reading before `codex-canary` was USD 0.0004,
when about USD 0.0016 had already been spent by the settled accounting, and
the script's `key-after` reading (USD 0.004268, 30 s after the last call) was
below the settled USD 0.005034875 reached about a minute later. After review,
`key-after` polls until two readings agree.

Stream-priced cost, using the Row 1 prices and the token counts in each
stream, for every spending scenario:

| Scenario | Tokens (uncached in / cache read / cache write / out) | Stream-priced USD |
|---|---|---:|
| live-slugs (4 calls) | from each response's `usage` | 0.000159600 |
| claude-auth (Haiku) | 4 / 0 / 1,602 / 4 | 0.000202650 |
| codex-provider (Luna) | 3 / 0 / 10,165 / 5 | 0.001273425 |
| claude-canary (Haiku) | 8 / 5,007 / 901 / 393 | 0.000359995 |
| codex-canary (Luna) | 21 / 69,173 / 13,639 / 1,281 | 0.003039205 |
| **Total** | | **0.005034875** |

This equals the billed usage exactly. Two mapping facts follow from it:
Codex's `input_tokens` **includes** both `cached_input_tokens` and
`cache_write_input_tokens` (82,833 = 21 + 69,173 + 13,639), and Codex's
reasoning tokens are part of `output_tokens`. Claude Code's own
`total_cost_usd` was 0.008106 for claude-auth and 0.0133984 for the canary,
about 40 and 37 times the billed amounts, because it does not recognise the
OpenRouter slug.

## Other findings

- **OpenRouter reports a per-call `usage.cost`** on both endpoints, but
  neither CLI surfaces it in its stream, so billed cost per run is only
  available per key (`GET /api/v1/key`) or, for Responses calls, per
  generation.
- **Claude Code assumes a 200,000-token context window** for the unrecognised
  slug, although the model offers 1,000,000. This may change when it compacts
  context.
- **Codex's PATH-alias warning appears in real runs**: "Refusing to create
  helper binaries under temporary dir /tmp", because `HOME` is
  `/tmp/home` (deferred item from S1).
- **The agent can read the key.** The key is in the agent container's
  environment (the Claude `apiKeyHelper` is `printenv`), the agent has a
  shell, and the container is on Docker's `bridge` network, so a command
  could read the key and send it anywhere. The same network lets either
  arm's shell commands reach the internet (for example `curl` or
  documentation sites), whatever the CLI's web-tool flags say.
- **Claude repeats `usage` per content block.** The canary stream's assistant
  events share message IDs across content blocks with identical `usage`.
  Summing them gives 1,128 cache-write tokens against 901 in `result`. Only
  the `result` event (or one event per message ID) should be counted.
- **Today's Claude profile cannot run.** Claude Code 2.1.302 rejects
  `--print --output-format stream-json` without `--verbose`:
  `Error: When using --print, --output-format=stream-json requires --verbose`.
  It exits before any request.
- **Claude error results look like zero usage.** When the API call fails, the
  final `result` event has `is_error: true`, `api_error_status`,
  `subtype: "success"`, `total_cost_usd: 0`, `modelUsage: {}` and zero token
  counts. `claude_result_usage` would read that as reported tokens with a
  vendor cost of USD 0.00.
- Under `--bare`, Claude's tool set is `Bash`, `Edit`, `Read`, with or
  without `--disallowedTools WebSearch,WebFetch`.
- Codex offered no web-search tool through the custom provider, with or
  without `-c web_search="disabled"`.
- Identifiers sent upstream: Claude's `metadata.user_id` carries `device_id`,
  `session_id` and an empty `account_uuid`. Codex's `x-codex-turn-metadata`
  header carries `installation_id` and session IDs. With `HOME` on the
  container's tmpfs, these are new for every container.
- When it cannot connect, Codex retries without limit ("Reconnecting...
  waiting for network"), so only the trial timeout stops it. Its provider
  settings accept `request_max_retries` and `stream_max_retries`.
- Claude Code's `--max-budget-usd` compares against its own cost estimate,
  which overstated OpenRouter cost 37 to 40 times. As a cap it would stop
  trials far too early.
- Fixtures under `crates/forgetest-agents/tests/` go into the crate package,
  because the crate's manifest has no `include` or `exclude`.

## Recommended S3 adjustments

1. Claude profile: add `--verbose`. Without it the profile fails at startup.
2. Claude profile: always pass `--effort LEVEL`. With an OpenRouter slug, the
   default is `high`.
3. Claude endpoint preset: `ANTHROPIC_BASE_URL=https://openrouter.ai/api`,
   authenticated with `--settings '{"apiKeyHelper":"printenv NAME"}'` and
   the key in `NAME` (verified live).
4. Codex profile: add `--skip-git-repo-check`.
5. Codex endpoint preset: the `-c model_provider=...` override with its own
   `env_key` (verified live with `--ignore-user-config`), and forward only
   that variable. Do not rely on
   `OPENAI_BASE_URL`. Note that `OPENAI_API_KEY` does not authenticate the
   built-in provider in 0.163.0 either; it reads `CODEX_API_KEY`.
6. Codex profile: always pass `-c model_reasoning_effort=...` and
   `-c web_search="disabled"`, and keep Claude's `--disallowedTools`.
7. Endpoint pinning: a provider allowlist of "OpenAI" still allows the
   `flex`, `fast` and `ultrafast` tags at 0.5× to 6× the price. Pin the
   provider tag in the key or account routing settings, or record each
   generation's provider and reject mismatches.
8. Keep `cli_version` from stdout only (already folded into S3): the image
   preflight also prints the emulation warning on arm64.
9. Study keys need a provider allowlist: without one, Anthropic models were
   served by Claude Platform on AWS. Record the serving provider per run.
10. Codex profile: `--sandbox danger-full-access` instead of
    `workspace-write` (Don decision 2026-10-26, see D5). Check it on the amd64
    runner before pilot-r2 freezes the profile; all spike evidence comes
    from an emulated host.
11. Bound Codex retries with the provider's `request_max_retries` and
    `stream_max_retries`, so an endpoint failure ends a trial quickly.
12. Do not use Claude's `--max-budget-usd` as a cost cap through OpenRouter;
    use forgetest's own pricing-table breaker.
13. Decide and disclose plugin and instruction-file symmetry: under `--bare`
    Claude still loads the `agents-md` plugin, while Codex runs with
    `--ignore-rules`.
14. The key is readable by the agent: use per-arm keys with tight limits and
    allowlists, and the month-3 egress seal (S7) to restrict both key use and
    shell network access to OpenRouter.

## Recommended S4 adjustments

1. Treat a Claude `result` with `is_error: true` or `api_error_status` as
   usage unavailable, not as zero tokens and USD 0.00 vendor cost.
2. Claude Code cannot price `anthropic/...` slugs (`costBasis: "unknown"`);
   its `total_cost_usd` overstated the billed cost about 37 to 40 times. The
   pricing table must take precedence over `total_cost_usd` through
   OpenRouter (confirmed live).
3. Golden fixtures (written): the Claude `result` carries `modelUsage` keyed
   by the slug; Codex `turn.completed` carries `cache_write_input_tokens`,
   which is a subset of `input_tokens`. Map it so cache writes are not
   counted twice. The stream-priced total matched billing exactly with this
   mapping. The fixtures are re-serialised after redaction (sorted keys,
   non-JSON lines dropped), so parsers must not depend on byte layout; see
   the fixture directory's README.
5. Count Claude usage from the `result` event only, or once per message ID;
   assistant events repeat `usage` per content block.
6. Price what the streams can express: Claude's `result.usage` splits cache
   writes into 5-minute and 1-hour tiers (different prices) but
   `modelUsage` does not, and long-prompt tiers cannot be applied from
   Codex's per-turn totals. Flag these as approximations.
7. Use the `gen-` `request_id` values in Claude streams to reconcile Claude
   runs per generation; reconcile Codex runs per key.
4. Decide in S4 or S6 whether the fixtures should ship in the crates.io
   package.

## D5 go/no-go

- **Auth: go.** Claude Code with `apiKeyHelper` and Codex with the `-c`
  provider override both complete turns through OpenRouter.
- **Sandbox: no-go for `workspace-write` under the agent container flags,
  confirmed end to end.** Codex's sandbox needs bubblewrap, bubblewrap cannot
  create a user namespace under Docker's default seccomp profile, and the
  live canary could not run a single command or write a file. The way
  forward was a Don decision.
- **Decision (Don, 2026-10-26):** Codex runs with `danger-full-access` inside
  the hardened agent container, which is its only sandbox, as Claude Code's
  is with `bypassPermissions`. S3 sets it; pilot-r2 pre-registers and
  discloses it. Not chosen: a relaxed container seccomp profile, or keeping
  `workspace-write` and reporting the failure.
- Consequences to disclose with it: under `danger-full-access`, Codex's
  commands get the container's network, as Claude's already do, and both
  agents can read the key from their environment. The S7 egress seal is the
  planned control for both.
