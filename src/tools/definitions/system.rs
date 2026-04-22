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

pub fn tool_check_update() -> Value {
    json!({
        "name": "check_update",
        "description": "Check if a newer version of mt5-quant is available on GitHub. A background check runs automatically on the first tool call of each session; this tool returns that cached result instantly or fetches it on demand.",
        "inputSchema": {
            "type": "object",
            "properties": {}
        }
    })
}

pub fn tool_update() -> Value {
    json!({
        "name": "update",
        "description": "Download and install the latest mt5-quant binary from GitHub Releases, then replace the current executable in place. Restart the MCP connection after updating to load the new version.",
        "inputSchema": {
            "type": "object",
            "properties": {}
        }
    })
}
