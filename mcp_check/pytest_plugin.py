"""Pytest fixtures for driving `mcp-check` from tests."""

from __future__ import annotations

import shutil
from pathlib import Path

import pytest

from mcp_check.runner import run_conformance, run_suite


def pytest_addoption(parser: pytest.Parser) -> None:
    parser.addoption(
        "--mcp-check-bin",
        action="store",
        default=None,
        help="Path to the `mcp-check` executable (defaults to PATH lookup).",
    )


@pytest.fixture
def mcp_check_bin(request: pytest.FixtureRequest) -> str:
    opt = request.config.getoption("--mcp-check-bin")
    if opt:
        return str(opt)
    exe = shutil.which("mcp-check")
    if exe:
        return exe
    # Common local dev layout after `cargo build`
    root = Path(request.config.rootpath)
    for candidate in (
        root / "target" / "debug" / "mcp-check",
        root / "target" / "release" / "mcp-check",
    ):
        if candidate.is_file():
            return str(candidate)
    pytest.skip("mcp-check binary not found; build the Rust workspace or pass --mcp-check-bin")


@pytest.fixture
def mcp_run_suite(mcp_check_bin: str):
    def _run(config: str | Path, **kwargs):
        kwargs.setdefault("mcp_check_bin", mcp_check_bin)
        return run_suite(config, **kwargs)

    return _run


@pytest.fixture
def mcp_run_conformance(mcp_check_bin: str):
    def _run(**kwargs):
        kwargs.setdefault("mcp_check_bin", mcp_check_bin)
        return run_conformance(**kwargs)

    return _run
