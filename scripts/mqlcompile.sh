#!/usr/bin/env bash
# mqlcompile.sh — Compile an MQL5 Expert Advisor via MetaEditor (Wine/CrossOver)
#
# Usage:
#   ./scripts/mqlcompile.sh <path/to/Expert.mq5>
#
# Output:
#   Compiled .ex5 written to MT5_EXPERTS_DIR
#   Exit 0 on success, 1 on compile errors

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/platform_detect.sh"

# ── Args ──────────────────────────────────────────────────────────────────────
SOURCE_FILE="${1:-}"
if [[ -z "$SOURCE_FILE" ]]; then
    echo "Usage: $0 <path/to/Expert.mq5>" >&2
    exit 1
fi

if [[ ! -f "$SOURCE_FILE" ]]; then
    echo "ERROR: Source file not found: $SOURCE_FILE" >&2
    exit 1
fi

SOURCE_FILE="$(realpath "$SOURCE_FILE")"
EXPERT_NAME="$(basename "$SOURCE_FILE" .mq5)"

# ── Resolve platform ──────────────────────────────────────────────────────────
resolve_platform

METAEDITOR="${MT5_DIR}/metaeditor64.exe"
if [[ ! -f "$METAEDITOR" ]]; then
    echo "ERROR: metaeditor64.exe not found at: $METAEDITOR" >&2
    exit 1
fi

# ── Copy source to MT5 Experts source dir ────────────────────────────────────
# MetaEditor requires the source file to be inside the MT5 directory tree
MT5_SRC_DIR="${MT5_DIR}/MQL5/Experts"
mkdir -p "$MT5_SRC_DIR"
cp "$SOURCE_FILE" "${MT5_SRC_DIR}/${EXPERT_NAME}.mq5"

WINE_SRC_PATH="$(host_to_wine_path "${MT5_SRC_DIR}/${EXPERT_NAME}.mq5")"

# ── Sync .mqh include files to MT5 Include dir ───────────────────────────────
# Auto-detect include/ directory relative to source file. Searches up to 2
# levels above the source file for an include/ sibling directory.
# Layout supported:
#   <project>/experts/EA.mq5  +  <project>/include/<subdir>/*.mqh
#   <project>/src/experts/EA.mq5  +  <project>/src/include/<subdir>/*.mqh
_find_include_dir() {
    local source_dir="$1"
    local candidate
    for candidate in "$source_dir" "$(dirname "$source_dir")" "$(dirname "$(dirname "$source_dir")")"; do
        if [[ -d "${candidate}/include" ]]; then
            echo "${candidate}/include"
            return 0
        fi
    done
    return 1
}

INCLUDE_BASE=""
INCLUDE_BASE=$(_find_include_dir "$(dirname "$SOURCE_FILE")") || true

if [[ -n "$INCLUDE_BASE" ]]; then
    synced_total=0
    # Sync each subdirectory under include/ → MQL5/Include/<subdir>/
    while IFS= read -r -d '' subdir; do
        dir_name="$(basename "$subdir")"
        mt5_dest="${MT5_DIR}/MQL5/Include/${dir_name}"
        rm -rf "$mt5_dest"
        cp -r "$subdir" "$mt5_dest"
        count=$(find "$mt5_dest" -name "*.mqh" | wc -l | tr -d ' ')
        echo "[compile] Synced ${count} .mqh → Include/${dir_name}/"
        synced_total=$((synced_total + count))
    done < <(find "$INCLUDE_BASE" -mindepth 1 -maxdepth 1 -type d -print0 2>/dev/null)

    # Also sync any .mqh files directly in include/ (flat layout)
    while IFS= read -r -d '' mqh; do
        cp "$mqh" "${MT5_DIR}/MQL5/Include/"
        synced_total=$((synced_total + 1))
    done < <(find "$INCLUDE_BASE" -maxdepth 1 -name "*.mqh" -print0 2>/dev/null)

    if [[ $synced_total -eq 0 ]]; then
        echo "[compile] INFO: include/ found but contains no .mqh files — skipping sync"
    else
        echo "[compile] Synced ${synced_total} .mqh file(s) total"
    fi
else
    echo "[compile] INFO: No include/ directory found — skipping .mqh sync"
fi

# ── Set Wine prefix ───────────────────────────────────────────────────────────
WINE_PREFIX_DIR=$(dirname "$(dirname "$(dirname "$MT5_DIR")")")
export WINEPREFIX="$WINE_PREFIX_DIR"
export WINEDEBUG="-all"

# ── Run MetaEditor ────────────────────────────────────────────────────────────
echo "[compile] Compiling ${EXPERT_NAME}.mq5 ..."
LOG_FILE="$(mktemp /tmp/mqlcompile_XXXXXX.log)"

set +e
${MT5_ARCH} "${MT5_WINE}" "${METAEDITOR}" \
    /compile:"${WINE_SRC_PATH}" \
    /log:"${LOG_FILE}" \
    2>/dev/null
WINE_EXIT=$?
set -e

# MetaEditor always exits 0 on macOS/Wine; check log for errors
ERRORS=0
WARNINGS=0
if [[ -f "$LOG_FILE" ]]; then
    # Log may be UTF-16LE
    LOG_TEXT=$(iconv -f UTF-16LE -t UTF-8 "$LOG_FILE" 2>/dev/null || cat "$LOG_FILE")
    ERRORS=$(echo "$LOG_TEXT" | grep -cE "^.*error" || true)
    WARNINGS=$(echo "$LOG_TEXT" | grep -cE "^.*warning" || true)

    if [[ $ERRORS -gt 0 ]]; then
        echo "[compile] FAILED: $ERRORS error(s), $WARNINGS warning(s)"
        echo "$LOG_TEXT" | grep -E "error|warning" | head -20
        rm -f "$LOG_FILE"
        exit 1
    fi
fi

# ── Verify .ex5 was produced ──────────────────────────────────────────────────
EX5_PATH="${MT5_SRC_DIR}/${EXPERT_NAME}.ex5"
if [[ ! -f "$EX5_PATH" ]]; then
    echo "[compile] ERROR: .ex5 not produced. MetaEditor may have failed silently." >&2
    [[ -f "$LOG_FILE" ]] && cat "$LOG_FILE"
    exit 1
fi

BINARY_SIZE=$(stat -f%z "$EX5_PATH" 2>/dev/null || stat -c%s "$EX5_PATH")
echo "[compile] OK: ${EXPERT_NAME}.ex5 (${BINARY_SIZE} bytes, ${WARNINGS} warning(s))"

rm -f "$LOG_FILE"
exit 0
