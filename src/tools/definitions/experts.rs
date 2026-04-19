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

pub fn tool_search_experts() -> Value {
    json!({
        "name": "search_experts",
        "description": "Search for Expert Advisors by name pattern across MT5 Experts directory and subdirectories",
        "inputSchema": {
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Search pattern (case-insensitive substring match)"
                }
            },
            "required": ["pattern"]
        }
    })
}

pub fn tool_search_indicators() -> Value {
    json!({
        "name": "search_indicators",
        "description": "Search for indicators by name pattern across MT5 directories and subdirectories",
        "inputSchema": {
            "type": "object",
            "properties": {
                "pattern": { 
                    "type": "string", 
                    "description": "Search pattern (case-insensitive substring match)" 
                },
                "include_builtin": { 
                    "type": "boolean", 
                    "description": "Include built-in MT5 indicators in search",
                    "default": false
                }
            },
            "required": ["pattern"]
        }
    })
}

pub fn tool_search_scripts() -> Value {
    json!({
        "name": "search_scripts",
        "description": "Search for scripts by name pattern across MT5 Scripts directory and subdirectories",
        "inputSchema": {
            "type": "object",
            "properties": {
                "pattern": { 
                    "type": "string", 
                    "description": "Search pattern (case-insensitive substring match)" 
                }
            },
            "required": ["pattern"]
        }
    })
}

pub fn tool_copy_indicator_to_project() -> Value {
    json!({
        "name": "copy_indicator_to_project",
        "description": "Copy an indicator file from MT5 Indicators directory to the project directory",
        "inputSchema": {
            "type": "object",
            "properties": {
                "source_path": { 
                    "type": "string", 
                    "description": "Full source path to the indicator file (.mq5 or .ex5)" 
                },
                "target_name": { 
                    "type": "string", 
                    "description": "Optional: Rename the file (without extension). If not provided, uses original name" 
                }
            },
            "required": ["source_path"]
        }
    })
}

pub fn tool_copy_script_to_project() -> Value {
    json!({
        "name": "copy_script_to_project",
        "description": "Copy a script file from MT5 Scripts directory to the project directory",
        "inputSchema": {
            "type": "object",
            "properties": {
                "source_path": { 
                    "type": "string", 
                    "description": "Full source path to the script file (.mq5 or .ex5)" 
                },
                "target_name": { 
                    "type": "string", 
                    "description": "Optional: Rename the file (without extension). If not provided, uses original name" 
                }
            },
            "required": ["source_path"]
        }
    })
}
