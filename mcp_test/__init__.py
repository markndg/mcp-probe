"""Python facade for the `mcp-test` Rust CLI."""

from mcp_test.runner import RunResult, run_suite

__all__ = ["RunResult", "run_suite"]
