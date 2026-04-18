use serde_json::{json, Value};

pub fn tool_compare_baseline() -> Value {
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
