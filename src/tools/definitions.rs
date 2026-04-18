use serde_json::{json, Value};

pub fn get_tools_list() -> Value {
    let tools = vec![
        tool_run_backtest(),
        tool_run_optimization(),
        tool_get_optimization_results(),
        tool_analyze_report(),
        tool_compare_baseline(),
        tool_compile_ea(),
        tool_verify_setup(),
        tool_list_symbols(),
        tool_list_experts(),
        tool_list_indicators(),
        tool_list_scripts(),
        tool_get_backtest_status(),
        tool_get_optimization_status(),
        tool_prune_reports(),
        tool_get_latest_report(),
        tool_list_reports(),
        tool_search_reports(),
        tool_tail_log(),
        tool_cache_status(),
        tool_clean_cache(),
        tool_read_set_file(),
        tool_write_set_file(),
        tool_patch_set_file(),
        tool_clone_set_file(),
        tool_set_from_optimization(),
        tool_diff_set_files(),
        tool_describe_sweep(),
        tool_list_set_files(),
        tool_list_jobs(),
        tool_archive_report(),
        tool_archive_all_reports(),
        tool_get_history(),
        tool_promote_to_baseline(),
        tool_annotate_history(),
        tool_healthcheck(),
    ];

    json!(tools)
}

fn tool_run_backtest() -> Value {
    json!({
        "name": "run_backtest",
        "description": "Run a complete MT5 backtest pipeline: compile → clean cache → backtest → extract → analyze",
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
                "skip_analyze": { "type": "boolean" },
                "deep": { "type": "boolean", "description": "Run deep analysis" },
                "shutdown": { "type": "boolean", "description": "Close MT5 after backtest" },
                "kill_existing": { "type": "boolean" },
                "timeout": { "type": "integer" },
                "gui": { "type": "boolean" }
            }
        }
    })
}

fn tool_run_optimization() -> Value {
    json!({
        "name": "run_optimization",
        "description": "Launch MT5 genetic parameter optimization",
        "inputSchema": {
            "type": "object",
            "required": ["expert", "set_file", "from_date", "to_date"],
            "properties": {
                "expert": { "type": "string" },
                "set_file": { "type": "string" },
                "symbol": { "type": "string" },
                "from_date": { "type": "string" },
                "to_date": { "type": "string" },
                "deposit": { "type": "integer" }
            }
        }
    })
}

fn tool_get_optimization_results() -> Value {
    json!({
        "name": "get_optimization_results",
        "description": "Parse completed MT5 optimization results",
        "inputSchema": {
            "type": "object",
            "properties": {
                "job_id": { "type": "string" },
                "report_file": { "type": "string" },
                "dd_threshold": { "type": "number" },
                "top_n": { "type": "integer" }
            }
        }
    })
}

fn tool_analyze_report() -> Value {
    json!({
        "name": "analyze_report",
        "description": "Read and summarize a completed backtest report",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

fn tool_compare_baseline() -> Value {
    json!({
        "name": "compare_baseline",
        "description": "Compare a backtest report against a baseline",
        "inputSchema": {
            "type": "object",
            "required": ["baseline"],
            "properties": {
                "baseline": {
                    "type": "object",
                    "properties": {
                        "net_profit": { "type": "number" },
                        "max_dd_pct": { "type": "number" },
                        "total_trades": { "type": "integer" }
                    }
                },
                "report_dir": { "type": "string" },
                "promote_dd_limit": { "type": "number" }
            }
        }
    })
}

fn tool_compile_ea() -> Value {
    json!({
        "name": "compile_ea",
        "description": "Compile an MQL5 Expert Advisor via MetaEditor. Provide either 'expert' (EA name, searches project and MT5 Experts dirs) or 'expert_path' (full path to .mq5 file).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "expert": { "type": "string", "description": "EA name without extension (e.g. 'DPS21')" },
                "expert_path": { "type": "string", "description": "Full path to the .mq5 source file" }
            }
        }
    })
}

fn tool_verify_setup() -> Value {
    json!({
        "name": "verify_setup",
        "description": "Verify MT5-Quant environment",
        "inputSchema": { "type": "object", "properties": {} }
    })
}

fn tool_list_symbols() -> Value {
    json!({
        "name": "list_symbols",
        "description": "List symbols with local tick history",
        "inputSchema": {
            "type": "object",
            "properties": {
                "server": { "type": "string" }
            }
        }
    })
}

fn tool_list_experts() -> Value {
    json!({
        "name": "list_experts",
        "description": "List all compiled Expert Advisors",
        "inputSchema": {
            "type": "object",
            "properties": {
                "filter": { "type": "string" }
            }
        }
    })
}

fn tool_list_indicators() -> Value {
    json!({
        "name": "list_indicators",
        "description": "List all custom indicators in MQL5/Indicators",
        "inputSchema": {
            "type": "object",
            "properties": {
                "filter": { "type": "string", "description": "Optional name filter pattern" },
                "include_builtin": { "type": "boolean", "description": "Include built-in MT5 indicators", "default": false }
            }
        }
    })
}

fn tool_list_scripts() -> Value {
    json!({
        "name": "list_scripts",
        "description": "List all scripts in MQL5/Scripts",
        "inputSchema": {
            "type": "object",
            "properties": {
                "filter": { "type": "string", "description": "Optional name filter pattern" }
            }
        }
    })
}

fn tool_get_backtest_status() -> Value {
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

fn tool_get_optimization_status() -> Value {
    json!({
        "name": "get_optimization_status",
        "description": "Check if an optimization job is running",
        "inputSchema": {
            "type": "object",
            "required": ["job_id"],
            "properties": {
                "job_id": { "type": "string" }
            }
        }
    })
}

fn tool_prune_reports() -> Value {
    json!({
        "name": "prune_reports",
        "description": "Delete old backtest report directories",
        "inputSchema": {
            "type": "object",
            "properties": {
                "keep_last": { "type": "integer" }
            }
        }
    })
}

fn tool_get_latest_report() -> Value {
    json!({
        "name": "get_latest_report",
        "description": "Get the latest backtest report with full details and equity chart image",
        "inputSchema": {
            "type": "object",
            "properties": {
                "include_chart": { "type": "boolean", "description": "Include equity chart as base64 PNG (default: true)", "default": true }
            }
        }
    })
}

fn tool_list_reports() -> Value {
    json!({
        "name": "list_reports",
        "description": "List most recent backtest reports from the central registry (SQLite). Returns id, metrics, set file, chart paths.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "limit": { "type": "integer", "description": "Max results (default 30)" }
            }
        }
    })
}

fn tool_search_reports() -> Value {
    json!({
        "name": "search_reports",
        "description": "Search backtest history with filters. All filters are optional and combinable.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "expert": { "type": "string", "description": "EA name substring match" },
                "symbol": { "type": "string", "description": "Exact symbol, e.g. XAUUSD" },
                "timeframe": { "type": "string", "description": "Exact timeframe, e.g. M5" },
                "after": { "type": "string", "description": "ISO8601 date — only reports created after this" },
                "min_profit": { "type": "number", "description": "Minimum net profit" },
                "max_dd": { "type": "number", "description": "Maximum drawdown %" },
                "verdict": { "type": "string", "enum": ["winner", "loser", "marginal", "reference"] },
                "limit": { "type": "integer", "description": "Max results (default 50)" }
            }
        }
    })
}

fn tool_tail_log() -> Value {
    json!({
        "name": "tail_log",
        "description": "Read the last N lines of a log file",
        "inputSchema": {
            "type": "object",
            "properties": {
                "n": { "type": "integer" },
                "filter": { "type": "string", "enum": ["all", "errors", "warnings"] },
                "log_file": { "type": "string" },
                "report_dir": { "type": "string" },
                "job_id": { "type": "string" }
            }
        }
    })
}

fn tool_cache_status() -> Value {
    json!({
        "name": "cache_status",
        "description": "Show MT5 tester cache size breakdown",
        "inputSchema": { "type": "object", "properties": {} }
    })
}

fn tool_clean_cache() -> Value {
    json!({
        "name": "clean_cache",
        "description": "Delete MT5 tester cache files",
        "inputSchema": {
            "type": "object",
            "properties": {
                "symbol": { "type": "string" },
                "dry_run": { "type": "boolean" }
            }
        }
    })
}

fn tool_read_set_file() -> Value {
    json!({
        "name": "read_set_file",
        "description": "Parse an MT5 .set parameter file",
        "inputSchema": {
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string" }
            }
        }
    })
}

fn tool_write_set_file() -> Value {
    json!({
        "name": "write_set_file",
        "description": "Write an MT5 .set parameter file",
        "inputSchema": {
            "type": "object",
            "required": ["path", "params"],
            "properties": {
                "path": { "type": "string" },
                "params": { "type": "object" }
            }
        }
    })
}

fn tool_patch_set_file() -> Value {
    json!({
        "name": "patch_set_file",
        "description": "Modify specific parameters in a .set file",
        "inputSchema": {
            "type": "object",
            "required": ["path", "patches"],
            "properties": {
                "path": { "type": "string" },
                "patches": { "type": "object" }
            }
        }
    })
}

fn tool_clone_set_file() -> Value {
    json!({
        "name": "clone_set_file",
        "description": "Copy a .set file to a new path with optional overrides",
        "inputSchema": {
            "type": "object",
            "required": ["source", "destination"],
            "properties": {
                "source": { "type": "string" },
                "destination": { "type": "string" },
                "overrides": { "type": "object" }
            }
        }
    })
}

fn tool_set_from_optimization() -> Value {
    json!({
        "name": "set_from_optimization",
        "description": "Generate .set file from optimization result params",
        "inputSchema": {
            "type": "object",
            "required": ["path", "params"],
            "properties": {
                "path": { "type": "string" },
                "params": { "type": "object" },
                "template": { "type": "string" },
                "sweep": { "type": "object" }
            }
        }
    })
}

fn tool_diff_set_files() -> Value {
    json!({
        "name": "diff_set_files",
        "description": "Compare two .set files",
        "inputSchema": {
            "type": "object",
            "required": ["path_a", "path_b"],
            "properties": {
                "path_a": { "type": "string" },
                "path_b": { "type": "string" }
            }
        }
    })
}

fn tool_describe_sweep() -> Value {
    json!({
        "name": "describe_sweep",
        "description": "Show .set file sweep configuration",
        "inputSchema": {
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string" }
            }
        }
    })
}

fn tool_list_set_files() -> Value {
    json!({
        "name": "list_set_files",
        "description": "List all .set files in tester profiles directory",
        "inputSchema": {
            "type": "object",
            "properties": {
                "ea": { "type": "string" }
            }
        }
    })
}

fn tool_list_jobs() -> Value {
    json!({
        "name": "list_jobs",
        "description": "List all optimization jobs",
        "inputSchema": {
            "type": "object",
            "properties": {
                "include_done": { "type": "boolean" }
            }
        }
    })
}

fn tool_archive_report() -> Value {
    json!({
        "name": "archive_report",
        "description": "Convert report to JSON and append to history",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": { "type": "string" },
                "delete_after": { "type": "boolean" },
                "notes": { "type": "string" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "verdict": { "type": "string", "enum": ["winner", "loser", "marginal", "reference"] }
            }
        }
    })
}

fn tool_archive_all_reports() -> Value {
    json!({
        "name": "archive_all_reports",
        "description": "Bulk-archive all backtest reports",
        "inputSchema": {
            "type": "object",
            "properties": {
                "delete_after": { "type": "boolean" },
                "keep_last": { "type": "integer" },
                "dry_run": { "type": "boolean" }
            }
        }
    })
}

fn tool_get_history() -> Value {
    json!({
        "name": "get_history",
        "description": "Query backtest history with filters",
        "inputSchema": {
            "type": "object",
            "properties": {
                "ea": { "type": "string" },
                "symbol": { "type": "string" },
                "tag": { "type": "string" },
                "verdict": { "type": "string" },
                "sort_by": { "type": "string" },
                "limit": { "type": "integer" },
                "include_monthly": { "type": "boolean" }
            }
        }
    })
}

fn tool_promote_to_baseline() -> Value {
    json!({
        "name": "promote_to_baseline",
        "description": "Promote a backtest result to baseline",
        "inputSchema": {
            "type": "object",
            "properties": {
                "history_id": { "type": "string" },
                "report_dir": { "type": "string" },
                "notes": { "type": "string" }
            }
        }
    })
}

fn tool_annotate_history() -> Value {
    json!({
        "name": "annotate_history",
        "description": "Add notes, verdict, or tags to a report entry in the registry",
        "inputSchema": {
            "type": "object",
            "required": ["history_id"],
            "properties": {
                "history_id": { "type": "string", "description": "Report id from list_reports or search_reports" },
                "notes": { "type": "string" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "verdict": { "type": "string", "enum": ["winner", "loser", "marginal", "reference"] }
            }
        }
    })
}

fn tool_healthcheck() -> Value {
    json!({
        "name": "healthcheck",
        "description": "System health check with OS detection and configuration validation",
        "inputSchema": {
            "type": "object",
            "properties": {
                "detailed": { "type": "boolean", "description": "Include detailed system info", "default": false }
            }
        }
    })
}
