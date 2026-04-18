use serde_json::{json, Value};

pub fn tool_compile_ea() -> Value {
    json!({
        "name": "compile_ea",
        "description": "Compile an MQL5 Expert Advisor via MetaEditor. Provide either 'expert' (EA name, searches project and MT5 Experts dirs) or 'expert_path' (full path to .mq5 file).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "expert": { "type": "string", "description": "EA name without extension (e.g. 'DPS21')" },
                "expert_path": { "type": "string", "description": "Full path to the .mq5 source file" }
            }
        }
    })
}

pub fn tool_list_experts() -> Value {
    json!({
        "name": "list_experts",
        "description": "List all compiled Expert Advisors",
        "inputSchema": {
            "type": "object",
            "properties": {
                "filter": { "type": "string" }
            }
        }
    })
}

pub fn tool_list_indicators() -> Value {
    json!({
        "name": "list_indicators",
        "description": "List all custom indicators in MQL5/Indicators",
        "inputSchema": {
            "type": "object",
            "properties": {
                "filter": { "type": "string", "description": "Optional name filter pattern" },
                "include_builtin": { "type": "boolean", "description": "Include built-in MT5 indicators", "default": false }
            }
        }
    })
}

pub fn tool_list_scripts() -> Value {
    json!({
        "name": "list_scripts",
        "description": "List all scripts in MQL5/Scripts",
        "inputSchema": {
            "type": "object",
            "properties": {
                "filter": { "type": "string", "description": "Optional name filter pattern" }
            }
        }
    })
}
