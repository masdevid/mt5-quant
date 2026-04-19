#!/usr/bin/env bash
# setup.sh — Auto-detect MT5/Wine paths and write config/mt5-quant.yaml
# Usage: bash scripts/setup.sh [--yes] [--output /path/to/config.yaml] [--keep-last N] [--claude-code]
#
# --yes            Overwrite existing config and register MCP without prompting
# --output FILE    Write to a custom path instead of config/mt5-quant.yaml
# --keep-last N    Keep only last N backtest reports (default: 20)
# --claude-code    Generate CLAUDE.md template and .claude/hooks/user-prompt-submit.sh
#                  (skips main config wizard — run standalone or alongside normal setup)
#
# MCP Auto-Registration (per official 2025 docs):
#   Automatically detects and registers with available MCP platforms:
#   - Claude Code    : via 'claude mcp add' (stored in ~/.claude.json)
#   - Windsurf     : ~/.codeium/windsurf/mcp_config.json (JSON, mcpServers)
#   - Cursor       : ~/.cursor/mcp.json (JSON, mcpServers)
#   - VS Code      : .vscode/mcp.json (JSON, servers - not mcpServers)
#   - Antigravity  : mcp_config.json via UI (JSON, mcpServers)
#
#   Previous installations are auto-detected and uninstalled before re-registering.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
CONFIG_OUT="${REPO_DIR}/config/mt5-quant.yaml"
AUTO_YES=false
KEEP_LAST=20
CLAUDE_CODE=false

# ── Argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --yes|-y)       AUTO_YES=true ;;
        --output)       CONFIG_OUT="$2"; shift ;;
        --keep-last)    KEEP_LAST="$2"; shift ;;
        --claude-code)  CLAUDE_CODE=true ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
    shift
done

# ── Helpers ───────────────────────────────────────────────────────────────────
_green()  { printf '\033[0;32m%s\033[0m\n' "$*"; }
_yellow() { printf '\033[0;33m%s\033[0m\n' "$*"; }
_red()    { printf '\033[0;31m%s\033[0m\n' "$*"; }
_bold()   { printf '\033[1m%s\033[0m\n' "$*"; }
_ok()     { printf '  \033[0;32m✓\033[0m  %s\n' "$*"; }
_warn()   { printf '  \033[0;33m⚠\033[0m  %s\n' "$*"; }
_fail()   { printf '  \033[0;31m✗\033[0m  %s\n' "$*"; }

_ask() {
    # _ask "Prompt text" default_value → echoes user input or default
    local prompt="$1"
    local default="${2:-}"
    if [[ -n "$default" ]]; then
        printf '%s [%s]: ' "$prompt" "$default" >&2
    else
        printf '%s: ' "$prompt" >&2
    fi
    local answer
    read -r answer
    echo "${answer:-$default}"
}

_pick() {
    # _pick "Label" item1 item2 ... → echoes chosen item
    local label="$1"; shift
    local items=("$@")
    if [[ ${#items[@]} -eq 1 ]]; then
        echo "${items[0]}"
        return
    fi
    printf '\n%s\n' "$label" >&2
    local i
    for i in "${!items[@]}"; do
        printf '  [%d] %s\n' "$((i+1))" "${items[$i]}" >&2
    done
    local choice
    while true; do
        printf '  Choose [1-%d]: ' "${#items[@]}" >&2
        read -r choice
        if [[ "$choice" =~ ^[0-9]+$ ]] && (( choice >= 1 && choice <= ${#items[@]} )); then
            echo "${items[$((choice-1))]}"
            return
        fi
        printf '  Invalid choice.\n' >&2
    done
}

# ── Wine candidate scanner ────────────────────────────────────────────────────
_scan_wine_candidates() {
    local candidates=()
    local os
    os="$(uname -s)"

    if [[ "$os" == "Darwin" ]]; then
        # Native MT5.app (bundled Wine — most common on macOS)
        local mt5_app_wine="/Applications/MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64"
        [[ -x "$mt5_app_wine" ]] && candidates+=("$mt5_app_wine")

        # CrossOver (classic install)
        local cxover="/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine64"
        [[ -x "$cxover" ]] && candidates+=("$cxover")

        # Homebrew wine
        for brew_wine in /opt/homebrew/bin/wine64 /usr/local/bin/wine64 \
                         /opt/homebrew/bin/wine   /usr/local/bin/wine; do
            [[ -x "$brew_wine" ]] && candidates+=("$brew_wine")
        done
    fi

    # Linux / fallback
    for sys_wine in wine64 wine; do
        local found
        found="$(command -v "$sys_wine" 2>/dev/null || true)"
        [[ -n "$found" && -x "$found" ]] && candidates+=("$found")
    done

    # Deduplicate
    local seen=()
    local c
    for c in "${candidates[@]}"; do
        local dup=false
        local s
        for s in "${seen[@]:-}"; do [[ "$s" == "$c" ]] && dup=true && break; done
        $dup || seen+=("$c")
    done

    printf '%s\n' "${seen[@]:-}"
}

# ── MT5 prefix scanner ────────────────────────────────────────────────────────
_scan_mt5_prefixes() {
    # Returns list of terminal_dir paths (each containing terminal64.exe)
    local results=()
    local os
    os="$(uname -s)"

    if [[ "$os" == "Darwin" ]]; then
        local search_roots=(
            # Native MT5.app sandboxed prefix
            "$HOME/Library/Application Support"
            # CrossOver old
            "$HOME/.cxoffice"
            # CrossOver 24+
            "$HOME/Library/Application Support/MetaQuotes"
        )
        local root
        for root in "${search_roots[@]}"; do
            [[ -d "$root" ]] || continue
            while IFS= read -r -d '' terminal_exe; do
                results+=("$(dirname "$terminal_exe")")
            done < <(find "$root" -maxdepth 8 -name "terminal64.exe" -print0 2>/dev/null)
        done
    fi

    # Linux Wine prefixes
    local linux_roots=(
        "$HOME/.wine"
        "$HOME/.wine64"
    )
    local root
    for root in "${linux_roots[@]}"; do
        [[ -d "$root" ]] || continue
        local mt5_dir="${root}/drive_c/Program Files/MetaTrader 5"
        [[ -f "${mt5_dir}/terminal64.exe" ]] && results+=("$mt5_dir")
    done

    # Deduplicate
    local seen=()
    local c
    for c in "${results[@]:-}"; do
        local dup=false
        local s
        for s in "${seen[@]:-}"; do [[ "$s" == "$c" ]] && dup=true && break; done
        $dup || seen+=("$c")
    done

    printf '%s\n' "${seen[@]:-}"
}

# Score an MT5 dir by activity: count .ex5 + .set files (higher = more active)
_score_mt5_dir() {
    local dir="$1"
    local count=0
    count=$(find "$dir" -name "*.ex5" -o -name "*.set" 2>/dev/null | wc -l | tr -d ' ')
    echo "$count"
}

# Pick the most active MT5 dir from a list
_best_mt5_dir() {
    local best="" best_score=-1
    local d
    while IFS= read -r d; do
        [[ -z "$d" ]] && continue
        local score
        score=$(_score_mt5_dir "$d")
        if (( score > best_score )); then
            best="$d"
            best_score=$score
        fi
    done
    echo "$best"
}

# ── MT5 auto-installer ───────────────────────────────────────────────────────
#
# MetaTrader 5 provides official installers for both platforms:
#   macOS  — DMG from MetaQuotes CDN (includes bundled Wine, no CrossOver needed)
#   Linux  — Official bash installer from MetaQuotes (handles Wine + MT5 prefix)
#
# After install, MT5 must be launched once to unpack terminal64.exe into the
# Wine prefix. _install_mt5() handles this automatically on Linux (headless via
# Xvfb). On macOS the user must launch the app once manually (GUI required).

MT5_DMG_URL="https://download.mql5.com/cdn/web/metaquotes.software.corp/mt5/MetaTrader5.dmg"
MT5_LINUX_URL="https://download.mql5.com/cdn/web/metaquotes.software.corp/mt5/mt5ubuntu.sh"
MT5_EXE_URL="https://download.mql5.com/cdn/web/metaquotes.software.corp/mt5/mt5setup.exe"

_install_mt5() {
    local os
    os="$(uname -s)"

    if [[ "$os" == "Darwin" ]]; then
        _install_mt5_macos
    else
        _install_mt5_linux
    fi
}

_install_mt5_macos() {
    echo ""
    _bold "Installing MetaTrader 5 for macOS..."

    if [[ -d "/Applications/MetaTrader 5.app" ]]; then
        _ok "MetaTrader 5.app already present in /Applications"
        return 0
    fi

    # Prefer mas (Mac App Store CLI) — avoids Gatekeeper issues
    if command -v mas &>/dev/null; then
        _ok "Using mas (Mac App Store CLI)..."
        # MT5 App Store ID: 413698442
        mas install 413698442 && {
            _ok "Installed via Mac App Store"
            return 0
        } || _warn "mas install failed — falling back to direct download"
    fi

    # Direct download from MetaQuotes CDN
    local dmg="/tmp/MetaTrader5.dmg"
    _ok "Downloading MetaTrader5.dmg from MetaQuotes CDN..."
    if ! curl -L --progress-bar --connect-timeout 30 "$MT5_DMG_URL" -o "$dmg"; then
        _fail "Download failed. Check your internet connection."
        echo ""
        echo "  Manual install: open https://www.metatrader5.com/en/terminal/help/start_advanced/install_mac"
        return 1
    fi

    _ok "Mounting DMG..."
    local mnt="/tmp/mt5_mount"
    if ! hdiutil attach "$dmg" -mountpoint "$mnt" -quiet -nobrowse; then
        _fail "Could not mount DMG: $dmg"
        return 1
    fi

    _ok "Copying MetaTrader 5.app to /Applications..."
    cp -R "${mnt}/MetaTrader 5.app" /Applications/ 2>/dev/null || {
        _fail "Copy failed — try: sudo cp -R \"${mnt}/MetaTrader 5.app\" /Applications/"
        hdiutil detach "$mnt" -quiet 2>/dev/null
        return 1
    }

    hdiutil detach "$mnt" -quiet 2>/dev/null
    rm -f "$dmg"
    _ok "MetaTrader 5.app installed to /Applications"

    echo ""
    _yellow "ACTION REQUIRED: Launch MetaTrader 5.app once to complete initialization."
    echo "  It will create the Wine prefix and download terminal64.exe."
    echo "  After it loads (shows the login screen), you can close it."
    echo ""
    if ! $AUTO_YES; then
        _ask "Press Enter when MT5 has been launched and closed..." ""
    fi
}

_install_mt5_linux() {
    echo ""
    _bold "Installing MetaTrader 5 for Linux..."

    # Check for existing Wine terminal64.exe anywhere
    local existing
    existing=$(find "$HOME/.wine" "$HOME/.wine64" \
        "$HOME/Library/Application Support" 2>/dev/null \
        -name "terminal64.exe" -print -quit 2>/dev/null || true)
    if [[ -n "$existing" ]]; then
        _ok "terminal64.exe already found at: $existing"
        return 0
    fi

    # MetaQuotes provides an official Ubuntu installer that handles Wine + MT5
    local has_wget has_curl
    has_wget=$(command -v wget &>/dev/null && echo yes || echo no)
    has_curl=$(command -v curl &>/dev/null && echo yes || echo no)

    local script="/tmp/mt5ubuntu.sh"
    _ok "Downloading MetaQuotes Linux installer..."
    if [[ "$has_wget" == "yes" ]]; then
        wget -q --show-progress "$MT5_LINUX_URL" -O "$script" || {
            _fail "Download failed. Check: $MT5_LINUX_URL"
            return 1
        }
    elif [[ "$has_curl" == "yes" ]]; then
        curl -L --progress-bar "$MT5_LINUX_URL" -o "$script" || {
            _fail "Download failed. Check: $MT5_LINUX_URL"
            return 1
        }
    else
        _fail "Neither wget nor curl found. Install one and retry."
        return 1
    fi

    chmod +x "$script"
    _ok "Running MetaQuotes Linux installer (may prompt for sudo)..."
    bash "$script" || {
        _fail "Installer failed. Check output above."
        rm -f "$script"
        return 1
    }
    rm -f "$script"

    # The MetaQuotes installer creates the Wine prefix and launches MT5 briefly.
    # On headless systems, we use Xvfb to allow MT5 to initialize.
    if [[ -z "${DISPLAY:-}" ]]; then
        _ok "Headless system — running MT5 init via Xvfb..."
        if ! command -v Xvfb &>/dev/null; then
            _warn "Xvfb not found. Install with: sudo apt install xvfb"
            _warn "Then launch MT5 manually: DISPLAY=:99 metatrader5"
            return 0
        fi
        Xvfb :99 -screen 0 1024x768x16 &>/dev/null &
        local xvfb_pid=$!
        sleep 2
        DISPLAY=:99 metatrader5 &>/dev/null &
        local mt5_pid=$!
        _ok "MT5 initializing (30s)..."
        sleep 30
        kill "$mt5_pid" 2>/dev/null || true
        kill "$xvfb_pid" 2>/dev/null || true
    else
        _ok "Launching MT5 to initialize Wine prefix (closes in 20s)..."
        metatrader5 &>/dev/null &
        local mt5_pid=$!
        sleep 20
        kill "$mt5_pid" 2>/dev/null || true
    fi

    _ok "MT5 initialization complete"
}

# ── Display mode detection ────────────────────────────────────────────────────
_detect_display_mode() {
    if [[ "$(uname -s)" == "Darwin" ]]; then
        echo "auto"  # macOS always GUI
    elif [[ -n "${DISPLAY:-}" ]]; then
        echo "auto"  # Linux with display
    else
        echo "auto"  # headless Linux — auto will pick Xvfb
    fi
}

# ── YAML writer ───────────────────────────────────────────────────────────────
_write_yaml() {
    local wine="$1"
    local terminal_dir="$2"
    local display_mode="$3"
    local keep_last="${4:-20}"

    # Quote paths that contain spaces for YAML (wrap in double quotes)
    local wine_q="\"${wine}\""
    local terminal_dir_q="\"${terminal_dir}\""
    local experts_q="\"${terminal_dir}/MQL5/Experts\""
    local tester_q="\"${terminal_dir}/MQL5/Profiles/Tester\""
    local cache_q="\"${terminal_dir}/Tester\""

    cat > "$CONFIG_OUT" <<YAML
# mt5-quant configuration — generated by scripts/setup.sh
# Re-run scripts/setup.sh to regenerate, or edit manually.

mt5:
  # Path to Wine / CrossOver wine64 binary
  wine_executable: ${wine_q}

  # MT5 installation directory (inside the Wine prefix)
  terminal_dir: ${terminal_dir_q}

  # Where MT5 looks for Expert Advisor binaries (.ex5)
  experts_dir: ${experts_q}

  # Where MT5 reads/writes .set files during backtests
  tester_profiles_dir: ${tester_q}

  # MT5 tester cache directory (cleared before each run)
  tester_cache_dir: ${cache_q}

display:
  # headless: true  → Xvfb (Linux only)
  # headless: false → GUI visible
  # headless: auto  → macOS=GUI, Linux with \$DISPLAY=GUI, Linux without=Xvfb
  mode: ${display_mode}

  # Virtual display for Xvfb (Linux headless only)
  xvfb_display: ":99"
  xvfb_screen: "1024x768x16"

backtest:
  symbol: "XAUUSD"
  deposit: 10000
  currency: "USD"
  leverage: 500
  model: 0       # 0=every tick (required for martingale/grid EAs)
  timeframe: "M5"
  timeout: 900   # seconds per backtest run

optimization:
  log_dir: "/tmp"
  min_agents: 1

reports:
  output_dir: "./reports"
  keep_last: ${keep_last}
YAML
}

# ── Validation ────────────────────────────────────────────────────────────────
_validate() {
    local wine="$1"
    local terminal_dir="$2"
    local ok=true

    echo ""
    _bold "Validating paths..."

    if [[ -x "$wine" ]]; then
        _ok "wine_executable: $wine"
    else
        _fail "wine_executable not found or not executable: $wine"
        ok=false
    fi

    if [[ -d "$terminal_dir" ]]; then
        _ok "terminal_dir: $terminal_dir"
    else
        _fail "terminal_dir not found: $terminal_dir"
        ok=false
    fi

    local terminal_exe="${terminal_dir}/terminal64.exe"
    if [[ -f "$terminal_exe" ]]; then
        _ok "terminal64.exe found"
    else
        _warn "terminal64.exe not found (expected at ${terminal_exe})"
    fi

    local experts_dir="${terminal_dir}/MQL5/Experts"
    if [[ -d "$experts_dir" ]]; then
        local ea_count
        ea_count=$(find "$experts_dir" -name "*.ex5" 2>/dev/null | wc -l | tr -d ' ')
        _ok "experts_dir: ${ea_count} .ex5 file(s) found"
    else
        _warn "experts_dir not found yet (will be created by MT5 on first run)"
    fi

    local tester_dir="${terminal_dir}/MQL5/Profiles/Tester"
    if [[ -d "$tester_dir" ]]; then
        local set_count
        set_count=$(find "$tester_dir" -name "*.set" 2>/dev/null | wc -l | tr -d ' ')
        _ok "tester_profiles_dir: ${set_count} .set file(s) found"
    else
        _warn "tester_profiles_dir not found yet (will be created by MT5 on first run)"
    fi

    $ok
}

# ── Claude Code generation ────────────────────────────────────────────────────
#
# Writes two files to help users integrate Claude Code with their backtesting workflow:
#
#   config/CLAUDE.template.md          — copy to your EA project root as CLAUDE.md
#   .claude/hooks/user-prompt-submit.sh — injects production baseline into every prompt
#
# The hook reads config/baseline.json (gitignored, user-maintained).
# baseline.json schema:
#   {
#     "symbol": "XAUUSD.cent",
#     "period": "2024-01-01/2024-12-31",
#     "net_profit": 1250.50,
#     "profit_factor": 1.43,
#     "max_drawdown_pct": 18.2,
#     "sharpe_ratio": 0.87,
#     "total_trades": 342,
#     "notes": "Best config as of 2024-12-15"
#   }

_generate_claude_code() {
    echo ""
    _bold "Generating Claude Code integration files..."

    # ── CLAUDE.md template ────────────────────────────────────────────────────
    local template_out="${REPO_DIR}/config/CLAUDE.template.md"
    if [[ -f "$template_out" ]] && ! $AUTO_YES; then
        local ans
        ans=$(_ask "config/CLAUDE.template.md already exists. Overwrite?" "no")
        if [[ ! "$ans" =~ ^[Yy] ]]; then
            _warn "Skipping CLAUDE.md template"
        else
            _write_claude_template "$template_out"
        fi
    else
        _write_claude_template "$template_out"
    fi

    # ── .claude/hooks/user-prompt-submit.sh ───────────────────────────────────
    local hooks_dir="${REPO_DIR}/.claude/hooks"
    mkdir -p "$hooks_dir"
    local hook_out="${hooks_dir}/user-prompt-submit.sh"
    if [[ -f "$hook_out" ]] && ! $AUTO_YES; then
        local ans
        ans=$(_ask ".claude/hooks/user-prompt-submit.sh already exists. Overwrite?" "no")
        if [[ ! "$ans" =~ ^[Yy] ]]; then
            _warn "Skipping hook generation"
            return
        fi
    fi
    _write_baseline_hook "$hook_out"
    chmod +x "$hook_out"
    _ok "Written: $hook_out"

    echo ""
    _green "Claude Code files ready."
    echo ""
    echo "  Next steps:"
    echo "  1. Copy config/CLAUDE.template.md to your EA project root as CLAUDE.md"
    echo "  2. Create config/baseline.json with your production backtest metrics"
    echo "     (see comments in .claude/hooks/user-prompt-submit.sh for the schema)"
    echo "  3. The hook auto-injects your baseline into every Claude prompt"
    echo ""
}

_write_claude_template() {
    local out="$1"
    cat > "$out" <<'CLAUDE_MD'
# CLAUDE.md — [Your EA Project Name]

## MT5 MCP Integration

This project uses [mt5-quant](https://github.com/masdevid/mt5-quant) for backtesting and
optimization via Claude Code MCP tools.

Available MCP tools: `run_backtest`, `run_optimization`, `get_results`,
`list_reports`, `get_report`, `get_analysis`

## Baseline Tracking

Production baseline is in `config/baseline.json` (gitignored, user-maintained).

- Always compare new backtest results against the baseline before calling a result
  an improvement. A result only counts as better if it beats the baseline on the
  primary metric without significantly degrading secondary metrics.
- Update `baseline.json` only after confirming improvement in live/demo testing.
- Key metrics: `net_profit`, `profit_factor`, `max_drawdown_pct`, `sharpe_ratio`,
  `total_trades`.

## Symbol Name Rules

- Always use the **exact** symbol name from the broker
  (e.g., `"XAUUSD.cent"` not `"XAUUSD"` — they are different instruments).
- The symbol is set in `config/mt5-quant.yaml` under `backtest.symbol`.
- Never hardcode a symbol name in tool calls — read it from config.

## Backtest Rules

- Model 0 (every tick) is required for martingale/grid EAs.
- Never run two backtests in parallel — MT5 uses a single Wine prefix.
- After optimization, reset `OptMode` in `terminal.ini` before the next backtest.

## Optimization Rules

- Optimizations run in the background (`nohup+disown`) — never add a timeout.
- Use Model 0 only — Model 1 overfits martingale/grid strategies.
- `.set` files must be UTF-16LE; UTF-8 causes MT5 to strip `||Y` parameter flags.
CLAUDE_MD
    _ok "Written: $out (copy to your EA project root as CLAUDE.md)"
}

_write_baseline_hook() {
    local out="$1"
    cat > "$out" <<'HOOK_SH'
#!/usr/bin/env bash
# .claude/hooks/user-prompt-submit.sh
# Injects the current production baseline into every Claude Code prompt.
#
# Reads: config/baseline.json  (gitignored — create and maintain manually)
#
# baseline.json schema:
# {
#   "symbol":           "XAUUSD.cent",
#   "period":           "2024-01-01/2024-12-31",
#   "net_profit":       1250.50,
#   "profit_factor":    1.43,
#   "max_drawdown_pct": 18.2,
#   "sharpe_ratio":     0.87,
#   "total_trades":     342,
#   "notes":            "Best config as of 2024-12-15"
# }

HOOK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${HOOK_DIR}/../.." && pwd)"
BASELINE="${REPO_DIR}/config/baseline.json"

[[ -f "$BASELINE" ]] || exit 0

python3 - "$BASELINE" <<'PY'
import json, sys

path = sys.argv[1]
try:
    with open(path) as f:
        baseline = json.load(f)
except Exception as e:
    sys.exit(0)  # Malformed baseline — don't block the prompt

context = (
    "## Production Baseline (config/baseline.json)\n"
    "Compare all backtest results against these metrics. "
    "A result is an improvement only if it beats the baseline on the primary "
    "metric without significantly degrading secondary metrics.\n"
    "```json\n"
    + json.dumps(baseline, indent=2)
    + "\n```"
)
print(json.dumps({"context": context}))
PY
HOOK_SH
}

# ── Main ──────────────────────────────────────────────────────────────────────
main() {
    # --claude-code: skip the config wizard — only generate Claude Code integration files
    if $CLAUDE_CODE; then
        _generate_claude_code
        exit 0
    fi

    echo ""
    _bold "MT5-Quant setup — auto-detecting Wine and MT5 paths"
    echo  "────────────────────────────────────────────────────"

    # ── Check for previous installation ────────────────────────────────────────
    if _check_any_existing_registration; then
        _yellow "Previous mt5-quant MCP installation detected on one or more platforms"
        echo "  New path: ${REPO_DIR}/server/main.py"

        local reinstall_ans="yes"
        if ! $AUTO_YES; then
            reinstall_ans=$(_ask "Unregister all previous installations and reinstall?" "yes")
        fi

        if [[ "$reinstall_ans" =~ ^[Yy] ]]; then
            _unregister_all_platforms || {
                if ! $AUTO_YES; then
                    local force_ans
                    force_ans=$(_ask "Some platforms failed to unregister. Continue anyway?" "no")
                    [[ ! "$force_ans" =~ ^[Yy] ]] && exit 1
                fi
            }
        else
            echo "Aborted — keeping existing installations."
            exit 0
        fi
    fi

    # ── Check if config already exists ───────────────────────────────────────
    if [[ -f "$CONFIG_OUT" ]] && ! $AUTO_YES; then
        _yellow "Config already exists: $CONFIG_OUT"
        local ans
        ans=$(_ask "Overwrite?" "no")
        if [[ ! "$ans" =~ ^[Yy] ]]; then
            echo "Aborted — existing config unchanged."
            exit 0
        fi
    fi

    # ── Detect Wine ───────────────────────────────────────────────────────────
    echo ""
    _bold "Scanning for Wine..."
    local wine_candidates=()
    while IFS= read -r line; do
        [[ -n "$line" ]] && wine_candidates+=("$line")
    done < <(_scan_wine_candidates)

    local wine=""
    if [[ ${#wine_candidates[@]} -eq 0 ]]; then
        _fail "No Wine installation found."
        local install_ans="yes"
        if ! $AUTO_YES; then
            install_ans=$(_ask "Download and install MetaTrader 5 automatically?" "yes")
        fi
        if [[ "$install_ans" =~ ^[Yy] ]]; then
            _install_mt5 || { _red "Auto-install failed. Install MT5 manually and re-run setup.sh."; exit 1; }
            # Re-scan after install
            while IFS= read -r line; do
                [[ -n "$line" ]] && wine_candidates+=("$line")
            done < <(_scan_wine_candidates)
        fi
        if [[ ${#wine_candidates[@]} -eq 0 ]]; then
            if ! $AUTO_YES; then
                wine=$(_ask "Enter wine64 path manually (or press Enter to abort)")
            fi
            [[ -z "$wine" ]] && { _red "Cannot continue without Wine."; exit 1; }
        fi
    fi
    if [[ -z "$wine" ]]; then
        if [[ ${#wine_candidates[@]} -eq 1 ]]; then
            wine="${wine_candidates[0]}"
            _ok "Found: $wine"
        else
            _ok "Found ${#wine_candidates[@]} Wine installations"
            if $AUTO_YES; then
                wine="${wine_candidates[0]}"
                _ok "Auto-selected: $wine"
            else
                wine=$(_pick "Select Wine executable:" "${wine_candidates[@]}")
            fi
        fi
    fi

    # ── Detect MT5 ────────────────────────────────────────────────────────────
    echo ""
    _bold "Scanning for MetaTrader 5 installations..."
    local mt5_candidates=()
    while IFS= read -r line; do
        [[ -n "$line" ]] && mt5_candidates+=("$line")
    done < <(_scan_mt5_prefixes)

    local terminal_dir=""
    if [[ ${#mt5_candidates[@]} -eq 0 ]]; then
        _fail "No MetaTrader 5 installation found."
        # Wine was found but MT5 prefix doesn't exist yet — offer to run installer
        local install_ans2="yes"
        if ! $AUTO_YES; then
            install_ans2=$(_ask "Install MetaTrader 5 and initialize Wine prefix?" "yes")
        fi
        if [[ "$install_ans2" =~ ^[Yy] ]]; then
            _install_mt5 || true
            # Re-scan
            while IFS= read -r line; do
                [[ -n "$line" ]] && mt5_candidates+=("$line")
            done < <(_scan_mt5_prefixes)
        fi
        if [[ ${#mt5_candidates[@]} -eq 0 ]]; then
            if ! $AUTO_YES; then
                terminal_dir=$(_ask "Enter terminal_dir path manually (or press Enter to abort)")
            fi
            [[ -z "$terminal_dir" ]] && { _red "Cannot continue without terminal_dir."; exit 1; }
        fi
    fi
    if [[ -z "$terminal_dir" ]]; then
      if [[ ${#mt5_candidates[@]} -eq 1 ]]; then
        terminal_dir="${mt5_candidates[0]}"
        _ok "Found: $terminal_dir"
      else
        _ok "Found ${#mt5_candidates[@]} MT5 installations"
        # Auto-pick: highest activity score
        local best
        best=$(printf '%s\n' "${mt5_candidates[@]}" | _best_mt5_dir)
        if $AUTO_YES; then
            terminal_dir="$best"
            _ok "Auto-selected (most active): $terminal_dir"
        else
            # Show scores for each candidate
            echo ""
            local i
            for i in "${!mt5_candidates[@]}"; do
                local d="${mt5_candidates[$i]}"
                local score
                score=$(_score_mt5_dir "$d")
                printf '    [%d] %s  (%d files)\n' "$((i+1))" "$d" "$score" >&2
                [[ "$d" == "$best" ]] && printf '        ^ most active\n' >&2
            done
            terminal_dir=$(_pick "Select MT5 installation:" "${mt5_candidates[@]}")
        fi
      fi
    fi

    # ── Display mode ──────────────────────────────────────────────────────────
    local display_mode
    display_mode=$(_detect_display_mode)

    # ── Validate ──────────────────────────────────────────────────────────────
    if ! _validate "$wine" "$terminal_dir"; then
        _red "Validation failed. Config NOT written."
        if ! $AUTO_YES; then
            local ans
            ans=$(_ask "Write config anyway?" "no")
            [[ ! "$ans" =~ ^[Yy] ]] && exit 1
        else
            exit 1
        fi
    fi

    # ── Write YAML ────────────────────────────────────────────────────────────
    echo ""
    _bold "Writing config..."
    _write_yaml "$wine" "$terminal_dir" "$display_mode" "$KEEP_LAST"
    _ok "Written: $CONFIG_OUT"
    echo "  Tip: see config/example.set for optimization .set file format"

    # ── Register with all detected MCP platforms ─────────────────────────────
    _register_all_mcp_platforms

    echo ""
    _green "Setup complete!"
    echo ""
}

# ── MCP Platform Detection & Registration ───────────────────────────────────

# Detect available MCP platforms and return them as a list
detect_mcp_platforms() {
    local platforms=()

    # Claude Code
    if command -v claude &>/dev/null; then
        platforms+=("claude")
    fi

    # Windsurf (uses ~/.codeium/windsurf/mcp_config.json)
    if [[ -d "$HOME/.codeium/windsurf" ]] || [[ -d "$HOME/.windsurf" ]] || command -v windsurf &>/dev/null; then
        platforms+=("windsurf")
    fi

    # Cursor (uses ~/.cursor/mcp.json)
    if [[ -d "$HOME/.cursor" ]] || [[ -d "$HOME/Library/Application Support/Cursor" ]] || command -v cursor &>/dev/null; then
        platforms+=("cursor")
    fi

    # VS Code (uses .vscode/mcp.json or user profile)
    if [[ -d "$HOME/.vscode" ]] || [[ -d "$HOME/Library/Application Support/Code" ]] || command -v code &>/dev/null; then
        platforms+=("vscode")
    fi

    printf '%s\n' "${platforms[@]:-}"
}

# Check if mt5-quant is registered on a specific platform
_is_registered_on_platform() {
    local platform="$1"
    case "$platform" in
        claude)
            if command -v claude &>/dev/null; then
                local mcp_list
                mcp_list=$(claude mcp list 2>/dev/null || true)
                echo "$mcp_list" | grep -q "mt5-quant"
                return $?
            fi
            return 1
            ;;
        windsurf)
            local config_file="$HOME/.codeium/windsurf/mcp_config.json"
            [[ -f "$config_file" ]] && grep -q '"mt5-quant"' "$config_file" 2>/dev/null
            return $?
            ;;
        cursor)
            local config_file
            config_file="$HOME/.cursor/mcp.json"
            [[ -f "$config_file" ]] && grep -q "mt5-quant" "$config_file" 2>/dev/null
            return $?
            ;;
        vscode)
            # VS Code uses .vscode/mcp.json in workspace or user profile
            local workspace_config=".vscode/mcp.json"
            local user_config
            user_config="$HOME/.vscode/mcp.json"
            [[ -f "$workspace_config" ]] && grep -q '"mt5-quant"' "$workspace_config" 2>/dev/null && return 0
            [[ -f "$user_config" ]] && grep -q '"mt5-quant"' "$user_config" 2>/dev/null && return 0
            return 1
            ;;
    esac
    return 1
}

# Get registered path for a platform
_get_platform_mcp_path() {
    local platform="$1"
    case "$platform" in
        claude)
            if command -v claude &>/dev/null; then
                local mcp_list
                mcp_list=$(claude mcp list 2>/dev/null || true)
                echo "$mcp_list" | grep "mt5-quant" | head -1 | sed -E 's/.*--[[:space:]]*//' | tr -d ' '
            fi
            ;;
        windsurf)
            local config_file="$HOME/.codeium/windsurf/mcp_config.json"
            if [[ -f "$config_file" ]]; then
                grep -A3 '"mt5-quant"' "$config_file" 2>/dev/null | grep '"command"' | sed 's/.*"command":[[:space:]]*"\([^"]*\)".*/\1/'
            fi
            ;;
        cursor)
            local config_file="$HOME/.cursor/mcp.json"
            if [[ -f "$config_file" ]]; then
                grep -A3 '"mt5-quant"' "$config_file" 2>/dev/null | grep '"command"' | sed 's/.*"command":[[:space:]]*"\([^"]*\)".*/\1/'
            fi
            ;;
    esac
}

# Unregister from a specific platform
_unregister_from_platform() {
    local platform="$1"
    echo ""
    _bold "Unregistering from $platform..."

    case "$platform" in
        claude)
            if ! command -v claude &>/dev/null; then
                _warn "claude CLI not found"
                return 1
            fi
            local out
            out=$(claude mcp remove mt5-quant 2>&1) || true
            if echo "$out" | grep -qi "removed\|success\|deleted"; then
                _ok "Unregistered from Claude Code"
                return 0
            elif echo "$out" | grep -qi "not found\|does not exist"; then
                _ok "No existing registration on Claude Code"
                return 0
            else
                _warn "Unregister result: $out"
                return 1
            fi
            ;;
        windsurf)
            local config_file="$HOME/.codeium/windsurf/mcp_config.json"
            if [[ -f "$config_file" ]]; then
                # Remove mt5-quant from JSON using Python
                if command -v python3 &>/dev/null; then
                    python3 -c "
import json, sys
with open('$config_file') as f:
    data = json.load(f)
if 'mcpServers' in data and 'mt5-quant' in data['mcpServers']:
    del data['mcpServers']['mt5-quant']
    with open('$config_file', 'w') as f:
        json.dump(data, f, indent=2)
" && { _ok "Unregistered from Windsurf"; return 0; }
                fi
                _warn "Could not auto-unregister from Windsurf — edit $config_file manually"
                return 1
            fi
            _ok "No existing registration on Windsurf"
            return 0
            ;;
        cursor)
            local config_file="$HOME/.cursor/mcp.json"
            if [[ -f "$config_file" ]]; then
                # Remove mt5-quant from JSON using Python if available, or sed as fallback
                if command -v python3 &>/dev/null; then
                    python3 -c "
import json, sys
with open('$config_file') as f:
    data = json.load(f)
if 'mcpServers' in data and 'mt5-quant' in data['mcpServers']:
    del data['mcpServers']['mt5-quant']
    with open('$config_file', 'w') as f:
        json.dump(data, f, indent=2)
" && { _ok "Unregistered from Cursor"; return 0; }
                fi
                _warn "Could not auto-unregister from Cursor — edit $config_file manually"
                return 1
            fi
            _ok "No existing registration on Cursor"
            return 0
            ;;
        vscode)
            # VS Code uses 'servers' (not 'mcpServers') in mcp.json
            local workspace_config=".vscode/mcp.json"
            local user_config="$HOME/.vscode/mcp.json"
            local config_file=""
            
            # Determine which config file to use
            if [[ -f "$workspace_config" ]] && grep -q '"mt5-quant"' "$workspace_config" 2>/dev/null; then
                config_file="$workspace_config"
            elif [[ -f "$user_config" ]] && grep -q '"mt5-quant"' "$user_config" 2>/dev/null; then
                config_file="$user_config"
            fi
            
            if [[ -n "$config_file" ]]; then
                if command -v python3 &>/dev/null; then
                    python3 -c "
import json, sys
with open('$config_file') as f:
    data = json.load(f)
if 'servers' in data and 'mt5-quant' in data['servers']:
    del data['servers']['mt5-quant']
    with open('$config_file', 'w') as f:
        json.dump(data, f, indent=2)
" && { _ok "Unregistered from VS Code ($config_file)"; return 0; }
                fi
                _warn "Could not auto-unregister from VS Code — edit $config_file manually"
                return 1
            fi
            _ok "No existing registration on VS Code"
            return 0
            ;;
    esac
}

# Register with a specific platform
_register_with_platform() {
    local platform="$1"
    local use_binary="${2:-false}"

    # Determine command path (binary for Windsurf/Cursor, Python for Claude)
    local cmd_path
    if $use_binary && [[ -f "${REPO_DIR}/target/release/mt5-quant" ]]; then
        cmd_path="${REPO_DIR}/target/release/mt5-quant"
    elif [[ -f "${REPO_DIR}/mt5-quant" ]]; then
        cmd_path="${REPO_DIR}/mt5-quant"
    else
        cmd_path="python3 ${REPO_DIR}/server/main.py"
    fi

    case "$platform" in
        claude)
            local out
            out=$(claude mcp add mt5-quant -- $cmd_path 2>&1) || true
            if echo "$out" | grep -qi "already\|exists"; then
                _ok "Already registered on Claude Code"
            elif echo "$out" | grep -qi "error\|failed"; then
                _warn "Claude Code registration failed: $out"
                return 1
            else
                _ok "Registered on Claude Code"
            fi
            ;;
        windsurf)
            local config_file="$HOME/.codeium/windsurf/mcp_config.json"
            mkdir -p "$(dirname "$config_file")"

            # Create or update JSON config
            if command -v python3 &>/dev/null; then
                python3 -c "
import json, os
config_path = '$config_file'
data = {'mcpServers': {}}
if os.path.exists(config_path):
    try:
        with open(config_path) as f:
            data = json.load(f)
    except:
        pass
if 'mcpServers' not in data:
    data['mcpServers'] = {}

data['mcpServers']['mt5-quant'] = {
    'command': '$cmd_path'
}

with open(config_path, 'w') as f:
    json.dump(data, f, indent=2)
" && { _ok "Registered on Windsurf ($config_file)"; return 0; }
            else
                _warn "Python3 required for Windsurf registration"
                return 1
            fi
            ;;
        cursor)
            local config_file="$HOME/.cursor/mcp.json"
            mkdir -p "$(dirname "$config_file")"

            # Create or update JSON config
            if command -v python3 &>/dev/null; then
                python3 -c "
import json, os
config_path = '$config_file'
data = {'mcpServers': {}}
if os.path.exists(config_path):
    try:
        with open(config_path) as f:
            data = json.load(f)
    except:
        pass
if 'mcpServers' not in data:
    data['mcpServers'] = {}

data['mcpServers']['mt5-quant'] = {
    'command': '$cmd_path'
}

with open(config_path, 'w') as f:
    json.dump(data, f, indent=2)
" && { _ok "Registered on Cursor ($config_file)"; return 0; }
            else
                _warn "Python3 required for Cursor registration"
                return 1
            fi
            ;;
        vscode)
            # VS Code uses .vscode/mcp.json (workspace) or user profile
            # Format: { "servers": { "name": { "command": "...", "args": [...], "env": {...} } }
            local workspace_config=".vscode/mcp.json"
            mkdir -p ".vscode"
            
            if command -v python3 &>/dev/null; then
                python3 -c "
import json, os
config_path = '$workspace_config'
data = {'servers': {}}
if os.path.exists(config_path):
    try:
        with open(config_path) as f:
            data = json.load(f)
    except:
        pass
if 'servers' not in data:
    data['servers'] = {}

# For stdio servers, VS Code uses 'command' and optional 'args'
data['servers']['mt5-quant'] = {
    'command': '$cmd_path'
}

with open(config_path, 'w') as f:
    json.dump(data, f, indent=2)
" && { _ok "Registered on VS Code ($workspace_config)"; return 0; }
            else
                _warn "Python3 required for VS Code registration"
                return 1
            fi
            ;;
    esac
}

# Check for any existing registrations across all platforms
_check_any_existing_registration() {
    local platforms=()
    while IFS= read -r platform; do
        [[ -n "$platform" ]] && platforms+=("$platform")
    done < <(detect_mcp_platforms)

    local found=false
    for platform in "${platforms[@]}"; do
        if _is_registered_on_platform "$platform"; then
            found=true
            break
        fi
    done

    $found
}

# Unregister from all platforms
_unregister_all_platforms() {
    local platforms=()
    while IFS= read -r platform; do
        [[ -n "$platform" ]] && platforms+=("$platform")
    done < <(detect_mcp_platforms)

    for platform in "${platforms[@]}"; do
        _is_registered_on_platform "$platform" && _unregister_from_platform "$platform"
    done
}

# Main registration function - detects platforms and registers with all
_register_all_mcp_platforms() {
    echo ""
    _bold "Detecting MCP platforms..."

    local platforms=()
    while IFS= read -r platform; do
        [[ -n "$platform" ]] && platforms+=("$platform")
    done < <(detect_mcp_platforms)

    if [[ ${#platforms[@]} -eq 0 ]]; then
        _warn "No MCP platforms detected (Claude, Windsurf, Cursor, VS Code)"
        echo ""
        echo "  Manual registration required:"
        echo "  - Claude Code: claude mcp add mt5-quant -- python3 \"${REPO_DIR}/server/main.py\""
        echo "  - Windsurf:   Edit ~/.codeium/windsurf/mcp_config.json"
        echo "  - Cursor:     Edit ~/.cursor/mcp.json"
        echo "  - VS Code:    Edit .vscode/mcp.json (workspace) or use MCP: Add Server command"
        echo "  - Antigravity:Use Agent panel → MCP Servers → Manage → Edit configuration"
        echo ""
        return
    fi

    _ok "Found platforms: ${platforms[*]}"

    # Check for existing registrations
    local has_existing=false
    for platform in "${platforms[@]}"; do
        if _is_registered_on_platform "$platform"; then
            has_existing=true
            local old_path
            old_path=$(_get_platform_mcp_path "$platform")
            _yellow "Existing registration detected on $platform"
            [[ -n "$old_path" ]] && echo "  Current path: $old_path"
        fi
    done

    # Prompt for reinstall if any existing registrations found
    if $has_existing && ! $AUTO_YES; then
        echo ""
        local reinstall_ans
        reinstall_ans=$(_ask "Unregister existing installations and reinstall on all platforms?" "yes")
        if [[ ! "$reinstall_ans" =~ ^[Yy] ]]; then
            echo "  Keeping existing registrations."
            return
        fi
    fi

    # Unregister from all platforms first
    for platform in "${platforms[@]}"; do
        _is_registered_on_platform "$platform" && _unregister_from_platform "$platform"
    done

    # Register with all detected platforms
    echo ""
    _bold "Registering mt5-quant MCP..."

    for platform in "${platforms[@]}"; do
        # Use binary for Windsurf/Cursor, Python for Claude
        case "$platform" in
            windsurf|cursor)
                _register_with_platform "$platform" true
                ;;
            *)
                _register_with_platform "$platform" false
                ;;
        esac
    done

    echo ""
    _green "MCP registration complete!"
    echo ""
    echo "  Registered on: ${platforms[*]}"
    echo ""
    echo "  Config locations:"
    [[ " ${platforms[*]} " =~ " claude " ]] && echo "    Claude Code:   ~/.claude.json (managed via CLI)"
    [[ " ${platforms[*]} " =~ " windsurf " ]] && echo "    Windsurf:      ~/.codeium/windsurf/mcp_config.json"
    [[ " ${platforms[*]} " =~ " cursor " ]] && echo "    Cursor:        ~/.cursor/mcp.json"
    [[ " ${platforms[*]} " =~ " vscode " ]] && echo "    VS Code:       .vscode/mcp.json"
    echo ""
    echo "  Binary: ${REPO_DIR}/target/release/mt5-quant"
    echo "  Python: ${REPO_DIR}/server/main.py"
    echo ""
}

main
