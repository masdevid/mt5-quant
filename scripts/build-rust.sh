#!/bin/bash
# Build MT5-Quant Rust MCP Server
# Output: target/release/mt5-quant

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "$PROJECT_ROOT"

echo "=== MT5-Quant Rust Build ==="
echo "Project root: $PROJECT_ROOT"
echo ""

echo "Building release binary..."
cargo build --release

echo ""
echo "=== Build Complete ==="
echo ""
echo "Executable location:"
ls -lh "$PROJECT_ROOT/target/release/mt5-quant"

echo ""
echo "To test:"
echo "  ./target/release/mt5-quant --help"
echo ""
echo "To install for Windsurf:"
echo "  Update ~/.windsurf/config.yaml:"
echo "    command: $PROJECT_ROOT/target/release/mt5-quant"
echo ""
