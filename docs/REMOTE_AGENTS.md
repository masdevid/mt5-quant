# Remote Agent Setup (Linux Server)

MT5's distributed testing lets you farm optimization work across multiple machines. A Linux server running `metatester64.exe` via Wine connects to a Mac master over a local network.

**Throughput:** Each agent handles one pass at a time. 10 local + 16 remote = 26 agents. A 17,000-combination genetic optimization that takes 3 hours locally finishes in ~70 minutes.

---

## Requirements

**Mac (master)**
- MetaTrader 5 installed via CrossOver or Wine
- MT5-Quant configured and working for local backtests
- Port 3000 open in macOS firewall (or whichever port MT5 uses)

**Linux server (agents)**
- Wine 7.0+
- `metatester64.exe` from an MT5 installation
- Access to the same MT5 data files (tick history) as the master

## Step 1: Find the MT5 agent port on Mac

After running a local optimization, check the MT5 agent directories:

```bash
ls ~/Library/Application\ Support/MetaQuotes/*/drive_c/Program\ Files/MetaTrader\ 5/
```

You'll see directories like:
```
Agent-127.0.0.1-3000/   ← local agents (loopback)
Agent-0.0.0.0-3000/     ← remote listener (if enabled)
```

If you don't see `Agent-0.0.0.0-*` directories, enable remote agents in MT5:
**Tools → Options → Expert Advisors → Allow remote agents**

Note the port number (default: 3000).

## Step 2: Copy `metatester64.exe` to Linux

From your Mac MT5 installation:
```bash
MAC_MT5="$HOME/Library/Application Support/MetaQuotes/Terminal/XXX/ drive_c/Program Files/MetaTrader 5"

scp "${MAC_MT5}/metatester64.exe" user@linux-server:~/mt5agents/
scp "${MAC_MT5}"/*.dll user@linux-server:~/mt5agents/
```

## Step 3: Launch agents on Linux

```bash
cd ~/mt5agents/
wine64 metatester64.exe /server:192.168.1.100:3000 /agents:8
```

**To run as a background service:**
```bash
nohup wine64 metatester64.exe /server:192.168.1.100:3000 /agents:8 \
    > ~/mt5agents/agents.log 2>&1 &
disown
```

## Step 4: Verify agents appear in MT5

On your Mac, open MT5:
**View → Strategy Tester → Agents tab**

You should see entries like:
```
Agent-192.168.1.200-3000  [Active]
```

## Step 5: Configure MT5-Quant for remote agents

In `config/mt5-quant.yaml`:
```yaml
optimization:
  remote_agents:
    enabled: true
    check_agent_count: true
    min_agents: 4
```

## Tick Data Sync

On first run, MT5 automatically downloads ticks from the broker. Pre-populate on Linux:

```bash
# On Mac — find tick data location
find ~/Library/Application\ Support/MetaQuotes -name "*.bin" | grep "XAUUSD"

# Copy to Linux
scp -r "${TICK_DIR}" user@linux-server:~/mt5agents/ticks/
```

## Troubleshooting

**Agents connect then immediately disconnect**
- MT5 version mismatch between `metatester64.exe` (from Mac) and the master. Use the exact same build number.

**Agents show as connected but don't receive work**
- Start an optimization from the MT5 GUI first to "activate" remote agents, then cancel it and use MT5-Quant.

**Performance is slower with remote agents**
- Use wired connection. WiFi or WAN: significant overhead.
