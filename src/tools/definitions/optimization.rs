use serde_json::{json, Value};

pub fn tool_run_optimization() -> Value {
    json!({
        "name": "run_optimization",
        "description": "Launch MT5 genetic parameter optimization in fire-and-forget mode. Returns immediately with job_id. Use get_optimization_status to poll for completion. Optimization typically runs for 2-6 hours.",
        "inputSchema": {
            "type": "object",
            "required": ["expert", "set_file", "from_date", "to_date"],
            "properties": {
                "expert": { "type": "string", "description": "EA name without path or extension" },
                "set_file": { "type": "string", "description": "Path to .set file with parameter ranges for optimization" },
                "symbol": { "type": "string", "description": "Trading symbol (default: XAUUSD)" },
                "from_date": { "type": "string", "description": "Start date YYYY.MM.DD" },
                "to_date": { "type": "string", "description": "End date YYYY.MM.DD" },
                "deposit": { "type": "integer", "description": "Initial deposit (default: 10000)" }
            }
        }
    })
}

pub fn tool_get_optimization_status() -> Value {
    json!({
        "name": "get_optimization_status",
        "description": "Check progress of a running optimization job. Poll periodically until status shows 'completed'.",
        "inputSchema": {
            "type": "object",
            "required": ["job_id"],
            "properties": {
                "job_id": { "type": "string", "description": "Job ID returned by run_optimization" }
            }
        }
    })
}

pub fn tool_get_optimization_results() -> Value {
    json!({
        "name": "get_optimization_results",
        "description": "Parse completed MT5 optimization results and find best parameter combinations",
        "inputSchema": {
            "type": "object",
            "properties": {
                "job_id": { "type": "string", "description": "Job ID to parse results for" },
                "report_file": { "type": "string", "description": "Direct path to optimization report XML file" },
                "dd_threshold": { "type": "number", "description": "Max drawdown percentage filter" },
                "top_n": { "type": "integer", "description": "Number of top passes to return (default: 30)" }
            }
        }
    })
}

pub fn tool_list_jobs() -> Value {
    json!({
        "name": "list_jobs",
        "description": "List all running and completed optimization jobs with their status",
        "inputSchema": {
            "type": "object"
        }
    })
}
