# MT5-Quant

**MCP server for MT5 strategy development on macOS/Linux.** 85 tools to compile, backtest, analyze, optimize, debug crashes, and manage MQL5 Expert Advisors — no Windows required.

```
You: "Backtest MyEA Jan-Mar, what caused the February drawdown?"

Claude: [compile → clean → backtest → analyze 1,847 deals]
        → Feb 14: BUY grid at L6, locking lot 1.75× base
        → Cutloss fired 17 points later
        → Recommendation: cap locking multiplier to ≤1.2×
```

## Why MT5-Quant

| | MT5-Quant | Other MT5 MCPs | QuantConnect |
|---|---|---|---|
| Backtest pipeline | ✅ Full | ❌ | Cloud only |
| Deal-level analytics | ✅ 15+ dims | ❌ | ❌ |
| MQL5 compilation | ✅ | ❌ | ❌ |
| macOS/Linux native | ✅ | Windows only | Cloud |
| Optimization | ✅ Background | ❌ | ✅ Paid |
| Crash debugging | ✅ Wine/MT5 diagnostics | ❌ | ❌ |

## Quick Install

### 1. Download & Setup

```bash
curl -L -o mt5.tar.gz https://github.com/masdevid/mt5-mcp/releases/latest/download/mt5-quant-macos-arm64.tar.gz
tar -xzf mt5.tar.gz
bash scripts/setup.sh
```

### 2. Register MCP Server

| Platform | Command / Config | Docs |
|----------|------------------|------|
| **Claude Code** | `claude mcp add mt5-quant -- $(pwd)/mt5-quant` | [Setup →](docs/QUICKSTART.md) |
| **Windsurf** | Edit `~/.codeium/windsurf/mcp_config.json` | [WINDSURF.md →](docs/WINDSURF.md) |
| **Cursor** | Edit `~/.cursor/mcp.json` or use Settings → MCP | [CURSOR.md →](docs/CURSOR.md) |
| **VS Code** | Edit `.vscode/mcp.json` or run `MCP: Add Server` | [VSCODE.md →](docs/VSCODE.md) |
| **Antigravity** | Agent Panel → ... → MCP Servers → Edit configuration | [ANTIGRAVITY.md →](docs/ANTIGRAVITY.md) |

> **Note:** Use absolute paths like `/Users/name/mt5-quant/mt5-quant` or `$(pwd)/mt5-quant`, not relative paths like `./mt5-quant`.

## Quick Start

```
Run a backtest on MyEA from 2025.01.01 to 2025.03.31
```

The AI runs the full pipeline: compile → clean cache → backtest → extract → analyze.

## Documentation

| Doc | Purpose |
|-----|---------|
| [QUICKSTART.md](docs/QUICKSTART.md) | Complete setup for macOS/Linux |
| [WINDSURF.md](docs/WINDSURF.md) | Windsurf IDE setup |
| [CURSOR.md](docs/CURSOR.md) | Cursor IDE setup |
| [VSCODE.md](docs/VSCODE.md) | VS Code setup |
| [ANTIGRAVITY.md](docs/ANTIGRAVITY.md) | Antigravity IDE setup |
| [CONFIG.md](docs/CONFIG.md) | Configuration reference |
| [TOOLS.md](docs/MCP_TOOLS.md) | All 75 tools documented |
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | Design and internals |
| [TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) | Common issues |
| [REMOTE_AGENTS.md](docs/REMOTE_AGENTS.md) | Linux optimization agents |

## MCP Tools (75)

### Core workflow

| Tool | Description |
|------|-------------|
| `run_backtest` | Full pipeline: compile → clean → backtest → extract → analyze |
| `run_optimization` | Genetic optimization (background, returns immediately) |
| `get_optimization_results` | Parse optimization results after MT5 finishes |
| `analyze_report` | Read `analysis.json` from any report directory |
| `compare_baseline` | Compare report vs baseline, return winner/loser verdict |
| `compile_ea` | Compile MQL5 EA via MetaEditor |
| `list_experts` | List all EAs in MQL5/Experts directory |
| `list_indicators` | List all indicators in MQL5/Indicators directory |
| `list_scripts` | List all scripts in MQL5/Scripts directory |
| `healthcheck` | Quick server health check |

### Granular Analytics (individual analysis)

| Tool | Description |
|------|-------------|
| `analyze_monthly_pnl` | Monthly P/L breakdown only |
| `analyze_drawdown_events` | Drawdown events and causes only |
| `analyze_top_losses` | Worst losing deals only |
| `analyze_loss_sequences` | Consecutive loss patterns only |
| `analyze_position_pairs` | Position hold time and P/L pairs |
| `analyze_direction_bias` | Buy vs Sell performance |
| `analyze_streaks` | Win/loss streak analysis |
| `analyze_concurrent_peak` | Peak simultaneous positions |

Use these for targeted analysis, or `analyze_report` to run all at once.

### Deal-Level Analytics (New)

| Tool | Description |
|------|-------------|
| `list_deals` | List individual deals with filters (type, profit range, volume, dates) |
| `search_deals_by_comment` | Full-text search in deal comments (e.g., "Layer #3") |
| `search_deals_by_magic` | Filter deals by EA magic number |
| `analyze_profit_distribution` | Profit histogram: small/medium/large wins and losses |
| `analyze_time_performance` | Performance by hour of day and day of week |
| `analyze_hold_time_distribution` | Hold time buckets + correlation with profit |
| `analyze_layer_performance` | Grid/martingale layer analysis from comments |
| `analyze_volume_vs_profit` | Volume correlation + performance by lot size |
| `analyze_costs` | Commission and swap impact on profitability |
| `analyze_efficiency` | Profit per hour/day, annualized return, trade frequency |

### Monitoring

| Tool | Description |
|------|-------------|
| `verify_setup` | Check Wine/MT5 paths, Wine version, and EA/set file counts |
| `get_backtest_status` | Check live progress of a running backtest pipeline |
| `get_optimization_status` | Check live state of a background optimization job |
| `list_jobs` | All optimization jobs with compact status in one call |

### Reports & logs

| Tool | Description |
|------|-------------|
| `list_reports` | Compact table of all runs with key metrics — no full analysis needed |
| `get_latest_report` | Get most recent report with optional equity chart |
| `search_reports` | Find reports by EA, symbol, date range, or profit criteria |
| `get_report_by_id` | Get specific report by ID with equity chart |
| `get_reports_summary` | Aggregate stats: counts, averages, pass rates |
| `get_best_reports` | Top N reports sorted by any metric (profit factor, drawdown, etc.) |
| `search_reports_by_tags` | Find reports by tags |
| `search_reports_by_date_range` | Query by backtest date range |
| `search_reports_by_notes` | Full-text search in report notes |
| `get_reports_by_set_file` | Find all reports using a specific .set file |
| `get_comparable_reports` | Find comparable reports (same EA/symbol/timeframe) |
| `tail_log` | Read last N lines of any log; `filter=errors` to see only failures |
| `prune_reports` | Delete old report directories, keep last N (skips `_opt` dirs) |

### History & baseline

| Tool | Description |
|------|-------------|
| `archive_report` | Convert one report dir → compact JSON entry in `backtest_history.json`, optionally delete source |
| `archive_all_reports` | Bulk-archive all report dirs then optionally delete them; keeps N newest safe |
| `get_history` | Query history with filters (EA, symbol, verdict, profit, DD) and sort options |
| `annotate_history` | Attach verdict / notes / tags to any history entry |
| `promote_to_baseline` | Write a history entry or report to `baseline.json` for compare_baseline |

### Cache management

| Tool | Description |
|------|-------------|
| `cache_status` | MT5 tester cache size breakdown by symbol — check before cleaning |
| `clean_cache` | Delete tester cache files; supports per-symbol and `dry_run` |

### Pre-flight & Validation

| Tool | Description |
|------|-------------|
| `get_active_account` | Get current MT5 account session (login, server, available symbols) |
| `check_symbol_data_status` | Validate symbol has sufficient history data for date range |
| `check_mt5_status` | Check if MT5 terminal is installed and ready |
| `validate_ea_syntax` | Pre-compile syntax check without running full compilation |

### Debugging & Diagnostics (New)

| Tool | Description |
|------|-------------|
| `diagnose_wine` | Check Wine installation, version, and prefix health |
| `get_mt5_logs` | Get MT5 terminal, tester, or MetaEditor logs with filtering |
| `search_mt5_errors` | Search logs for error patterns (crash, exception, access violation) |
| `check_mt5_process` | Check if MT5 processes are running, get PID, CPU, memory usage |
| `kill_mt5_process` | Kill stuck MT5 processes (force=true for wineserver) |
| `check_system_resources` | Check disk space, memory, CPU availability |
| `validate_mt5_config` | Validate terminal.ini and tester configuration files |
| `get_wine_prefix_info` | Get Wine prefix details: Windows version, installed programs, registry |
| `get_backtest_crash_info` | Investigate backtest failures: incomplete markers, missing deals.csv, errors |

### Project Management

| Tool | Description |
|------|-------------|
| `init_project` | Scaffold new MQL5 project with templates (scalper/swing/grid/basic) |
| `create_set_template` | Generate .set parameter file from EA input variables |
| `export_report` | Export backtest report to CSV, JSON, or Markdown |

### History & Comparison

| Tool | Description |
|------|-------------|
| `get_backtest_history` | List all backtests for EA/symbol with summary metrics |
| `compare_backtests` | Compare 2+ backtest results side-by-side with analysis |

### .set file — read / write

| Tool | Description |
|------|-------------|
| `list_set_files` | All .set files in tester profiles dir with sweep stats and combination counts |
| `read_set_file` | Parse UTF-16LE `.set` file → structured JSON params |
| `write_set_file` | Write full params dict → UTF-16LE `.set` with `chmod 444` |
| `patch_set_file` | Update specific params in-place, return diff — replaces read→edit→write |
| `clone_set_file` | Copy `.set` to new path with optional overrides in one call |

### .set file — analysis & generation

| Tool | Description |
|------|-------------|
| `describe_sweep` | Swept params, value counts, and total optimization combinations |
| `diff_set_files` | Side-by-side diff of two `.set` files — only changed params returned |
| `set_from_optimization` | Generate a clean backtest `.set` from `get_optimization_results` params; optionally narrow sweep |

### Search & Discovery

| Tool | Description |
|------|-------------|
| `search_experts` | Search EAs by name pattern across all directories |
| `search_indicators` | Search indicators by name pattern |
| `search_scripts` | Search scripts by name pattern |
| `copy_indicator_to_project` | Copy indicator to project directory |
| `copy_script_to_project` | Copy script to project directory |

Full schema: [docs/MCP_TOOLS.md](docs/MCP_TOOLS.md)

## Troubleshooting

Run `verify_setup` from Claude first — it checks all paths and returns actionable hints.

For crashes or unexplained failures during backtest/compile/optimization:
- `diagnose_wine` — Check Wine installation and prefix health
- `search_mt5_errors` — Find crash causes in logs
- `check_mt5_process` + `kill_mt5_process` — Detect and kill stuck processes
- `get_backtest_crash_info` — Investigate failed backtest reports

**[Full Troubleshooting Guide →](docs/TROUBLESHOOTING.md)**

---

## License

MIT

---
