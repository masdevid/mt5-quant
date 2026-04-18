use serde_json::{json, Value};

pub fn tool_run_backtest() -> Value {
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
