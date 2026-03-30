#!/bin/bash
# Drop OSS - Full Uninstaller
# Removes every trace of Drop regardless of how it was installed:
#   - Flatpak (org.droposs.client)
#   - Distrobox container (drop-app)
#   - Desktop entries, icons, build caches, export folders

echo "======================================"
echo " Drop OSS - Uninstaller"
echo "======================================"
echo ""

REMOVED=0

# ── Flatpak ───────────────────────────────────────────────────────────────────

if flatpak list --user 2>/dev/null | grep -q "org.droposs.client"; then
    echo ">> Removing Flatpak (org.droposs.client)..."
    flatpak uninstall --user -y org.droposs.client 2>/dev/null || true
    echo "   Done."
    REMOVED=1
else
    echo ">> Flatpak not installed — skipping."
fi

# ── Distrobox container ───────────────────────────────────────────────────────

if command -v distrobox &>/dev/null; then
    if distrobox list 2>/dev/null | grep -q "drop-app"; then
        echo ">> Removing distrobox container (drop-app)..."
        distrobox rm drop-app --force 2>/dev/null || true
        echo "   Done."
        REMOVED=1
    else
        echo ">> Distrobox container not found — skipping."
    fi
fi

# ── Desktop entries ───────────────────────────────────────────────────────────

echo ">> Removing desktop entries..."
DESK_REMOVED=0

for f in \
    "$HOME/.local/share/applications/drop-app.desktop" \
    "$HOME/.local/share/applications/org.droposs.client.desktop"; do
    if [ -f "$f" ]; then
        rm -f "$f"
        echo "   Removed: $f"
        DESK_REMOVED=1
        REMOVED=1
    fi
done

# Also catch any leftover flatpak-exported desktop entries
for f in "$HOME/.local/share/flatpak/exports/share/applications/"*droposs*; do
    [ -f "$f" ] || continue
    rm -f "$f"
    echo "   Removed: $f"
    DESK_REMOVED=1
    REMOVED=1
done

[ "$DESK_REMOVED" -eq 0 ] && echo "   None found."

update-desktop-database "$HOME/.local/share/applications/" 2>/dev/null || true

# ── Icons ─────────────────────────────────────────────────────────────────────

echo ">> Removing icons..."
ICON_REMOVED=0
while IFS= read -r -d '' f; do
    rm -f "$f"
    echo "   Removed: $f"
    ICON_REMOVED=1
    REMOVED=1
done < <(find "$HOME/.local/share/icons" \
    -name "*droposs*" -o -name "*drop-app*" \
    -print0 2>/dev/null)

[ "$ICON_REMOVED" -eq 0 ] && echo "   None found."

# ── Build cache and export folders ────────────────────────────────────────────

echo ">> Removing build cache and export folders..."
BUILD_REMOVED=0

for d in \
    "$HOME/.cache/drop-flatpak-build" \
    "$HOME/.cache/drop-flatpak-repo" \
    "$HOME/drop-app-export" \
    "$HOME/drop-app-flatpak" \
    "$HOME/drop-app-staging"; do
    if [ -d "$d" ]; then
        rm -rf "$d"
        echo "   Removed: $d"
        BUILD_REMOVED=1
        REMOVED=1
    fi
done

[ "$BUILD_REMOVED" -eq 0 ] && echo "   None found."

# ── Toolchain installs (left by build script) ─────────────────────────────────

echo ">> Removing temporary build toolchains..."
TOOL_REMOVED=0

# These are under the build cache (already removed above), but check separately
# in case someone ran an older build script that used different paths
for d in \
    "$HOME/.drop-build-cargo" \
    "$HOME/.drop-build-rustup" \
    "$HOME/.drop-build-nvm" \
    "$HOME/.drop-build-npm" \
    "$HOME/.drop-build-src" \
    "$HOME/.drop-build-corepack"; do
    if [ -d "$d" ]; then
        rm -rf "$d"
        echo "   Removed: $d"
        TOOL_REMOVED=1
        REMOVED=1
    fi
done

[ "$TOOL_REMOVED" -eq 0 ] && echo "   None found."

# ── Flatpak runtime (optional, ask user) ──────────────────────────────────────

if flatpak list --user 2>/dev/null | grep -qE "org\.gnome\.(Platform|Sdk)"; then
    echo ""
    echo ">> The GNOME SDK/Platform runtime is still installed (~1 GB)."
    read -r -p "   Remove it? [y/N] " REPLY
    if [[ "$REPLY" =~ ^[Yy]$ ]]; then
        flatpak uninstall --user -y \
            org.gnome.Platform//47 \
            org.gnome.Sdk//47 \
            org.freedesktop.Sdk.Extension.rust-stable//24.08 \
            org.freedesktop.Sdk.Extension.node22//24.08 \
            2>/dev/null || true
        echo "   Runtime removed."
        REMOVED=1
    else
        echo "   Keeping runtime (useful if you plan to rebuild)."
    fi
fi

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "======================================"
if [ "$REMOVED" -eq 1 ]; then
    echo " Drop removed."
else
    echo " Nothing to remove — Drop was not installed."
fi
echo "======================================"
echo ""
