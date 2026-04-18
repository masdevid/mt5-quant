#!/bin/bash
# Build release binaries for distribution
# Creates: dist/mt5-quant-{platform}.tar.gz

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "$PROJECT_ROOT"

VERSION=$(grep -E '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo "=== Building MCP-MT5-Quant v${VERSION} ==="
echo ""

# Clean previous builds
rm -rf "$PROJECT_ROOT/dist"
mkdir -p "$PROJECT_ROOT/dist"

# Build current platform
echo "Building for current platform..."
RUSTFLAGS="-D warnings" cargo build --release

# Detect platform
UNAME=$(uname -s)
ARCH=$(uname -m)

if [[ "$UNAME" == "Darwin" ]]; then
    PLATFORM="macos-${ARCH}"
elif [[ "$UNAME" == "Linux" ]]; then
    PLATFORM="linux-${ARCH}"
else
    PLATFORM="unknown"
fi

PACKAGE_NAME="mcp-mt5-quant-${PLATFORM}"
PACKAGE_DIR="$PROJECT_ROOT/dist/${PACKAGE_NAME}"

echo "Packaging for ${PLATFORM}..."
mkdir -p "$PACKAGE_DIR"

# Copy binary
cp "$PROJECT_ROOT/target/release/mt5-quant" "$PACKAGE_DIR/"

# Copy config template
mkdir -p "$PACKAGE_DIR/config"
cp "$PROJECT_ROOT/config/mt5-quant.example.yaml" "$PACKAGE_DIR/config/"

# Copy docs
cp "$PROJECT_ROOT/README.md" "$PACKAGE_DIR/"
cp "$PROJECT_ROOT/docs/WINDSURF.md" "$PACKAGE_DIR/WINDSURF_SETUP.md"
cp "$PROJECT_ROOT/CLAUDE.md" "$PACKAGE_DIR/"

# Create tarball
cd "$PROJECT_ROOT/dist"
tar -czf "${PACKAGE_NAME}.tar.gz" "$PACKAGE_NAME"

echo ""
echo "=== Build Complete ==="
echo ""
echo "Package: dist/${PACKAGE_NAME}.tar.gz"
echo "Binary: mt5-quant"
echo "Size: $(du -h "${PACKAGE_NAME}.tar.gz" | cut -f1)"
echo ""
echo "Contents:"
tar -tzf "${PACKAGE_NAME}.tar.gz" | head -10
echo ""
echo "To install:"
echo "  tar -xzf ${PACKAGE_NAME}.tar.gz"
echo "  cd ${PACKAGE_NAME}"
echo "  sudo cp mt5-quant /usr/local/bin/"
echo "  mt5-quant --help"
