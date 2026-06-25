# Configuration Reference

## Config File

```
config/mt5-quant.yaml          # Project directory (development)
~/.config/mt5-quant/config/mt5-quant.yaml    # System-wide (production)
```

Override with env: `export MT5_MCP_HOME=/path/to/config`

## Full Example

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

## Auto-Detection

`setup.sh` detects:
- Wine executable (MetaTrader 5.app, CrossOver, or system Wine)
- MT5 terminal directory
- Architecture (Apple Silicon adds `arch -x86_64`)
- Display mode (GUI vs headless)

Run `setup.sh` whenever you move the installation.
