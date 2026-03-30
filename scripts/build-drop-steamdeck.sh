#!/bin/bash
# Drop OSS v0.4.0 - Steam Deck Build Script
# Run from the SteamOS host terminal (NOT inside a distrobox)

set -e

CONTAINER_NAME="drop-app"
REPO_TAG="v0.4.0-rc-4"
EXPORT_PATH="$HOME/drop-app-export"

echo "======================================"
echo " Drop OSS - Steam Deck Builder"
echo " Tag: $REPO_TAG"
echo "======================================"
echo " ~20-40 minutes. Keep your Deck plugged in!"
echo ""

# Must run from host
if [ -f /run/.containerenv ] || [ -f /.dockerenv ]; then
    echo "ERROR: Run this from the SteamOS host, not inside a distrobox."
    exit 1
fi

# ── Clean previous build artifacts ───────────────────────────────────────────
echo ">> Cleaning previous build..."
rm -rf "$EXPORT_PATH"
rm -f "$HOME/drop-patch.py"
if distrobox list 2>/dev/null | grep -q "$CONTAINER_NAME"; then
    distrobox rm "$CONTAINER_NAME" --force 2>/dev/null || true
fi

# ── Step 1: Container ────────────────────────────────────────────────────────
echo ">> [1/6] Setting up container..."
distrobox create --image archlinux "$CONTAINER_NAME" -Y

# ── Step 2: Dependencies ─────────────────────────────────────────────────────
echo ">> [2/6] Installing build dependencies..."
distrobox enter "$CONTAINER_NAME" -- bash -c '
    sudo sh -c "printf \"\n\n[multilib]\nInclude = /etc/pacman.d/mirrorlist\n\" >> /etc/pacman.conf"
    sudo pacman -Syu --noconfirm
    sudo pacman -S --needed --noconfirm \
        base-devel git gcc pkgconf python \
        rustup nodejs pnpm \
        webkit2gtk-4.1 gtk3 glib2 \
        libappindicator-gtk3 libayatana-appindicator \
        openssl curl wget python-pip xdg-utils
    rustup default nightly
    rustup update nightly
    echo "   Rust: $(rustc --version) | Node: $(node --version) | pnpm: $(pnpm --version)"
'

# ── Step 3: Source ───────────────────────────────────────────────────────────
echo ">> [3/6] Cloning source..."
distrobox enter "$CONTAINER_NAME" -- bash -c '
    rm -rf "$HOME/drop-app-build"
    git clone https://github.com/Drop-OSS/drop-app.git "$HOME/drop-app-build"
    cd "$HOME/drop-app-build"
    git checkout '"$REPO_TAG"'
    git submodule update --init --recursive
    echo "   Checked out: $(git describe --tags 2>/dev/null || git rev-parse --short HEAD)"
'

# ── Step 4: Patch ────────────────────────────────────────────────────────────
echo ">> [4/6] Applying patches..."
cat > "$HOME/drop-patch.py" << 'PYEOF'
path = "src-tauri/process/src/process_handlers.rs"
with open(path) as f:
    content = f.read()

# Patch 1: add ParsedCommand to imports
old_import = 'use crate::{error::ProcessError, process_manager::ProcessHandler};'
new_import = 'use crate::{error::ProcessError, parser::ParsedCommand, process_manager::ProcessHandler};'
if old_import in content:
    content = content.replace(old_import, new_import)
    print("  [OK] Added ParsedCommand import")
elif new_import in content:
    print("  [SKIP] ParsedCommand import already present")
else:
    print("  [WARN] Import line not found")

# Patch 2: fix WindowsLauncher to resolve absolute path (fixes spaces in path + Windows exe lookup)
old = '''        _current_dir: &str,
        _database: &Database,
    ) -> Result<String, ProcessError> {
        Ok(format!("cmd /C \\"{}\\"", launch_command))
    }

    fn valid_for_platform(&self, _db: &Database, _target: &Platform) -> bool {
        true
    }

    #[allow(unused_variables)]
    fn modify_command(&self, command: &mut Command) {'''
new = '''        current_dir: &str,
        _database: &Database,
    ) -> Result<String, ProcessError> {
        let mut parsed = ParsedCommand::parse(launch_command)?;
        parsed.make_absolute(PathBuf::from(current_dir));
        Ok(parsed.reconstruct())
    }

    fn valid_for_platform(&self, _db: &Database, _target: &Platform) -> bool {
        true
    }

    #[allow(unused_variables)]
    fn modify_command(&self, command: &mut Command) {'''
if old in content:
    content = content.replace(old, new)
    print("  [OK] WindowsLauncher patched (absolute path + space handling)")
elif 'parsed.make_absolute(PathBuf::from(current_dir))' in content:
    print("  [SKIP] WindowsLauncher already patched")
else:
    print("  [WARN] WindowsLauncher patch target not found")

with open(path, "w") as f:
    f.write(content)
PYEOF
distrobox enter "$CONTAINER_NAME" -- bash -c '
    cd "$HOME/drop-app-build"
    python3 "$HOME/drop-patch.py"
'
rm -f "$HOME/drop-patch.py"

# ── Step 5: JS deps ──────────────────────────────────────────────────────────
echo ">> [5/6] Installing JS dependencies..."
distrobox enter "$CONTAINER_NAME" -- bash -c '
    cd "$HOME/drop-app-build"
    pnpm install
'

# ── Step 6: Build ────────────────────────────────────────────────────────────
echo ">> [6/6] Building... (grab a coffee)"
distrobox enter "$CONTAINER_NAME" -- bash -c '
    cd "$HOME/drop-app-build"
    pnpm tauri build 2>&1 | tail -5
    sudo cp src-tauri/target/release/drop-app /usr/local/bin/drop-app
    sudo chmod +x /usr/local/bin/drop-app
    rm -rf "$HOME/drop-app-build"
    echo "   Binary installed."
'

# ── umu-launcher ─────────────────────────────────────────────────────────────
echo ">> Installing umu-launcher..."
distrobox enter "$CONTAINER_NAME" -- bash -c '
    if command -v umu-run &>/dev/null; then
        echo "   Already installed: $(umu-run --version 2>/dev/null)"
    else
        UMU_URL=$(curl -s "https://api.github.com/repos/Open-Wine-Components/umu-launcher/releases/latest" | \
            python3 -c "import sys,json; d=json.load(sys.stdin); print(next((a[\"browser_download_url\"] for a in d[\"assets\"] if \"zipapp.tar\" in a[\"name\"]), \"\"))" 2>/dev/null)
        if [ -n "$UMU_URL" ]; then
            wget -q -O /tmp/umu.tar "$UMU_URL"
            tar -xf /tmp/umu.tar -C /tmp/ 2>/dev/null
            UMU_BIN=$(find /tmp -maxdepth 3 -name "umu-run" -type f 2>/dev/null | head -1)
            if [ -n "$UMU_BIN" ]; then
                sudo cp "$UMU_BIN" /usr/local/bin/umu-run
                sudo chmod +x /usr/local/bin/umu-run
                rm -f /tmp/umu.tar
                echo "   Installed: $(umu-run --version 2>/dev/null)"
            else
                echo "   WARNING: umu-run not found in archive"
            fi
        else
            echo "   WARNING: Could not fetch umu-launcher URL"
        fi
    fi
'

# ── Desktop entry (host) ─────────────────────────────────────────────────────
echo ">> Creating desktop entry..."
mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/drop-app.desktop" << HOSTEOF
[Desktop Entry]
Name=Drop
Comment=Drop OSS Game Client
Exec=distrobox enter ${CONTAINER_NAME} -- /usr/local/bin/drop-app
Icon=applications-games
Type=Application
Categories=Game;
Terminal=false
StartupNotify=true
HOSTEOF
update-desktop-database "$HOME/.local/share/applications/" 2>/dev/null || true

# ── Export package for friends ────────────────────────────────────────────────
echo ">> Creating install package..."
rm -rf "$EXPORT_PATH"
mkdir -p "$EXPORT_PATH"

# Copy the compiled binary out of the container
distrobox enter "$CONTAINER_NAME" -- bash -c "cp /usr/local/bin/drop-app '$EXPORT_PATH/drop-app'"
chmod +x "$EXPORT_PATH/drop-app"

cat > "$EXPORT_PATH/install-drop-steamdeck.sh" << 'INSTALLEOF'
#!/bin/bash
# Drop OSS - Quick Install for Steam Deck
# Place this file next to the "drop-app" binary and run it from a terminal.

set -e
SCRIPT_DIR="$(dirname "$(readlink -f "$0")")"
CONTAINER_NAME="drop-app"
BINARY="$SCRIPT_DIR/drop-app"

echo "======================================"
echo " Drop OSS - Quick Install"
echo "======================================"
echo ""

if [ ! -f "$BINARY" ]; then
    echo "ERROR: 'drop-app' binary not found next to this script."
    echo "Make sure both files are in the same folder."
    exit 1
fi

if [ -f /run/.containerenv ] || [ -f /.dockerenv ]; then
    echo "ERROR: Run this from the SteamOS host, not inside a distrobox."
    exit 1
fi

echo ">> [1/4] Setting up container..."
if distrobox list 2>/dev/null | grep -q "$CONTAINER_NAME"; then
    distrobox rm "$CONTAINER_NAME" --force 2>/dev/null || true
fi
distrobox create --image archlinux "$CONTAINER_NAME" -Y

echo ">> [2/4] Installing runtime libraries..."
distrobox enter "$CONTAINER_NAME" -- bash -c '
    sudo pacman -Syu --noconfirm
    sudo pacman -S --needed --noconfirm \
        webkit2gtk-4.1 gtk3 glib2 \
        libappindicator-gtk3 libayatana-appindicator \
        openssl curl wget python python-pip
'

echo ">> [3/4] Installing Drop binary and umu-launcher..."
distrobox enter "$CONTAINER_NAME" -- bash -c "
    sudo cp '$BINARY' /usr/local/bin/drop-app
    sudo chmod +x /usr/local/bin/drop-app

    UMU_URL=\$(curl -s 'https://api.github.com/repos/Open-Wine-Components/umu-launcher/releases/latest' | \
        python3 -c \"import sys,json; d=json.load(sys.stdin); print(next((a['browser_download_url'] for a in d['assets'] if 'zipapp.tar' in a['name']), ''))\" 2>/dev/null)
    if [ -n \"\$UMU_URL\" ]; then
        wget -q -O /tmp/umu.tar \"\$UMU_URL\"
        tar -xf /tmp/umu.tar -C /tmp/ 2>/dev/null
        UMU_BIN=\$(find /tmp -maxdepth 3 -name 'umu-run' -type f 2>/dev/null | head -1)
        if [ -n \"\$UMU_BIN\" ]; then
            sudo cp \"\$UMU_BIN\" /usr/local/bin/umu-run
            sudo chmod +x /usr/local/bin/umu-run
            rm -f /tmp/umu.tar
            echo '   umu-launcher installed.'
        fi
    fi
"

echo ">> [4/4] Creating desktop entry..."
mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/drop-app.desktop" << DESKTOPEOF
[Desktop Entry]
Name=Drop
Comment=Drop OSS Game Client
Exec=distrobox enter $CONTAINER_NAME -- /usr/local/bin/drop-app
Icon=applications-games
Type=Application
Categories=Game;
Terminal=false
StartupNotify=true
DESKTOPEOF
update-desktop-database "$HOME/.local/share/applications/" 2>/dev/null || true

echo ""
echo "======================================"
echo " Drop installed!"
echo " Launch from your application menu."
echo "======================================"
INSTALLEOF
chmod +x "$EXPORT_PATH/install-drop-steamdeck.sh"

echo ""
echo "======================================"
echo " Done!"
echo "======================================"
echo ""
echo " Launch Drop:"
echo "   distrobox enter ${CONTAINER_NAME} -- /usr/local/bin/drop-app"
echo "   (or find it in your application menu)"
echo ""
echo " Share with friends — give them this folder:"
echo "   $EXPORT_PATH/"
echo "   ├── drop-app               (pre-built binary)"
echo "   └── install-drop-steamdeck.sh"
echo ""
echo " Friends run:  bash install-drop-steamdeck.sh"
echo " (No compiling — installs in ~5 minutes)"
echo ""
