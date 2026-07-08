#!/usr/bin/env bash
# Install the `mari` binary that the Claude Code plugin wraps:
#   curl -fsSL https://raw.githubusercontent.com/MariHQ/mari-cc/main/install.sh | sh
# It's a single self-contained binary (DuckDB + llama.cpp statically linked).
set -euo pipefail

REPO="MariHQ/mari-cc"
BINDIR="${MARI_BINDIR:-$HOME/.local/bin}"
ASSET="mari-macos-arm64.gz"

os="$(uname -s)"; arch="$(uname -m)"
if [ "$os" != "Darwin" ] || [ "$arch" != "arm64" ]; then
  echo "The prebuilt binary is macOS arm64 only. On other platforms build from source:"
  echo "  cargo install --git https://github.com/$REPO --locked"
  exit 1
fi

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
echo "Downloading mari…"
curl -fL# "https://github.com/$REPO/releases/latest/download/$ASSET" -o "$tmp/$ASSET"
gunzip -c "$tmp/$ASSET" > "$tmp/mari"
mkdir -p "$BINDIR"
install -m 0755 "$tmp/mari" "$BINDIR/mari"
xattr -dr com.apple.quarantine "$BINDIR/mari" 2>/dev/null || true

echo "Installed mari → $BINDIR/mari"
case ":$PATH:" in
  *":$BINDIR:"*) ;;
  *) echo "Add $BINDIR to your PATH (e.g. echo 'export PATH=\"$BINDIR:\$PATH\"' >> ~/.zshrc)";;
esac
