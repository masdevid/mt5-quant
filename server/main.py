#!/usr/bin/env python3
"""
MT5-Quant MCP Server

Exposes MT5 backtest and optimization tools via the Model Context Protocol.
Run with: python3 server/main.py

Add to Claude Code:
  claude mcp add MT5-Quant -- python3 /path/to/mt5-quant/server/main.py
"""

import asyncio
import difflib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

try:
    import mcp.server.stdio
    import mcp.types as types
    from mcp.server import Server
except ImportError:
    print("ERROR: mcp package not installed. Run: pip install mcp", file=sys.stderr)
    sys.exit(1)

# Config/ROOT_DIR resolution (priority order):
#   1. MT5_MCP_HOME env var   (explicit override)
#   2. ~/.config/mt5-quant/   (installed-package default)
#   3. parent of this file    (development / run-from-repo)
def _resolve_root() -> Path:
    env_home = os.environ.get('MT5_MCP_HOME')
    if env_home:
        return Path(env_home).expanduser().resolve()
    user_cfg = Path.home() / '.config' / 'mt5-quant'
    if (user_cfg / 'config' / 'mt5-quant.yaml').exists():
        return user_cfg
    return Path(__file__).parent.parent

ROOT_DIR = _resolve_root()

# SCRIPTS_DIR always points to the package's scripts/ (adjacent to server/main.py),
# not the config dir. Scripts are never copied to ~/.config/mt5-quant.
SCRIPTS_DIR = Path(__file__).parent.parent / 'scripts'
if not SCRIPTS_DIR.exists():
    # Installed via pip — scripts landed in the package root via pyproject.toml include
    SCRIPTS_DIR = ROOT_DIR / 'scripts'

sys.path.insert(0, str(ROOT_DIR))
# Also ensure analytics imports resolve from the package dir
_pkg_dir = str(Path(__file__).parent.parent)
if _pkg_dir not in sys.path:
    sys.path.insert(0, _pkg_dir)

from analytics.extract import detect_format, parse_html, parse_xml, write_outputs
from analytics.analyze import (
    load_deals, load_metrics, monthly_pnl, reconstruct_dd_events,
    grid_depth_histogram, top_losses, loss_sequences, build_summary
)
from analytics.optimize_parser import (
    detect_format as opt_detect_format,
    parse_html as opt_parse_html,
    parse_xml as opt_parse_xml,
    normalize, convergence_analysis
)

app = Server("MT5-Quant")

# ── Config ────────────────────────────────────────────────────────────────────

def load_config() -> dict:
    config_path = ROOT_DIR / 'config' / 'mt5-quant.yaml'
    if not config_path.exists():
        return {}
    config = {}
    with open(config_path) as f:
        # Simple YAML key: value parser (no nested support needed for basic config)
        for line in f:
            line = line.strip()
            if line.startswith('#') or ':' not in line:
                continue
            key, _, val = line.partition(':')
            val = val.strip().strip('"').strip("'")
            if val and val not in ('null', '~', ''):
                config[key.strip()] = val
    return config


CONFIG = load_config()


def cfg(key: str, default: str = '') -> str:
    return CONFIG.get(key, default) or default


REPORTS_DIR = ROOT_DIR / cfg('reports_dir', 'reports')
HISTORY_FILE = ROOT_DIR / 'config' / 'backtest_history.json'
BASELINE_FILE = ROOT_DIR / 'config' / 'baseline.json'


def _validate_environment() -> dict | None:
    """Fast pre-flight check — returns error dict if environment is broken, None if OK."""
    config_path = ROOT_DIR / 'config' / 'mt5-quant.yaml'
    missing = []
    if not config_path.exists():
        missing.append('config/mt5-quant.yaml not found')
    wine = cfg('wine_executable')
    if not wine:
        missing.append('wine_executable not set in config')
    elif not os.access(wine, os.X_OK):
        missing.append(f'wine_executable not found or not executable: {wine}')
    terminal_dir = cfg('terminal_dir')
    if not terminal_dir:
        missing.append('terminal_dir not set in config')
    elif not Path(terminal_dir).is_dir():
        missing.append(f'terminal_dir not found: {terminal_dir}')
    if missing:
        return {
            'success': False,
            'error': 'SETUP_REQUIRED',
            'missing': missing,
            'hint': 'Run: bash scripts/setup.sh',
        }
    return None


def _check_symbol(symbol: str) -> tuple[str | None, list[str]]:
    """Check symbol against active server's history. Returns (warning, suggestions)."""
    terminal_dir = cfg('terminal_dir')
    if not terminal_dir:
        return None, []

    ini = _read_terminal_ini()
    active_server = ini.get('LastScanServer', '')
    bases_dir = Path(terminal_dir) / 'Bases'

    # Try active server first, then fall back to scanning all servers
    history_dir = None
    if active_server and (bases_dir / active_server / 'history').is_dir():
        history_dir = bases_dir / active_server / 'history'
    elif bases_dir.is_dir():
        for srv in bases_dir.iterdir():
            if (srv / 'history').is_dir():
                history_dir = srv / 'history'
                break

    if not history_dir:
        return None, []

    known = [d.name for d in history_dir.iterdir() if d.is_dir()]
    if not known or symbol in known:
        return None, []

    suggestions = difflib.get_close_matches(symbol, known, n=3, cutoff=0.5)

    # Check if symbol exists in any other server
    other_servers = []
    if bases_dir.is_dir():
        for srv in bases_dir.iterdir():
            if srv.name == active_server:
                continue
            if (srv / 'history' / symbol).is_dir():
                other_servers.append(srv.name)

    msg = f"Symbol '{symbol}' not found in active server '{active_server}'. Available: {', '.join(known[:6])}{'...' if len(known) > 6 else ''}"
    if other_servers:
        msg += f". Found in: {', '.join(other_servers)}"
    return msg, suggestions


# ── History helpers ───────────────────────────────────────────────────────────

def load_history() -> list[dict]:
    if not HISTORY_FILE.exists():
        return []
    with open(HISTORY_FILE) as f:
        return json.load(f)


def save_history(entries: list[dict]) -> None:
    HISTORY_FILE.parent.mkdir(exist_ok=True)
    with open(HISTORY_FILE, 'w') as f:
        json.dump(entries, f, indent=2)


def _build_history_entry(report_dir: str) -> dict | None:
    """Build a compact, self-contained history entry from a report directory."""
    from datetime import datetime, timezone
    d = Path(report_dir)
    metrics = read_json(str(d / 'metrics.json'))
    analysis = read_json(str(d / 'analysis.json'))

    if not metrics and not analysis:
        return None

    entry: dict = {
        'id': d.name,
        'report_dir': str(d),
        'report_dir_deleted': False,
        'archived_at': datetime.now(timezone.utc).isoformat(),
        'ea': metrics.get('expert') or metrics.get('ea') or '',
        'symbol': metrics.get('symbol', ''),
        'timeframe': metrics.get('timeframe', ''),
        'from_date': metrics.get('from_date') or metrics.get('testing_from', ''),
        'to_date': metrics.get('to_date') or metrics.get('testing_to', ''),
        'metrics': {
            'net_profit':      metrics.get('net_profit'),
            'profit_factor':   metrics.get('profit_factor'),
            'max_dd_pct':      metrics.get('max_dd_pct'),
            'sharpe_ratio':    metrics.get('sharpe_ratio'),
            'total_trades':    metrics.get('total_trades'),
            'recovery_factor': metrics.get('recovery_factor'),
            'win_rate_pct':    metrics.get('win_rate_pct'),
            'expected_payoff': metrics.get('expected_payoff'),
        },
        'verdict': None,
        'notes': '',
        'tags': [],
        'promoted_to_baseline': False,
    }

    if analysis:
        summary = analysis.get('summary', {})
        entry['summary'] = {k: summary.get(k) for k in (
            'green_months', 'total_months', 'worst_month', 'worst_month_pnl',
            'worst_dd_event_pct', 'max_grid_depth', 'l5_plus_count',
            'dominant_exit', 'max_win_streak', 'max_loss_streak',
            'current_streak', 'current_streak_type',
        ) if summary.get(k) is not None}
        monthly = analysis.get('monthly_pnl', [])
        if monthly:
            entry['monthly_pnl'] = monthly
        dd_events = analysis.get('dd_events', [])
        if dd_events:
            entry['worst_dd_event'] = dd_events[0]

    return entry


# ── Tool helpers ──────────────────────────────────────────────────────────────

def run_script(cmd: list[str], timeout: int = 900) -> tuple[bool, str]:
    """Run a shell script synchronously and return (success, output)."""
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            cwd=str(ROOT_DIR),
        )
        output = result.stdout + result.stderr
        return result.returncode == 0, output
    except subprocess.TimeoutExpired:
        return False, f"Timeout after {timeout}s"
    except Exception as e:
        return False, str(e)


def latest_report_dir() -> str | None:
    """Find most recently created report directory."""
    REPORTS_DIR.mkdir(exist_ok=True)
    dirs = sorted(REPORTS_DIR.iterdir(), reverse=True)
    for d in dirs:
        if d.is_dir() and not d.name.endswith('_opt'):
            return str(d)
    return None


def read_json(path: str) -> dict:
    if not os.path.exists(path):
        return {}
    with open(path) as f:
        return json.load(f)


def format_result(data: dict) -> str:
    return json.dumps(data, indent=2)


# ── Tool definitions ──────────────────────────────────────────────────────────

@app.list_tools()
async def list_tools() -> list[types.Tool]:
    return [
        types.Tool(
            name="run_backtest",
            description=(
                "Run a complete MT5 backtest pipeline: compile → clean cache → "
                "backtest → extract → analyze. Returns profit, DD%, Sharpe, monthly P/L, "
                "and drawdown event reconstruction. Always compiles and clears cache unless "
                "skip flags are set."
            ),
            inputSchema={
                "type": "object",
                "required": ["expert"],
                "properties": {
                    "expert": {
                        "type": "string",
                        "description": "EA name without path or extension. e.g. 'MyEA_v1.2'"
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Trading symbol. Use your broker's exact name. e.g. 'XAUUSD'"
                    },
                    "from_date": {
                        "type": "string",
                        "description": "Start date in YYYY.MM.DD format"
                    },
                    "to_date": {
                        "type": "string",
                        "description": "End date in YYYY.MM.DD format"
                    },
                    "preset": {
                        "type": "string",
                        "enum": ["last_month", "last_3months", "ytd", "last_year"],
                        "description": "Date preset (alternative to from/to)"
                    },
                    "timeframe": {
                        "type": "string",
                        "enum": ["M1", "M5", "M15", "M30", "H1", "H4", "D1"],
                        "description": "Chart timeframe (default: M5)"
                    },
                    "deposit": {
                        "type": "number",
                        "description": "Initial deposit (default: from config)"
                    },
                    "model": {
                        "type": "integer",
                        "enum": [0, 1, 2],
                        "description": "0=every tick (default), 1=1min OHLC, 2=open price"
                    },
                    "set_file": {
                        "type": "string",
                        "description": "Path to .set parameter file"
                    },
                    "skip_compile": {
                        "type": "boolean",
                        "description": "Skip compilation (use existing .ex5)"
                    },
                    "skip_clean": {
                        "type": "boolean",
                        "description": "Skip cache clean (faster, risks stale results)"
                    },
                    "skip_analyze": {
                        "type": "boolean",
                        "description": "Extract only, skip deal analysis"
                    },
                    "deep": {
                        "type": "boolean",
                        "description": "Run deep analysis (grid + regime breakdown)"
                    },
                    "gui": {
                        "type": "boolean",
                        "description": "Show MT5 visual mode (chart animation). Default: false (headless). Use for debugging or demo."
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Backtest timeout in seconds (default: 900)"
                    },
                    "shutdown": {
                        "type": "boolean",
                        "description": "Close MT5 after backtest completes. Default: false — MT5 stays open and report is detected via file watching. Set true for CI/headless environments."
                    },
                    "kill_existing": {
                        "type": "boolean",
                        "description": "Kill a running MT5 instance before launching. REQUIRED when MT5 is already open — Wine does not support single-instance config passthrough. With shutdown=false (default), MT5 restarts, runs the backtest, then stays open so results are visible in the GUI."
                    },
                },
            },
        ),
        types.Tool(
            name="run_optimization",
            description=(
                "Launch MT5 genetic parameter optimization as a detached background process. "
                "Returns immediately — MT5 runs for 2-6 hours. "
                "Always uses model=0 (every tick). Call get_optimization_results only after "
                "user confirms MT5 has finished."
            ),
            inputSchema={
                "type": "object",
                "required": ["expert", "set_file", "from_date", "to_date"],
                "properties": {
                    "expert": {"type": "string"},
                    "set_file": {
                        "type": "string",
                        "description": "Path to optimization .set file with ||Y sweep flags"
                    },
                    "from_date": {"type": "string"},
                    "to_date": {"type": "string"},
                    "symbol": {"type": "string"},
                    "deposit": {"type": "number"},
                },
            },
        ),
        types.Tool(
            name="get_optimization_results",
            description=(
                "Parse completed MT5 optimization results. Call only after user signals "
                "that MT5 optimization has finished. Returns top passes sorted by profit "
                "with convergence analysis."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "job_id": {
                        "type": "string",
                        "description": "Job ID from run_optimization response"
                    },
                    "report_file": {
                        "type": "string",
                        "description": "Direct path to optimization.htm or .htm.xml"
                    },
                    "top_n": {
                        "type": "integer",
                        "description": "Number of top results to return (default: 20)"
                    },
                    "dd_threshold": {
                        "type": "number",
                        "description": "Flag results above this DD% as high-risk (default: 20)"
                    },
                },
            },
        ),
        types.Tool(
            name="analyze_report",
            description=(
                "Read and summarize a completed backtest report. Does not re-run MT5. "
                "Returns monthly P/L, drawdown events, grid depth histogram, and top losses."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "report_dir": {
                        "type": "string",
                        "description": "Path to report directory. If omitted, uses latest."
                    },
                },
            },
        ),
        types.Tool(
            name="compare_baseline",
            description=(
                "Compare a backtest report against a baseline. Returns winner/loser verdict "
                "and delta metrics. Baseline must include net_profit and max_dd_pct."
            ),
            inputSchema={
                "type": "object",
                "required": ["baseline"],
                "properties": {
                    "report_dir": {
                        "type": "string",
                        "description": "Report to evaluate. If omitted, uses latest."
                    },
                    "baseline": {
                        "type": "object",
                        "required": ["net_profit", "max_dd_pct"],
                        "properties": {
                            "net_profit": {"type": "number"},
                            "max_dd_pct": {"type": "number"},
                            "total_trades": {"type": "integer"},
                            "label": {"type": "string"},
                        },
                    },
                    "promote_dd_limit": {
                        "type": "number",
                        "description": "Auto-promote only if DD < this % (default: 20)"
                    },
                },
            },
        ),
        types.Tool(
            name="compile_ea",
            description="Compile an MQL5 Expert Advisor via MetaEditor (Wine/CrossOver).",
            inputSchema={
                "type": "object",
                "required": ["expert_path"],
                "properties": {
                    "expert_path": {
                        "type": "string",
                        "description": "Path to .mq5 source file"
                    },
                },
            },
        ),
        types.Tool(
            name="verify_setup",
            description=(
                "Verify the MT5-Quant environment without launching MT5. "
                "Checks Wine executable, MT5 installation paths, and config file. "
                "Run this first if other tools return SETUP_REQUIRED errors."
            ),
            inputSchema={"type": "object", "properties": {}},
        ),
        types.Tool(
            name="list_symbols",
            description=(
                "Detect the active MT5 broker session and list symbols that have local "
                "tick history available for backtesting. Also shows all broker servers "
                "found in the MT5 installation. Use this before run_backtest to confirm "
                "the correct symbol name for the connected broker."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "server": {
                        "type": "string",
                        "description": "Filter to a specific server name. If omitted, shows active server and all servers.",
                    },
                },
            },
        ),
        types.Tool(
            name="list_experts",
            description=(
                "List all compiled Expert Advisors (.ex5 files) found in the MT5 Experts "
                "directory, including those inside sub-folders. Returns the expert name to "
                "use in run_backtest and the sub-folder path if applicable."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "filter": {
                        "type": "string",
                        "description": "Optional substring filter on EA name (case-insensitive).",
                    },
                },
            },
        ),
        types.Tool(
            name="get_backtest_status",
            description=(
                "Check progress of a running or recently completed backtest pipeline. "
                "Returns current stage (COMPILE/CLEAN/BACKTEST/EXTRACT/ANALYZE/DONE), "
                "elapsed time, and whether the pipeline has finished."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "report_dir": {
                        "type": "string",
                        "description": "Report directory path. If omitted, uses latest.",
                    },
                },
            },
        ),
        types.Tool(
            name="get_optimization_status",
            description=(
                "Check whether a background optimization job is still running. "
                "Returns process alive status, elapsed time, last 20 log lines, "
                "and whether the report file has appeared (definitive completion signal)."
            ),
            inputSchema={
                "type": "object",
                "required": ["job_id"],
                "properties": {
                    "job_id": {
                        "type": "string",
                        "description": "Job ID returned by run_optimization.",
                    },
                },
            },
        ),
        types.Tool(
            name="prune_reports",
            description=(
                "Delete old backtest report directories, keeping only the N most recent. "
                "Optimization result directories (_opt suffix) are never deleted."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "keep_last": {
                        "type": "integer",
                        "description": "Number of most recent reports to keep (default: 20).",
                    },
                },
            },
        ),
        types.Tool(
            name="list_reports",
            description=(
                "List all backtest report directories with compact key metrics "
                "(profit, DD%, trades, date). Much cheaper than calling analyze_report "
                "repeatedly. Use this to survey what runs exist before drilling in."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "include_opt": {
                        "type": "boolean",
                        "description": "Include optimization result dirs (_opt suffix). Default: false.",
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max reports to return, newest first (default: 30).",
                    },
                },
            },
        ),
        types.Tool(
            name="tail_log",
            description=(
                "Read the last N lines of a backtest progress log or optimization log. "
                "Use filter='errors' to get only ERROR/WARN lines. Cheaper than "
                "get_optimization_status when you only need log content."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "report_dir": {
                        "type": "string",
                        "description": "Backtest report dir (reads progress.log). Omit for latest.",
                    },
                    "job_id": {
                        "type": "string",
                        "description": "Optimization job ID (reads its log file).",
                    },
                    "log_file": {
                        "type": "string",
                        "description": "Absolute path to any log file.",
                    },
                    "n": {
                        "type": "integer",
                        "description": "Number of lines to return (default: 50).",
                    },
                    "filter": {
                        "type": "string",
                        "enum": ["all", "errors", "warnings"],
                        "description": "Line filter (default: all).",
                    },
                },
            },
        ),
        types.Tool(
            name="cache_status",
            description=(
                "Show MT5 tester cache size breakdown by symbol/timeframe directory. "
                "Call before clean_cache to understand what will be deleted."
            ),
            inputSchema={"type": "object", "properties": {}},
        ),
        types.Tool(
            name="clean_cache",
            description=(
                "Delete MT5 tester cache files to force fresh price data on next backtest. "
                "Optionally target a specific symbol. Returns bytes freed."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Delete only cache for this symbol. Omit to delete all.",
                    },
                    "dry_run": {
                        "type": "boolean",
                        "description": "Report what would be deleted without deleting. Default: false.",
                    },
                },
            },
        ),
        types.Tool(
            name="read_set_file",
            description=(
                "Parse an MT5 .set parameter file (UTF-16LE or UTF-8) into structured JSON. "
                "Returns each parameter with its value and optimization sweep config. "
                "Use this instead of reading raw .set files."
            ),
            inputSchema={
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to .set file.",
                    },
                },
            },
        ),
        types.Tool(
            name="write_set_file",
            description=(
                "Write an MT5 .set parameter file in UTF-16LE encoding (required by MT5). "
                "Accepts a dict of params. For optimization sweeps include from/to/step keys. "
                "Existing file is overwritten and chmod 444 is applied."
            ),
            inputSchema={
                "type": "object",
                "required": ["path", "params"],
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Output path for .set file.",
                    },
                    "params": {
                        "type": "object",
                        "description": (
                            "Dict of param_name → value or dict with keys: "
                            "value, from, to, step, optimize (bool)."
                        ),
                    },
                },
            },
        ),
        types.Tool(
            name="patch_set_file",
            description=(
                "Modify specific parameters in an existing .set file in-place. "
                "Preserves all other params, comments, and sweep config. "
                "Returns a diff of what changed. "
                "Use instead of read_set_file → edit → write_set_file (saves 2 round-trips)."
            ),
            inputSchema={
                "type": "object",
                "required": ["path", "patches"],
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the .set file to modify.",
                    },
                    "patches": {
                        "type": "object",
                        "description": (
                            "Params to update. Each key is a param name. "
                            "Value can be a scalar (just updates value) or a dict "
                            "with keys: value, from, to, step, optimize."
                        ),
                    },
                },
            },
        ),
        types.Tool(
            name="clone_set_file",
            description=(
                "Copy a .set file to a new path, applying optional overrides. "
                "One call instead of read → modify → write. "
                "Useful for creating variant .set files from a base config."
            ),
            inputSchema={
                "type": "object",
                "required": ["source", "destination"],
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Path to source .set file.",
                    },
                    "destination": {
                        "type": "string",
                        "description": "Output path for the cloned .set file.",
                    },
                    "overrides": {
                        "type": "object",
                        "description": (
                            "Optional param overrides to apply in the clone. "
                            "Same format as patch_set_file patches."
                        ),
                    },
                },
            },
        ),
        types.Tool(
            name="set_from_optimization",
            description=(
                "Generate a .set file directly from an optimization result's params dict. "
                "Strips all sweep flags (||Y) to produce a clean backtest .set. "
                "Optionally uses a template .set for params not in the optimization result. "
                "Optionally re-adds sweep ranges to selected params for follow-on optimization. "
                "Use immediately after get_optimization_results — params dict comes from results[0].params."
            ),
            inputSchema={
                "type": "object",
                "required": ["path", "params"],
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Output path for the generated .set file.",
                    },
                    "params": {
                        "type": "object",
                        "description": (
                            "Flat dict of param_name → value from optimization result. "
                            "e.g. {'TP_Pips': 400, 'Min_Confidence': 0.61}"
                        ),
                    },
                    "template": {
                        "type": "string",
                        "description": (
                            "Optional path to an existing .set file. "
                            "Params not in 'params' are filled from the template as fixed values."
                        ),
                    },
                    "sweep": {
                        "type": "object",
                        "description": (
                            "Optional: re-add sweep ranges to specific params after applying opt values. "
                            "Dict of param_name → {from, to, step, optimize: true}. "
                            "Use to create a narrowed follow-on optimization .set."
                        ),
                    },
                },
            },
        ),
        types.Tool(
            name="diff_set_files",
            description=(
                "Compare two .set files and return only the differences: "
                "params added, removed, or changed (value or sweep flag). "
                "Use instead of reading both files and comparing manually."
            ),
            inputSchema={
                "type": "object",
                "required": ["path_a", "path_b"],
                "properties": {
                    "path_a": {"type": "string", "description": "First .set file (baseline/old)."},
                    "path_b": {"type": "string", "description": "Second .set file (candidate/new)."},
                },
            },
        ),
        types.Tool(
            name="describe_sweep",
            description=(
                "Show a .set file's sweep configuration: which params are swept, "
                "their ranges, value counts, and total optimization combinations. "
                "Use before run_optimization to verify scope."
            ),
            inputSchema={
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": {"type": "string", "description": "Path to .set file."},
                },
            },
        ),
        types.Tool(
            name="list_set_files",
            description=(
                "List all .set files in the MT5 tester profiles directory with "
                "param counts, swept param counts, and total optimization combinations. "
                "Use instead of reading each file individually to find the right .set."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "ea": {
                        "type": "string",
                        "description": "Filter by EA name substring (case-insensitive).",
                    },
                },
            },
        ),
        types.Tool(
            name="list_jobs",
            description=(
                "List all optimization jobs with compact status (alive/done/failed, elapsed). "
                "Cheaper than calling get_optimization_status for each job individually."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "include_done": {
                        "type": "boolean",
                        "description": "Include completed jobs (default: true).",
                    },
                },
            },
        ),
        types.Tool(
            name="archive_report",
            description=(
                "Convert a backtest report directory into a compact JSON entry appended to "
                "config/backtest_history.json. Captures all metrics, analysis summary, monthly P/L, "
                "and worst DD event. Optionally deletes the source directory to reclaim disk space. "
                "Skips if the report is already in history (idempotent)."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "report_dir": {
                        "type": "string",
                        "description": "Report directory to archive. If omitted, uses latest.",
                    },
                    "delete_after": {
                        "type": "boolean",
                        "description": "Delete source directory after archiving (default: false).",
                    },
                    "verdict": {
                        "type": "string",
                        "enum": ["winner", "loser", "marginal", "reference"],
                        "description": "Optional verdict to attach to the entry.",
                    },
                    "notes": {
                        "type": "string",
                        "description": "Free-text notes to attach to the entry.",
                    },
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tags to attach (e.g. ['tight-sl', 'new-entry-filter']).",
                    },
                },
            },
        ),
        types.Tool(
            name="archive_all_reports",
            description=(
                "Bulk-archive all backtest report directories into config/backtest_history.json, "
                "then optionally delete the source directories. Skips dirs already in history. "
                "Optimization dirs (_opt suffix) are never deleted. "
                "Use this to clean up disk space while preserving all results as JSON."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "delete_after": {
                        "type": "boolean",
                        "description": "Delete source directories after archiving (default: false).",
                    },
                    "keep_last": {
                        "type": "integer",
                        "description": "Keep this many newest dirs even if delete_after=true (default: 5).",
                    },
                    "dry_run": {
                        "type": "boolean",
                        "description": "Report what would happen without making changes (default: false).",
                    },
                },
            },
        ),
        types.Tool(
            name="get_history",
            description=(
                "Query config/backtest_history.json with filters. Returns compact entries sorted "
                "newest-first by default. Use this to compare past runs, find regressions, or "
                "pick a candidate to promote to baseline."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "ea": {
                        "type": "string",
                        "description": "Filter by EA name (substring match).",
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Filter by symbol (exact match).",
                    },
                    "verdict": {
                        "type": "string",
                        "enum": ["winner", "loser", "marginal", "reference"],
                        "description": "Filter by verdict.",
                    },
                    "tag": {
                        "type": "string",
                        "description": "Filter entries that contain this tag.",
                    },
                    "min_profit": {
                        "type": "number",
                        "description": "Filter entries with net_profit >= this value.",
                    },
                    "max_dd_pct": {
                        "type": "number",
                        "description": "Filter entries with max_dd_pct <= this value.",
                    },
                    "sort_by": {
                        "type": "string",
                        "enum": ["date", "profit", "dd", "sharpe"],
                        "description": "Sort order (default: date, newest first).",
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max entries to return (default: 20).",
                    },
                    "include_monthly": {
                        "type": "boolean",
                        "description": "Include monthly_pnl arrays (default: false, saves tokens).",
                    },
                },
            },
        ),
        types.Tool(
            name="promote_to_baseline",
            description=(
                "Promote a backtest result to config/baseline.json — the reference used by "
                "compare_baseline and the Claude Code baseline hook. "
                "Accepts a history entry id, a report_dir, or defaults to the latest report. "
                "Also marks the history entry as promoted."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "Entry id from get_history (report dir basename).",
                    },
                    "report_dir": {
                        "type": "string",
                        "description": "Direct path to report directory (alternative to history_id).",
                    },
                    "notes": {
                        "type": "string",
                        "description": "Notes written to baseline.json (e.g. 'v1.3 promoted after 3-month walk-forward').",
                    },
                },
            },
        ),
        types.Tool(
            name="annotate_history",
            description=(
                "Add or update notes, verdict, or tags on a history entry in "
                "config/backtest_history.json. Use this after compare_baseline to record "
                "the verdict, or to tag runs for later retrieval."
            ),
            inputSchema={
                "type": "object",
                "required": ["history_id"],
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "Entry id (report dir basename) to update.",
                    },
                    "verdict": {
                        "type": "string",
                        "enum": ["winner", "loser", "marginal", "reference"],
                    },
                    "notes": {
                        "type": "string",
                        "description": "Free-text notes (replaces existing notes).",
                    },
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tags to set (replaces existing tags).",
                    },
                    "add_tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tags to append without replacing existing ones.",
                    },
                },
            },
        ),
    ]


# ── Tool handlers ─────────────────────────────────────────────────────────────

@app.call_tool()
async def call_tool(name: str, arguments: dict[str, Any]) -> list[types.TextContent]:
    try:
        if name == "run_backtest":
            result = await handle_run_backtest(arguments)
        elif name == "run_optimization":
            result = await handle_run_optimization(arguments)
        elif name == "get_optimization_results":
            result = await handle_get_optimization_results(arguments)
        elif name == "analyze_report":
            result = await handle_analyze_report(arguments)
        elif name == "compare_baseline":
            result = await handle_compare_baseline(arguments)
        elif name == "compile_ea":
            result = await handle_compile_ea(arguments)
        elif name == "list_symbols":
            result = await handle_list_symbols(arguments)
        elif name == "list_experts":
            result = await handle_list_experts(arguments)
        elif name == "verify_setup":
            result = await handle_verify_setup(arguments)
        elif name == "get_backtest_status":
            result = await handle_get_backtest_status(arguments)
        elif name == "get_optimization_status":
            result = await handle_get_optimization_status(arguments)
        elif name == "prune_reports":
            result = await handle_prune_reports(arguments)
        elif name == "list_reports":
            result = await handle_list_reports(arguments)
        elif name == "tail_log":
            result = await handle_tail_log(arguments)
        elif name == "cache_status":
            result = await handle_cache_status(arguments)
        elif name == "clean_cache":
            result = await handle_clean_cache(arguments)
        elif name == "read_set_file":
            result = await handle_read_set_file(arguments)
        elif name == "write_set_file":
            result = await handle_write_set_file(arguments)
        elif name == "patch_set_file":
            result = await handle_patch_set_file(arguments)
        elif name == "clone_set_file":
            result = await handle_clone_set_file(arguments)
        elif name == "set_from_optimization":
            result = await handle_set_from_optimization(arguments)
        elif name == "diff_set_files":
            result = await handle_diff_set_files(arguments)
        elif name == "describe_sweep":
            result = await handle_describe_sweep(arguments)
        elif name == "list_set_files":
            result = await handle_list_set_files(arguments)
        elif name == "list_jobs":
            result = await handle_list_jobs(arguments)
        elif name == "archive_report":
            result = await handle_archive_report(arguments)
        elif name == "archive_all_reports":
            result = await handle_archive_all_reports(arguments)
        elif name == "get_history":
            result = await handle_get_history(arguments)
        elif name == "promote_to_baseline":
            result = await handle_promote_to_baseline(arguments)
        elif name == "annotate_history":
            result = await handle_annotate_history(arguments)
        else:
            result = {"error": f"Unknown tool: {name}"}
    except Exception as e:
        result = {"error": str(e), "success": False}

    return [types.TextContent(type="text", text=format_result(result))]


async def handle_run_backtest(args: dict) -> dict:
    env_error = _validate_environment()
    if env_error:
        return env_error

    symbol = args.get('symbol') or cfg('backtest_symbol', 'XAUUSD')
    symbol_warning, symbol_suggestions = _check_symbol(symbol)

    cmd = [str(SCRIPTS_DIR / 'backtest_pipeline.sh')]
    cmd += ['--expert', args['expert']]
    project_dir = cfg('project_dir', '')
    if project_dir:
        cmd += ['--project-dir', project_dir]

    if 'symbol' in args:
        cmd += ['--symbol', args['symbol']]
    if 'preset' in args:
        cmd += ['--preset', args['preset']]
    if 'from_date' in args:
        cmd += ['--from', args['from_date']]
    if 'to_date' in args:
        cmd += ['--to', args['to_date']]
    if 'timeframe' in args:
        cmd += ['--timeframe', args['timeframe']]
    if 'deposit' in args:
        cmd += ['--deposit', str(args['deposit'])]
    if 'model' in args:
        cmd += ['--model', str(args['model'])]
    if 'set_file' in args:
        set_file = args['set_file']
        # Resolve relative paths against project_dir (where the EA repo lives)
        if project_dir and not os.path.isabs(set_file):
            set_file = os.path.join(project_dir, set_file)
        cmd += ['--set', set_file]
    if args.get('skip_compile'):
        cmd.append('--skip-compile')
    if args.get('skip_clean'):
        cmd.append('--skip-clean')
    if args.get('skip_analyze'):
        cmd.append('--skip-analyze')
    if args.get('deep'):
        cmd.append('--deep')
    if args.get('gui'):
        cmd.append('--gui')
    if args.get('shutdown'):
        cmd.append('--shutdown')
    if args.get('kill_existing'):
        cmd.append('--kill-existing')

    timeout = args.get('timeout', 900)
    success, output = run_script(cmd, timeout=timeout)

    if not success:
        return {'success': False, 'error': output[-2000:]}  # last 2k chars

    # Parse report dir from pipeline output (reliable, avoids stale REPORTS_DIR at startup)
    report_dir = None
    for line in output.splitlines():
        if line.strip().startswith('Report:') or ' Report: ' in line:
            parts = line.split('Report:', 1)
            if len(parts) == 2:
                candidate = parts[1].strip()
                if os.path.isdir(candidate):
                    report_dir = candidate
                    break
    if not report_dir:
        report_dir = latest_report_dir()
    if not report_dir:
        return {'success': False, 'error': 'Pipeline completed but no report directory found'}

    metrics = read_json(os.path.join(report_dir, 'metrics.json'))
    analysis = read_json(os.path.join(report_dir, 'analysis.json'))

    result = {
        'success': True,
        'report_dir': report_dir,
        'metrics': metrics,
        'analysis_summary': analysis.get('summary', {}),
        'worst_dd_event': analysis.get('dd_events', [{}])[0] if analysis.get('dd_events') else None,
        'monthly_pnl': analysis.get('monthly_pnl', []),
        'grid_depth_histogram': analysis.get('grid_depth_histogram', {}),
        'output': output[-1000:],
    }
    if symbol_warning:
        result['symbol_warning'] = symbol_warning
        result['symbol_suggestions'] = symbol_suggestions
    return result


async def handle_run_optimization(args: dict) -> dict:
    env_error = _validate_environment()
    if env_error:
        return env_error

    cmd = [str(SCRIPTS_DIR / 'optimize.sh')]
    cmd += ['--expert', args['expert']]
    cmd += ['--set', args['set_file']]
    cmd += ['--from', args['from_date']]
    cmd += ['--to', args['to_date']]

    if 'symbol' in args:
        cmd += ['--symbol', args['symbol']]
    if 'deposit' in args:
        cmd += ['--deposit', str(args['deposit'])]

    success, output = run_script(cmd, timeout=60)  # script returns quickly (nohup)

    # Extract job ID from output
    import re
    job_match = re.search(r'opt_\d{8}_\d{6}', output)
    job_id = job_match.group(0) if job_match else None

    return {
        'success': success,
        'job_id': job_id,
        'message': 'Optimization launched in background. Do NOT poll. Signal me when MT5 completes.',
        'output': output[-500:],
    }


async def handle_get_optimization_results(args: dict) -> dict:
    from analytics.optimize_parser import find_report as find_opt_report

    # Locate report
    report_path = None
    if 'report_file' in args:
        report_path = args['report_file']
    elif 'job_id' in args:
        try:
            report_path = find_opt_report(args['job_id'])
        except FileNotFoundError as e:
            return {'success': False, 'error': str(e)}

    if not report_path or not os.path.exists(report_path):
        return {'success': False, 'error': 'Report not found. Is optimization still running?'}

    fmt = opt_detect_format(report_path)
    if fmt == 'xml':
        raw = opt_parse_xml(report_path)
    else:
        raw = opt_parse_html(report_path)

    results = normalize(raw)
    results.sort(key=lambda r: r.get('net_profit', 0), reverse=True)

    top_n = args.get('top_n', 20)
    dd_threshold = args.get('dd_threshold', 20.0)
    conv = convergence_analysis(results, top_n=10)

    # Flag high-risk results
    for r in results:
        r['high_risk'] = r.get('max_dd_pct', 0) > dd_threshold

    return {
        'success': True,
        'total_passes': len(results),
        'results': results[:top_n],
        'convergence': conv,
        'recommendation': _opt_recommendation(results, dd_threshold),
    }


def _opt_recommendation(results: list[dict], dd_threshold: float) -> dict:
    safe = [r for r in results if r.get('max_dd_pct', 999) < dd_threshold]
    if not safe:
        return {
            'verdict': 'all_high_risk',
            'message': f'All top results exceed DD threshold ({dd_threshold}%). Widen parameter ranges or increase DD threshold.',
        }
    best = safe[0]
    return {
        'verdict': 'verify_model0' if best.get('model', 0) != 0 else 'promote_candidate',
        'best_params': best.get('params', {}),
        'net_profit': best.get('net_profit', 0),
        'max_dd_pct': best.get('max_dd_pct', 0),
        'message': f"Run verification backtest with these params before promoting.",
    }


async def handle_analyze_report(args: dict) -> dict:
    report_dir = args.get('report_dir') or latest_report_dir()
    if not report_dir:
        return {'success': False, 'error': 'No report directory found'}

    metrics = read_json(os.path.join(report_dir, 'metrics.json'))
    analysis = read_json(os.path.join(report_dir, 'analysis.json'))

    if not metrics and not analysis:
        # Try re-running analysis on deals.csv
        deals_csv = os.path.join(report_dir, 'deals.csv')
        if os.path.exists(deals_csv):
            deals = load_deals(deals_csv)
            monthly = monthly_pnl(deals)
            dd_events = reconstruct_dd_events(deals, metrics)
            analysis = {
                'summary': build_summary(metrics, monthly, dd_events),
                'monthly_pnl': monthly,
                'dd_events': dd_events,
                'grid_depth_histogram': grid_depth_histogram(deals),
                'top_losses': top_losses(deals),
                'loss_sequences': loss_sequences(deals),
            }
        else:
            return {'success': False, 'error': f'No data found in {report_dir}'}

    return {
        'success': True,
        'report_dir': report_dir,
        'metrics': metrics,
        **analysis,
    }


async def handle_compare_baseline(args: dict) -> dict:
    report_dir = args.get('report_dir') or latest_report_dir()
    if not report_dir:
        return {'success': False, 'error': 'No report directory found'}

    metrics = read_json(os.path.join(report_dir, 'metrics.json'))
    baseline = args['baseline']
    dd_limit = args.get('promote_dd_limit', 20.0)

    candidate_profit = metrics.get('net_profit', 0)
    candidate_dd = metrics.get('max_dd_pct', 999)
    baseline_profit = baseline['net_profit']
    baseline_dd = baseline['max_dd_pct']

    profit_delta = candidate_profit - baseline_profit
    dd_delta = candidate_dd - baseline_dd
    profit_pct = (profit_delta / baseline_profit * 100) if baseline_profit else 0

    is_winner = candidate_profit > baseline_profit and candidate_dd < dd_limit

    if is_winner:
        verdict = 'winner'
    elif candidate_profit > baseline_profit:
        verdict = 'marginal'  # Better profit but DD too high
    else:
        verdict = 'loser'

    sign = '+' if profit_delta >= 0 else ''
    dd_sign = '+' if dd_delta >= 0 else ''

    return {
        'success': True,
        'verdict': verdict,
        'auto_promote': is_winner,
        'delta': {
            'profit_usd': round(profit_delta, 2),
            'profit_pct': round(profit_pct, 1),
            'dd_pp': round(dd_delta, 2),
        },
        'summary': (
            f"{sign}${profit_delta:,.2f} ({sign}{profit_pct:.1f}%) profit vs {baseline.get('label', 'baseline')}. "
            f"DD: {candidate_dd:.2f}% vs {baseline_dd:.2f}% ({dd_sign}{dd_delta:.2f}pp). "
            f"{'Auto-promoting.' if is_winner else 'Not promoting (DD too high).' if verdict == 'marginal' else 'Regression.'}"
        ),
        'candidate': {
            'net_profit': candidate_profit,
            'max_dd_pct': candidate_dd,
            'total_trades': metrics.get('total_trades', 0),
        },
        'baseline': baseline,
    }


async def handle_compile_ea(args: dict) -> dict:
    env_error = _validate_environment()
    if env_error:
        return env_error

    expert_path = args['expert_path']
    cmd = [str(SCRIPTS_DIR / 'mqlcompile.sh'), expert_path]
    success, output = run_script(cmd, timeout=120)

    return {
        'success': success,
        'output': output,
        'expert_path': expert_path,
    }


def _read_terminal_ini() -> dict:
    """Parse terminal.ini (UTF-16LE or UTF-8) into a flat key→value dict."""
    terminal_dir = cfg('terminal_dir')
    if not terminal_dir:
        return {}
    ini_path = Path(terminal_dir) / 'config' / 'terminal.ini'
    if not ini_path.exists():
        return {}
    try:
        raw = ini_path.read_bytes()
        text = raw.decode('utf-16') if raw[:2] in (b'\xff\xfe', b'\xfe\xff') else raw.decode('utf-8', errors='replace')
    except Exception:
        return {}
    result: dict = {}
    for line in text.splitlines():
        line = line.strip()
        if '=' in line and not line.startswith(';') and not line.startswith('['):
            k, _, v = line.partition('=')
            result[k.strip()] = v.strip()
    return result


async def handle_list_symbols(args: dict) -> dict:
    env_error = _validate_environment()
    if env_error:
        return env_error

    terminal_dir = cfg('terminal_dir')
    bases_dir = Path(terminal_dir) / 'Bases'

    if not bases_dir.is_dir():
        return {'success': False, 'error': f'Bases directory not found: {bases_dir}'}

    # Detect active server from terminal.ini
    ini = _read_terminal_ini()
    active_server = ini.get('LastScanServer', '')
    opt_mode = ini.get('OptMode', '0')

    # Collect all servers and their symbols
    filter_server = args.get('server', '').lower()
    servers: list[dict] = []
    for server_dir in sorted(bases_dir.iterdir()):
        if not server_dir.is_dir():
            continue
        name = server_dir.name
        if filter_server and filter_server not in name.lower():
            continue
        history_dir = server_dir / 'history'
        symbols = sorted(d.name for d in history_dir.iterdir() if d.is_dir()) if history_dir.is_dir() else []
        servers.append({
            'server': name,
            'active': name == active_server,
            'symbol_count': len(symbols),
            'symbols': symbols,
        })

    # Put active server first
    servers.sort(key=lambda s: (0 if s['active'] else 1, s['server']))

    warnings = []
    if opt_mode == '-1':
        warnings.append('OptMode=-1 detected in terminal.ini — the CLEAN stage will reset this before backtest.')

    return {
        'success': True,
        'active_server': active_server or '(unknown — open MT5 to connect)',
        'servers': servers,
        'warnings': warnings,
        'hint': 'Use the symbol name exactly as shown (e.g. "XAUUSD.cent" not "XAUUSD") in run_backtest.',
    }


async def handle_list_experts(args: dict) -> dict:
    env_error = _validate_environment()
    if env_error:
        return env_error

    terminal_dir = cfg('terminal_dir')
    experts_root = Path(cfg('experts_dir') or os.path.join(terminal_dir, 'MQL5', 'Experts'))

    if not experts_root.is_dir():
        return {'success': False, 'error': f'Experts directory not found: {experts_root}'}

    name_filter = args.get('filter', '').lower()
    experts: list[dict] = []

    for ex5 in sorted(experts_root.rglob('*.ex5')):
        rel = ex5.relative_to(experts_root)
        expert_name = ex5.stem          # filename without .ex5
        subfolder = str(rel.parent) if rel.parent != Path('.') else ''
        if name_filter and name_filter not in expert_name.lower():
            continue
        experts.append({
            'name': expert_name,
            'subfolder': subfolder,
            'run_backtest_expert': f'{subfolder}/{expert_name}' if subfolder else expert_name,
            'path': str(ex5),
        })

    return {
        'success': True,
        'count': len(experts),
        'experts_root': str(experts_root),
        'experts': experts,
        'hint': 'Use the "run_backtest_expert" value as the expert parameter in run_backtest.',
    }


async def handle_verify_setup(args: dict) -> dict:
    checks: dict = {}
    all_ok = True

    # Config file
    config_path = ROOT_DIR / 'config' / 'mt5-quant.yaml'
    checks['config_file'] = {
        'ok': config_path.exists(),
        'detail': str(config_path) if config_path.exists() else 'Not found — run: bash scripts/setup.sh',
    }
    if not config_path.exists():
        all_ok = False

    # Wine executable
    wine = cfg('wine_executable')
    if not wine:
        checks['wine_executable'] = {'ok': False, 'detail': 'Not configured in mt5-quant.yaml'}
        all_ok = False
    else:
        executable = os.access(wine, os.X_OK)
        version = ''
        if executable:
            try:
                r = subprocess.run([wine, '--version'], capture_output=True, text=True, timeout=5)
                version = ((r.stdout or '') + (r.stderr or '')).strip().splitlines()[0]
            except Exception as e:
                version = f'error: {e}'
        checks['wine_executable'] = {
            'ok': executable,
            'version': version,
            'detail': wine if executable else f'Not executable: {wine}',
        }
        if not executable:
            all_ok = False

    # terminal_dir and derived paths
    terminal_dir = cfg('terminal_dir')
    if not terminal_dir:
        checks['terminal_dir'] = {'ok': False, 'detail': 'Not configured in mt5-quant.yaml'}
        all_ok = False
    else:
        td_ok = Path(terminal_dir).is_dir()
        checks['terminal_dir'] = {
            'ok': td_ok,
            'detail': terminal_dir if td_ok else f'Directory not found: {terminal_dir}',
        }
        if not td_ok:
            all_ok = False

        terminal_exe = Path(terminal_dir) / 'terminal64.exe'
        checks['terminal64_exe'] = {
            'ok': terminal_exe.exists(),
            'detail': str(terminal_exe) if terminal_exe.exists()
                      else 'Not found — launch MT5 once to unpack it',
        }

        experts_dir = Path(cfg('experts_dir') or os.path.join(terminal_dir, 'MQL5', 'Experts'))
        ea_count = len(list(experts_dir.glob('*.ex5'))) if experts_dir.is_dir() else 0
        checks['experts_dir'] = {
            'ok': experts_dir.is_dir(),
            'detail': f'{ea_count} .ex5 file(s)' if experts_dir.is_dir()
                      else f'Not found (will be created on first EA compile): {experts_dir}',
        }

        tester_dir = Path(cfg('tester_profiles_dir') or os.path.join(terminal_dir, 'MQL5', 'Profiles', 'Tester'))
        set_count = len(list(tester_dir.glob('*.set'))) if tester_dir.is_dir() else 0
        checks['tester_profiles_dir'] = {
            'ok': tester_dir.is_dir(),
            'detail': f'{set_count} .set file(s)' if tester_dir.is_dir()
                      else f'Not found (will be created on first backtest): {tester_dir}',
        }

        cache_dir = Path(cfg('tester_cache_dir') or os.path.join(terminal_dir, 'Tester'))
        checks['tester_cache_dir'] = {
            'ok': cache_dir.is_dir(),
            'detail': str(cache_dir) if cache_dir.is_dir() else f'Not found: {cache_dir}',
        }

    return {
        'all_ok': all_ok,
        'checks': checks,
        'hint': 'Run: bash scripts/setup.sh' if not all_ok else 'Environment looks good.',
    }


async def handle_get_backtest_status(args: dict) -> dict:
    report_dir = args.get('report_dir') or latest_report_dir()
    if not report_dir:
        return {'success': False, 'error': 'No report directory found'}

    progress_log = Path(report_dir) / 'progress.log'
    pipeline_meta = Path(report_dir) / 'pipeline_metadata.json'

    stages = []
    if progress_log.exists():
        for line in progress_log.read_text().splitlines():
            parts = line.split()
            if len(parts) >= 3:
                stages.append({'stage': parts[0], 'timestamp': parts[1], 'elapsed': parts[2]})

    current_stage = stages[-1]['stage'] if stages else 'UNKNOWN'
    finished = pipeline_meta.exists() or current_stage == 'DONE'
    elapsed = None
    if stages:
        try:
            elapsed = int(stages[-1]['elapsed'].replace('elapsed=', '').rstrip('s'))
        except (ValueError, AttributeError):
            pass

    return {
        'success': True,
        'report_dir': report_dir,
        'current_stage': current_stage,
        'elapsed_seconds': elapsed,
        'finished': finished,
        'stages': stages,
    }


async def handle_get_optimization_status(args: dict) -> dict:
    job_id = args['job_id']
    meta_path = ROOT_DIR / '.mt5mcp_jobs' / f'{job_id}.json'

    if not meta_path.exists():
        return {'success': False, 'error': f'Job not found: {job_id}. Check .mt5mcp_jobs/'}

    with open(meta_path) as f:
        meta = json.load(f)

    pid = meta.get('pid')
    log_file = meta.get('log_file', '')
    wine_prefix = meta.get('wine_prefix', '')
    started_at = meta.get('started_at', '')

    # Check process alive via kill -0
    alive = False
    if pid:
        try:
            os.kill(int(pid), 0)
            alive = True
        except (OSError, ProcessLookupError):
            alive = False

    # Report file existence = definitive completion signal
    report_found = False
    report_path = None
    if wine_prefix:
        base = os.path.join(wine_prefix, 'drive_c', 'mt5mcp_opt_report')
        for ext in ('.htm', '.htm.xml', '.html'):
            candidate = base + ext
            if os.path.exists(candidate):
                report_found = True
                report_path = candidate
                break

    # Tail log
    log_tail: list[str] = []
    if log_file and os.path.exists(log_file):
        try:
            log_tail = Path(log_file).read_text(errors='replace').splitlines()[-20:]
        except Exception:
            pass

    # Elapsed time
    elapsed_seconds = None
    if started_at:
        try:
            from datetime import datetime, timezone
            start_dt = datetime.fromisoformat(started_at.replace('Z', '+00:00'))
            elapsed_seconds = int((datetime.now(timezone.utc) - start_dt).total_seconds())
        except Exception:
            pass

    if report_found:
        hint = f'Optimization complete. Call get_optimization_results with job_id="{job_id}".'
    elif alive:
        hint = f'Still running. Monitor: tail -f {log_file}'
    else:
        hint = f'Process not running and no report found. Check log: {log_file}'

    return {
        'success': True,
        'job_id': job_id,
        'alive': alive,
        'finished': report_found,
        'elapsed_seconds': elapsed_seconds,
        'report_found': report_found,
        'report_path': report_path,
        'log_file': log_file,
        'log_tail': log_tail,
        'hint': hint,
    }


async def handle_prune_reports(args: dict) -> dict:
    keep_last = int(args.get('keep_last') or cfg('keep_last', '20') or 20)
    REPORTS_DIR.mkdir(exist_ok=True)

    all_dirs = sorted(
        [d for d in REPORTS_DIR.iterdir() if d.is_dir() and not d.name.endswith('_opt')],
        key=lambda d: d.stat().st_mtime,
    )
    to_delete = all_dirs[:-keep_last] if len(all_dirs) > keep_last else []
    kept = all_dirs[-keep_last:] if len(all_dirs) > keep_last else all_dirs

    deleted_names = []
    for d in to_delete:
        try:
            shutil.rmtree(str(d))
            deleted_names.append(d.name)
        except Exception:
            pass

    return {
        'success': True,
        'deleted_count': len(deleted_names),
        'kept_count': len(kept),
        'deleted_dirs': deleted_names,
        'kept_dirs': [d.name for d in kept],
    }


async def handle_list_reports(args: dict) -> dict:
    REPORTS_DIR.mkdir(exist_ok=True)
    include_opt = args.get('include_opt', False)
    limit = int(args.get('limit') or 30)

    dirs = sorted(
        [d for d in REPORTS_DIR.iterdir() if d.is_dir()],
        key=lambda d: d.stat().st_mtime,
        reverse=True,
    )
    if not include_opt:
        dirs = [d for d in dirs if not d.name.endswith('_opt')]
    dirs = dirs[:limit]

    rows = []
    for d in dirs:
        m = read_json(str(d / 'metrics.json'))
        row: dict = {'name': d.name, 'is_opt': d.name.endswith('_opt')}
        if m:
            row['net_profit'] = m.get('net_profit')
            row['max_dd_pct'] = m.get('max_dd_pct')
            row['total_trades'] = m.get('total_trades')
            row['symbol'] = m.get('symbol')
            row['timeframe'] = m.get('timeframe')
            row['from_date'] = m.get('from_date') or m.get('testing_from')
            row['to_date'] = m.get('to_date') or m.get('testing_to')
        else:
            row['metrics'] = 'missing'
        rows.append(row)

    return {'success': True, 'count': len(rows), 'reports': rows}


async def handle_tail_log(args: dict) -> dict:
    n = int(args.get('n') or 50)
    filt = args.get('filter', 'all')

    log_path: str | None = args.get('log_file')

    if not log_path and 'job_id' in args:
        job_id = args['job_id']
        meta_path = ROOT_DIR / '.mt5mcp_jobs' / f'{job_id}.json'
        if not meta_path.exists():
            return {'success': False, 'error': f'Job not found: {job_id}'}
        with open(meta_path) as f:
            meta = json.load(f)
        log_path = meta.get('log_file', '')

    if not log_path:
        report_dir = args.get('report_dir') or latest_report_dir()
        if report_dir:
            log_path = str(Path(report_dir) / 'progress.log')

    if not log_path or not os.path.exists(log_path):
        return {'success': False, 'error': f'Log file not found: {log_path}'}

    try:
        lines = Path(log_path).read_text(errors='replace').splitlines()
    except Exception as e:
        return {'success': False, 'error': str(e)}

    if filt == 'errors':
        lines = [l for l in lines if 'error' in l.lower() or 'fail' in l.lower() or 'err:' in l.lower()]
    elif filt == 'warnings':
        lines = [l for l in lines if 'warn' in l.lower() or 'error' in l.lower()]

    return {
        'success': True,
        'log_file': log_path,
        'total_lines': len(lines),
        'lines': lines[-n:],
    }


def _dir_size(path: Path) -> int:
    return sum(f.stat().st_size for f in path.rglob('*') if f.is_file())


async def handle_cache_status(args: dict) -> dict:
    terminal_dir = cfg('terminal_dir')
    if not terminal_dir:
        return {'success': False, 'error': 'terminal_dir not configured'}

    cache_dir = Path(cfg('tester_cache_dir') or os.path.join(terminal_dir, 'Tester'))
    if not cache_dir.is_dir():
        return {'success': False, 'error': f'Cache dir not found: {cache_dir}'}

    total_bytes = 0
    breakdown: list[dict] = []

    for item in sorted(cache_dir.iterdir()):
        if item.is_dir():
            sz = _dir_size(item)
            total_bytes += sz
            breakdown.append({'symbol': item.name, 'size_mb': round(sz / 1024 / 1024, 2)})
        elif item.is_file():
            sz = item.stat().st_size
            total_bytes += sz

    return {
        'success': True,
        'cache_dir': str(cache_dir),
        'total_size_mb': round(total_bytes / 1024 / 1024, 2),
        'symbols': breakdown,
    }


async def handle_clean_cache(args: dict) -> dict:
    terminal_dir = cfg('terminal_dir')
    if not terminal_dir:
        return {'success': False, 'error': 'terminal_dir not configured'}

    cache_dir = Path(cfg('tester_cache_dir') or os.path.join(terminal_dir, 'Tester'))
    if not cache_dir.is_dir():
        return {'success': False, 'error': f'Cache dir not found: {cache_dir}'}

    symbol = args.get('symbol', '').strip()
    dry_run = bool(args.get('dry_run', False))

    targets: list[Path] = []
    if symbol:
        target = cache_dir / symbol
        if target.is_dir():
            targets.append(target)
        else:
            return {'success': False, 'error': f'No cache found for symbol: {symbol}'}
    else:
        targets = [d for d in cache_dir.iterdir() if d.is_dir()]

    freed_bytes = sum(_dir_size(t) for t in targets)
    names = [t.name for t in targets]

    if not dry_run:
        for t in targets:
            shutil.rmtree(str(t))

    return {
        'success': True,
        'dry_run': dry_run,
        'deleted_symbols': names,
        'freed_mb': round(freed_bytes / 1024 / 1024, 2),
        'hint': 'Next backtest will regenerate tick data (slower first run).',
    }


# ── .set file helpers ─────────────────────────────────────────────────────────

def _parse_set_line(line: str) -> tuple[str, dict] | None:
    """Parse one .set file line → (name, param_dict) or None."""
    line = line.strip()
    if not line or line.startswith(';') or '=' not in line:
        return None
    name, _, raw = line.partition('=')
    name = name.strip()
    parts = raw.split('||')
    value = parts[0].strip()
    param: dict = {'value': value}
    if len(parts) >= 4:
        param['from'] = parts[1].strip()
        param['to'] = parts[2].strip()
        param['step'] = parts[3].strip() if len(parts) > 3 else ''
        param['optimize'] = parts[4].strip() == 'Y' if len(parts) > 4 else False
    return name, param


def _decode_set(path: str) -> tuple[dict, list[str]]:
    """Load a .set file → (params, comments). Raises ValueError on decode failure."""
    content = None
    raw = Path(path).read_bytes()
    for enc in ('utf-16-le', 'utf-16', 'utf-8-sig', 'utf-8'):
        try:
            if enc in ('utf-16-le', 'utf-16') and raw[:2] in (b'\xff\xfe', b'\xfe\xff'):
                content = raw.decode('utf-16')
            else:
                content = raw.decode(enc)
            break
        except (UnicodeDecodeError, LookupError):
            continue
    if content is None:
        raise ValueError(f'Cannot decode {path} — unknown encoding')

    params: dict = {}
    comments: list[str] = []
    for line in content.splitlines():
        if line.strip().startswith(';'):
            comments.append(line.strip().lstrip(';').strip())
            continue
        result = _parse_set_line(line)
        if result:
            name, param = result
            params[name] = param
    return params, comments


def _encode_set(params: dict, comments: list[str] | None = None) -> bytes:
    """Serialize params (and optional header comments) to UTF-16LE bytes."""
    lines: list[str] = []
    if comments:
        for c in comments:
            lines.append(f'; {c}')
    for name, spec in params.items():
        if isinstance(spec, dict):
            value = str(spec.get('value', ''))
            if 'from' in spec:
                flag = 'Y' if spec.get('optimize', False) else 'N'
                lines.append(f"{name}={value}||{spec['from']}||{spec.get('to', value)}||{spec.get('step', '1')}||{flag}")
            else:
                lines.append(f"{name}={value}")
        else:
            lines.append(f"{name}={spec}")
    return ('\r\n'.join(lines) + '\r\n').encode('utf-16-le')


def _write_set(path: str, data: bytes) -> None:
    """Write bytes to path and apply chmod 444 (required by MT5)."""
    p = Path(path)
    p.parent.mkdir(parents=True, exist_ok=True)
    # chmod 644 first in case file already exists as 444
    if p.exists():
        os.chmod(path, 0o644)
    p.write_bytes(data)
    os.chmod(path, 0o444)


def _sweep_combinations(params: dict) -> tuple[list[dict], int]:
    """Return (swept_param_details, total_combinations) for a parsed params dict."""
    import math
    swept = []
    total = 1
    for name, spec in params.items():
        if not isinstance(spec, dict) or not spec.get('optimize'):
            continue
        try:
            f = float(spec['from'])
            t = float(spec['to'])
            s = float(spec['step'])
            count = max(1, math.floor(abs(t - f) / s) + 1) if s else 1
        except (KeyError, ValueError, ZeroDivisionError):
            count = 1
        swept.append({
            'name': name,
            'from': spec.get('from'),
            'to': spec.get('to'),
            'step': spec.get('step'),
            'count': count,
        })
        total *= count
    return swept, total


async def handle_read_set_file(args: dict) -> dict:
    path = args['path']
    if not os.path.exists(path):
        return {'success': False, 'error': f'File not found: {path}'}
    try:
        params, comments = _decode_set(path)
    except Exception as e:
        return {'success': False, 'error': str(e)}
    return {
        'success': True,
        'path': path,
        'param_count': len(params),
        'comments': comments,
        'params': params,
    }


async def handle_write_set_file(args: dict) -> dict:
    path = args['path']
    params: dict = args['params']
    try:
        _write_set(path, _encode_set(params))
    except Exception as e:
        return {'success': False, 'error': str(e)}
    return {
        'success': True,
        'path': path,
        'param_count': len(params),
        'encoding': 'utf-16-le',
        'permissions': '444 (read-only, required by MT5)',
    }


async def handle_patch_set_file(args: dict) -> dict:
    path = args['path']
    patches: dict = args['patches']
    if not os.path.exists(path):
        return {'success': False, 'error': f'File not found: {path}'}
    try:
        params, comments = _decode_set(path)
    except Exception as e:
        return {'success': False, 'error': str(e)}

    changed: list[dict] = []
    for name, new_spec in patches.items():
        old = params.get(name, {})
        old_value = old.get('value') if isinstance(old, dict) else str(old)
        if isinstance(new_spec, dict):
            # Merge: keep existing sweep config unless overridden
            merged = dict(old) if isinstance(old, dict) else {'value': old_value}
            merged.update(new_spec)
            params[name] = merged
            new_value = str(merged.get('value', ''))
        else:
            new_value = str(new_spec)
            if isinstance(params.get(name), dict):
                params[name] = dict(params[name])
                params[name]['value'] = new_value
            else:
                params[name] = {'value': new_value}
        if old_value != new_value:
            changed.append({'name': name, 'old': old_value, 'new': new_value})

    try:
        _write_set(path, _encode_set(params, comments))
    except Exception as e:
        return {'success': False, 'error': str(e)}

    return {
        'success': True,
        'path': path,
        'changed_count': len(changed),
        'changed': changed,
        'param_count': len(params),
    }


async def handle_clone_set_file(args: dict) -> dict:
    source = args['source']
    destination = args['destination']
    overrides: dict = args.get('overrides', {})
    if not os.path.exists(source):
        return {'success': False, 'error': f'Source not found: {source}'}
    try:
        params, comments = _decode_set(source)
    except Exception as e:
        return {'success': False, 'error': str(e)}

    changed: list[dict] = []
    for name, new_spec in overrides.items():
        old = params.get(name, {})
        old_value = old.get('value') if isinstance(old, dict) else str(old) if old else None
        if isinstance(new_spec, dict):
            merged = dict(old) if isinstance(old, dict) else {}
            merged.update(new_spec)
            params[name] = merged
            new_value = str(merged.get('value', ''))
        else:
            new_value = str(new_spec)
            if isinstance(params.get(name), dict):
                params[name] = dict(params[name])
                params[name]['value'] = new_value
            else:
                params[name] = {'value': new_value}
        if old_value != new_value:
            changed.append({'name': name, 'old': old_value, 'new': new_value})

    try:
        _write_set(destination, _encode_set(params, comments))
    except Exception as e:
        return {'success': False, 'error': str(e)}

    return {
        'success': True,
        'source': source,
        'destination': destination,
        'param_count': len(params),
        'overridden_count': len(changed),
        'overridden': changed,
    }


async def handle_set_from_optimization(args: dict) -> dict:
    path = args['path']
    opt_params: dict = args['params']   # {name: value} from optimization result
    template: str | None = args.get('template')
    sweep: dict = args.get('sweep', {})  # {name: {from, to, step}} to add sweep flags

    base_params: dict = {}
    base_comments: list[str] = []

    if template:
        if not os.path.exists(template):
            return {'success': False, 'error': f'Template not found: {template}'}
        try:
            base_params, base_comments = _decode_set(template)
        except Exception as e:
            return {'success': False, 'error': str(e)}

    # Start from template (or empty), apply opt values, strip all sweep flags
    merged: dict = {}
    for name, spec in base_params.items():
        # Copy as fixed value (no sweep)
        value = spec.get('value') if isinstance(spec, dict) else str(spec)
        merged[name] = {'value': value}

    # Apply optimization result values (overwrite template values, add new params)
    for name, value in opt_params.items():
        merged[name] = {'value': str(value)}

    # Optionally re-add sweep ranges for a subset of params
    for name, sweep_spec in sweep.items():
        if name in merged:
            merged[name].update({
                'from': str(sweep_spec.get('from', '')),
                'to': str(sweep_spec.get('to', '')),
                'step': str(sweep_spec.get('step', '1')),
                'optimize': bool(sweep_spec.get('optimize', True)),
            })

    try:
        _write_set(path, _encode_set(merged, base_comments))
    except Exception as e:
        return {'success': False, 'error': str(e)}

    swept, total = _sweep_combinations(merged)
    return {
        'success': True,
        'path': path,
        'param_count': len(merged),
        'from_template': bool(template),
        'opt_params_applied': len(opt_params),
        'swept_params': len(swept),
        'total_combinations': total if swept else 0,
    }


async def handle_diff_set_files(args: dict) -> dict:
    path_a = args['path_a']
    path_b = args['path_b']

    for p in (path_a, path_b):
        if not os.path.exists(p):
            return {'success': False, 'error': f'File not found: {p}'}
    try:
        params_a, _ = _decode_set(path_a)
        params_b, _ = _decode_set(path_b)
    except Exception as e:
        return {'success': False, 'error': str(e)}

    keys_a = set(params_a)
    keys_b = set(params_b)

    added = []
    for k in sorted(keys_b - keys_a):
        spec = params_b[k]
        added.append({'name': k, 'value': spec.get('value') if isinstance(spec, dict) else str(spec)})

    removed = []
    for k in sorted(keys_a - keys_b):
        spec = params_a[k]
        removed.append({'name': k, 'value': spec.get('value') if isinstance(spec, dict) else str(spec)})

    changed = []
    for k in sorted(keys_a & keys_b):
        sa = params_a[k]
        sb = params_b[k]
        va = sa.get('value') if isinstance(sa, dict) else str(sa)
        vb = sb.get('value') if isinstance(sb, dict) else str(sb)
        opt_a = sa.get('optimize', False) if isinstance(sa, dict) else False
        opt_b = sb.get('optimize', False) if isinstance(sb, dict) else False
        if va != vb or opt_a != opt_b:
            entry: dict = {'name': k, 'a': va, 'b': vb}
            if opt_a != opt_b:
                entry['sweep_a'] = opt_a
                entry['sweep_b'] = opt_b
            changed.append(entry)

    identical = not added and not removed and not changed
    return {
        'success': True,
        'path_a': path_a,
        'path_b': path_b,
        'identical': identical,
        'added_count': len(added),
        'removed_count': len(removed),
        'changed_count': len(changed),
        'added': added,
        'removed': removed,
        'changed': changed,
    }


async def handle_describe_sweep(args: dict) -> dict:
    path = args['path']
    if not os.path.exists(path):
        return {'success': False, 'error': f'File not found: {path}'}
    try:
        params, comments = _decode_set(path)
    except Exception as e:
        return {'success': False, 'error': str(e)}

    swept, total = _sweep_combinations(params)
    fixed_count = len(params) - len(swept)

    return {
        'success': True,
        'path': path,
        'total_params': len(params),
        'swept_count': len(swept),
        'fixed_count': fixed_count,
        'total_combinations': total,
        'swept_params': swept,
        'hint': (
            'No swept params — this is a backtest .set, not an optimization .set.'
            if not swept else
            f'{total:,} combinations. Typical range: 1–8h depending on EA tick speed.'
        ),
    }


async def handle_list_set_files(args: dict) -> dict:
    terminal_dir = cfg('terminal_dir')
    if not terminal_dir:
        return {'success': False, 'error': 'terminal_dir not configured'}

    profiles_dir = Path(cfg('tester_profiles_dir') or
                        os.path.join(terminal_dir, 'MQL5', 'Profiles', 'Tester'))
    if not profiles_dir.is_dir():
        return {'success': False, 'error': f'Tester profiles dir not found: {profiles_dir}'}

    ea_filter = args.get('ea', '').lower()
    rows: list[dict] = []

    for f in sorted(profiles_dir.glob('*.set'), key=lambda x: x.stat().st_mtime, reverse=True):
        if ea_filter and ea_filter not in f.stem.lower():
            continue
        try:
            params, _ = _decode_set(str(f))
            swept, total = _sweep_combinations(params)
            rows.append({
                'name': f.name,
                'param_count': len(params),
                'swept_count': len(swept),
                'total_combinations': total if swept else 0,
                'modified': f.stat().st_mtime,
            })
        except Exception:
            rows.append({'name': f.name, 'error': 'unreadable'})

    # Convert mtime to ISO for readability
    from datetime import datetime
    for r in rows:
        if 'modified' in r:
            r['modified'] = datetime.fromtimestamp(r['modified']).strftime('%Y-%m-%d %H:%M')

    return {
        'success': True,
        'profiles_dir': str(profiles_dir),
        'count': len(rows),
        'files': rows,
    }


async def handle_archive_report(args: dict) -> dict:
    report_dir = args.get('report_dir') or latest_report_dir()
    if not report_dir:
        return {'success': False, 'error': 'No report directory found'}

    entry = _build_history_entry(report_dir)
    if not entry:
        return {'success': False, 'error': f'No metrics.json or analysis.json in {report_dir}'}

    if args.get('verdict'):
        entry['verdict'] = args['verdict']
    if args.get('notes'):
        entry['notes'] = args['notes']
    if args.get('tags'):
        entry['tags'] = args['tags']

    history = load_history()
    existing_ids = {e['id'] for e in history}
    already_exists = entry['id'] in existing_ids

    if not already_exists:
        history.append(entry)
        save_history(history)

    deleted = False
    if args.get('delete_after') and not already_exists:
        try:
            shutil.rmtree(report_dir)
            deleted = True
            # Update the entry in history to reflect deletion
            for e in history:
                if e['id'] == entry['id']:
                    e['report_dir_deleted'] = True
                    break
            save_history(history)
        except Exception as exc:
            return {'success': False, 'error': f'Archive succeeded but delete failed: {exc}'}

    return {
        'success': True,
        'id': entry['id'],
        'already_existed': already_exists,
        'deleted_source': deleted,
        'history_file': str(HISTORY_FILE),
        'entry_summary': {
            'ea': entry['ea'],
            'symbol': entry['symbol'],
            'metrics': entry['metrics'],
            'verdict': entry['verdict'],
        },
    }


async def handle_archive_all_reports(args: dict) -> dict:
    REPORTS_DIR.mkdir(exist_ok=True)
    delete_after = bool(args.get('delete_after', False))
    keep_last = int(args.get('keep_last', 5))
    dry_run = bool(args.get('dry_run', False))

    all_dirs = sorted(
        [d for d in REPORTS_DIR.iterdir() if d.is_dir() and not d.name.endswith('_opt')],
        key=lambda d: d.stat().st_mtime,
    )

    history = load_history()
    existing_ids = {e['id'] for e in history}

    # Dirs protected from deletion regardless of keep_last
    protected = {d.name for d in all_dirs[-keep_last:]} if keep_last > 0 else set()

    results = {'archived': [], 'skipped': [], 'deleted': [], 'failed': []}

    for d in all_dirs:
        if d.name in existing_ids:
            results['skipped'].append(d.name)
            continue

        entry = _build_history_entry(str(d))
        if not entry:
            results['failed'].append(d.name)
            continue

        if not dry_run:
            history.append(entry)
        results['archived'].append(d.name)

        should_delete = delete_after and d.name not in protected
        if should_delete and not dry_run:
            try:
                shutil.rmtree(str(d))
                entry['report_dir_deleted'] = True
                results['deleted'].append(d.name)
            except Exception:
                results['failed'].append(d.name)

    if not dry_run and results['archived']:
        save_history(history)

    return {
        'success': True,
        'dry_run': dry_run,
        'archived_count': len(results['archived']),
        'skipped_count': len(results['skipped']),
        'deleted_count': len(results['deleted']),
        'failed_count': len(results['failed']),
        'history_file': str(HISTORY_FILE),
        **results,
    }


async def handle_get_history(args: dict) -> dict:
    history = load_history()
    if not history:
        return {'success': True, 'count': 0, 'entries': []}

    ea_filter = args.get('ea', '').lower()
    symbol_filter = args.get('symbol', '').upper()
    verdict_filter = args.get('verdict')
    tag_filter = args.get('tag', '')
    min_profit = args.get('min_profit')
    max_dd = args.get('max_dd_pct')
    sort_by = args.get('sort_by', 'date')
    limit = int(args.get('limit') or 20)
    include_monthly = bool(args.get('include_monthly', False))

    filtered = []
    for e in history:
        if ea_filter and ea_filter not in e.get('ea', '').lower():
            continue
        if symbol_filter and e.get('symbol', '').upper() != symbol_filter:
            continue
        if verdict_filter and e.get('verdict') != verdict_filter:
            continue
        if tag_filter and tag_filter not in e.get('tags', []):
            continue
        m = e.get('metrics', {})
        if min_profit is not None and (m.get('net_profit') or 0) < min_profit:
            continue
        if max_dd is not None and (m.get('max_dd_pct') or 999) > max_dd:
            continue
        filtered.append(e)

    key_map = {
        'date':   lambda e: e.get('archived_at', ''),
        'profit': lambda e: (e.get('metrics') or {}).get('net_profit') or 0,
        'dd':     lambda e: (e.get('metrics') or {}).get('max_dd_pct') or 999,
        'sharpe': lambda e: (e.get('metrics') or {}).get('sharpe_ratio') or 0,
    }
    reverse = sort_by != 'dd'
    filtered.sort(key=key_map.get(sort_by, key_map['date']), reverse=reverse)
    filtered = filtered[:limit]

    if not include_monthly:
        for e in filtered:
            e.pop('monthly_pnl', None)

    return {'success': True, 'count': len(filtered), 'entries': filtered}


async def handle_promote_to_baseline(args: dict) -> dict:
    from datetime import datetime, timezone

    # Resolve source: history entry, explicit report_dir, or latest
    entry: dict | None = None
    report_dir: str | None = None

    if 'history_id' in args:
        history = load_history()
        matches = [e for e in history if e['id'] == args['history_id']]
        if not matches:
            return {'success': False, 'error': f"History entry not found: {args['history_id']}"}
        entry = matches[0]
        report_dir = entry.get('report_dir') if not entry.get('report_dir_deleted') else None
    else:
        report_dir = args.get('report_dir') or latest_report_dir()
        if not report_dir:
            return {'success': False, 'error': 'No report directory found'}

    # Load metrics — prefer live report dir, fall back to history entry
    if report_dir and Path(report_dir).is_dir():
        metrics = read_json(os.path.join(report_dir, 'metrics.json'))
    elif entry:
        metrics = entry.get('metrics', {})
    else:
        return {'success': False, 'error': 'Source not found (report dir missing and no history entry)'}

    if not metrics:
        return {'success': False, 'error': 'No metrics found in source'}

    now = datetime.now(timezone.utc).strftime('%Y-%m-%d')
    ea = (entry or {}).get('ea') or metrics.get('expert') or metrics.get('ea') or ''
    symbol = (entry or {}).get('symbol') or metrics.get('symbol') or ''
    from_date = (entry or {}).get('from_date') or ''
    to_date = (entry or {}).get('to_date') or ''
    period = f"{from_date}/{to_date}" if from_date and to_date else ''

    baseline = {
        'ea': ea,
        'symbol': symbol,
        'period': period,
        'net_profit': metrics.get('net_profit'),
        'profit_factor': metrics.get('profit_factor'),
        'max_drawdown_pct': metrics.get('max_dd_pct'),
        'sharpe_ratio': metrics.get('sharpe_ratio'),
        'total_trades': metrics.get('total_trades'),
        'recovery_factor': metrics.get('recovery_factor'),
        'promoted_from': (entry or {}).get('id') or Path(report_dir or '').name,
        'promoted_at': now,
        'notes': args.get('notes', f'Promoted {now}'),
    }

    BASELINE_FILE.parent.mkdir(exist_ok=True)
    with open(BASELINE_FILE, 'w') as f:
        json.dump(baseline, f, indent=2)

    # Mark in history
    if entry:
        history = load_history()
        for e in history:
            if e['id'] == entry['id']:
                e['promoted_to_baseline'] = True
                e['verdict'] = e.get('verdict') or 'reference'
                break
        save_history(history)

    return {
        'success': True,
        'baseline_file': str(BASELINE_FILE),
        'baseline': baseline,
    }


async def handle_annotate_history(args: dict) -> dict:
    history_id = args['history_id']
    history = load_history()

    target = next((e for e in history if e['id'] == history_id), None)
    if not target:
        return {'success': False, 'error': f'Entry not found: {history_id}'}

    if 'verdict' in args:
        target['verdict'] = args['verdict']
    if 'notes' in args:
        target['notes'] = args['notes']
    if 'tags' in args:
        target['tags'] = args['tags']
    if 'add_tags' in args:
        existing = target.get('tags') or []
        for t in args['add_tags']:
            if t not in existing:
                existing.append(t)
        target['tags'] = existing

    save_history(history)

    return {
        'success': True,
        'id': history_id,
        'verdict': target.get('verdict'),
        'notes': target.get('notes'),
        'tags': target.get('tags'),
    }


async def handle_list_jobs(args: dict) -> dict:
    jobs_dir = ROOT_DIR / '.mt5mcp_jobs'
    if not jobs_dir.is_dir():
        return {'success': True, 'jobs': [], 'count': 0}

    include_done = args.get('include_done', True)
    rows: list[dict] = []

    from datetime import datetime, timezone

    for meta_file in sorted(jobs_dir.glob('*.json'), reverse=True):
        try:
            with open(meta_file) as f:
                meta = json.load(f)
        except Exception:
            continue

        job_id = meta_file.stem
        pid = meta.get('pid')
        started_at = meta.get('started_at', '')
        log_file = meta.get('log_file', '')
        wine_prefix = meta.get('wine_prefix', '')

        alive = False
        if pid:
            try:
                os.kill(int(pid), 0)
                alive = True
            except (OSError, ProcessLookupError):
                pass

        report_found = False
        if wine_prefix:
            base = os.path.join(wine_prefix, 'drive_c', 'mt5mcp_opt_report')
            for ext in ('.htm', '.htm.xml', '.html'):
                if os.path.exists(base + ext):
                    report_found = True
                    break

        status = 'running' if alive else ('done' if report_found else 'failed')

        elapsed_seconds = None
        if started_at:
            try:
                start_dt = datetime.fromisoformat(started_at.replace('Z', '+00:00'))
                elapsed_seconds = int((datetime.now(timezone.utc) - start_dt).total_seconds())
            except Exception:
                pass

        if not include_done and status != 'running':
            continue

        rows.append({
            'job_id': job_id,
            'status': status,
            'elapsed_seconds': elapsed_seconds,
            'expert': meta.get('expert', ''),
            'started_at': started_at,
            'log_file': log_file,
        })

    return {'success': True, 'count': len(rows), 'jobs': rows}


# ── Entry point ───────────────────────────────────────────────────────────────

async def main():
    async with mcp.server.stdio.stdio_server() as (read_stream, write_stream):
        await app.run(
            read_stream,
            write_stream,
            app.create_initialization_options(),
        )


def cli():
    """Sync entry point for [project.scripts] — pyproject.toml requires a sync callable."""
    asyncio.run(main())


if __name__ == '__main__':
    asyncio.run(main())
