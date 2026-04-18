#!/usr/bin/env bash
# platform_detect.sh — Detect Wine path, MT5 location, and display mode
# Sourced by other scripts: source scripts/platform_detect.sh
# Sets: MT5_WINE, MT5_DIR, MT5_EXPERTS_DIR, MT5_TESTER_DIR, MT5_CACHE_DIR, DISPLAY_ENV

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Config resolution: user config (~/.config/mt5-quant/) takes precedence over repo config
_USER_CFG="${HOME}/.config/mt5-quant/config/mt5-quant.yaml"
_REPO_CFG="${SCRIPT_DIR}/../config/mt5-quant.yaml"
if [[ -f "$_USER_CFG" ]]; then
    CONFIG_FILE="$_USER_CFG"
elif [[ -f "$_REPO_CFG" ]]; then
    CONFIG_FILE="$_REPO_CFG"
else
    CONFIG_FILE="$_REPO_CFG"  # will fail gracefully in _cfg
fi

# ── Config reader (minimal YAML parser for simple key: value) ────────────────
_cfg() {
    local key="$1"
    local default="${2:-}"
    if [[ -f "$CONFIG_FILE" ]]; then
        local val
        val=$(grep -E "^[[:space:]]*${key}[[:space:]]*:" "$CONFIG_FILE" 2>/dev/null \
              | head -1 | sed 's/.*:[[:space:]]*//' | tr -d '"' | tr -d "'" | tr -d '\r')
        if [[ -n "$val" && "$val" != "null" && "$val" != '""' ]]; then
            echo "$val"
            return
        fi
    fi
    echo "$default"
}

# ── Wine / CrossOver detection ───────────────────────────────────────────────
_detect_wine() {
    local configured
    configured=$(_cfg "wine_executable")

    if [[ -n "$configured" && -x "$configured" ]]; then
        echo "$configured"
        return 0
    fi

    # Auto-detect: native MT5.app (macOS App Store / direct download)
    local mt5_app_wine="/Applications/MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64"
    if [[ -x "$mt5_app_wine" ]]; then
        echo "$mt5_app_wine"
        return 0
    fi

    # Auto-detect: CrossOver (macOS)
    local crossover_wine="/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine64"
    if [[ -x "$crossover_wine" ]]; then
        echo "$crossover_wine"
        return 0
    fi

    # Auto-detect: Wine (Linux / Homebrew)
    for candidate in wine64 wine; do
        if command -v "$candidate" &>/dev/null; then
            echo "$(command -v "$candidate")"
            return 0
        fi
    done

    echo ""
    return 1
}

# ── MT5 terminal directory detection ─────────────────────────────────────────
_detect_mt5_dir() {
    local configured
    configured=$(_cfg "terminal_dir")
    [[ -n "$configured" && -d "$configured" ]] && { echo "$configured"; return 0; }

    # macOS CrossOver — scan all bottles
    if [[ "$(uname -s)" == "Darwin" ]]; then
        # Native MT5.app sandboxed Wine prefix (most common on macOS)
        local app_support="$HOME/Library/Application Support"
        for prefix_pattern in \
            "net.metaquotes.wine.metatrader5" \
            "MetaTrader 5/Bottles/metatrader5"; do
            local candidate="${app_support}/${prefix_pattern}/drive_c/Program Files/MetaTrader 5"
            if [[ -f "${candidate}/terminal64.exe" ]]; then
                echo "$candidate"
                return 0
            fi
        done

        # CrossOver old-style ~/.cxoffice bottles
        local bottle_base="$HOME/.cxoffice"
        if [[ -d "$bottle_base" ]]; then
            while IFS= read -r -d '' terminal; do
                echo "$(dirname "$terminal")"
                return 0
            done < <(find "$bottle_base" -name "terminal64.exe" -print0 2>/dev/null | head -z -1)
        fi
        # CrossOver 24+ default location
        local mq_base="$HOME/Library/Application Support/MetaQuotes"
        if [[ -d "$mq_base" ]]; then
            while IFS= read -r -d '' terminal; do
                echo "$(dirname "$terminal")"
                return 0
            done < <(find "$mq_base" -name "terminal64.exe" -print0 2>/dev/null | head -z -1)
        fi
    fi

    # Linux Wine — common default
    local wine_mt5="$HOME/.wine/drive_c/Program Files/MetaTrader 5"
    [[ -d "$wine_mt5" ]] && { echo "$wine_mt5"; return 0; }

    echo ""
    return 1
}

# ── Display / headless mode ───────────────────────────────────────────────────
_detect_display_env() {
    local mode
    mode=$(_cfg "display_mode" "auto")
    local xvfb_display
    xvfb_display=$(_cfg "xvfb_display" ":99")
    local xvfb_screen
    xvfb_screen=$(_cfg "xvfb_screen" "1024x768x16")

    case "$mode" in
        false|gui)
            # GUI mode — use whatever DISPLAY is set
            echo "gui"
            return 0
            ;;
        true|headless)
            # Force headless via Xvfb
            _start_xvfb "$xvfb_display" "$xvfb_screen"
            echo "headless:${xvfb_display}"
            return 0
            ;;
        auto|*)
            # macOS: CrossOver handles display — no Xvfb needed
            if [[ "$(uname -s)" == "Darwin" ]]; then
                echo "gui"
                return 0
            fi
            # Linux: if $DISPLAY is set, use it; otherwise start Xvfb
            if [[ -n "${DISPLAY:-}" ]]; then
                echo "gui"
                return 0
            fi
            _start_xvfb "$xvfb_display" "$xvfb_screen"
            echo "headless:${xvfb_display}"
            return 0
            ;;
    esac
}

_start_xvfb() {
    local display="$1"
    local screen="$2"

    if ! command -v Xvfb &>/dev/null; then
        echo "[platform_detect] ERROR: headless mode requires Xvfb. Install with:" >&2
        echo "  sudo apt install xvfb   # Debian/Ubuntu" >&2
        echo "  sudo yum install xorg-x11-server-Xvfb  # RHEL/CentOS" >&2
        exit 1
    fi

    # Check if already running
    if xdpyinfo -display "$display" &>/dev/null 2>&1; then
        return 0  # already up
    fi

    Xvfb "$display" -screen 0 "$screen" &>/dev/null &
    local xvfb_pid=$!
    sleep 1  # brief wait for Xvfb to initialize

    if ! xdpyinfo -display "$display" &>/dev/null 2>&1; then
        echo "[platform_detect] ERROR: Xvfb failed to start on display ${display}" >&2
        exit 1
    fi

    echo "[platform_detect] Xvfb started (pid=${xvfb_pid}, display=${display})" >&2
}

# ── macOS arch flag ───────────────────────────────────────────────────────────
_arch_prefix() {
    if [[ "$(uname -s)" == "Darwin" && "$(uname -m)" == "arm64" ]]; then
        echo "arch -x86_64"
    else
        echo ""
    fi
}

# ── Wine path converter (host path → Windows C:\ path) ───────────────────────
host_to_wine_path() {
    local host_path="$1"
    # Convert Unix absolute path to Wine C:\ equivalent
    # Works for both CrossOver and standard Wine
    echo "$host_path" | sed \
        -e 's|.*drive_c/|C:\\|' \
        -e 's|/|\\|g'
}

# ── Main: resolve everything and export ───────────────────────────────────────
resolve_platform() {
    MT5_WINE=$(_detect_wine) || {
        echo "[platform_detect] ERROR: Wine/CrossOver not found." >&2
        echo "  Configure wine_executable in config/mt5-quant.yaml" >&2
        echo "  macOS: install CrossOver from https://www.codeweavers.com/" >&2
        echo "  Linux: sudo apt install wine64" >&2
        exit 1
    }

    MT5_DIR=$(_detect_mt5_dir) || {
        echo "[platform_detect] ERROR: MetaTrader 5 installation not found." >&2
        echo "  Configure terminal_dir in config/mt5-quant.yaml" >&2
        exit 1
    }

    # Derive sub-paths from MT5_DIR (override via config if needed)
    MT5_EXPERTS_DIR="$(_cfg "experts_dir" "${MT5_DIR}/MQL5/Experts")"
    MT5_TESTER_DIR="$(_cfg "tester_profiles_dir" "${MT5_DIR}/MQL5/Profiles/Tester")"
    MT5_CACHE_DIR="$(_cfg "tester_cache_dir" "${MT5_DIR}/Tester")"
    MT5_ARCH="$(_arch_prefix)"

    DISPLAY_MODE=$(_detect_display_env)
    if [[ "$DISPLAY_MODE" == headless:* ]]; then
        export DISPLAY="${DISPLAY_MODE#headless:}"
    fi

    export MT5_WINE MT5_DIR MT5_EXPERTS_DIR MT5_TESTER_DIR MT5_CACHE_DIR MT5_ARCH DISPLAY_MODE
}

# Run if executed directly (not sourced)
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    resolve_platform
    echo "Wine:       $MT5_WINE"
    echo "MT5 dir:    $MT5_DIR"
    echo "Experts:    $MT5_EXPERTS_DIR"
    echo "Tester:     $MT5_TESTER_DIR"
    echo "Cache:      $MT5_CACHE_DIR"
    echo "Display:    $DISPLAY_MODE"
    echo "Arch:       ${MT5_ARCH:-native}"
fi
