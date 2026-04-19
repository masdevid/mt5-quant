# Remote Agent Setup (Linux Server)

MT5's distributed testing lets you farm optimization work across multiple machines. A Linux server running `metatester64.exe` via Wine connects to a Mac master over a local network.

**Throughput:** Each agent handles one pass at a time. 10 local + 16 remote = 26 agents. A 17,000-combination genetic optimization that takes 3 hours locally finishes in ~70 minutes.

---

## Requirements

**Mac (master)**
- MetaTrader 5 installed via CrossOver or Wine
- MT5-Quant configured and working for local backtests
- Port 3000 open in macOS firewall (or whichever port MT5 uses — check below)

**Linux server (agents)**
- Wine 7.0+ (or Wine Staging for better compatibility)
- `metatester64.exe` from an MT5 installation
- Access to the same MT5 data files (tick history) as the master — or let agents download on first run

---

## Step 1: Find the MT5 agent port on Mac

After running a local optimization, check the MT5 agent directories:

```bash
ls ~/Library/Application\ Support/MetaQuotes/*/drive_c/Program\ Files/MetaTrader\ 5/
```

You'll see directories like:
```
Agent-127.0.0.1-3000/   ← local agents (loopback)
Agent-127.0.0.1-3001/
Agent-0.0.0.0-3000/     ← remote listener (if enabled)
```

If you don't see `Agent-0.0.0.0-*` directories, enable remote agents in MT5:
**Tools → Options → Expert Advisors → Allow remote agents**

Note the port number (default: 3000).

---

## Step 2: Open firewall on Mac

```bash
# Check current firewall rules
sudo pfctl -s rules

# Allow incoming on agent port (example: 3000)
# Add to /etc/pf.conf or use macOS Firewall in System Settings
```

Or use macOS System Settings → Privacy & Security → Firewall → Firewall Options → Add MetaTrader 5.

---

## Step 3: Install Wine on Linux server

```bash
# Ubuntu/Debian
sudo dpkg --add-architecture i386
sudo apt update
sudo apt install wine64 wine32

# Verify
wine64 --version
```

---

## Step 4: Copy `metatester64.exe` to Linux

From your Mac MT5 installation:
```bash
MAC_MT5="$HOME/Library/Application Support/MetaQuotes/Terminal/XXXXX/drive_c/Program Files/MetaTrader 5"

scp "${MAC_MT5}/metatester64.exe" user@linux-server:~/mt5agents/
scp "${MAC_MT5}/metaeditor64.exe" user@linux-server:~/mt5agents/  # not required for agents
```

Also copy the required DLLs (they're in the same directory):
```bash
scp "${MAC_MT5}"/*.dll user@linux-server:~/mt5agents/
```

---

## Step 5: Launch agents on Linux

```bash
# On the Linux server
cd ~/mt5agents/

# Replace MAC_IP with your Mac's local IP address
# Replace 8 with number of CPU cores - 1
wine64 metatester64.exe /server:192.168.1.100:3000 /agents:8
```

**What this does:**
- Connects to your Mac's MT5 master at `192.168.1.100:3000`
- Registers 8 worker agents
- MT5 on Mac will show 8 new agents in the agent manager

**To run as a background service:**
```bash
nohup wine64 metatester64.exe /server:192.168.1.100:3000 /agents:8 \
    > ~/mt5agents/agents.log 2>&1 &
disown
```

---

## Step 6: Verify agents appear in MT5

On your Mac, open MT5:
**View → Strategy Tester → Agents tab**

You should see entries like:
```
Agent-192.168.1.200-3000  [Active]
Agent-192.168.1.200-3001  [Active]
...
```

If agents appear as `[Inactive]`, check:
1. Mac firewall is allowing incoming connections on the agent port
2. Linux server can reach Mac IP: `ping 192.168.1.100`
3. Port is open: `nc -zv 192.168.1.100 3000`

---

## Step 7: Configure MT5-Quant for remote agents

In `config/MT5-Quant.yaml`:
```yaml
optimization:
  remote_agents:
    enabled: true
    check_agent_count: true   # Verify remote agents are connected before launching
    min_agents: 4             # Require at least N agents before optimizing
```

MT5-Quant will log the agent count before launching optimization:
```
[optimize] Local agents: 9, Remote agents: 8, Total: 17
[optimize] Estimated completion: ~105 minutes (17,640 passes at 17 agents)
```

---

## Tick Data Sync

Remote agents need tick history to replay trades. On first run, MT5 automatically downloads ticks from the broker for each symbol+period combination tested. This can take 10-30 minutes per symbol.

**Speed up first run:** Pre-populate the tick cache on the Linux server by copying from Mac:

```bash
# On Mac — find tick data location
find ~/Library/Application\ Support/MetaQuotes -name "*.bin" | grep "XAUUSD"

# Typical path:
# Terminal/XXXXX/drive_c/users/user/AppData/Roaming/MetaQuotes/Terminal/Common/Files/

scp -r "${TICK_DIR}" user@linux-server:~/mt5agents/ticks/
```

---

## Troubleshooting

**Agents connect then immediately disconnect**
- MT5 version mismatch between `metatester64.exe` (from Mac) and the master. Use the exact same build number.
- Check `~/mt5agents/agents.log` for Wine errors.

**Agents show as connected but don't receive work**
- MT5 sometimes requires optimization to be started before assigning work to new agents.
- Start an optimization from the MT5 GUI first to "activate" remote agents, then cancel it and use MT5-Quant.

**Performance is slower with remote agents**
- Network latency between Mac and Linux server. Results are sent over TCP after each pass.
- Local gigabit network: negligible. WiFi or WAN: significant overhead. Use wired connection.

**Linux server runs out of memory**
- Each `metatester64.exe` instance uses ~200-400MB.
- 8 agents = ~2-3GB RAM. Size agent count accordingly.
