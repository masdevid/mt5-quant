use serde_json::{json, Value};

pub fn tool_check_update() -> Value {
    json!({
        "name": "check_update",
        "description": "Check if a newer version of mt5-quant is available on GitHub. A background check runs automatically on the first tool call of each session; this tool returns that cached result instantly or fetches it on demand.",
        "inputSchema": {
            "type": "object"
        }
    })
}

pub fn tool_update() -> Value {
    json!({
        "name": "update",
        "description": "Download and install the latest mt5-quant binary from GitHub Releases, then replace the current executable in place. Restart the MCP connection after updating to load the new version.",
        "inputSchema": {
            "type": "object"
        }
    })
}
