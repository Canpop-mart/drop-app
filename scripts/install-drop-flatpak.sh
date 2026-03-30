#!/bin/bash
# Drop OSS - Flatpak Installer
# ────────────────────────────────────────────────────────────────────────────
# For friends installing Drop on Steam Deck (or any Linux with Flatpak).
#
# Instructions:
#   1. Put this script and drop.flatpak in the same folder
#   2. Open a terminal (Steam Deck: switch to Desktop Mode, open Konsole)
#   3. Run:  bash install-drop-flatpak.sh
#
# That's it. Takes about 5 minutes. No compiling required.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUNDLE="$SCRIPT_DIR/drop.flatpak"
APP_ID="org.droposs.client"

echo "======================================"
echo " Drop OSS - Installer"
echo "======================================"
echo ""

# ── Checks ────────────────────────────────────────────────────────────────────

if [ ! -f "$BUNDLE" ]; then
    echo "ERROR: drop.flatpak not found."
    echo "Expected location: $BUNDLE"
    echo ""
    echo "Make sure drop.flatpak is in the same folder as this script."
    exit 1
fi

if ! command -v flatpak &>/dev/null; then
    echo "ERROR: flatpak is not installed."
    echo "On Steam Deck, flatpak is built in — try restarting and running again."
    exit 1
fi

# ── Flathub (for the GNOME runtime that Drop needs) ───────────────────────────

echo ">> [1/4] Adding Flathub..."
flatpak remote-add --user --if-not-exists flathub \
    https://flathub.org/repo/flathub.flatpakrepo
echo "   Done."

# ── Runtime ───────────────────────────────────────────────────────────────────

echo ""
echo ">> [2/4] Installing GNOME runtime (~400 MB, one-time download)..."
flatpak install --user -y flathub org.gnome.Platform//47
echo "   Done."

# ── Install Drop ──────────────────────────────────────────────────────────────

echo ""
echo ">> [3/4] Installing Drop..."
flatpak install --user -y --bundle "$BUNDLE"
echo "   Done."

# ── Pre-download Steam Runtime (steamrt3) ────────────────────────────────────
# umu-launcher needs steamrt3 to run Windows games via Proton. It downloads
# it automatically on first use, but only if there's no stale metadata saying
# "up to date" while the actual files are missing. We clear that and trigger
# the download now so the first game launch just works.

echo ""
echo ">> [4/4] Downloading game compatibility layer (~1 GB)..."
echo "   This is Steam Runtime — needed to run Windows games."
echo "   One-time download, may take a few minutes."
echo ""

# Clear any partial/stale steamrt3 state
rm -rf "$HOME/.local/share/umu/"

# Trigger steamrt3 download inside the Flatpak sandbox.
# GAMEID and PROTONPATH are required by umu-run but don't need to be valid
# for the runtime download to happen — it downloads first, then checks those.
# The command will exit non-zero after downloading (PROTONPATH won't resolve),
# but the runtime will be fully downloaded. We ignore the exit code.
flatpak run \
    --command=bash \
    --share=network \
    --filesystem=home \
    "$APP_ID" \
    -c 'GAMEID=umu-default PROTONPATH=/usr /app/bin/umu-run /bin/true 2>&1 | \
        grep -v "^$" | grep -v "^Traceback" | grep -v "^  File" | \
        grep -v "FileNotFoundError" | grep -v "raise " || true' \
    || true

# Check if steamrt3 landed
if [ -f "$HOME/.local/share/umu/steamrt3/_v2-entry-point" ]; then
    echo "   Steam Runtime ready."
else
    echo "   NOTE: Steam Runtime will finish downloading on first game launch."
    echo "   (This is normal on slow connections — it will work fine.)"
fi

# ── Desktop shortcut ──────────────────────────────────────────────────────────

echo ""
echo ">> Creating app menu shortcut..."
mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/$APP_ID.desktop" << EOF
[Desktop Entry]
Name=Drop
Comment=Drop OSS Game Client
Exec=flatpak run $APP_ID
Icon=org.droposs.client
Type=Application
Categories=Game;
Terminal=false
StartupNotify=true
EOF
update-desktop-database "$HOME/.local/share/applications/" 2>/dev/null || true

# ── Done ──────────────────────────────────────────────────────────────────────

echo ""
echo "======================================"
echo " Drop is installed!"
echo "======================================"
echo ""
echo " Find it in your application menu, or run:"
echo "   flatpak run $APP_ID"
echo ""
echo " To add to Steam Gaming Mode:"
echo "   1. Open Steam > Library > Add a Game > Add a Non-Steam Game"
echo "   2. Click Browse and select: /usr/bin/flatpak"
echo "   3. After adding, open its Properties and set:"
echo "      Launch Options:  run $APP_ID"
echo ""
