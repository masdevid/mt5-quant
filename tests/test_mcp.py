#!/usr/bin/env python3
"""MCP test harness: run backtest then exercise all granular analysis tools."""

import json
import subprocess
import sys
import time
import threading

BINARY = "/Users/masdevid/.cargo/bin/mt5-quant"
TIMEOUT = 1200  # 20 min for backtest

def send(proc, msg):
    line = json.dumps(msg) + "\n"
    proc.stdin.write(line.encode())
    proc.stdin.flush()

def recv(proc, timeout=30):
    """Read one JSON-RPC response line."""
    proc.stdout.readline()  # skip blank / notification lines
    deadline = time.time() + timeout
    while time.time() < deadline:
        line = proc.stdout.readline().decode("utf-8", errors="replace").strip()
        if not line:
            continue
        try:
            return json.loads(line)
        except json.JSONDecodeError:
            pass  # skip non-JSON lines (logs etc)
    return None

def recv_with_id(proc, expected_id, timeout=1200):
    """Read lines until we get a response with the expected id."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        line = proc.stdout.readline().decode("utf-8", errors="replace").strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
            if msg.get("id") == expected_id:
                return msg
        except json.JSONDecodeError:
            pass
    return None

def tool_call(proc, call_id, name, arguments=None):
    send(proc, {
        "jsonrpc": "2.0",
        "id": call_id,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments or {}}
    })

def extract_text(resp):
    if not resp:
        return "NO RESPONSE"
    result = resp.get("result", {})
    content = result.get("content", [])
    if content:
        text = content[0].get("text", "")
        try:
            parsed = json.loads(text)
            return json.dumps(parsed, indent=2)[:1500]
        except Exception:
            return text[:1500]
    if "error" in resp:
        return f"ERROR: {resp['error']}"
    return str(resp)[:500]

def ok(resp):
    if not resp:
        return False
    result = resp.get("result", {})
    content = result.get("content", [])
    is_error = result.get("isError", True)
    if is_error:
        return False
    if content:
        text = content[0].get("text", "")
        try:
            parsed = json.loads(text)
            return parsed.get("success", True) is not False
        except Exception:
            return True
    return not is_error

# ── Start server ──────────────────────────────────────────────────────────────

print("Starting mt5-quant MCP server...")
proc = subprocess.Popen(
    [BINARY],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.DEVNULL,
)

# ── Handshake ─────────────────────────────────────────────────────────────────

send(proc, {
    "jsonrpc": "2.0", "id": 0,
    "method": "initialize",
    "params": {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "test-harness", "version": "1.0"}
    }
})
init_resp = recv_with_id(proc, 0, timeout=10)
if not init_resp:
    print("FATAL: no initialize response")
    proc.terminate()
    sys.exit(1)
print("Initialized OK")

send(proc, {"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}})

# ── Run backtest ──────────────────────────────────────────────────────────────

print("\n=== run_backtest (DPS21, XAUUSD, M5, OHLC, skip_compile) ===")
tool_call(proc, 1, "run_backtest", {
    "expert": "DPS21",
    "symbol": "XAUUSDc",
    "timeframe": "M5",
    "model": 1,            # OHLC – faster
    "skip_compile": True,
    "timeout": 900,
    "startup_delay_secs": 15,
})
resp = recv_with_id(proc, 1, timeout=TIMEOUT)
print(extract_text(resp))
backtest_ok = ok(resp)
print(f"backtest_ok={backtest_ok}")

if not backtest_ok:
    print("Backtest failed – aborting analysis tests")
    proc.terminate()
    sys.exit(1)

# ── Full analysis ─────────────────────────────────────────────────────────────

print("\n=== analyze_report (latest, all analytics) ===")
tool_call(proc, 2, "analyze_report", {})
resp = recv_with_id(proc, 2, timeout=60)
print(extract_text(resp))
print(f"analyze_report ok={ok(resp)}")

# ── Granular analytics ────────────────────────────────────────────────────────

granular = [
    ("analyze_monthly_pnl",           {}),
    ("analyze_drawdown_events",       {}),
    ("analyze_top_losses",            {"limit": 5}),
    ("analyze_loss_sequences",        {}),
    ("analyze_position_pairs",        {}),
    ("analyze_direction_bias",        {}),
    ("analyze_streaks",               {}),
    ("analyze_concurrent_peak",       {}),
    ("analyze_profit_distribution",   {}),
    ("analyze_time_performance",      {}),
    ("analyze_hold_time_distribution",{}),
    ("analyze_layer_performance",     {}),
    ("analyze_volume_vs_profit",      {}),
    ("analyze_costs",                 {}),
    ("analyze_efficiency",            {}),
    ("list_deals",                    {"limit": 5}),
    ("search_deals_by_comment",       {"query": "tp"}),
    ("search_deals_by_magic",         {"magic": "0"}),
]

results = {}
for i, (tool, args) in enumerate(granular, start=10):
    print(f"\n=== {tool} ===")
    tool_call(proc, i, tool, args)
    resp = recv_with_id(proc, i, timeout=30)
    status = "OK" if ok(resp) else "FAIL"
    results[tool] = status
    print(f"  {status}")
    if status == "FAIL":
        print("  " + extract_text(resp)[:400])

# ── Summary ───────────────────────────────────────────────────────────────────

print("\n\n========== SUMMARY ==========")
passed = [t for t, s in results.items() if s == "OK"]
failed = [t for t, s in results.items() if s != "OK"]
print(f"PASSED ({len(passed)}): {', '.join(passed)}")
if failed:
    print(f"FAILED ({len(failed)}): {', '.join(failed)}")
else:
    print("All granular analytics PASSED")

proc.terminate()
