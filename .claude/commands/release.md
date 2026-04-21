---
description: Release a new version of mt5-quant — bump, changelog, tag, push
---

# /release — MT5-Quant Release Workflow

Automates the full release cycle: version bump → changelog → git tag → GitHub push → CI triggers build + MCP publish.

## Usage

```
/release [patch|minor|major|X.Y.Z]
```

Default is `patch` if no argument is given.

---

## Step 1 — Check current version and state

```bash
grep '^version' Cargo.toml | head -1
git status --short
git log --oneline -5
```

## Step 2 — Count tools (update docs if needed)

```bash
grep -r "pub fn tool_" src/tools/definitions/ | wc -l
```

If the count changed since the last release, update:
- `README.md` — "MCP Tools (N)" section header and description
- `docs/MCP_TOOLS.md` — "documents X of Y total tools" line

## Step 3 — Run the release script

```bash
bash scripts/release.sh patch
```

Replace `patch` with `minor`, `major`, or an explicit version like `1.33.0`.

The script will:
1. Bump `Cargo.toml` version
2. Update `server.json` version + download URL (SHA256 = TBD, set by CI)
3. Generate a `CHANGELOG.md` entry from `git log` since the last tag
4. Run `cargo check` to verify the build
5. Commit all changes
6. Create an annotated git tag
7. Push branch + tag → triggers GitHub Actions

## Step 4 — Monitor GitHub Actions

```bash
gh run list --limit 5
```

Or open: https://github.com/masdevid/mt5-quant/actions

The CI pipeline will:
- Build macOS arm64 + Linux x64 binaries
- Create the GitHub Release with both artifacts
- Build the MCP installable package
- Compute the real SHA256 and commit it back to `server.json`
- Attempt to publish to the MCP Registry

## Step 5 — Install updated binary locally

```bash
cp target/release/mt5-quant ~/.local/bin/mt5-quant
```

Or rebuild from the release tag:

```bash
cargo build --release && cp target/release/mt5-quant ~/.local/bin/mt5-quant
```

## Step 6 — Verify MCP registration

```bash
claude mcp list
~/.local/bin/mt5-quant --help 2>/dev/null || echo "binary works"
```

---

## Troubleshooting

**MCP publish failed in CI?**

```bash
# Download latest mcp-publisher
curl -L "https://github.com/modelcontextprotocol/registry/releases/latest/download/mcp-publisher_linux_amd64.tar.gz" | tar xz
./mcp-publisher login github
./mcp-publisher publish
```

**Need to re-run CI without a new tag?**

```bash
gh workflow run release.yml -f version=v1.32.0
```

**Rollback a bad release?**

```bash
git tag -d v1.32.0
git push origin :refs/tags/v1.32.0
# Then delete the GitHub Release via web UI or:
gh release delete v1.32.0
```
