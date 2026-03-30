#!/bin/bash
# Drop OSS - Full Uninstaller
# Removes every trace of Drop from Steam Deck — distrobox install,
# old Flatpak install, desktop shortcuts, and leftover files.
# Run from the SteamOS host terminal (Desktop Mode > Konsole).

CONTAINER_NAME="drop-app"
APP_ID="org.droposs.client"

echo "======================================"
echo " Drop OSS - Full Uninstaller"
echo "======================================"
echo ""
echo " This will remove:"
echo "   • The '$CONTAINER_NAME' distrobox container (if present)"
echo "   • The '$APP_ID' Flatpak (if installed)"
echo "   • All Drop desktop shortcuts"
echo "   • ~/drop-app-export/"
echo "   • Any leftover build/patch files"
echo ""
read -rp " Continue? [y/N] " confirm
if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
    echo "Cancelled."
    exit 0
fi
echo ""

REMOVED_ANYTHING=false

# ── Distrobox container ───────────────────────────────────────────────────────

echo ">> Distrobox container..."
if command -v distrobox &>/dev/null; then
    if distrobox list 2>/dev/null | grep -q "$CONTAINER_NAME"; then
        distrobox rm "$CONTAINER_NAME" --force
        echo "   Removed container '$CONTAINER_NAME'."
        REMOVED_ANYTHING=true
    else
        echo "   No container named '$CONTAINER_NAME' — skipping."
    fi
else
    echo "   distrobox not installed — skipping."
fi

# ── Flatpak ───────────────────────────────────────────────────────────────────

echo ""
echo ">> Flatpak installation..."
if command -v flatpak &>/dev/null; then
    if flatpak list --user 2>/dev/null | grep -q "$APP_ID"; then
        flatpak uninstall --user -y "$APP_ID"
        echo "   Removed Flatpak '$APP_ID'."
        REMOVED_ANYTHING=true
    else
        echo "   '$APP_ID' not installed as user Flatpak — skipping."
    fi
    # Also check system-wide install
    if flatpak list --system 2>/dev/null | grep -q "$APP_ID"; then
        sudo flatpak uninstall --system -y "$APP_ID"
        echo "   Removed system Flatpak '$APP_ID'."
        REMOVED_ANYTHING=true
    fi
else
    echo "   flatpak not found — skipping."
fi

# ── Desktop shortcuts ─────────────────────────────────────────────────────────

echo ""
echo ">> Desktop shortcuts..."
DESKTOP_FILES=(
    "$HOME/.local/share/applications/drop-app.desktop"
    "$HOME/.local/share/applications/${APP_ID}.desktop"
)
REMOVED_DESKTOP=false
for f in "${DESKTOP_FILES[@]}"; do
    if [ -f "$f" ]; then
        rm -f "$f"
        echo "   Removed: $f"
        REMOVED_DESKTOP=true
        REMOVED_ANYTHING=true
    fi
done
# Catch any other Drop-related .desktop files
while IFS= read -r f; do
    rm -f "$f"
    echo "   Removed: $f"
    REMOVED_DESKTOP=true
    REMOVED_ANYTHING=true
done < <(grep -rl --include="*.desktop" -e "drop-app" -e "droposs" \
    "$HOME/.local/share/applications/" 2>/dev/null || true)

if $REMOVED_DESKTOP; then
    update-desktop-database "$HOME/.local/share/applications/" 2>/dev/null || true
else
    echo "   None found — skipping."
fi

# ── Export / build folders ────────────────────────────────────────────────────

echo ""
echo ">> Leftover folders..."
DIRS_TO_REMOVE=(
    "$HOME/drop-app-export"
    "$HOME/drop-app-build"
    "$HOME/drop-flatpak-staging"
    "$HOME/drop-flatpak-repo"
    "$HOME/.cache/drop-flatpak-build"
    "$HOME/.cache/drop-flatpak-repo"
)
for d in "${DIRS_TO_REMOVE[@]}"; do
    if [ -d "$d" ]; then
        rm -rf "$d"
        echo "   Removed: $d"
        REMOVED_ANYTHING=true
    fi
done

# ── Leftover files ────────────────────────────────────────────────────────────

echo ""
echo ">> Leftover files..."
FILES_TO_REMOVE=(
    "$HOME/drop-patch.py"
    "$HOME/drop-app.flatpak"
)
for f in "${FILES_TO_REMOVE[@]}"; do
    if [ -f "$f" ]; then
        rm -f "$f"
        echo "   Removed: $f"
        REMOVED_ANYTHING=true
    fi
done

# ── Done ──────────────────────────────────────────────────────────────────────

echo ""
echo "======================================"
if $REMOVED_ANYTHING; then
    echo " Uninstall complete."
else
    echo " Nothing to remove — already clean."
fi
echo "======================================"
echo ""
echo " To install the Flatpak version, run:"
echo "   bash install-drop-flatpak.sh"
echo ""
