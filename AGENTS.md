# AGENTS.md — Developer instructions for MT5-Quant

## Project Overview

Open-source MCP server for MetaTrader 5 backtesting, optimization, and analytics — written entirely in Rust. Exposes 89 stdio MCP tools for compiling MQL5 EAs, running backtests via Wine, extracting deal-level data, organizing reports in SQLite, and performing 19 dimensions of analysis.

Target users: MQL developers on macOS (CrossOver or native MetaTrader5.app) and Linux (Wine/Xvfb).

## Repository Layout

```
MT5-Quant/
├── src/
│   ├── main.rs                 # CLI entry + stdio MCP transport
│   ├── mcp_server.rs           # McpServer struct, tool dispatch, init/notification
│   ├── mt5.rs                  # Wine/MT5 executable launchers
│   ├── models/
│   │   ├── mod.rs
│   │   ├── config.rs           # Config, CurrentAccount, wine detection, .set helpers
│   │   ├── deals.rs            # Deal, PositionPair, DrawdownEvent
│   │   ├── metrics.rs          # BacktestMetrics parsing (HTML + XML formats)
│   │   └── report.rs           # ReportEntry, PipelineMetadata, BacktestJob, status
│   ├── analytics/
│   │   ├── mod.rs
│   │   ├── extract.rs          # HTML/XML report → Deal[] + metrics
│   │   └── analyze.rs          # DealAnalyzer: 19+ analysis methods
│   ├── compile/
│   │   ├── mod.rs
│   │   └── mql_compiler.rs     # MetaEditor compiler via Wine
│   ├── pipeline/
│   │   ├── mod.rs
│   │   ├── backtest.rs         # 5-stage pipeline: COMPILE→CLEAN→BACKTEST→EXTRACT→ANALYZE
│   │   └── stages.rs           # PipelineStage enum
│   ├── optimization/
│   │   ├── mod.rs
│   │   ├── optimizer.rs        # Background optimization launcher
│   │   └── parser.rs           # Optimization result XML → best params
│   ├── storage/
│   │   ├── mod.rs
│   │   └── database.rs         # ReportDb: reports + deals SQLite tables, queries
│   ├── utils/                  # (new) shared utilities — growing directory
│   └── tools/
│       ├── mod.rs              # MCP definitions aggregation
│       ├── definitions/        # Tool schemas (JSON Schema) — 11 modules, 90 tools
│       │   ├── mod.rs           # exports: all_tools(), system_tools, etc.
│       │   ├── analytics.rs     # 19 analysis tools + compare_baseline
│       │   ├── backtest.rs      # 7 backtest tools
│       │   ├── baseline.rs      # 1 baseline tool
│       │   ├── experts.rs       # 9 EA/indicator/script tools
│       │   ├── optimization.rs  # 4 optimization tools
│       │   ├── reports.rs       # 20 report management tools
│       │   ├── setfiles.rs      # 8 .set file tools
│       │   ├── system.rs        # 6 system/health tools + version update
│       │   ├── update.rs        # update tool definition
│       │   └── utility.rs       # 10 pre-flight/compat tools
│       └── handlers/            # Tool dispatch functions — 11 modules
│           ├── mod.rs           # ToolHandler struct, dispatch table (89+ cases)
│           ├── analysis.rs      # analytics/analyze dispatch
│           ├── backtest.rs      # pipeline/backtest dispatch
│           ├── experts.rs       # wine + WalkDir helpers: scan_mql_dir, copy_mql_to_project
│           ├── optimization.rs  # optimizer dispatch
│           ├── reports.rs       # ReportDb + ReportEntry dispatch
│           ├── setfiles.rs      # UTF-16LE .set read/write/patch helpers
│           ├── system.rs        # healthcheck, version update
│           ├── utility.rs       # pre-flight, diagnostics, init_project helpers
│           └── update.rs        # update tool handler
├── scripts/
│   ├── setup.sh                # Auto-detect Wine/MT5, write config (1271 lines)
│   ├── platform_detect.sh      # Wine path + headless detection
│   ├── build-rust.sh           # cargo build --release (25 lines)
│   ├── release.sh              # Version bump + changelog + tag + push (238 lines)
│   └── optimize.sh             # Legacy optimization driver (pending full Rust migration)
├── analytics/                  # Legacy Python (reference only — do not modify)
├── config/
│   ├── mt5-quant.example.yaml # Template user copies to mt5-quant.yaml
│   └── mt5-quant.yaml         # Generated config (gitignored)
├── tests/
│   ├── test_mcp.py            # End-to-end MCP tests (stdio-driven)
│   ├── integration_test.sh    # Shell-based integration tests
│   └── fixtures/              # Sample reports + CSVs for testing
├── server.json                # MCP registry manifest (tracked)
├── Cargo.toml                 # version = 1.32.4
└── CHANGELOG.md
```

## Key Design Constraints

- **Headless/GUI**: `platform_detect.sh` auto-selects. macOS CrossOver = GUI (CrossOver abstracts display). Linux without `$DISPLAY` = Xvfb. Controlled by `display.mode` in config (`auto` | `gui` | `headless`).
- **Single MT5 instance**: Never run two backtests in parallel on the same Wine prefix.
- **Model 0 only for optimization**: Model 1 overfits martingale/grid EAs — intra-bar movement not simulated. Use `model=0` (every tick).
- **UTF-16LE .set files**: MT5 strips `||Y` flags from UTF-8. All `.set` writes use UTF-16LE + `chmod 444`. See `setfiles.rs` helpers.
- **OptMode reset**: After any optimization, `terminal.ini` gets `OptMode=-1`. Must reset to `0` before next backtest (handled in `backtest.rs` cleanup stage).
- **Report format detection**: MT5 Build 48+ writes SpreadsheetML XML (`.htm.xml`), not HTML. Both formats handled in `extract.rs` — always check for the XML variant first.
- **EA-agnostic**: Never hardcode magic numbers, strategy names, or assume specific EA behavior. All code must work with any MQL5 EA.

## Architecture

1. **CLI (`main.rs`)** — parses CLi args (`--stdio`, `--port`, `--test-launch`), initializes `McpServer`, runs stdio or TCP transport loop, handles JSON-RPC requests.

2. **McpServer (`mcp_server.rs`)** — wraps `ToolHandler` with `Arc<Mutex>`, handles `initialize`/`notifications`/`tool/error` method routing. Auto-verifies Wine/MT5 on first call.

3. **ToolHandler (`tools/handlers/mod.rs`)** — dispatch table mapping 89 tool names to handler functions. Each handler module is self-contained.

4. **Definitions (`tools/definitions/`)** — JSON Schema objects for tool input/output. Never executes logic.

5. **Analytics (`analytics/`)** — two core structs:
   - `ReportExtractor` — reads HTML/XML report + tester log → `Vec<Deal>` + `BacktestMetrics`
   - `DealAnalyzer` — takes `Vec<Deal>` → 19 analysis methods (monthly PnL, drawdown events, streaks, etc.)

6. **Pipeline (`pipeline/backtest.rs`)** — `BacktestPipeline` struct with 5 stages:
   - `COMPILE` — `MqlCompiler::compile()` via MetaEditor
   - `CLEAN` — clean cache, reset OptMode
   - `BACKTEST` — launch MT5 tester via Wine, poll status
   - `EXTRACT` — parse HTML/XML → deals + metrics
   - `ANALYZE` — run all analytics, write to SQLite

7. **Storage (`storage/database.rs`)** — `ReportDb` with two tables:
   - `reports` — metrics, set file paths, verdict, tags, notes
   - `deals` — individual deal rows, keyed by `report_id`
   - `CREATE TABLE IF NOT EXISTS` in `init()` — idempotent on startup

## Building

```bash
bash scripts/build-rust.sh
# or
cargo build --release
```

Binary: `target/release/mt5-quant`

## Testing

- **Unit/integration:** `cargo test`
- **End-to-end MCP:** `python3 tests/test_mcp.py` — drives the MCP server over stdio (full backtest + all analytics)
- **Analytics only:** No MT5/Wine needed; loads deals from SQLite DB via `db.get_deals(report_id)`
- **Symbol note:** Testing account uses `XAUUSDc` (broker suffix required), not `XAUUSD`

## Configuration

Users copy `config/mt5-quant.example.yaml` → `config/mt5-quant.yaml` (gitignored).
Run `bash scripts/setup.sh` to auto-detect Wine, MT5 paths, architecture, and write config.

Minimum config:
```yaml
wine_executable: "/path/to/wine64"
terminal_dir: "/path/to/MetaTrader 5"
```

Optional:
```yaml
defaults:
  symbol: "XAUUSD.cent"
  timeframe: "M5"
  deposit: 10000
display:
  mode: auto               # auto | gui | headless
reports_dir: "./reports"
experts_dir: "~/.../MQL5/Experts"
```

## Developing New Tools

Each new tool follows a 4-file pattern:

1. **Schema** — Add tool input/output to `tools/definitions/{domain}.rs` via `Tool::new(name) {...}` with `Input::schema` and `Output::schema`.
2. **Export** — Add to `tools/definitions/mod.rs`:
   - Include in `all_tools()` array
   - Add to domain exports if creating new domain
3. **Handler** — Add function in `tools/handlers/{domain}.rs`:
   ```rust
   pub async fn handle_tool_name(config: &Config, args: &Value) -> Result<Value> {
       let required = required_str(args, "param_name")?;
       ok_response(json! { "result": "..." })
   }
   ```
4. **Dispatch** — Add case in `handlers/mod.rs` match arm under `ToolHandler::handle()`.

### Handler Helper Functions (use instead of inline logic)

| Helper | Purpose |
|--------|---------|
| `required_str(args, key)` | Extract required `&str` or return error |
| `ok_response(data)` / `err_response(msg)` | Wrap in MCP content envelope |
| `resolve_report(args)` | → `(deals, metrics, report_dir)` — `report_id > report_dir > latest` |
| `prepare_analysis(args)` | → `(deals, metrics, analyzer, report_dir)` — wraps resolve_report |
| `scan_mql_dir(dir, filter, type_label)` | WalkDir with optional filter for list/search |
| `copy_mql_to_project(config, args, fallback)` | Shared copy logic for indicator/script |

Extend helpers when adding new handlers.

## Commit Style

Conventional commits: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `release:`
Keep commits focused — one logical change per commit.

## Release Process

```bash
bash scripts/release.sh <patch|minor|major|X.Y.Z> [--yes]
```

Script handles:
1. Bump `Cargo.toml` + `Cargo.lock` version
2. Update `server.json` (version + SHA256 placeholder)
3. Generate CHANGELOG entry from git log
4. `cargo check --quiet`
5. Commit `release: vX.Y.Z`
6. Tag `vX.Y.Z`
7. Push — CI (`release.yml`) triggered:
   - Build macOS arm64 + Linux x64 binaries
   - Publish to crates.io
   - Create GitHub Release with artifacts
   - Build MCP package, compute SHA256
   - Update `server.json` SHA256, push
   - Publish to MCP Registry

## Deal Storage

- Deals stored in `deals` SQLite table, keyed by `report_id`. **No `deals.csv` written automatically.**
- `db.insert_deals(report_id, &[Deal])` called in pipeline after extraction.
- `db.get_deals(report_id)` loads deals for analytics. `db.get_by_report_dir(path)` resolves by filesystem path.
- `export_deals_csv` tool writes CSV on demand.

## DB Schema Evolution

Add new tables/columns in `ReportDb::init()` using `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` — runs on every startup, is idempotent. No migration files needed.

## Wine / MT5 Notes

- Wine executable paths differ across platforms:
  - macOS MT5.app: `/Applications/MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64`
  - macOS CrossOver: `/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine64`
  - Linux: `/usr/bin/wine64` (or first `wine64` on PATH)
- MT5 terminal_dir is auto-resolved from wine_executable path.
- Apple Silicon: append `arch -x86_64` prefix for x86 Wine binaries (auto-detected).

## Common Pitfalls

- **M1/Memory issues**: Wine on Apple Silicon runs x86 via Rosetta 2. Watch memory pressure.
- **Model mismatch**: Grid/martingale EAs need `model=0` (every tick). Model 1/2 will produce zero deals.
- **Set file encoding**: Always use UTF-16LE with `chmod 444`. UTF-8 strips `||Y` flags.
- **OptMode=-1**: Must clear after optimization. Handled in pipeline cleanup stage.
- **Report not found**: Check `<terminal_dir>/history/` for correct symbol name (broker suffix matters: `XAUUSDc` vs `XAUUSD`).
- **Journal-only backtest**: `ShutdownTerminal=1` doesn't exit MT5 on Wine. Inactivity watchdog (in `launch_backtest`) handles this with `inactivity_kill_secs` param.
