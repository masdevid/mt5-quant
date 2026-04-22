#!/usr/bin/env bash
# scripts/release.sh — Automate version bump, changelog, git tag, and push
#
# Usage:
#   bash scripts/release.sh patch          # 1.31.0 → 1.31.1
#   bash scripts/release.sh minor          # 1.31.0 → 1.32.0
#   bash scripts/release.sh major          # 1.31.0 → 2.0.0
#   bash scripts/release.sh 1.32.0         # explicit version
#   bash scripts/release.sh v1.32.0        # with v prefix
#   bash scripts/release.sh patch --yes    # non-interactive (CI / Claude Code)

set -euo pipefail
cd "$(dirname "$0")/.."

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; BOLD='\033[1m'; NC='\033[0m'
info()  { echo -e "${BLUE}▶  $*${NC}"; }
ok()    { echo -e "${GREEN}✓  $*${NC}"; }
warn()  { echo -e "${YELLOW}⚠  $*${NC}"; }
die()   { echo -e "${RED}✗  $*${NC}" >&2; exit 1; }
hr()    { echo -e "${BLUE}────────────────────────────────────────${NC}"; }

# ── Argument parsing ───────────────────────────────────────────────────────────

bump="${1:-patch}"
AUTO_YES=false
if [[ "${2:-}" == "--yes" || "${2:-}" == "-y" || "${CI:-}" == "true" ]]; then
  AUTO_YES=true
fi

# ── Prerequisites ──────────────────────────────────────────────────────────────

command -v cargo   >/dev/null 2>&1 || die "cargo not found"
command -v python3 >/dev/null 2>&1 || die "python3 not found"

# ── Current version ────────────────────────────────────────────────────────────

current=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
[[ -n "$current" ]] || die "Could not parse version from Cargo.toml"
info "Current version: ${BOLD}$current${NC}"

# ── Compute new version ────────────────────────────────────────────────────────

IFS='.' read -r major minor patch_v <<< "$current"

case "$bump" in
  major)  new="$((major + 1)).0.0" ;;
  minor)  new="${major}.$((minor + 1)).0" ;;
  patch)  new="${major}.${minor}.$((patch_v + 1))" ;;
  v*.*.*) new="${bump#v}" ;;
  *.*.*)  new="$bump" ;;
  *)      die "Usage: $0 [patch|minor|major|X.Y.Z] [--yes]" ;;
esac

[[ "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "Invalid version: $new"

hr
info "Releasing: ${BOLD}$current → $new${NC}"
hr

# ── Confirm ────────────────────────────────────────────────────────────────────

if [[ "$AUTO_YES" == false ]]; then
  read -rp "Proceed with release v${new}? [y/N] " confirm
  [[ "$confirm" =~ ^[Yy]$ ]] || die "Aborted"
else
  info "Auto-confirmed (--yes)"
fi

# ── Check git state ────────────────────────────────────────────────────────────

info "Checking git state..."
if ! git diff --quiet 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
  die "Working tree has uncommitted changes — commit or stash first"
fi
current_branch=$(git rev-parse --abbrev-ref HEAD)
info "Branch: $current_branch"

if git rev-parse "v${new}" >/dev/null 2>&1; then
  die "Tag v${new} already exists"
fi
ok "Git state clean"

# ── 1. Bump Cargo.toml ─────────────────────────────────────────────────────────

info "Bumping Cargo.toml..."
if [[ "$(uname)" == "Darwin" ]]; then
  sed -i '' "s/^version = \"${current}\"/version = \"${new}\"/" Cargo.toml
else
  sed -i "s/^version = \"${current}\"/version = \"${new}\"/" Cargo.toml
fi
cargo metadata --no-deps --format-version 1 >/dev/null 2>&1 || true
ok "Cargo.toml → $new"

# ── 2. Update server.json ──────────────────────────────────────────────────────

info "Updating server.json..."
NEW_VERSION="$new" python3 - <<'PYEOF'
import json, os, re

version = os.environ['NEW_VERSION']

with open('server.json') as f:
    data = json.load(f)

data['version'] = version

for pkg in data.get('packages', []):
    pkg['version'] = version
    if 'identifier' in pkg:
        pkg['identifier'] = re.sub(
            r'/v[0-9]+\.[0-9]+\.[0-9]+/',
            f'/v{version}/',
            pkg['identifier']
        )
    # SHA256 is computed by CI after building — placeholder signals this
    pkg['fileSha256'] = 'TBD_CI_WILL_UPDATE'

with open('server.json', 'w') as f:
    json.dump(data, f, indent=2)
    f.write('\n')

print(f"  server.json version={version}, identifier URL updated")
PYEOF
ok "server.json → $new (SHA256 set by CI)"

# ── 3. Generate CHANGELOG entry ────────────────────────────────────────────────

info "Generating CHANGELOG entry..."
prev_tag=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
today=$(date +%Y-%m-%d)

{
  echo "## [$new] — $today"
  echo ""
  if [[ -n "$prev_tag" ]]; then
    git log "${prev_tag}..HEAD" --pretty=format:"- %s" --no-merges \
      | grep -v "^- ci:" \
      | grep -v "^- chore:" \
      | grep -v "^$" \
      || echo "- Minor improvements and bug fixes"
  else
    echo "- Initial public release"
  fi
  echo ""
} > /tmp/release_entry.md

if [[ -f CHANGELOG.md ]]; then
  tmp=$(mktemp)
  head -1 CHANGELOG.md > "$tmp"
  echo "" >> "$tmp"
  cat /tmp/release_entry.md >> "$tmp"
  tail -n +2 CHANGELOG.md >> "$tmp"
  mv "$tmp" CHANGELOG.md
else
  { echo "# Changelog"; echo ""; cat /tmp/release_entry.md; } > CHANGELOG.md
fi
rm -f /tmp/release_entry.md
ok "CHANGELOG.md updated"

# ── 4. Verify build ────────────────────────────────────────────────────────────

info "Verifying build (cargo check)..."
cargo check --quiet 2>&1 || die "cargo check failed — fix errors before releasing"
ok "Build check passed"

# ── 5. Commit ──────────────────────────────────────────────────────────────────

info "Creating release commit..."
git add Cargo.toml Cargo.lock server.json CHANGELOG.md
git commit -m "release: v${new}

- Version bump ${current} → ${new}
- server.json identifier URL updated (SHA256 set by CI after build)
- CHANGELOG.md updated"
ok "Release commit created"

# ── 6. Tag ─────────────────────────────────────────────────────────────────────

info "Creating annotated tag v${new}..."
git tag -a "v${new}" -m "Release v${new}"
ok "Tagged v${new}"

# ── 7. Push (rebase if remote has new commits from CI) ─────────────────────────

info "Pushing to GitHub..."
if ! git push origin "$current_branch" 2>/dev/null; then
  warn "Push rejected — rebasing on remote (CI may have committed SHA256)..."
  git fetch origin "$current_branch"

  # Resolve server.json conflict automatically: keep our version
  if ! git rebase "origin/${current_branch}"; then
    if git diff --name-only --diff-filter=U | grep -q "server.json"; then
      python3 - <<'PYEOF'
import re
with open('server.json') as f:
    raw = f.read()
resolved = re.sub(
    r'<<<<<<< HEAD.*?=======\n(.*?)>>>>>>> [^\n]+\n',
    r'\1',
    raw,
    flags=re.DOTALL
)
with open('server.json', 'w') as f:
    f.write(resolved)
PYEOF
      git add server.json
      GIT_EDITOR=true git rebase --continue
    else
      git rebase --abort
      die "Rebase failed with unexpected conflicts — push manually"
    fi
  fi

  git push origin "$current_branch"
fi

# Push tag (delete and re-push if rebase moved the commit)
git push origin "v${new}" 2>/dev/null || {
  git push origin ":refs/tags/v${new}" 2>/dev/null || true
  git tag -f -a "v${new}" -m "Release v${new}"
  git push origin "v${new}"
}

ok "Pushed — GitHub Actions triggered"

# ── Done ───────────────────────────────────────────────────────────────────────

hr
echo ""
echo -e "${GREEN}${BOLD}  Release v${new} kicked off!${NC}"
echo ""
echo -e "  Actions:  https://github.com/masdevid/mt5-quant/actions"
echo -e "  Release:  https://github.com/masdevid/mt5-quant/releases/tag/v${new}"
echo ""
echo -e "  ${YELLOW}CI will compute the MCP package SHA256 and${NC}"
echo -e "  ${YELLOW}commit it back to server.json automatically.${NC}"
echo ""
hr
