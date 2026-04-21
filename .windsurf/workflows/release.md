---
description: Release workflow - bump version, build, and publish MCP server
tags: [release, publish, workflow]
---

# MT5-Quant Release Workflow

Steps to release a new version of the MCP server.

## 1. Pre-Release Checklist

- [ ] All features implemented and tested
- [ ] Documentation updated (README.md, MCP_TOOLS.md)
- [ ] Tool count verified: `grep -r "pub fn tool_" src/tools/definitions/ | wc -l`
- [ ] Version bumped in Cargo.toml
- [ ] Build passes: `cargo build --release`

## 2. Update Documentation

// turbo
```bash
grep -r "pub fn tool_" src/tools/definitions/ | wc -l
```

Update these files with correct tool counts:
- `README.md` - header and "MCP Tools (N)" section
- `docs/MCP_TOOLS.md` - "documents X of Y total tools" line

## 3. Bump Version

Edit `Cargo.toml` and update the version:
```toml
version = "X.Y.Z"
```

## 4. Build Release

// turbo
```bash
bash scripts/build-release.sh
```

This creates: `dist/mcp-mt5-quant-{platform}.tar.gz`

Calculate SHA256 for server.json:
```bash
shasum -a 256 dist/mcp-mt5-quant-macos-arm64.tar.gz
```

## 5. Update MCP Server Configuration

Update both `server.json` files with:
- New version
- New SHA256 hash from step 4
- Updated download URL
- Tool count in description

Files to update:
- `server.json`
- `mcp-package/server.json`

Then copy the release package:
```bash
cp dist/mcp-mt5-quant-macos-arm64.tar.gz mcp-package/
```

## 6. Create Git Tag & Commit

```bash
git add .
git commit -m "Release vX.Y.Z - brief description"
git tag vX.Y.Z
git push origin vX.Y.Z
```

## 7. Create GitHub Release (Manual)

1. Go to GitHub → Releases → Draft new release
2. Choose tag: `vX.Y.Z`
3. Release title: `vX.Y.Z`
4. Attach binary: `dist/mcp-mt5-quant-macos-arm64.tar.gz`
5. Publish release

## 8. Install Binary to System Path

Install the binary to a single location for all MCP clients:

```bash
# Create local bin if needed
mkdir -p ~/.local/bin

# Install binary
cp target/release/mt5-quant ~/.local/bin/

# Or system-wide (requires sudo)
# sudo cp target/release/mt5-quant /usr/local/bin/
```

**Installation Path:** `~/.local/bin/mt5-quant` (single location for all clients)

## 9. MCP Registration

Use the installed binary path (not project directory):

### Claude Code
```bash
claude mcp add mt5-quant -- ~/.local/bin/mt5-quant
```

### Windsurf
Edit `~/.codeium/windsurf/mcp_config.json`:
```json
{
  "mcpServers": {
    "mt5-quant": {
      "command": "~/.local/bin/mt5-quant"
    }
  }
}
```

### VS Code / Cursor
Edit `~/.cursor/mcp.json` or `.vscode/mcp.json`:
```json
{
  "mcpServers": {
    "mt5-quant": {
      "command": "~/.local/bin/mt5-quant"
    }
  }
}
```

## 10. MCP Client Registration

After installing the binary, register with your MCP clients:

### Windsurf

// turbo
```bash
# Add to Windsurf MCP config
cat >> ~/.codeium/windsurf/mcp_config.json << 'EOF'
{
  "mcpServers": {
    "io.github.masdevid/mt5-quant": {
      "command": "~/.local/bin/mt5-quant",
      "disabled": false,
      "registry": "io.github.masdevid/mt5-quant"
    }
  }
}
EOF
```

### Claude Code

// turbo
```bash
# Register with Claude Code
claude mcp add io.github.masdevid/mt5-quant -- ~/.local/bin/mt5-quant

# Or with custom name
claude mcp add mt5-quant -- ~/.local/bin/mt5-quant
```

### VS Code / Cursor

// turbo
```bash
# Add to Cursor MCP config
mkdir -p ~/.cursor
cat > ~/.cursor/mcp.json << 'EOF'
{
  "mcpServers": {
    "mt5-quant": {
      "command": "~/.local/bin/mt5-quant",
      "env": {
        "MT5_MCP_HOME": "~/.config/mt5-quant"
      }
    }
  }
}
EOF
```

## 11. Post-Release Verification

// turbo
```bash
# Test the binary
./target/release/mt5-quant --help

# Verify tool count
./target/release/mt5-quant 2>&1 | head -20

# Check MCP registration
claude mcp list
```

## Quick Release Commands

```bash
# Full release cycle
vim Cargo.toml  # bump version
bash scripts/build-release.sh
git add . && git commit -m "Release vX.Y.Z"
git tag vX.Y.Z && git push origin vX.Y.Z
```
