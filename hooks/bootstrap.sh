#!/usr/bin/env bash
# SessionStart hook: ensure the `mari` binary the plugin wraps is installed.
# One-time — once `mari` is present this exits immediately. Never breaks the
# session: it always exits 0, even on failure.
{
  command -v mari >/dev/null 2>&1 && exit 0

  # Prebuilt binary is macOS arm64. On anything else, leave it to the user to
  # `cargo install --git https://github.com/MariHQ/mari-cc --locked`.
  [ "$(uname -s)" = "Darwin" ] && [ "$(uname -m)" = "arm64" ] || exit 0

  BINDIR="$HOME/.local/bin"
  [ -x "$BINDIR/mari" ] && exit 0   # already installed, just not on PATH yet

  url="https://github.com/MariHQ/mari-cc/releases/latest/download/mari-macos-arm64.gz"
  tmp="$(mktemp -d)" || exit 0
  if curl -fsSL "$url" -o "$tmp/m.gz" 2>/dev/null && gunzip -c "$tmp/m.gz" > "$tmp/mari" 2>/dev/null; then
    mkdir -p "$BINDIR"
    install -m 0755 "$tmp/mari" "$BINDIR/mari" 2>/dev/null || true
    xattr -dr com.apple.quarantine "$BINDIR/mari" 2>/dev/null || true
    # Put ~/.local/bin on PATH for future shells if it isn't already.
    case ":$PATH:" in
      *":$BINDIR:"*) ;;
      *)
        for rc in "$HOME/.zshrc" "$HOME/.bashrc" "$HOME/.profile"; do
          [ -f "$rc" ] || continue
          grep -q '# mari PATH' "$rc" 2>/dev/null && continue
          printf '\n# mari PATH\nexport PATH="$HOME/.local/bin:$PATH"\n' >> "$rc"
        done
        ;;
    esac
    echo "Installed mari → $BINDIR/mari (run 'mari sync' in a repo to index it)"
  fi
  rm -rf "$tmp"
} 2>/dev/null
exit 0
