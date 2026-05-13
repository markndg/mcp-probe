"""Pytest fixtures for driving `mcp-test` from tests."""

from __future__ import annotations

import shutil
from pathlib import Path

import pytest

from mcp_test.runner import run_conformance, run_suite


def pytest_addoption(parser: pytest.Parser) -> None:
    parser.addoption(
        "--mcp-test-bin",
        action="store",
        default=None,
        help="Path to the `mcp-test` executable (defaults to PATH lookup).",
    )


@pytest.fixture
def mcp_test_bin(request: pytest.FixtureRequest) -> str:
    opt = request.config.getoption("--mcp-test-bin")
    if opt:
        return str(opt)
    exe = shutil.which("mcp-test")
    if exe:
        return exe
    # Common local dev layout after `cargo build`
    root = Path(request.config.rootpath)
    for candidate in (
        root / "target" / "debug" / "mcp-test",
        root / "target" / "release" / "mcp-test",
    ):
        if candidate.is_file():
            return str(candidate)
    pytest.skip("mcp-test binary not found; build the Rust workspace or pass --mcp-test-bin")


@pytest.fixture
def mcp_run_suite(mcp_test_bin: str):
    def _run(config: str | Path, **kwargs):
        kwargs.setdefault("mcp_test_bin", mcp_test_bin)
        return run_suite(config, **kwargs)

    return _run


@pytest.fixture
def mcp_run_conformance(mcp_test_bin: str):
    def _run(**kwargs):
        kwargs.setdefault("mcp_test_bin", mcp_test_bin)
        return run_conformance(**kwargs)

    return _run
