#!/usr/bin/env bash
# Install the Relapse executor UI as a desktop application in the user account.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_ID="dev.relapse.Relapse"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
ICON_ROOT="$HERE/assets/icons/hicolor"
APPLICATIONS="$DATA/applications"
DESKTOP="$APPLICATIONS/$APP_ID.desktop"

for size in 16 24 32 48 64 128 256; do
  source_icon="$ICON_ROOT/${size}x${size}/apps/$APP_ID.png"
  if [[ ! -f "$source_icon" ]]; then
    echo "Missing icon: $source_icon" >&2
    exit 1
  fi
  install -Dm644 "$source_icon" "$DATA/icons/hicolor/${size}x${size}/apps/$APP_ID.png"
done

mkdir -p "$APPLICATIONS"
sed \
  -e "s|@EXEC@|$HERE/run.sh|g" \
  -e "s|@PROJECT@|$HERE|g" \
  "$HERE/packaging/$APP_ID.desktop.in" > "$DESKTOP"
chmod 644 "$DESKTOP"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APPLICATIONS" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$DATA/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Installed Relapse +: $DESKTOP"
echo "Launch it from the application menu, or run: $HERE/run.sh"