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
    mcp_test_bin: str | None = None,
) -> RunResult:
    """Execute `mcp-test run` and parse the JSON report printed to stdout."""
    bin_path = mcp_test_bin or default_mcp_test_binary()
    config_path = Path(config_path)
    cmd = [
        bin_path,
        "run",
        "--config",
        str(config_path),
        "--timeout-ms",
        str(timeout_ms),
    ]
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
