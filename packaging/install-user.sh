#!/usr/bin/env bash
# User-local install: binary, desktop entry, and hicolor icons.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
APP_DIR="$PREFIX/share/applications"
ICON_BASE="$PREFIX/share/icons/hicolor"
ICON_SRC="$ROOT/assets/icon"

mkdir -p "$BIN_DIR" "$APP_DIR"

if [[ -x "$ROOT/target/release/celestine" ]]; then
  install -Dm755 "$ROOT/target/release/celestine" "$BIN_DIR/celestine"
else
  echo "warning: $ROOT/target/release/celestine missing — build with:" >&2
  echo "  cargo build -p celestine-app --release" >&2
fi

# Desktop entry with PREFIX-relative Exec/TryExec
sed \
  -e "s|^Exec=.*|Exec=$BIN_DIR/celestine %F|" \
  -e "s|^TryExec=.*|TryExec=$BIN_DIR/celestine|" \
  "$ROOT/packaging/celestine.desktop" >"$APP_DIR/celestine.desktop"
chmod 644 "$APP_DIR/celestine.desktop"

for sz in 16 24 32 48 64 128 256 512 1024; do
  src="$ICON_SRC/celestine-${sz}.png"
  if [[ -f "$src" ]]; then
    install -Dm644 "$src" "$ICON_BASE/${sz}x${sz}/apps/celestine.png"
  fi
done
install -Dm644 "$ICON_SRC/celestine.svg" "$ICON_BASE/scalable/apps/celestine.svg"

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$ICON_BASE" 2>/dev/null || true
elif command -v update-icon-caches >/dev/null 2>&1; then
  update-icon-caches "$ICON_BASE" 2>/dev/null || true
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APP_DIR" 2>/dev/null || true
fi

echo "Installed CelestineMD to $PREFIX"
echo "  binary:  $BIN_DIR/celestine"
echo "  desktop: $APP_DIR/celestine.desktop"
echo "  icons:   $ICON_BASE/*/apps/celestine.*"
