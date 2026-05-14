from __future__ import annotations

import argparse
import json
import sys

from mcp_probe.runner import run_conformance, run_suite


def main() -> None:
    parser = argparse.ArgumentParser(prog="mcp-probe-py")
    sub = parser.add_subparsers(dest="cmd", required=True)

    run_p = sub.add_parser("run", help="Run a suite JSON file")
    run_p.add_argument("--config", required=True)
    run_p.add_argument("--junit", default=None)
    run_p.add_argument("--timeout-ms", type=int, default=5000)
    run_p.add_argument("--mcp-probe-bin", default=None)

    conf_p = sub.add_parser("conformance", help="Run built-in conformance pack against a server command")
    conf_p.add_argument("--command", required=True)
    conf_p.add_argument("--server-arg", action="append", default=[])
    conf_p.add_argument("--cwd", default=None)
    conf_p.add_argument("--junit", default=None)
    conf_p.add_argument("--timeout-ms", type=int, default=5000)
    conf_p.add_argument("--mcp-probe-bin", default=None)

    args = parser.parse_args()
    if args.cmd == "run":
        res = run_suite(
            args.config,
            junit_path=args.junit,
            timeout_ms=args.timeout_ms,
            mcp_probe_bin=args.mcp_probe_bin,
        )
        print(json.dumps(res.report, indent=2))
        sys.exit(0 if res.ok else 1)
    if args.cmd == "conformance":
        res = run_conformance(
            command=args.command,
            server_args=args.server_arg,
            cwd=args.cwd,
            junit_path=args.junit,
            timeout_ms=args.timeout_ms,
            mcp_probe_bin=args.mcp_probe_bin,
        )
        print(json.dumps(res.report, indent=2))
        sys.exit(0 if res.ok else 1)


if __name__ == "__main__":
    main()
