use serde_json::{json, Value};

pub fn tool_run_backtest() -> Value {
    json!({
        "name": "run_backtest",
        "description": "Full backtest pipeline: compile EA → clean cache → run backtest → extract results → analyze. Use this when you have modified the EA source code.",
        "inputSchema": {
            "type": "object",
            "required": ["expert"],
            "properties": {
                "expert": { "type": "string", "description": "EA name without path or extension" },
                "symbol": { "type": "string", "description": "Trading symbol (default: from config or first available)" },
                "from_date": { "type": "string", "description": "Start date YYYY.MM.DD (default: past complete month)" },
                "to_date": { "type": "string", "description": "End date YYYY.MM.DD (default: past complete month)" },
                "timeframe": { "type": "string", "enum": ["M1", "M5", "M15", "M30", "H1", "H4", "D1"], "description": "Chart timeframe (default: M5)" },
                "deposit": { "type": "integer", "description": "Initial deposit (default: 10000)" },
                "model": { "type": "integer", "enum": [0, 1, 2], "description": "Tick model: 0=Every tick, 1=OHLC, 2=Open prices" },
                "set_file": { "type": "string", "description": "Path to .set parameter file for EA inputs" },
                "skip_compile": { "type": "boolean", "description": "Skip compilation (use existing .ex5)" },
                "skip_clean": { "type": "boolean", "description": "Skip cache cleaning" },
                "skip_analyze": { "type": "boolean", "description": "Skip analysis phase" },
                "deep": { "type": "boolean", "description": "Run deep analysis with extra metrics" },
                "shutdown": { "type": "boolean", "description": "Close MT5 after backtest completes" },
                "kill_existing": { "type": "boolean", "description": "Kill any running MT5 instance first" },
                "timeout": { "type": "integer", "description": "Max wait time in seconds (default: 900)" },
                "gui": { "type": "boolean", "description": "Enable MT5 visualization window" },
                "startup_delay_secs": { "type": "integer", "description": "Seconds to wait for MT5 initialization (default: 10, set to 0 for default 10s)" }
            }
        }
    })
}

pub fn tool_run_backtest_quick() -> Value {
    json!({
        "name": "run_backtest_quick",
        "description": "Quick backtest using pre-compiled EA: clean cache → run backtest → extract → analyze. Skips compilation. Use when EA code hasn't changed.",
        "inputSchema": {
            "type": "object",
            "required": ["expert"],
            "properties": {
                "expert": { "type": "string", "description": "EA name without path or extension (must have .ex5 compiled)" },
                "symbol": { "type": "string", "description": "Trading symbol (default: from config or first available)" },
                "from_date": { "type": "string", "description": "Start date YYYY.MM.DD (default: past complete month)" },
                "to_date": { "type": "string", "description": "End date YYYY.MM.DD (default: past complete month)" },
                "timeframe": { "type": "string", "enum": ["M1", "M5", "M15", "M30", "H1", "H4", "D1"], "description": "Chart timeframe (default: M5)" },
                "deposit": { "type": "integer", "description": "Initial deposit (default: 10000)" },
                "model": { "type": "integer", "enum": [0, 1, 2], "description": "Tick model" },
                "set_file": { "type": "string", "description": "Path to .set parameter file for EA inputs" },
                "deep": { "type": "boolean", "description": "Run deep analysis" },
                "shutdown": { "type": "boolean", "description": "Close MT5 after backtest" },
                "timeout": { "type": "integer", "description": "Max wait time in seconds (default: 900)" },
                "gui": { "type": "boolean", "description": "Enable MT5 visualization" },
                "startup_delay_secs": { "type": "integer", "description": "Seconds to wait for MT5 initialization (default: 10)" }
            }
        }
    })
}

pub fn tool_run_backtest_only() -> Value {
    json!({
        "name": "run_backtest_only",
        "description": "Backtest only: just run backtest and extract results. No compile, no analysis. Fastest option when you just need raw trade data.",
        "inputSchema": {
            "type": "object",
            "required": ["expert"],
            "properties": {
                "expert": { "type": "string", "description": "EA name without path or extension (must have .ex5 compiled)" },
                "symbol": { "type": "string", "description": "Trading symbol (default: from config)" },
                "from_date": { "type": "string", "description": "Start date YYYY.MM.DD" },
                "to_date": { "type": "string", "description": "End date YYYY.MM.DD" },
                "timeframe": { "type": "string", "enum": ["M1", "M5", "M15", "M30", "H1", "H4", "D1"], "description": "Chart timeframe (default: M5)" },
                "deposit": { "type": "integer", "description": "Initial deposit (default: 10000)" },
                "model": { "type": "integer", "enum": [0, 1, 2], "description": "Tick model" },
                "set_file": { "type": "string", "description": "Path to .set parameter file" },
                "shutdown": { "type": "boolean", "description": "Close MT5 after backtest" },
                "timeout": { "type": "integer", "description": "Max wait time (default: 900)" },
                "gui": { "type": "boolean", "description": "Enable MT5 visualization" }
            }
        }
    })
}

pub fn tool_launch_backtest() -> Value {
    json!({
        "name": "launch_backtest",
        "description": "Launch MT5 backtest in fire-and-forget mode: compile → clean cache → launch MT5 backtest, then return immediately. Use get_backtest_status to poll for completion.",
        "inputSchema": {
            "type": "object",
            "required": ["expert"],
            "properties": {
                "expert": { "type": "string", "description": "EA name without path or extension" },
                "symbol": { "type": "string", "description": "Trading symbol" },
                "from_date": { "type": "string", "description": "Start date YYYY.MM.DD" },
                "to_date": { "type": "string", "description": "End date YYYY.MM.DD" },
                "timeframe": { "type": "string", "enum": ["M1", "M5", "M15", "M30", "H1", "H4", "D1"] },
                "deposit": { "type": "integer" },
                "model": { "type": "integer", "enum": [0, 1, 2] },
                "set_file": { "type": "string", "description": "Path to .set parameter file" },
                "skip_compile": { "type": "boolean" },
                "skip_clean": { "type": "boolean" },
                "timeout": { "type": "integer", "description": "Max time in seconds to wait for backtest (default: 900)" },
                "shutdown": { "type": "boolean", "description": "Shut down MT5 after test (default: true — required for HTML report to be written)" },
                "gui": { "type": "boolean", "description": "Enable visualization during backtest" },
                "startup_delay_secs": { "type": "integer", "description": "Seconds to wait for MT5 initialization (default: 10)" },
                "inactivity_kill_secs": { "type": "integer", "description": "Kill MT5 if tester log hasn't grown for this many seconds (0 = disabled). Use to abort EAs that stop trading mid-test." }
            }
        }
    })
}

pub fn tool_get_backtest_status() -> Value {
    json!({
        "name": "get_backtest_status",
        "description": "Check progress of a running backtest pipeline",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

pub fn tool_get_tester_log() -> Value {
    json!({
        "name": "get_tester_log",
        "description": "Read the active MT5 tester agent journal log. Returns parsed deals, final balance, test progress, and raw log tail. Works during a backtest (if the log is being written) or after completion. Use this to inspect what trades occurred, check for EA activity, or debug issues when the HTML report wasn't produced.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "tail_lines": {
                    "type": "integer",
                    "description": "Number of log tail lines to return (default: 100)"
                }
            }
        }
    })
}

pub fn tool_cache_status() -> Value {
    json!({
        "name": "cache_status",
        "description": "Check tester cache disk usage",
        "inputSchema": {
            "type": "object"
        }
    })
}

pub fn tool_clean_cache() -> Value {
    json!({
        "name": "clean_cache",
        "description": "Delete old tester cache files",
        "inputSchema": {
            "type": "object",
            "properties": {
                "symbol": { "type": "string" },
                "dry_run": { "type": "boolean" }
            }
        }
    })
}
