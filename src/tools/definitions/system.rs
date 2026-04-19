use serde_json::{json, Value};

pub fn tool_healthcheck() -> Value {
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

pub fn tool_verify_setup() -> Value {
    json!({
        "name": "verify_setup",
        "description": "Validate MT5-Quant environment configuration",
        "inputSchema": {
            "type": "object"
        }
    })
}

pub fn tool_list_symbols() -> Value {
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

pub fn tool_get_active_account() -> Value {
    json!({
        "name": "get_active_account",
        "description": "Get the currently active MT5 account session info (login, server, available symbols). Use this before backtesting to ensure symbol compatibility.",
        "inputSchema": {
            "type": "object"
        }
    })
}
