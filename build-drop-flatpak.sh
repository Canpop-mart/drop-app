#!/bin/bash
# Drop OSS v0.4.0 - Flatpak Build Script
# No distrobox, no sudo, no flatpak-builder. Works on Steam Deck.
#
# How it works:
#   HOST (full network, writable $HOME):
#     - Download umu-run
#     - Install Rust nightly via rustup  (→ $BUILD_DIR/cargo)
#     - Install Node 23 via nvm          (→ $BUILD_DIR/nvm)
#     - Install pnpm                     (→ $BUILD_DIR/pnpm)
#     - git clone + patch + pnpm install
#
#   SDK SHELL (flatpak run org.gnome.Sdk//47 --share=network --filesystem=home):
#     - Provides webkit2gtk-4.1, gtk3, glib2 headers for Rust linking
#     - Uses host-installed Rust/Node/pnpm from $HOME via --filesystem=home
#     - Runs: pnpm tauri build
#
#   HOST again:
#     - flatpak build-init / build-finish / build-export / build-bundle
#
# Output: ~/drop-app-export/drop.flatpak
# Build time: ~30-60 minutes

set -e

REPO_TAG="v0.4.0-rc-4"
APP_ID="org.droposs.client"
SCRIPT_DIR="$(dirname "$(readlink -f "$0")")"
EXPORT_DIR="$HOME/drop-app-export"
BUILD_DIR="$HOME/.cache/drop-flatpak-build"
STAGING_DIR="$BUILD_DIR/staging"
REPO_DIR="$BUILD_DIR/repo"
BUNDLE="$EXPORT_DIR/drop.flatpak"

# Toolchain dirs (all under $BUILD_DIR, no system pollution)
RUSTUP_HOME="$BUILD_DIR/rustup"
CARGO_HOME="$BUILD_DIR/cargo"
NVM_DIR="$BUILD_DIR/nvm"
PNPM_PREFIX="$BUILD_DIR/pnpm"

echo "======================================"
echo " Drop OSS - Flatpak Builder"
echo "======================================"
echo ""

# ── Clean ─────────────────────────────────────────────────────────────────────

echo ">> Cleaning previous build..."
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR" "$EXPORT_DIR"

# ── SDK ───────────────────────────────────────────────────────────────────────

echo ""
echo ">> Setting up Flathub and SDK (one-time, ~1 GB)..."
flatpak remote-add --user --if-not-exists flathub \
    https://flathub.org/repo/flathub.flatpakrepo

flatpak install --user -y flathub \
    org.gnome.Platform//47 \
    org.gnome.Sdk//47

echo "   SDK ready."

# ── Download umu-run ──────────────────────────────────────────────────────────

echo ""
echo ">> Downloading umu-run..."
UMU_URL=$(curl -fsSL "https://api.github.com/repos/Open-Wine-Components/umu-launcher/releases/latest" | \
    python3 -c "
import sys, json
d = json.load(sys.stdin)
print(next((a['browser_download_url'] for a in d['assets'] if 'zipapp.tar' in a['name']), ''))
")

if [ -z "$UMU_URL" ]; then
    echo "ERROR: Could not get umu-launcher download URL"
    exit 1
fi

curl -fsSL -o "$BUILD_DIR/umu.tar" "$UMU_URL"
tar -xf "$BUILD_DIR/umu.tar" -C "$BUILD_DIR/"
UMU_BIN=$(find "$BUILD_DIR" -maxdepth 4 -name "umu-run" -type f | head -1)
if [ -z "$UMU_BIN" ]; then
    echo "ERROR: umu-run not found in archive"
    exit 1
fi
cp "$UMU_BIN" "$BUILD_DIR/umu-run"
chmod +x "$BUILD_DIR/umu-run"
echo "   umu-run downloaded."

# ── Install Rust nightly (to BUILD_DIR, no system changes) ───────────────────

echo ""
echo ">> Installing Rust nightly..."
export RUSTUP_HOME CARGO_HOME
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- \
    --default-toolchain nightly \
    --profile minimal \
    --no-modify-path \
    -y \
    -q
. "$CARGO_HOME/env"
echo "   $(rustc --version)"

# ── Install Node 23 via nvm (to BUILD_DIR, no system changes) ────────────────

echo ""
echo ">> Installing Node 23..."
mkdir -p "$NVM_DIR"
curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/master/install.sh | \
    NVM_DIR="$NVM_DIR" PROFILE=/dev/null bash > /dev/null 2>&1
. "$NVM_DIR/nvm.sh" --no-use
nvm install 23 --no-progress -s > /dev/null 2>&1
NODE_VER=$(nvm version 23)
NODE_BIN="$NVM_DIR/versions/node/$NODE_VER/bin"
echo "   Node $NODE_VER"

# ── Install pnpm ──────────────────────────────────────────────────────────────

echo ""
echo ">> Installing pnpm..."
"$NODE_BIN/npm" install -g pnpm --prefix "$PNPM_PREFIX" --silent
PNPM="$PNPM_PREFIX/bin/pnpm"
echo "   pnpm $("$PNPM" --version)"

# Set up PATH for subsequent host steps
export PATH="$CARGO_HOME/bin:$NODE_BIN:$PNPM_PREFIX/bin:$PATH"

# ── Clone, patch, and install JS deps (on host) ───────────────────────────────

echo ""
echo ">> Cloning Drop $REPO_TAG..."
git clone -q --branch "$REPO_TAG" https://github.com/Drop-OSS/drop-app.git "$BUILD_DIR/src"
cd "$BUILD_DIR/src"
git submodule update --init --recursive -q
echo "   Checked out: $(git describe --tags 2>/dev/null || git rev-parse --short HEAD)"

echo ""
echo ">> Applying patches..."

# Patch 1: remove bundle targets that can't be built in this env
python3 -c "
import json
with open('src-tauri/tauri.conf.json') as f:
    c = json.load(f)
c['bundle']['targets'] = [t for t in c['bundle']['targets'] if t not in ('appimage', 'dmg')]
with open('src-tauri/tauri.conf.json', 'w') as f:
    json.dump(c, f, indent=2)
print('  [OK] Removed appimage/dmg targets')
"

# Patch 2: fix WindowsLauncher (absolute path + spaces in game folder)
python3 << 'PYEOF'
path = "src-tauri/process/src/process_handlers.rs"
with open(path) as f:
    content = f.read()

old_import = 'use crate::{error::ProcessError, process_manager::ProcessHandler};'
new_import = 'use crate::{error::ProcessError, parser::ParsedCommand, process_manager::ProcessHandler};'
if old_import in content:
    content = content.replace(old_import, new_import)
    print("  [OK] ParsedCommand import added")
elif new_import in content:
    print("  [SKIP] ParsedCommand import already present")
else:
    print("  [WARN] Import not found — check process_handlers.rs")

old = '        _current_dir: &str,\n        _database: &Database,\n    ) -> Result<String, ProcessError> {\n        Ok(format!("cmd /C \\"{}\\"", launch_command))\n    }'
new = '        current_dir: &str,\n        _database: &Database,\n    ) -> Result<String, ProcessError> {\n        let mut parsed = ParsedCommand::parse(launch_command)?;\n        parsed.make_absolute(PathBuf::from(current_dir));\n        Ok(parsed.reconstruct())\n    }'
if old in content:
    content = content.replace(old, new)
    print("  [OK] WindowsLauncher patched")
elif 'parsed.make_absolute' in content:
    print("  [SKIP] WindowsLauncher already patched")
else:
    print("  [WARN] WindowsLauncher patch not applied — check process_handlers.rs")

with open(path, "w") as f:
    f.write(content)
PYEOF

# Patch 3: convert Unix paths to Wine Z:\ paths in UMUCompatLauncher so that
# umu-run uses "launching normally" instead of /unix mode (which drops extra args).
python3 << 'PYEOF'
path = "src-tauri/process/src/process_handlers.rs"
with open(path) as f:
    content = f.read()

SKIP = 'fn to_wine(p: &str)'
if SKIP in content:
    print("  [SKIP] UMUCompatLauncher wine path conversion already patched")
else:
    old = '        let proton_env = format!("PROTONPATH={}", proton_path);\n\n        Ok(format!('
    new = (
        '        let proton_env = format!("PROTONPATH={}", proton_path);\n\n'
        '        // Convert absolute Unix paths to Wine Z:\\ paths so umu-run launches\n'
        '        // the exe normally (not with /unix flag which drops extra arguments).\n'
        '        fn to_wine(p: &str) -> String {\n'
        '            if p.starts_with(\'/\') { format!("Z:{}", p.replace(\'/\', "\\\\")) } else { p.to_string() }\n'
        '        }\n'
        '        let mut parsed = ParsedCommand::parse(launch_command)?;\n'
        '        parsed.command = to_wine(&parsed.command);\n'
        '        parsed.args = parsed.args.iter().map(|a| to_wine(a)).collect();\n'
        '        let launch_command = parsed.reconstruct();\n\n'
        '        Ok(format!('
    )
    if old in content:
        content = content.replace(old, new)
        with open(path, "w") as f:
            f.write(content)
        print("  [OK] UMUCompatLauncher: Unix->Z:\\ path conversion added")
    else:
        print("  [WARN] UMUCompatLauncher format block not found — check process_handlers.rs")
PYEOF

echo ""
echo ">> Installing JS dependencies..."
"$PNPM" install -s

# ── Write SDK compile script ──────────────────────────────────────────────────
# Variables expanded here (outer shell): BUILD_DIR, CARGO_HOME, NODE_BIN, PNPM_PREFIX
# The script runs inside the SDK shell where those paths are accessible via --filesystem=home

cat > "$BUILD_DIR/sdk-compile.sh" << SDKEOF
#!/bin/bash
set -e
export RUSTUP_HOME="$RUSTUP_HOME"
export CARGO_HOME="$CARGO_HOME"
export PATH="$CARGO_HOME/bin:$NODE_BIN:$PNPM_PREFIX/bin:/usr/bin:/bin"
export LIBRARY_PATH="/usr/lib/x86_64-linux-gnu:/usr/lib:\${LIBRARY_PATH:-}"
export PKG_CONFIG_PATH="/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig:\${PKG_CONFIG_PATH:-}"

echo "   Rust: \$(rustc --version)"
echo "   Node: \$(node --version)"
echo "   pnpm: \$(pnpm --version)"

cd $BUILD_DIR/src
echo "   Building... (20-40 minutes)"
pnpm tauri build 2>&1 | tail -5

cp src-tauri/target/release/drop-app    $BUILD_DIR/drop-app
cp src-tauri/icons/icon.png              $BUILD_DIR/drop-icon.png
cp src-tauri/icons/128x128@2x.png        $BUILD_DIR/drop-icon-512.png

echo "   Compile done."
SDKEOF
chmod +x "$BUILD_DIR/sdk-compile.sh"

# ── Compile inside SDK shell ──────────────────────────────────────────────────
# org.gnome.Sdk//47 provides webkit2gtk-4.1, gtk3, glib2 headers for Rust linking.
# --share=network gives cargo access to crates.io to download Rust dependencies.
# --filesystem=home makes the host-installed Rust/Node/pnpm visible inside the shell.
# No inner sandbox is created — this is just a bash process in a Flatpak env.

echo ""
echo ">> Compiling inside GNOME SDK shell... (~30-60 minutes)"
echo "   Full log: $EXPORT_DIR/build.log"
echo ""

flatpak run \
    --command=bash \
    --share=network \
    --filesystem=home \
    org.gnome.Sdk//47 \
    "$BUILD_DIR/sdk-compile.sh" \
    2>&1 | tee "$EXPORT_DIR/build.log" | grep --line-buffered -E \
        "(^   |error\[|^error|Finished|Downloading|WARN|OK\]|SKIP\])" || true

if [ ! -f "$BUILD_DIR/drop-app" ]; then
    echo ""
    echo "ERROR: Build failed — drop-app binary not found."
    echo "Check: $EXPORT_DIR/build.log"
    exit 1
fi

echo ""
echo "   Binary ready: $(du -sh "$BUILD_DIR/drop-app" | cut -f1)"

# ── Package as Flatpak ────────────────────────────────────────────────────────
# Uses only base `flatpak` commands — already present on Steam Deck.

echo ""
echo ">> Packaging..."
rm -rf "$STAGING_DIR"
flatpak build-init "$STAGING_DIR" "$APP_ID" org.gnome.Platform org.gnome.Sdk 47

mkdir -p \
    "$STAGING_DIR/files/bin" \
    "$STAGING_DIR/files/share/applications" \
    "$STAGING_DIR/files/share/icons/hicolor/256x256/apps" \
    "$STAGING_DIR/files/share/icons/hicolor/512x512/apps"

install -Dm755 "$BUILD_DIR/drop-app"         "$STAGING_DIR/files/bin/drop-app"
install -Dm755 "$BUILD_DIR/umu-run"           "$STAGING_DIR/files/bin/umu-run"
install -Dm644 "$BUILD_DIR/drop-icon.png"     "$STAGING_DIR/files/share/icons/hicolor/256x256/apps/$APP_ID.png"
install -Dm644 "$BUILD_DIR/drop-icon-512.png" "$STAGING_DIR/files/share/icons/hicolor/512x512/apps/$APP_ID.png"

# Bundle libs that aren't in org.gnome.Platform but are needed at runtime.
# libayatana-appindicator3 (system tray) is dlopen()d at startup — Drop panics
# without it. These come from the host (Ubuntu apt), not the GNOME SDK.
echo ">> Bundling runtime libs (appindicator + deps)..."
apt-get install -y -qq \
    libayatana-appindicator3-1 \
    libayatana-indicator3-7 \
    libdbusmenu-gtk3-4 \
    libdbusmenu-glib4 \
    libayatana-ido3-0.4-0 2>/dev/null || true
mkdir -p "$STAGING_DIR/files/lib"
for lib in \
    libayatana-appindicator3.so.1.0.0 libayatana-appindicator3.so.1 libappindicator3.so.1 \
    libayatana-indicator3.so.7.0.0    libayatana-indicator3.so.7 \
    libdbusmenu-gtk3.so.4.0.12        libdbusmenu-gtk3.so.4 \
    libdbusmenu-glib.so.4.0.12        libdbusmenu-glib.so.4 \
    libayatana-ido3-0.4.so.0.0.0      libayatana-ido3-0.4.so.0; do
    src="/usr/lib/x86_64-linux-gnu/$lib"
    if [ -f "$src" ]; then
        install -Dm644 "$src" "$STAGING_DIR/files/lib/$lib"
        echo "   + $lib"
    fi
done

cat > "$STAGING_DIR/files/share/applications/$APP_ID.desktop" << DESKEOF
[Desktop Entry]
Name=Drop
Comment=Drop OSS Game Client
Exec=drop-app
Icon=$APP_ID
Type=Application
Categories=Game;
Terminal=false
StartupNotify=true
DESKEOF

flatpak build-finish "$STAGING_DIR" \
    --share=network \
    --share=ipc \
    --socket=wayland \
    --socket=fallback-x11 \
    --socket=pulseaudio \
    --device=all \
    --filesystem=host \
    --talk-name=org.freedesktop.Notifications \
    --env=PATH=/app/bin:/usr/bin:/bin \
    --command=drop-app

echo "   Exporting..."
rm -rf "$REPO_DIR"
flatpak build-export --arch=x86_64 "$REPO_DIR" "$STAGING_DIR"

echo "   Bundling..."
flatpak build-bundle --arch=x86_64 "$REPO_DIR" "$BUNDLE" "$APP_ID"

BUNDLE_SIZE=$(du -sh "$BUNDLE" 2>/dev/null | cut -f1)
echo "   Bundle: $BUNDLE ($BUNDLE_SIZE)"

# ── Copy install script ───────────────────────────────────────────────────────

if [ -f "$SCRIPT_DIR/install-drop-flatpak.sh" ]; then
    cp "$SCRIPT_DIR/install-drop-flatpak.sh" "$EXPORT_DIR/install-drop-flatpak.sh"
    chmod +x "$EXPORT_DIR/install-drop-flatpak.sh"
fi

# ── Done ──────────────────────────────────────────────────────────────────────

echo ""
echo "======================================"
echo " Done!"
echo "======================================"
echo ""
echo " Share this folder with your friends:"
echo "   $EXPORT_DIR/"
echo "   ├── drop.flatpak               (the app)"
echo "   └── install-drop-flatpak.sh    (friends run this)"
echo ""
echo " To install on this machine:"
echo "   flatpak install --user $BUNDLE"
echo "   flatpak run $APP_ID"
echo ""
