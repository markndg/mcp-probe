# mcp-probe

**Contract testing and conformance checks for [Model Context Protocol](https://modelcontextprotocol.io/) (MCP) servers.**

MCP is becoming the default way to connect agents to tools—but most teams still validate servers by hand: click through a client, eyeball JSON, and hope production traffic looks the same. **mcp-probe** turns that into something you can run in CI: spawn your server, complete a real MCP session over **stdio**, assert on structured responses, optionally enforce **JSON Schema**, and ship reports your pipeline already understands (**JSON + JUnit**).

If you ship an MCP server, this is the kind of harness that catches regressions *before* a user’s agent does.

---

## Why this exists

- **MCP is a wire protocol**, not just an SDK convenience. Behaviour lives across `initialize`, capability negotiation, listing surfaces (`tools/*`, `resources/*`, `prompts/*`), and eventual `tools/call` traffic.
- **There is no shared conformance suite** most teams run against—everyone reinvents smoke tests.
- **CI wants artifacts**: machine-readable results, stable exit codes, and reports that integrate with GitHub Actions, Jenkins, GitLab, etc.

**mcp-probe** is a small, opinionated runner focused on *repeatable* black-box validation: declarative suites in JSON, a Rust core for speed and safety, and a thin Python façade for teams that orchestrate everything in pytest.

---

## What works today

| Capability | Notes |
|------------|--------|
| **Stdio transport** | Spawns your server as a subprocess; newline-delimited JSON-RPC as per common MCP stdio practice. |
| **HTTP (experimental)** | Optional JSON-RPC POST + `Mcp-Session-Id` client for bring-up; stdio remains the primary tested path. |
| **Real handshake** | Sends `initialize`, validates a sane `initialize` result, then `notifications/initialized`. |
| **Declarative suites** | JSON files with `server`, `scenarios`, and `steps` (`send` + `expect`). |
| **Subset matching** | Deep object subset + order-independent array matching for flexible assertions on `result`. |
| **JSON Schema (v2)** | Inline `result_schema` and/or on-disk `result_schema_path` (relative to the suite file). |
| **Starter conformance pack** | `mcp-probe conformance` runs schema-backed checks for `tools/list`, `resources/list`, and `prompts/list`. |
| **JUnit + JSON reports** | `--junit` for CI dashboards; `--report` for a JSON copy; human-readable summary on stdout. |
| **Protocol version flag** | `--protocol-version` passed through `initialize` (default `2024-11-05`). |
| **Rust library** | Embed `run_suite`, `McpStdioSession`, matchers, and JUnit rendering in your own tooling. |
| **Python wrapper** | Subprocess bridge around the CLI—stdlib only at runtime. |
| **SARIF reports** | `--sarif` on `run` and `conformance`; upload to GitHub Security tab via `github/codeql-action/upload-sarif`. |

**Non-goals (today):** Streamable HTTP / SSE, server-initiated client requests, or a full formal verification of every MCP edge case. Experimental JSON-RPC-over-HTTP exists for local bring-up, but it is not yet parity-tested like stdio. The `mcp-probe fuzz` command is a **crash smoke** tool (randomised params and methods on a fixed suite step), not protocol-aware framing fuzzing. The project is intentionally narrow so it stays fast to adopt and hard to misuse.

---

## Install

### Rust (CLI + library)

From a checkout of this repository:

```bash
cargo build --release
```

Add `target/release` to your `PATH` (or invoke `target/release/mcp-probe` by absolute path in CI).

## GitHub Security tab integration

`mcp-probe` can emit [SARIF](https://sarifweb.azurewebsites.net/) alongside JUnit, letting failed conformance checks appear as **code scanning alerts** in your pull requests — in the same place as CVE findings and secret scanning.

```yaml
- name: Run mcp-probe conformance
  run: |
    mcp-probe conformance \
      --command my-mcp-server \
      --junit results/conformance.xml \
      --sarif results/conformance.sarif

- name: Upload results to GitHub Security tab
  uses: github/codeql-action/upload-sarif@v3
  if: always()
  with:
    sarif_file: results/conformance.sarif
```

Each failed scenario becomes a code scanning alert with the rule ID, failure message, and a link back to the suite file. Pass `--sarif` to `mcp-probe run` for custom suites in the same way.

### Python (optional)

```bash
pip install -e .
```

This installs the `mcp-probe-py` helper, which shells out to the `mcp-probe` binary—**build the Rust CLI first** and ensure it is on `PATH`, or pass an explicit binary path in code.

---

## Thirty-second tour

### 1) Run the built-in conformance pack

Point at *your* MCP server executable (stdio). Extra arguments use repeatable `--server-arg`:

```bash
mcp-probe conformance \
  --command npx \
  --server-arg -y \
  --server-arg @modelcontextprotocol/server-memory \
  --timeout-ms 15000 \
  --junit target/mcp-conformance.xml
```

Exit code **0** means every scenario passed; **1** means at least one failure (CI-friendly).

### 2) Run your own suite

```bash
mcp-probe run \
  --config ./suites/production-smoke.json \
  --report target/mcp-report.json \
  --junit target/mcp-junit.xml \
  --protocol-version 2024-11-05
```

---

## Suite file format

Top-level shape:

```json
{
  "version": 2,
  "server": {
    "command": "my-mcp-server",
    "args": ["--stdio"],
    "cwd": "/optional/workdir",
    "env": { "MY_FLAG": "1" }
  },
  "scenarios": [
    {
      "name": "lists tools",
      "steps": [
        {
          "send": { "method": "tools/list", "params": {} },
          "expect": {
            "result": { "tools": [{ "name": "search" }] },
            "result_schema": { "type": "object", "required": ["tools"] }
          }
        }
      ]
    }
  ]
}
```

### `version`

- **1** or **2** are accepted by the CLI.
- **v2** is the format that officially documents schema fields; **v1** suites that only used `expect.result` remain valid.

### `expect` (applies to the JSON-RPC **`result`** object only)

You must specify **at least one** of:

| Field | Purpose |
|--------|---------|
| `result` | Structural subset match against `result`. |
| `result_schema` | Inline JSON Schema document for `result`. |
| `result_schema_path` | Path to a `.json` schema file, **relative to the directory containing the suite file**. |

**Do not** set both `result_schema` and `result_schema_path` on the same step.

You may combine **`result` + a schema** for “shape + important invariants” testing.

### Subset matching (when `result` is present)

- **Objects:** every key in the expected object must exist in the actual object with recursively matching values.
- **Arrays:** each expected element must match **some** unused actual element (order-independent).
- **Scalars:** must be equal.

### JSON Schema (when `result_schema` or `result_schema_path` is present)

Powered by the [`jsonschema`](https://docs.rs/jsonschema) crate with **default features disabled** in this workspace to keep the dependency graph lean (no automatic network fetches for remote `$ref` in the default build).

---

## CLI reference

### `mcp-probe run`

| Flag | Meaning |
|------|---------|
| `--config PATH` | Suite JSON file (**required**). |
| `--timeout-ms N` | Per-request timeout (default `5000`). |
| `--protocol-version VER` | MCP `initialize` protocol string (default `2024-11-05`). |
| `--report PATH` | Optional JSON report (pretty-printed). |
| `--junit PATH` | Optional JUnit XML report. |

### `mcp-probe conformance`

Runs the embedded starter pack (schema checks on list endpoints).

| Flag | Meaning |
|------|---------|
| `--command CMD` | Server executable (**required**). |
| `--server-arg ARG` | Extra argv token (**repeatable**). |
| `--cwd PATH` | Working directory for the child process. |
| `--timeout-ms`, `--protocol-version`, `--report`, `--junit` | Same semantics as `run`. |

---

## Python API

```python
from pathlib import Path
from mcp_probe.runner import run_suite, run_conformance

# Custom suite
r = run_suite(
    Path("suites/smoke.json"),
    timeout_ms=10_000,
    protocol_version="2024-11-05",
    junit_path=Path("out/junit.xml"),
)
assert r.ok, r.report

# Built-in conformance pack
r = run_conformance(
    command="npx",
    server_args=["-y", "@modelcontextprotocol/server-memory"],
    junit_path=Path("out/conformance.xml"),
)
```

Console helper:

```bash
mcp-probe-py run --config suites/smoke.json --junit out/junit.xml
mcp-probe-py conformance --command ./my-server --server-arg --config ./config.toml
```

---

## Architecture (high level)

- **`mcp-probe-core`** — MCP stdio session (`McpStdioSession`), experimental HTTP POST session (`McpHttpSession`), handshake, JSON-RPC `call`, subset matcher, optional JSON Schema validation, suite model, conformance definitions, JUnit rendering, `run_suite` / `run_scenario`.
- **`mcp-probe`** — `clap` CLI (`run`, `conformance`) and filesystem glue for reports.
- **`mcp_probe` (Python)** — Typed subprocess wrapper; no heavy native bindings yet.

Each **scenario** currently gets a **fresh server process** and handshake for isolation. That trades speed for determinism—ideal for CI, easy to relax later if you add a “shared session” mode.

---

## Development

See [`CLAUDE.md`](./CLAUDE.md) for project conventions (Rust + Python commands, style, and workflow expectations).

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```

---

## Roadmap (where this can go)

Short term, high leverage:

- **Streamable HTTP / SSE transport** so the same suites run against remotely hosted MCP servers.
- **Richer assertions**: first-class JSON Pointer / JMESPath, expected JSON-RPC **errors** (`code` / `message`), and timing assertions.
- **`tools/call` recipes** with per-tool argument + result schemas (today you compose that manually in steps).
- **Larger conformance packs** versioned with the MCP spec revision, including negative cases and capability-gated skips.

Medium term:

- **Recording mode** to capture golden transcripts from a working session and shrink them into suites.
- **Fuzzing / property tests** on framing + parser boundaries (oversized messages, unicode, duplicate ids)—beyond what the current CLI `fuzz` subcommand does (see non-goals above).
- **Server→client request** handling in the harness (today server-initiated JSON-RPC requests are largely ignored in v1 client mode).
- **Published binaries** and a **GitHub Action** so adoption is `uses: …` instead of `cargo install`.

Ecosystem:

- **pytest plugin** with fixtures (`mcp_server`, `mcp_suite`) for Python-heavy repos.
- **Editor integration** (run suite under cursor, jump to failing step).

If you are building MCP infrastructure or a fleet of internal servers, those items are the difference between “we have tests” and “we have *coverage of the protocol surface*.”

---

## License

MIT (see workspace `Cargo.toml` / `pyproject.toml` metadata).

---

## Contributing

Issues and PRs welcome—especially:

- additional **conformance** cases grounded in the public MCP spec,
- **transport** backends,
- and **real-world suite examples** (redacted) that stress edge cases.

If you want this project to become the *de facto* MCP CI gate, the fastest path is dogfooding: run `mcp-probe conformance` against every server you maintain, and open issues for anything the harness should catch but does not.
