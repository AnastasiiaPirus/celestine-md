#!/usr/bin/env bash
# Build a relocatable Linux x86_64 release tarball (local / CI helper).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VER="${1:-$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')}"
NAME="celestine-linux-x86_64"
STAGE="$ROOT/dist/$NAME"
OUT="$ROOT/dist/${NAME}-v${VER}.tar.gz"

echo "==> Building celestine v${VER} (release)"
cargo build -p celestine-app --release

rm -rf "$STAGE"
mkdir -p "$STAGE/bin" \
  "$STAGE/share/celestine/themes" \
  "$STAGE/share/celestine/config" \
  "$STAGE/share/applications" \
  "$STAGE/share/icons/hicolor/scalable/apps"

install -m755 "$ROOT/target/release/celestine" "$STAGE/bin/celestine"
cp -a "$ROOT/themes/"*.css "$STAGE/share/celestine/themes/"
cp -a "$ROOT/config/"keybindings.*.toml "$STAGE/share/celestine/config/"
cp "$ROOT/packaging/celestine.desktop" "$STAGE/share/applications/"
cp "$ROOT/assets/icon/celestine.svg" "$STAGE/share/icons/hicolor/scalable/apps/celestine.svg"

for sz in 16 24 32 48 64 128 256 512 1024; do
  src="$ROOT/assets/icon/celestine-${sz}.png"
  if [[ -f "$src" ]]; then
    mkdir -p "$STAGE/share/icons/hicolor/${sz}x${sz}/apps"
    cp "$src" "$STAGE/share/icons/hicolor/${sz}x${sz}/apps/celestine.png"
  fi
done

cat >"$STAGE/install.sh" <<'EOF'
#!/usr/bin/env bash
# Install this release bundle into ~/.local
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
mkdir -p "$PREFIX/bin" "$PREFIX/share"
install -m755 "$HERE/bin/celestine" "$PREFIX/bin/celestine"
cp -a "$HERE/share/"* "$PREFIX/share/"
# Fix desktop Exec to absolute path
if [[ -f "$PREFIX/share/applications/celestine.desktop" ]]; then
  sed -i "s|^Exec=.*|Exec=$PREFIX/bin/celestine %F|" "$PREFIX/share/applications/celestine.desktop"
  sed -i "s|^TryExec=.*|TryExec=$PREFIX/bin/celestine|" "$PREFIX/share/applications/celestine.desktop"
fi
echo "Installed to $PREFIX/bin/celestine"
EOF
chmod +x "$STAGE/install.sh"

cat >"$STAGE/README.txt" <<EOF
CelestineMD v${VER} — Linux x86_64
https://github.com/nek0pi/celestine-md

Quick start:
  ./bin/celestine ~/Notes

User install (binary + desktop + icons + themes):
  ./install.sh

Or copy bin/celestine somewhere on your PATH.
EOF

mkdir -p "$ROOT/dist"
tar -C "$ROOT/dist" -czf "$OUT" "$NAME"
echo "==> Wrote $OUT"
ls -lh "$OUT"
