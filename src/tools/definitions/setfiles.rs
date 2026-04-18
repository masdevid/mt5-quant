use serde_json::{json, Value};

pub fn tool_read_set_file() -> Value {
    json!({
        "name": "read_set_file",
        "description": "Read a .set parameter file into a JSON object",
        "inputSchema": {
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string" }
            }
        }
    })
}

pub fn tool_write_set_file() -> Value {
    json!({
        "name": "write_set_file",
        "description": "Write a JSON object to a .set parameter file",
        "inputSchema": {
            "type": "object",
            "required": ["path", "parameters"],
            "properties": {
                "path": { "type": "string" },
                "parameters": { "type": "object" }
            }
        }
    })
}

pub fn tool_patch_set_file() -> Value {
    json!({
        "name": "patch_set_file",
        "description": "Update specific keys in an existing .set file",
        "inputSchema": {
            "type": "object",
            "required": ["path", "patches"],
            "properties": {
                "path": { "type": "string" },
                "patches": { "type": "object" }
            }
        }
    })
}

pub fn tool_clone_set_file() -> Value {
    json!({
        "name": "clone_set_file",
        "description": "Duplicate an existing .set file to a new path",
        "inputSchema": {
            "type": "object",
            "required": ["source", "destination"],
            "properties": {
                "source": { "type": "string" },
                "destination": { "type": "string" }
            }
        }
    })
}

pub fn tool_diff_set_files() -> Value {
    json!({
        "name": "diff_set_files",
        "description": "Compare two .set files and return differences",
        "inputSchema": {
            "type": "object",
            "required": ["file_a", "file_b"],
            "properties": {
                "file_a": { "type": "string" },
                "file_b": { "type": "string" }
            }
        }
    })
}

pub fn tool_set_from_optimization() -> Value {
    json!({
        "name": "set_from_optimization",
        "description": "Generate a .set file from optimization best pass results",
        "inputSchema": {
            "type": "object",
            "required": ["path", "params"],
            "properties": {
                "path": { "type": "string" },
                "params": { "type": "object" }
            }
        }
    })
}

pub fn tool_describe_sweep() -> Value {
    json!({
        "name": "describe_sweep",
        "description": "List the parameters being swept in a .set file",
        "inputSchema": {
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string" }
            }
        }
    })
}

pub fn tool_list_set_files() -> Value {
    json!({
        "name": "list_set_files",
        "description": "List all .set files in the tester profiles directory",
        "inputSchema": {
            "type": "object"
        }
    })
}
