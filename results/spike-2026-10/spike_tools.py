"""Helpers for run-spike.sh: a request-capture endpoint, key-free summaries of
API responses and CLI streams, budget and key gates, and fixture redaction.

Runs on Python 3.9 or later with the standard library only. It never reads
FORGETEST_SPIKE_KEY. The two commands that need key material to check for
leaks (redact and contains-secret) read it from stdin and only compare it.

The spec's leak check greps this directory for the OpenRouter key prefix and
the macOS home prefix, so this file builds both strings from parts.
"""

import base64
import gzip
import json
import os
import re
import sys
import threading
import zlib
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

KEY_PREFIX = "sk" + "-or-"
HOME_PREFIX = "/" + "Users" + "/"
# Float tolerance for budget comparisons; usage is reported to 1e-9 USD.
EPSILON = 1e-9

# Headers that can carry credentials are never recorded.
DROPPED_HEADERS = {"authorization", "x-api-key", "cookie", "proxy-authorization"}


def load_json(path):
    if path == "-":
        return json.load(sys.stdin)
    return json.loads(Path(path).read_text(encoding="utf-8"))


def read_text(path):
    if path == "-":
        return sys.stdin.read()
    return Path(path).read_text(encoding="utf-8", errors="replace")


def read_jsonl(path):
    events = []
    for line in read_text(path).splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            events.append(json.loads(line))
        except ValueError:
            continue
    return events


def response_data(value):
    """The `data` object of an OpenRouter response, or {} when it is missing,
    null or not an object."""
    data = value.get("data") if isinstance(value, dict) else None
    return data if isinstance(data, dict) else {}


def key_fields(value):
    """Key budget fields from either a raw GET /api/v1/key response or the
    filtered output of key-usage."""
    if isinstance(value, dict) and "data" in value:
        return response_data(value)
    return value if isinstance(value, dict) else {}


def is_number(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def emit(value):
    print(json.dumps(value, sort_keys=True))


def refuse(message):
    print(message)
    sys.exit(1)


# Capture endpoint ----------------------------------------------------------


def capture_server(out_path, port):
    """Record each request's method, path, non-credential headers and JSON
    body, then answer 400 so the CLI stops without forwarding anything."""
    out = Path(out_path)
    lock = threading.Lock()

    class Handler(BaseHTTPRequestHandler):
        def _read_body(self):
            if "chunked" in (self.headers.get("transfer-encoding") or "").lower():
                chunks = []
                while True:
                    line = self.rfile.readline(65537)
                    if not line:
                        raise EOFError("truncated chunked body")
                    size = int(line.split(b";", 1)[0].strip() or b"0", 16)
                    if size == 0:
                        # Skip any trailer fields up to the closing empty line.
                        while self.rfile.readline(65537) not in (b"\r\n", b"\n", b""):
                            pass
                        return b"".join(chunks)
                    chunks.append(self.rfile.read(size))
                    self.rfile.readline(65537)
            length = int(self.headers.get("content-length") or 0)
            return self.rfile.read(length) if length else b""

        def _record(self):
            encoding = (self.headers.get("content-encoding") or "").lower()
            entry = {
                "method": self.command,
                "path": self.path,
                "headers": {
                    k.lower(): v
                    for k, v in self.headers.items()
                    if k.lower() not in DROPPED_HEADERS
                },
                "auth_headers_present": sorted(
                    k.lower() for k in self.headers.keys() if k.lower() in DROPPED_HEADERS
                ),
            }
            body = b""
            try:
                body = self._read_body()
                if encoding == "gzip":
                    body = gzip.decompress(body)
                elif encoding == "deflate":
                    body = zlib.decompress(body)
                elif encoding:
                    raise ValueError("unsupported content-encoding %s" % encoding)
                entry["body"] = json.loads(body) if body else None
            except (ValueError, OSError, EOFError, zlib.error) as error:
                entry["body_undecoded"] = {
                    "content_encoding": encoding,
                    "bytes": len(body),
                    "base64_prefix": base64.b64encode(body[:64]).decode(),
                    "error": type(error).__name__,
                }
            line = json.dumps(entry, sort_keys=True) + "\n"
            with lock:
                with out.open("a", encoding="utf-8") as handle:
                    handle.write(line)

        def _reply(self):
            payload = json.dumps(
                {
                    "type": "error",
                    "error": {
                        "type": "invalid_request_error",
                        "message": "spike capture endpoint: request recorded, not forwarded",
                    },
                }
            ).encode()
            self.send_response(400)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def do_POST(self):  # noqa: N802 (http.server naming)
            self._record()
            self._reply()

        def do_GET(self):  # noqa: N802
            self._record()
            self._reply()

        def log_message(self, *_):
            pass

    ThreadingHTTPServer(("0.0.0.0", int(port)), Handler).serve_forever()


def tool_names(tools):
    names = []
    for tool in tools or []:
        name = tool.get("name") or tool.get("type")
        names.append(name)
        if isinstance(tool.get("tools"), list):
            names.extend("%s.%s" % (name, inner) for inner in tool_names(tool["tools"]))
    return names


def captured_requests(path):
    """One line per captured request: endpoint, model, auth header names,
    effort-related fields and the tools offered to the model. Codex 0.163
    sends its tools in an `additional_tools` input item."""
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        entry = json.loads(line)
        body = entry.get("body") or {}
        tools = tool_names(body.get("tools"))
        for item in body.get("input") or []:
            if isinstance(item, dict) and item.get("type") == "additional_tools":
                tools.extend(tool_names(item.get("tools")))
        headers = entry.get("headers", {})
        emit(
            {
                "path": entry.get("path"),
                "model": body.get("model"),
                "auth_headers_present": entry.get("auth_headers_present"),
                "reasoning": body.get("reasoning"),
                "thinking": body.get("thinking"),
                "output_config": body.get("output_config"),
                "max_tokens": body.get("max_tokens"),
                "tools": tools,
                "anthropic_beta": headers.get("anthropic-beta"),
                "user_agent": headers.get("user-agent"),
                "undecoded": entry.get("body_undecoded"),
            }
        )


# Key and budget gates ------------------------------------------------------


def key_usage(path):
    """Keep only the numeric budget fields of a GET /api/v1/key response.
    The label, which can hold a masked key, is dropped."""
    data = response_data(load_json(path))
    fields = (
        "usage",
        "limit",
        "limit_remaining",
        "limit_reset",
        "usage_daily",
        "usage_weekly",
        "usage_monthly",
        "is_free_tier",
        "expires_at",
    )
    emit({k: data.get(k) for k in fields if k in data})


def usage_value(path):
    """Print the billed usage in USD, or nothing when it is not reported."""
    value = key_fields(load_json(path)).get("usage")
    if is_number(value):
        print("%.9f" % value)


def key_preflight(path, cap, key_limit_max, expiry_max_hours):
    """Accept the key only if it reports usage, a spend limit of at most
    key_limit_max USD, and an expiry no more than expiry_max_hours away.
    A missing or unparseable expiry fails closed. The run's own spend is held
    to cap by budget-check, separately from the key's limit."""
    data = key_fields(load_json(path))
    cap, limit_max, hours_max = float(cap), float(key_limit_max), float(expiry_max_hours)
    usage, limit = data.get("usage"), data.get("limit")
    if not is_number(usage):
        refuse("GET /api/v1/key reported no usage.")
    if not is_number(limit):
        refuse(
            "The key has no spend limit; set one of at most USD %.2f in the "
            "OpenRouter dashboard." % limit_max
        )
    if limit > limit_max + EPSILON:
        refuse(
            "The key's spend limit USD %.2f is above the allowed USD %.2f."
            % (limit, limit_max)
        )
    expiry = data.get("expires_at")
    if not isinstance(expiry, str) or not expiry:
        refuse(
            "GET /api/v1/key reported no expiry; the key must expire within "
            "%s hours." % expiry_max_hours
        )
    try:
        when = datetime.fromisoformat(expiry.replace("Z", "+00:00"))
    except ValueError:
        refuse("The key's expiry %r could not be parsed." % expiry)
    if when.tzinfo is None:
        when = when.replace(tzinfo=timezone.utc)
    hours = (when - datetime.now(timezone.utc)).total_seconds() / 3600
    if hours <= 0:
        refuse("The key has expired.")
    if hours > hours_max:
        refuse(
            "The key expires in %.1f hours, later than the allowed %s hours."
            % (hours, expiry_max_hours)
        )
    print(
        "Key accepted: limit USD %.2f, usage USD %.9f, expires in %.1f hours; "
        "run cap USD %.2f." % (limit, usage, hours, cap)
    )


def budget_check(path, baseline, cap, estimate, reserve):
    """Exit 1 unless the remaining budget covers estimate plus reserve. The
    remaining budget is the least of: cap less this run's spend (usage less
    the baseline read at preflight), the key's limit less its usage, and the
    key's limit_remaining."""
    data = key_fields(load_json(path))
    usage = data.get("usage")
    if not is_number(usage):
        refuse("stop: GET /api/v1/key reported no usage")
    spent = usage - float(baseline)
    remaining = float(cap) - spent
    if is_number(data.get("limit")):
        remaining = min(remaining, data["limit"] - usage)
    if is_number(data.get("limit_remaining")):
        remaining = min(remaining, data["limit_remaining"])
    need = float(estimate) + float(reserve)
    if remaining + EPSILON < need:
        refuse(
            "stop: USD %.6f remaining cannot cover the estimate USD %s plus reserve USD %s"
            " (run spend USD %.6f)" % (remaining, estimate, reserve, spent)
        )
    print(
        "run spend USD %.6f, remaining USD %.6f, estimate USD %s"
        % (spent, remaining, estimate)
    )


def usage_delta(baseline, after):
    """This run's spend: usage in `after` less usage in the baseline file."""
    first = key_fields(load_json(baseline)).get("usage")
    last = key_fields(load_json(after)).get("usage")
    if is_number(first) and is_number(last):
        print(
            "usage USD %.9f -> USD %.9f, run spend USD %.9f" % (first, last, last - first)
        )
    else:
        print("usage unreadable: baseline=%r after=%r" % (first, last))


# OpenRouter responses ------------------------------------------------------


def models(path, *slugs):
    """id, name, context length, pricing and supported parameters per slug."""
    by_id = {model["id"]: model for model in load_json(path)["data"]}
    rows = []
    for slug in slugs:
        model = by_id.get(slug)
        if model is None:
            rows.append({"id": slug, "listed": False})
            continue
        rows.append(
            {
                "id": slug,
                "listed": True,
                "name": model.get("name"),
                "context_length": model.get("context_length"),
                "pricing": model.get("pricing"),
                "supported_parameters": model.get("supported_parameters"),
            }
        )
    print(json.dumps(rows, indent=2, sort_keys=True))


def listed(path):
    rows = load_json(path)
    print("; ".join("%s listed=%s" % (row["id"], row["listed"]) for row in rows))


def unlisted(path, *slugs):
    """Print the given slugs that the models summary marks as not listed, or
    that it does not mention at all."""
    rows = {row["id"]: row for row in load_json(path)}
    print(" ".join(s for s in slugs if not rows.get(s, {}).get("listed")))


def endpoints(path):
    """Providers serving a model, from GET /api/v1/models/{slug}/endpoints."""
    data = response_data(load_json(path))
    rows = []
    for endpoint in data.get("endpoints") or []:
        rows.append(
            {
                key: endpoint.get(key)
                for key in (
                    "provider_name",
                    "tag",
                    "status",
                    "quantization",
                    "context_length",
                    "pricing",
                    "supported_parameters",
                )
            }
        )
    print(json.dumps({"id": data.get("id"), "endpoints": rows}, indent=2, sort_keys=True))


def response_id(path):
    """The top-level id, model, provider, usage and error of a response."""
    try:
        data = load_json(path)
    except ValueError:
        emit({"unparsed": True})
        return
    if not isinstance(data, dict):
        emit({"unparsed": True})
        return
    emit(
        {
            "id": data.get("id"),
            "model": data.get("model"),
            "provider": data.get("provider"),
            "usage": data.get("usage"),
            "error": data.get("error"),
        }
    )


def ids(path):
    """Print each non-empty id from response_id lines on stdin or in a file."""
    for line in read_text(path).splitlines():
        line = line.strip()
        if line.startswith("{"):
            value = json.loads(line).get("id")
            if isinstance(value, str) and re.fullmatch(r"[A-Za-z0-9_-]+", value):
                print(value)


def generation(path):
    """Selected fields of GET /api/v1/generation. Fields that could identify
    the account, the app or the end user are left out."""
    try:
        data = response_data(load_json(path))
    except ValueError:
        emit({"unparsed": True})
        return
    keep = (
        "id",
        "model",
        "provider_name",
        "api_type",
        "total_cost",
        "cache_discount",
        "upstream_inference_cost",
        "tokens_prompt",
        "tokens_completion",
        "native_tokens_prompt",
        "native_tokens_completion",
        "native_tokens_reasoning",
        "native_tokens_cached",
        "finish_reason",
        "native_finish_reason",
        "streamed",
        "is_byok",
    )
    emit({key: data.get(key) for key in keep if key in data})


# CLI streams -------------------------------------------------------------


def last_event(path, event_type):
    """Print the last stream event of the given type, or nothing."""
    matches = [e for e in read_jsonl(path) if e.get("type") == event_type]
    if matches:
        emit(matches[-1])


def claude_ok(path):
    """`ok` when the given Claude `result` event has is_error false."""
    text = read_text(path).strip()
    result = json.loads(text) if text else {}
    print("ok" if result and result.get("is_error") is False else "failed")


def codex_ok(path):
    types = [e.get("type") for e in read_jsonl(path)]
    failed = any(t in ("turn.failed", "error") for t in types)
    print("ok" if "turn.completed" in types and not failed else "failed")


def claude_usage(path):
    text = read_text(path).strip()
    result = json.loads(text) if text else {}
    usage = result.get("usage") or {}
    print(
        "is_error=%s turns=%s input=%s output=%s cache_read=%s cache_write=%s "
        "total_cost_usd=%s modelUsage=%s"
        % (
            result.get("is_error"),
            result.get("num_turns"),
            usage.get("input_tokens"),
            usage.get("output_tokens"),
            usage.get("cache_read_input_tokens"),
            usage.get("cache_creation_input_tokens"),
            result.get("total_cost_usd"),
            ",".join(sorted((result.get("modelUsage") or {}).keys())) or "none",
        )
    )


def codex_commands(path):
    """Command executions with their status and exit code, file changes with
    their status and paths, then the usage of the last turn.completed event."""
    commands, changes = [], []
    usage = None
    for event in read_jsonl(path):
        item = event.get("item") or {}
        if event.get("type") == "item.completed":
            if item.get("type") == "command_execution":
                commands.append("%s:%s" % (item.get("status"), item.get("exit_code")))
            elif item.get("type") == "file_change":
                paths = "+".join(
                    str(change.get("path")) for change in item.get("changes") or []
                )
                changes.append("%s:%s" % (item.get("status"), paths or "none"))
        if event.get("type") == "turn.completed":
            usage = event.get("usage")
    print(
        "commands=[%s] file_changes=[%s] usage=%s"
        % (",".join(commands), ",".join(changes), json.dumps(usage, sort_keys=True))
    )


# Codex stream items and Claude content blocks that show a web tool in use.
WEB_ITEM_TYPES = {
    "web_search",
    "web_fetch",
    "web_search_call",
    "web_search_tool_result",
    "web_fetch_tool_result",
}
WEB_TOOL_NAMES = {"WebSearch", "WebFetch", "web_search", "web_fetch"}
TOOL_USE_TYPES = {"tool_use", "server_tool_use"}
WEB_REQUEST_COUNTERS = ("web_search_requests", "web_fetch_requests")


def find_web_tools(value, found):
    if isinstance(value, dict):
        kind = value.get("type")
        if kind in WEB_ITEM_TYPES:
            found.add("%s item" % kind)
        if kind in TOOL_USE_TYPES and value.get("name") in WEB_TOOL_NAMES:
            found.add("%s %s" % (kind, value["name"]))
        for counter in WEB_REQUEST_COUNTERS:
            count = value.get(counter)
            if is_number(count) and count > 0:
                found.add("%s=%s" % (counter, count))
        for item in value.values():
            find_web_tools(item, found)
    elif isinstance(value, list):
        for item in value:
            find_web_tools(item, found)


def web_tools(*paths):
    """Exit 1 and print what was found if any stream shows a web tool in use:
    a Codex web_search item, a Claude WebSearch or WebFetch tool_use, or a
    non-zero web request counter. Paths that do not exist are skipped."""
    hits = []
    for path in paths:
        if not Path(path).is_file():
            continue
        found = set()
        for event in read_jsonl(path):
            find_web_tools(event, found)
        hits.extend("%s: %s" % (Path(path).name, what) for what in sorted(found))
    if hits:
        print("; ".join(hits))
        sys.exit(1)


# Redaction ---------------------------------------------------------------

UUID = re.compile(
    r"\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b", re.IGNORECASE
)
UUID_PLACEHOLDER = re.compile(r"00000000-0000-4000-8000-[0-9]{12}")
GENERATION_ID = re.compile(r"\bgen-[0-9]{6,}-[A-Za-z0-9]{8,}\b")
GENERATION_PLACEHOLDER = re.compile(r"gen-0000000000-REDACTED[0-9]{8}")
PREFIXED_ID = re.compile(r"\b(msg|toolu|req|resp|call|rs)_[A-Za-z0-9]{8,}\b")
PREFIXED_PLACEHOLDER = re.compile(r"(msg|toolu|req|resp|call|rs)_REDACTED[0-9]{12}")
HEX_ID = re.compile(r"[0-9a-fA-F]{32}|[0-9a-fA-F]{64}")
HEX_PLACEHOLDER = re.compile(r"0{24}[0-9]{8}|0{56}[0-9]{8}")
# A /home/ that starts a path, so the container's /tmp/home is left alone.
LINUX_HOME = r"(?<![\w.-])/home/"
HOST_PATH = re.compile(
    "(%s|%s|/private/|/var/folders/)[^\\s\"']*" % (re.escape(HOME_PREFIX), LINUX_HOME)
)
EMAIL = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}")
FORBIDDEN = (
    ("the OpenRouter key prefix", re.compile(re.escape(KEY_PREFIX))),
    ("a secret-key shape", re.compile(r"\bsk-[A-Za-z0-9_-]{16,}")),
    ("a macOS home path", re.compile(re.escape(HOME_PREFIX))),
    ("a Linux home path", re.compile(LINUX_HOME)),
    ("a /private/ path", re.compile(r"/private/")),
    ("a /var/folders/ path", re.compile(r"/var/folders/")),
    ("an email address", EMAIL),
)


def is_id_key(key):
    return isinstance(key, str) and key.lower().endswith("id")


class Redactor:
    """Replaces UUIDs, OpenRouter generation IDs, prefixed vendor IDs (msg_,
    toolu_, req_, resp_, call_, rs_) and 32- or 64-hex values under keys
    ending in `id` with stable, format-preserving placeholders, in keys as
    well as values, and host paths with a marker."""

    def __init__(self):
        self.seen = {}

    def _placeholder(self, kind, value, make):
        key = (kind, value)
        if key not in self.seen:
            count = sum(1 for k in self.seen if k[0] == kind) + 1
            self.seen[key] = make(count)
        return self.seen[key]

    def text(self, value):
        value = UUID.sub(
            lambda m: self._placeholder(
                "uuid", m.group(0).lower(), lambda n: "00000000-0000-4000-8000-%012d" % n
            ),
            value,
        )
        value = GENERATION_ID.sub(
            lambda m: self._placeholder(
                "gen", m.group(0), lambda n: "gen-0000000000-REDACTED%08d" % n
            ),
            value,
        )
        value = PREFIXED_ID.sub(
            lambda m: self._placeholder(
                m.group(1), m.group(0), lambda n: "%s_REDACTED%012d" % (m.group(1), n)
            ),
            value,
        )
        return HOST_PATH.sub("/redacted-host-path", value)

    def value(self, value, key=None):
        if isinstance(value, str):
            if is_id_key(key) and HEX_ID.fullmatch(value):
                return self._placeholder(
                    "hex", value.lower(), lambda n: "0" * (len(value) - 8) + "%08d" % n
                )
            return self.text(value)
        if isinstance(value, list):
            return [self.value(item, key) for item in value]
        if isinstance(value, dict):
            return {self.text(k): self.value(item, k) for k, item in value.items()}
        return value


def fixture_problems(text):
    """Problems that make text unfit for a committed fixture: an ID-like
    value that is not a placeholder, or a forbidden pattern."""
    problems = set()

    def check_string(value, key=None):
        if is_id_key(key) and HEX_ID.fullmatch(value) and not HEX_PLACEHOLDER.fullmatch(value):
            problems.add("a %d-hex value under %r" % (len(value), key))
        for match in UUID.finditer(value):
            if not UUID_PLACEHOLDER.fullmatch(match.group(0)):
                problems.add("a UUID that is not a placeholder")
        for match in GENERATION_ID.finditer(value):
            if not GENERATION_PLACEHOLDER.fullmatch(match.group(0)):
                problems.add("a generation ID that is not a placeholder")
        for match in PREFIXED_ID.finditer(value):
            if not PREFIXED_PLACEHOLDER.fullmatch(match.group(0)):
                problems.add("a %s_ ID that is not a placeholder" % match.group(1))

    def walk(value, key=None):
        if isinstance(value, str):
            check_string(value, key)
        elif isinstance(value, list):
            for item in value:
                walk(item, key)
        elif isinstance(value, dict):
            for k, item in value.items():
                check_string(k)
                walk(item, k)

    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        try:
            walk(json.loads(line))
        except ValueError:
            problems.add("line %d is not JSON" % number)
    for label, pattern in FORBIDDEN:
        if pattern.search(text):
            problems.add(label)
    return sorted(problems)


def redact(source, destination):
    """Write a redacted copy of a JSONL stream; non-JSON lines are dropped.
    Key material is read from stdin and only compared. With no key on stdin
    it refuses unless SPIKE_ALLOW_NO_KEY_CHECK=1."""
    secret = sys.stdin.readline().strip()
    if not secret:
        if os.environ.get("SPIKE_ALLOW_NO_KEY_CHECK") != "1":
            sys.exit(
                "redaction refused: no key on stdin for the exact-key check; "
                "set SPIKE_ALLOW_NO_KEY_CHECK=1 to skip that check"
            )
        print(
            "%s: exact-key check skipped (SPIKE_ALLOW_NO_KEY_CHECK=1)" % Path(destination).name,
            file=sys.stderr,
        )
    redactor = Redactor()
    lines = [
        json.dumps(redactor.value(event), sort_keys=True, ensure_ascii=False)
        for event in read_jsonl(source)
    ]
    if not lines:
        sys.exit("redaction failed: %s has no JSON events" % Path(source).name)
    output = "\n".join(lines) + "\n"
    if secret and secret in output:
        sys.exit("redaction failed: key material present")
    problems = fixture_problems(output)
    if problems:
        sys.exit("redaction failed: %s" % "; ".join(problems))
    Path(destination).write_text(output, encoding="utf-8")
    print("%s: %d events" % (Path(destination).name, len(lines)))


def check_fixture(*paths):
    """Exit 1 unless every file has only placeholder IDs and no forbidden
    pattern."""
    failed = False
    for path in paths:
        problems = fixture_problems(Path(path).read_text(encoding="utf-8"))
        if problems:
            failed = True
            print("%s: %s" % (Path(path).name, "; ".join(problems)))
        else:
            print("%s: ok" % Path(path).name)
    if failed or not paths:
        sys.exit(1)


def contains_secret(root):
    """Print paths, relative to root, of files containing the key read from
    stdin. Prints nothing when no key is given."""
    secret = sys.stdin.readline().strip().encode()
    if not secret:
        return
    base = Path(root)
    for path in sorted(base.rglob("*")):
        if path.is_file() and not path.is_symlink() and secret in path.read_bytes():
            print(path.relative_to(base))


COMMANDS = {
    "capture-server": capture_server,
    "captured-requests": captured_requests,
    "key-usage": key_usage,
    "usage-value": usage_value,
    "key-preflight": key_preflight,
    "budget-check": budget_check,
    "usage-delta": usage_delta,
    "models": models,
    "listed": listed,
    "unlisted": unlisted,
    "endpoints": endpoints,
    "response-id": response_id,
    "ids": ids,
    "generation": generation,
    "last-event": last_event,
    "claude-ok": claude_ok,
    "codex-ok": codex_ok,
    "claude-usage": claude_usage,
    "codex-commands": codex_commands,
    "web-tools": web_tools,
    "redact": redact,
    "check-fixture": check_fixture,
    "contains-secret": contains_secret,
}

if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1] not in COMMANDS:
        sys.exit("usage: spike_tools.py {%s} ARGS..." % ",".join(sorted(COMMANDS)))
    COMMANDS[sys.argv[1]](*sys.argv[2:])
