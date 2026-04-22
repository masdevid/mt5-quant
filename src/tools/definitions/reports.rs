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

pub fn tool_get_report_by_id() -> Value {
    json!({
        "name": "get_report_by_id",
        "description": "Get a specific report by its ID with full details and optional equity chart",
        "inputSchema": {
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string", "description": "Report ID (from list_reports or search_reports)" },
                "include_chart": { "type": "boolean", "description": "Include equity chart as base64 PNG (default: true)", "default": true }
            }
        }
    })
}

pub fn tool_get_reports_summary() -> Value {
    json!({
        "name": "get_reports_summary",
        "description": "Get aggregate statistics across reports - counts, averages by EA/symbol/timeframe/verdict",
        "inputSchema": {
            "type": "object",
            "properties": {
                "expert": { "type": "string", "description": "Filter by EA name substring" },
                "symbol": { "type": "string", "description": "Filter by exact symbol" },
                "timeframe": { "type": "string", "description": "Filter by exact timeframe" },
                "verdict": { "type": "string", "description": "Filter by verdict (pass/fail/marginal)" }
            }
        }
    })
}

pub fn tool_get_best_reports() -> Value {
    json!({
        "name": "get_best_reports",
        "description": "Get top N reports sorted by performance metric (profit factor, win rate, drawdown, etc.)",
        "inputSchema": {
            "type": "object",
            "properties": {
                "sort_by": { "type": "string", "enum": ["net_profit", "profit_factor", "max_dd_pct", "win_rate_pct", "sharpe_ratio", "recovery_factor", "total_trades"], "default": "profit_factor", "description": "Metric to sort by" },
                "order": { "type": "string", "enum": ["asc", "desc"], "default": "desc", "description": "Sort order (use 'asc' for drawdown, 'desc' for profit)" },
                "limit": { "type": "integer", "default": 10, "description": "Number of reports to return" },
                "expert": { "type": "string", "description": "Filter by EA name substring" },
                "symbol": { "type": "string", "description": "Filter by exact symbol" },
                "timeframe": { "type": "string", "description": "Filter by exact timeframe" },
                "verdict": { "type": "string", "description": "Filter by verdict" }
            }
        }
    })
}

pub fn tool_search_reports_by_tags() -> Value {
    json!({
        "name": "search_reports_by_tags",
        "description": "Search reports by tags - at least one tag must match (OR logic)",
        "inputSchema": {
            "type": "object",
            "required": ["tags"],
            "properties": {
                "tags": { "type": "array", "items": { "type": "string" }, "description": "Tags to search for (e.g., ['production', 'verified'])" },
                "limit": { "type": "integer", "default": 50 }
            }
        }
    })
}

pub fn tool_search_reports_by_date_range() -> Value {
    json!({
        "name": "search_reports_by_date_range",
        "description": "Search reports by backtest date range (from_date and to_date fields)",
        "inputSchema": {
            "type": "object",
            "properties": {
                "from_start": { "type": "string", "description": "From date >= this (YYYY.MM.DD)" },
                "from_end": { "type": "string", "description": "From date <= this (YYYY.MM.DD)" },
                "to_start": { "type": "string", "description": "To date >= this (YYYY.MM.DD)" },
                "to_end": { "type": "string", "description": "To date <= this (YYYY.MM.DD)" },
                "limit": { "type": "integer", "default": 50 }
            }
        }
    })
}

pub fn tool_search_reports_by_notes() -> Value {
    json!({
        "name": "search_reports_by_notes",
        "description": "Full-text search in report notes field (case-insensitive LIKE search)",
        "inputSchema": {
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": { "type": "string", "description": "Search text (partial match)" },
                "limit": { "type": "integer", "default": 50 }
            }
        }
    })
}

pub fn tool_get_reports_by_set_file() -> Value {
    json!({
        "name": "get_reports_by_set_file",
        "description": "Find all reports that used a specific .set parameter file",
        "inputSchema": {
            "type": "object",
            "required": ["set_file"],
            "properties": {
                "set_file": { "type": "string", "description": "Set filename or partial path to match" },
                "limit": { "type": "integer", "default": 50 }
            }
        }
    })
}

pub fn tool_get_comparable_reports() -> Value {
    json!({
        "name": "get_comparable_reports",
        "description": "Find reports comparable to a given report (same EA, symbol, timeframe) - useful for before/after analysis",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_id": { "type": "string", "description": "Reference report ID (if provided, uses its expert/symbol/timeframe)" },
                "expert": { "type": "string", "description": "EA name (required if report_id not provided)" },
                "symbol": { "type": "string", "description": "Symbol (required if report_id not provided)" },
                "timeframe": { "type": "string", "description": "Timeframe (required if report_id not provided)" },
                "exclude_id": { "type": "string", "description": "Exclude this report ID from results" },
                "limit": { "type": "integer", "default": 20 }
            }
        }
    })
}

pub fn tool_export_deals_csv() -> Value {
    json!({
        "name": "export_deals_csv",
        "description": "Export deals for a report to a CSV file on demand. Deals are stored in the database — use this to get a CSV file for external tools.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_id": { "type": "string", "description": "Report ID to export (default: latest report)" },
                "output_path": { "type": "string", "description": "File path for the CSV output (default: <report_dir>/deals.csv)" }
            }
        }
    })
}
