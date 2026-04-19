# Antigravity MCP Integration Setup

## Quick Setup

### Option 1: Download Prebuilt Binary (Recommended)

```bash
# macOS (Apple Silicon)
curl -L -o mt5-quant.tar.gz https://github.com/masdevid/mt5-mcp/releases/latest/download/mt5-quant-macos-arm64.tar.gz
tar -xzf mt5-quant.tar.gz

# Linux (x64)
curl -L -o mt5-quant.tar.gz https://github.com/masdevid/mt5-mcp/releases/latest/download/mt5-quant-linux-x64.tar.gz
tar -xzf mt5-quant.tar.gz
```

### Option 2: Build from Source

```bash
cargo build --release
```

## Configure Antigravity

### Step 1: Open MCP Manager

1. Launch Antigravity
2. Look at the **right-side Agent Panel**
3. Click the **"..."** (More Options) menu at the top
4. Select **MCP Servers**

### Step 2: Access Configuration

1. Click **Manage MCP Servers**
2. Click **View raw config** or **Edit configuration**
3. This opens `mcp_config.json`

### Step 3: Add mt5-quant Configuration

Add to `mcp_config.json`:

```json
{
  "mcpServers": {
    "mt5-quant": {
      "command": "/absolute/path/to/mt5-quant"
    }
  }
}
```

### Step 4: Reload and Verify

1. Save the file
2. **Restart Antigravity** (or reload the window)
3. Open the Agent chat and type: `What tools do you have access to?`
4. The agent should list MT5 tools like `verify_setup`, `run_backtest`, etc.

## Full Example Configuration

```json
{
  "mcpServers": {
    "mt5-quant": {
      "command": "/Users/name/mt5-quant/target/release/mt5-quant"
    },
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "${env:GITHUB_TOKEN}"
      }
    }
  }
}
```

## Environment Variables

Antigravity supports `${VAR_NAME}` syntax for environment variable substitution:

```json
{
  "mcpServers": {
    "mt5-quant": {
      "command": "/Users/name/mt5-quant/target/release/mt5-quant",
      "env": {
        "CUSTOM_VAR": "${env:MY_VAR}"
      }
    }
  }
}
```

This keeps secrets out of the config file.

## Verify Setup

In Antigravity chat, type:

```
Run verify_setup
```

Expected output:
```
Wine:    /Applications/MetaTrader 5.app/.../wine64
MT5 dir: ~/Library/Application Support/.../MetaTrader 5
Display: gui
Arch:    arch -x86_64
```

## Troubleshooting

### "Connection Refused" or "Tool not found"

1. Double-check the server path in `mcp_config.json`
2. Ensure the binary has execute permissions: `chmod +x /path/to/mt5-quant`
3. Try completely restarting Antigravity
4. Check the server is listed in MCP manager

### "Stdio Error" or JSON Parsing Error

1. Verify the JSON syntax in `mcp_config.json`
2. Use a JSON validator if needed
3. Ensure no trailing commas

### Agent Hallucinating Tool Parameters

If the agent makes up incorrect parameters:
1. Check the tool is actually available: "List your available tools"
2. Restart the agent session
3. Be explicit in your requests

## Configuration Location

| Platform | Path |
|----------|------|
| All | Via UI: Agent Panel → ... → MCP Servers → Manage → Edit configuration |

## Resources

- [Antigravity Documentation](https://docs.antigravity.dev/)
- [MCP Server Reference](https://github.com/modelcontextprotocol/servers)
- [MT5-Quant Tools Reference](./MCP_TOOLS.md)
