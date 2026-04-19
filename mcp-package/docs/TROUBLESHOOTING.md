# Troubleshooting

Run `verify_setup` first — it checks all paths and returns actionable hints.

## Wine Not Found

### macOS

Confirm `/Applications/MetaTrader 5.app` exists and has been launched at least once.

Check detection:
```bash
bash scripts/platform_detect.sh
```

If using CrossOver, confirm bottle is named `MetaTrader5`.

### Linux

```bash
sudo apt install wine64        # Debian/Ubuntu
sudo dnf install wine          # Fedora/RHEL
which wine64                   # confirm on PATH
```

## terminal64.exe Missing

MT5 unpacks `terminal64.exe` only after first launch.

1. Open MetaTrader 5.app
2. Wait for initialization (~30s)
3. Quit
4. Re-run setup:
```bash
bash scripts/setup.sh --yes
```

## MCP Server Not Appearing

### Claude Code

```bash
claude mcp list                # should show MT5-Quant
claude mcp remove MT5-Quant    # remove stale entry
claude mcp add MT5-Quant -- /absolute/path/to/mt5-quant
```

**Must use absolute path** — relative paths break when Claude starts from different directories.

### Windsurf

1. Check logs: `~/.windsurf/logs/`
2. Verify executable path is absolute
3. Test manually: `./mt5-quant --help`

## Config Not Found

Set `MT5_MCP_HOME` or ensure config exists at:
- macOS: `~/.config/mt5-quant/config/mt5-quant.yaml`
- Project: `config/mt5-quant.yaml`

## Report Not Found After Backtest

1. **Wrong symbol name** — brokers use custom names (`XAUUSDm`, `XAUUSD.cent`). Check `verify_setup` or look in `<terminal_dir>/history/`.

2. **No history data** — open MT5, open symbol chart, wait for history download.

3. **EA crash at startup** — check `<terminal_dir>/MQL5/Logs/` for `OnInit` errors.

4. **Date range has no trades** — try wider range or confirm symbol was active.

## MetaEditor Compile Errors

Check `<terminal_dir>/MQL5/Logs/`:

- **Missing `#include`** — copy dependencies into `Experts/` alongside `.mq5`
- **Stale `.ex5`** — delete old binary and recompile

## No Deals in Backtest Report

- Use `model=0` (every tick) — models 1/2 skip intra-bar movement, producing zero deals for grid/martingale EAs
- Check `.set` file values appropriate for symbol/broker
- Confirm `OnInit()` returns `INIT_SUCCEEDED` (MT5 Journal tab)

## Optimization Never Finishes

```bash
# From Claude:
tail_log(job_id=X, filter=errors)
get_optimization_status(job_id=X)
```

If MT5 crashed, edit `<terminal_dir>/terminal.ini` and remove line containing `OptMode=-1`, then retry.

## Permission Denied

```bash
chmod +x /path/to/mt5-quant
```

## Still Stuck?

1. Run `verify_setup` and share output
2. Check `tail_log` for errors
3. Review `<terminal_dir>/MQL5/Logs/` for EA errors
