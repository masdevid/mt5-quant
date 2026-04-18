use serde_json::{json, Value};

pub fn tool_run_optimization() -> Value {
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

pub fn tool_get_optimization_status() -> Value {
    json!({
        "name": "get_optimization_status",
        "description": "Check progress of a running optimization job",
        "inputSchema": {
            "type": "object",
            "required": ["job_id"],
            "properties": {
                "job_id": { "type": "string" }
            }
        }
    })
}

pub fn tool_get_optimization_results() -> Value {
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

pub fn tool_list_jobs() -> Value {
    json!({
        "name": "list_jobs",
        "description": "List running and completed optimization jobs",
        "inputSchema": {
            "type": "object"
        }
    })
}
