# MCP Tool Specification

Full input/output schemas for MT5-Quant tools.

> **Documentation Status:** This file documents 56 of 89 total tools. Missing:
> - `list_experts`, `list_indicators`, `list_scripts`
> - `healthcheck`, `list_symbols`
> - Reports query: `search_reports`, `get_latest_report`, `list_reports`, `prune_reports`, `tail_log`, `get_report_by_id`, `get_reports_summary`, `get_best_reports`, `search_reports_by_tags`, `search_reports_by_date_range`, `search_reports_by_notes`, `get_reports_by_set_file`, `get_comparable_reports`
> - Granular analytics: `analyze_monthly_pnl`, `analyze_drawdown_events`, `analyze_top_losses`, `analyze_loss_sequences`, `analyze_position_pairs`, `analyze_direction_bias`, `analyze_streaks`, `analyze_concurrent_peak`
> - Deal analytics: `list_deals`, `search_deals_by_comment`, `search_deals_by_magic`, `analyze_profit_distribution`, `analyze_time_performance`, `analyze_hold_time_distribution`, `analyze_layer_performance`, `analyze_volume_vs_profit`, `analyze_costs`, `analyze_efficiency`
> - Archive/history tools: `archive_report`, `archive_all_reports`, `get_history`, `annotate_history`, `promote_to_baseline`
> - Experts search: `search_experts`, `search_indicators`, `search_scripts`, `copy_indicator_to_project`, `copy_script_to_project`
> - Debugging/diagnostics: `diagnose_wine`, `get_mt5_logs`, `search_mt5_errors`, `check_mt5_process`, `kill_mt5_process`, `check_system_resources`, `validate_mt5_config`, `get_wine_prefix_info`, `get_backtest_crash_info`

---

## `run_backtest`

Run a complete backtest pipeline: compile → clean cache → backtest → extract → analyze.

**When to call:** Any time you need fresh backtest results. Always runs the full pipeline unless `skip_*` flags are set.

### Input schema

```typescript
{
  // Required
  expert: string;          // EA name without path or extension. e.g. "MyEA_v1.2"

  // Date range — use either preset OR from+to
  preset?: "last_month" | "last_3months" | "ytd" | "last_year";
  from?: string;           // "YYYY-MM-DD"
  to?: string;             // "YYYY-MM-DD"

  // Optional overrides
  symbol?: string;         // Default from config. e.g. "XAUUSD"
  timeframe?: "M1" | "M5" | "M15" | "M30" | "H1" | "H4" | "D1"; // Default: M5
  deposit?: number;        // Default from config. e.g. 10000
  currency?: string;       // Default: "USD"
  model?: 0 | 1 | 2;      // 0=every tick (default), 1=1min OHLC, 2=open price
  set_file?: string;       // Path to .set file. If omitted, uses EA defaults.
  leverage?: number;       // Default: 500

  // Pipeline flags
  skip_compile?: boolean;  // Skip EA compilation (use existing .ex5)
  skip_clean?: boolean;    // Skip cache clean (faster but risks stale cache)
  skip_analyze?: boolean;  // Extract only, skip deal analysis
  deep_analyze?: boolean;  // Add hourly_pnl and volume_profile to analysis.json
  strategy?: "grid" | "scalper" | "trend" | "hedge" | "generic";
                           // Analysis strategy profile (default: "grid").
                           // Controls depth tracking, exit keywords, and cycle grouping.
}
```

### Output schema

```typescript
{
  success: boolean;
  report_dir: string;       // "reports/20250619_143022_MyEA_XAUUSD_M5"
  duration_seconds: number;

  // Inline summary from metrics.json (always present on success)
  metrics: {
    net_profit: number;
    profit_factor: number;
    max_dd_pct: number;
    sharpe_ratio: number;
    total_trades: number;
    recovery_factor: number;
    expected_payoff: number;
    gross_profit: number;
    gross_loss: number;
    win_rate_pct: number;
    avg_profit: number;
    avg_loss: number;
  };

  // Deal analysis summary (present unless skip_analyze=true)
  analysis_summary: {
    green_months: number;
    total_months: number;
    worst_month: string;        // "2025-10"
    worst_month_pnl: number;
    worst_dd_event_pct: number;
    worst_dd_date: string;
    max_grid_depth: number;     // highest layer reached in any cycle
    l5_plus_count: number;      // cycles that reached L5+
  };

  // File paths for direct reading
  files: {
    metrics_json: string;
    analysis_json: string;
    deals_csv: string;
    deals_json: string;
  };

  error?: string;  // Present on failure
}
```

### Example

```json
// Input
{
  "expert": "MyEA_v1.2",
  "from": "2025-01-01",
  "to": "2025-06-30",
  "deposit": 10000,
  "model": 0
}

// Output
{
  "success": true,
  "report_dir": "reports/20250619_143022_MyEA_XAUUSD_M5",
  "duration_seconds": 287,
  "metrics": {
    "net_profit": 4832.10,
    "profit_factor": 1.54,
    "max_dd_pct": 12.3,
    "sharpe_ratio": 1.18,
    "total_trades": 891
  },
  "analysis_summary": {
    "green_months": 5,
    "total_months": 6,
    "worst_month": "2025-03",
    "worst_month_pnl": -412.80,
    "worst_dd_event_pct": 12.3,
    "worst_dd_date": "2025-03-14",
    "max_grid_depth": 6,
    "l5_plus_count": 8
  },
  "files": {
    "metrics_json": "reports/20250619_143022_MyEA_XAUUSD_M5/metrics.json",
    "analysis_json": "reports/20250619_143022_MyEA_XAUUSD_M5/analysis.json",
    "deals_csv": "reports/20250619_143022_MyEA_XAUUSD_M5/deals.csv",
    "deals_json": "reports/20250619_143022_MyEA_XAUUSD_M5/deals.json"
  }
}
```

---

## `run_backtest_quick`

Quick backtest using pre-compiled EA: clean cache → backtest → extract → analyze.

**When to call:** When EA code hasn't changed and you just want to test different parameters or date ranges. Faster than `run_backtest` because it skips compilation.

### Input schema

Same as `run_backtest`, but `skip_compile` is automatically set to `true`.

### Output schema

Same as `run_backtest`.

---

## `run_backtest_only`

Backtest only: clean cache → backtest → extract. No analysis phase.

**When to call:** When you just need raw trade data (deals.csv) and don't need analytics. Fastest option for batch processing.

### Input schema

Same as `run_backtest`, but `skip_compile` and `skip_analyze` are automatically set to `true`.

### Output schema

Same as `run_backtest` but without `analysis_summary`.

---

## `launch_backtest`

Fire-and-forget mode: compile → clean → launch MT5 backtest, return immediately with job info.

**When to call:** When you want to launch a backtest without waiting for completion. Use `get_backtest_status` to poll for completion.

### Input schema

```typescript
{
  expert: string;          // Required. EA name without path or extension
  symbol?: string;         // Trading symbol (default: from config or first available)
  from_date?: string;      // Start date YYYY.MM.DD (default: past complete month)
  to_date?: string;        // End date YYYY.MM.DD (default: past complete month)
  timeframe?: string;      // M1, M5, M15, M30, H1, H4, D1 (default: M5)
  deposit?: number;        // Initial deposit (default: 10000)
  model?: 0 | 1 | 2;      // Tick model (default: 0)
  set_file?: string;       // Path to .set parameter file
  skip_compile?: boolean;  // Skip compilation
  skip_clean?: boolean;    // Skip cache cleaning
  timeout?: number;        // Max time in seconds (default: 900)
  gui?: boolean;          // Enable MT5 visualization
}
```

### Output schema

```typescript
{
  success: true;
  message: string;         // "Backtest launched successfully..."
  report_id: string;       // e.g., "20250122_034455_MyEA_XAUUSD_M5"
  report_dir: string;      // Full path to report directory
  expert: string;
  symbol: string;
  timeframe: string;
  launched_at: string;     // ISO8601 timestamp
  timeout_seconds: number;
  poll_hint: string;       // "Call get_backtest_status with report_dir to check progress"
}
```

---

## `get_backtest_status`

Check progress of a running backtest pipeline launched via `launch_backtest`.

**When to call:** Poll periodically after calling `launch_backtest` to check completion status.

### Input schema

```typescript
{
  report_dir: string;  // Report directory path from launch_backtest output
}
```

### Output schema

```typescript
{
  success: true;
  report_dir: string;
  status: "completed" | "running" | "failed" | "in_progress" | "not_started";
  stage: string;           // Current pipeline stage: COMPILE, CLEAN, BACKTEST, EXTRACT, ANALYZE, DONE
  is_complete: boolean;    // True if backtest finished successfully
  mt5_running: boolean;    // Whether MT5 process is active
  report_found: boolean;     // Whether report file exists
  metrics_extracted: boolean;
  deals_extracted: boolean;
  elapsed_seconds: number;   // Time since launch
  message: string;          // Human-readable status message
  job?: {
    report_id: string;
    expert: string;
    symbol: string;
    timeframe: string;
    launched_at: string;
    timeout_seconds: number;
  }
}
```

---

## `run_optimization`

Launch genetic parameter optimization as a detached background process.

**Important:** This tool returns immediately. MT5 runs for 2-6 hours. The AI agent must NOT poll for results — the user monitors MT5 and signals when done. Call `get_optimization_results` only after user confirmation.

**Always uses model 0.** Model 1 (1-min OHLC) overfits grid/martingale EAs because intra-bar price movement is not simulated. Parameters that look optimal on model 1 fail on model 0 verification — this is a known trap.

### Input schema

```typescript
{
  expert: string;          // EA name
  set_file: string;        // Path to optimization .set file (with ||Y flags)
  from: string;            // "YYYY-MM-DD"
  to: string;              // "YYYY-MM-DD"
  symbol?: string;         // Default from config
  deposit?: number;        // Default from config
  currency?: string;       // Default: "USD"
  leverage?: number;       // Default: 500
  log_file?: string;       // Where to write nohup output (default: /tmp/opt_<timestamp>.log)
}
```

### Output schema

```typescript
{
  success: boolean;
  job_id: string;          // "opt_20250619_143022"
  log_file: string;        // "/tmp/opt_20250619_143022.log"
  pid: number;             // Process ID (for user monitoring if needed)
  combinations: number;    // Estimated from set_file analysis (product of all ||Y ranges)
  message: string;         // "Optimization launched. Signal me when MT5 completes."
}
```

### Optimization set file format

```ini
; param=current_value||start||step||stop||Y   (Y = include in sweep)
; param=value||N                               (N = fixed, not swept)

Min_Entry_Confidence=0.610||0.580||0.010||0.650||Y   ; 8 values
TP_Pips_Layer1=400||300||50||500||Y                   ; 5 values
Max_DD_Percent=15.0||N                                ; fixed

; Total combinations: 8 × 5 = 40
```

**MT5-Quant handles automatically:**
- UTF-16LE encoding with BOM
- `chmod 444` (read-only) before launch
- `OptMode=0` reset in `terminal.ini`
- `LastOptimization` line removal from `terminal.ini`
- `ExpertParameters` = filename only (not full path) in launch INI

---

## `get_optimization_results`

Parse completed optimization results. Handles both HTML (`.htm`) and SpreadsheetML XML (`.htm.xml`) formats transparently.

### Input schema

```typescript
{
  job_id?: string;         // From run_optimization response. If omitted, uses latest _opt/ dir.
  report_dir?: string;     // Explicit path to *_opt/ directory
  top_n?: number;          // How many top results to return (default: 20)
  dd_threshold?: number;   // Flag results above this DD% as high-risk (default: 20)
  sort_by?: "profit" | "profit_factor" | "sharpe"; // Default: "profit"
}
```

### Output schema

```typescript
{
  success: boolean;
  total_passes: number;
  converged: boolean;         // True if passes stopped improving in last 10%
  report_format: "html" | "xml";

  results: Array<{
    rank: number;
    net_profit: number;
    profit_factor: number;
    max_dd_pct: number;
    total_trades: number;
    sharpe_ratio: number;
    high_risk: boolean;       // DD > dd_threshold
    params: Record<string, number | boolean>;  // All swept parameter values
  }>;

  convergence_analysis: {
    top_10_agreement: Record<string, string>;  // Params same across top 10 = strong signal
    high_variance_params: string[];             // Params that vary in top 10 = uncertain
  };

  recommendation: {
    best_params: Record<string, number | boolean>;
    reasoning: string;
    next_step: "verify_model0" | "auto_promote" | "investigate";
  };
}
```

### Convergence analysis

A parameter that appears with the same value across all top-10 results is a strong optimization signal — the genetic algorithm converged on it. A parameter that varies across top-10 means the optimizer couldn't distinguish between values — either the parameter doesn't matter much, or more passes are needed.

---

## `verify_setup`

Check all required paths, Wine version, and EA/set file inventory. Run this first if `run_backtest` or `run_optimization` fails with path errors.

### Input schema

```typescript
{}  // No parameters required
```

### Output schema

```typescript
{
  success: boolean;
  wine_path: string;
  wine_version: string;
  mt5_dir: string;
  terminal_exe: string;
  experts_dir: string;
  display_mode: "gui" | "headless";
  ea_count: number;           // .ex5 files found in Experts/
  set_count: number;          // .set files found
  missing: string[];          // List of paths/tools that couldn't be found
  hints: string[];            // Actionable fix hints for each missing item
}
```

---

## `get_backtest_status`

Check the current stage and elapsed time of a running backtest pipeline by reading its `progress.log`.

### Input schema

```typescript
{
  report_dir: string;    // Path to the report directory from run_backtest
}
```

### Output schema

```typescript
{
  success: boolean;
  report_dir: string;
  stage: "COMPILE" | "CLEAN" | "BACKTEST" | "EXTRACT" | "ANALYZE" | "DONE";
  elapsed_seconds: number;
  finished: boolean;
  log_lines: string[];   // Last 5 lines of progress.log
}
```

---

## `get_optimization_status`

Check the live state of a background optimization job (started by `run_optimization`).

### Input schema

```typescript
{
  job_id: string;        // From run_optimization response
}
```

### Output schema

```typescript
{
  success: boolean;
  job_id: string;
  alive: boolean;        // True if the optimization process is still running
  pid: number;
  started_at: string;    // ISO timestamp
  elapsed_seconds: number;
  report_found: boolean; // True if MT5 has written the result file
  report_path: string | null;
  log_tail: string[];    // Last 10 lines of the nohup log
}
```

---

## `prune_reports`

Delete old report directories to reclaim disk space, keeping the most recent N runs. Optimization result directories (`*_opt/`) are always preserved.

### Input schema

```typescript
{
  keep_last?: number;    // How many recent reports to keep (default from config, usually 10)
  dry_run?: boolean;     // If true, list what would be deleted without deleting (default: false)
}
```

### Output schema

```typescript
{
  success: boolean;
  deleted: string[];     // Paths that were (or would be) deleted
  kept: string[];        // Paths that were kept
  freed_mb: number;      // Approximate disk space freed
}
```

---

## `analyze_report`

Read and summarize a completed backtest report without re-running MT5.

### Input schema

```typescript
{
  report_dir: string;          // Path to report directory
  strategy?: "grid" | "scalper" | "trend" | "hedge" | "generic";
                               // Strategy profile that was used (default: "grid").
                               // Only affects interpretation of analysis.json fields —
                               // does not re-run analysis.
  include_deals?: boolean;     // Include top 20 deals in output (default: false)
  include_monthly?: boolean;   // Include full monthly P/L table (default: true)
  include_dd_events?: boolean; // Include DD event reconstruction (default: true)
  deep?: boolean;              // Include hourly_pnl and volume_profile (default: false)
}
```

### Output schema

```typescript
{
  success: boolean;
  report_dir: string;
  strategy: string;           // Active profile: "grid" | "scalper" | "trend" | "hedge" | "generic"

  metrics: { /* same as run_backtest metrics */ };

  // ── Always present (strategy-agnostic) ─────────────────────────────────────

  monthly_pnl: Array<{
    month: string;          // "2025-01"
    pnl: number;
    trades: number;
    green: boolean;
  }>;

  dd_events: Array<{
    peak_dd_pct: number;
    start_date: string;
    end_date: string;
    duration_days: number;
    recovery_date: string | null;
    recovery_days: number | null;
    cause: string;          // Profile-driven: e.g. "locking_cascade" (grid) or "whipsaw" (trend)
                            // Falls back to "unknown" when no keyword matched
  }>;

  top_losses: Array<{
    date: string;
    loss_usd: number;
    grid_depth_at_close: number;  // 0 for non-grid strategies
    volume: number;
    comment: string;
  }>;

  loss_sequences: Array<{
    length: number;
    total_loss: number;
    start: string;
    end: string;
  }>;

  position_pairs: Array<{
    time: string;
    type: "buy" | "sell";
    profit: number;
    volume: number;
    layer: number;
    hold_minutes: number | null;
    comment: string;
    magic: string;
    order: string;
  }>;

  // ── Strategy-driven (content varies by profile) ────────────────────────────

  depth_histogram: Record<string, number>;
                            // grid:    { L1: n, L2: n, …, "L8+": n }
                            // others:  {} (empty — no depth_re in profile)

  grid_depth_histogram: Record<string, number>;
                            // Backward-compat alias for depth_histogram (grid only)

  cycle_stats: {
    total_cycles: number;
    win_rate: number;       // percent
    avg_profit: number;
    win_rate_by_depth: Record<string, { total: number; win_rate: number }>;
    // win_rate_by_depth populated for grid; keys = "L?" for non-depth profiles
  };

  exit_reason_breakdown: Record<
    string,                 // Keys depend on strategy profile exit_keywords
                            // grid:    "locking" | "cutloss" | "zombie" | "timeout" | "tp" | "sl"
                            // scalper: "manual" | "trailing" | "tp" | "sl"
                            // trend:   "breakeven" | "trailing" | "partial" | "tp" | "sl"
                            // generic: "tp" | "sl"
    { count: number; total_pnl: number; avg_pnl: number }
  >;

  direction_bias: {
    buy?: { trades: number; win_rate: number; total_pnl: number; avg_pnl: number };
    sell?: { trades: number; win_rate: number; total_pnl: number; avg_pnl: number };
  };

  streak_analysis: {
    max_win_streak: number;
    max_win_start: string;
    max_win_end: string;
    max_loss_streak: number;
    max_loss_start: string;
    max_loss_end: string;
    current_streak: number;
    current_streak_type: "win" | "loss";
  };

  session_breakdown: Record<
    "asian" | "london" | "london_ny_overlap" | "new_york" | "off_hours",
    { trades: number; win_rate: number; total_pnl: number }
  >;

  weekday_pnl: Array<{
    day: string;            // "Monday" … "Sunday"
    pnl: number;
    trades: number;
    win_rate: number;
  }>;

  concurrent_peak: {
    peak_open: number;
    peak_time: string;
  };

  // ── Deep mode only (deep=true) ──────────────────────────────────────────────

  hourly_pnl?: Array<{
    hour: number;           // 0–23
    pnl: number;
    trades: number;
    win_rate: number;
  }>;

  volume_profile?: Array<{
    lot_tier: string;       // "0.01" | "0.02-0.04" | "0.05-0.09" | "0.10-0.49" | …
    pnl: number;
    trades: number;
    win_rate: number;
  }>;

  // ── Optional raw deals ──────────────────────────────────────────────────────

  deals?: Array<{ /* all 13 deal columns */ }>; // Only if include_deals=true
}
```

---

## `compare_baseline`

Compare a report against a baseline and return a structured verdict.

### Input schema

```typescript
{
  report_dir: string;       // Report to evaluate
  baseline: {
    net_profit: number;
    max_dd_pct: number;
    total_trades?: number;
    label?: string;         // e.g. "v1.2 production"
  };
  promote_threshold?: {
    profit_gt: number;      // Auto-promote if profit > this (default: baseline profit)
    dd_lt: number;          // AND DD < this (default: 20)
  };
}
```

### Output schema

```typescript
{
  verdict: "winner" | "loser" | "marginal";
  auto_promote: boolean;

  delta: {
    profit_usd: number;     // positive = improvement
    profit_pct: number;     // relative to baseline
    dd_pp: number;          // positive = DD got worse
    trades_delta: number;
  };

  summary: string;          // Human-readable one-liner

  details: {
    candidate: { net_profit: number; max_dd_pct: number; total_trades: number; };
    baseline: { net_profit: number; max_dd_pct: number; label: string; };
  };
}
```

### Example

```json
// Input
{
  "report_dir": "reports/20250619_143022_MyEA_v1.3_XAUUSD_M5",
  "baseline": {
    "net_profit": 8660,
    "max_dd_pct": 15.66,
    "label": "v1.2 production"
  }
}

// Output
{
  "verdict": "winner",
  "auto_promote": true,
  "delta": {
    "profit_usd": 3186.32,
    "profit_pct": 36.8,
    "dd_pp": -7.27,
    "trades_delta": -3
  },
  "summary": "+$3,186 (+37%) profit vs v1.2. DD dropped from 15.66% to 8.39%. Auto-promoting.",
  "details": {
    "candidate": { "net_profit": 11846.32, "max_dd_pct": 8.39, "total_trades": 1963 },
    "baseline": { "net_profit": 8660.00, "max_dd_pct": 15.66, "label": "v1.2 production" }
  }
}
```

---

## `compile_ea`

Compile an MQL5 Expert Advisor via MetaEditor (Wine/CrossOver).

### Input schema

```typescript
{
  expert_path: string;     // e.g. "src/MyEA_v1.2.mq5"
  include_dirs?: string[]; // Additional include search paths
}
```

### Output schema

```typescript
{
  success: boolean;
  binary_path: string;     // Path where .ex5 was written
  binary_size_bytes: number;
  warnings: number;
  errors: number;
  error_list: Array<{
    file: string;
    line: number;
    message: string;
  }>;
  compile_time_ms: number;
}
```

---

## Error Handling

All tools return `success: false` with an `error` field on failure. Pipeline failures are non-fatal by default — the tool returns partial results if any stages completed.

```typescript
{
  success: false,
  error: "COMPILE_FAILED",
  error_detail: "2 errors in src/MyEA_v1.2.mq5: line 847: undeclared identifier 'Max_New_Param'",
  completed_stages: ["COMPILE"],
  failed_stage: "COMPILE"
}
```

**Error codes:**

| Code | Stage | Cause |
|------|-------|-------|
| `COMPILE_FAILED` | COMPILE | MQL5 syntax errors |
| `WINE_NOT_FOUND` | Any | Wine/CrossOver not installed or wrong path |
| `MT5_TIMEOUT` | BACKTEST | MT5 didn't exit within timeout (default: 15min) |
| `REPORT_NOT_FOUND` | EXTRACT | MT5 produced no report (usually parameter error) |
| `EXTRACT_FAILED` | EXTRACT | Report parse error (format change?) |
| `NO_DEALS` | ANALYZE | Report has 0 trades (check date range, symbol) |
| `OPT_NOT_FINISHED` | get_opt_results | Optimization still running |

---

## `list_reports`

List all backtest report directories with compact key metrics. Use this to survey what runs exist before deciding which to analyze — much cheaper than calling `analyze_report` repeatedly.

### Input schema

```typescript
{
  include_opt?: boolean;   // Include _opt dirs (default: false)
  limit?: number;          // Max reports, newest first (default: 30)
}
```

### Output schema

```typescript
{
  success: boolean;
  count: number;
  reports: Array<{
    name: string;           // "20250619_143022_MyEA_XAUUSD_M5"
    is_opt: boolean;
    net_profit?: number;
    max_dd_pct?: number;
    total_trades?: number;
    symbol?: string;
    timeframe?: string;
    from_date?: string;
    to_date?: string;
    metrics?: "missing";    // Present only if metrics.json is absent
  }>;
}
```

---

## `tail_log`

Read the last N lines of a log file. Supports `filter=errors` to return only lines containing error/fail keywords — avoids streaming full logs into context.

### Input schema

```typescript
{
  // Provide one of: report_dir, job_id, or log_file
  report_dir?: string;     // Reads progress.log from this dir (omit for latest)
  job_id?: string;         // Reads the nohup log for this optimization job
  log_file?: string;       // Absolute path to any log file

  n?: number;              // Lines to return (default: 50)
  filter?: "all" | "errors" | "warnings";  // Default: "all"
}
```

### Output schema

```typescript
{
  success: boolean;
  log_file: string;        // Resolved path of the file that was read
  total_lines: number;     // Lines matched after filter applied
  lines: string[];         // Last n of the matched lines
}
```

---

## `cache_status`

Show the MT5 tester cache directory size broken down by symbol. Use before `clean_cache` to see what's there.

### Input schema

```typescript
{}  // No parameters
```

### Output schema

```typescript
{
  success: boolean;
  cache_dir: string;
  total_size_mb: number;
  symbols: Array<{
    symbol: string;        // Subdirectory name (broker symbol)
    size_mb: number;
  }>;
}
```

---

## `clean_cache`

Delete MT5 tester cache files. Forces MT5 to regenerate tick data on the next backtest (slower first run after clean). Supports dry-run preview and per-symbol targeting.

### Input schema

```typescript
{
  symbol?: string;         // Delete only this symbol's cache. Omit to delete all.
  dry_run?: boolean;       // Report what would be deleted without deleting (default: false)
}
```

### Output schema

```typescript
{
  success: boolean;
  dry_run: boolean;
  deleted_symbols: string[];
  freed_mb: number;
  hint: string;            // Reminder that next backtest will be slower
}
```

---

## `read_set_file`

Parse an MT5 `.set` parameter file (UTF-16LE or UTF-8) into structured JSON. Handles BOM detection automatically. Use this instead of reading raw `.set` files.

### Input schema

```typescript
{
  path: string;            // Path to .set file
}
```

### Output schema

```typescript
{
  success: boolean;
  path: string;
  param_count: number;
  comments: string[];      // Header comment lines (stripped of semicolons)
  params: Record<string, {
    value: string;          // Current / default value
    from?: string;          // Sweep start (present for optimization params)
    to?: string;            // Sweep end
    step?: string;          // Sweep step
    optimize?: boolean;     // True if ||Y flag is set
  }>;
}
```

### Example

```json
// Input
{ "path": "config/MyEA_opt.set" }

// Output
{
  "success": true,
  "path": "config/MyEA_opt.set",
  "param_count": 5,
  "comments": ["MyEA optimization set — XAUUSD M5"],
  "params": {
    "Min_Entry_Confidence": { "value": "0.610", "from": "0.580", "to": "0.650", "step": "0.010", "optimize": true },
    "TP_Pips": { "value": "400", "from": "300", "to": "500", "step": "50", "optimize": true },
    "Max_DD_Percent": { "value": "15.0" }
  }
}
```

---

## `write_set_file`

Write an MT5 `.set` parameter file with correct UTF-16LE encoding and `chmod 444`. Overwrites any existing file at the path.

### Input schema

```typescript
{
  path: string;            // Output path for .set file

  params: Record<string,
    | string | number      // Simple fixed value
    | {
        value: string | number;
        from?: string | number;   // Include for optimization sweep
        to?: string | number;
        step?: string | number;
        optimize?: boolean;       // true → ||Y, false → ||N (default: false)
      }
  >;
}
```

### Output schema

```typescript
{
  success: boolean;
  path: string;
  param_count: number;
  encoding: "utf-16-le";
  permissions: string;     // "444 (read-only, required by MT5)"
}
```

### Example

```json
// Input
{
  "path": "config/MyEA_opt.set",
  "params": {
    "Min_Entry_Confidence": { "value": 0.61, "from": 0.58, "to": 0.65, "step": 0.01, "optimize": true },
    "TP_Pips": { "value": 400, "from": 300, "to": 500, "step": 50, "optimize": true },
    "Max_DD_Percent": 15.0
  }
}

// Output
{
  "success": true,
  "path": "config/MyEA_opt.set",
  "param_count": 3,
  "encoding": "utf-16-le",
  "permissions": "444 (read-only, required by MT5)"
}
```

---

## `list_jobs`

List all optimization jobs tracked in `.mt5mcp_jobs/` with compact status. Cheaper than calling `get_optimization_status` per job.

### Input schema

```typescript
{
  include_done?: boolean;  // Include completed/failed jobs (default: true)
}
```

### Output schema

```typescript
{
  success: boolean;
  count: number;
  jobs: Array<{
    job_id: string;          // "opt_20250619_143022"
    status: "running" | "done" | "failed";
    elapsed_seconds: number | null;
    expert: string;
    started_at: string;      // ISO timestamp
    log_file: string;
  }>;
}
```

---

## `patch_set_file`

Modify specific parameters in an existing `.set` file in-place. Preserves all other params, comments, and sweep config untouched. Returns a diff of what changed. **Use instead of `read_set_file` → edit → `write_set_file`** — saves two round-trips.

### Input schema

```typescript
{
  path: string;            // .set file to modify (must exist)
  patches: Record<string,
    | string | number      // scalar → only updates value, keeps existing sweep config
    | {
        value?: string | number;
        from?: string | number;
        to?: string | number;
        step?: string | number;
        optimize?: boolean;
      }
  >;
}
```

### Output schema

```typescript
{
  success: boolean;
  path: string;
  changed_count: number;
  param_count: number;
  changed: Array<{ name: string; old: string; new: string; }>;
}
```

### Example

```json
// Input — change two params without touching the rest of the file
{
  "path": "config/MyEA_opt.set",
  "patches": {
    "TP_Pips": 350,
    "Min_Entry_Confidence": { "value": 0.62, "from": 0.60, "to": 0.65, "optimize": true }
  }
}

// Output
{
  "success": true,
  "path": "config/MyEA_opt.set",
  "changed_count": 2,
  "param_count": 12,
  "changed": [
    { "name": "TP_Pips", "old": "400", "new": "350" },
    { "name": "Min_Entry_Confidence", "old": "0.610", "new": "0.62" }
  ]
}
```

---

## `clone_set_file`

Copy a `.set` file to a new path, applying optional param overrides. One call instead of read → modify → write. Preserves header comments.

### Input schema

```typescript
{
  source: string;          // Source .set file
  destination: string;     // Output path (created if needed)
  overrides?: Record<string, string | number | { value; from?; to?; step?; optimize? }>;
}
```

### Output schema

```typescript
{
  success: boolean;
  source: string;
  destination: string;
  param_count: number;
  overridden_count: number;
  overridden: Array<{ name: string; old: string | null; new: string; }>;
}
```

---

## `set_from_optimization`

Generate a clean backtest `.set` file directly from an optimization result's params dict. Strips all sweep flags (`||Y`) so the file is ready for `run_backtest`. Optionally fills params not in the optimization result from a template `.set`, and optionally re-adds sweep ranges to selected params for a narrowed follow-on optimization.

**Typical call**: immediately after `get_optimization_results`, use `results[0].params` as the `params` argument.

### Input schema

```typescript
{
  path: string;            // Output .set file path

  params: Record<string, string | number>;
                           // Flat param→value dict from optimization result.
                           // e.g. { "TP_Pips": 400, "Min_Confidence": 0.61 }

  template?: string;       // Path to existing .set. Params NOT in 'params' are
                           // copied from here as fixed values.

  sweep?: Record<string, { from: number; to: number; step: number; optimize?: boolean }>;
                           // Re-add sweep ranges to specific params after applying opt values.
                           // Used to create a narrowed follow-on optimization .set.
}
```

### Output schema

```typescript
{
  success: boolean;
  path: string;
  param_count: number;
  from_template: boolean;
  opt_params_applied: number;
  swept_params: number;        // > 0 if sweep was provided
  total_combinations: number;  // 0 for pure backtest .set
}
```

### Example

```json
// After get_optimization_results returned:
// results[0].params = { "TP_Pips": 400, "Min_Entry_Confidence": 0.62, "Max_DD_Percent": 15.0 }

{
  "path": "config/MyEA_v1.3.set",
  "params": { "TP_Pips": 400, "Min_Entry_Confidence": 0.62, "Max_DD_Percent": 15.0 },
  "template": "config/MyEA_base.set"
}

// Output
{
  "success": true,
  "path": "config/MyEA_v1.3.set",
  "param_count": 12,
  "from_template": true,
  "opt_params_applied": 3,
  "swept_params": 0,
  "total_combinations": 0
}
```

---

## `diff_set_files`

Compare two `.set` files and return only the differences. Use instead of reading both files and comparing manually.

### Input schema

```typescript
{
  path_a: string;   // Baseline / old file
  path_b: string;   // Candidate / new file
}
```

### Output schema

```typescript
{
  success: boolean;
  path_a: string;
  path_b: string;
  identical: boolean;
  added_count: number;    // Params in b but not a
  removed_count: number;  // Params in a but not b
  changed_count: number;  // Params in both but with different value or sweep flag

  added:   Array<{ name: string; value: string; }>;
  removed: Array<{ name: string; value: string; }>;
  changed: Array<{
    name: string;
    a: string;           // value in path_a
    b: string;           // value in path_b
    sweep_a?: boolean;   // Present only if sweep flag differs
    sweep_b?: boolean;
  }>;
}
```

### Example

```json
{
  "path_a": "config/MyEA_v1.2.set",
  "path_b": "config/MyEA_v1.3.set"
}

// Output
{
  "success": true,
  "identical": false,
  "added_count": 1,
  "removed_count": 0,
  "changed_count": 2,
  "added":   [{ "name": "Trailing_Activation", "value": "50" }],
  "removed":  [],
  "changed": [
    { "name": "TP_Pips", "a": "400", "b": "350" },
    { "name": "Min_Entry_Confidence", "a": "0.610", "b": "0.620", "sweep_a": true, "sweep_b": false }
  ]
}
```

---

## `describe_sweep`

Show a `.set` file's sweep configuration: which params are swept, their ranges, per-param value counts, and total combinations. Use before `run_optimization` to verify scope.

### Input schema

```typescript
{
  path: string;
}
```

### Output schema

```typescript
{
  success: boolean;
  path: string;
  total_params: number;
  swept_count: number;
  fixed_count: number;
  total_combinations: number;
  swept_params: Array<{
    name: string;
    from: string;
    to: string;
    step: string;
    count: number;      // Number of distinct values in this param's range
  }>;
  hint: string;         // e.g. "240 combinations. Typical range: 1–8h depending on EA tick speed."
}
```

### Example

```json
// Input
{ "path": "config/MyEA_opt.set" }

// Output
{
  "success": true,
  "total_params": 12,
  "swept_count": 3,
  "fixed_count": 9,
  "total_combinations": 240,
  "swept_params": [
    { "name": "TP_Pips",              "from": "300", "to": "500", "step": "50",   "count": 5 },
    { "name": "Min_Entry_Confidence", "from": "0.58","to": "0.65","step": "0.01", "count": 8 },
    { "name": "Max_DD_Percent",       "from": "12",  "to": "20",  "step": "2",    "count": 5 }
  ],
  "hint": "240 combinations. Typical range: 1–8h depending on EA tick speed."
}
```

---

## `list_set_files`

List all `.set` files in the MT5 tester profiles directory with param counts, swept param counts, and total combinations per file. Use to find the right `.set` without reading each one.

### Input schema

```typescript
{
  ea?: string;    // Filter by EA name substring (case-insensitive)
}
```

### Output schema

```typescript
{
  success: boolean;
  profiles_dir: string;
  count: number;
  files: Array<{
    name: string;               // filename only
    param_count: number;
    swept_count: number;
    total_combinations: number; // 0 for backtest-only .set files
    modified: string;           // "YYYY-MM-DD HH:MM"
    error?: string;             // Present only if file is unreadable
  }>;
}
```

---

## `get_active_account`

Get current MT5 account session information: login, server, and available symbols. This is essential for pre-flight checks to ensure symbol availability before backtesting.

### Input schema

```typescript
{}  // No parameters
```

### Output schema

```typescript
{
  success: boolean;
  ready_for_backtest: boolean;    // true if account exists and symbols available
  account: {
    login: string;
    server: string;
  } | null;
  server: string;                   // Active server name
  available_servers: string[];      // All servers with history data
  symbols: string[];                // Symbols available for active server
  symbol_count: number;
  hint: string;                     // "Ready for backtesting" or instructions
}
```

---

## `check_symbol_data_status`

Validate if a symbol has sufficient historical tick data for a specified date range before running backtest. Prevents failed backtests due to missing history data.

### Input schema

```typescript
{
  symbol: string;        // e.g., "XAUUSDc"
  from_date: string;     // "YYYY.MM.DD"
  to_date: string;       // "YYYY.MM.DD"
}
```

### Output schema

```typescript
{
  success: boolean;
  symbol: string;
  server: string;
  has_sufficient_data: boolean;
  requested_range: { from: string; to: string };
  data_range: string;           // "YYYY.MM.DD - YYYY.MM.DD" or "unknown"
  years_available: number;      // Count of years with data
  hcc_files_count: number;      // Number of history cache files
  warnings: string[] | null;    // Data range issues
  suggestion: string;           // Action recommendation
}
```

---

## `check_mt5_status`

Check if MT5 terminal is properly installed and configured. Returns comprehensive status of all required components.

### Input schema

```typescript
{}  // No parameters
```

### Output schema

```typescript
{
  success: boolean;
  terminal_ready: boolean;      // true if all components present
  checks: {
    mt5_dir_exists: boolean;
    terminal64_exe: boolean;
    metaeditor64_exe: boolean;
    metatester64_exe: boolean;
    wine_executable: boolean;
    wine_path: string | null;
  };
  mt5_version: string | null;
  current_account: {
    login: string;
    server: string;
  } | null;
  hint: string;
}
```

---

## `get_backtest_history`

List all backtests previously run for a specific EA and/or symbol with summary metrics. Use for tracking performance over time.

### Input schema

```typescript
{
  expert?: string;       // Filter by EA name
  symbol?: string;       // Filter by symbol
  limit?: number;        // Max results (default: 10)
}
```

### Output schema

```typescript
{
  success: boolean;
  count: number;
  total: number;
  filters: {
    expert: string | null;
    symbol: string | null;
  };
  history: Array<{
    report_dir: string;
    date: string | null;
    expert: string | null;
    symbol: string | null;
    period: string | null;
    profit: number | null;
    profit_factor: number | null;
    expected_payoff: number | null;
    drawdown_pct: number | null;
    total_trades: number | null;
    win_rate: number | null;
  }>;
  hint: string;
}
```

---

## `compare_backtests`

Compare two or more backtest results side-by-side with key metrics analysis. Includes profit/drawdown differences and verdict on which performed better.

### Input schema

```typescript
{
  report_dirs: string[];  // List of report directory paths to compare
}
```

### Output schema

```typescript
{
  success: boolean;
  count: number;
  comparisons: Array<{
    report_dir: string;
    expert: string | null;
    symbol: string | null;
    net_profit: number | null;
    profit_factor: number | null;
    drawdown_pct: number | null;
    total_trades: number | null;
    win_rate: number | null;
    expected_payoff: number | null;
    recovery_factor: number | null;
    sharpe_ratio: number | null;
  }>;
  analysis: Array<{
    compare_to: string | null;
    report: string | null;
    profit_diff: number;
    profit_pct_change: number;
    drawdown_diff: number;
    profit_factor_diff: number;
    verdict: "better" | "worse" | "mixed";
  }> | null;
  verdict: string | null;  // "Best: <report_dir>"
}
```

---

## `init_project`

Create a new MQL5 project with standard directory structure and template files. Supports scalper, swing, grid, and basic templates.

### Input schema

```typescript
{
  name: string;                    // Project name (used for EA filename)
  template?: "scalper" | "swing" | "grid" | "basic";  // Default: "basic"
}
```

### Output schema

```typescript
{
  success: boolean;
  project_name: string;
  template: string;
  created_files: string[];  // Paths to created files
  hint: string;
}
```

---

## `validate_ea_syntax`

Perform pre-compile syntax check on MQL5 source file without running full compilation. Detects common issues before expensive MetaEditor compilation.

### Input schema

```typescript
{
  path: string;  // Path to .mq5 source file
}
```

### Output schema

```typescript
{
  success: boolean;
  valid: boolean;
  path: string;
  checks: {
    has_on_init: boolean;
    has_on_tick: boolean;
    has_on_deinit: boolean;
    lines: number;
  };
  errors: Array<{
    line: number;
    message: string;
    severity: "error";
  }> | null;
  warnings: Array<{
    line: number;
    message: string;
    severity: "warning";
  }> | null;
  hint: string;
}
```

---

## `create_set_template`

Generate a .set parameter file template based on an EA's input variables. Automatically parses input declarations from source code.

### Input schema

```typescript
{
  ea: string;              // EA name or path to .mq5/.ex5 file
  output_path?: string;    // Optional custom output path
}
```

### Output schema

```typescript
{
  success: boolean;
  ea: string;
  inputs_found: number;
  inputs: Array<{
    name: string;
    type: string;
    default: string;
    description: string | null;
  }>;
  set_file: string;  // Path to generated file
  hint: string;
}
```

---

## `export_report`

Export backtest report to various formats (CSV, JSON, Markdown) for external analysis or sharing.

### Input schema

```typescript
{
  report_dir: string;        // Path to backtest report directory
  format?: "csv" | "json" | "md";  // Default: "csv"
  output_path?: string;      // Optional custom output file path
}
```

### Output schema

```typescript
{
  success: boolean;
  format: string;
  output_file: string;
  source: string;
  hint: string;
}
```

---

## `archive_report`

Convert a backtest report directory into a compact JSON entry appended to `config/backtest_history.json`. Idempotent — re-archiving the same report is a no-op. Optionally deletes the source directory to reclaim disk space.

### Input schema

```typescript
{
  report_dir?: string;     // Directory to archive. Omit for latest.
  delete_after?: boolean;  // Delete source dir after archiving (default: false)
  verdict?: "winner" | "loser" | "marginal" | "reference";
  notes?: string;          // Free-text notes for the entry
  tags?: string[];         // Tags e.g. ["tight-sl", "new-filter"]
}
```

### Output schema

```typescript
{
  success: boolean;
  id: string;              // Report dir basename used as history entry id
  already_existed: boolean;
  deleted_source: boolean;
  history_file: string;    // Absolute path to backtest_history.json
  entry_summary: {
    ea: string;
    symbol: string;
    metrics: { net_profit: number; profit_factor: number; max_dd_pct: number; sharpe_ratio: number; total_trades: number; };
    verdict: string | null;
  };
}
```

---

## `archive_all_reports`

Bulk-archive all backtest report directories into `config/backtest_history.json`. Entries already in history are skipped. Use `delete_after=true` to reclaim disk space while preserving all results as JSON. Optimization dirs (`_opt` suffix) are never deleted.

### Input schema

```typescript
{
  delete_after?: boolean;  // Delete source dirs after archiving (default: false)
  keep_last?: number;      // Protect newest N dirs from deletion even with delete_after=true (default: 5)
  dry_run?: boolean;       // Preview without making changes (default: false)
}
```

### Output schema

```typescript
{
  success: boolean;
  dry_run: boolean;
  archived_count: number;
  skipped_count: number;   // Already in history
  deleted_count: number;
  failed_count: number;    // Dirs with no parseable metrics
  archived: string[];
  skipped: string[];
  deleted: string[];
  failed: string[];
  history_file: string;
}
```

---

## `get_history`

Query `config/backtest_history.json` with filters and sorting. Strips `monthly_pnl` arrays by default — set `include_monthly=true` when you need the full breakdown.

### Input schema

```typescript
{
  ea?: string;               // Substring match on EA name
  symbol?: string;           // Exact match (uppercase)
  verdict?: "winner" | "loser" | "marginal" | "reference";
  tag?: string;              // Entry must contain this tag
  min_profit?: number;       // net_profit >= this
  max_dd_pct?: number;       // max_dd_pct <= this
  sort_by?: "date" | "profit" | "dd" | "sharpe";  // Default: date, newest first
  limit?: number;            // Default: 20
  include_monthly?: boolean; // Include monthly_pnl arrays (default: false)
}
```

### Output schema

```typescript
{
  success: boolean;
  count: number;
  entries: Array<{
    id: string;                    // Report dir basename
    archived_at: string;           // ISO timestamp
    report_dir_deleted: boolean;
    ea: string;
    symbol: string;
    timeframe: string;
    from_date: string;
    to_date: string;
    metrics: {
      net_profit: number;
      profit_factor: number;
      max_dd_pct: number;
      sharpe_ratio: number;
      total_trades: number;
      recovery_factor: number;
      win_rate_pct: number;
      expected_payoff: number;
    };
    summary?: {
      green_months: number;
      total_months: number;
      worst_month: string;
      worst_month_pnl: number;
      dominant_exit?: string;
      max_win_streak?: number;
      max_loss_streak?: number;
    };
    worst_dd_event?: {
      peak_dd_pct: number;
      start_date: string;
      end_date: string;
      duration_days: number;
      cause: string;
    };
    monthly_pnl?: Array<{ month: string; pnl: number; trades: number; green: boolean; }>;
    verdict: string | null;
    notes: string;
    tags: string[];
    promoted_to_baseline: boolean;
  }>;
}
```

---

## `promote_to_baseline`

Write a backtest result to `config/baseline.json` — the production reference used by `compare_baseline` and the Claude Code baseline hook. Also marks the source history entry as `promoted_to_baseline: true`.

### Input schema

```typescript
{
  // Provide one: history_id, report_dir, or neither (uses latest report)
  history_id?: string;     // Entry id from get_history
  report_dir?: string;     // Direct path to report directory
  notes?: string;          // Written to baseline.json notes field
}
```

### Output schema

```typescript
{
  success: boolean;
  baseline_file: string;
  baseline: {
    ea: string;
    symbol: string;
    period: string;            // "YYYY-MM-DD/YYYY-MM-DD"
    net_profit: number;
    profit_factor: number;
    max_drawdown_pct: number;
    sharpe_ratio: number;
    total_trades: number;
    recovery_factor: number;
    promoted_from: string;     // History entry id
    promoted_at: string;       // Date promoted (YYYY-MM-DD)
    notes: string;
  };
}
```

### Example

```json
// Input
{ "history_id": "20250619_143022_MyEA_XAUUSD_M5", "notes": "v1.3 after walk-forward validation" }

// Output
{
  "success": true,
  "baseline_file": "/path/to/config/baseline.json",
  "baseline": {
    "ea": "MyEA",
    "symbol": "XAUUSD",
    "period": "2025-01-01/2025-06-30",
    "net_profit": 4832.10,
    "profit_factor": 1.54,
    "max_drawdown_pct": 12.3,
    "sharpe_ratio": 1.18,
    "total_trades": 891,
    "recovery_from": "20250619_143022_MyEA_XAUUSD_M5",
    "promoted_at": "2025-06-20",
    "notes": "v1.3 after walk-forward validation"
  }
}
```

---

## `annotate_history`

Update the verdict, notes, or tags on an existing history entry. Use this after `compare_baseline` to record the decision, or to tag runs for later retrieval.

### Input schema

```typescript
{
  history_id: string;      // Required — entry id to update
  verdict?: "winner" | "loser" | "marginal" | "reference";
  notes?: string;          // Replaces existing notes
  tags?: string[];         // Replaces existing tags
  add_tags?: string[];     // Appends to existing tags without overwriting
}
```

### Output schema

```typescript
{
  success: boolean;
  id: string;
  verdict: string | null;
  notes: string;
  tags: string[];
}
```

---

## Token-efficient usage patterns

### Surveying past runs

```
list_reports(limit=10)          → see what's there (live dirs)
get_history(ea="MyEA", limit=10) → see what's been archived
analyze_report(report_dir=X)    → drill into one specific run
```

Never call `analyze_report` on multiple directories to find the best run — use `list_reports` or `get_history` first.

### Checking logs without noise

```
tail_log(job_id=X, filter=errors)   → only failures
tail_log(report_dir=X, n=20)        → last 20 lines of backtest progress
```

### Managing disk space

```
archive_all_reports(dry_run=true)               → preview what would be archived
archive_all_reports(delete_after=true, keep_last=3)  → archive all, delete old, keep 3 newest
get_history(sort_by=profit, limit=5)            → find best archived runs
```

### Labelling experiments

```
annotate_history(history_id=X, verdict="loser", notes="SL too tight, reversed at L3")
annotate_history(history_id=X, add_tags=["walk-forward-fail"])
get_history(verdict="winner")                   → all winners across all sessions
```

### Promoting a new production config

```
run_backtest(...)
compare_baseline(...)                           → get verdict
archive_report(delete_after=true, verdict="winner")
promote_to_baseline(notes="v1.4 after WF")     → update baseline.json
```

### Managing cache

```
cache_status()                                  → see symbol breakdown and total size
clean_cache(symbol=XAUUSD, dry_run=true)        → preview
clean_cache(symbol=XAUUSD)                      → execute
```

### Pre-flight validation

```
get_active_account()                            → current login, server, available symbols
check_symbol_data_status(symbol=XAUUSD, from=2025.01.01, to=2025.03.31)
                                                → verify data availability before backtest
check_mt5_status()                              → verify MT5 installation and readiness
validate_ea_syntax(path=MyEA.mq5)               → pre-compile syntax check
```

### Project management

```
init_project(name=MyStrategy, template=scalper)  → scaffold new EA with template
create_set_template(ea=MyEA)                     → generate .set from EA inputs
export_report(report_dir=..., format=csv)         → export to CSV/JSON/Markdown
```

### History and comparison

```
get_backtest_history(expert=MyEA, limit=10)       → list past backtests with metrics
compare_backtests(report_dirs=["dir1", "dir2"])   → side-by-side comparison
```

### Working with set files

```
# Inspect
list_set_files(ea="MyEA")               → all variants, swept param counts, combinations
describe_sweep(path=MyEA_opt.set)       → verify 240 combinations before launching opt
diff_set_files(a=v1.2.set, b=v1.3.set) → only changed params, not full file content

# Edit (never read+write manually)
patch_set_file(path, {TP_Pips: 350})    → change one param, keep everything else intact
clone_set_file(src, dest, overrides)    → create variant from base in one call

# Generate after optimization
set_from_optimization(                  → map results[0].params → clean backtest .set
  path=MyEA_v1.3.set,
  params=results[0].params,
  template=MyEA_base.set                → fills non-swept params from existing file
)
```

---

## Autonomous Loop Pattern

The tools are designed to support a fully autonomous experiment → evaluate → promote → optimize loop:

```
1.  run_backtest(new_params)
2.  compare_baseline(result, current_production)
3a. if winner:
     - archive_report(delete_after=true, verdict="winner")
     - promote_to_baseline(notes="...")
     - write_set_file(new_production.set)
     - run_optimization(new_production_set)
     - [wait for user signal]
     - get_optimization_results()
     - set_from_optimization(path=verify.set, params=results[0].params, template=prod.set)
     - verify top result: run_backtest(expert, set_file=verify.set, skip_compile=true)
     - if still beats baseline → goto step 1
3b. if loser:
     - archive_report(delete_after=true, verdict="loser", notes="root cause")
     - analyze_report(result) → find root cause
     - read_set_file() → inspect current params
     - propose parameter or code change → goto step 1
```

No user confirmation needed between steps 1→2→3. The AI agent drives the full loop; the user monitors and signals when optimization completes (since that runs for hours). Every run is archived before the directory is deleted, so nothing is lost.
