---
description: Create a new MCP tool for MT5-Quant with full build, install, and test cycle
---

# MCP Tool Development Workflow

Complete workflow for adding a new MCP tool to MT5-Quant.

## Step 1: Define Tool Schema

Add the tool definition to `src/tools/definitions.rs`:

1. Create a function `tool_<your_tool_name>()` that returns `serde_json::Value`
2. Add the tool to the `get_tools_list()` vector
3. Follow the JSON schema format:
   - `name`: Tool identifier (snake_case)
   - `description`: Clear purpose statement
   - `inputSchema`: JSON Schema with `required` and `properties`

**Template:**
```rust
fn tool_<your_tool_name>() -> Value {
    json!({
        "name": "<tool_name>",
        "description": "<clear description of what it does>",
        "inputSchema": {
            "type": "object",
            "required": ["<param1>"],
            "properties": {
                "<param1>": { "type": "string", "description": "..." },
                "<param2>": { "type": "integer", "description": "..." }
            }
        }
    })
}
```

## Step 2: Implement Handler

Add the handler implementation in `src/tools/handlers.rs`:

1. Add a match arm in `ToolHandler::handle()` dispatch
2. Implement `handle_<your_tool_name>()` method
3. Return `Result<Value>` with proper MCP response format:

```rust
async fn handle_<your_tool_name>(&self, args: &Value) -> Result<Value> {
    // Extract parameters
    let param = args.get("param").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: param"))?;
    
    // Implementation logic here
    let result = /* ... */;
    
    Ok(json!({
        "content": [{ 
            "type": "text", 
            "text": json!({
                "success": true,
                "data": result
            }).to_string() 
        }],
        "isError": false
    }))
}
```

## Step 3: Build Release

// turbo
Run the release build script:
```bash
./scripts/build-release.sh
```

This creates `dist/mt5-quant-{platform}.tar.gz` with the binary and config.

## Step 4: Install

Extract and install the new binary:
```bash
tar -xzf dist/mt5-quant-*.tar.gz
cp mt5-quant-*/mt5-quant ~/.cargo/bin/mt5-quant
```

Or install directly from cargo:
```bash
cargo install --path . --force
```

## Step 5: Test with MCP Request

Test the tool using the MCP server. Start the server and send a request:

```bash
# Start the MCP server
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | mt5-quant

# List tools to verify registration
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | mt5-quant

# Call your new tool
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"<your_tool_name>","arguments":{"<param>":"value"}}}' | mt5-quant
```

**Expected output format:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [{"type": "text", "text": "{\"success\":true,...}"}],
    "isError": false
  }
}
```

## Example: Complete Tool Implementation

**In `src/tools/definitions.rs`:**
```rust
fn tool_healthcheck() -> Value {
    json!({
        "name": "healthcheck",
        "description": "System health check with OS detection and configuration validation",
        "inputSchema": {
            "type": "object",
            "properties": {
                "detailed": { "type": "boolean", "description": "Include detailed system info" }
            }
        }
    })
}
```

**Add to `get_tools_list()`:**
```rust
let tools = vec![
    // ... existing tools
    tool_healthcheck(),  // Add here
];
```

**In `src/tools/handlers.rs` - add match arm:**
```rust
"healthcheck" => self.handle_healthcheck(args).await,
```

**Add handler method with OS detection:**
```rust
async fn handle_healthcheck(&self, args: &Value) -> Result<Value> {
    let detailed = args.get("detailed").and_then(|v| v.as_bool()).unwrap_or(false);
    
    // OS Detection
    let platform = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    
    // Configuration validation
    let config_path = Config::writable_config_path();
    let config_exists = config_path.exists();
    
    let wine_found = self.config.wine_executable.as_ref()
        .map(|p| Path::new(p).exists())
        .unwrap_or(false);
    
    let mt5_dir_found = self.config.terminal_dir.as_ref()
        .map(|p| Path::new(p).is_dir())
        .unwrap_or(false);
    
    let healthy = config_exists && wine_found && mt5_dir_found;
    
    let mut response = json!({
        "success": true,
        "healthy": healthy,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "os": {
            "platform": platform,
            "arch": arch,
        },
        "configuration": {
            "config_exists": config_exists,
            "wine_found": wine_found,
            "mt5_dir_found": mt5_dir_found,
        },
    });
    
    if detailed {
        response["detailed"] = json!({
            "exe_path": std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
        });
    }
    
    Ok(json!({
        "content": [{ "type": "text", "text": response.to_string() }],
        "isError": false
    }))
}
```

**Test the tool:**
```bash
# Basic healthcheck
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"healthcheck","arguments":{}}}' | mt5-quant

# Detailed healthcheck with system info
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"healthcheck","arguments":{"detailed":true}}}' | mt5-quant
```

## Testing Checklist

- [ ] Tool appears in `tools/list` output
- [ ] Required parameters are validated
- [ ] Error responses have `isError: true`
- [ ] Success responses have `isError: false`
- [ ] Output is valid JSON in the `text` field
- [ ] Handler doesn't panic on invalid input
