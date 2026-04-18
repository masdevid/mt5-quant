# Quickstart Guide

## 1. Download or Build

### Option A: Prebuilt Binary (Recommended)

```bash
# macOS (Apple Silicon)
curl -L -o mt5-quant.tar.gz https://github.com/masdevid/mt5-mcp/releases/latest/download/mt5-quant-macos-arm64.tar.gz
tar -xzf mt5-quant.tar.gz

# Linux (x64)
curl -L -o mt5-quant.tar.gz https://github.com/masdevid/mt5-mcp/releases/latest/download/mt5-quant-linux-x64.tar.gz
tar -xzf mt5-quant.tar.gz
```

### Option B: Build from Source

```bash
git clone https://github.com/masdevid/mt5-mcp
cd mt5-mcp
bash scripts/build-rust.sh
```

## 2. Install MetaTrader 5

### macOS - MetaTrader 5.app (Free)

1. Download from [metatrader5.com](https://www.metatrader5.com/en/download)
2. Install to `/Applications`
3. **Launch once** to initialize Wine prefix (~30s), then quit

Auto-detected paths:
- Wine: `/Applications/MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64`
- MT5: `~/Library/Application Support/net.metaquotes.wine.metatrader5/drive_c/Program Files/MetaTrader 5`

### macOS - CrossOver (Paid, Better Compatibility)

1. Install [CrossOver](https://www.codeweavers.com/)
2. Create bottle `MetaTrader5`
3. Install MT5 inside bottle

Auto-detected paths:
- Wine: `/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine64`
- MT5: `~/Library/Application Support/MetaQuotes/<hash>/drive_c/Program Files/MetaTrader 5`

### Linux

```bash
# Debian/Ubuntu
sudo apt install wine64 xvfb

# Fedora/RHEL
sudo dnf install wine xorg-x11-server-Xvfb

# Install MT5
wine64 MetaTrader5Setup.exe
```

MT5 location: `~/.wine/drive_c/Program Files/MetaTrader 5`

## 3. Configure

Run the setup script to auto-detect paths:

```bash
bash scripts/setup.sh          # interactive
bash scripts/setup.sh --yes    # non-interactive (CI)
```

This creates `config/mt5-quant.yaml` (gitignored).

Minimum config:
```yaml
wine_executable: "/Applications/MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64"
terminal_dir: "~/Library/Application Support/net.metaquotes.wine.metatrader5/drive_c/Program Files/MetaTrader 5"
```

## 4. Register MCP Server

### Claude Code

```bash
claude mcp add MT5-Quant -- /path/to/mt5-quant/target/release/mt5-quant
```

Verify:
```bash
claude mcp list
```

### Windsurf

Add to `~/.windsurf/config.yaml`:
```yaml
mcpServers:
  mt5-quant:
    command: /path/to/mt5-quant
    env:
      MT5_MCP_HOME: /path/to/mt5-mcp
```

## 5. Verify Setup

```bash
bash scripts/platform_detect.sh
```

Or in Claude/Windsurf:
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

## 6. Run First Backtest

```
Run a backtest on MyEA from 2025.01.01 to 2025.03.31
```

The AI will:
1. Verify setup
2. Compile your EA
3. Clean MT5 cache
4. Run backtest
5. Extract and analyze results
6. Report key findings

---

**Next:** See [TOOLS.md](TOOLS.md) for all 43 available tools.
