#!/bin/bash
# Build MT5-Quant as a directory bundle (onedir mode) using PyInstaller
# This mode preserves stdin/stdout better for MCP communication
# Output: dist/mt5-quant/ directory

set -e

# Get script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "$PROJECT_ROOT"

echo "=== MT5-Quant PyInstaller Build (ONEDIR Mode) ==="
echo "Project root: $PROJECT_ROOT"
echo ""

# Determine Python interpreter
if [ -f "$PROJECT_ROOT/.venv/bin/python3" ]; then
    PYTHON="$PROJECT_ROOT/.venv/bin/python3"
elif [ -f "$PROJECT_ROOT/.venv/bin/python" ]; then
    PYTHON="$PROJECT_ROOT/.venv/bin/python"
else
    PYTHON="python3"
fi

echo "Using Python: $PYTHON"
echo ""

# Ensure dependencies are installed
echo "Installing dependencies..."
$PYTHON -m pip install pyinstaller mcp pyyaml typer --quiet 2>/dev/null || true

# Clean previous builds
echo "Cleaning previous builds..."
rm -rf "$PROJECT_ROOT/build" "$PROJECT_ROOT/dist"

# Build the executable in onedir mode
echo "Building executable in ONEDIR mode (this may take 1-2 minutes)..."
cd "$PROJECT_ROOT"

# Use onedir mode (no --onefile flag)
# Note: mode is controlled by the .spec file
$PYTHON -m PyInstaller \
    --clean \
    --noconfirm \
    --distpath "$PROJECT_ROOT/dist" \
    --workpath "$PROJECT_ROOT/build" \
    "$PROJECT_ROOT/mt5-quant.spec"

# Report results
echo ""
echo "=== Build Complete ==="
echo ""
echo "Executable location:"
ls -la "$PROJECT_ROOT/dist/mt5-quant/" | head -10

echo ""
echo "Main executable:"
ls -lh "$PROJECT_ROOT/dist/mt5-quant/mt5-quant"

echo ""
echo "Total size:"
du -sh "$PROJECT_ROOT/dist/mt5-quant/"

echo ""
echo "To test:"
echo "  ./dist/mt5-quant/mt5-quant"
echo ""
echo "To register with Claude Code:"
echo "  claude mcp add MT5-Quant -- $(pwd)/dist/mt5-quant/mt5-quant"
echo ""
echo "=== Deployment to Multiple Machines ==="
echo ""
echo "1. Copy entire directory to target machines:"
echo "     scp -r dist/mt5-quant/ user@server:/opt/"
echo "     ssh user@server ln -s /opt/mt5-quant/mt5-quant /usr/local/bin/"
echo ""
echo "2. Or create tarball for distribution:"
echo "     tar -czf mt5-quant-macos-arm64.tar.gz -C dist mt5-quant"
echo ""
echo "Requirements on target machine:"
echo "  - MetaTrader 5 installed (via Wine on macOS/Linux)"
echo "  - Config file at ~/.config/mt5-quant/config/mt5-quant.yaml"
echo "  - NO Python installation required!"
