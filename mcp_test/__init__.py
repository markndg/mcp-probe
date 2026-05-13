"""Python facade for the `mcp-test` Rust CLI."""

from mcp_test.runner import RunResult, run_conformance, run_fuzz, run_suite

__all__ = ["RunResult", "run_conformance", "run_fuzz", "run_suite"]
