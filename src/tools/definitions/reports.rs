use serde_json::{json, Value};

pub fn tool_get_latest_report() -> Value {
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

pub fn tool_list_reports() -> Value {
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

pub fn tool_search_reports() -> Value {
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
                "verdict": { "type": "string", "description": "Only reports with this verdict (pass, fail, marginal)" },
                "limit": { "type": "integer", "default": 50 }
            }
        }
    })
}

pub fn tool_tail_log() -> Value {
    json!({
        "name": "tail_log",
        "description": "Stream the last N lines of a job log",
        "inputSchema": {
            "type": "object",
            "properties": {
                "job_id": { "type": "string" },
                "file": { "type": "string" },
                "lines": { "type": "integer", "default": 50 }
            }
        }
    })
}

pub fn tool_archive_report() -> Value {
    json!({
        "name": "archive_report",
        "description": "Compress a backtest report directory to tar.gz",
        "inputSchema": {
            "type": "object",
            "required": ["report_dir"],
            "properties": {
                "report_dir": { "type": "string" },
                "delete_after": { "type": "boolean" }
            }
        }
    })
}

pub fn tool_archive_all_reports() -> Value {
    json!({
        "name": "archive_all_reports",
        "description": "Archive all old reports except the N most recent",
        "inputSchema": {
            "type": "object",
            "properties": {
                "keep_last": { "type": "integer" }
            }
        }
    })
}

pub fn tool_get_history() -> Value {
    json!({
        "name": "get_history",
        "description": "Retrieve backtest history for an EA or symbol",
        "inputSchema": {
            "type": "object",
            "properties": {
                "ea": { "type": "string" },
                "symbol": { "type": "string" },
                "verdict": { "type": "string" },
                "limit": { "type": "integer" }
            }
        }
    })
}

pub fn tool_promote_to_baseline() -> Value {
    json!({
        "name": "promote_to_baseline",
        "description": "Copy report metrics to config/baseline.json for future comparisons",
        "inputSchema": {
            "type": "object",
            "required": ["report_dir"],
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

pub fn tool_annotate_history() -> Value {
    json!({
        "name": "annotate_history",
        "description": "Add notes, tags or verdict to a history entry",
        "inputSchema": {
            "type": "object",
            "required": ["history_id"],
            "properties": {
                "history_id": { "type": "string" },
                "notes": { "type": "string" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "verdict": { "type": "string", "enum": ["pass", "fail", "marginal"] }
            }
        }
    })
}

pub fn tool_prune_reports() -> Value {
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
