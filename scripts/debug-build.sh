#!/bin/bash
set -euo pipefail
export RUSTUP_HOME=/root/.cache/drop-flatpak-build/rustup
export CARGO_HOME=/root/.cache/drop-flatpak-build/cargo
NVM_DIR=/root/.cache/drop-flatpak-build/nvm
NODE_VER=$(ls "$NVM_DIR/versions/node/" 2>/dev/null | sort -V | tail -1)
NODE_BIN="$NVM_DIR/versions/node/$NODE_VER/bin"
PNPM_PREFIX=/root/.cache/drop-flatpak-build/pnpm
export PATH="$CARGO_HOME/bin:$NODE_BIN:$PNPM_PREFIX/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

cat > /root/sdk-debug.sh << 'INNER'
#!/bin/bash
set -euo pipefail
export RUSTUP_HOME=/root/.cache/drop-flatpak-build/rustup
export CARGO_HOME=/root/.cache/drop-flatpak-build/cargo
NVM_DIR=/root/.cache/drop-flatpak-build/nvm
NODE_VER=$(ls "$NVM_DIR/versions/node/" | sort -V | tail -1)
NODE_BIN="$NVM_DIR/versions/node/$NODE_VER/bin"
PNPM_PREFIX=/root/.cache/drop-flatpak-build/pnpm
export PATH="$CARGO_HOME/bin:$NODE_BIN:$PNPM_PREFIX/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
cd /root/.cache/drop-flatpak-build/src
pnpm tauri build 2>&1
INNER
chmod +x /root/sdk-debug.sh

flatpak run \
    --command=bash \
    --share=network \
    --filesystem=home \
    org.gnome.Sdk//47 \
    /root/sdk-debug.sh 2>&1 | grep -v "^<3>WSL" | tee /root/drop-app-export/build-full.log | grep -E "error\[|^error |aborting|Finished|FAILED" | head -60
