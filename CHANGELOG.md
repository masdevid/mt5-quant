# Changelog

## [1.32.4] — 2026-04-25

### Fixed
- **HTML report extraction**: `ShutdownTerminal=1` does not cause `terminal64.exe` to exit on Wine/macOS.
  Inactivity watchdog now waits 30 s for the report file after the tester log goes quiet, then kills
  MT5 unconditionally — no longer depends on natural process exit.
- **Duplicate deals**: tester agent log writes each deal twice; deduplicated via `HashSet<deal_number>`
  in `parse_journal_deals`.
- **Entry direction inference**: all journal deals were marked `entry="in"` because no direction info
  exists in the log. Fixed with a per-symbol position tracker (signed lot accumulation) to infer
  "in" / "out" correctly.
- **`list_deals` returning 0 results**: `is_closed_trade()` was gating on `profit != 0.0`; journal
  deals legitimately have zero profit. Removed the profit gate.
- **Wrong tester log selected**: `Agent-0.0.0.0` logs only contain startup lines, not deal lines.
  `find_active_tester_agent_log` now prefers `Agent-127.0.0.1` and tiebreaks by file size.

### Added
- `launch_backtest` exposes `shutdown` (default `true`) and `inactivity_kill_secs` (default: disabled;
  set to e.g. `120` to enable the inactivity watchdog) as explicit tool parameters.
- Report DB stores deals in SQLite; all analytics tools resolve by `report_id` / `report_dir` / latest.

### Verified
- 1-month DPS21/XAUUSD.cent/M5 backtest produces full HTML report: win_rate=70%,
  profit_factor=0.77, sharpe=-3.61, max_dd=6.93%. All 17 analytics tools return real data.


## [1.31.5] — 2026-04-22

- feat: add check_update and update tools with background auto-check
- release: v1.31.4
- fix: update all scripts for correctness and consistency
- release: v1.31.3
- docs: clean up public repo — remove IDE files, fix stale refs
- release: v1.31.2
- refactor: reduce handler boilerplate in analysis and experts modules
- fix: registryType mcpbPackageType → mcpb


## [1.31.4] — 2026-04-22

- fix: update all scripts for correctness and consistency
- release: v1.31.3
- docs: clean up public repo — remove IDE files, fix stale refs
- release: v1.31.2
- refactor: reduce handler boilerplate in analysis and experts modules
- fix: registryType mcpbPackageType → mcpb


## [1.31.3] — 2026-04-22

- docs: clean up public repo — remove IDE files, fix stale refs
- release: v1.31.2
- refactor: reduce handler boilerplate in analysis and experts modules
- fix: registryType mcpbPackageType → mcpb


## [1.31.2] — 2026-04-22

- refactor: reduce handler boilerplate in analysis and experts modules
- fix: registryType mcpbPackageType → mcpb


## [1.31.1] — 2026-04-22

- Minor improvements and bug fixes

