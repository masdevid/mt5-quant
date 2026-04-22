#!/usr/bin/env bash
# Build release binary and package it for distribution
# Creates: dist/mcp-mt5-quant-{platform}.tar.gz
#
# Usage:
#   bash scripts/build-release.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "$PROJECT_ROOT"

VERSION=$(grep -E '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo "=== Building mt5-quant v${VERSION} ==="
echo ""

# Clean previous builds
rm -rf "$PROJECT_ROOT/dist"
mkdir -p "$PROJECT_ROOT/dist"

# Build release binary
echo "Building release binary..."
RUSTFLAGS="-D warnings" cargo build --release
echo ""

# Detect platform
UNAME=$(uname -s)
ARCH=$(uname -m)

if [[ "$UNAME" == "Darwin" ]]; then
    PLATFORM="macos-${ARCH}"
elif [[ "$UNAME" == "Linux" ]]; then
    PLATFORM="linux-${ARCH}"
else
    PLATFORM="unknown-${ARCH}"
fi

PACKAGE_NAME="mcp-mt5-quant-${PLATFORM}"
PACKAGE_DIR="$PROJECT_ROOT/dist/${PACKAGE_NAME}"

echo "Packaging for ${PLATFORM}..."
mkdir -p "$PACKAGE_DIR/docs"

# Binary
cp "$PROJECT_ROOT/target/release/mt5-quant" "$PACKAGE_DIR/"
chmod +x "$PACKAGE_DIR/mt5-quant"

# Config template
mkdir -p "$PACKAGE_DIR/config"
cp "$PROJECT_ROOT/config/mt5-quant.example.yaml" "$PACKAGE_DIR/config/"

# Documentation
cp "$PROJECT_ROOT/README.md"            "$PACKAGE_DIR/"
cp "$PROJECT_ROOT/docs/QUICKSTART.md"   "$PACKAGE_DIR/docs/"
cp "$PROJECT_ROOT/docs/CLAUDE.md"       "$PACKAGE_DIR/docs/"
cp "$PROJECT_ROOT/docs/CURSOR.md"       "$PACKAGE_DIR/docs/"
cp "$PROJECT_ROOT/docs/VSCODE.md"       "$PACKAGE_DIR/docs/"
cp "$PROJECT_ROOT/docs/WINDSURF.md"     "$PACKAGE_DIR/docs/"

# Create tarball
cd "$PROJECT_ROOT/dist"
tar -czf "${PACKAGE_NAME}.tar.gz" "$PACKAGE_NAME"
rm -rf "$PACKAGE_NAME"

echo ""
echo "=== Build complete ==="
echo ""
echo "Package : dist/${PACKAGE_NAME}.tar.gz"
echo "Size    : $(du -h "${PACKAGE_NAME}.tar.gz" | cut -f1)"
echo ""
echo "Contents:"
tar -tzf "${PACKAGE_NAME}.tar.gz"
echo ""
echo "Install:"
echo "  tar -xzf ${PACKAGE_NAME}.tar.gz"
echo "  sudo cp ${PACKAGE_NAME}/mt5-quant /usr/local/bin/"
echo "  mt5-quant --help"
