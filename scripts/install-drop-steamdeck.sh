#!/bin/bash
# Drop OSS - Steam Deck Installer
# Place this file next to drop-app.AppImage and run it.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPIMAGE_SRC="$SCRIPT_DIR/drop-app.AppImage"
APPIMAGE_DEST="$HOME/drop-app.AppImage"

echo "======================================"
echo "   Drop OSS - Steam Deck Installer"
echo "======================================"
echo ""

# Check if running inside distrobox
if [ -f /run/.containerenv ] || [ -f /.dockerenv ]; then
    echo "ERROR: Run this from the SteamOS host terminal, not inside a container."
    exit 1
fi

# Check AppImage exists
if [ ! -f "$APPIMAGE_SRC" ]; then
    echo "ERROR: drop-app.AppImage not found."
    echo "Make sure it is in the same folder as this script."
    echo "Expected: $APPIMAGE_SRC"
    exit 1
fi

# Check if already installed
if [ -f "$APPIMAGE_DEST" ]; then
    echo " Drop is already installed!"
    echo ""
    read -p " Reinstall anyway? (y/N): " confirm
    if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
        echo "Cancelled."
        exit 0
    fi
fi

echo " Installing Drop..."
echo ""

# Copy AppImage to home directory and make executable
cp "$APPIMAGE_SRC" "$APPIMAGE_DEST"
chmod +x "$APPIMAGE_DEST"

# Create desktop entry pointing directly to the AppImage on the host.
# Running on the host (not inside distrobox) means Flatpak-based emulators
# (snes9x, Dolphin, etc.) can launch without bwrap nesting errors.
mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/drop-app.desktop" << DESKTOPEOF
[Desktop Entry]
Name=Drop
Comment=Drop OSS Game Client
Exec=${APPIMAGE_DEST}
Icon=applications-games
Type=Application
Categories=Game;
Terminal=false
StartupNotify=true
DESKTOPEOF
chmod +x "$HOME/.local/share/applications/drop-app.desktop"

# Refresh desktop database so Drop appears in the app menu immediately
update-desktop-database ~/.local/share/applications/ 2>/dev/null || true
gtk-update-icon-cache 2>/dev/null || true

echo "======================================"
echo "   Installation complete!"
echo "======================================"
echo ""
echo " Find Drop in your application menu."
echo " If you don't see it, try logging out and back in."
echo ""
echo " To use snes9x or other Flatpak emulators, install them first:"
echo "   flatpak install flathub net.kuribo64.melonDS"
echo "   flatpak install flathub com.snes9x.Snes9x"
echo "   flatpak install flathub org.DolphinEmu.dolphin-emu"
echo ""
