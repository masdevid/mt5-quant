# MT5-Quant

**MCP server for MT5 strategy development on macOS/Linux.** 43 tools to compile, backtest, analyze, and optimize MQL5 Expert Advisors — no Windows required.

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

## Quick Install

```bash
curl -L -o mt5.tar.gz https://github.com/masdevid/mt5-mcp/releases/latest/download/mt5-quant-macos-arm64.tar.gz
tar -xzf mt5.tar.gz
bash scripts/setup.sh
claude mcp add MT5-Quant -- $(pwd)/mt5-quant
```

**[Full Setup →](docs/QUICKSTART.md)**

## Quick Start

```
Run a backtest on MyEA from 2025.01.01 to 2025.03.31
```

The AI runs the full pipeline: compile → clean cache → backtest → extract → analyze.

## Documentation

| Doc | Purpose |
|-----|---------|
| [QUICKSTART.md](docs/QUICKSTART.md) | Complete setup for macOS/Linux |
| [CONFIG.md](docs/CONFIG.md) | Configuration reference |
| [WINDSURF.md](docs/WINDSURF.md) | Windsurf IDE integration |
| [TOOLS.md](docs/MCP_TOOLS.md) | All 43 tools (31 documented) |
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | Design and internals |
| [TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) | Common issues |
| [REMOTE_AGENTS.md](docs/REMOTE_AGENTS.md) | Linux optimization agents |

## MCP Tools (43)

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

Full schema: [docs/MCP_TOOLS.md](docs/MCP_TOOLS.md)

## Troubleshooting

Run `verify_setup` from Claude first — it checks all paths and returns actionable hints.

**[Full Troubleshooting Guide →](docs/TROUBLESHOOTING.md)**

---

## License

MIT

---

*Built from battle-tested production infrastructure. Every edge case in the pipeline was hit in production.*
