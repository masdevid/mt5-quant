# Windsurf MCP Integration Setup

## Quick Setup

### 1. Build Executable
```bash
bash scripts/build-executable-onedir.sh
```

### 2. Configure Windsurf

Edit `~/.windsurf/config.yaml`:

```yaml
mcpServers:
  mt5-quant:
    command: /Users/masdevid/jobs/mt5-mcp/dist/mt5-quant/mt5-quant
```

Or dengan environment variable:
```yaml
mcpServers:
  mt5-quant:
    command: /Users/masdevid/jobs/mt5-mcp/dist/mt5-quant/mt5-quant
    env:
      MT5_MCP_HOME: /Users/masdevid/jobs/mt5-mcp
```

### 3. Restart Windsurf
Close dan reopen Windsurf untuk load MCP server.

### 4. Verify
Di Windsurf chat, test dengan:
```
Run verify_setup
```

## Deployment ke Multiple Machines

### Build untuk Distribution
```bash
# Build
bash scripts/build-executable-onedir.sh

# Create tarball
tar -czf mt5-quant-macos-arm64.tar.gz -C dist mt5-quant

# Deploy ke remote server
scp mt5-quant-macos-arm64.tar.gz user@server:~/
ssh user@server "tar -xzf mt5-quant-macos-arm64.tar.gz -C /opt/"
ssh user@server "ln -s /opt/mt5-quant/mt5-quant /usr/local/bin/"

# Copy config
scp -r config/mt5-quant.yaml user@server:~/.config/mt5-quant/config/
```

### Target Machine Requirements
- MetaTrader 5 installed (via Wine/CrossOver)
- Config file di `~/.config/mt5-quant/config/mt5-quant.yaml`
- **NO Python required!**

### Windsurf Config di Target Machine
```yaml
mcpServers:
  mt5-quant:
    command: /usr/local/bin/mt5-quant
```

## Troubleshooting

### MCP server not appearing
1. Check Windsurf logs: `~/.windsurf/logs/`
2. Verify executable path is absolute
3. Test executable manually: `./dist/mt5-quant/mt5-quant --help`

### Config not found
Set `MT5_MCP_HOME` environment variable atau pastikan config di default location:
- macOS: `~/.config/mt5-quant/config/mt5-quant.yaml`

### Permission denied
```bash
chmod +x /path/to/mt5-quant
```
