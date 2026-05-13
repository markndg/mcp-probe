"""Python facade for the `mcp-check` Rust CLI."""

from mcp_check.runner import RunResult, run_conformance, run_fuzz, run_suite

__all__ = ["RunResult", "run_conformance", "run_fuzz", "run_suite"]
