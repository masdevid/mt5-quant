#!/bin/bash
# Build MT5-Quant as a single executable using PyInstaller
# Output: dist/mt5-quant

set -e

# Get script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "$PROJECT_ROOT"

echo "=== MT5-Quant PyInstaller Build ==="
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
$PYTHON -m pip install pyinstaller mcp pyyaml --quiet

# Clean previous builds
echo "Cleaning previous builds..."
rm -rf "$PROJECT_ROOT/build" "$PROJECT_ROOT/dist"

# Build the executable
echo "Building executable (this may take 1-2 minutes)..."
cd "$PROJECT_ROOT"
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
ls -lh dist/mt5-quant

echo ""
echo "Size breakdown:"
du -sh dist/ 2>/dev/null || echo "  dist/: $(du -sh dist/mt5-quant 2>/dev/null | cut -f1)"

echo ""
echo "To test:"
echo "  ./dist/mt5-quant --help"
echo ""
echo "To register with Claude Code:"
echo "  claude mcp add MT5-Quant -- $(pwd)/dist/mt5-quant"
echo ""
echo "=== Deployment to Multiple Machines ==="
echo ""
echo "1. Copy this single binary to target machines:"
echo "     scp dist/mt5-quant user@server:/usr/local/bin/"
echo "     ssh user@server chmod +x /usr/local/bin/mt5-quant"
echo ""
echo "2. Copy config directory (includes mt5-quant.yaml):"
echo "     scp -r config/ user@server:~/.config/mt5-quant/"
echo ""
echo "3. Or use bundled config (minimal):"
echo "     MT5_MCP_HOME=/path/to/config ./mt5-quant"
echo ""
echo "4. Register on target machine:"
echo "     claude mcp add MT5-Quant -- /usr/local/bin/mt5-quant"
echo ""
echo "Requirements on target machine:"
echo "  - MetaTrader 5 installed (via Wine on macOS/Linux)"
echo "  - Config file at ~/.config/mt5-quant/config/mt5-quant.yaml"
echo "  - NO Python installation required!"
