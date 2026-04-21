# VS Code MCP Integration Setup

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

## Configure VS Code

### Method 1: Command Palette (Recommended)

1. Press `Cmd/Ctrl + Shift + P`
2. Run `MCP: Add Server`
3. Choose **Workspace** or **User** scope
4. Enter server name: `mt5-quant`
5. Enter command: `~/.local/bin/mt5-quant`

### Method 2: Edit mcp.json Directly

Add to `.vscode/mcp.json` in your workspace:

```json
{
  "servers": {
    "mt5-quant": {
      "command": "~/.local/bin/mt5-quant"
    }
  }
}
```

Create the file:

```bash
mkdir -p .vscode
cat > .vscode/mcp.json << 'EOF'
{
  "servers": {
    "mt5-quant": {
      "command": "~/.local/bin/mt5-quant"
    }
  }
}
EOF
```

### Method 3: VS Code CLI

```bash
code --add-mcp '{"name":"mt5-quant","command":"~/.local/bin/mt5-quant"}'
```

## Verify Setup

In Copilot chat, type:

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
| Workspace | `.vscode/mcp.json` | Share with team via source control |
| User | `~/.vscode/mcp.json` | Personal tools across all projects |
| Dev Container | `devcontainer.json` → `customizations.vscode.mcp` | Containerized environments |

## Troubleshooting

### MCP server not appearing

1. Open **Output** panel (`Cmd/Ctrl + Shift + U`)
2. Select **MCP** from dropdown
3. Check for connection errors
4. Verify the path is absolute

### Config not found

The binary auto-detects its config, but you can also:
1. Run `setup.sh` to create `config/mt5-quant.yaml`
2. Or let the binary auto-discover on first run

### Dev Container Setup

Add to `.devcontainer/devcontainer.json`:

```json
{
  "customizations": {
    "vscode": {
      "mcp": {
        "servers": {
          "mt5-quant": {
            "command": "/path/to/mt5-quant"
          }
        }
      }
    }
  }
}
```

## Key Differences from Other IDEs

VS Code uses `servers` (not `mcpServers`) in the JSON structure:

```json
{
  "servers": {        // ← VS Code uses "servers"
    "mt5-quant": {
      "command": "..."
    }
  }
}
```

Other platforms use `mcpServers`.

## Resources

- [VS Code MCP Documentation](https://code.visualstudio.com/docs/copilot/customization/mcp-servers)
- [MCP Configuration Reference](https://code.visualstudio.com/docs/copilot/reference/mcp-configuration)
- [MT5-Quant Tools Reference](./MCP_TOOLS.md)
