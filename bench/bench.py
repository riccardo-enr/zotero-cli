"""
Benchmark zotero-cli vs the Zotero MCP server (zotero-mcp).

Both call the same Zotero local API at localhost:23119 — this script
measures end-to-end latency and response payload size for equivalent
operations, so the README numbers stay honest and reproducible.

Usage:
    python3 bench/bench.py
    python3 bench/bench.py --runs 10 --query "mppi"

Requirements:
    - Zotero must be running (localhost:23119)
    - zotero-cli installed in PATH  (cargo install --path .)
    - zotero-mcp installed in PATH  (uv tool install zotero-mcp)
"""

import argparse
import json
import os
import subprocess
import sys
import time

RUNS = 5
MCP_CMD = "zotero-mcp"
CLI_CMD = "zotero-cli"
MCP_ENV = {**os.environ, "ZOTERO_LOCAL": "true"}

# ---------------------------------------------------------------------------
# MCP client (JSON-RPC over stdio)
# ---------------------------------------------------------------------------


class McpClient:
    """Minimal synchronous MCP client that speaks to zotero-mcp over stdio."""

    def __init__(self):
        self.proc = subprocess.Popen(
            [MCP_CMD],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            env=MCP_ENV,
        )
        self._id = 0
        self._initialize()

    def _send(self, msg: dict) -> None:
        line = json.dumps(msg) + "\n"
        self.proc.stdin.write(line.encode())
        self.proc.stdin.flush()

    def _recv(self) -> dict:
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise EOFError("MCP server closed stdout")
            line = line.strip()
            if line:
                return json.loads(line)

    def _recv_id(self, target_id: int) -> dict:
        while True:
            msg = self._recv()
            if msg.get("id") == target_id:
                return msg

    def _initialize(self):
        self._id += 1
        self._send(
            {
                "jsonrpc": "2.0",
                "id": self._id,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {"name": "bench", "version": "0.1"},
                },
            }
        )
        self._recv_id(self._id)
        self._send(
            {"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}
        )

    def call(self, tool: str, arguments: dict) -> tuple[float, int]:
        """Call a tool, return (latency_seconds, response_bytes)."""
        self._id += 1
        msg_id = self._id
        t0 = time.perf_counter()
        self._send(
            {
                "jsonrpc": "2.0",
                "id": msg_id,
                "method": "tools/call",
                "params": {"name": tool, "arguments": arguments},
            }
        )
        resp = self._recv_id(msg_id)
        latency = time.perf_counter() - t0
        content = resp.get("result", {}).get("content", [])
        payload = "".join(c.get("text", "") for c in content)
        return latency, len(payload.encode())

    def close(self):
        self.proc.stdin.close()
        self.proc.terminate()
        self.proc.wait()


# ---------------------------------------------------------------------------
# CLI runner
# ---------------------------------------------------------------------------


def cli_call(args: list[str]) -> tuple[float, int]:
    """Run zotero-cli, return (latency_seconds, response_bytes)."""
    t0 = time.perf_counter()
    result = subprocess.run(
        [CLI_CMD] + args,
        capture_output=True,
    )
    latency = time.perf_counter() - t0
    return latency, len(result.stdout)


# ---------------------------------------------------------------------------
# Benchmark cases
# ---------------------------------------------------------------------------

CASES = [
    {
        "name": "search",
        "cli_args": ["search", "{query}", "--json", "-l", "{limit}"],
        "mcp_tool": "zotero_search_items",
        "mcp_args": lambda q, l: {"query": q, "limit": l},
    },
    {
        "name": "recent",
        "cli_args": ["recent", "{limit}", "--json"],
        "mcp_tool": "zotero_get_recent",
        "mcp_args": lambda q, l: {"limit": l},
    },
]


def run_case(case: dict, query: str, limit: int, runs: int, mcp: McpClient):
    cli_args = [a.format(query=query, limit=str(limit)) for a in case["cli_args"]]
    mcp_args = case["mcp_args"](query, limit)

    cli_latencies, cli_sizes = [], []
    mcp_latencies, mcp_sizes = [], []

    for _ in range(runs):
        lat, size = cli_call(cli_args)
        cli_latencies.append(lat)
        cli_sizes.append(size)

    for _ in range(runs):
        lat, size = mcp.call(case["mcp_tool"], mcp_args)
        mcp_latencies.append(lat)
        mcp_sizes.append(size)

    return {
        "cli_latency_ms": sorted(cli_latencies)[runs // 2] * 1000,  # median
        "mcp_latency_ms": sorted(mcp_latencies)[runs // 2] * 1000,
        "cli_bytes": sorted(cli_sizes)[runs // 2],
        "mcp_bytes": sorted(mcp_sizes)[runs // 2],
        "cli_tokens_approx": sorted(cli_sizes)[runs // 2] // 4,
        "mcp_tokens_approx": sorted(mcp_sizes)[runs // 2] // 4,
    }


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--runs", type=int, default=RUNS)
    parser.add_argument("--query", default="mppi")
    parser.add_argument("--limit", type=int, default=25)
    args = parser.parse_args()

    print(f"Benchmarking: {args.runs} runs, query='{args.query}', limit={args.limit}")
    print(f"zotero-cli: {subprocess.check_output(['which', CLI_CMD]).decode().strip()}")
    print(f"zotero-mcp: {subprocess.check_output(['which', MCP_CMD]).decode().strip()}")
    print()

    mcp = McpClient()

    results = {}
    for case in CASES:
        print(f"Running case: {case['name']}...")
        try:
            results[case["name"]] = run_case(
                case, args.query, args.limit, args.runs, mcp
            )
        except Exception as e:
            print(f"  FAILED: {e}", file=sys.stderr)
            results[case["name"]] = None

    mcp.close()

    # Print table
    print()
    print(
        f"{'Operation':<20} {'Tool':<12} {'Latency (ms)':<16} {'Payload (bytes)':<18} {'~Tokens'}"
    )
    print("-" * 80)
    for name, r in results.items():
        if r is None:
            print(f"{name:<20} FAILED")
            continue
        print(
            f"{name:<20} {'zotero-cli':<12} {r['cli_latency_ms']:<16.1f} {r['cli_bytes']:<18} {r['cli_tokens_approx']}"
        )
        print(
            f"{'':<20} {'zotero-mcp':<12} {r['mcp_latency_ms']:<16.1f} {r['mcp_bytes']:<18} {r['mcp_tokens_approx']}"
        )
        speedup = (
            r["mcp_latency_ms"] / r["cli_latency_ms"] if r["cli_latency_ms"] else 0
        )
        token_ratio = (
            r["mcp_tokens_approx"] / r["cli_tokens_approx"]
            if r["cli_tokens_approx"]
            else 0
        )
        print(
            f"  → CLI is {speedup:.1f}x faster, MCP sends {token_ratio:.1f}x more tokens"
        )
        print()

    # Emit markdown table for README copy-paste
    print("\n--- Markdown table ---")
    print("| Operation | Tool | Latency (ms) | Payload | ~Tokens |")
    print("|---|---|---|---|---|")
    for name, r in results.items():
        if r is None:
            continue
        print(
            f"| `{name}` | `zotero-cli` | {r['cli_latency_ms']:.0f} ms | {r['cli_bytes']} B | {r['cli_tokens_approx']} |"
        )
        print(
            f"| | `zotero-mcp` | {r['mcp_latency_ms']:.0f} ms | {r['mcp_bytes']} B | {r['mcp_tokens_approx']} |"
        )


if __name__ == "__main__":
    main()
