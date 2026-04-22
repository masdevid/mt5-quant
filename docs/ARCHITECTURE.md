# Architecture Deep Dive

## Design Philosophy

**Deal-level over aggregate.** MT5's built-in HTML report gives you: profit, profit factor, max DD%, trade count. That's it. You cannot tell from those numbers whether a drawdown was caused by overleveraged locking, a bad entry during a news spike, or a grid that reached L8 in a trending market.

MT5-Quant extracts every individual deal — entry price, exit price, P/L, comment string — and reconstructs what was happening when each loss event occurred. The `analysis.json` artifact is the result: AI-readable, stable schema, diffable between runs.

**Pipeline idempotency.** MT5 caches aggressively. Cached `.ex5` binaries, cached `.set` files, stale `terminal.ini` flags. The pipeline exists specifically to invalidate all of these before every run. A backtest result that came from a cached EA binary is wrong in a way that's impossible to detect without the cache clear step.

**Background isolation for optimization.** Genetic optimizations run 2-6 hours. Running them inside a parent process that can be killed (Claude task runner, SSH session, terminal) corrupts the MT5 optimization state. The only correct pattern is `nohup + disown` — fully detached from all parent process trees.

---

## Component Map

```
MT5-Quant/
├── src/
│   ├── main.rs                 # MCP server entry (stdio transport)
│   ├── mcp_server.rs           # MCP protocol handling
│   ├── models/                 # Data structures
│   │   ├── config.rs           # Configuration
│   │   ├── deals.rs            # Deal, PositionPair, DrawdownEvent, etc.
│   │   ├── metrics.rs          # Metrics parsing from HTML/XML
│   │   └── report.rs           # Report, PipelineMetadata, etc.
│   ├── analytics/              # Report extraction & analysis (migrated from Python)
│   │   ├── extract.rs          # HTML/XML report parser → metrics.json + deals.csv
│   │   └── analyze.rs          # Deal-level analysis engine → analysis.json
│   ├── compile/                # MQL5 compilation
│   │   └── mql_compiler.rs   # MetaEditor wrapper (Wine/CrossOver)
│   ├── pipeline/               # Backtest orchestration
│   │   ├── backtest.rs         # 5-stage pipeline (COMPILE→CLEAN→BACKTEST→EXTRACT→ANALYZE)
│   │   └── stages.rs           # Pipeline stage definitions
│   └── tools/                  # MCP tool definitions
│       ├── definitions/        # Tool schemas (9 domain modules, 89 tools)
│       │   ├── mod.rs
│       │   ├── analytics.rs      # 9 analysis tools
│       │   ├── backtest.rs       # 4 backtest tools
│       │   ├── baseline.rs       # 1 baseline tool
│       │   ├── experts.rs        # 4 EA/indicator/script tools
│       │   ├── optimization.rs   # 4 optimization tools
│       │   ├── reports.rs        # 11 report management tools
│       │   ├── setfiles.rs       # 8 .set file tools
│       │   └── system.rs         # 3 system tools
│       └── handlers/             # Tool dispatch (9 domain modules)
│           ├── mod.rs
│           ├── analysis.rs
│           ├── backtest.rs
│           ├── experts.rs
│           ├── optimization.rs
│           ├── reports.rs
│           ├── setfiles.rs
│           └── system.rs
│
├── scripts/
│   ├── setup.sh                # Auto-detect Wine/MT5, write config, register MCP
│   ├── platform_detect.sh      # Wine path + headless detection
│   ├── build-rust.sh           # Rust build script
│   └── optimize.sh             # Genetic optimization launcher (nohup + disown)
│
├── analytics/                    # Legacy Python (reference only)
│   ├── extract.py
│   ├── analyze.py
│   └── optimize_parser.py
│
├── config/
│   ├── mt5-quant.example.yaml  # Template config
│   └── mt5-quant.yaml          # Live config (gitignored)
│
└── docs/
    ├── ARCHITECTURE.md         # This file
    ├── MCP_TOOLS.md            # Full tool spec
    └── REMOTE_AGENTS.md        # Linux agent farm setup
```

---

## Pipeline Stages

### Stage 1: COMPILE

```rust
// src/compile/mql_compiler.rs
let compiler = MqlCompiler::new(config);
let result = compiler.compile("src/experts/MyEA.mq5")?;
```

Invokes MetaEditor via Wine with the MQL5 source file. Copies resulting `.ex5` to the MT5 Experts directory. Fails the pipeline on compile errors.

**Why not skip this?** MT5 caches the `.ex5` binary by filename. If you edit your EA and re-run without recompiling, MT5 runs the old binary silently. Always compile.

---

### Stage 2: CLEAN

```bash
rm -f "${MT5_TESTER_DIR}/cache/*.tst"
rm -f "${MT5_PROFILES_DIR}/Tester/${EXPERT}.set"
```

Clears:
- Tester cache (`.tst` files): compiled test results MT5 reuses to skip re-running ticks
- Cached `.set` file: MT5 writes the current parameter values here after each run; if stale, next run picks up wrong params

**The `.set` encoding trap:** MT5 reads parameter files as UTF-16LE with BOM. It writes them back as UTF-16LE. If you provide a UTF-8 `.set` file for optimization, MT5 reads the parameters correctly (it tries multiple encodings) but when it writes the optimization variants, it uses UTF-16LE and **strips the `||Y` optimization flags**. Every subsequent pass uses the base value. Your 500-combination optimization runs 500 identical backtests.

Solution: always write `.set` files as UTF-16LE with BOM, mark them read-only before MT5 starts.

```python
# Python: write UTF-16LE with BOM
content = "\n".join(lines)
with open(set_path, 'w', encoding='utf-16-le') as f:
    f.write('\ufeff')  # BOM
    f.write(content)
os.chmod(set_path, 0o444)  # read-only
```

---

### Stage 3: BACKTEST

```bash
arch -x86_64 "${WINE}" cmd.exe /c 'C:\_backtest.bat'
```

The batch file sets MT5 CLI flags and launches `terminal64.exe`:

```bat
"C:\Program Files\MetaTrader 5\terminal64.exe" /config:C:\backtest.ini
```

`backtest.ini` contains:
```ini
[Tester]
Expert=MyEA
Symbol=XAUUSD
Period=M5
Deposit=10000
Currency=USD
Leverage=500
Model=0
FromDate=2025.01.01
ToDate=2025.06.30
Report=C:\report
Optimization=0
```

MT5 runs in headless mode, writes the report, and exits.

**Process isolation note:** On macOS with CrossOver, `arch -x86_64` is required — CrossOver ships arm64 Wine wrappers that don't support the x86_64 MT5 binary correctly. Without it, MT5 appears to start but produces no output.

---

### Stage 4: EXTRACT

Single HTML/XML parse pass that produces three artifacts:

```rust
// src/analytics/extract.rs
let extractor = ReportExtractor::new();
let result = extractor.extract(&report_path, &output_dir)?;
// → metrics.json  (aggregate summary)
// → deals.csv     (all deals, 13 columns)
// → deals.json    (same data, JSON)
```

**Why single-pass?** MT5 HTML reports are large (1-5MB for 14-month tests). Each regex pass over the file takes ~200ms. The old pipeline ran 5 separate grep/regex passes. The Rust implementation uses a single-pass parser: 5× faster and no partial-read inconsistencies.

**Format detection:**
```rust
// MT5 Build 48+ saves SpreadsheetML XML, not HTML
let ext = Path::new(&path).extension()
    .and_then(|e| e.to_str())
    .unwrap_or("");

if ext == "xml" || path.ends_with(".htm.xml") {
    // Parse as SpreadsheetML XML
    let doc = roxmltree::Document::parse(&text)?;
} else {
    // Parse as HTML with regex
}
```

**Deal columns (13):**
```
Time | Type | Direction | Volume | Price | S/L | T/P | Profit | Balance | Comment | Order | Magic | Entry
```

The `Comment` column is the key to grid analytics. The EA writes `"Layer #3"`, `"Locking Total"`, `"Zombie Exit"` etc. Pattern matching on comments reconstructs which position was at which layer.

---

### Stage 5: ANALYZE

```rust
// src/analytics/analyze.rs
let analyzer = DealAnalyzer::new();
let result = analyzer.analyze(&deals, &metrics, strategy, deep)?;
// → analysis.json
```

All functions operate on the parsed deal data — no MT5 or Wine required.

**Strategy profiles** (defined in `analyze.rs`):
- `grid` — Layer depth tracking, locking/cutloss/zombie keywords
- `scalper` — TP/SL/manual/trailing exit classification
- `trend` — TP/SL/trailing/breakeven/partial exits
- `hedge` — TP/SL/net_close/partial, magic+direction grouping
- `generic` — Simple profit-based TP/SL classification

#### Strategy profiles

The analysis engine is driven by a `PROFILES` dict. Each profile controls:

| Field | Type | Controls |
|-------|------|----------|
| `depth_re` | regex or `None` | Whether/how to extract depth from comments |
| `exit_keywords` | `{reason: [kw]}` | Comment patterns for exit classification |
| `dd_cause_keywords` | `{cause: [kw]}` | Comment patterns for DD cause classification |
| `cycle_group_by` | `'magic'` or `'magic+direction'` | How deals are grouped into cycles |
| `cycle_gap_min` | int | Minutes between opens that mark a new cycle |

Built-in profiles:

| Profile | `depth_re` | `cycle_group_by` | `cycle_gap_min` | Exit keywords |
|---------|-----------|-----------------|----------------|---------------|
| `generic` | — | `magic` | 60 | profit-sign only (tp/sl) |
| `grid` | `Layer #N` | `magic+direction` | 60 | locking, cutloss, zombie, timeout |
| `scalper` | — | `magic` | 10 | tp, sl, manual, trailing |
| `trend` | — | `magic` | 240 | breakeven, trailing, partial, tp, sl |
| `hedge` | — | `magic+direction` | 120 | tp, sl, net_close, partial |

#### Entry points (after `pip install -e .`)

```bash
mt5-analyze         deals.csv   # generic
mt5-analyze-grid    deals.csv   # grid / martingale (default in pipeline)
mt5-analyze-scalper deals.csv   # scalper
mt5-analyze-trend   deals.csv   # trend following
mt5-analyze-hedge   deals.csv   # hedging
```

#### Analytics functions

**Core (always run, strategy-agnostic):**

| Function | What it computes |
|----------|-----------------|
| `monthly_pnl` | P/L, trade count, green flag per calendar month |
| `reconstruct_dd_events` | Balance curve → local minima; cause from profile keywords |
| `top_losses` | Worst individual closing deals by P/L |
| `loss_sequences` | Consecutive losing closed deals (runs of length ≥ 2) |
| `position_pairs` | Match in/out by order ticket → hold time, depth at close |
| `direction_bias` | Buy vs sell win rate, total P/L, average trade |
| `streak_analysis` | Max consecutive win/loss streaks; current streak |
| `session_breakdown` | Asian (00–08h) / London (08–13h) / London-NY (13–17h) / New York (17–22h) |
| `weekday_pnl` | Mon–Sun P/L and win rate |
| `concurrent_peak` | Peak simultaneous open positions |

**Strategy-driven (output varies by profile):**

| Function | Generic | Grid | Scalper/Trend/Hedge |
|----------|---------|------|---------------------|
| `depth_histogram` | `{}` (empty) | L1–L8+ counts | `{}` (no `depth_re`) |
| `cycle_stats` | magic, 60-min gap | magic+direction, 60-min gap | per-profile config |
| `exit_reason_breakdown` | tp / sl | locking / cutloss / zombie / timeout | profile-specific |

**Deep analytics (`--deep` flag):**

| Function | What it computes |
|----------|-----------------|
| `hourly_pnl` | Hour-by-hour (0–23) P/L and win rate |
| `volume_profile` | P/L breakdown by lot size tier |

**DD event reconstruction:**
1. Walk deals chronologically, track running balance
2. At each local minimum (DD > 1%), record timestamp, depth (%), recovery date
3. Classify `cause` using `profile['dd_cause_keywords']`; returns `"unknown"` for generic/unmatched

**Cycle statistics:**
Deals are grouped by `cycle_group_by` key. A gap greater than `cycle_gap_min` between consecutive opens marks a new cycle boundary. Win rate is computed per cycle (not per deal), then broken down by max depth reached.

**Exit reason classification:**
Iterates `exit_keywords` in definition order — more specific patterns must appear before general ones to avoid substring false-positives (e.g. `"stop"` inside `"breakeven stop"`). Falls back to profit-sign if no keyword matches.

**Loss sequence detection:**
Consecutive closed deals where P/L < 0 (minimum length 2). Captures clusters of losses better than any single worst-trade metric.

---

## Optimization Pipeline

### Why `nohup + disown` is mandatory

```bash
nohup ./scripts/optimize.sh ... > /tmp/opt.log 2>&1 & disown
```

MT5 optimization uses Unix signals to coordinate between `terminal64.exe` (master) and `metatester64.exe` instances (workers). When the parent process tree is killed:

1. `SIGHUP` propagates to child processes
2. `metatester64.exe` workers receive the signal and terminate
3. The master `terminal64.exe` detects worker failure and aborts the optimization
4. `terminal.ini` is left with `OptMode=-1`, requiring manual reset before next run

`nohup` prevents `SIGHUP` propagation. `disown` removes the process from the shell's job table so it's not killed when the shell exits. Both are required.

---

### `OptMode` state machine

`terminal.ini` contains an `OptMode` key that MT5 uses to track optimization state:

| `OptMode` value | Meaning |
|----------------|---------|
| `0` | Normal backtest mode (ready) |
| `1` | Optimization in progress |
| `2` | Optimization complete — show results |
| `-1` | Optimization aborted / crashed |

After any optimization run (complete or aborted), MT5 writes `-1` or `2`. On next launch with `Optimization=2` in `backtest.ini`, MT5 reads `OptMode=-1` and exits immediately without running.

**Fix:** Before every optimization launch, force `OptMode=0` in `terminal.ini`:

```bash
sed -i 's/OptMode=.*/OptMode=0/' "${MT5_DIR}/terminal.ini"
# Also remove LastOptimization line if present
sed -i '/LastOptimization=/d' "${MT5_DIR}/terminal.ini"
```

---

## Remote Agent Architecture

MT5's distributed testing works via a custom TCP protocol. The master `terminal64.exe` listens on a port. Remote agents (`metatester64.exe`) connect and receive test configurations.

```
Mac (master)                    Linux server (agents)
terminal64.exe                  metatester64.exe × N
    │                                   │
    └──── TCP:3000 ─────────────────────┘
```

**Linux setup:**
```bash
# On Linux server (Wine required)
wine metatester64.exe /server:MAC_IP:3000 /agents:8
```

MT5 shows remote agents in the agent manager as `Agent-0.0.0.0-PORT` entries when listening, and activates them when the remote `metatester64.exe` connects.

**Throughput:** Linear scaling with agent count. 10 local + 16 remote = 26 agents. A 17,000-combination optimization that takes 3 hours locally completes in ~70 minutes.

---

## Headless Operation

MT5-Quant uses MT5's CLI mode (`terminal64.exe /config:backtest.ini`) — no user interaction, no clicking in the Strategy Tester GUI. Whether this is truly "headless" depends on platform:

| Platform | Status | Notes |
|----------|--------|-------|
| macOS + CrossOver | Near-headless | CrossOver manages the display internally. MT5 window may flash briefly or be suppressed entirely depending on bottle settings. No monitor required in practice. |
| Linux + Wine | Requires Xvfb | Wine needs an X11 display connection. Without one, `wine64 terminal64.exe` fails with `cannot open display`. |
| Linux + Wine + Xvfb | Full headless | Virtual framebuffer satisfies Wine's X11 requirement. Use on servers with no monitor. |

**Linux headless setup (Xvfb):**

```bash
# Install Xvfb
sudo apt install xvfb

# Start virtual display on :99
Xvfb :99 -screen 0 1024x768x16 &
export DISPLAY=:99

# Now Wine can launch MT5 without a physical display
wine64 terminal64.exe /config:backtest.ini
```

**Persistent virtual display (systemd):**

```ini
# /etc/systemd/system/xvfb.service
[Unit]
Description=Virtual Display for MT5

[Service]
ExecStart=/usr/bin/Xvfb :99 -screen 0 1024x768x16
Restart=always

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable xvfb
sudo systemctl start xvfb
```

Then set `DISPLAY=:99` in MT5-Quant's environment config.

**Note:** `metatester64.exe` (the agent worker process) is fully headless — it runs tick simulation with no display requirement. Only the master `terminal64.exe` needs a display to orchestrate the session. On Mac with CrossOver this is handled transparently.

---

## Known Limitations

**macOS-specific:**
- Requires Wine. The native MT5.app from the Mac App Store (or MetaQuotes CDN) ships bundled Wine at `MetaTrader 5.app/Contents/SharedSupport/wine/`. CrossOver is an alternative.
- `arch -x86_64` required on Apple Silicon.
- File paths must go through Wine's virtual filesystem (`C:\` = inside the Wine prefix `drive_c/`).

**Report format dependency:**
- SpreadsheetML XML format (`.htm.xml`) has no documented schema from MetaQuotes. The parser is reverse-engineered from observed output. May break on future MT5 builds.

**Comment-based analytics:**
- Strategy-specific analytics (depth histogram, exit reason, DD cause) depend on EA comment strings. EAs that don't write structured comments will get `generic` profile results — summary metrics, session breakdown, streaks, and direction bias all still work; only keyword-classified fields fall back to `"unknown"` or profit-sign.
- Custom comment patterns can be supported by adding a new entry to `PROFILES` in `analytics/analyze.py` — no other code changes needed.

**Single MT5 instance:**
- MT5 is single-instance per Windows drive. Two backtests cannot run simultaneously on the same Wine prefix. Parallelism requires multiple Wine prefixes (separate installations).

---

## Claude Code Integration

`setup.sh --claude-code` generates two files that give Claude persistent context about the user's trading setup:

### `config/CLAUDE.template.md`

A project-level CLAUDE.md template the user copies to their EA project root. Encodes:
- MT5-Quant tool names and when to use them
- Baseline tracking policy (never call something an improvement without comparing to `baseline.json`)
- Symbol name reminder (broker-specific suffix matters — `XAUUSD.cent` ≠ `XAUUSD`)
- Backtest and optimization constraints (model 0, single instance, UTF-16LE .set files)

### `.claude/hooks/user-prompt-submit.sh`

A Claude Code hook that runs before every prompt submission. Reads `config/baseline.json` and outputs a JSON context block:

```json
{"context": "## Production Baseline (config/baseline.json)\n..."}
```

Claude Code injects this into the system context for every conversation turn. The result: Claude always knows the current production metrics without the user having to paste them.

**Hook execution path:**
```
User types prompt
    → user-prompt-submit.sh executes
    → reads config/baseline.json
    → outputs {"context": "..."} to stdout
    → Claude Code prepends to system context
    → Claude sees baseline in every prompt
```

**Graceful degradation:** If `baseline.json` doesn't exist or is malformed, the hook exits 0 silently — no prompt is blocked. The baseline section simply doesn't appear until the user creates the file.
