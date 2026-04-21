---
description: Release workflow — bump version, build, tag, push, publish MCP
tags: [release, publish, workflow]
---

# MT5-Quant Release Workflow

One-command release: `bash scripts/release.sh [patch|minor|major|X.Y.Z]`

---

## 0. Pre-release checklist

- [ ] Features implemented and tested
- [ ] Docs updated (README.md, docs/MCP_TOOLS.md) with correct tool count
- [ ] `cargo check` passes

Check tool count:

// turbo
```bash
grep -r "pub fn tool_" src/tools/definitions/ | wc -l
```

---

## 1. Run the release script

// turbo
```bash
bash scripts/release.sh patch
```

Accepts: `patch` · `minor` · `major` · `1.32.0` · `v1.32.0`

The script handles:
- Version bump in `Cargo.toml`
- `server.json` version + download URL update
- `CHANGELOG.md` entry generation from git log
- `cargo check` verification
- Release commit + annotated git tag
- `git push` → triggers GitHub Actions

---

## 2. Monitor CI

// turbo
```bash
gh run list --limit 3
```

CI pipeline stages:
1. **build-macos** / **build-linux** — parallel Rust builds with cache
2. **release** — creates GitHub Release with both binaries
3. **build-mcp-package** — packages binary + config + docs, computes SHA256
4. **update-server-json** — commits real SHA256 back to `server.json` on master
5. **mcp-publish** — publishes to MCP Registry via GitHub OIDC
6. **release-info** — prints registration commands for all MCP clients

---

## 3. Install locally

// turbo
```bash
cp target/release/mt5-quant ~/.local/bin/mt5-quant
```

---

## 4. Verify

// turbo
```bash
claude mcp list
```

---

## Manual MCP publish (if CI publish fails)

// turbo
```bash
# Get latest mcp-publisher
LATEST=$(curl -sf https://api.github.com/repos/modelcontextprotocol/registry/releases/latest | python3 -c "import sys,json; print(json.load(sys.stdin)['tag_name'])")
VERSION="${LATEST#v}"
curl -fsSL -o pub.tar.gz "https://github.com/modelcontextprotocol/registry/releases/download/${LATEST}/mcp-publisher_${VERSION}_$(uname -s | tr '[:upper:]' '[:lower:]')_amd64.tar.gz"
tar -xzf pub.tar.gz && chmod +x mcp-publisher
./mcp-publisher login github
./mcp-publisher publish
```

---

## Re-trigger CI without a new tag

// turbo
```bash
gh workflow run release.yml -f version=v1.32.0
```

---

## Rollback

```bash
# Remove tag locally and remotely
git tag -d v1.32.0
git push origin :refs/tags/v1.32.0
# Delete GitHub Release
gh release delete v1.32.0 --yes
```
