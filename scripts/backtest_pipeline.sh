#!/usr/bin/env bash
# backtest_pipeline.sh — 5-stage MT5 backtest pipeline
# Stages: COMPILE → CLEAN → BACKTEST → EXTRACT → ANALYZE
#
# Usage:
#   ./scripts/backtest_pipeline.sh [options]
#
# Options:
#   --expert NAME        EA name (without path or .mq5 extension)
#   --symbol SYMBOL      Trading symbol (default: from config)
#   --from YYYY.MM.DD    Start date
#   --to   YYYY.MM.DD    End date
#   --preset PRESET      last_month | last_3months | ytd | last_year
#   --timeframe TF       M1 M5 M15 M30 H1 H4 D1 (default: M5)
#   --deposit AMOUNT     Initial deposit (default: from config)
#   --model 0|1|2        Tick model (default: 0=every tick)
#   --set FILE           Path to .set parameter file
#   --leverage N         Leverage (default: 500)
#   --skip-compile       Skip compilation stage
#   --skip-clean         Skip cache clean stage
#   --skip-analyze       Skip analysis stage (extract only)
#   --deep               Run deep analysis (hourly + volume profile)
#   --strategy NAME      Analysis strategy profile: grid (default) | scalper | trend | hedge | generic
#   --timeout N          Backtest timeout in seconds (default: 900)
#   --shutdown           Close MT5 after backtest (default: keep open). Use for CI/headless.
#   --kill-existing      Kill a running MT5 instance before launching (default: pass config to it)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Resolve real physical path (follows symlinks) so analytics/ is found even when
# scripts/ is a symlink (e.g. ~/.config/mt5-quant/scripts -> /path/to/mt5-quant/scripts)
REAL_SCRIPT_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${REAL_SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/platform_detect.sh"

# ── Defaults from config ──────────────────────────────────────────────────────
DEFAULT_SYMBOL=$(_cfg "backtest_symbol" "XAUUSD")
DEFAULT_DEPOSIT=$(_cfg "backtest_deposit" "10000")
DEFAULT_CURRENCY=$(_cfg "backtest_currency" "USD")
DEFAULT_LEVERAGE=$(_cfg "backtest_leverage" "500")
DEFAULT_MODEL=$(_cfg "backtest_model" "0")
DEFAULT_TF=$(_cfg "backtest_timeframe" "M5")
DEFAULT_TIMEOUT=$(_cfg "backtest_timeout" "900")
REPORTS_DIR="$(_cfg "reports_dir" "${ROOT_DIR}/reports")"
# Optional: force headless terminal to a specific broker account (needed when live
# trading terminal uses a different broker than the backtest symbol requires).
DEFAULT_LOGIN=$(_cfg "backtest_login" "")
DEFAULT_SERVER=$(_cfg "backtest_server" "")

# ── Parse arguments ───────────────────────────────────────────────────────────
EXPERT=""
SYMBOL="$DEFAULT_SYMBOL"
FROM_DATE=""
TO_DATE=""
PRESET=""
TIMEFRAME="$DEFAULT_TF"
DEPOSIT="$DEFAULT_DEPOSIT"
CURRENCY="$DEFAULT_CURRENCY"
MODEL="$DEFAULT_MODEL"
SET_FILE=""
LEVERAGE="$DEFAULT_LEVERAGE"
SKIP_COMPILE=false
SKIP_CLEAN=false
SKIP_ANALYZE=false
DEEP_ANALYZE=false
STRATEGY="grid"
TIMEOUT="$DEFAULT_TIMEOUT"
PROJECT_DIR="$(_cfg "project_dir" "")"
GUI_MODE=false
SHUTDOWN_TERMINAL=false
KILL_EXISTING=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --expert)       EXPERT="$2";       shift 2 ;;
        --project-dir)  PROJECT_DIR="$2";  shift 2 ;;
        --gui)          GUI_MODE=true;     shift ;;
        --symbol)    SYMBOL="$2";     shift 2 ;;
        --from)      FROM_DATE="$2";  shift 2 ;;
        --to)        TO_DATE="$2";    shift 2 ;;
        --preset)    PRESET="$2";     shift 2 ;;
        --timeframe) TIMEFRAME="$2";  shift 2 ;;
        --deposit)   DEPOSIT="$2";    shift 2 ;;
        --model)     MODEL="$2";      shift 2 ;;
        --set)       SET_FILE="$2";   shift 2 ;;
        --leverage)  LEVERAGE="$2";   shift 2 ;;
        --timeout)   TIMEOUT="$2";    shift 2 ;;
        --skip-compile) SKIP_COMPILE=true;      shift ;;
        --skip-clean)   SKIP_CLEAN=true;        shift ;;
        --skip-analyze) SKIP_ANALYZE=true;      shift ;;
        --deep)         DEEP_ANALYZE=true;      shift ;;
        --strategy)     STRATEGY="$2";          shift 2 ;;
        --shutdown)     SHUTDOWN_TERMINAL=true; shift ;;
        --kill-existing) KILL_EXISTING=true;    shift ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

[[ -z "$EXPERT" ]] && { echo "ERROR: --expert is required" >&2; exit 1; }

# ── Preset date resolution ────────────────────────────────────────────────────
if [[ -n "$PRESET" ]]; then
    TODAY=$(date +%Y.%m.%d)
    case "$PRESET" in
        last_month)
            FROM_DATE=$(date -d "1 month ago" +%Y.%m.01 2>/dev/null || \
                        date -v-1m +%Y.%m.01)
            TO_DATE="$TODAY"
            ;;
        last_3months)
            FROM_DATE=$(date -d "3 months ago" +%Y.%m.01 2>/dev/null || \
                        date -v-3m +%Y.%m.01)
            TO_DATE="$TODAY"
            ;;
        ytd)
            FROM_DATE=$(date +%Y.01.01)
            TO_DATE="$TODAY"
            ;;
        last_year)
            PREV_YEAR=$(( $(date +%Y) - 1 ))
            FROM_DATE="${PREV_YEAR}.01.01"
            TO_DATE="${PREV_YEAR}.12.31"
            ;;
        *) echo "ERROR: Unknown preset: $PRESET" >&2; exit 1 ;;
    esac
fi

[[ -z "$FROM_DATE" || -z "$TO_DATE" ]] && {
    echo "ERROR: Provide --from/--to dates or --preset" >&2; exit 1
}

# ── Report directory ──────────────────────────────────────────────────────────
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_ID="${TIMESTAMP}_${EXPERT}_${SYMBOL}_${TIMEFRAME}"
REPORT_DIR="${REPORTS_DIR}/${REPORT_ID}"
mkdir -p "$REPORT_DIR"

PIPELINE_START=$(date +%s)
PROGRESS_LOG="${REPORT_DIR}/progress.log"
_progress() { echo "$1 $(date -u +%Y-%m-%dT%H:%M:%SZ) elapsed=$(( $(date +%s) - PIPELINE_START ))" >> "$PROGRESS_LOG"; }

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo " MT5-Quant Backtest Pipeline"
echo " Expert:    $EXPERT"
echo " Symbol:    $SYMBOL  Timeframe: $TIMEFRAME  Model: $MODEL"
echo " Period:    $FROM_DATE → $TO_DATE"
echo " Deposit:   $CURRENCY $DEPOSIT  Leverage: 1:$LEVERAGE"
[[ -n "$SET_FILE" ]] && echo " Set file:  $SET_FILE"
echo " Report:    $REPORT_DIR"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# ── Resolve platform ──────────────────────────────────────────────────────────
resolve_platform

# ── Stage 1: COMPILE ──────────────────────────────────────────────────────────
if [[ "$SKIP_COMPILE" == false ]]; then
    _progress "COMPILE"
    echo ""
    echo "[1/5] COMPILE"

    # Find source file — check project.dir first, then fall back to pipeline root
    EA_SOURCE=""
    search_paths=(
        "${ROOT_DIR}/src/experts/${EXPERT}.mq5"
        "${ROOT_DIR}/src/${EXPERT}.mq5"
        "${ROOT_DIR}/${EXPERT}.mq5"
    )
    if [[ -n "$PROJECT_DIR" ]]; then
        search_paths=(
            "${PROJECT_DIR}/src/experts/${EXPERT}.mq5"
            "${PROJECT_DIR}/src/${EXPERT}.mq5"
            "${PROJECT_DIR}/${EXPERT}.mq5"
            "${search_paths[@]}"
        )
    fi
    for search_path in "${search_paths[@]}"; do
        [[ -f "$search_path" ]] && { EA_SOURCE="$search_path"; break; }
    done

    [[ -z "$EA_SOURCE" ]] && {
        echo "  ERROR: Cannot find ${EXPERT}.mq5" >&2
        exit 1
    }

    "${SCRIPT_DIR}/mqlcompile.sh" "$EA_SOURCE"
else
    echo "[1/5] COMPILE  skipped"
fi

# ── Stage 2: CLEAN ────────────────────────────────────────────────────────────
if [[ "$SKIP_CLEAN" == false ]]; then
    _progress "CLEAN"
    echo ""
    echo "[2/5] CLEAN"

    # Clear tester cache
    if [[ -d "$MT5_CACHE_DIR" ]]; then
        find "$MT5_CACHE_DIR" -name "*.tst" -delete 2>/dev/null || true
        echo "  Cleared tester cache: $MT5_CACHE_DIR"
    fi

    # Remove cached .set file for this expert
    CACHED_SET="${MT5_TESTER_DIR}/${EXPERT}.set"
    if [[ -f "$CACHED_SET" ]]; then
        rm -f "$CACHED_SET"
        echo "  Removed cached .set: $CACHED_SET"
    fi

    # Reset terminal.ini OptMode — after any test/optimization MT5 sets OptMode=-1
    # which causes the next headless run to exit immediately (exit 49, no report)
    TERMINAL_INI="${MT5_DIR}/config/terminal.ini"
    if [[ -f "$TERMINAL_INI" ]]; then
        python3 -c "
import sys, re
path = sys.argv[1]
try:
    with open(path, 'rb') as f:
        raw = f.read()
    # Detect encoding: UTF-16 with BOM, or plain text
    if raw[:2] in (b'\xff\xfe', b'\xfe\xff'):
        text = raw.decode('utf-16')
        encoding = 'utf-16'
    else:
        text = raw.decode('utf-8', errors='replace')
        encoding = 'utf-8'
    text = re.sub(r'(?m)^OptMode=-1\s*$', 'OptMode=0', text)
    text = re.sub(r'(?m)^LastOptimization=1\s*\n?', '', text)
    with open(path, 'wb') as f:
        f.write(text.encode(encoding))
    print('  Reset OptMode=-1 -> OptMode=0 in terminal.ini')
except Exception as e:
    print(f'  Warning: could not reset OptMode in terminal.ini: {e}')
" "$TERMINAL_INI" 2>/dev/null || true
        echo "  Reset terminal.ini OptMode"
    fi
else
    echo "[2/5] CLEAN    skipped"
fi

# ── Prepare .set file ─────────────────────────────────────────────────────────
if [[ -n "$SET_FILE" ]]; then
    # Resolve relative paths against PROJECT_DIR (fallback: script ROOT_DIR, then CWD)
    if [[ ! -f "$SET_FILE" ]]; then
        for base in "$PROJECT_DIR" "$ROOT_DIR" "$(pwd)"; do
            [[ -n "$base" && -f "${base}/${SET_FILE}" ]] && { SET_FILE="${base}/${SET_FILE}"; break; }
        done
    fi
    if [[ ! -f "$SET_FILE" ]]; then
        echo "ERROR: Set file not found: $SET_FILE" >&2
        exit 1
    fi
    # Copy to tester profiles dir (MT5 reads from here)
    mkdir -p "$MT5_TESTER_DIR"
    cp "$SET_FILE" "${MT5_TESTER_DIR}/${EXPERT}.set"
    SET_FILENAME="$(basename "$SET_FILE")"
fi

# ── Stage 3: BACKTEST ─────────────────────────────────────────────────────────
_progress "BACKTEST"
echo ""
echo "[3/5] BACKTEST"

# Detect running MT5 instance (match terminal64.exe regardless of wine wrapper name)
_mt5_is_running() { pgrep -f "terminal64\.exe" > /dev/null 2>&1; }

MT5_WAS_RUNNING=false
_mt5_is_running && MT5_WAS_RUNNING=true

# --kill-existing: terminate running MT5 before launch (use with --shutdown for CI)
if $KILL_EXISTING && $MT5_WAS_RUNNING; then
    echo "  Stopping running MT5 (--kill-existing)..."
    pkill -TERM -f "terminal64\.exe" 2>/dev/null || true
    for _i in 1 2 3 4 5; do
        sleep 1
        _mt5_is_running || break
    done
    _mt5_is_running && { pkill -KILL -f "terminal64\.exe" 2>/dev/null || true; sleep 1; }
    echo "  MT5 stopped."
    MT5_WAS_RUNNING=false
fi

# Build backtest.ini
REPORT_FILENAME="${REPORT_ID}.htm"
# Relative path — MT5 resolves against its working dir (C:\Program Files\MetaTrader 5\reports\)
WINE_REPORT_PATH="reports\\${REPORT_FILENAME}"

INI_HOST_PATH="${MT5_DIR}/backtest_config.ini"
REPORTS_HOST_DIR="${MT5_DIR}/reports"
mkdir -p "$REPORTS_HOST_DIR"

INI_CONTENT=""
if [[ -n "$DEFAULT_LOGIN" && -n "$DEFAULT_SERVER" ]]; then
    INI_CONTENT="[Common]
Login=${DEFAULT_LOGIN}
Server=${DEFAULT_SERVER}

"
fi

# ShutdownTerminal=0 (default): MT5 stays open; report detected via file watching.
# ShutdownTerminal=1 (--shutdown): MT5 exits after backtest; process-wait mode.
SHUTDOWN_VAL=0
$SHUTDOWN_TERMINAL && SHUTDOWN_VAL=1

INI_CONTENT+="[Tester]
Expert=${EXPERT}.ex5
Symbol=${SYMBOL}
Period=${TIMEFRAME}
Optimization=0
Model=${MODEL}
FromDate=${FROM_DATE}
ToDate=${TO_DATE}
ForwardMode=0
Deposit=${DEPOSIT}
Currency=${CURRENCY}
ProfitInPips=1
Leverage=${LEVERAGE}
ExecutionMode=10
OptimizationCriterion=0
Visual=$([[ "$GUI_MODE" == true ]] && echo 1 || echo 0)
Report=${WINE_REPORT_PATH}
ReplaceReport=1
ShutdownTerminal=${SHUTDOWN_VAL}
"
[[ -n "$SET_FILE" ]] && INI_CONTENT+="ExpertParameters=${EXPERT}.set
"

# MT5 requires UTF-16LE with BOM — plain UTF-8 is silently ignored
printf "%s" "$INI_CONTENT" | iconv -f UTF-8 -t UTF-16LE > "${INI_HOST_PATH}.tmp"
printf '\xff\xfe' | cat - "${INI_HOST_PATH}.tmp" > "${INI_HOST_PATH}"
rm -f "${INI_HOST_PATH}.tmp"

# Set Wine prefix
WINE_PREFIX_DIR=$(dirname "$(dirname "$(dirname "$MT5_DIR")")")
export WINEPREFIX="$WINE_PREFIX_DIR"
export WINEDEBUG="-all"

BACKTEST_START=$(date +%s)
BAT_PATH="${WINE_PREFIX_DIR}/drive_c/_mt5mcp_run.bat"

if $SHUTDOWN_TERMINAL; then
    # ── Synchronous mode (--shutdown) ─────────────────────────────────────────
    # terminal64.exe is a single-instance app: if MT5 is running, a second
    # launch exits immediately with no report. Kill first to avoid this.
    if $MT5_WAS_RUNNING; then
        echo "  WARNING: MT5 is running — stopping it (required for --shutdown mode)."
        pkill -TERM -f "terminal64\.exe" 2>/dev/null || true
        for _i in 1 2 3 4 5; do sleep 1; _mt5_is_running || break; done
        _mt5_is_running && { pkill -KILL -f "terminal64\.exe" 2>/dev/null || true; sleep 1; }
        echo "  MT5 stopped."
    fi
    cat > "$BAT_PATH" << 'BATEOF'
@echo off
cd /d "C:\Program Files\MetaTrader 5"
start /wait terminal64.exe /config:"C:\Program Files\MetaTrader 5\backtest_config.ini"
BATEOF
    echo "  Launching MT5 (timeout: ${TIMEOUT}s, shutdown mode)..."
    set +e
    timeout "${TIMEOUT}" ${MT5_ARCH} "${MT5_WINE}" cmd.exe /c 'C:\_mt5mcp_run.bat' 2>/dev/null
    WINE_EXIT=$?
    set -e
    rm -f "$BAT_PATH"
    BACKTEST_ELAPSED=$(( $(date +%s) - BACKTEST_START ))
    echo "  MT5 completed in ${BACKTEST_ELAPSED}s (exit: ${WINE_EXIT})"
    sleep 2
    # Locate report
    MT5_REPORT=""
    for ext in ".htm" ".htm.xml" ".html"; do
        candidate="${REPORTS_HOST_DIR}/${REPORT_ID}${ext}"
        [[ -f "$candidate" ]] && { MT5_REPORT="$candidate"; break; }
    done
    [[ -z "$MT5_REPORT" ]] && \
        MT5_REPORT=$(find "${MT5_DIR}" -maxdepth 3 -name "*.htm" -newer "${INI_HOST_PATH}" 2>/dev/null | head -1)
else
    # ── Background mode (default) ──────────────────────────────────────────────
    # MT5 is a single-instance Windows app: if already running, launching a
    # second instance passes the /config: args to the existing window and exits.
    # This triggers the Strategy Tester in the running MT5 without closing it.
    # If MT5 is not running, it starts fresh. Either way, ShutdownTerminal=0
    # keeps it alive; we detect completion by watching for the report file.
    if $MT5_WAS_RUNNING; then
        echo "  MT5 is running — passing config to existing instance..."
    else
        echo "  Launching MT5 in background (ShutdownTerminal=0)..."
    fi
    # `start` (no /wait): cmd.exe launches terminal64.exe and returns immediately.
    cat > "$BAT_PATH" << 'BATEOF'
@echo off
cd /d "C:\Program Files\MetaTrader 5"
start terminal64.exe /config:"C:\Program Files\MetaTrader 5\backtest_config.ini"
BATEOF
    nohup ${MT5_ARCH} "${MT5_WINE}" cmd.exe /c 'C:\_mt5mcp_run.bat' &>/dev/null &
    LAUNCHER_PID=$!
    disown "$LAUNCHER_PID" 2>/dev/null || true
    # Give the launcher time to start terminal64.exe and exit
    sleep 5
    rm -f "$BAT_PATH"

    # ── Poll for report file ───────────────────────────────────────────────────
    # MT5 writes the report when Strategy Tester finishes, before any shutdown.
    REPORT_DEADLINE=$(( $(date +%s) + TIMEOUT ))
    MT5_REPORT=""
    echo "  Waiting for backtest report (timeout: ${TIMEOUT}s)..."
    while [[ -z "$MT5_REPORT" ]]; do
        if [[ $(date +%s) -ge $REPORT_DEADLINE ]]; then
            echo "  ERROR: Timeout (${TIMEOUT}s) — no report produced." >&2
            echo "  Possible causes:" >&2
            echo "    - Symbol not found in MT5 history for this broker" >&2
            echo "    - EA name mismatch (check .ex5 exists in Experts/)" >&2
            if $MT5_WAS_RUNNING; then
                echo "    - Running MT5 ignored the config (try: --kill-existing)" >&2
            fi
            exit 1
        fi
        sleep 5
        ELAPSED=$(( $(date +%s) - BACKTEST_START ))
        for ext in ".htm" ".htm.xml" ".html"; do
            candidate="${REPORTS_HOST_DIR}/${REPORT_ID}${ext}"
            if [[ -f "$candidate" && -s "$candidate" ]]; then
                # Wait for file to stabilise (fully flushed to disk)
                S1=$(stat -f%z "$candidate" 2>/dev/null || stat -c%s "$candidate")
                sleep 2
                S2=$(stat -f%z "$candidate" 2>/dev/null || stat -c%s "$candidate")
                [[ "$S1" -gt 0 && "$S1" == "$S2" ]] && { MT5_REPORT="$candidate"; break 2; }
            fi
        done
        printf "  ... %ds elapsed\r" "$ELAPSED"
    done
    BACKTEST_ELAPSED=$(( $(date +%s) - BACKTEST_START ))
    echo "  Report ready in ${BACKTEST_ELAPSED}s"
fi

# ── Locate report file ────────────────────────────────────────────────────────
if [[ -z "$MT5_REPORT" ]]; then
    echo "  ERROR: MT5 produced no report." >&2
    echo "  Check: symbol name, date range, EA name, and that MT5 ran to completion." >&2
    exit 1
fi

echo "  Report: $MT5_REPORT"

# ── Stage 4: EXTRACT ─────────────────────────────────────────────────────────
_progress "EXTRACT"
echo ""
echo "[4/5] EXTRACT"

python3 "${ROOT_DIR}/analytics/extract.py" \
    "$MT5_REPORT" \
    --output-dir "$REPORT_DIR" \
    && echo "  → metrics.json, deals.csv, deals.json"

# ── Stage 5: ANALYZE ─────────────────────────────────────────────────────────
if [[ "$SKIP_ANALYZE" == false ]]; then
    _progress "ANALYZE"
    echo ""
    echo "[5/5] ANALYZE"

    ANALYZE_FLAGS="$STRATEGY"
    [[ "$DEEP_ANALYZE" == true ]] && ANALYZE_FLAGS="$ANALYZE_FLAGS --deep"

    python3 "${ROOT_DIR}/analytics/analyze.py" \
        $ANALYZE_FLAGS \
        "${REPORT_DIR}/deals.csv" \
        --output-dir "$REPORT_DIR" \
        && echo "  → analysis.json [$STRATEGY]"
else
    echo "[5/5] ANALYZE  skipped"
fi

# ── Save pipeline metadata ────────────────────────────────────────────────────
_progress "DONE"
PIPELINE_ELAPSED=$(( $(date +%s) - PIPELINE_START ))

python3 - << PYEOF
import json, os
meta = {
    "expert": "${EXPERT}",
    "symbol": "${SYMBOL}",
    "timeframe": "${TIMEFRAME}",
    "from_date": "${FROM_DATE}",
    "to_date": "${TO_DATE}",
    "deposit": ${DEPOSIT},
    "currency": "${CURRENCY}",
    "model": ${MODEL},
    "leverage": ${LEVERAGE},
    "set_file": "${SET_FILE}",
    "report_dir": "${REPORT_DIR}",
    "duration_seconds": ${PIPELINE_ELAPSED},
    "files": {
        "metrics": "${REPORT_DIR}/metrics.json",
        "analysis": "${REPORT_DIR}/analysis.json",
        "deals_csv": "${REPORT_DIR}/deals.csv",
        "deals_json": "${REPORT_DIR}/deals.json"
    }
}
with open("${REPORT_DIR}/pipeline_metadata.json", "w") as f:
    json.dump(meta, f, indent=2)
PYEOF

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo " Pipeline complete in ${PIPELINE_ELAPSED}s"
echo " Report: $REPORT_DIR"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Print key metrics inline
if [[ -f "${REPORT_DIR}/metrics.json" ]]; then
    python3 - << PYEOF
import json
with open("${REPORT_DIR}/metrics.json") as f:
    m = json.load(f)
print(f" Profit: \${m.get('net_profit',0):,.2f}  PF: {m.get('profit_factor',0):.2f}  DD: {m.get('max_dd_pct',0):.2f}%  Sharpe: {m.get('sharpe_ratio',0):.2f}  Trades: {m.get('total_trades',0)}")
PYEOF
fi
