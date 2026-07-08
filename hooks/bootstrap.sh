#!/usr/bin/env bash
# SessionStart hook: keep the `mari` binary the plugin wraps installed and in
# sync with the plugin version. Installs it when missing, and re-installs when
# the plugin has been updated to a newer version than the binary — so updating
# the plugin in Claude Code (`/plugin marketplace update mari`) also updates the
# binary on the next session. Never breaks the session: always exits 0.
{
  # Prebuilt binary is macOS arm64; other platforms build from source.
  [ "$(uname -s)" = "Darwin" ] && [ "$(uname -m)" = "arm64" ] || exit 0

  BINDIR="$HOME/.local/bin"
  MARI="$(command -v mari 2>/dev/null || true)"
  [ -n "$MARI" ] || { [ -x "$BINDIR/mari" ] && MARI="$BINDIR/mari"; }

  # Version the plugin expects (from its manifest) vs the installed binary's.
  want="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
          "${CLAUDE_PLUGIN_ROOT:-}/.claude-plugin/plugin.json" 2>/dev/null | head -1)"
  have=""
  [ -n "$MARI" ] && have="$("$MARI" --version 2>/dev/null | awk '{print $2}')"

  if [ -n "$MARI" ]; then
    # Installed. Update only on a definite version mismatch (avoid re-download loops
    # when either version can't be read).
    [ -z "$want" ] || [ -z "$have" ] || [ "$have" = "$want" ] && exit 0
  fi

  tmp="$(mktemp -d)" || exit 0
  url="https://github.com/MariHQ/mari-cc/releases/latest/download/mari-macos-arm64.gz"
  if curl -fsSL "$url" -o "$tmp/m.gz" 2>/dev/null && gunzip -c "$tmp/m.gz" > "$tmp/mari" 2>/dev/null; then
    mkdir -p "$BINDIR"
    install -m 0755 "$tmp/mari" "$BINDIR/mari" 2>/dev/null || true
    xattr -dr com.apple.quarantine "$BINDIR/mari" 2>/dev/null || true
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
    if [ -n "$have" ] && [ -n "$want" ]; then
      echo "Updated mari $have → $want"
    else
      echo "Installed mari → $BINDIR/mari"
    fi
  fi
  rm -rf "$tmp"
} 2>/dev/null
exit 0
