#!/bin/bash
set -euo pipefail

echo "=== Drop Desktop Client — Steam Deck Build Script ==="
echo ""

# Steam Deck is read-only by default, need to unlock
echo "[1/7] Unlocking filesystem (requires sudo password)..."
sudo steamos-readonly disable

echo "[2/7] Installing system dependencies..."
sudo pacman -Syu --noconfirm --needed \
  base-devel \
  webkit2gtk-4.1 \
  libappindicator-gtk3 \
  librsvg \
  patchelf \
  xdg-utils \
  openssl \
  fuse2 \
  file \
  pkgconf

echo "[3/7] Installing Rust nightly..."
if command -v rustup &> /dev/null; then
  rustup default nightly
  rustup update nightly
else
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly
  source "$HOME/.cargo/env"
fi

echo "[4/7] Installing Node.js + pnpm..."
if ! command -v node &> /dev/null; then
  curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
  export NVM_DIR="$HOME/.nvm"
  [ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"
  nvm install --lts
fi
if ! command -v pnpm &> /dev/null; then
  npm install -g pnpm
fi

echo "[5/7] Installing frontend dependencies..."
pnpm install

echo "[6/7] Building Drop Desktop Client (this takes a few minutes)..."
pnpm tauri build 2>&1 | tail -20

echo ""
echo "[7/7] Build complete!"
echo ""
echo "=== Output files ==="
RELEASE_DIR="src-tauri/target/release"
BUNDLE_DIR="$RELEASE_DIR/bundle"

if [ -f "$RELEASE_DIR/drop-app" ]; then
  echo "  Binary:   $RELEASE_DIR/drop-app"
  ls -lh "$RELEASE_DIR/drop-app"
fi
if ls "$BUNDLE_DIR"/appimage/*.AppImage 1>/dev/null 2>&1; then
  echo "  AppImage: $(ls $BUNDLE_DIR/appimage/*.AppImage)"
  ls -lh "$BUNDLE_DIR"/appimage/*.AppImage
fi

echo ""
echo "To install, copy the binary to a permanent location:"
echo "  mkdir -p ~/.local/bin"
echo "  cp $RELEASE_DIR/drop-app ~/.local/bin/"
echo ""
echo "Re-locking filesystem..."
sudo steamos-readonly enable
echo "Done!"
