# Cursor MCP Integration Setup

## Quick Setup

### Option 1: Download Prebuilt Binary (Recommended)

```bash
# macOS (Apple Silicon)
curl -L -o mt5.tar.gz https://github.com/masdevid/mt5-quant/releases/latest/download/mcp-mt5-quant-macos-arm64.tar.gz
tar -xzf mt5.tar.gz

# Linux (x64)
curl -L -o mt5.tar.gz https://github.com/masdevid/mt5-quant/releases/latest/download/mcp-mt5-quant-linux-x64.tar.gz
tar -xzf mt5.tar.gz
```

### Option 2: Build from Source

```bash
cargo build --release
```

## Configure Cursor

### Method 1: Settings UI (Recommended)

1. Open Cursor Settings (`Cmd/Ctrl + ,`)
2. Navigate to **Features** → **MCP**
3. Click **Add Custom MCP**
4. Enter:
   - **Name**: `mt5-quant`
   - **Command**: `/path/to/mt5-quant/mcp-server/bin/mt5-quant`
   - **Type**: `stdio`

### Method 2: Edit mcp.json Directly

Add to `~/.cursor/mcp.json` (global) or `.cursor/mcp.json` (project):

```json
{
  "mcpServers": {
    "mt5-quant": {
      "type": "stdio",
      "command": "/path/to/mt5-quant/mcp-server/bin/mt5-quant"
    }
  }
}
```

Create the file if it doesn't exist:

```bash
mkdir -p ~/.cursor
cat > ~/.cursor/mcp.json << 'EOF'
{
  "mcpServers": {
    "mt5-quant": {
      "type": "stdio",
      "command": "/path/to/mt5-quant/mcp-server/bin/mt5-quant"
    }
  }
}
EOF
```

## Verify Setup

In Cursor chat, type:

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

## Configuration Locations

| Scope | Path | Use Case |
|-------|------|----------|
| Global | `~/.cursor/mcp.json` | Available in all projects |
| Project | `.cursor/mcp.json` | Project-specific tools |

## Troubleshooting

### MCP server not appearing

1. Check MCP panel in Cursor Settings
2. Verify the path is absolute (not relative)
3. Test binary: `/path/to/mt5-quant/mcp-server/bin/mt5-quant --help`
4. View MCP logs: Output panel → select "MCP" from dropdown

### Config interpolation

Cursor supports variable substitution in `mcp.json`:

```json
{
  "mcpServers": {
    "mt5-quant": {
      "command": "${userHome}/mt5-quant/mcp-server/bin/mt5-quant",
      "args": ["--config", "${workspaceFolder}/config.yaml"]
    }
  }
}
```

Available variables:
- `${userHome}` - Home directory
- `${workspaceFolder}` - Project root
- `${workspaceFolderBasename}` - Project folder name
- `${env:VAR_NAME}` - Environment variable

### Tool not found errors

If the agent says "Tool not found":
1. Check the server is enabled in MCP settings
2. Try disabling and re-enabling the server
3. Restart Cursor

## Resources

- [Cursor MCP Documentation](https://cursor.com/docs/context/mcp)
- [MT5-Quant Tools Reference](./MCP_TOOLS.md)
