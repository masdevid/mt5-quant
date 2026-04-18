#!/usr/bin/env bash
# optimize.sh — Launch MT5 genetic optimization (always background + detached)
#
# Usage:
#   ./scripts/optimize.sh [options]
#
# Options:
#   --expert NAME        EA name
#   --set FILE           Optimization .set file (with ||Y flags)
#   --symbol SYMBOL      Trading symbol
#   --from YYYY.MM.DD    Start date
#   --to   YYYY.MM.DD    End date
#   --deposit AMOUNT     Initial deposit
#   --model 0|1|2        Tick model (ALWAYS use 0 for grid/martingale EAs)
#   --log FILE           Log file path (default: /tmp/mt5opt_TIMESTAMP.log)
#
# IMPORTANT: This script launches MT5 as a detached background process.
# It returns immediately. Do NOT set a timeout on this script.
# Monitor /tmp/mt5opt_*.log and wait for user signal before parsing results.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/platform_detect.sh"

# ── Defaults ──────────────────────────────────────────────────────────────────
DEFAULT_SYMBOL=$(_cfg "backtest_symbol" "XAUUSD")
DEFAULT_DEPOSIT=$(_cfg "backtest_deposit" "10000")
DEFAULT_CURRENCY=$(_cfg "backtest_currency" "USD")
DEFAULT_LEVERAGE=$(_cfg "backtest_leverage" "500")

EXPERT=""
SET_FILE=""
SYMBOL="$DEFAULT_SYMBOL"
FROM_DATE=""
TO_DATE=""
DEPOSIT="$DEFAULT_DEPOSIT"
CURRENCY="$DEFAULT_CURRENCY"
LEVERAGE="$DEFAULT_LEVERAGE"
MODEL=0  # ALWAYS 0 for optimization — see below
LOG_FILE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --expert)  EXPERT="$2";    shift 2 ;;
        --set)     SET_FILE="$2";  shift 2 ;;
        --symbol)  SYMBOL="$2";    shift 2 ;;
        --from)    FROM_DATE="$2"; shift 2 ;;
        --to)      TO_DATE="$2";   shift 2 ;;
        --deposit) DEPOSIT="$2";   shift 2 ;;
        --model)
            # Warn if user tries to use model != 0
            if [[ "$2" != "0" ]]; then
                echo "WARNING: --model $2 ignored. Optimization always uses model=0." >&2
                echo "  Model 1/2 overfits martingale/grid EAs (intra-bar price not simulated)." >&2
            fi
            shift 2
            ;;
        --log)     LOG_FILE="$2";  shift 2 ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

[[ -z "$EXPERT" ]]    && { echo "ERROR: --expert is required" >&2; exit 1; }
[[ -z "$SET_FILE" ]]  && { echo "ERROR: --set is required" >&2; exit 1; }
[[ -z "$FROM_DATE" ]] && { echo "ERROR: --from is required" >&2; exit 1; }
[[ -z "$TO_DATE" ]]   && { echo "ERROR: --to is required" >&2; exit 1; }

[[ ! -f "$SET_FILE" ]] && { echo "ERROR: Set file not found: $SET_FILE" >&2; exit 1; }

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG_FILE="${LOG_FILE:-/tmp/mt5opt_${TIMESTAMP}.log}"
JOB_ID="opt_${TIMESTAMP}"

# ── Resolve platform ──────────────────────────────────────────────────────────
resolve_platform

# ── Write .set file as UTF-16LE with BOM (read-only) ─────────────────────────
# MT5 REQUIREMENT: optimization .set files must be UTF-16LE with BOM.
# If provided as UTF-8, MT5 strips the ||Y optimization flags silently —
# every pass runs with the fixed base value and optimization is useless.
python3 - << PYEOF
import sys, os, shutil

src = "${SET_FILE}"
dst = "${MT5_TESTER_DIR}/${EXPERT}.set"
os.makedirs("${MT5_TESTER_DIR}", exist_ok=True)

with open(src, 'r', encoding='utf-8', errors='replace') as f:
    content = f.read()

# Write UTF-16LE with BOM
with open(dst, 'w', encoding='utf-16-le') as f:
    f.write('\ufeff')  # BOM
    f.write(content)

# Make read-only — prevents MT5 from overwriting ||Y flags during optimization
os.chmod(dst, 0o444)
print(f"  .set → {dst} (UTF-16LE, read-only)")
PYEOF

# ── Reset OptMode in terminal.ini ─────────────────────────────────────────────
# After any optimization run (complete or aborted), MT5 writes OptMode=-1.
# On next launch, MT5 reads OptMode=-1 and exits immediately without running.
# Must reset to 0 before every optimization launch.
TERMINAL_INI="${MT5_DIR}/terminal.ini"
if [[ -f "$TERMINAL_INI" ]]; then
    # Use Python for safe in-place edit (sed -i behaves differently on macOS vs Linux)
    python3 - << PYEOF
import re

ini_path = "${TERMINAL_INI}"
with open(ini_path, 'r', errors='replace') as f:
    content = f.read()

# Reset OptMode
content = re.sub(r'OptMode=.*', 'OptMode=0', content)
# Remove LastOptimization (causes MT5 to skip running)
content = re.sub(r'LastOptimization=.*\n?', '', content)

with open(ini_path, 'w') as f:
    f.write(content)
print(f"  terminal.ini: OptMode reset to 0")
PYEOF
fi

# ── Build optimization INI ────────────────────────────────────────────────────
WINE_PREFIX_DIR=$(dirname "$(dirname "$MT5_DIR")")

cat > "${WINE_PREFIX_DIR}/drive_c/mt5mcp_backtest.ini" << INI
[Tester]
Expert=${EXPERT}
Symbol=${SYMBOL}
Period=M5
Deposit=${DEPOSIT}
Currency=${CURRENCY}
Leverage=${LEVERAGE}
Model=${MODEL}
FromDate=${FROM_DATE}
ToDate=${TO_DATE}
Report=C:\\mt5mcp_opt_report
Optimization=2
ExpertParameters=${EXPERT}.set
ShutdownTerminal=1
INI

cat > "${WINE_PREFIX_DIR}/drive_c/mt5mcp_run.bat" << 'EOF'
@echo off
"C:\Program Files\MetaTrader 5\terminal64.exe" /config:C:\mt5mcp_backtest.ini
EOF

# ── Count optimization combinations ──────────────────────────────────────────
COMBINATIONS=$(python3 - << PYEOF
import re, math

with open("${SET_FILE}", 'r', errors='replace') as f:
    lines = f.readlines()

total = 1
for line in lines:
    line = line.strip()
    if line.startswith(';') or '=' not in line:
        continue
    # Format: param=value||start||step||stop||Y
    parts = line.split('||')
    if len(parts) >= 5 and parts[-1].strip().upper() == 'Y':
        try:
            start = float(parts[1])
            step  = float(parts[2])
            stop  = float(parts[3])
            count = max(1, int((stop - start) / step) + 1)
            total *= count
        except (ValueError, ZeroDivisionError):
            pass

print(total)
PYEOF
)

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo " MT5-Quant Genetic Optimization"
echo " Job ID:    $JOB_ID"
echo " Expert:    $EXPERT"
echo " Symbol:    $SYMBOL  Model: ${MODEL} (every tick)"
echo " Period:    $FROM_DATE → $TO_DATE"
echo " Set file:  $SET_FILE"
echo " Combos:    $COMBINATIONS (genetic — converges in ~300-500 passes)"
echo " Log:       $LOG_FILE"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# ── Launch detached ───────────────────────────────────────────────────────────
# nohup: prevents SIGHUP when parent (Claude task, SSH session) exits
# disown: removes from shell job table so shell exit doesn't kill it
# Both are required for true detachment.

nohup bash -c "${MT5_ARCH} '${MT5_WINE}' cmd.exe /c 'C:\\mt5mcp_run.bat' 2>/dev/null || true" \
    > "$LOG_FILE" 2>&1 &
OPT_PID=$!
disown $OPT_PID

# Write job metadata
JOBS_DIR="${ROOT_DIR}/.mt5mcp_jobs"
mkdir -p "$JOBS_DIR"
cat > "${JOBS_DIR}/${JOB_ID}.json" << JEOF
{
  "job_id": "${JOB_ID}",
  "pid": ${OPT_PID},
  "expert": "${EXPERT}",
  "symbol": "${SYMBOL}",
  "from_date": "${FROM_DATE}",
  "to_date": "${TO_DATE}",
  "set_file": "${SET_FILE}",
  "combinations": ${COMBINATIONS},
  "log_file": "${LOG_FILE}",
  "wine_prefix": "${WINE_PREFIX_DIR}",
  "started_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
JEOF

echo ""
echo " Launched (pid: $OPT_PID)"
echo " Optimization runs for 2-6 hours. Do NOT kill this process."
echo " Signal when MT5 shows 'Optimization complete' and use:"
echo "   python3 analytics/optimize_parser.py --job $JOB_ID"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
