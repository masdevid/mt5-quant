# Quickstart Guide

For end-users wanting full platform-specific steps, see the [TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md).

## Install

Ask your LLM platform to install MT5-Quant:

> "Please install mt5-quant from https://github.com/masdevid/mt5-quant. Download the pre-built binary, run `scripts/setup.sh` to auto-detect Wine/MT5 paths, and register the MCP server."

The LLM will handle:
1. Download binary or `cargo build --release`
2. Run setup script to auto-detect Wine and MT5 paths
3. Write `config/mt5-quant.yaml` (gitignored)
4. Register MCP server with your platform

## Minimal Config

If manual config is needed, `config/mt5-quant.yaml` requires only:

```yaml
wine_executable: "/path/to/wine64"
terminal_dir: "/path/to/MetaTrader 5"
```

`setup.sh` auto-detects both.

## Verify

```
Run verify_setup
```

## Run Backtest

```
Run a backtest on MyEA from 2025.01.01 to 2025.03.31
```

The AI runs: compile → clean → backtest → extract → analyze.

---

**Next:** See [TOOLS.md](docs/MCP_TOOLS.md) for all 89 tools.
