use serde_json::{json, Value};

pub fn tool_analyze_report() -> Value {
    json!({
        "name": "analyze_report",
        "description": "Run comprehensive analytics on a backtest report. By default runs all analytics. Use 'analytics' array to run specific ones: monthly_pnl, drawdown_events, top_losses, loss_sequences, position_pairs, direction_bias, streak_analysis, concurrent_peak",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": { "type": "string", "description": "Path to report directory containing deals.csv" },
                "analytics": {
                    "type": "array",
                    "description": "Optional: specific analytics to run. If omitted, runs all.",
                    "items": {
                        "type": "string",
                        "enum": ["monthly_pnl", "drawdown_events", "top_losses", "loss_sequences", "position_pairs", "direction_bias", "streak_analysis", "concurrent_peak"]
                    }
                },
                "top_losses_limit": { "type": "integer", "description": "Number of top losses to return (default: 10)" }
            }
        }
    })
}

pub fn tool_analyze_monthly_pnl() -> Value {
    json!({
        "name": "analyze_monthly_pnl",
        "description": "Analyze monthly profit/loss breakdown from a backtest report",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": { "type": "string", "description": "Path to report directory containing deals.csv" }
            }
        }
    })
}

pub fn tool_analyze_drawdown_events() -> Value {
    json!({
        "name": "analyze_drawdown_events",
        "description": "Analyze drawdown events reconstructed from balance curve",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

pub fn tool_analyze_top_losses() -> Value {
    json!({
        "name": "analyze_top_losses",
        "description": "Get top N worst losses with grid depth analysis",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": { "type": "string" },
                "limit": { "type": "integer", "description": "Number of losses to return (default: 10)", "default": 10 }
            }
        }
    })
}

pub fn tool_analyze_loss_sequences() -> Value {
    json!({
        "name": "analyze_loss_sequences",
        "description": "Analyze consecutive loss streaks and their impact",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

pub fn tool_analyze_position_pairs() -> Value {
    json!({
        "name": "analyze_position_pairs",
        "description": "Analyze entry/exit position pairs and their performance",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

pub fn tool_analyze_direction_bias() -> Value {
    json!({
        "name": "analyze_direction_bias",
        "description": "Analyze long vs short performance bias",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

pub fn tool_analyze_streaks() -> Value {
    json!({
        "name": "analyze_streaks",
        "description": "Analyze win/loss streaks with dates and current streak",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

pub fn tool_analyze_concurrent_peak() -> Value {
    json!({
        "name": "analyze_concurrent_peak",
        "description": "Find peak number of concurrent open positions",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}
