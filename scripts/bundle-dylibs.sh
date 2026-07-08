#!/usr/bin/env bash
# Collect the dynamic libs mari links against into <target>/<profile>/lib/ so the
# binary resolves @rpath/*.dylib at runtime. Run after `cargo build [--release]`.
#
# Usage: scripts/bundle-dylibs.sh [debug|release]   (default: release)
set -euo pipefail

PROFILE="${1:-release}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
OUT="$TARGET_DIR/$PROFILE/lib"
mkdir -p "$OUT"

copy() { # src -> OUT, preserving name, and normalize install_name to @rpath
  local src="$1" base
  base="$(basename "$src")"
  install -m 0755 "$src" "$OUT/$base"
  install_name_tool -id "@rpath/$base" "$OUT/$base" 2>/dev/null || true
}

echo "==> DuckDB (vendored v1.5.4)"
copy "$ROOT/vendor/duckdb/libduckdb.dylib"

echo "==> llama.cpp / ggml (built by llama-cpp-sys-2)"
# The sys crate emits its .dylibs under its OUT_DIR build tree.
found=0
while IFS= read -r dylib; do
  copy "$dylib"; found=$((found+1))
done < <(find "$TARGET_DIR/$PROFILE/build" -type d -name 'llama-cpp-sys-2-*' \
           -exec find {} -name '*.dylib' \; 2>/dev/null | sort -u)
[ "$found" -gt 0 ] || { echo "  !! no llama/ggml dylibs found — did the build run with dynamic-link?" >&2; exit 1; }

echo "==> bundled into $OUT:"
ls -la "$OUT"
