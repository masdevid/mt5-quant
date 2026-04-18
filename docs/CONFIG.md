# Configuration Reference

## Config File Location

```
config/mt5-quant.yaml          # Project directory (development)
~/.config/mt5-quant/config/mt5-quant.yaml    # System-wide (production)
```

Environment variable to override:
```bash
export MT5_MCP_HOME=/path/to/config
```

## Full Config Example

```yaml
# Required: Wine executable path
wine_executable: "/Applications/MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64"

# Required: MT5 installation directory
terminal_dir: "~/Library/Application Support/net.metaquotes.wine.metatrader5/drive_c/Program Files/MetaTrader 5"

# Optional: Default backtest parameters
defaults:
  symbol: "XAUUSD.cent"
  timeframe: "M5"
  deposit: 10000
  currency: "USD"
  model: 0                    # 0=every tick, 1=1min OHLC, 2=open price
  leverage: 500

# Optional: Display settings
display:
  mode: auto                  # auto, gui, headless
  xvfb_display: ":99"         # Linux headless only
  xvfb_screen: "1024x768x16"  # Linux headless only

# Optional: Directories (auto-detected from terminal_dir if not set)
experts_dir: "~/.../MetaTrader 5/MQL5/Experts"
indicators_dir: "~/.../MetaTrader 5/MQL5/Indicators"
scripts_dir: "~/.../MetaTrader 5/MQL5/Scripts"

# Optional: Reports directory
reports_dir: "./reports"

# Optional: Optimization settings
optimization:
  remote_agents:
    enabled: false
    check_agent_count: true
    min_agents: 4
```

## Platform-Specific Examples

### macOS with MetaTrader 5.app

```yaml
wine_executable: "/Applications/MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64"
terminal_dir: "~/Library/Application Support/net.metaquotes.wine.metatrader5/drive_c/Program Files/MetaTrader 5"
defaults:
  symbol: "XAUUSDc"
  timeframe: "M5"
  deposit: 10000
```

### macOS with CrossOver

```yaml
wine_executable: "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine64"
terminal_dir: "~/Library/Application Support/MetaQuotes/Terminal/<hash>/drive_c/Program Files/MetaTrader 5"
```

### Linux

```yaml
wine_executable: "/usr/bin/wine64"
terminal_dir: "~/.wine/drive_c/Program Files/MetaTrader 5"
display:
  mode: headless
  xvfb_display: ":99"
```

## Headless Mode (Linux VPS)

```yaml
display:
  mode: headless
  xvfb_display: ":99"
  xvfb_screen: "1024x768x16"
```

Requires:
```bash
sudo apt install xvfb
```

## Auto-Detection

`setup.sh` automatically detects:
- Wine executable (MetaTrader 5.app, CrossOver, or system Wine)
- MT5 terminal directory
- Architecture (Apple Silicon adds `arch -x86_64`)
- Display mode (GUI vs headless)

Run `setup.sh` whenever you move the installation.
