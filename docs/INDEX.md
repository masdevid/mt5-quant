# Documentation Index

**Start here:** [QUICKSTART.md](QUICKSTART.md) — install MT5-Quant, run `scripts/setup.sh` to auto-detect Wine/MT5 paths, register it with your LLM client, and complete your first backtest.

| Doc | Description |
|-----|-------------|
| [QUICKSTART.md](QUICKSTART.md) | Step-by-step install, setup, and first backtest |
| [MCP_TOOLS.md](MCP_TOOLS.md) | Full input/output schemas for all 90 tools |
| [CONFIG.md](CONFIG.md) | Configuration reference (`config/mt5-quant.yaml`) |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Design principles and internal module layout |
| [REMOTE_AGENTS.md](REMOTE_AGENTS.md) | Distribute optimization across Linux agents via Wine |
| [TROUBLESHOOTING.md](TROUBLESHOOTING.md) | Platform-specific fixes for Wine, MT5, and backtests |

## Suggested reading order

1. [QUICKSTART.md](QUICKSTART.md) — get a working installation and first report
2. [MCP_TOOLS.md](MCP_TOOLS.md) — browse what your LLM agent can call
3. [CONFIG.md](CONFIG.md) — tune symbols, timeframes, deposit defaults
4. [TROUBLESHOOTING.md](TROUBLESHOOTING.md) — when something fails
5. [ARCHITECTURE.md](ARCHITECTURE.md) — how the pipeline works under the hood
6. [REMOTE_AGENTS.md](REMOTE_AGENTS.md) — scale up optimization throughput

These files are also bundled in every release tarball and MCP package, so installed copies stay available offline.
