from __future__ import annotations

import argparse
import json
import sys

from mcp_test.runner import run_conformance, run_suite


def main() -> None:
    parser = argparse.ArgumentParser(prog="mcp-test-py")
    sub = parser.add_subparsers(dest="cmd", required=True)

    run_p = sub.add_parser("run", help="Run a suite JSON file")
    run_p.add_argument("--config", required=True, help="Path to suite JSON")
    run_p.add_argument("--timeout-ms", type=int, default=5000)
    run_p.add_argument("--protocol-version", default=None)
    run_p.add_argument("--junit", default=None, help="Optional JUnit XML output path")
    run_p.add_argument(
        "--mcp-test-bin",
        default=None,
        help="Explicit path to the `mcp-test` executable",
    )

    conf_p = sub.add_parser("conformance", help="Run built-in conformance pack")
    conf_p.add_argument("--command", required=True, help="Server executable")
    conf_p.add_argument(
        "--server-arg",
        action="append",
        default=[],
        help="Extra server argument (repeatable)",
    )
    conf_p.add_argument("--cwd", default=None)
    conf_p.add_argument("--timeout-ms", type=int, default=5000)
    conf_p.add_argument("--protocol-version", default=None)
    conf_p.add_argument("--junit", default=None)
    conf_p.add_argument("--mcp-test-bin", default=None)

    args = parser.parse_args()
    if args.cmd == "run":
        result = run_suite(
            args.config,
            timeout_ms=args.timeout_ms,
            protocol_version=args.protocol_version,
            junit_path=args.junit,
            mcp_test_bin=args.mcp_test_bin,
        )
    elif args.cmd == "conformance":
        result = run_conformance(
            command=args.command,
            server_args=list(args.server_arg),
            cwd=args.cwd,
            timeout_ms=args.timeout_ms,
            protocol_version=args.protocol_version,
            junit_path=args.junit,
            mcp_test_bin=args.mcp_test_bin,
        )
    else:
        raise AssertionError("unreachable")

    sys.stdout.write(json.dumps(result.report, indent=2))
    sys.stdout.write("\n")
    raise SystemExit(0 if result.ok else 1)


if __name__ == "__main__":
    main()
