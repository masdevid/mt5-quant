# Claude Desktop MCP Integration Setup

## Quick Setup

### Option 1: Install via MCP Registry (Recommended)

Search for `mt5-quant` in Claude Desktop's MCP manager, or add directly to your config.

### Option 2: Download Prebuilt Binary

```bash
# macOS (Apple Silicon)
curl -L -o mt5-quant.tar.gz https://github.com/masdevid/mt5-quant/releases/latest/download/mt5-quant-macos-arm64.tar.gz
tar -xzf mt5-quant.tar.gz

# Linux (x64)
curl -L -o mt5-quant.tar.gz https://github.com/masdevid/mt5-quant/releases/latest/download/mt5-quant-linux-x64.tar.gz
tar -xzf mt5-quant.tar.gz
```

### Option 3: Build from Source

```bash
git clone https://github.com/masdevid/mt5-quant
cd mt5-quant
cargo build --release
```

## Configure Claude Desktop

### Step 1: Open MCP Configuration

Edit `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows).

### Step 2: Add mt5-quant

```json
{
  "mcpServers": {
    "mt5-quant": {
      "command": "/absolute/path/to/mt5-quant"
    }
  }
}
```

### Step 3: Reload and Verify

1. Save the file
2. **Restart Claude Desktop**
3. In a new conversation, type: `What tools do you have access to?`
4. The agent should list MT5 tools like `verify_setup`, `run_backtest`, etc.

## Full Example Configuration

```json
{
  "mcpServers": {
    "mt5-quant": {
      "command": "/Users/name/mt5-quant/mt5-quant"
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

**Path notes:**
- Prebuilt binary: `/Users/name/mt5-quant/mt5-quant` (extracted from release tarball)
- Dev build: `/Users/name/mt5-quant/target/release/mt5-quant` (after `cargo build --release`)

## Environment Variables

Claude Desktop supports `${env:VAR_NAME}` syntax for environment variable substitution:

```json
{
  "mcpServers": {
    "mt5-quant": {
      "command": "/Users/name/mt5-quant/mt5-quant",
      "env": {
        "MT5_MCP_HOME": "${env:HOME}/.config/mt5-quant"
      }
    }
  }
}
```

## Verify Setup

In a Claude Desktop conversation, type:

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

### "Tool not found" or server not appearing

1. Double-check the absolute path in `claude_desktop_config.json`
2. Ensure the binary has execute permissions: `chmod +x /path/to/mt5-quant`
3. Restart Claude Desktop completely
4. Check Claude Desktop logs: `~/Library/Logs/Claude/` (macOS)

### JSON syntax errors

1. Validate the JSON at [jsonlint.com](https://jsonlint.com)
2. Ensure no trailing commas
3. Use absolute paths (not `~` or relative paths)

### Agent hallucinating tool parameters

1. Verify the tool is available: "List your available MCP tools"
2. Start a new conversation
3. Be explicit: "Use the `run_backtest` tool with expert=MyEA"

## Configuration Location

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Windows | `%APPDATA%\Claude\claude_desktop_config.json` |

## Resources

- [Claude Desktop MCP Documentation](https://docs.anthropic.com/en/docs/claude-code/mcp)
- [MCP Server Reference](https://github.com/modelcontextprotocol/servers)
- [MT5-Quant Tools Reference](./MCP_TOOLS.md)
