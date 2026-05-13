from __future__ import annotations

import argparse
import json
import sys

from mcp_test.runner import run_suite


def main() -> None:
    parser = argparse.ArgumentParser(prog="mcp-test-py")
    parser.add_argument("--config", required=True, help="Path to suite JSON")
    parser.add_argument("--timeout-ms", type=int, default=5000)
    parser.add_argument(
        "--mcp-test-bin",
        default=None,
        help="Explicit path to the `mcp-test` executable",
    )
    args = parser.parse_args()
    result = run_suite(
        args.config,
        timeout_ms=args.timeout_ms,
        mcp_test_bin=args.mcp_test_bin,
    )
    sys.stdout.write(json.dumps(result.report, indent=2))
    sys.stdout.write("\n")
    raise SystemExit(0 if result.ok else 1)


if __name__ == "__main__":
    main()
