#!/usr/bin/env bash
# Install the `mari` binary that the Claude Code plugin wraps.
#   curl -fsSL https://raw.githubusercontent.com/MariHQ/mari-cc/main/install.sh | sh
# Uses the GitHub CLI when available (so it works for a PRIVATE repo with your
# auth); otherwise falls back to an anonymous download (public repo only).
set -euo pipefail

REPO="MariHQ/mari-cc"
BINDIR="${MARI_BINDIR:-$HOME/.local/bin}"
ASSET="mari-macos-arm64.zip"

os="$(uname -s)"; arch="$(uname -m)"
if [ "$os" != "Darwin" ] || [ "$arch" != "arm64" ]; then
  echo "The prebuilt binary is macOS arm64 only. On other platforms build from source:"
  echo "  cargo install --git https://github.com/$REPO --locked"
  exit 1
fi

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
echo "Downloading mari…"
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
  gh release download --repo "$REPO" --pattern "$ASSET" --dir "$tmp" --clobber
else
  url="https://github.com/$REPO/releases/latest/download/$ASSET"
  if ! curl -fsSL "$url" -o "$tmp/$ASSET"; then
    echo "✗ download failed. If $REPO is private, install the GitHub CLI and run 'gh auth login', then re-run this." >&2
    exit 1
  fi
fi

( cd "$tmp" && unzip -q "$ASSET" )
mkdir -p "$BINDIR"
install -m 0755 "$tmp/mari-macos-arm64/mari" "$BINDIR/mari"
xattr -dr com.apple.quarantine "$BINDIR/mari" 2>/dev/null || true

echo "Installed mari → $BINDIR/mari"
case ":$PATH:" in
  *":$BINDIR:"*) ;;
  *) echo "Add $BINDIR to your PATH (e.g. echo 'export PATH=\"$BINDIR:\$PATH\"' >> ~/.zshrc)";;
esac
