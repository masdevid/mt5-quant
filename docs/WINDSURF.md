# Windsurf MCP Integration Setup

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
bash scripts/build-rust.sh
```

### 2. Configure Windsurf

Edit `~/.codeium/windsurf/mcp_config.json`:

```json
{
  "mcpServers": {
    "io.github.masdevid/mt5-quant": {
      "command": "~/.local/bin/mt5-quant",
      "disabled": false,
      "registry": "io.github.masdevid/mt5-quant"
    }
  }
}
```

Or use the automated setup:

```bash
# Install binary to standard location
cp target/release/mt5-quant ~/.local/bin/

# Then configure
bash scripts/setup.sh
```

### 3. Restart Windsurf
Close and reopen Windsurf to load the MCP server.

### 4. Verify
In Windsurf chat, test with:
```
Run verify_setup
```

## Deployment to Multiple Machines

### Build for Distribution
```bash
# Build release binary
cargo build --release

# Create tarball
tar -czf mt5-quant-macos-arm64.tar.gz -C target/release mt5-quant

# Deploy to remote server
scp mt5-quant-macos-arm64.tar.gz user@server:~/
ssh user@server "tar -xzf mt5-quant-macos-arm64.tar.gz -C /opt/"
ssh user@server "ln -s /opt/mt5-quant /usr/local/bin/"

# Copy config
scp -r config/mt5-quant.yaml user@server:~/.config/mt5-quant/config/
```

### Target Machine Requirements
- MetaTrader 5 installed (via Wine/CrossOver)
- Config file at `~/.config/mt5-quant/config/mt5-quant.yaml`
- **NO Python required!**

### Windsurf Config on Target Machine

```json
{
  "mcpServers": {
    "mt5-quant": {
      "command": "/usr/local/bin/mt5-quant"
    }
  }
}
```

The binary auto-detects its config location. No environment variables needed.

## Troubleshooting

### MCP server not appearing
1. Check Windsurf logs: `~/.windsurf/logs/`
2. Verify executable path is absolute
3. Test executable manually: `./target/release/mt5-quant --help`

### Config not found
Set `MT5_MCP_HOME` environment variable or ensure config is at default location:
- macOS: `~/.config/mt5-quant/config/mt5-quant.yaml`

### Permission denied
```bash
chmod +x /path/to/mt5-quant
```
