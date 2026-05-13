from __future__ import annotations

import json
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping


@dataclass(frozen=True)
class RunResult:
    """Structured output from a suite run."""

    ok: bool
    report: Mapping[str, Any]
    stdout: str
    stderr: str
    returncode: int


def default_mcp_test_binary() -> str:
    exe = shutil.which("mcp-test")
    if exe:
        return exe
    raise FileNotFoundError(
        "Could not find `mcp-test` on PATH. Build the Rust workspace and add "
        "`target/release` (or `target/debug`) to PATH, or pass `mcp_test_bin=` explicitly."
    )


def run_suite(
    config_path: str | Path,
    *,
    timeout_ms: int = 5000,
    protocol_version: str | None = None,
    junit_path: str | Path | None = None,
    sarif_path: str | Path | None = None,
    trace_file: str | Path | None = None,
    client_reply_file: str | Path | None = None,
    mcp_test_bin: str | None = None,
) -> RunResult:
    """Execute `mcp-test run` and parse the JSON report printed to stdout."""
    bin_path = mcp_test_bin or default_mcp_test_binary()
    config_path = Path(config_path)
    cmd: list[str] = [
        bin_path,
        "run",
        "--config",
        str(config_path),
        "--timeout-ms",
        str(timeout_ms),
    ]
    if protocol_version is not None:
        cmd.extend(["--protocol-version", protocol_version])
    if junit_path is not None:
        cmd.extend(["--junit", str(junit_path)])
    if sarif_path is not None:
        cmd.extend(["--sarif", str(sarif_path)])
    if trace_file is not None:
        cmd.extend(["--trace-file", str(trace_file)])
    if client_reply_file is not None:
        cmd.extend(["--client-reply-file", str(client_reply_file)])
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    try:
        report = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(
            "mcp-test did not emit valid JSON on stdout.\n"
            f"stdout:\n{proc.stdout}\n"
            f"stderr:\n{proc.stderr}\n"
        ) from exc

    passed = bool(report.get("passed", False))
    ok = proc.returncode == 0 and passed
    return RunResult(
        ok=ok,
        report=report,
        stdout=proc.stdout,
        stderr=proc.stderr,
        returncode=proc.returncode,
    )


def run_conformance(
    *,
    command: str,
    server_args: list[str] | None = None,
    cwd: str | Path | None = None,
    timeout_ms: int = 5000,
    protocol_version: str | None = None,
    junit_path: str | Path | None = None,
    sarif_path: str | Path | None = None,
    trace_file: str | Path | None = None,
    client_reply_file: str | Path | None = None,
    pack: str = "builtin",
    mcp_test_bin: str | None = None,
) -> RunResult:
    """Execute `mcp-test conformance` against a server command."""
    bin_path = mcp_test_bin or default_mcp_test_binary()
    cmd: list[str] = [bin_path, "conformance", "--command", command, "--timeout-ms", str(timeout_ms)]
    for arg in server_args or []:
        cmd.extend(["--server-arg", arg])
    if cwd is not None:
        cmd.extend(["--cwd", str(cwd)])
    if protocol_version is not None:
        cmd.extend(["--protocol-version", protocol_version])
    if junit_path is not None:
        cmd.extend(["--junit", str(junit_path)])
    if sarif_path is not None:
        cmd.extend(["--sarif", str(sarif_path)])
    if trace_file is not None:
        cmd.extend(["--trace-file", str(trace_file)])
    if client_reply_file is not None:
        cmd.extend(["--client-reply-file", str(client_reply_file)])
    cmd.extend(["--pack", pack])
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    try:
        report = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(
            "mcp-test did not emit valid JSON on stdout.\n"
            f"stdout:\n{proc.stdout}\n"
            f"stderr:\n{proc.stderr}\n"
        ) from exc

    passed = bool(report.get("passed", False))
    ok = proc.returncode == 0 and passed
    return RunResult(
        ok=ok,
        report=report,
        stdout=proc.stdout,
        stderr=proc.stderr,
        returncode=proc.returncode,
    )


def run_fuzz(
    config_path: str | Path,
    *,
    iterations: int = 50,
    timeout_ms: int = 2000,
    protocol_version: str | None = None,
    mcp_test_bin: str | None = None,
) -> RunResult:
    """Execute `mcp-test fuzz` against the first step of the first scenario in a suite."""
    bin_path = mcp_test_bin or default_mcp_test_binary()
    cmd: list[str] = [
        bin_path,
        "fuzz",
        "--config",
        str(config_path),
        "--iterations",
        str(iterations),
        "--timeout-ms",
        str(timeout_ms),
    ]
    if protocol_version is not None:
        cmd.extend(["--protocol-version", protocol_version])
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    report: dict[str, Any] = {"fuzz": "completed", "returncode": proc.returncode}
    ok = proc.returncode == 0
    return RunResult(
        ok=ok,
        report=report,
        stdout=proc.stdout,
        stderr=proc.stderr,
        returncode=proc.returncode,
    )
