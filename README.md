# MT5-Quant

**The MCP server for MT5 strategy development — not live trading.**

Most MT5 MCP servers let an AI place orders. MT5-Quant lets an AI *build the strategy*: compile an Expert Advisor, run a backtest, parse the report, analyze every deal, and optimize parameters — all in one conversation, on **macOS or Linux**, without a Windows machine.

```
You: "Run a backtest from Jan to March and tell me what caused the drawdown spike in February"

Claude: [compile → clean cache → backtest → parse XML report → analyze 1,847 deals]

        Worst DD event: Feb 14, BUY grid at L6.
        Locking hedge placed into a trending adverse move. Locking lot 1.75× base.
        Cutloss fired 17 price points later.

        Recommendation: cap locking lot multiplier to ≤1.2× to prevent cascade on adverse trend.
```

### How MT5-Quant is different

| | MT5-Quant | Other MT5 MCPs¹ | QuantConnect MCP | ai-trader |
|---|---|---|---|---|
| **Backtest via MCP** | ✅ full pipeline | ❌ | ✅ cloud only | ✅ Python only |
| **EA optimization via MCP** | ✅ genetic, background | ❌ | ✅ cloud only | ❌ |
| **Deal-level analytics** | ✅ 15+ dimensions | ❌ | ❌ | ❌ |
| **MQL5 EA compilation** | ✅ | ❌ | ❌ | ❌ |
| **macOS + Linux** | ✅ | ❌ Windows only | ❌ cloud | ✅ |
| **Headless / CI-CD** | ✅ Xvfb | ❌ | ❌ | ✅ |
| **Offline / no subscription** | ✅ | ✅ | ❌ paid cloud | ✅ |
| **MT5 .set file tooling** | ✅ 8 tools | ❌ | ❌ | ❌ |
| **Backtest history ledger** | ✅ JSON archive | ❌ | ✅ | ❌ |

¹ *ariadng/metatrader-mcp-server, Qoyyuum/mcp-metatrader5-server, Cloudmeru/MetaTrader-5-MCP-Server — all live-trading execution bridges, Windows-only.*

### What it covers

28 MCP tools across the full EA development loop:

```
list_set_files / describe_sweep / patch_set_file / set_from_optimization
    ↓ prepare parameters
compile_ea
    ↓ build .ex5
run_backtest  (compile → clean cache → MT5 tester → extract HTML/XML → analyze deals)
    ↓ fresh results
analyze_report / compare_baseline / get_history
    ↓ evaluate and record
archive_report(delete_after=true) → promote_to_baseline
    ↓ clean up, lock in new production reference
run_optimization  (background, nohup — takes hours)
    ↓ when MT5 finishes
get_optimization_results → set_from_optimization → run_backtest (verify)
```

The AI drives every step. You watch and approve.

---

## Quickstart

### 1. Clone and install

```bash
git clone https://github.com/masdevid/mt5-mcp
cd mt5-mcp
python3 -m venv .venv && source .venv/bin/activate
pip install -e .
```

> **Python 3.11+** required. The venv is optional but recommended.

### 2. Install MetaTrader 5

MT5 runs under Wine on both macOS and Linux. Two supported paths:

**macOS — MetaTrader 5.app (recommended, free)**

Download from [metatrader5.com](https://www.metatrader5.com/en/download) and install to `/Applications`. Launch it once so it initializes the Wine prefix (~30 s), then quit.

Wine binary auto-detected at:
```
/Applications/MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64
```
MT5 prefix auto-detected at:
```
~/Library/Application Support/net.metaquotes.wine.metatrader5/drive_c/Program Files/MetaTrader 5
```

**macOS — CrossOver (paid, better Wine compatibility)**

Install [CrossOver](https://www.codeweavers.com/), create a bottle named `MetaTrader5`, install MT5 inside it.

Wine binary:
```
/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine64
```
MT5 prefix (CrossOver 24+):
```
~/Library/Application Support/MetaQuotes/<hash>/drive_c/Program Files/MetaTrader 5
```

> **Apple Silicon (M1/M2/M3):** `setup.sh` automatically prepends `arch -x86_64` to all Wine calls. No manual action needed.

**Linux**

```bash
sudo apt install wine64 xvfb          # Debian/Ubuntu
# or
sudo dnf install wine xorg-x11-server-Xvfb   # Fedora/RHEL

# Install MT5 under Wine
wine64 MetaTrader5Setup.exe
```

MT5 prefix auto-detected at:
```
~/.wine/drive_c/Program Files/MetaTrader 5
```

### 3. Configure

```bash
bash scripts/setup.sh          # auto-detects paths and writes config
bash scripts/setup.sh --yes    # non-interactive (CI / fresh machine)
```

`setup.sh` writes `config/mt5-quant.yaml`. To configure manually, copy the example:

```bash
cp config/mt5-quant.example.yaml config/mt5-quant.yaml
```

Minimum required fields:

```yaml
# macOS (MetaTrader 5.app)
wine_executable: "/Applications/MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64"
terminal_dir: "~/Library/Application Support/net.metaquotes.wine.metatrader5/drive_c/Program Files/MetaTrader 5"

# Linux
# wine_executable: "/usr/bin/wine64"
# terminal_dir: "~/.wine/drive_c/Program Files/MetaTrader 5"
```

### 4. Register the MCP server

```bash
# Add to Claude Code (adjust path to where you cloned the repo)
claude mcp add MT5-Quant -- python3 /path/to/mt5-quant/server/main.py

# Or with the venv python explicitly:
claude mcp add MT5-Quant -- /path/to/mt5-quant/.venv/bin/python3 /path/to/mt5-quant/server/main.py
```

`setup.sh` runs this automatically. To check registration:

```bash
claude mcp list
```

Expected output:
```
MT5-Quant: python3 /path/to/mt5-quant/server/main.py
```

**Claude Code integration files** (CLAUDE.md template + baseline hook):
```bash
bash scripts/setup.sh --claude-code
```

### 5. Verify

```bash
bash scripts/platform_detect.sh
```

Expected output (macOS):
```
Wine:    /Applications/MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64
MT5 dir: ~/Library/Application Support/net.metaquotes.wine.metatrader5/.../MetaTrader 5
Display: gui
Arch:    arch -x86_64   ← Apple Silicon only
```

Or from Claude: *"Run verify_setup"*

### 6. Run a backtest

```
Run a backtest on MyEA from 2025.01.01 to 2025.06.30
```

---

## Headless Support

| Platform | `display.mode` | Notes |
|----------|---------------|-------|
| macOS (MT5.app or CrossOver) | `auto` | Wine handles display internally — no Xvfb needed |
| Linux with `$DISPLAY` set | `auto` | Uses existing X11 session |
| Linux VPS / CI (no monitor) | `headless` | Auto-starts Xvfb on `:99` |

**Linux headless setup:**

```bash
sudo apt install xvfb
```

Then set in `config/mt5-quant.yaml`:
```yaml
display:
  mode: headless
  xvfb_display: ":99"
  xvfb_screen: "1024x768x16"
```

`platform_detect.sh` starts Xvfb automatically before each backtest run. To test manually:

```bash
Xvfb :99 -screen 0 1024x768x16 &
DISPLAY=:99 xdpyinfo | grep dimensions
```

---

## MCP Tools (28)

### Core workflow

| Tool | Description |
|------|-------------|
| `run_backtest` | Full pipeline: compile → clean → backtest → extract → analyze |
| `run_optimization` | Genetic optimization (background, returns immediately) |
| `get_optimization_results` | Parse optimization results after MT5 finishes |
| `analyze_report` | Read `analysis.json` from any report directory |
| `compare_baseline` | Compare report vs baseline, return winner/loser verdict |
| `compile_ea` | Compile MQL5 EA via MetaEditor |

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

---

## Architecture

```
AI Agent (Claude / Cursor)
    │ MCP protocol (stdio)
MT5-Quant server (Python)
    │ subprocess
Pipeline scripts (bash)
    │ Wine/CrossOver
MetaTrader 5 (Windows/Wine)
    │
analysis.json ← AI reads this
```

Every backtest produces `analysis.json` — a stable, AI-readable artifact.

The analysis engine is **strategy-agnostic**: a `PROFILES` system drives keyword matching, depth extraction, and cycle grouping so the same pipeline works for any EA type.

| Strategy | CLI / entry point | Depth tracking | Exit keywords | Cycle grouping |
|----------|------------------|----------------|---------------|----------------|
| `grid` (default) | `mt5-analyze-grid` | `Layer #N` → L1–L8+ | locking / cutloss / zombie / timeout | magic + direction, 60 min gap |
| `scalper` | `mt5-analyze-scalper` | — | tp / sl / manual / trailing | magic, 10 min gap |
| `trend` | `mt5-analyze-trend` | — | tp / sl / trailing / breakeven / partial | magic, 4 h gap |
| `hedge` | `mt5-analyze-hedge` | — | tp / sl / net_close / partial | magic + direction, 2 h gap |
| `generic` | `mt5-analyze` | — | tp / sl (profit-sign) | magic, 60 min gap |

**`analysis.json` fields** (strategy name controls what each field contains):

| Field | All strategies | Grid only |
|-------|---------------|-----------|
| `strategy` | Active profile name | — |
| `summary` | KPIs + streaks + cycle win rate + dominant exit | — |
| `monthly_pnl` | P/L, trade count, green flag per month | — |
| `dd_events` | DD events; `cause` uses profile keywords | Cause = locking_cascade / cutloss / zombie_exit |
| `top_losses` | Worst closing deals | `grid_depth_at_close` populated |
| `loss_sequences` | Consecutive losing runs | — |
| `position_pairs` | Hold time, layer, P/L per closed position | — |
| `depth_histogram` | Profile-driven depth counts | L1–L8+ (empty for other strategies) |
| `cycle_stats` | Cycle win rate; grouping + gap from profile | win_rate_by_depth populated |
| `exit_reason_breakdown` | Profile-specific exit reasons | locking / cutloss / zombie / timeout |
| `direction_bias` | Buy vs sell win rate and P/L | — |
| `streak_analysis` | Max win/loss streaks, current streak | — |
| `session_breakdown` | Asian / London / London-NY / New York P/L | — |
| `weekday_pnl` | Mon–Sun P/L and win rate | — |
| `concurrent_peak` | Peak simultaneous open positions | — |
| `hourly_pnl` | Hour 0–23 (`--deep` only) | — |
| `volume_profile` | P/L by lot tier (`--deep` only) | — |

This is what makes AI reasoning over backtest results possible — across any EA type.

Full architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)

---

## Backtest History

Every experiment can be archived into a single JSON ledger at `config/backtest_history.json` — a permanent, queryable record of every run. This lets you delete raw report directories (which can be GBs of tick data) while keeping all results as compact JSON.

```
[workflow]
run_backtest → archive_report(delete_after=true)   # disk reclaimed
              ↓
     backtest_history.json (grows forever, tiny)
              ↓
     get_history(min_profit=5000, max_dd_pct=15)    # query past runs
              ↓
     promote_to_baseline                            # set new production reference
```

**Files** (both gitignored, live in `config/`):

| File | Purpose |
|------|---------|
| `config/backtest_history.json` | Ledger of all archived backtest runs with full metrics, analysis summary, monthly P/L, and verdict |
| `config/baseline.json` | Current production reference — used by `compare_baseline` and the Claude Code hook |

Each history entry captures: all `metrics.json` fields + `analysis.json` summary + monthly P/L array + worst DD event. The raw report directory can be safely deleted after archiving.

**Typical session:**

```
1. run_backtest(expert, from, to)
2. archive_report(delete_after=true, verdict="winner", notes="tight SL test")
3. promote_to_baseline()              ← if this is the new production config
4. get_history(ea="MyEA", verdict="winner")  ← survey all winners later
```

---

## .set File Workflow

```ini
; param=current||start||step||stop||Y   (Y=sweep, N=fixed)
Min_Entry_Confidence=0.610||0.580||0.010||0.650||Y
TP_Pips=400||300||50||500||Y
Max_DD_Percent=15.0||N
```

MT5-Quant handles the undocumented requirements automatically: UTF-16LE encoding, `chmod 444`, OptMode reset, SpreadsheetML XML result parsing.

**Common .set patterns:**

```
# Find and inspect
list_set_files(ea="MyEA")                       → see all variants with combination counts
describe_sweep(path=MyEA_opt.set)               → verify 240 combinations before launching
diff_set_files(path_a=v1.2.set, path_b=v1.3.set) → what changed between versions

# Edit
patch_set_file(path, patches={TP_Pips: 350})    → change one param, keep rest intact
clone_set_file(source, dest, overrides={...})   → new variant from a base in one call

# Post-optimization
get_optimization_results(job_id=X)              → results[0].params = {TP:400, Conf:0.61, ...}
set_from_optimization(                          → write clean backtest .set from those params
  path=MyEA_v1.3.set,
  params=results[0].params,
  template=MyEA_base.set                        → fill non-swept params from existing .set
)
```

---

## Remote Agents (Linux server farm)

Scale optimization throughput by connecting a Linux server as an MT5 agent farm. Linear speedup with agent count.

Setup guide: [docs/REMOTE_AGENTS.md](docs/REMOTE_AGENTS.md)

---

## Claude Code Integration

`--claude-code` generates two files that make Claude aware of your trading context:

```bash
bash scripts/setup.sh --claude-code
```

| File | Purpose |
|------|---------|
| `config/CLAUDE.template.md` | Copy to your EA project root as `CLAUDE.md`. Encodes MT5-Quant rules, baseline tracking policy, and symbol name reminders. |
| `.claude/hooks/user-prompt-submit.sh` | Runs before every Claude prompt. Reads `config/baseline.json` and injects your production metrics as context. |

**Production baseline** (`config/baseline.json`, gitignored):

```json
{
  "symbol": "XAUUSD.cent",
  "period": "2024-01-01/2024-12-31",
  "net_profit": 1250.50,
  "profit_factor": 1.43,
  "max_drawdown_pct": 18.2,
  "sharpe_ratio": 0.87,
  "total_trades": 342,
  "notes": "Best config as of 2024-12-15"
}
```

Update this file whenever a new version is promoted to production — either by running `promote_to_baseline` from Claude, or by editing it manually. Every subsequent Claude prompt will automatically include these metrics so Claude can tell you whether a new backtest is actually an improvement without you having to paste the numbers.

**Why this matters:** Without the baseline hook, you have to manually remind Claude what the production numbers are at the start of every session. With it, that context is always present.

---

## Troubleshooting

Run `verify_setup` from Claude first — it checks all paths and returns actionable hints.

### Wine not found

**macOS:** Confirm `/Applications/MetaTrader 5.app` exists and has been launched at least once. If using CrossOver, confirm the bottle is named correctly.

```bash
# Check what setup.sh found:
bash scripts/platform_detect.sh
```

**Linux:**
```bash
sudo apt install wine64        # Debian/Ubuntu
sudo dnf install wine          # Fedora/RHEL
which wine64                   # confirm it's on PATH
```

### terminal64.exe missing

MT5 unpacks `terminal64.exe` only after its first launch. Open MetaTrader 5.app, wait for initialization (~30 s), then quit. Re-run setup:

```bash
bash scripts/setup.sh --yes
```

### MCP server not appearing in Claude

```bash
claude mcp list                # should show MT5-Quant
claude mcp remove MT5-Quant   # remove stale entry if needed
claude mcp add MT5-Quant -- python3 /absolute/path/to/mt5-quant/server/main.py
```

Use an **absolute path** — relative paths break when Claude starts from a different working directory.

### Report not found after backtest

1. **Wrong symbol name** — brokers use custom names (`XAUUSDm`, `XAUUSD.cent`). Check `verify_setup` → `experts_dir`, or look in `<terminal_dir>/history/` for available symbols.
2. **No history data** — open MT5, open the symbol's chart, wait for history to download, then retry.
3. **EA crash at startup** — check `<terminal_dir>/MQL5/Logs/` for `OnInit` errors.
4. **Date range has no trades** — try a wider range or confirm the symbol was active during that period.

### MetaEditor compile errors

Log at `<terminal_dir>/MQL5/Logs/`. Common causes:
- Missing `#include` files — copy dependencies into `Experts/` alongside the `.mq5`
- Stale `.ex5` from a different MT5 build — delete it and recompile

### No deals in backtest report

- Use `model=0` (every tick) — models 1 and 2 skip intra-bar movement, producing zero deals for grid/martingale EAs
- Check the `.set` file has values appropriate for the symbol/broker
- Confirm `OnInit()` returns `INIT_SUCCEEDED` (MT5 Journal tab)

### Optimization never finishes / no report

```bash
# From Claude:
tail_log(job_id=X, filter=errors)
get_optimization_status(job_id=X)
```

If MT5 crashed mid-run, open `<terminal_dir>/terminal.ini` and remove the line containing `OptMode=-1`, then retry.

---

## License

MIT

---

*Built from battle-tested production infrastructure. Every edge case in the pipeline was hit in production.*
