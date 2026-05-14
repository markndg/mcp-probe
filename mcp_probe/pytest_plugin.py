"""Pytest fixtures for driving ``mcp-probe`` from tests."""

from __future__ import annotations

import shutil
from pathlib import Path

import pytest

from mcp_probe.runner import run_conformance, run_suite


def pytest_addoption(parser: pytest.Parser) -> None:
    parser.addoption(
        "--mcp-probe-bin",
        action="store",
        default=None,
        help="Path to the `mcp-probe` executable (defaults to PATH lookup).",
    )


@pytest.fixture(scope="session")
def mcp_probe_bin(request: pytest.FixtureRequest) -> str:
    opt = request.config.getoption("--mcp-probe-bin")
    if opt:
        return str(opt)
    exe = shutil.which("mcp-probe")
    if exe:
        return exe
    root = Path(__file__).resolve().parents[1]
    for candidate in (
        root / "target" / "debug" / "mcp-probe",
        root / "target" / "release" / "mcp-probe",
    ):
        if candidate.is_file():
            return str(candidate)
    pytest.skip("mcp-probe binary not found; build the Rust workspace or pass --mcp-probe-bin")


@pytest.fixture
def mcp_run_suite(mcp_probe_bin: str):
    def _run(**kwargs):
        kwargs.setdefault("mcp_probe_bin", mcp_probe_bin)
        return run_suite(**kwargs)

    return _run


@pytest.fixture
def mcp_run_conformance(mcp_probe_bin: str):
    def _run(**kwargs):
        kwargs.setdefault("mcp_probe_bin", mcp_probe_bin)
        return run_conformance(**kwargs)

    return _run
