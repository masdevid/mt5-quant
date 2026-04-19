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

pub fn tool_list_deals() -> Value {
    json!({
        "name": "list_deals",
        "description": "List individual deals from a backtest report with optional filters",
        "inputSchema": {
            "type": "object",
            "required": ["report_dir"],
            "properties": {
                "report_dir": { "type": "string", "description": "Path to report directory containing deals.csv" },
                "deal_type": { "type": "string", "enum": ["buy", "sell"], "description": "Filter by deal type" },
                "min_profit": { "type": "number", "description": "Minimum profit (use negative for losses)" },
                "max_profit": { "type": "number", "description": "Maximum profit" },
                "start_date": { "type": "string", "description": "Start date filter (YYYY.MM.DD)" },
                "end_date": { "type": "string", "description": "End date filter (YYYY.MM.DD)" },
                "min_volume": { "type": "number", "description": "Minimum volume/lots" },
                "max_volume": { "type": "number", "description": "Maximum volume/lots" },
                "limit": { "type": "integer", "default": 100, "description": "Max deals to return" }
            }
        }
    })
}

pub fn tool_search_deals_by_comment() -> Value {
    json!({
        "name": "search_deals_by_comment",
        "description": "Search deals by comment text (case-insensitive partial match)",
        "inputSchema": {
            "type": "object",
            "required": ["report_dir", "query"],
            "properties": {
                "report_dir": { "type": "string" },
                "query": { "type": "string", "description": "Search text in comments" },
                "limit": { "type": "integer", "default": 50 }
            }
        }
    })
}

pub fn tool_search_deals_by_magic() -> Value {
    json!({
        "name": "search_deals_by_magic",
        "description": "Filter deals by magic number (EA identifier)",
        "inputSchema": {
            "type": "object",
            "required": ["report_dir", "magic"],
            "properties": {
                "report_dir": { "type": "string" },
                "magic": { "type": "string", "description": "Magic number to filter by" },
                "limit": { "type": "integer", "default": 100 }
            }
        }
    })
}

pub fn tool_analyze_profit_distribution() -> Value {
    json!({
        "name": "analyze_profit_distribution",
        "description": "Analyze profit distribution - small/medium/large wins and losses with detailed buckets",
        "inputSchema": {
            "type": "object",
            "required": ["report_dir"],
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

pub fn tool_analyze_time_performance() -> Value {
    json!({
        "name": "analyze_time_performance",
        "description": "Analyze performance by hour of day and day of week",
        "inputSchema": {
            "type": "object",
            "required": ["report_dir"],
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

pub fn tool_analyze_hold_time_distribution() -> Value {
    json!({
        "name": "analyze_hold_time_distribution",
        "description": "Analyze hold time distribution and correlation with profit",
        "inputSchema": {
            "type": "object",
            "required": ["report_dir"],
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

pub fn tool_analyze_layer_performance() -> Value {
    json!({
        "name": "analyze_layer_performance",
        "description": "Analyze performance by grid layer (extracted from deal comments)",
        "inputSchema": {
            "type": "object",
            "required": ["report_dir"],
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

pub fn tool_analyze_volume_vs_profit() -> Value {
    json!({
        "name": "analyze_volume_vs_profit",
        "description": "Analyze correlation between volume and profit, plus performance by volume bucket",
        "inputSchema": {
            "type": "object",
            "required": ["report_dir"],
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

pub fn tool_analyze_costs() -> Value {
    json!({
        "name": "analyze_costs",
        "description": "Analyze commission and swap costs impact on profitability",
        "inputSchema": {
            "type": "object",
            "required": ["report_dir"],
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}

pub fn tool_analyze_efficiency() -> Value {
    json!({
        "name": "analyze_efficiency",
        "description": "Calculate efficiency metrics: profit per hour/day, annualized return, trade frequency",
        "inputSchema": {
            "type": "object",
            "required": ["report_dir"],
            "properties": {
                "report_dir": { "type": "string" }
            }
        }
    })
}
